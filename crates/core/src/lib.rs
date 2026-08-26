//! Local core acceptance boundary and deterministic Phase 0 fixture.
//!
//! This crate has no network, filesystem, database, recording, or cloud code.
//! It accepts only canonically encoded batches whose signature matches an
//! independently supplied device key.

use std::str::FromStr;

use academic_contracts::{ContractError, VerifiedBatch, sign_batch, verify_signed_batch};
use academic_domain::{
    Actor, ArtifactDescriptor, AuthorityClass, BatchId, Claim, ClaimId, ClaimObject, ClaimRelation,
    ClaimRelationKind, ConfidencePermille, Confidentiality, ContentDigest, DecisionAction,
    DecisionId, DeviceId, DomainError, EpistemicStatus, EventPayload, EvidenceItem,
    EvidenceLocator, EvidenceRole, EvidenceStrength, FreshnessBand, MasteryLevel, MediaType,
    PredicateId, RetentionClass, TimestampMillis, UserDecision, ValidInterval, VaultLocator,
};
use academic_ledger::{
    AcceptanceReceipt, AuthorityPolicy, EVENT_SCHEMA_VERSION, LedgerError, LedgerState,
    ResolutionQuery, UnsignedBatch, event,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Fixture contract version.
pub const FIXTURE_VERSION: u16 = 1;
/// Fixed synthetic artifact bytes. They contain no personal or production data.
pub const SYNTHETIC_ARTIFACT_BYTES: &[u8] =
    b"SYNTHETIC ONLY: no personal data; no network egress.\n";
/// Default final valid-time coordinate used by fixture replay.
pub const FINAL_VALID_AT: TimestampMillis = TimestampMillis::new(700);

const FIXTURE_SIGNING_SEED: [u8; 32] = [7; 32];

const DOMAIN_ID: &str = "01900000-0000-7000-8000-000000000001";
const PERMISSION_ID: &str = "01900000-0000-7000-8000-000000000002";
const ARTIFACT_ID: &str = "01900000-0000-7000-8000-000000000003";
const EVIDENCE_ID: &str = "01900000-0000-7000-8000-000000000004";
const CONCEPT_ID: &str = "01900000-0000-7000-8000-000000000005";
const DEADLINE_SUBJECT_ID: &str = "01900000-0000-7000-8000-000000000006";
const SCOPE_ID: &str = "01900000-0000-7000-8000-000000000007";
const MODEL_RUN_ID: &str = "01900000-0000-7000-8000-000000000008";
const BATCH_ID: &str = "01900000-0000-7000-8000-000000000009";
const DEVICE_ID: &str = "01900000-0000-7000-8000-00000000000a";

const CLAIM_AI_UNDERSTOOD: &str = "01900000-0000-7000-8000-000000000201";
const CLAIM_USER_PRACTICED: &str = "01900000-0000-7000-8000-000000000202";
const CLAIM_FRESH_HIGH: &str = "01900000-0000-7000-8000-000000000203";
const CLAIM_FRESH_STALE: &str = "01900000-0000-7000-8000-000000000204";
const CLAIM_DEADLINE_OLD: &str = "01900000-0000-7000-8000-000000000205";
const CLAIM_DEADLINE_NEW: &str = "01900000-0000-7000-8000-000000000206";
const CLAIM_AI_FLUENT: &str = "01900000-0000-7000-8000-000000000207";

/// Core boundary error.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A domain value failed validation.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A transport or signature check failed.
    #[error(transparent)]
    Contract(#[from] ContractError),
    /// Ledger acceptance or replay failed.
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    /// Fixture JSON could not be parsed or serialized.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Fixture hex was malformed.
    #[error("invalid fixture hex: {0}")]
    Hex(#[from] hex::FromHexError),
    /// A fixture key did not have the Ed25519 length.
    #[error("fixture public key must contain 32 bytes, got {0}")]
    InvalidPublicKeyLength(usize),
    /// The fixture was validly signed but did not reproduce its declared result.
    #[error("fixture expected replay does not match actual replay")]
    ExpectedReplayMismatch,
    /// The committed fixture differs from the deterministic Phase 0 builder.
    #[error("fixture bytes or metadata drifted from the deterministic builder")]
    FixtureDrift,
    /// The requested fixture version is unsupported.
    #[error("unsupported fixture version {0}")]
    UnsupportedFixtureVersion(u16),
    /// A required active projection value was missing or had the wrong type.
    #[error("fixture projection missing {0}")]
    MissingProjection(&'static str),
}

/// Minimal local authority boundary.
#[derive(Debug, Default)]
pub struct Core {
    ledger: LedgerState,
}

impl Core {
    /// Creates a core with an empty append-only ledger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ledger: LedgerState::new(),
        }
    }

    /// Verifies canonical encoding and signature before atomically accepting a batch.
    pub fn accept_signed_batch(
        &mut self,
        envelope_bytes: &[u8],
        expected_device_key: &VerifyingKey,
    ) -> Result<(VerifiedBatch, AcceptanceReceipt), CoreError> {
        let verified = verify_signed_batch(envelope_bytes, expected_device_key)?;
        let receipt = self
            .ledger
            .accept_batch(&verified.batch, verified.envelope_hash)?;
        Ok((verified, receipt))
    }

    /// Returns a read-only view of the accepted ledger.
    #[must_use]
    pub const fn ledger(&self) -> &LedgerState {
        &self.ledger
    }
}

/// Portable JSON wrapper around exact deterministic signed bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureDocument {
    pub fixture_version: u16,
    pub name: String,
    pub data_class: String,
    pub network_egress: String,
    pub contract: FixtureContract,
    pub public_key_hex: String,
    pub signed_batch_cbor_hex: String,
    pub expected_replay: ReplaySummary,
}

/// Human-inspectable contract metadata for a golden fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureContract {
    pub envelope: String,
    pub payload: String,
    pub signature: String,
    pub event_schema_version: u16,
}

/// Deterministic semantic result of replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaySummary {
    pub accepted_events: usize,
    pub accept_seq_head: u64,
    pub payload_hash: ContentDigest,
    pub envelope_hash: ContentDigest,
    pub artifact_digest: ContentDigest,
    pub artifact_locator: VaultLocator,
    pub mastery: MasteryLevel,
    pub freshness: FreshnessBand,
    pub mastery_active_claim_ids: Vec<ClaimId>,
    pub mastery_conflicting_claim_ids: Vec<ClaimId>,
    pub mastery_rejected_claim_ids: Vec<ClaimId>,
    pub deadline_active_claim_ids: Vec<ClaimId>,
    pub semantic_digest: ContentDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReplayDigestMaterial<'a> {
    accepted_events: usize,
    accept_seq_head: u64,
    payload_hash: ContentDigest,
    envelope_hash: ContentDigest,
    artifact_digest: ContentDigest,
    artifact_locator: &'a VaultLocator,
    mastery: MasteryLevel,
    freshness: FreshnessBand,
    mastery_active_claim_ids: &'a [ClaimId],
    mastery_conflicting_claim_ids: &'a [ClaimId],
    mastery_rejected_claim_ids: &'a [ClaimId],
    deadline_active_claim_ids: &'a [ClaimId],
}

/// Builds the one deterministic, signed, synthetic Phase 0 fixture.
pub fn build_fixture_document() -> Result<FixtureDocument, CoreError> {
    let batch = build_unsigned_fixture_batch()?;
    let signing_key = fixture_signing_key();
    let signed = sign_batch(&batch, &signing_key)?;
    let mut core = Core::new();
    let (verified, _) = core.accept_signed_batch(&signed, &signing_key.verifying_key())?;
    let expected_replay = summarize_replay(&core, &verified, FINAL_VALID_AT, u64::MAX)?;
    Ok(FixtureDocument {
        fixture_version: FIXTURE_VERSION,
        name: "phase0-synthetic-bitemporal-ledger".to_owned(),
        data_class: "SYNTHETIC_ONLY".to_owned(),
        network_egress: "NONE".to_owned(),
        contract: FixtureContract {
            envelope: "academic.signed-batch-envelope/v1 deterministic-cbor".to_owned(),
            payload: "academic.event-batch/v1 deterministic-cbor".to_owned(),
            signature: "Ed25519".to_owned(),
            event_schema_version: EVENT_SCHEMA_VERSION,
        },
        public_key_hex: hex::encode(signing_key.verifying_key().as_bytes()),
        signed_batch_cbor_hex: hex::encode(signed),
        expected_replay,
    })
}

/// Verifies, accepts, replays, and compares a fixture with the deterministic builder.
pub fn verify_fixture_document(document: &FixtureDocument) -> Result<ReplaySummary, CoreError> {
    if document.fixture_version != FIXTURE_VERSION {
        return Err(CoreError::UnsupportedFixtureVersion(
            document.fixture_version,
        ));
    }
    let expected_key = decode_public_key(&document.public_key_hex)?;
    let signed = hex::decode(&document.signed_batch_cbor_hex)?;
    let mut core = Core::new();
    let (verified, _) = core.accept_signed_batch(&signed, &expected_key)?;
    let actual = summarize_replay(&core, &verified, FINAL_VALID_AT, u64::MAX)?;
    if actual != document.expected_replay {
        return Err(CoreError::ExpectedReplayMismatch);
    }
    if *document != build_fixture_document()? {
        return Err(CoreError::FixtureDrift);
    }
    Ok(actual)
}

/// Replays a fixture at caller-selected valid and known coordinates.
pub fn replay_fixture_document(
    document: &FixtureDocument,
    valid_at: TimestampMillis,
    known_at_accept_seq: u64,
) -> Result<ReplaySummary, CoreError> {
    let expected_key = decode_public_key(&document.public_key_hex)?;
    let signed = hex::decode(&document.signed_batch_cbor_hex)?;
    let mut core = Core::new();
    let (verified, _) = core.accept_signed_batch(&signed, &expected_key)?;
    summarize_replay(&core, &verified, valid_at, known_at_accept_seq)
}

/// Serializes a fixture with stable pretty JSON and a final newline.
pub fn fixture_json(document: &FixtureDocument) -> Result<String, CoreError> {
    let mut json = serde_json::to_string_pretty(document)?;
    json.push('\n');
    Ok(json)
}

fn decode_public_key(hex_value: &str) -> Result<VerifyingKey, CoreError> {
    let bytes = hex::decode(hex_value)?;
    let array: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::InvalidPublicKeyLength(bytes.len()))?;
    VerifyingKey::from_bytes(&array).map_err(|_| CoreError::InvalidPublicKeyLength(bytes.len()))
}

fn summarize_replay(
    core: &Core,
    verified: &VerifiedBatch,
    valid_at: TimestampMillis,
    known_at_accept_seq: u64,
) -> Result<ReplaySummary, CoreError> {
    let concept_id = parse_id(CONCEPT_ID)?;
    let mastery_predicate = PredicateId::parse("knowledge.mastery")?;
    let freshness_predicate = PredicateId::parse("knowledge.freshness")?;
    let knowledge = core.ledger().knowledge_state_as_of(
        concept_id,
        mastery_predicate,
        freshness_predicate,
        valid_at,
        known_at_accept_seq,
    );
    let deadline = core.ledger().resolve(&ResolutionQuery {
        subject_entity_id: parse_id(DEADLINE_SUBJECT_ID)?,
        predicate_id: PredicateId::parse("academic.deadline")?,
        valid_at,
        known_at_accept_seq,
        policy: AuthorityPolicy::OfficialFact,
    });
    let artifact = core
        .ledger()
        .artifact(parse_id(ARTIFACT_ID)?)
        .ok_or(CoreError::MissingProjection("artifact descriptor"))?;
    let mastery = knowledge
        .mastery
        .ok_or(CoreError::MissingProjection("mastery"))?;
    let freshness = knowledge
        .freshness
        .ok_or(CoreError::MissingProjection("freshness"))?;
    let material = ReplayDigestMaterial {
        accepted_events: core.ledger().accepted_events().len(),
        accept_seq_head: core.ledger().accept_seq_head(),
        payload_hash: verified.payload_hash,
        envelope_hash: verified.envelope_hash,
        artifact_digest: artifact.content_digest,
        artifact_locator: &artifact.vault_locator,
        mastery,
        freshness,
        mastery_active_claim_ids: &knowledge.mastery_resolution.active_claim_ids,
        mastery_conflicting_claim_ids: &knowledge.mastery_resolution.conflicting_claim_ids,
        mastery_rejected_claim_ids: &knowledge.mastery_resolution.rejected_claim_ids,
        deadline_active_claim_ids: &deadline.active_claim_ids,
    };
    let semantic_bytes = serde_json::to_vec(&material)?;
    Ok(ReplaySummary {
        accepted_events: material.accepted_events,
        accept_seq_head: material.accept_seq_head,
        payload_hash: material.payload_hash,
        envelope_hash: material.envelope_hash,
        artifact_digest: material.artifact_digest,
        artifact_locator: material.artifact_locator.clone(),
        mastery: material.mastery,
        freshness: material.freshness,
        mastery_active_claim_ids: material.mastery_active_claim_ids.to_vec(),
        mastery_conflicting_claim_ids: material.mastery_conflicting_claim_ids.to_vec(),
        mastery_rejected_claim_ids: material.mastery_rejected_claim_ids.to_vec(),
        deadline_active_claim_ids: material.deadline_active_claim_ids.to_vec(),
        semantic_digest: ContentDigest::sha256(&semantic_bytes),
    })
}

fn fixture_signing_key() -> SigningKey {
    SigningKey::from_bytes(&FIXTURE_SIGNING_SEED)
}

fn parse_id<T>(value: &str) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    value.parse()
}

fn claim(
    id: &str,
    subject: &str,
    predicate: &str,
    object: ClaimObject,
    provenance: (AuthorityClass, EpistemicStatus, Option<u16>),
    valid_time: ValidInterval,
) -> Result<Claim, DomainError> {
    let (authority_class, epistemic_status, confidence) = provenance;
    Ok(Claim {
        id: parse_id(id)?,
        subject_entity_id: parse_id(subject)?,
        predicate_id: PredicateId::parse(predicate)?,
        object,
        scope_id: Some(parse_id(SCOPE_ID)?),
        authority_class,
        epistemic_status,
        confidence: confidence.map(ConfidencePermille::new).transpose()?,
        valid_time,
        evidence_ids: vec![parse_id(EVIDENCE_ID)?],
    })
}

fn fixture_event(
    sequence: u64,
    actor: Actor,
    payload: EventPayload,
) -> Result<academic_domain::Event, DomainError> {
    let event_id = format!("01900000-0000-7000-8000-{sequence:012x}");
    Ok(event(
        parse_id(&event_id)?,
        sequence,
        TimestampMillis::new(100_i64.saturating_add(i64::try_from(sequence).unwrap_or(i64::MAX))),
        actor,
        parse_id(DOMAIN_ID)?,
        payload,
    ))
}

fn build_unsigned_fixture_batch() -> Result<UnsignedBatch, CoreError> {
    let media_type = MediaType::parse("text/plain")?;
    let artifact_digest = ContentDigest::sha256(SYNTHETIC_ARTIFACT_BYTES);
    let artifact_locator = VaultLocator::derive(
        b"phase0-synthetic-domain-locator-key",
        1,
        &media_type,
        artifact_digest,
    )?;
    let artifact = ArtifactDescriptor {
        id: parse_id(ARTIFACT_ID)?,
        content_digest: artifact_digest,
        media_type,
        byte_length: u64::try_from(SYNTHETIC_ARTIFACT_BYTES.len())
            .map_err(|_| CoreError::MissingProjection("artifact byte length"))?,
        domain_id: parse_id(DOMAIN_ID)?,
        confidentiality: Confidentiality::Personal,
        retention_class: RetentionClass::UserManaged,
        permission_lineage_id: parse_id(PERMISSION_ID)?,
        format_version: 1,
        vault_locator: artifact_locator,
    };
    let evidence = EvidenceItem {
        id: parse_id(EVIDENCE_ID)?,
        artifact_id: parse_id(ARTIFACT_ID)?,
        locator: EvidenceLocator::TextBytes {
            source_digest: artifact_digest,
            start: 0,
            end: u64::try_from(SYNTHETIC_ARTIFACT_BYTES.len())
                .map_err(|_| CoreError::MissingProjection("evidence byte range"))?,
        },
        excerpt_digest: artifact_digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "phase0.synthetic.fixture".to_owned(),
        extractor_version: "1.0.0".to_owned(),
    };
    let ai_actor = Actor::ModelRun {
        run_id: parse_id(MODEL_RUN_ID)?,
    };
    let engine_actor = Actor::DeterministicEngine {
        name: "academic.fixture".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let official_actor = Actor::Importer {
        name: "synthetic.official.fixture".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let valid_from_100 = ValidInterval::open_ended(TimestampMillis::new(100));
    let valid_from_200 = ValidInterval::open_ended(TimestampMillis::new(200));

    let ai_understood = claim(
        CLAIM_AI_UNDERSTOOD,
        CONCEPT_ID,
        "knowledge.mastery",
        ClaimObject::Mastery(MasteryLevel::Understood),
        (
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            Some(720),
        ),
        valid_from_100,
    )?;
    let user_practiced = claim(
        CLAIM_USER_PRACTICED,
        CONCEPT_ID,
        "knowledge.mastery",
        ClaimObject::Mastery(MasteryLevel::Practiced),
        (
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed,
            Some(1000),
        ),
        valid_from_200,
    )?;
    let fresh_high = claim(
        CLAIM_FRESH_HIGH,
        CONCEPT_ID,
        "knowledge.freshness",
        ClaimObject::Freshness(FreshnessBand::High),
        (
            AuthorityClass::DeterministicEngine,
            EpistemicStatus::DeterministicDerived,
            Some(900),
        ),
        ValidInterval::new(TimestampMillis::new(200), Some(TimestampMillis::new(600)))?,
    )?;
    let fresh_stale = claim(
        CLAIM_FRESH_STALE,
        CONCEPT_ID,
        "knowledge.freshness",
        ClaimObject::Freshness(FreshnessBand::Stale),
        (
            AuthorityClass::DeterministicEngine,
            EpistemicStatus::DeterministicDerived,
            Some(900),
        ),
        ValidInterval::open_ended(TimestampMillis::new(600)),
    )?;
    let deadline_old = claim(
        CLAIM_DEADLINE_OLD,
        DEADLINE_SUBJECT_ID,
        "academic.deadline",
        ClaimObject::Text("2027-04-01".to_owned()),
        (
            AuthorityClass::Official,
            EpistemicStatus::OfficialConfirmed,
            None,
        ),
        ValidInterval::open_ended(TimestampMillis::new(300)),
    )?;
    let deadline_new = claim(
        CLAIM_DEADLINE_NEW,
        DEADLINE_SUBJECT_ID,
        "academic.deadline",
        ClaimObject::Text("2027-04-15".to_owned()),
        (
            AuthorityClass::Official,
            EpistemicStatus::OfficialConfirmed,
            None,
        ),
        ValidInterval::open_ended(TimestampMillis::new(300)),
    )?;
    let ai_fluent = claim(
        CLAIM_AI_FLUENT,
        CONCEPT_ID,
        "knowledge.mastery",
        ClaimObject::Mastery(MasteryLevel::Fluent),
        (
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            Some(810),
        ),
        valid_from_200,
    )?;

    let events = vec![
        fixture_event(
            1,
            official_actor.clone(),
            EventPayload::ArtifactRegistered(artifact),
        )?,
        fixture_event(
            2,
            official_actor.clone(),
            EventPayload::EvidenceRegistered(evidence),
        )?,
        fixture_event(
            3,
            ai_actor.clone(),
            EventPayload::ClaimAsserted(ai_understood),
        )?,
        fixture_event(
            4,
            Actor::User,
            EventPayload::DecisionRecorded(UserDecision {
                id: parse_id::<DecisionId>("01900000-0000-7000-8000-000000000301")?,
                target_claim_id: parse_id(CLAIM_AI_UNDERSTOOD)?,
                action: DecisionAction::Reject,
                scope_id: Some(parse_id(SCOPE_ID)?),
                rationale_evidence_ids: vec![parse_id(EVIDENCE_ID)?],
                decided_at: TimestampMillis::new(104),
                reversible_until: None,
            }),
        )?,
        fixture_event(5, Actor::User, EventPayload::ClaimAsserted(user_practiced))?,
        fixture_event(
            6,
            Actor::User,
            EventPayload::DecisionRecorded(UserDecision {
                id: parse_id::<DecisionId>("01900000-0000-7000-8000-000000000302")?,
                target_claim_id: parse_id(CLAIM_USER_PRACTICED)?,
                action: DecisionAction::Confirm,
                scope_id: Some(parse_id(SCOPE_ID)?),
                rationale_evidence_ids: vec![parse_id(EVIDENCE_ID)?],
                decided_at: TimestampMillis::new(106),
                reversible_until: None,
            }),
        )?,
        fixture_event(
            7,
            engine_actor.clone(),
            EventPayload::ClaimAsserted(fresh_high),
        )?,
        fixture_event(8, engine_actor, EventPayload::ClaimAsserted(fresh_stale))?,
        fixture_event(
            9,
            official_actor.clone(),
            EventPayload::ClaimAsserted(deadline_old),
        )?,
        fixture_event(
            10,
            official_actor.clone(),
            EventPayload::ClaimAsserted(deadline_new),
        )?,
        fixture_event(
            11,
            official_actor,
            EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: parse_id(CLAIM_DEADLINE_NEW)?,
                target_claim_id: parse_id(CLAIM_DEADLINE_OLD)?,
                kind: ClaimRelationKind::Supersedes,
            }),
        )?,
        fixture_event(12, ai_actor, EventPayload::ClaimAsserted(ai_fluent))?,
    ];
    Ok(UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: parse_id::<BatchId>(BATCH_ID)?,
        device_id: parse_id::<DeviceId>(DEVICE_ID)?,
        origin_seq_start: 1,
        origin_seq_end: u64::try_from(events.len())
            .map_err(|_| CoreError::MissingProjection("event count"))?,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(112),
        events,
    })
}

/// Bitemporal conformance example.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeTravelCase {
    pub name: &'static str,
    pub valid_at: TimestampMillis,
    pub known_at: u64,
    pub expected_mastery: Option<MasteryLevel>,
    pub expected_freshness: Option<FreshnessBand>,
    pub expected_deadline_claims: Vec<ClaimId>,
}

/// Returns the ADR-003 table of at least twelve independent time-travel assertions.
pub fn time_travel_cases() -> Result<Vec<TimeTravelCase>, DomainError> {
    Ok(vec![
        TimeTravelCase {
            name: "nothing-known-at-zero",
            valid_at: TimestampMillis::new(250),
            known_at: 0,
            expected_mastery: None,
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "ai-mastery-before-user-review",
            valid_at: TimestampMillis::new(250),
            known_at: 3,
            expected_mastery: Some(MasteryLevel::Understood),
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "rejected-ai-mastery-is-not-active",
            valid_at: TimestampMillis::new(250),
            known_at: 4,
            expected_mastery: None,
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "future-valid-user-claim-not-yet-valid",
            valid_at: TimestampMillis::new(150),
            known_at: 5,
            expected_mastery: None,
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "user-mastery-active-at-valid-time",
            valid_at: TimestampMillis::new(250),
            known_at: 5,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "explicit-confirmation-preserves-user-mastery",
            valid_at: TimestampMillis::new(250),
            known_at: 6,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "freshness-high-in-first-valid-window",
            valid_at: TimestampMillis::new(250),
            known_at: 7,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "accepted-stale-claim-does-not-apply-before-valid-from",
            valid_at: TimestampMillis::new(250),
            known_at: 8,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "freshness-stale-without-mastery-decay",
            valid_at: TimestampMillis::new(700),
            known_at: 8,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::Stale),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "old-official-deadline-as-known",
            valid_at: TimestampMillis::new(400),
            known_at: 9,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![parse_id(CLAIM_DEADLINE_OLD)?],
        },
        TimeTravelCase {
            name: "competing-official-deadlines-before-relation",
            valid_at: TimestampMillis::new(400),
            known_at: 10,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![
                parse_id(CLAIM_DEADLINE_OLD)?,
                parse_id(CLAIM_DEADLINE_NEW)?,
            ],
        },
        TimeTravelCase {
            name: "official-correction-after-supersession",
            valid_at: TimestampMillis::new(400),
            known_at: 11,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![parse_id(CLAIM_DEADLINE_NEW)?],
        },
        TimeTravelCase {
            name: "official-correction-not-valid-before-effective-time",
            valid_at: TimestampMillis::new(250),
            known_at: 11,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "later-ai-does-not-override-user-decision",
            valid_at: TimestampMillis::new(700),
            known_at: 12,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::Stale),
            expected_deadline_claims: vec![parse_id(CLAIM_DEADLINE_NEW)?],
        },
    ])
}

#[cfg(test)]
mod tests {
    use academic_domain::LogicalPath;

    use super::*;

    #[test]
    fn signed_fixture_round_trips_and_replays() -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let replay = verify_fixture_document(&document)?;
        assert_eq!(replay.accepted_events, 12);
        assert_eq!(replay.mastery, MasteryLevel::Practiced);
        assert_eq!(replay.freshness, FreshnessBand::Stale);
        assert_eq!(
            replay.mastery_active_claim_ids,
            vec![parse_id(CLAIM_USER_PRACTICED)?]
        );
        assert_eq!(
            replay.mastery_conflicting_claim_ids,
            vec![parse_id(CLAIM_AI_FLUENT)?]
        );
        assert_eq!(
            replay.mastery_rejected_claim_ids,
            vec![parse_id(CLAIM_AI_UNDERSTOOD)?]
        );
        assert_eq!(
            replay.deadline_active_claim_ids,
            vec![parse_id(CLAIM_DEADLINE_NEW)?]
        );
        Ok(())
    }

    #[test]
    fn adr_003_has_at_least_twelve_bitemporal_examples() -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let expected_key = decode_public_key(&document.public_key_hex)?;
        let signed = hex::decode(&document.signed_batch_cbor_hex)?;
        let mut core = Core::new();
        let (verified, _) = core.accept_signed_batch(&signed, &expected_key)?;
        let cases = time_travel_cases()?;
        assert!(cases.len() >= 12);

        for case in cases {
            let knowledge = core.ledger().knowledge_state_as_of(
                parse_id(CONCEPT_ID)?,
                PredicateId::parse("knowledge.mastery")?,
                PredicateId::parse("knowledge.freshness")?,
                case.valid_at,
                case.known_at,
            );
            let deadline = core.ledger().resolve(&ResolutionQuery {
                subject_entity_id: parse_id(DEADLINE_SUBJECT_ID)?,
                predicate_id: PredicateId::parse("academic.deadline")?,
                valid_at: case.valid_at,
                known_at_accept_seq: case.known_at,
                policy: AuthorityPolicy::OfficialFact,
            });
            assert_eq!(knowledge.mastery, case.expected_mastery, "{}", case.name);
            assert_eq!(
                knowledge.freshness, case.expected_freshness,
                "{}",
                case.name
            );
            assert_eq!(
                deadline.active_claim_ids, case.expected_deadline_claims,
                "{}",
                case.name
            );
        }
        assert_eq!(verified.batch.events.len(), 12);
        Ok(())
    }

    #[test]
    fn artifact_locator_is_keyed_and_not_plain_digest() -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let replay = document.expected_replay;
        assert!(
            !replay
                .artifact_locator
                .to_string()
                .contains(&hex::encode(replay.artifact_digest.as_bytes()))
        );
        Ok(())
    }

    #[test]
    fn normalized_repository_locator_rejects_parent_escape() {
        assert!(LogicalPath::parse("src/domain.rs").is_ok());
        assert!(LogicalPath::parse("../secret.env").is_err());
    }
}
