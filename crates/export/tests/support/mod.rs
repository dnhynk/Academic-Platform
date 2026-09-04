//! The deterministic corpus every `P2-P1` fixture is built from.
//!
//! Two halves, and both are real rather than typed.
//!
//! **The canonical half** is a Phase 1 synthetic profile: signed batches are
//! accepted into it through the store's own acceptance path, artifacts are
//! sealed through the vault's own ingest, and the rows the bundle carries are
//! read back out of that database by `academic-portability`. A `SourceView`
//! assembled from a literal would have made the round trip a comparison of one
//! test value with itself.
//!
//! **The graduation half** is `P2-U3`'s own corpus, included by path exactly as
//! that crate includes `P2-U6`'s. A second transcription of a rule set, a
//! transcript and a profile would drift, and the audit the bundle records has
//! to be an audit the audit crate would recognise.
//!
//! # Two artifacts hold identical bytes, on purpose
//!
//! `academic_vault::VaultLocator::derive` is a function of the domain key, the
//! media type and the content digest — not of the artifact identifier. So
//! [`DUPLICATE_BYTES`] is registered twice under two identifiers, and the two
//! descriptors carry **one** locator. That is the `P2-A1` shape: a bundle that
//! addressed an original by its locator would publish one file where two
//! artifacts exist. `original_inclusion_is_user_selected_with_no_dangling_locator`
//! is the test that observes both survive.

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
    ValidInterval, engines::EngineVersion,
};
use academic_export::{
    ArtifactSource, BatchSource, ClaimSource, CopyrightNotice, DeviceHead, DomainRecord,
    DomainTerms, GitRef, OriginalInclusion, RecordedAudit, SensitivityLabel, SourceView,
    StoreIdentity, TermsRegister, Watermark, bundle::PostureBlock,
};
use academic_portability::verify::{
    CanonicalDatabase, CanonicalRows, canonical_json, read_artifact_descriptors,
    read_canonical_rows,
};
use academic_store::{
    idempotency::AcceptanceCommand,
    path_policy::NativePathProbe,
    profile::{SyntheticProfile, create_synthetic_profile},
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring, Vault};
use ed25519_dalek::SigningKey;

#[path = "../../../audit/tests/support/mod.rs"]
// `P2-U3`'s fixture module is written for that crate's own suite and offers
// more than this one uses, exactly as it does for `P2-U6`'s.
pub mod audit_support;

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Locator key used only by disposable synthetic profiles.
pub const DOMAIN_KEY: &[u8] = b"p2-p1-graduation-export-synthetic-locator-key";
/// The second security domain's locator key.
///
/// There are **two** domains on purpose. Content files are written per domain,
/// so a corpus with one domain makes the order they are written in
/// unobservable: `P1-I5` replaced the sorted domain list with a hash set and
/// every determinism assertion still passed, because one element is in order
/// whatever the container. The second domain is what gives that assertion
/// something to be about.
pub const SECOND_DOMAIN_KEY: &[u8] = b"p2-p1-graduation-export-second-locator-key";
/// Fixed Ed25519 seed for the single synthetic device.
pub const SIGNING_SEED: [u8; 32] = [0x71; 32];
/// Fixed build digest recorded by the synthetic profile.
pub const BUILD_DIGEST: [u8; 32] = [0xb2; 32];
/// Bytes of the second domain's original.
pub const SECOND_DOMAIN_BYTES: &[u8] = b"synthetic second-domain original\n";
/// Bytes of the lecture original.
pub const LECTURE_BYTES: &[u8] = b"synthetic lecture capture original\n";
/// Bytes registered twice, under two artifact identifiers.
///
/// The vault derives a locator from the domain key, the media type and the
/// content digest, so the two descriptors share one locator and one object
/// file. See the module docs.
pub const DUPLICATE_BYTES: &[u8] = b"synthetic repository snapshot original\n";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
fn temporary_base() -> io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

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
    pub fn new(label: &str) -> TestResult<Self> {
        let label = sanitize(label);
        for _ in 0..64 {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "system clock is before the Unix epoch")?
                .as_nanos();
            let path = temporary_base()?.join(format!(
                "acad-p1-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not reserve a unique export test root".into())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// One synthetic profile, its Phase 1 export directory, and the rows it holds.
#[derive(Debug)]
pub struct Fixture {
    root: TestRoot,
    profile_root: PathBuf,
    export_root: PathBuf,
    profile: SyntheticProfile,
    signing_key: SigningKey,
    authorization: DeviceAuthorization,
    next_origin_seq: u64,
    previous_batch_hash: Option<ContentDigest>,
    revision: u64,
    batch_counter: u64,
    accepted_at: i64,
}

impl Fixture {
    /// Creates a profile, accepts the corpus, and publishes a Phase 1 export.
    ///
    /// The Phase 1 export is where the exact object bytes and the exact signed
    /// envelopes are readable as files, which is what a graduation bundle
    /// copies. Producing it here rather than reading the vault directly keeps
    /// the byte-for-byte envelope claim standing on the format that already
    /// makes it.
    pub fn new(label: &str) -> TestResult<Self> {
        let root = TestRoot::new(label)?;
        let profile_root = root.child("profile");
        let profile =
            create_synthetic_profile(&profile_root, &NativePathProbe::default(), BUILD_DIGEST)?;
        let signing_key = SigningKey::from_bytes(&SIGNING_SEED);
        let authorization = DeviceAuthorization::new(
            id::<DeviceId>(0xd101)?,
            id::<EntityId>(0xd102)?,
            signing_key.verifying_key(),
        );
        let mut fixture = Self {
            root,
            profile_root,
            export_root: PathBuf::new(),
            profile,
            signing_key,
            authorization,
            next_origin_seq: 1,
            previous_batch_hash: None,
            revision: 0,
            batch_counter: 0,
            accepted_at: 20_000,
        };
        fixture.seed_corpus()?;
        let export_root = fixture.root.child("phase1-export");
        academic_portability::export::export_profile(
            &fixture.profile_root,
            &export_root,
            fixture.keyring()?,
        )?;
        fixture.export_root = export_root;
        Ok(fixture)
    }

    #[must_use]
    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }

    #[must_use]
    pub fn export_root(&self) -> &Path {
        &self.export_root
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    #[must_use]
    pub fn work_path(&self, name: &str) -> PathBuf {
        self.root.child(name)
    }

    pub fn keyring(&self) -> TestResult<DomainKeyring> {
        let mut keyring = DomainKeyring::new();
        keyring.insert(domain_id()?, DOMAIN_KEY)?;
        keyring.insert(second_domain_id()?, SECOND_DOMAIN_KEY)?;
        Ok(keyring)
    }

    /// Reads the canonical rows straight out of the database.
    ///
    /// This is the round trip's independent side: the bundle is written from a
    /// view and read back into records, and what those records are compared
    /// with is what the store itself still holds.
    pub fn canonical_rows(&self) -> TestResult<CanonicalRows> {
        let database = CanonicalDatabase::open_source(
            &self.profile_root.join(academic_store::STORE_DATABASE_FILE),
        )?;
        Ok(read_canonical_rows(&database)?)
    }

    /// Assembles the source view a bundle is written from.
    pub fn source_view(&self) -> TestResult<SourceView> {
        let rows = self.canonical_rows()?;
        let database = CanonicalDatabase::open_source(
            &self.profile_root.join(academic_store::STORE_DATABASE_FILE),
        )?;
        let descriptors = read_artifact_descriptors(&database)?;
        let domain = domain_id()?.to_string();

        let mut batches = Vec::new();
        for batch in &rows.batches {
            batches.push(BatchSource::new(
                batch.batch_id.clone(),
                domain.clone(),
                batch.envelope_sha256.clone(),
                batch.envelope_byte_length,
                self.export_root
                    .join("ledger")
                    .join("batches")
                    .join(format!("{}.cbor", batch.batch_id)),
                text(canonical_json(batch)?)?,
            )?);
        }

        let mut events = Vec::new();
        for event in &rows.events {
            events.push(DomainRecord::new(
                event.event_id.clone(),
                event.domain_id.clone(),
                text(canonical_json(event)?)?,
            )?);
        }
        events.sort_by(|left, right| left.id().cmp(right.id()));

        let mut scopes = Vec::new();
        for scope in &rows.scopes {
            scopes.push(DomainRecord::new(
                scope.scope_id.clone(),
                scope.domain_id.clone(),
                text(canonical_json(scope)?)?,
            )?);
        }

        let mut artifacts = Vec::new();
        for row in &rows.artifacts {
            let descriptor = descriptors
                .iter()
                .find(|candidate| candidate.id.to_string() == row.artifact_id)
                .ok_or("an exported artifact row has no descriptor")?;
            artifacts.push(ArtifactSource::new(
                row.artifact_id.clone(),
                row.domain_id.clone(),
                confidentiality(&row.confidentiality)?,
                row.content_digest.clone(),
                row.byte_length,
                row.media_type.clone(),
                row.vault_locator.clone(),
                self.export_root
                    .join("objects")
                    .join(&row.domain_id)
                    .join(format!("{}.bin", row.artifact_id)),
                text(canonical_json(row)?)?,
            )?);
            let _ = descriptor;
        }

        let scope_domain = |scope_id: &str| -> String {
            rows.scopes
                .iter()
                .find(|scope| scope.scope_id == scope_id)
                .map_or_else(|| domain.clone(), |scope| scope.domain_id.clone())
        };
        let artifact_domain = |artifact_id: &str| -> String {
            rows.artifacts
                .iter()
                .find(|artifact| artifact.artifact_id == artifact_id)
                .map_or_else(|| domain.clone(), |artifact| artifact.domain_id.clone())
        };

        let mut evidence = Vec::new();
        for item in &rows.evidence {
            evidence.push(DomainRecord::new(
                item.evidence_id.clone(),
                artifact_domain(&item.artifact_id),
                text(canonical_json(item)?)?,
            )?);
        }

        let mut claims = Vec::new();
        for claim in &rows.claims {
            claims.push(ClaimSource::new(
                claim.claim_id.clone(),
                scope_domain(&claim.scope_id),
                claim.predicate_id.clone(),
                claim.evidence_ids.clone(),
                text(canonical_json(claim)?)?,
            )?);
        }

        let mut relations = Vec::new();
        for relation in &rows.relations {
            relations.push(DomainRecord::new(
                relation.relation_event_id.clone(),
                scope_domain(&relation.scope_id),
                text(canonical_json(relation)?)?,
            )?);
        }
        relations.sort_by(|left, right| left.id().cmp(right.id()));

        let mut decisions = Vec::new();
        for decision in &rows.decisions {
            decisions.push(DomainRecord::new(
                decision.decision_id.clone(),
                scope_domain(&decision.resolution_scope_id),
                text(canonical_json(decision)?)?,
            )?);
        }

        let device_heads = rows
            .device_heads
            .iter()
            .map(|head| DeviceHead {
                device_id: head.device_id.clone(),
                next_origin_seq: head.next_origin_seq,
                head_envelope_sha256: head.head_envelope_sha256.clone(),
            })
            .collect();

        Ok(SourceView {
            store: StoreIdentity {
                format_uuid: rows.schema.format_uuid.clone(),
                schema_version: rows.schema.schema_version,
                schema_semver: rows.schema.schema_semver.clone(),
            },
            watermark: Watermark {
                next_accept_seq: rows.watermark.next_accept_seq,
                profile_revision: rows.watermark.profile_revision,
                accept_seq_head: rows.watermark.accept_seq_head,
                outbox_head: rows.watermark.outbox_head,
            },
            device_heads,
            canonical_semantic_digest: hex_lower(rows.semantic_digest()?.as_bytes().as_slice()),
            batches,
            events,
            scopes,
            artifacts,
            evidence,
            claims,
            relations,
            decisions,
            git_refs: git_refs()?,
        })
    }

    fn seed_corpus(&mut self) -> TestResult<()> {
        let domain = domain_id()?;
        let scope_id: ScopeId = id(0x0202)?;
        let keyring = self.keyring()?;
        let vault = Vault::open(&self.profile_root, keyring)?;

        let lecture = self.register_artifact(&vault, 0x0301, 0x0401, LECTURE_BYTES)?;
        // Registered twice with identical bytes and one media type, so both
        // descriptors derive the same vault locator.
        let first_snapshot = self.register_artifact(&vault, 0x0302, 0x0402, DUPLICATE_BYTES)?;
        let second_snapshot = self.register_artifact(&vault, 0x0303, 0x0403, DUPLICATE_BYTES)?;
        let lecture_evidence: EvidenceId = id(0x0501)?;
        let first_evidence: EvidenceId = id(0x0502)?;
        let second_evidence: EvidenceId = id(0x0503)?;

        self.accept(
            importer_actor()?,
            domain,
            vec![
                EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: scope_id,
                    domain_id: domain,
                    label: "synthetic graduation export scope".to_owned(),
                }),
                EventPayload::ArtifactRegistered(lecture.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    lecture_evidence,
                    &lecture.descriptor,
                    lecture.locator.clone(),
                )),
                EventPayload::ArtifactRegistered(first_snapshot.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    first_evidence,
                    &first_snapshot.descriptor,
                    first_snapshot.locator.clone(),
                )),
                EventPayload::ArtifactRegistered(second_snapshot.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    second_evidence,
                    &second_snapshot.descriptor,
                    second_snapshot.locator.clone(),
                )),
            ],
        )?;
        drop(vault);

        // One claim per section 37 topical part, plus one whose predicate names
        // no topic. Without all six the part coverage test would be measuring a
        // corpus rather than the writer.
        let official = text_claim(
            id(0x0601)?,
            id(0x0701)?,
            "course.final.grade",
            "CSE300 A0",
            scope_id,
            lecture_evidence,
        )?;
        let archive = text_claim(
            id(0x0602)?,
            id(0x0702)?,
            "lecture.segment.topic",
            "virtual memory, working set",
            scope_id,
            lecture_evidence,
        )?;
        let competency = text_claim(
            id(0x0603)?,
            id(0x0703)?,
            "concept.mastery.evidence",
            "applied paging in a systems project",
            scope_id,
            first_evidence,
        )?;
        let repository = text_claim(
            id(0x0604)?,
            id(0x0704)?,
            "repository.architecture.note",
            "the cache invalidation boundary moved",
            scope_id,
            second_evidence,
        )?;
        let role = text_claim(
            id(0x0605)?,
            id(0x0705)?,
            "role.interest.change",
            "opened a systems path beside backend",
            scope_id,
            second_evidence,
        )?;
        let untopical = entity_claim(
            id(0x0606)?,
            id(0x0706)?,
            "graph.related",
            id(0x0707)?,
            scope_id,
            lecture_evidence,
        )?;
        let confirmed = user_claim(
            id(0x0607)?,
            id(0x0708)?,
            "role.alternative.path",
            "backend path kept open and not marked failed",
            scope_id,
            second_evidence,
        )?;
        let confirmed_object = confirmed.object.clone();

        self.accept(
            importer_actor()?,
            domain,
            vec![
                EventPayload::ClaimAsserted(official),
                EventPayload::ClaimAsserted(archive),
                EventPayload::ClaimAsserted(competency),
                EventPayload::ClaimAsserted(repository),
                EventPayload::ClaimAsserted(role),
                EventPayload::ClaimAsserted(untopical),
            ],
        )?;
        self.accept(
            user_actor(self.authorization.user_id()),
            domain,
            vec![EventPayload::ClaimAsserted(confirmed)],
        )?;
        self.accept(
            importer_actor()?,
            domain,
            vec![EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: id(0x0603)?,
                target_claim_id: id(0x0604)?,
                kind: ClaimRelationKind::Supports,
                scope_id,
            })],
        )?;
        // A second security domain, so the order domains are written in is
        // observable. See `SECOND_DOMAIN_KEY`.
        let second = second_domain_id()?;
        let second_scope: ScopeId = id(0x0203)?;
        let keyring = self.keyring()?;
        let vault = Vault::open(&self.profile_root, keyring)?;
        // Deliberately a weaker confidentiality than the first domain's, so the
        // two domains produce two labels, two restrictions and two notices.
        let second_artifact = self.register_artifact_in(
            &vault,
            second,
            Confidentiality::Personal,
            0x0304,
            0x0404,
            SECOND_DOMAIN_BYTES,
        )?;
        drop(vault);
        let second_domain_evidence: EvidenceId = id(0x0504)?;
        self.accept(
            importer_actor()?,
            second,
            vec![
                EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: second_scope,
                    domain_id: second,
                    label: "synthetic second security domain".to_owned(),
                }),
                EventPayload::ArtifactRegistered(second_artifact.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    second_domain_evidence,
                    &second_artifact.descriptor,
                    second_artifact.locator.clone(),
                )),
            ],
        )?;
        self.accept(
            importer_actor()?,
            second,
            vec![EventPayload::ClaimAsserted(text_claim(
                id(0x0608)?,
                id(0x0709)?,
                "concept.mastery.evidence",
                "a second domain carries its own terms",
                second_scope,
                second_domain_evidence,
            )?)],
        )?;

        self.accept(
            user_actor(self.authorization.user_id()),
            domain,
            vec![EventPayload::DecisionRecorded(UserDecision {
                id: id::<DecisionId>(0x0801)?,
                target_claim_id: id(0x0607)?,
                target_object: confirmed_object,
                resolution_slot: ResolutionSlot {
                    subject_entity_id: id(0x0708)?,
                    predicate_id: PredicateId::parse("role.alternative.path")?,
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
        self.register_artifact_in(
            vault,
            domain_id()?,
            Confidentiality::Restricted,
            artifact_seed,
            lineage_seed,
            bytes,
        )
    }

    fn register_artifact_in(
        &self,
        vault: &Vault,
        domain: DomainId,
        confidentiality: Confidentiality,
        artifact_seed: u64,
        lineage_seed: u64,
        bytes: &[u8],
    ) -> TestResult<RegisteredArtifact> {
        let request = ArtifactIngestRequest::new(
            id::<ArtifactId>(artifact_seed)?,
            MediaType::parse("text/plain")?,
            domain,
            confidentiality,
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
                client_instance_id: [0x62; 16],
                idempotency_key: *ContentDigest::sha256(
                    &[
                        b"graduation-export-fixture-request".as_slice(),
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
        self.accepted_at = self
            .accepted_at
            .checked_add(1)
            .ok_or("synthetic acceptance clock overflow")?;
        Ok(())
    }
}

#[derive(Debug)]
struct RegisteredArtifact {
    descriptor: ArtifactDescriptor,
    locator: EvidenceLocator,
}

/// The graduation audit the bundle records, and what a re-run needs.
#[derive(Debug)]
pub struct Graduation {
    pub rules: academic_requirement::RuleSet,
    pub inputs: academic_domain::engines::FrozenInputs,
    pub scope: academic_audit::RuleSetScope,
    pub audit: academic_audit::DegreeAudit,
    pub engine_version: EngineVersion,
}

impl Graduation {
    /// Runs `P2-U3`'s baseline case against the real engine.
    pub fn baseline() -> TestResult<Self> {
        let rules = audit_support::baseline_rules()?;
        let facts = audit_support::audit_facts(
            audit_support::transcript()?,
            audit_support::sources(&rules)?,
            Vec::new(),
            Some(audit_support::FRESHNESS),
        )?;
        let inputs = academic_audit::encode(&facts)?;
        let scope = audit_support::scope()?;
        let catalog = audit_support::catalog(&rules)?;
        let selection = academic_audit::select(&facts.profile, &catalog);
        let selected = selection
            .selected()
            .ok_or("the baseline profile selected no rule set")?
            .clone();
        let engine_version = EngineVersion::new(1)?;
        let engine = academic_audit::GraduationAuditEngine::new(selected, engine_version);
        let audit = academic_audit::DegreeAudit::evaluate(&engine, &inputs)?;
        Ok(Self {
            rules,
            inputs,
            scope,
            audit,
            engine_version,
        })
    }

    /// The same corpus with the transcript whose exchange attempt no dated
    /// policy row reaches, which reaches a different verdict.
    ///
    /// Used to build a bundle whose recorded outcome is internally consistent
    /// -- every digest matches, the reader accepts every file -- and does not
    /// belong to the frozen inputs beside it. Nothing but the re-run notices,
    /// which is what makes the re-run load-bearing.
    pub fn with_other_inputs(&self) -> TestResult<Self> {
        let rules = audit_support::baseline_rules()?;
        let facts = audit_support::audit_facts(
            audit_support::transcript_with_undated_external()?,
            audit_support::sources(&rules)?,
            Vec::new(),
            Some(audit_support::FRESHNESS),
        )?;
        let inputs = academic_audit::encode(&facts)?;
        let scope = audit_support::scope()?;
        let catalog = audit_support::catalog(&rules)?;
        let selection = academic_audit::select(&facts.profile, &catalog);
        let selected = selection
            .selected()
            .ok_or("the alternate profile selected no rule set")?
            .clone();
        let engine_version = EngineVersion::new(1)?;
        let engine = academic_audit::GraduationAuditEngine::new(selected, engine_version);
        let audit = academic_audit::DegreeAudit::evaluate(&engine, &inputs)?;
        Ok(Self {
            rules,
            inputs,
            scope,
            audit,
            engine_version,
        })
    }

    /// The recorded-audit half of a bundle request.
    #[must_use]
    pub fn recorded(&self) -> RecordedAudit<'_> {
        RecordedAudit {
            engine_version: self.engine_version,
            inputs: &self.inputs,
            rules: &self.rules,
            scope: &self.scope,
            audit: &self.audit,
        }
    }
}

/// The instant every bundle in this suite records.
pub const GENERATED_AT_UNIX_MS: i64 = 1_772_200_000_000;

/// The bundle-level source copyright notice.
pub const BUNDLE_NOTICE: &str =
    "Synthetic fixture bundle. Generated records are the exporting build's own.";
/// The security domain's source copyright notice.
pub const DOMAIN_NOTICE: &str =
    "Synthetic lecture material. Teaching use retained under the source institution's terms.";

/// The second domain's source copyright notice, which is a different string.
pub const SECOND_DOMAIN_NOTICE: &str =
    "Synthetic second-domain material. Held under a second source's own terms.";

/// The terms register every bundle in this suite is written under.
pub fn terms() -> TestResult<TermsRegister> {
    Ok(TermsRegister::new(CopyrightNotice::new(BUNDLE_NOTICE)?)
        .with_domain(
            domain_id()?.to_string(),
            DomainTerms::new(
                SensitivityLabel::Restricted,
                CopyrightNotice::new(DOMAIN_NOTICE)?,
            ),
        )
        .with_domain(
            second_domain_id()?.to_string(),
            DomainTerms::new(
                SensitivityLabel::Personal,
                CopyrightNotice::new(SECOND_DOMAIN_NOTICE)?,
            ),
        ))
}

/// A register declaring the domain weaker than its own artifacts are.
///
/// The fixture's artifacts are `RESTRICTED`, so a register saying `PERSONAL`
/// is a declaration a recipient would read while the ledger said something
/// stronger. The writer refuses it.
pub fn terms_understating_the_domain() -> TestResult<TermsRegister> {
    Ok(TermsRegister::new(CopyrightNotice::new(BUNDLE_NOTICE)?)
        .with_domain(
            domain_id()?.to_string(),
            DomainTerms::new(
                SensitivityLabel::Personal,
                CopyrightNotice::new(DOMAIN_NOTICE)?,
            ),
        )
        .with_domain(
            second_domain_id()?.to_string(),
            DomainTerms::new(
                SensitivityLabel::Personal,
                CopyrightNotice::new(SECOND_DOMAIN_NOTICE)?,
            ),
        ))
}

/// The posture a Phase 1 synthetic profile is under.
///
/// Read from `academic-portability`'s own frozen block rather than retyped, so
/// a posture change there is a compile or a comparison failure here instead of
/// two blocks that quietly disagree.
pub fn posture() -> PostureBlock {
    let phase1 = academic_portability::verify::PolicyBlock::phase1();
    PostureBlock {
        data_policy: phase1.data_policy.clone(),
        storage_mode: phase1.storage_mode.clone(),
        storage_encryption: phase1.storage_encryption.clone(),
        production_data_allowed: phase1.production_data_allowed,
        product_network: phase1.product_network.clone(),
    }
}

/// One version-control reference, so the repository part carries git refs.
pub fn git_refs() -> TestResult<Vec<GitRef>> {
    Ok(vec![GitRef {
        repository_id: "synthetic-repository".to_owned(),
        snapshot_id: "synthetic-snapshot-0001".to_owned(),
        domain_id: domain_id()?.to_string(),
        branch: Some("main".to_owned()),
        commit: Some("0f1e2d3c4b5a69788796a5b4c3d2e1f001234567".to_owned()),
        parent_snapshots: vec!["synthetic-snapshot-0000".to_owned()],
        submodules: vec![(
            "vendor/toolkit".to_owned(),
            "9876543210fedcba9876543210fedcba98765432".to_owned(),
        )],
    }])
}

/// The first synthetic security domain.
pub fn domain_id() -> Result<DomainId, DomainError> {
    id(0x0201)
}

/// The second synthetic security domain.
pub fn second_domain_id() -> Result<DomainId, DomainError> {
    id(0x0204)
}

/// Returns the importer actor.
pub fn importer_actor() -> TestResult<Actor> {
    Ok(Actor::Importer {
        name: "academic.export.synthetic".to_owned(),
        version: "1.0.0".to_owned(),
    })
}

/// Returns the user actor bound to the fixture's authorized identity.
#[must_use]
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
        extraction_method: "academic.export.synthetic".to_owned(),
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

fn confidentiality(value: &str) -> TestResult<Confidentiality> {
    match value {
        "PUBLIC" => Ok(Confidentiality::Public),
        "PERSONAL" => Ok(Confidentiality::Personal),
        "RESTRICTED" => Ok(Confidentiality::Restricted),
        "SECRET" => Ok(Confidentiality::Secret),
        other => Err(format!("unknown confidentiality {other}").into()),
    }
}

fn text(bytes: Vec<u8>) -> TestResult<String> {
    Ok(String::from_utf8(bytes)?)
}

/// Constructs one deterministic UUIDv7-shaped identifier from a numeric seed.
pub fn id<T>(suffix: u64) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}

/// Encodes bytes as lowercase hexadecimal.
#[must_use]
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
    let sanitized = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "case".to_owned()
    } else {
        sanitized
    }
}

/// Recursively lists every file below a root, as sorted relative paths.
pub fn list_files(root: &Path) -> TestResult<Vec<String>> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect(root: &Path, current: &Path, files: &mut Vec<String>) -> TestResult<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(current)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for entry in entries {
        let metadata = fs::symlink_metadata(&entry)?;
        if metadata.is_dir() {
            collect(root, &entry, files)?;
        } else {
            let relative = entry.strip_prefix(root)?;
            files.push(
                relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
    Ok(())
}

/// The originals choice used by most cases, spelled at each call site.
pub const WITH_ORIGINALS: OriginalInclusion = OriginalInclusion::Included;
/// The other choice.
pub const WITHOUT_ORIGINALS: OriginalInclusion = OriginalInclusion::Withheld;
