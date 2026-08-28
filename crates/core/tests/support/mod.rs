#![allow(dead_code)]

use std::{
    error::Error,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

use academic_contracts::{DeviceAuthorization, sign_batch};
use academic_core::service::AcceptanceService;
use academic_domain::{
    Actor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim, ClaimId,
    ClaimObject, ClaimRelation, Confidentiality, ContentDigest, DecisionId, DeviceId, DomainError,
    DomainId, EntityId, EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem,
    EvidenceLocator, EvidenceRole, EvidenceStrength, MediaType, PermissionLineageId, PredicateId,
    RetentionClass, ScopeDescriptor, ScopeId, TimestampMillis, UnsignedBatch, UserDecision,
    ValidInterval,
};
use academic_ledger::{AuthorityPolicy, EVENT_SCHEMA_VERSION};
use academic_projections::{
    fts::{ExactSymbolHit, SearchHit},
    generation::{ProjectionCoordinates, ProjectionKind, ProjectionPage},
    graph::GraphEdge,
    query::ProjectionReader,
    resolution::PredicatePolicies,
    runner::ProjectionRunner,
};
use academic_store::{
    connection::{ReaderConnection, open_reader},
    idempotency::AcceptanceCommand,
    path_policy::NativePathProbe,
    profile::create_synthetic_profile,
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring};
use ed25519_dalek::SigningKey;

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct EvidenceFixture {
    pub domain_id: DomainId,
    pub scope_id: ScopeId,
    pub artifact_id: ArtifactId,
    pub evidence_id: EvidenceId,
    pub locator: EvidenceLocator,
    pub digest: ContentDigest,
    pub vault_object_path: PathBuf,
}

#[derive(Debug)]
pub struct Fixture {
    root: PathBuf,
    canonical: PathBuf,
    sidecar: PathBuf,
    service: Option<AcceptanceService>,
    signing_key: SigningKey,
    authorization: DeviceAuthorization,
    next_origin_seq: u64,
    previous_batch_hash: Option<ContentDigest>,
    revision: u64,
    batch_counter: u64,
    accepted_at: i64,
    known_at_accept_seq: u64,
}

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native facade
/// refuses to follow a link component, so the tests address the real directory.
#[cfg(unix)]
fn temporary_base() -> std::io::Result<PathBuf> {
    std::fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> std::io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

impl Fixture {
    pub fn new(label: &str) -> TestResult<Self> {
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = temporary_base()?.join(format!(
            "academic-projections-{}-{}-{sequence}",
            sanitize(label),
            std::process::id()
        ));
        let profile = create_synthetic_profile(&root, &NativePathProbe::default(), [0x91; 32])?;
        let canonical = profile.database_path().to_path_buf();
        let sidecar = root.join("projection.sqlite3");
        let mut keyring = DomainKeyring::new();
        for domain_seed in 1_u8..=16 {
            let key = [domain_seed.wrapping_add(0x40); 32];
            keyring.insert(domain(domain_seed)?, &key)?;
        }
        let service = AcceptanceService::open(&profile, keyring)?;
        let signing_key = SigningKey::from_bytes(&[0x6d; 32]);
        let device_id = id::<DeviceId>(0xd001)?;
        let user_id = id::<EntityId>(0xd002)?;
        let authorization =
            DeviceAuthorization::new(device_id, user_id, signing_key.verifying_key());
        Ok(Self {
            root,
            canonical,
            sidecar,
            service: Some(service),
            signing_key,
            authorization,
            next_origin_seq: 1,
            previous_batch_hash: None,
            revision: 0,
            batch_counter: 0,
            accepted_at: 10_000,
            known_at_accept_seq: 0,
        })
    }

    pub fn canonical_path(&self) -> &Path {
        &self.canonical
    }

    pub fn sidecar_path(&self) -> &Path {
        &self.sidecar
    }

    pub fn runner(&self) -> TestResult<ProjectionRunner> {
        let reader = open_reader(&self.canonical)?;
        Ok(ProjectionRunner::open(
            &reader,
            &self.sidecar,
            ContentDigest::sha256(b"projection-real-acceptance-test-builder"),
            ContentDigest::sha256(b"projection-real-acceptance-test-config"),
        )?)
    }

    pub fn projection_reader(&self) -> TestResult<ProjectionReader> {
        let reader = open_reader(&self.canonical)?;
        Ok(ProjectionReader::new(&reader, &self.sidecar))
    }

    pub fn store_reader(&self) -> TestResult<ReaderConnection> {
        Ok(open_reader(&self.canonical)?)
    }

    pub const fn known_at_accept_seq(&self) -> u64 {
        self.known_at_accept_seq
    }

    pub fn user_actor(&self) -> Actor {
        Actor::User {
            user_id: self.authorization.user_id(),
        }
    }

    pub const fn coordinates(&self, valid_at: i64) -> ProjectionCoordinates {
        ProjectionCoordinates::new(self.known_at_accept_seq, TimestampMillis::new(valid_at))
    }

    pub fn register_scope_evidence(
        &mut self,
        domain_seed: u8,
        item_seed: u16,
        bytes: &[u8],
    ) -> TestResult<EvidenceFixture> {
        let domain_id = domain(domain_seed)?;
        let scope_id = scoped_id::<ScopeId>(0x10, domain_seed, item_seed)?;
        self.register_evidence_inner(domain_id, scope_id, domain_seed, item_seed, bytes, true)
    }

    pub fn register_evidence(
        &mut self,
        domain_seed: u8,
        scope_id: ScopeId,
        item_seed: u16,
        bytes: &[u8],
    ) -> TestResult<EvidenceFixture> {
        self.register_evidence_inner(
            domain(domain_seed)?,
            scope_id,
            domain_seed,
            item_seed,
            bytes,
            false,
        )
    }

    fn register_evidence_inner(
        &mut self,
        domain_id: DomainId,
        scope_id: ScopeId,
        domain_seed: u8,
        item_seed: u16,
        bytes: &[u8],
        register_scope: bool,
    ) -> TestResult<EvidenceFixture> {
        let artifact_id = scoped_id::<ArtifactId>(0x20, domain_seed, item_seed)?;
        let evidence_id = scoped_id::<EvidenceId>(0x30, domain_seed, item_seed)?;
        let permission_lineage_id = scoped_id::<PermissionLineageId>(0x40, domain_seed, item_seed)?;
        let request = ArtifactIngestRequest::new(
            artifact_id,
            MediaType::parse("text/plain")?,
            domain_id,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            permission_lineage_id,
        );
        let receipt = self
            .service
            .as_ref()
            .ok_or("synthetic acceptance service is closed")?
            .vault()
            .ingest(&request, Cursor::new(bytes))?;
        let mut descriptor = receipt.descriptor().clone();
        let vault_object_path = receipt.object_path().to_path_buf();
        let locator = EvidenceLocator::TextBytes {
            source_digest: descriptor.content_digest,
            start: 0,
            end: descriptor.byte_length,
        };
        descriptor
            .evidence_representations
            .push(ArtifactRepresentation {
                locator: locator.clone(),
                content_digest: descriptor.content_digest,
                byte_length: descriptor.byte_length,
            });
        let evidence = EvidenceItem {
            id: evidence_id,
            artifact_id,
            locator: locator.clone(),
            excerpt_digest: descriptor.content_digest,
            role: EvidenceRole::Supports,
            strength: EvidenceStrength::Direct,
            extraction_method: "academic.projections.synthetic".to_owned(),
            extractor_version: "1.0.0".to_owned(),
        };
        let mut payloads = Vec::new();
        if register_scope {
            payloads.push(EventPayload::ScopeRegistered(ScopeDescriptor {
                id: scope_id,
                domain_id,
                label: format!("synthetic projection scope {domain_seed}-{item_seed}"),
            }));
        }
        payloads.push(EventPayload::ArtifactRegistered(descriptor.clone()));
        payloads.push(EventPayload::EvidenceRegistered(evidence));
        self.accept_payloads(importer_actor(), domain_id, payloads)?;
        Ok(EvidenceFixture {
            domain_id,
            scope_id,
            artifact_id,
            evidence_id,
            locator,
            digest: descriptor.content_digest,
            vault_object_path,
        })
    }

    pub fn accept_claim(
        &mut self,
        actor: Actor,
        domain_id: DomainId,
        claim: Claim,
    ) -> TestResult<u64> {
        self.accept_payloads(actor, domain_id, vec![EventPayload::ClaimAsserted(claim)])
    }

    pub fn accept_relation(
        &mut self,
        actor: Actor,
        domain_id: DomainId,
        relation: ClaimRelation,
    ) -> TestResult<u64> {
        self.accept_payloads(actor, domain_id, vec![EventPayload::ClaimRelated(relation)])
    }

    pub fn accept_decision(
        &mut self,
        domain_id: DomainId,
        decision: UserDecision,
    ) -> TestResult<u64> {
        self.accept_payloads(
            Actor::User {
                user_id: self.authorization.user_id(),
            },
            domain_id,
            vec![EventPayload::DecisionRecorded(decision)],
        )
    }

    pub fn accept_payloads(
        &mut self,
        actor: Actor,
        domain_id: DomainId,
        payloads: Vec<EventPayload>,
    ) -> TestResult<u64> {
        if payloads.is_empty() {
            return Err("synthetic acceptance batch cannot be empty".into());
        }
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
                    0xe000_0000_u64
                        .checked_add(self.batch_counter << 12)
                        .and_then(|value| value.checked_add(offset))
                        .ok_or("synthetic event identifier overflow")?,
                )?,
                origin_seq,
                origin_observed_at: TimestampMillis::new(
                    1_000_i64
                        .checked_add(i64::try_from(origin_seq)?)
                        .ok_or("synthetic observed-at overflow")?,
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
                0xb000_0000_u64
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
                    .ok_or("synthetic created-at overflow")?,
            ),
            events,
        };
        let envelope = sign_batch(&batch, &self.signing_key)?;
        let request_byte = u8::try_from(self.batch_counter % 251)?;
        let outcome = self
            .service
            .as_mut()
            .ok_or("synthetic acceptance service is closed")?
            .accept_signed_command(
                AcceptanceCommand {
                    request_id: [request_byte; 16],
                    client_instance_id: [0x61; 16],
                    idempotency_key: *ContentDigest::sha256(
                        &[
                            b"projection-fixture-request".as_slice(),
                            &self.batch_counter.to_be_bytes(),
                        ]
                        .concat(),
                    )
                    .as_bytes(),
                    expected_revision: Some(self.revision),
                    envelope_bytes: &envelope,
                },
                &self.authorization,
                TimestampMillis::new(self.accepted_at),
            )?;
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
        Ok(self.known_at_accept_seq)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.service.take());
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

pub fn policies(entries: &[(&str, AuthorityPolicy)]) -> TestResult<PredicatePolicies> {
    Ok(PredicatePolicies::new(
        "projection-test-policies-v1",
        entries
            .iter()
            .map(|(predicate, policy)| Ok((PredicateId::parse(*predicate)?, *policy)))
            .collect::<Result<Vec<_>, DomainError>>()?,
    )?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankSkewCorpus {
    InactiveHistorical,
    FailedOrOtherDomain,
}

#[derive(Debug)]
pub struct RankedIsolationFixture {
    pub fixture: Fixture,
    pub kind: ProjectionKind,
    pub selected_domain: DomainId,
    pub noise_domain: DomainId,
    pub selected_coordinates: ProjectionCoordinates,
    pub noise_coordinates: ProjectionCoordinates,
    pub policies: PredicatePolicies,
    pub query: &'static str,
    pub exact_symbol: &'static str,
    pub graph_source: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedIsolationSnapshot {
    ranked: ProjectionPage<SearchHit>,
    exact_symbol: ProjectionPage<ExactSymbolHit>,
    graph: ProjectionPage<GraphEdge>,
}

impl RankedIsolationFixture {
    pub fn new(
        label: &str,
        kind: ProjectionKind,
        corpus: RankSkewCorpus,
        noise_in_other_domain: bool,
    ) -> TestResult<Self> {
        let mut fixture = Fixture::new(label)?;
        let selected_evidence =
            fixture.register_scope_evidence(8, 1, b"rank isolation selected evidence")?;
        let noise_evidence = if noise_in_other_domain {
            fixture.register_scope_evidence(9, 1, b"rank isolation noise evidence")?
        } else {
            selected_evidence.clone()
        };
        let (query, first_body, second_body, noise_body, noise_count) = match (kind, corpus) {
            (ProjectionKind::Unicode61, RankSkewCorpus::InactiveHistorical) => {
                ("alpha", "alpha", "alpha alpha alpha z", "z", 100_u64)
            }
            (ProjectionKind::Trigram, RankSkewCorpus::InactiveHistorical) => {
                ("abc", "abc", "abcabcabcabcabc", "zzz", 100_u64)
            }
            (
                ProjectionKind::Unicode61 | ProjectionKind::Trigram,
                RankSkewCorpus::FailedOrOtherDomain,
            ) => (
                "alpha",
                "alpha",
                "alpha alpha z",
                "q q q q q q q q q q",
                20_u64,
            ),
            (ProjectionKind::Graph, _) => {
                return Err("rank isolation fixture requires a lexical projection kind".into());
            }
        };
        let graph_source = entity(80_003)?;
        for (seed, predicate, body) in [
            (80_001, "code.symbol", first_body),
            (80_002, "note.body", second_body),
        ] {
            fixture.accept_claim(
                importer_actor(),
                selected_evidence.domain_id,
                text_claim(
                    claim_id(seed)?,
                    entity(seed)?,
                    predicate,
                    body,
                    selected_evidence.scope_id,
                    selected_evidence.evidence_id,
                    AuthorityClass::DirectObservation,
                    EpistemicStatus::CodeObserved,
                    75,
                    None,
                )?,
            )?;
        }
        fixture.accept_claim(
            importer_actor(),
            selected_evidence.domain_id,
            observed_entity_claim(
                claim_id(80_003)?,
                graph_source,
                "graph.related",
                entity(80_004)?,
                selected_evidence.scope_id,
                selected_evidence.evidence_id,
                75,
                None,
            )?,
        )?;
        let noise_payloads = (0..noise_count)
            .map(|offset| {
                Ok(EventPayload::ClaimAsserted(text_claim(
                    claim_id(90_000 + offset)?,
                    entity(90_000 + offset)?,
                    "note.body",
                    noise_body,
                    noise_evidence.scope_id,
                    noise_evidence.evidence_id,
                    AuthorityClass::DirectObservation,
                    EpistemicStatus::CodeObserved,
                    0,
                    Some(50),
                )?))
            })
            .collect::<TestResult<Vec<_>>>()?;
        fixture.accept_payloads(importer_actor(), noise_evidence.domain_id, noise_payloads)?;

        let selected_coordinates = fixture.coordinates(100);
        let noise_coordinates = fixture.coordinates(25);
        let policies = policies(&[
            ("code.symbol", AuthorityPolicy::ImplementationObservation),
            ("graph.related", AuthorityPolicy::ImplementationObservation),
            ("note.body", AuthorityPolicy::ImplementationObservation),
        ])?;
        let runner = fixture.runner()?;
        runner.rebuild_at(
            ProjectionKind::Graph,
            selected_evidence.domain_id,
            selected_coordinates,
            &policies,
        )?;
        runner.rebuild_at(
            kind,
            selected_evidence.domain_id,
            selected_coordinates,
            &policies,
        )?;
        Ok(Self {
            fixture,
            kind,
            selected_domain: selected_evidence.domain_id,
            noise_domain: noise_evidence.domain_id,
            selected_coordinates,
            noise_coordinates,
            policies,
            query,
            exact_symbol: first_body,
            graph_source,
        })
    }

    pub fn snapshot(&self) -> TestResult<RankedIsolationSnapshot> {
        let reader = self.fixture.projection_reader()?;
        let ranked = reader.search_ranked(
            self.kind,
            self.selected_domain,
            self.selected_coordinates,
            &self.policies,
            self.query,
            20,
        )?;
        assert_eq!(ranked.records.len(), 2);
        assert_eq!(
            ranked
                .records
                .iter()
                .map(|record| record.rank)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        let exact_symbol = reader.exact_symbol_lookup(
            self.kind,
            self.selected_domain,
            self.selected_coordinates,
            &self.policies,
            self.exact_symbol,
        )?;
        assert_eq!(exact_symbol.records.len(), 1);
        let graph = reader.graph_neighbors(
            self.selected_domain,
            self.graph_source,
            self.selected_coordinates,
            &self.policies,
        )?;
        assert_eq!(graph.records.len(), 1);
        Ok(RankedIsolationSnapshot {
            ranked,
            exact_symbol,
            graph,
        })
    }
}

pub fn importer_actor() -> Actor {
    Actor::Importer {
        name: "academic.projections.synthetic".to_owned(),
        version: "1.0.0".to_owned(),
    }
}

pub fn model_actor(seed: u64) -> TestResult<Actor> {
    Ok(Actor::ModelRun {
        run_id: id::<EntityId>(
            0x7000_0000_u64
                .checked_add(seed)
                .ok_or("model id overflow")?,
        )?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn observed_entity_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    target: EntityId,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
    valid_from: i64,
    valid_to: Option<i64>,
) -> TestResult<Claim> {
    claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Entity(target),
        scope_id,
        evidence_id,
        AuthorityClass::DirectObservation,
        EpistemicStatus::CodeObserved,
        valid_from,
        valid_to,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn text_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    text: &str,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
    authority: AuthorityClass,
    status: EpistemicStatus,
    valid_from: i64,
    valid_to: Option<i64>,
) -> TestResult<Claim> {
    claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Text(text.to_owned()),
        scope_id,
        evidence_id,
        authority,
        status,
        valid_from,
        valid_to,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    object: ClaimObject,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
    valid_from: i64,
    valid_to: Option<i64>,
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
        valid_time: ValidInterval::new(
            TimestampMillis::new(valid_from),
            valid_to.map(TimestampMillis::new),
        )?,
        evidence_ids: vec![evidence_id],
    };
    claim.validate()?;
    Ok(claim)
}

pub fn domain(seed: u8) -> Result<DomainId, DomainError> {
    scoped_id(0x01, seed, 0)
}

pub fn entity(seed: u64) -> Result<EntityId, DomainError> {
    id(0x5000_0000_u64.saturating_add(seed))
}

pub fn claim_id(seed: u64) -> Result<ClaimId, DomainError> {
    id(0x6000_0000_u64.saturating_add(seed))
}

pub fn decision_id(seed: u64) -> Result<DecisionId, DomainError> {
    id(0x8000_0000_u64.saturating_add(seed))
}

pub fn scoped_id<T>(kind: u8, domain_seed: u8, item_seed: u16) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    id((u64::from(kind) << 32) | (u64::from(domain_seed) << 24) | u64::from(item_seed))
}

pub fn id<T>(suffix: u64) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
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
