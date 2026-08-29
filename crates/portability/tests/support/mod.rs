//! Deterministic synthetic profile used by every portability test.
//!
//! Every identifier, timestamp, artifact byte, and signing key is fixed, so two
//! fixtures built on different hosts hold byte-identical canonical state. That
//! is what makes the cross-platform export determinism claim testable rather
//! than merely asserted.

#![allow(dead_code)]

use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim,
    ClaimId, ClaimObject, ClaimRelation, ClaimRelationKind, Confidentiality, ContentDigest,
    DecisionAction, DecisionId, DeviceId, DomainError, DomainId, EVENT_SCHEMA_VERSION, EntityId,
    EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem, EvidenceLocator,
    EvidenceRole, EvidenceStrength, MediaType, PermissionLineageId, PredicateId, ResolutionSlot,
    RetentionClass, ScopeDescriptor, ScopeId, TimestampMillis, UnsignedBatch, UserDecision,
    ValidInterval,
};
use academic_portability::restore::ProjectionRebuildTarget;
use academic_projections::{
    generation::{ProjectionCoordinates, ProjectionKind},
    resolution::{AuthorityPolicy, PredicatePolicies},
    runner::ProjectionRunner,
};
use academic_store::{
    connection::open_reader,
    idempotency::AcceptanceCommand,
    path_policy::NativePathProbe,
    profile::{SyntheticProfile, create_synthetic_profile},
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring, Vault};
use ed25519_dalek::SigningKey;

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Locator key used only by disposable synthetic profiles.
pub const DOMAIN_KEY: &[u8] = b"phase-1-portability-synthetic-locator-key";
/// Fixed Ed25519 seed for the single synthetic device.
pub const SIGNING_SEED: [u8; 32] = [0x51; 32];
/// Fixed build digest recorded by the synthetic profile.
pub const BUILD_DIGEST: [u8; 32] = [0xb1; 32];
/// Exact bytes of the first synthetic artifact.
pub const FIRST_ARTIFACT_BYTES: &[u8] = b"synthetic portability artifact one\n";
/// Exact bytes of the second synthetic artifact.
pub const SECOND_ARTIFACT_BYTES: &[u8] = b"synthetic portability artifact two\n";
/// Versioned policy registry used by every projection rebuild in these tests.
pub const POLICY_REGISTRY_VERSION: &str = "portability-test-policies-v1";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native path
/// facade refuses to follow a link component, so the tests address the real
/// directory. This mirrors `crates/daemon/tests/support`.
#[cfg(unix)]
fn temporary_base() -> io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

/// Owner of one disposable temporary tree removed on drop.
#[derive(Debug)]
pub struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    /// Reserves and creates a unique temporary root.
    pub fn new(label: &str) -> TestResult<Self> {
        let label = sanitize(label);
        for _ in 0..64 {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "system clock is before the Unix epoch")?
                .as_nanos();
            let path = temporary_base()?.join(format!(
                "acad-b1-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not reserve a unique portability test root".into())
    }

    /// Returns the temporary root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns a child path below the root.
    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("test cleanup failed for {}: {error}", self.path.display());
        }
    }
}

/// A complete synthetic profile with a fixed canonical corpus.
#[derive(Debug)]
pub struct Fixture {
    root: TestRoot,
    profile_root: PathBuf,
    profile: SyntheticProfile,
    signing_key: SigningKey,
    authorization: DeviceAuthorization,
    next_origin_seq: u64,
    previous_batch_hash: Option<ContentDigest>,
    revision: u64,
    batch_counter: u64,
    accepted_at: i64,
    known_at_accept_seq: u64,
}

impl Fixture {
    /// Creates a profile and accepts the fixed four-batch synthetic corpus.
    pub fn new(label: &str) -> TestResult<Self> {
        let root = TestRoot::new(label)?;
        let profile_root = root.child("profile");
        let profile =
            create_synthetic_profile(&profile_root, &NativePathProbe::default(), BUILD_DIGEST)?;
        let signing_key = SigningKey::from_bytes(&SIGNING_SEED);
        let authorization = DeviceAuthorization::new(
            id::<DeviceId>(0xd001)?,
            id::<EntityId>(0xd002)?,
            signing_key.verifying_key(),
        );
        let mut fixture = Self {
            root,
            profile_root,
            profile,
            signing_key,
            authorization,
            next_origin_seq: 1,
            previous_batch_hash: None,
            revision: 0,
            batch_counter: 0,
            accepted_at: 10_000,
            known_at_accept_seq: 0,
        };
        fixture.seed_corpus()?;
        Ok(fixture)
    }

    /// Returns the synthetic profile root.
    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }

    /// Returns the canonical database path.
    pub fn database_path(&self) -> &Path {
        self.profile.database_path()
    }

    /// Returns a work path beside the profile, inside the disposable test root.
    pub fn work_path(&self, name: &str) -> PathBuf {
        self.root.child(name)
    }

    /// Returns the disposable test root.
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// Returns a fresh keyring for the single synthetic security domain.
    pub fn keyring(&self) -> TestResult<DomainKeyring> {
        synthetic_keyring()
    }

    /// Returns the independent trust anchors a restore must be given.
    pub fn authorizations(&self) -> Vec<DeviceAuthorization> {
        vec![self.authorization.clone()]
    }

    /// Returns an authorization bound to a different device identity.
    pub fn foreign_authorization(&self) -> TestResult<DeviceAuthorization> {
        Ok(DeviceAuthorization::new(
            id::<DeviceId>(0xdfff)?,
            id::<EntityId>(0xd002)?,
            self.signing_key.verifying_key(),
        ))
    }

    /// Returns the single synthetic security domain.
    pub fn domain_id(&self) -> TestResult<DomainId> {
        Ok(synthetic_domain_id()?)
    }

    /// Returns the highest accepted acceptance sequence.
    pub const fn known_at_accept_seq(&self) -> u64 {
        self.known_at_accept_seq
    }

    /// Returns the versioned predicate policy registry used by projections.
    pub fn policies(&self) -> TestResult<PredicatePolicies> {
        synthetic_policies()
    }

    /// Returns the projection generations a restore must rebuild from empty.
    pub fn projection_targets(&self) -> TestResult<Vec<ProjectionRebuildTarget>> {
        synthetic_projection_targets(self.known_at_accept_seq)
    }

    /// Builds every projection generation in the source profile and returns its
    /// kind and canonical checksum, so a restored rebuild can be compared.
    pub fn source_projection_checksums(&self) -> TestResult<Vec<(String, String)>> {
        let sidecar = self
            .profile_root
            .join(academic_portability::restore::PROJECTION_SIDECAR_FILE);
        let reader = open_reader(self.profile.database_path())?;
        let runner = ProjectionRunner::open(
            &reader,
            &sidecar,
            projection_builder_digest(),
            projection_config_hash(),
        )?;
        let policies = self.policies()?;
        let mut checksums = Vec::new();
        for target in self.projection_targets()? {
            let receipt =
                runner.rebuild_at(target.kind, target.domain, target.coordinates, &policies)?;
            let checksum = receipt
                .metadata
                .canonical_checksum
                .ok_or("verified generation reported no canonical checksum")?;
            checksums.push((
                target.kind.as_str().to_owned(),
                hex_lower(checksum.as_bytes()),
            ));
        }
        Ok(checksums)
    }

    /// Accepts one extra claim so the canonical watermark advances.
    pub fn accept_additional_claim(&mut self, seed: u64) -> TestResult<()> {
        let domain_id = self.domain_id()?;
        let scope_id: ScopeId = id(0x0102)?;
        let evidence_id: EvidenceId = id(0x0402)?;
        let claim = text_claim(
            id(0x0f00 + seed)?,
            id(0x0fa0 + seed)?,
            "note.body",
            "additional synthetic note",
            scope_id,
            evidence_id,
        )?;
        self.accept(
            importer_actor(),
            domain_id,
            vec![EventPayload::ClaimAsserted(claim)],
        )
    }

    fn seed_corpus(&mut self) -> TestResult<()> {
        let domain_id = self.domain_id()?;
        let scope_id: ScopeId = id(0x0102)?;
        let keyring = self.keyring()?;
        let vault = Vault::open(&self.profile_root, keyring)?;

        let first = self.register_artifact(&vault, 0x0201, 0x0301, FIRST_ARTIFACT_BYTES)?;
        let second = self.register_artifact(&vault, 0x0202, 0x0302, SECOND_ARTIFACT_BYTES)?;
        let first_evidence: EvidenceId = id(0x0401)?;
        let second_evidence: EvidenceId = id(0x0402)?;

        self.accept(
            importer_actor(),
            domain_id,
            vec![
                EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: scope_id,
                    domain_id,
                    label: "synthetic portability scope".to_owned(),
                }),
                EventPayload::ArtifactRegistered(first.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    first_evidence,
                    &first.descriptor,
                    first.locator.clone(),
                )),
                EventPayload::ArtifactRegistered(second.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    second_evidence,
                    &second.descriptor,
                    second.locator.clone(),
                )),
            ],
        )?;
        drop(vault);

        let symbol_claim = text_claim(
            id(0x0501)?,
            id(0x0601)?,
            "code.symbol",
            "portability_export_symbol",
            scope_id,
            first_evidence,
        )?;
        let note_claim = text_claim(
            id(0x0502)?,
            id(0x0602)?,
            "note.body",
            "deterministic export keeps canonical bytes",
            scope_id,
            second_evidence,
        )?;
        let graph_claim = entity_claim(
            id(0x0503)?,
            id(0x0603)?,
            "graph.related",
            id(0x0604)?,
            scope_id,
            first_evidence,
        )?;
        let confirmed_claim = user_claim(
            id(0x0504)?,
            id(0x0605)?,
            "note.body",
            "user confirmed synthetic note",
            scope_id,
            second_evidence,
        )?;
        let confirmed_object = confirmed_claim.object.clone();
        self.accept(
            importer_actor(),
            domain_id,
            vec![
                EventPayload::ClaimAsserted(symbol_claim),
                EventPayload::ClaimAsserted(note_claim),
                EventPayload::ClaimAsserted(graph_claim),
            ],
        )?;
        self.accept(
            user_actor(self.authorization.user_id()),
            domain_id,
            vec![EventPayload::ClaimAsserted(confirmed_claim)],
        )?;

        self.accept(
            importer_actor(),
            domain_id,
            vec![EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: id(0x0502)?,
                target_claim_id: id(0x0501)?,
                kind: ClaimRelationKind::Supports,
                scope_id,
            })],
        )?;

        self.accept(
            user_actor(self.authorization.user_id()),
            domain_id,
            vec![EventPayload::DecisionRecorded(UserDecision {
                id: id::<DecisionId>(0x0701)?,
                target_claim_id: id(0x0504)?,
                target_object: confirmed_object,
                resolution_slot: ResolutionSlot {
                    subject_entity_id: id(0x0605)?,
                    predicate_id: PredicateId::parse("note.body")?,
                    scope_id,
                },
                action: DecisionAction::Confirm,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
                rationale_evidence_ids: vec![second_evidence],
                decided_at: TimestampMillis::new(400),
                reversible_until: Some(TimestampMillis::new(900)),
            })],
        )
    }

    fn register_artifact(
        &self,
        vault: &Vault,
        artifact_seed: u64,
        lineage_seed: u64,
        bytes: &[u8],
    ) -> TestResult<RegisteredArtifact> {
        let request = ArtifactIngestRequest::new(
            id::<ArtifactId>(artifact_seed)?,
            MediaType::parse("text/plain")?,
            self.domain_id()?,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            id::<PermissionLineageId>(lineage_seed)?,
        );
        let receipt = vault.ingest(&request, bytes)?;
        let mut descriptor = receipt.descriptor().clone();
        let locator = EvidenceLocator::TextBytes {
            source_digest: descriptor.content_digest,
            start: 0,
            end: descriptor.byte_length,
        };
        descriptor.evidence_representations = vec![ArtifactRepresentation {
            locator: locator.clone(),
            content_digest: descriptor.content_digest,
            byte_length: descriptor.byte_length,
        }];
        Ok(RegisteredArtifact {
            descriptor,
            locator,
        })
    }

    fn accept(
        &mut self,
        actor: Actor,
        domain_id: DomainId,
        payloads: Vec<EventPayload>,
    ) -> TestResult<()> {
        self.batch_counter = self
            .batch_counter
            .checked_add(1)
            .ok_or("synthetic batch counter overflow")?;
        let origin_start = self.next_origin_seq;
        let mut events = Vec::with_capacity(payloads.len());
        for (offset, payload) in payloads.into_iter().enumerate() {
            let offset = u64::try_from(offset)?;
            let origin_seq = origin_start
                .checked_add(offset)
                .ok_or("synthetic origin sequence overflow")?;
            let event = Event {
                id: id::<EventId>(
                    0x0e00_0000_u64
                        .checked_add(self.batch_counter << 8)
                        .and_then(|value| value.checked_add(offset))
                        .ok_or("synthetic event identifier overflow")?,
                )?,
                origin_seq,
                origin_observed_at: TimestampMillis::new(
                    1_000_i64
                        .checked_add(i64::try_from(origin_seq)?)
                        .ok_or("synthetic observation overflow")?,
                ),
                actor: actor.clone(),
                domain_id,
                payload,
            };
            event.validate()?;
            events.push(event);
        }
        let event_count = u64::try_from(events.len())?;
        let origin_end = origin_start
            .checked_add(event_count - 1)
            .ok_or("synthetic origin end overflow")?;
        let batch = UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: id::<BatchId>(
                0x0b00_0000_u64
                    .checked_add(self.batch_counter)
                    .ok_or("synthetic batch identifier overflow")?,
            )?,
            device_id: self.authorization.device_id(),
            origin_seq_start: origin_start,
            origin_seq_end: origin_end,
            previous_batch_hash: self.previous_batch_hash,
            origin_created_at: TimestampMillis::new(
                2_000_i64
                    .checked_add(i64::try_from(self.batch_counter)?)
                    .ok_or("synthetic creation overflow")?,
            ),
            events,
        };
        let envelope = sign_batch(&batch, &self.signing_key)?;
        let verified = verify_signed_batch(&envelope, &self.authorization)?;
        let keyring = self.keyring()?;
        let vault = Vault::open(&self.profile_root, keyring)?;
        let mut store = self.profile.open_acceptance_store()?;
        let outcome = store.accept_verified_batch(
            &verified,
            AcceptanceCommand {
                request_id: [u8::try_from(self.batch_counter % 251)?; 16],
                client_instance_id: [0x61; 16],
                idempotency_key: *ContentDigest::sha256(
                    &[
                        b"portability-fixture-request".as_slice(),
                        &self.batch_counter.to_be_bytes(),
                    ]
                    .concat(),
                )
                .as_bytes(),
                expected_revision: Some(self.revision),
                envelope_bytes: &envelope,
            },
            TimestampMillis::new(self.accepted_at),
            &vault,
        )?;
        drop(store);
        drop(vault);
        self.next_origin_seq = origin_end
            .checked_add(1)
            .ok_or("synthetic next origin overflow")?;
        self.previous_batch_hash = Some(outcome.receipt.envelope_hash);
        self.revision = outcome.receipt.committed_revision;
        self.known_at_accept_seq = outcome.receipt.accept_seq_end;
        self.accepted_at = self
            .accepted_at
            .checked_add(1)
            .ok_or("synthetic acceptance clock overflow")?;
        Ok(())
    }
}

/// Valid-time coordinate every projection rebuild in these tests evaluates at.
pub const PROJECTION_VALID_AT: i64 = 200;

/// Returns the single synthetic security domain used by every fixture.
pub fn synthetic_domain_id() -> Result<DomainId, DomainError> {
    id(0x0101)
}

/// Returns a fresh keyring for the single synthetic security domain.
///
/// A crash-harness child process reconstructs the same keyring from these fixed
/// constants rather than inheriting one from its parent.
pub fn synthetic_keyring() -> TestResult<DomainKeyring> {
    let mut keyring = DomainKeyring::new();
    keyring.insert(synthetic_domain_id()?, DOMAIN_KEY)?;
    Ok(keyring)
}

/// Returns the independent trust anchor bound to the fixed synthetic device.
pub fn synthetic_authorizations() -> TestResult<Vec<DeviceAuthorization>> {
    let signing_key = SigningKey::from_bytes(&SIGNING_SEED);
    Ok(vec![DeviceAuthorization::new(
        id::<DeviceId>(0xd001)?,
        id::<EntityId>(0xd002)?,
        signing_key.verifying_key(),
    )])
}

/// Returns the versioned predicate policy registry used by projections.
pub fn synthetic_policies() -> TestResult<PredicatePolicies> {
    Ok(PredicatePolicies::new(
        POLICY_REGISTRY_VERSION,
        [
            (
                PredicateId::parse("code.symbol")?,
                AuthorityPolicy::ImplementationObservation,
            ),
            (
                PredicateId::parse("note.body")?,
                AuthorityPolicy::ImplementationObservation,
            ),
            (
                PredicateId::parse("graph.related")?,
                AuthorityPolicy::ImplementationObservation,
            ),
        ],
    )?)
}

/// Returns the projection rebuild plan for one explicit acceptance watermark.
pub fn synthetic_projection_targets(
    known_at_accept_seq: u64,
) -> TestResult<Vec<ProjectionRebuildTarget>> {
    let domain = synthetic_domain_id()?;
    let coordinates = ProjectionCoordinates::new(
        known_at_accept_seq,
        TimestampMillis::new(PROJECTION_VALID_AT),
    );
    Ok(vec![
        ProjectionRebuildTarget {
            kind: ProjectionKind::Graph,
            domain,
            coordinates,
        },
        ProjectionRebuildTarget {
            kind: ProjectionKind::Unicode61,
            domain,
            coordinates,
        },
        ProjectionRebuildTarget {
            kind: ProjectionKind::Trigram,
            domain,
            coordinates,
        },
    ])
}

#[derive(Debug)]
struct RegisteredArtifact {
    descriptor: ArtifactDescriptor,
    locator: EvidenceLocator,
}

/// Fixed builder digest bound into every projection generation here.
pub fn projection_builder_digest() -> ContentDigest {
    ContentDigest::sha256(b"portability-test-projection-builder")
}

/// Fixed effective configuration hash bound into every projection generation.
pub fn projection_config_hash() -> ContentDigest {
    ContentDigest::sha256(b"portability-test-projection-config")
}

/// Returns the importer actor used by every synthetic ingest event.
pub fn importer_actor() -> Actor {
    Actor::Importer {
        name: "academic.portability.synthetic".to_owned(),
        version: "1.0.0".to_owned(),
    }
}

/// Returns the user actor bound to the fixture's authorized identity.
pub fn user_actor(user_id: EntityId) -> Actor {
    Actor::User { user_id }
}

fn evidence_item(
    id: EvidenceId,
    descriptor: &ArtifactDescriptor,
    locator: EvidenceLocator,
) -> EvidenceItem {
    EvidenceItem {
        id,
        artifact_id: descriptor.id,
        locator,
        excerpt_digest: descriptor.content_digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "academic.portability.synthetic".to_owned(),
        extractor_version: "1.0.0".to_owned(),
    }
}

fn text_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    text: &str,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
) -> TestResult<Claim> {
    build_claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Text(text.to_owned()),
        scope_id,
        evidence_id,
        AuthorityClass::DirectObservation,
        EpistemicStatus::CodeObserved,
    )
}

fn user_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    text: &str,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
) -> TestResult<Claim> {
    build_claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Text(text.to_owned()),
        scope_id,
        evidence_id,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )
}

fn entity_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    target: EntityId,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
) -> TestResult<Claim> {
    build_claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Entity(target),
        scope_id,
        evidence_id,
        AuthorityClass::DirectObservation,
        EpistemicStatus::CodeObserved,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    object: ClaimObject,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
) -> TestResult<Claim> {
    let claim = Claim {
        id: claim_id,
        subject_entity_id: subject,
        predicate_id: PredicateId::parse(predicate)?,
        object,
        scope_id,
        authority_class,
        epistemic_status,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
        evidence_ids: vec![evidence_id],
    };
    claim.validate()?;
    Ok(claim)
}

/// Constructs one deterministic UUIDv7-shaped identifier from a numeric seed.
pub fn id<T>(suffix: u64) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}

/// Encodes bytes as lowercase hexadecimal.
pub fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn sanitize(label: &str) -> String {
    let label = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if label.is_empty() {
        "case".to_owned()
    } else {
        label
    }
}
