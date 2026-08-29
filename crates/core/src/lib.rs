//! Local core acceptance boundary and deterministic Phase 0 fixture.
//!
//! This crate has no network, filesystem, database, recording, or cloud code.
//! It accepts only canonically encoded batches whose signature matches an
//! independently supplied device key.

pub mod local_service;
pub mod operations;
pub mod service;

use std::{collections::BTreeSet, fmt, str::FromStr};

use academic_contracts::{
    ContractError, DeviceAuthorization, VerifiedBatch, sign_batch, verify_signed_batch,
};
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactRepresentation, AttemptRegistration, AuditRegistration,
    AuthorityClass, BatchId, CapturePermissionRegistration, Claim, ClaimId, ClaimObject,
    ClaimRelation, ClaimRelationKind, ConfidencePermille, Confidentiality, ConsentRegistration,
    ContentDigest, CourseRevisionRegistration, CurriculumVersionRegistration, DecisionAction,
    DecisionId, DeviceId, DomainError, EVENT_SCHEMA_VERSION_V1, EVENT_SCHEMA_VERSION_V2,
    EVENT_SCHEMA_VERSION_V3, EgressDecisionRegistration, EntityIdentityChangeRegistration,
    EpistemicStatus, EventPayload, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength,
    FindingRegistration, FreshnessBand, LectureDocumentRegistration, LectureSessionRegistration,
    MasteryLevel, MediaType, ModelRunRegistration, OfferingRegistration,
    PREDICTION_METADATA_VERSION_V1, PredicateId, PredictionMetadata, PredictionObservationWindow,
    ProposalDispositionRegistration, RequirementSetRegistration, ResolutionSlot,
    RetentionActionRegistration, RetentionClass, ScopeDescriptor, ScopeId, SnapshotRegistration,
    TimestampMillis, TranscriptVersionRegistration, UserDecision, ValidInterval, VaultLocator,
};
use academic_ledger::{
    AcceptanceReceipt, AuthorityPolicy, EVENT_SCHEMA_VERSION, LedgerError, LedgerState,
    ResolutionQuery, UnsignedBatch, event,
};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

/// Immutable legacy fixture wrapper version.
pub const FIXTURE_VERSION_V1: u16 = EVENT_SCHEMA_VERSION_V1;
/// Immutable legacy fixture wrapper version carrying event schema v2.
pub const FIXTURE_VERSION_V2: u16 = EVENT_SCHEMA_VERSION_V2;
/// Current fixture wrapper version carrying event schema v3.
pub const FIXTURE_VERSION_V3: u16 = EVENT_SCHEMA_VERSION_V3;
/// Fixture version emitted by current writers.
pub const FIXTURE_VERSION: u16 = FIXTURE_VERSION_V3;
/// Fixed synthetic artifact bytes. They contain no personal or production data.
pub const SYNTHETIC_ARTIFACT_BYTES: &[u8] =
    b"SYNTHETIC ONLY: no personal data; no network egress.\n";
/// Default final valid-time coordinate used by fixture replay.
pub const FINAL_VALID_AT: TimestampMillis = TimestampMillis::new(700);
const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
const IMMUTABLE_V1_FIXTURE_JSON: &str =
    include_str!("../../../schemas/fixtures/signed-batch-v1.json");
// v2 joined v1 as a read-only compatibility golden when v3 became the writer
// version. Both are compared against their committed document rather than
// regenerated, so no builder change can rewrite historical signed bytes.
const IMMUTABLE_V2_FIXTURE_JSON: &str =
    include_str!("../../../schemas/fixtures/signed-batch-v2.json");

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
const USER_ID: &str = "01900000-0000-7000-8000-00000000000b";

const CLAIM_AI_UNDERSTOOD: &str = "01900000-0000-7000-8000-000000000201";
const CLAIM_USER_PRACTICED: &str = "01900000-0000-7000-8000-000000000202";
const CLAIM_FRESH_HIGH: &str = "01900000-0000-7000-8000-000000000203";
const CLAIM_FRESH_STALE: &str = "01900000-0000-7000-8000-000000000204";
const CLAIM_DEADLINE_OLD: &str = "01900000-0000-7000-8000-000000000205";
const CLAIM_DEADLINE_NEW: &str = "01900000-0000-7000-8000-000000000206";
const CLAIM_AI_FLUENT: &str = "01900000-0000-7000-8000-000000000207";
const CLAIM_COURSE_OFFERING_PREDICTION: &str = "01900000-0000-7000-8000-000000000208";
const PREDICTION_SUBJECT_ID: &str = "01900000-0000-7000-8000-00000000000c";

// Event schema v3 aggregate identifiers, one per arm in Proto tag order 16..=33.
// `REPOSITORY_ID` is a parent reference only: no v3 arm registers a repository.
const CURRICULUM_VERSION_ID: &str = "01900000-0000-7000-8000-000000000410";
const COURSE_REVISION_ID: &str = "01900000-0000-7000-8000-000000000411";
const OFFERING_ID: &str = "01900000-0000-7000-8000-000000000412";
const ATTEMPT_ID: &str = "01900000-0000-7000-8000-000000000413";
const REQUIREMENT_SET_ID: &str = "01900000-0000-7000-8000-000000000414";
const AUDIT_ID: &str = "01900000-0000-7000-8000-000000000415";
const CAPTURE_PERMISSION_ID: &str = "01900000-0000-7000-8000-000000000416";
const LECTURE_SESSION_ID: &str = "01900000-0000-7000-8000-000000000417";
const TRANSCRIPT_VERSION_ID: &str = "01900000-0000-7000-8000-000000000418";
const LECTURE_DOCUMENT_ID: &str = "01900000-0000-7000-8000-000000000419";
const REPOSITORY_ID: &str = "01900000-0000-7000-8000-00000000041a";
const SNAPSHOT_ID: &str = "01900000-0000-7000-8000-00000000041b";
const FINDING_ID: &str = "01900000-0000-7000-8000-00000000041c";
const MODEL_RUN_AGGREGATE_ID: &str = "01900000-0000-7000-8000-00000000041d";
const PROPOSAL_ID: &str = "01900000-0000-7000-8000-00000000041e";
const EGRESS_DECISION_ID: &str = "01900000-0000-7000-8000-00000000041f";
const CONSENT_ID: &str = "01900000-0000-7000-8000-000000000420";
const ENTITY_IDENTITY_CHANGE_ID: &str = "01900000-0000-7000-8000-000000000421";
const RETENTION_ACTION_ID: &str = "01900000-0000-7000-8000-000000000422";

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
    /// Fixture input was not strict UTF-8.
    #[error("fixture JSON bytes are not strict UTF-8")]
    Utf8(#[from] std::str::Utf8Error),
    /// Fixture hex was malformed.
    #[error("invalid fixture hex: {0}")]
    Hex(#[from] hex::FromHexError),
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
    /// The JSON wrapper did not satisfy its exact schema-level invariants.
    #[error("invalid fixture contract: {0}")]
    InvalidFixtureContract(&'static str),
    /// Wrapper trust metadata did not match the independent fixture authorization.
    #[error("fixture trust metadata does not match the independent trust anchor")]
    FixtureTrustAnchorMismatch,
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
        authorization: &DeviceAuthorization,
    ) -> Result<(VerifiedBatch, AcceptanceReceipt), CoreError> {
        let verified = verify_signed_batch(envelope_bytes, authorization)?;
        let receipt = self.ledger.accept_verified_batch(&verified)?;
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
#[serde(deny_unknown_fields)]
pub struct FixtureDocument {
    #[serde(deserialize_with = "deserialize_json_safe_u16")]
    pub fixture_version: u16,
    pub name: String,
    pub data_class: String,
    pub network_egress: String,
    pub contract: FixtureContract,
    pub device_id: DeviceId,
    pub user_id: academic_domain::EntityId,
    pub public_key_hex: String,
    pub signed_batch_cbor_hex: String,
    pub expected_replay: ReplaySummary,
}

/// Human-inspectable contract metadata for a golden fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureContract {
    pub envelope: String,
    pub payload: String,
    pub signature: String,
    #[serde(deserialize_with = "deserialize_json_safe_u16")]
    pub event_schema_version: u16,
}

/// Deterministic semantic result of replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplaySummary {
    #[serde(deserialize_with = "deserialize_json_safe_u64")]
    pub accepted_events: u64,
    #[serde(deserialize_with = "deserialize_json_safe_u64")]
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_prediction_claims"
    )]
    pub prediction_claims: Option<Vec<PredictionClaimDisclosure>>,
    pub semantic_digest: ContentDigest,
}

/// Human-inspectable current-fixture disclosure for a signed Prediction claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionClaimDisclosure {
    pub claim_id: ClaimId,
    pub confidence: ConfidencePermille,
    pub prediction_metadata: PredictionMetadata,
    pub valid_time: ValidInterval,
}

fn deserialize_present_prediction_claims<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<PredictionClaimDisclosure>>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<PredictionClaimDisclosure>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionClaimDisclosureWire {
    claim_id: ClaimId,
    #[serde(deserialize_with = "deserialize_json_safe_u16")]
    confidence: u16,
    prediction_metadata: PredictionMetadataWire,
    valid_time: PredictionValidIntervalWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionMetadataWire {
    #[serde(deserialize_with = "deserialize_json_safe_u16")]
    version: u16,
    observation_window: PredictionObservationWindowWire,
    #[serde(deserialize_with = "deserialize_json_safe_u32")]
    positive_sample_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionObservationWindowWire {
    #[serde(deserialize_with = "deserialize_json_safe_timestamp")]
    from: TimestampMillis,
    #[serde(deserialize_with = "deserialize_json_safe_timestamp")]
    to: TimestampMillis,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionValidIntervalWire {
    #[serde(deserialize_with = "deserialize_json_safe_timestamp")]
    from: TimestampMillis,
    to: RequiredNullableJsonSafeTimestamp,
}

#[derive(Debug)]
struct RequiredNullableJsonSafeTimestamp(Option<TimestampMillis>);

impl<'de> Deserialize<'de> for RequiredNullableJsonSafeTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RequiredNullableJsonSafeTimestampVisitor;

        impl de::Visitor<'_> for RequiredNullableJsonSafeTimestampVisitor {
            type Value = RequiredNullableJsonSafeTimestamp;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("null or a non-negative JSON safe integer")
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RequiredNullableJsonSafeTimestamp(None))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                json_safe_u64_from_u64(value).map(|value| {
                    RequiredNullableJsonSafeTimestamp(Some(TimestampMillis::new(value as i64)))
                })
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                json_safe_u64_from_i64(value).map(|value| {
                    RequiredNullableJsonSafeTimestamp(Some(TimestampMillis::new(value as i64)))
                })
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                json_safe_u64_from_f64(value).map(|value| {
                    RequiredNullableJsonSafeTimestamp(Some(TimestampMillis::new(value as i64)))
                })
            }
        }

        // `deserialize_any` makes a JSON null visit `visit_unit`, while Serde's
        // missing-field deserializer remains an error. That preserves the
        // fixture contract's required-but-nullable `to` key.
        deserializer.deserialize_any(RequiredNullableJsonSafeTimestampVisitor)
    }
}

impl<'de> Deserialize<'de> for PredictionClaimDisclosure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = PredictionClaimDisclosureWire::deserialize(deserializer)?;
        if value.prediction_metadata.version != PREDICTION_METADATA_VERSION_V1 {
            return Err(de::Error::custom(
                DomainError::UnsupportedPredictionMetadataVersion(
                    value.prediction_metadata.version,
                ),
            ));
        }
        let observation_window = PredictionObservationWindow::new(
            value.prediction_metadata.observation_window.from,
            value.prediction_metadata.observation_window.to,
        )
        .map_err(de::Error::custom)?;
        let prediction_metadata = PredictionMetadata::new(
            observation_window,
            value.prediction_metadata.positive_sample_count,
        )
        .map_err(de::Error::custom)?;
        let valid_time = ValidInterval::new(value.valid_time.from, value.valid_time.to.0)
            .map_err(de::Error::custom)?;
        Ok(Self {
            claim_id: value.claim_id,
            confidence: ConfidencePermille::new(value.confidence).map_err(de::Error::custom)?,
            prediction_metadata,
            valid_time,
        })
    }
}

fn deserialize_json_safe_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct JsonSafeIntegerVisitor;

    impl de::Visitor<'_> for JsonSafeIntegerVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a non-negative JSON safe integer")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            json_safe_u64_from_u64(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            json_safe_u64_from_i64(value)
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            json_safe_u64_from_f64(value)
        }
    }

    deserializer.deserialize_any(JsonSafeIntegerVisitor)
}

fn json_safe_u64_from_u64<E>(value: u64) -> Result<u64, E>
where
    E: de::Error,
{
    if value <= JSON_SAFE_INTEGER_MAX {
        Ok(value)
    } else {
        Err(E::custom("number is not a non-negative JSON safe integer"))
    }
}

fn json_safe_u64_from_i64<E>(value: i64) -> Result<u64, E>
where
    E: de::Error,
{
    u64::try_from(value)
        .map_err(E::custom)
        .and_then(json_safe_u64_from_u64)
}

fn json_safe_u64_from_f64<E>(value: f64) -> Result<u64, E>
where
    E: de::Error,
{
    if value.is_finite()
        && value >= 0.0
        && value <= JSON_SAFE_INTEGER_MAX as f64
        && value.fract() == 0.0
    {
        Ok(value as u64)
    } else {
        Err(E::custom("number is not a non-negative JSON safe integer"))
    }
}

fn deserialize_json_safe_u16<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_json_safe_u64(deserializer)?;
    u16::try_from(value)
        .map_err(|_| <D::Error as de::Error>::custom("number exceeds the unsigned 16-bit range"))
}

fn deserialize_json_safe_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_json_safe_u64(deserializer)?;
    u32::try_from(value)
        .map_err(|_| <D::Error as de::Error>::custom("number exceeds the unsigned 32-bit range"))
}

fn deserialize_json_safe_timestamp<'de, D>(deserializer: D) -> Result<TimestampMillis, D::Error>
where
    D: Deserializer<'de>,
{
    let value = deserialize_json_safe_u64(deserializer)?;
    i64::try_from(value)
        .map(TimestampMillis::new)
        .map_err(|_| <D::Error as de::Error>::custom("timestamp exceeds the signed 64-bit range"))
}

impl FixtureDocument {
    /// Enforces the same exact wrapper constraints as JSON Schema and TypeScript.
    pub fn validate_contract(&self) -> Result<(), CoreError> {
        if !matches!(
            self.fixture_version,
            FIXTURE_VERSION_V1 | FIXTURE_VERSION_V2 | FIXTURE_VERSION_V3
        ) {
            return Err(CoreError::UnsupportedFixtureVersion(self.fixture_version));
        }
        if self.name.is_empty() {
            return Err(CoreError::InvalidFixtureContract("name must be nonempty"));
        }
        if self.data_class != "SYNTHETIC_ONLY" || self.network_egress != "NONE" {
            return Err(CoreError::InvalidFixtureContract(
                "data_class and network_egress must match fixture constants",
            ));
        }
        let expected_payload = match self.fixture_version {
            FIXTURE_VERSION_V1 => "academic.event-batch/v1 deterministic-cbor",
            FIXTURE_VERSION_V2 => "academic.event-batch/v2 deterministic-cbor",
            FIXTURE_VERSION_V3 => "academic.event-batch/v3 deterministic-cbor",
            _ => return Err(CoreError::UnsupportedFixtureVersion(self.fixture_version)),
        };
        if self.contract.envelope != "academic.signed-batch-envelope/v1 deterministic-cbor"
            || self.contract.payload != expected_payload
            || self.contract.signature != "Ed25519"
            || self.contract.event_schema_version != self.fixture_version
        {
            return Err(CoreError::InvalidFixtureContract(
                "contract metadata must match exact versioned constants",
            ));
        }
        if !is_lower_hex(&self.public_key_hex, Some(64))
            || !is_lower_hex(&self.signed_batch_cbor_hex, None)
            || self.signed_batch_cbor_hex.len() < 2
        {
            return Err(CoreError::InvalidFixtureContract(
                "fixture hex must be nonempty canonical lowercase bytes",
            ));
        }
        if self.expected_replay.accepted_events == 0
            || self.expected_replay.accepted_events > JSON_SAFE_INTEGER_MAX
            || self.expected_replay.accept_seq_head == 0
            || self.expected_replay.accept_seq_head > JSON_SAFE_INTEGER_MAX
        {
            return Err(CoreError::InvalidFixtureContract(
                "replay counts must be positive JSON-safe integers",
            ));
        }
        for ids in [
            &self.expected_replay.mastery_active_claim_ids,
            &self.expected_replay.mastery_conflicting_claim_ids,
            &self.expected_replay.mastery_rejected_claim_ids,
            &self.expected_replay.deadline_active_claim_ids,
        ] {
            if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
                return Err(CoreError::InvalidFixtureContract(
                    "replay claim id arrays must contain unique values",
                ));
            }
        }
        match (
            self.fixture_version,
            self.expected_replay.prediction_claims.as_deref(),
        ) {
            (FIXTURE_VERSION_V1, None) => {}
            (FIXTURE_VERSION_V1, Some(_)) => {
                return Err(CoreError::InvalidFixtureContract(
                    "v1 replay must not invent prediction disclosures",
                ));
            }
            (FIXTURE_VERSION_V2 | FIXTURE_VERSION_V3, Some(disclosures))
                if !disclosures.is_empty() =>
            {
                let ids = disclosures
                    .iter()
                    .map(|disclosure| disclosure.claim_id)
                    .collect::<BTreeSet<_>>();
                if ids.len() != disclosures.len() {
                    return Err(CoreError::InvalidFixtureContract(
                        "prediction claim disclosures must have unique claim ids",
                    ));
                }
                for disclosure in disclosures {
                    disclosure.prediction_metadata.validate()?;
                }
            }
            (FIXTURE_VERSION_V2 | FIXTURE_VERSION_V3, _) => {
                return Err(CoreError::InvalidFixtureContract(
                    "v2 and v3 replay require prediction claim disclosures",
                ));
            }
            (other, _) => return Err(CoreError::UnsupportedFixtureVersion(other)),
        }
        Ok(())
    }
}

fn json_number_token_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    match bytes.get(index) {
        Some(b'0') => index += 1,
        Some(b'1'..=b'9') => {
            index += 1;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
        }
        _ => return None,
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let fraction_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        index += 1;
        if bytes
            .get(index)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            index += 1;
        }
        let exponent_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == exponent_start {
            return None;
        }
    }
    Some(index)
}

fn bounded_decimal_magnitude(digits: &[u8], limit: usize) -> usize {
    let mut value = 0_usize;
    for digit in digits {
        let digit = usize::from(*digit - b'0');
        if value > limit.saturating_sub(digit) / 10 {
            return limit.saturating_add(1);
        }
        value = value * 10 + digit;
        if value > limit {
            return limit.saturating_add(1);
        }
    }
    value
}

fn is_nonnegative_mathematical_integer_token(token: &str) -> bool {
    if token.starts_with('-') {
        return false;
    }
    let bytes = token.as_bytes();
    let exponent_marker = bytes.iter().position(|byte| matches!(byte, b'e' | b'E'));
    let mantissa_end = exponent_marker.unwrap_or(bytes.len());
    let fraction_digits = bytes[..mantissa_end]
        .iter()
        .position(|byte| *byte == b'.')
        .map_or(0, |point| mantissa_end - point - 1);
    let coefficient_is_zero = bytes[..mantissa_end]
        .iter()
        .filter(|byte| byte.is_ascii_digit())
        .all(|byte| *byte == b'0');
    if coefficient_is_zero {
        return true;
    }
    let trailing_zeros = bytes[..mantissa_end]
        .iter()
        .rev()
        .filter(|byte| **byte != b'.')
        .take_while(|byte| **byte == b'0')
        .count();
    let Some(marker) = exponent_marker else {
        return trailing_zeros >= fraction_digits;
    };
    let mut exponent = &bytes[marker + 1..];
    let exponent_is_negative = exponent.first() == Some(&b'-');
    if exponent
        .first()
        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
    {
        exponent = &exponent[1..];
    }
    let magnitude = bounded_decimal_magnitude(
        exponent,
        fraction_digits
            .saturating_add(trailing_zeros)
            .saturating_add(1),
    );
    if exponent_is_negative {
        magnitude <= trailing_zeros && fraction_digits <= trailing_zeros.saturating_sub(magnitude)
    } else {
        magnitude >= fraction_digits || trailing_zeros >= fraction_digits - magnitude
    }
}

fn assert_fixture_integer_lexemes(input: &str) -> Result<(), CoreError> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' => index = index.saturating_add(2),
                    b'"' => {
                        index += 1;
                        break;
                    }
                    _ => index += 1,
                }
            }
            continue;
        }
        if bytes[index] == b'-' || bytes[index].is_ascii_digit() {
            let end = json_number_token_end(bytes, index).ok_or(
                CoreError::InvalidFixtureContract("fixture number token is malformed"),
            )?;
            let token = &input[index..end];
            if !is_nonnegative_mathematical_integer_token(token) {
                return Err(CoreError::InvalidFixtureContract(
                    "fixture integer fields must be mathematically integral before conversion",
                ));
            }
            index = end;
            continue;
        }
        index += 1;
    }
    Ok(())
}

/// Parses original fixture-wrapper bytes before any normalized value boundary.
///
/// UTF-8 decoding is strict, raw arbitrary-precision number lexemes must be
/// mathematically integral, and Serde then enforces decoded-name, scalar-string,
/// bounded-value, and exact typed-object semantics.
pub fn parse_fixture_document_json(input: &[u8]) -> Result<FixtureDocument, CoreError> {
    let text = std::str::from_utf8(input)?;
    assert_fixture_integer_lexemes(text)?;
    let document: FixtureDocument = serde_json::from_str(text)?;
    document.validate_contract()?;
    Ok(document)
}

fn is_lower_hex(value: &str, exact_len: Option<usize>) -> bool {
    exact_len.is_none_or(|length| value.len() == length)
        && value.len().is_multiple_of(2)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReplayDigestMaterial<'a> {
    accepted_events: u64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    prediction_claims: Option<&'a [PredictionClaimDisclosure]>,
}

/// Returns the frozen, read-only event schema v2 compatibility fixture.
///
/// The Phase 1 store lane still ingests this document: `ledger_event.event_kind`
/// is a closed CHECK over the v1/v2 arms and gains the v3 values only with
/// migration 0004, so a batch carrying a v3 arm has no canonical table yet. Read
/// verification upcasts it to v3 without rewriting a byte of it.
pub fn immutable_v2_fixture_document() -> Result<FixtureDocument, CoreError> {
    let document: FixtureDocument = serde_json::from_str(IMMUTABLE_V2_FIXTURE_JSON)?;
    document.validate_contract()?;
    Ok(document)
}

/// Builds the current deterministic, signed, synthetic Phase 0 fixture.
pub fn build_fixture_document() -> Result<FixtureDocument, CoreError> {
    let batch = build_unsigned_fixture_batch()?;
    let signing_key = fixture_signing_key();
    let authorization = fixture_device_authorization()?;
    let signed = sign_batch(&batch, &signing_key)?;
    let mut core = Core::new();
    let (verified, _) = core.accept_signed_batch(&signed, &authorization)?;
    if verified.source_schema_version() != FIXTURE_VERSION_V3 {
        return Err(CoreError::FixtureDrift);
    }
    let expected_replay = summarize_replay(&core, &verified, FINAL_VALID_AT, u64::MAX)?;
    Ok(FixtureDocument {
        fixture_version: FIXTURE_VERSION_V3,
        name: "phase0-synthetic-bitemporal-ledger-v3".to_owned(),
        data_class: "SYNTHETIC_ONLY".to_owned(),
        network_egress: "NONE".to_owned(),
        contract: FixtureContract {
            envelope: "academic.signed-batch-envelope/v1 deterministic-cbor".to_owned(),
            payload: "academic.event-batch/v3 deterministic-cbor".to_owned(),
            signature: "Ed25519".to_owned(),
            event_schema_version: FIXTURE_VERSION_V3,
        },
        device_id: authorization.device_id(),
        user_id: authorization.user_id(),
        public_key_hex: hex::encode(signing_key.verifying_key().as_bytes()),
        signed_batch_cbor_hex: hex::encode(signed),
        expected_replay,
    })
}

/// Verifies, accepts, replays, and compares a fixture with the deterministic builder.
pub fn verify_fixture_document(document: &FixtureDocument) -> Result<ReplaySummary, CoreError> {
    document.validate_contract()?;
    let authorization = fixture_device_authorization()?;
    ensure_fixture_trust_anchor(document, &authorization)?;
    let signed = hex::decode(&document.signed_batch_cbor_hex)?;
    let mut core = Core::new();
    let (verified, _) = core.accept_signed_batch(&signed, &authorization)?;
    if verified.source_schema_version() != document.contract.event_schema_version {
        return Err(CoreError::FixtureDrift);
    }
    let actual = summarize_replay(&core, &verified, FINAL_VALID_AT, u64::MAX)?;
    if actual != document.expected_replay {
        return Err(CoreError::ExpectedReplayMismatch);
    }
    match document.fixture_version {
        FIXTURE_VERSION_V1 => {
            let immutable: FixtureDocument = serde_json::from_str(IMMUTABLE_V1_FIXTURE_JSON)?;
            if *document != immutable {
                return Err(CoreError::FixtureDrift);
            }
        }
        FIXTURE_VERSION_V2 => {
            let immutable: FixtureDocument = serde_json::from_str(IMMUTABLE_V2_FIXTURE_JSON)?;
            if *document != immutable {
                return Err(CoreError::FixtureDrift);
            }
        }
        FIXTURE_VERSION_V3 => {
            if *document != build_fixture_document()? {
                return Err(CoreError::FixtureDrift);
            }
        }
        other => return Err(CoreError::UnsupportedFixtureVersion(other)),
    }
    Ok(actual)
}

/// Replays a fixture at caller-selected valid and known coordinates.
pub fn replay_fixture_document(
    document: &FixtureDocument,
    valid_at: TimestampMillis,
    known_at_accept_seq: u64,
) -> Result<ReplaySummary, CoreError> {
    document.validate_contract()?;
    let authorization = fixture_device_authorization()?;
    ensure_fixture_trust_anchor(document, &authorization)?;
    let signed = hex::decode(&document.signed_batch_cbor_hex)?;
    let mut core = Core::new();
    let (verified, _) = core.accept_signed_batch(&signed, &authorization)?;
    if verified.source_schema_version() != document.contract.event_schema_version {
        return Err(CoreError::FixtureDrift);
    }
    summarize_replay(&core, &verified, valid_at, known_at_accept_seq)
}

/// Serializes a fixture with stable pretty JSON and a final newline.
pub fn fixture_json(document: &FixtureDocument) -> Result<String, CoreError> {
    let mut json = serde_json::to_string_pretty(document)?;
    json.push('\n');
    Ok(json)
}

fn fixture_device_authorization() -> Result<DeviceAuthorization, DomainError> {
    Ok(DeviceAuthorization::new(
        parse_id(DEVICE_ID)?,
        parse_id(USER_ID)?,
        fixture_signing_key().verifying_key(),
    ))
}

fn ensure_fixture_trust_anchor(
    document: &FixtureDocument,
    authorization: &DeviceAuthorization,
) -> Result<(), CoreError> {
    if document.device_id != authorization.device_id()
        || document.user_id != authorization.user_id()
        || document.public_key_hex != hex::encode(authorization.verifying_key().as_bytes())
    {
        return Err(CoreError::FixtureTrustAnchorMismatch);
    }
    Ok(())
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
        parse_id(SCOPE_ID)?,
        mastery_predicate,
        freshness_predicate,
        valid_at,
        known_at_accept_seq,
    );
    let deadline = core.ledger().resolve(&ResolutionQuery {
        subject_entity_id: parse_id(DEADLINE_SUBJECT_ID)?,
        scope_id: parse_id(SCOPE_ID)?,
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
    let prediction_claims = if verified.source_schema_version() == EVENT_SCHEMA_VERSION_V1 {
        None
    } else {
        let claim = core
            .ledger()
            .claim(parse_id(CLAIM_COURSE_OFFERING_PREDICTION)?)
            .ok_or(CoreError::MissingProjection("prediction claim"))?;
        Some(vec![PredictionClaimDisclosure {
            claim_id: claim.id,
            confidence: claim
                .confidence
                .ok_or(DomainError::MissingPredictionConfidence)?,
            prediction_metadata: claim
                .prediction_metadata
                .ok_or(DomainError::MissingPredictionMetadata)?,
            valid_time: claim.valid_time,
        }])
    };
    let material = ReplayDigestMaterial {
        accepted_events: u64::try_from(core.ledger().accepted_events().len())
            .map_err(|_| CoreError::MissingProjection("accepted event count"))?,
        accept_seq_head: core.ledger().accept_seq_head(),
        payload_hash: verified.payload_hash(),
        envelope_hash: verified.envelope_hash(),
        artifact_digest: artifact.content_digest,
        artifact_locator: &artifact.vault_locator,
        mastery,
        freshness,
        mastery_active_claim_ids: &knowledge.mastery_resolution.active_claim_ids,
        mastery_conflicting_claim_ids: &knowledge.mastery_resolution.conflicting_claim_ids,
        mastery_rejected_claim_ids: &knowledge.mastery_resolution.rejected_claim_ids,
        deadline_active_claim_ids: &deadline.active_claim_ids,
        prediction_claims: prediction_claims.as_deref(),
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
        prediction_claims,
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
    prediction_metadata: Option<PredictionMetadata>,
    valid_time: ValidInterval,
) -> Result<Claim, DomainError> {
    let (authority_class, epistemic_status, confidence) = provenance;
    Ok(Claim {
        id: parse_id(id)?,
        subject_entity_id: parse_id(subject)?,
        predicate_id: PredicateId::parse(predicate)?,
        object,
        scope_id: parse_id(SCOPE_ID)?,
        authority_class,
        epistemic_status,
        confidence: confidence.map(ConfidencePermille::new).transpose()?,
        prediction_metadata,
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
    let artifact_byte_length = u64::try_from(SYNTHETIC_ARTIFACT_BYTES.len())
        .map_err(|_| CoreError::MissingProjection("artifact byte length"))?;
    let evidence_locator = EvidenceLocator::TextBytes {
        source_digest: artifact_digest,
        start: 0,
        end: artifact_byte_length,
    };
    let artifact = ArtifactDescriptor {
        id: parse_id(ARTIFACT_ID)?,
        content_digest: artifact_digest,
        media_type,
        byte_length: artifact_byte_length,
        domain_id: parse_id(DOMAIN_ID)?,
        confidentiality: Confidentiality::Personal,
        retention_class: RetentionClass::UserManaged,
        permission_lineage_id: parse_id(PERMISSION_ID)?,
        format_version: 1,
        vault_locator: artifact_locator,
        evidence_representations: vec![ArtifactRepresentation {
            locator: evidence_locator.clone(),
            content_digest: artifact_digest,
            byte_length: artifact_byte_length,
        }],
    };
    let evidence = EvidenceItem {
        id: parse_id(EVIDENCE_ID)?,
        artifact_id: parse_id(ARTIFACT_ID)?,
        locator: evidence_locator,
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
    let user_actor = Actor::User {
        user_id: parse_id(USER_ID)?,
    };
    let registrar_actor = Actor::Importer {
        name: "synthetic.registrar.fixture".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let scope = ScopeDescriptor {
        id: parse_id::<ScopeId>(SCOPE_ID)?,
        domain_id: parse_id(DOMAIN_ID)?,
        label: "phase0.synthetic.scope".to_owned(),
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
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
        None,
        valid_from_200,
    )?;
    let course_offering_prediction = claim(
        CLAIM_COURSE_OFFERING_PREDICTION,
        PREDICTION_SUBJECT_ID,
        "academic.course.offering",
        ClaimObject::Boolean(true),
        (
            AuthorityClass::Prediction,
            EpistemicStatus::Prediction,
            Some(720),
        ),
        Some(PredictionMetadata::new(
            PredictionObservationWindow::new(TimestampMillis::new(100), TimestampMillis::new(700))?,
            6,
        )?),
        ValidInterval::new(TimestampMillis::new(800), Some(TimestampMillis::new(1_200)))?,
    )?;

    let events = vec![
        fixture_event(
            1,
            official_actor.clone(),
            EventPayload::ScopeRegistered(scope),
        )?,
        fixture_event(
            2,
            official_actor.clone(),
            EventPayload::ArtifactRegistered(artifact),
        )?,
        fixture_event(
            3,
            official_actor.clone(),
            EventPayload::EvidenceRegistered(evidence),
        )?,
        fixture_event(
            4,
            ai_actor.clone(),
            EventPayload::ClaimAsserted(ai_understood),
        )?,
        fixture_event(
            5,
            user_actor.clone(),
            EventPayload::DecisionRecorded(UserDecision {
                id: parse_id::<DecisionId>("01900000-0000-7000-8000-000000000301")?,
                target_claim_id: parse_id(CLAIM_AI_UNDERSTOOD)?,
                target_object: ClaimObject::Mastery(MasteryLevel::Understood),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: parse_id(CONCEPT_ID)?,
                    predicate_id: PredicateId::parse("knowledge.mastery")?,
                    scope_id: parse_id(SCOPE_ID)?,
                },
                action: DecisionAction::Reject,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
                rationale_evidence_ids: vec![parse_id(EVIDENCE_ID)?],
                decided_at: TimestampMillis::new(104),
                reversible_until: None,
            }),
        )?,
        fixture_event(
            6,
            user_actor.clone(),
            EventPayload::ClaimAsserted(user_practiced),
        )?,
        fixture_event(
            7,
            user_actor,
            EventPayload::DecisionRecorded(UserDecision {
                id: parse_id::<DecisionId>("01900000-0000-7000-8000-000000000302")?,
                target_claim_id: parse_id(CLAIM_USER_PRACTICED)?,
                target_object: ClaimObject::Mastery(MasteryLevel::Practiced),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: parse_id(CONCEPT_ID)?,
                    predicate_id: PredicateId::parse("knowledge.mastery")?,
                    scope_id: parse_id(SCOPE_ID)?,
                },
                action: DecisionAction::Confirm,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(200)),
                rationale_evidence_ids: vec![parse_id(EVIDENCE_ID)?],
                decided_at: TimestampMillis::new(106),
                reversible_until: None,
            }),
        )?,
        fixture_event(
            8,
            engine_actor.clone(),
            EventPayload::ClaimAsserted(fresh_high),
        )?,
        fixture_event(9, engine_actor, EventPayload::ClaimAsserted(fresh_stale))?,
        fixture_event(
            10,
            official_actor.clone(),
            EventPayload::ClaimAsserted(deadline_old),
        )?,
        fixture_event(
            11,
            official_actor.clone(),
            EventPayload::ClaimAsserted(deadline_new),
        )?,
        fixture_event(
            12,
            official_actor,
            EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: parse_id(CLAIM_DEADLINE_NEW)?,
                target_claim_id: parse_id(CLAIM_DEADLINE_OLD)?,
                kind: ClaimRelationKind::Supersedes,
                scope_id: parse_id(SCOPE_ID)?,
            }),
        )?,
        fixture_event(13, ai_actor.clone(), EventPayload::ClaimAsserted(ai_fluent))?,
        fixture_event(
            14,
            ai_actor,
            EventPayload::ClaimAsserted(course_offering_prediction),
        )?,
        // Event schema v3 arms, Proto tags 16..=33 in declaration order. Each
        // registers an aggregate at registration depth only; `source_digest` is
        // present on the arms whose fixture registration ingests a document and
        // absent on the rest, so both encodings appear in signed golden bytes.
        fixture_event(
            15,
            registrar_actor.clone(),
            EventPayload::CurriculumVersionPublished(CurriculumVersionRegistration {
                id: parse_id(CURRICULUM_VERSION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
            }),
        )?,
        fixture_event(
            16,
            registrar_actor.clone(),
            EventPayload::CourseRevisionPublished(CourseRevisionRegistration {
                id: parse_id(COURSE_REVISION_ID)?,
                curriculum_version_id: parse_id(CURRICULUM_VERSION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
            }),
        )?,
        fixture_event(
            17,
            registrar_actor.clone(),
            EventPayload::OfferingObserved(OfferingRegistration {
                id: parse_id(OFFERING_ID)?,
                course_revision_id: parse_id(COURSE_REVISION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(200)),
            }),
        )?,
        fixture_event(
            18,
            registrar_actor.clone(),
            EventPayload::AttemptRecorded(AttemptRegistration {
                id: parse_id(ATTEMPT_ID)?,
                offering_id: parse_id(OFFERING_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(200)),
            }),
        )?,
        fixture_event(
            19,
            registrar_actor.clone(),
            EventPayload::RequirementSetPublished(RequirementSetRegistration {
                id: parse_id(REQUIREMENT_SET_ID)?,
                curriculum_version_id: parse_id(CURRICULUM_VERSION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
            }),
        )?,
        fixture_event(
            20,
            registrar_actor.clone(),
            EventPayload::AuditComputed(AuditRegistration {
                id: parse_id(AUDIT_ID)?,
                requirement_set_id: parse_id(REQUIREMENT_SET_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(300)),
            }),
        )?,
        fixture_event(
            21,
            registrar_actor.clone(),
            EventPayload::CapturePermissionRecorded(CapturePermissionRegistration {
                id: parse_id(CAPTURE_PERMISSION_ID)?,
                offering_id: parse_id(OFFERING_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(200)),
            }),
        )?,
        fixture_event(
            22,
            registrar_actor.clone(),
            EventPayload::LectureSessionRecorded(LectureSessionRegistration {
                id: parse_id(LECTURE_SESSION_ID)?,
                offering_id: parse_id(OFFERING_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(300)),
            }),
        )?,
        fixture_event(
            23,
            registrar_actor.clone(),
            EventPayload::TranscriptVersionAdded(TranscriptVersionRegistration {
                id: parse_id(TRANSCRIPT_VERSION_ID)?,
                lecture_session_id: parse_id(LECTURE_SESSION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(300)),
            }),
        )?,
        fixture_event(
            24,
            registrar_actor.clone(),
            EventPayload::LectureDocumentPublished(LectureDocumentRegistration {
                id: parse_id(LECTURE_DOCUMENT_ID)?,
                lecture_session_id: parse_id(LECTURE_SESSION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(300)),
            }),
        )?,
        fixture_event(
            25,
            registrar_actor.clone(),
            EventPayload::SnapshotRegistered(SnapshotRegistration {
                id: parse_id(SNAPSHOT_ID)?,
                repository_id: parse_id(REPOSITORY_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: Some(artifact_digest),
                valid_time: ValidInterval::open_ended(TimestampMillis::new(400)),
            }),
        )?,
        fixture_event(
            26,
            registrar_actor.clone(),
            EventPayload::FindingPublished(FindingRegistration {
                id: parse_id(FINDING_ID)?,
                snapshot_id: parse_id(SNAPSHOT_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(400)),
            }),
        )?,
        fixture_event(
            27,
            registrar_actor.clone(),
            EventPayload::ModelRunRecorded(ModelRunRegistration {
                id: parse_id(MODEL_RUN_AGGREGATE_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
            }),
        )?,
        fixture_event(
            28,
            registrar_actor.clone(),
            EventPayload::ProposalDisposed(ProposalDispositionRegistration {
                id: parse_id(PROPOSAL_ID)?,
                model_run_id: parse_id(MODEL_RUN_AGGREGATE_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
            }),
        )?,
        fixture_event(
            29,
            registrar_actor.clone(),
            EventPayload::EgressDecided(EgressDecisionRegistration {
                id: parse_id(EGRESS_DECISION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(500)),
            }),
        )?,
        fixture_event(
            30,
            registrar_actor.clone(),
            EventPayload::ConsentRecorded(ConsentRegistration {
                id: parse_id(CONSENT_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(500)),
            }),
        )?,
        fixture_event(
            31,
            registrar_actor.clone(),
            EventPayload::EntityIdentityChanged(EntityIdentityChangeRegistration {
                id: parse_id(ENTITY_IDENTITY_CHANGE_ID)?,
                entity_id: parse_id(CONCEPT_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(600)),
            }),
        )?,
        fixture_event(
            32,
            registrar_actor.clone(),
            EventPayload::RetentionActionRecorded(RetentionActionRegistration {
                id: parse_id(RETENTION_ACTION_ID)?,
                domain_id: parse_id(DOMAIN_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
                source_digest: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(600)),
            }),
        )?,
    ];
    Ok(UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: parse_id::<BatchId>(BATCH_ID)?,
        device_id: parse_id::<DeviceId>(DEVICE_ID)?,
        origin_seq_start: 1,
        origin_seq_end: u64::try_from(events.len())
            .map_err(|_| CoreError::MissingProjection("event count"))?,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(114),
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
            known_at: 4,
            expected_mastery: Some(MasteryLevel::Understood),
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "rejected-ai-mastery-is-not-active",
            valid_at: TimestampMillis::new(250),
            known_at: 5,
            expected_mastery: None,
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "future-valid-user-claim-not-yet-valid",
            valid_at: TimestampMillis::new(150),
            known_at: 6,
            expected_mastery: None,
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "user-mastery-active-at-valid-time",
            valid_at: TimestampMillis::new(250),
            known_at: 6,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "explicit-confirmation-preserves-user-mastery",
            valid_at: TimestampMillis::new(250),
            known_at: 7,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: None,
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "freshness-high-in-first-valid-window",
            valid_at: TimestampMillis::new(250),
            known_at: 8,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "accepted-stale-claim-does-not-apply-before-valid-from",
            valid_at: TimestampMillis::new(250),
            known_at: 9,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "freshness-stale-without-mastery-decay",
            valid_at: TimestampMillis::new(700),
            known_at: 9,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::Stale),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "old-official-deadline-as-known",
            valid_at: TimestampMillis::new(400),
            known_at: 10,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![parse_id(CLAIM_DEADLINE_OLD)?],
        },
        TimeTravelCase {
            name: "competing-official-deadlines-before-relation",
            valid_at: TimestampMillis::new(400),
            known_at: 11,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "official-correction-after-supersession",
            valid_at: TimestampMillis::new(400),
            known_at: 12,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![parse_id(CLAIM_DEADLINE_NEW)?],
        },
        TimeTravelCase {
            name: "official-correction-not-valid-before-effective-time",
            valid_at: TimestampMillis::new(250),
            known_at: 12,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::High),
            expected_deadline_claims: vec![],
        },
        TimeTravelCase {
            name: "later-ai-does-not-override-user-decision",
            valid_at: TimestampMillis::new(700),
            known_at: 13,
            expected_mastery: Some(MasteryLevel::Practiced),
            expected_freshness: Some(FreshnessBand::Stale),
            expected_deadline_claims: vec![parse_id(CLAIM_DEADLINE_NEW)?],
        },
    ])
}

#[cfg(test)]
mod tests {
    use academic_contracts::{
        decode_claim_relation_event_proto, encode_claim_relation_event_proto,
    };
    use academic_domain::LogicalPath;

    use super::*;

    #[test]
    fn signed_fixture_round_trips_and_replays() -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let replay = verify_fixture_document(&document)?;
        assert_eq!(replay.accepted_events, 32);
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
        let prediction_claims = replay
            .prediction_claims
            .ok_or(CoreError::MissingProjection("prediction disclosures"))?;
        assert_eq!(prediction_claims.len(), 1);
        let disclosure = &prediction_claims[0];
        assert_eq!(
            disclosure.claim_id,
            parse_id(CLAIM_COURSE_OFFERING_PREDICTION)?
        );
        assert_eq!(disclosure.confidence, ConfidencePermille::new(720)?);
        assert_eq!(disclosure.prediction_metadata.positive_sample_count(), 6);
        assert_eq!(
            disclosure.prediction_metadata.observation_window().from(),
            TimestampMillis::new(100)
        );
        assert_eq!(disclosure.valid_time.from(), TimestampMillis::new(800));
        Ok(())
    }

    /// Neither historical fixture moved a byte when v3 became the writer version.
    ///
    /// Both documents are restated by their frozen SHA-256, both are compared
    /// against the exact committed text rather than regenerated, and the
    /// deterministic builder is proved to emit neither of them. `git diff
    /// --exit-code -- schemas/fixtures/` is the same claim at the tree level.
    #[test]
    fn v1_and_v2_bytes_remain_byte_identical() -> Result<(), Box<dyn std::error::Error>> {
        let committed_v1 = include_str!("../../../schemas/fixtures/signed-batch-v1.json");
        let committed_v2 = include_str!("../../../schemas/fixtures/signed-batch-v2.json");
        assert_eq!(
            ContentDigest::sha256(committed_v1.as_bytes()).to_string(),
            "sha256:287f7dea8fd24c3c6eb205c3f1e2873f6afdf7d6532fe7be4fccfb44a0b7e163"
        );
        assert_eq!(
            ContentDigest::sha256(committed_v2.as_bytes()).to_string(),
            "sha256:f94dfcf7e3e376e54b5514ceb3016b0b7d97d17366562f7ac4a16286d3aa367d"
        );

        let built = fixture_json(&build_fixture_document()?)?;
        assert_ne!(built, committed_v1, "the writer cannot mint v1");
        assert_ne!(built, committed_v2, "the writer cannot mint v2");

        let v1: FixtureDocument = serde_json::from_str(committed_v1)?;
        let v2: FixtureDocument = serde_json::from_str(committed_v2)?;
        assert_eq!(v1.fixture_version, FIXTURE_VERSION_V1);
        assert_eq!(v2.fixture_version, FIXTURE_VERSION_V2);
        assert_eq!(v2, immutable_v2_fixture_document()?);

        // Both still verify and replay to their frozen digests through the v3
        // reader, so compatibility is executable rather than asserted.
        let v1_replay = verify_fixture_document(&v1)?;
        assert_eq!(
            v1_replay.payload_hash.to_string(),
            "sha256:b45c7eea2e1b7bf8071638a31c790519f96a7b0e17bd6963866495557701c3c9"
        );
        assert_eq!(
            v1_replay.envelope_hash.to_string(),
            "sha256:dc498fa435985b94fd573ce1e94b4b7da16646ebd2abe73865d33e43070968ed"
        );
        let v2_replay = verify_fixture_document(&v2)?;
        assert_eq!(
            v2_replay.payload_hash.to_string(),
            "sha256:4d326913780bbf93c61d7e4b20492ccf5c2553f53e61994874b290c78e3638fc"
        );
        assert_eq!(
            v2_replay.envelope_hash.to_string(),
            "sha256:9fb709fd242c4ff4992337a7813e06b9c4e20174a1de22a57e28013e9b6994b6"
        );

        let authorization = fixture_device_authorization()?;
        for (document, source) in [
            (&v1, EVENT_SCHEMA_VERSION_V1),
            (&v2, EVENT_SCHEMA_VERSION_V2),
        ] {
            let signed = hex::decode(&document.signed_batch_cbor_hex)?;
            let verified = verify_signed_batch(&signed, &authorization)?;
            assert_eq!(verified.source_schema_version(), source);
            assert_eq!(verified.batch().schema_version, EVENT_SCHEMA_VERSION_V3);
            assert_eq!(
                verified.source_envelope(),
                signed.as_slice(),
                "verification retains the original envelope bytes"
            );
            assert!(
                verified
                    .batch()
                    .events
                    .iter()
                    .all(|event| event.payload.registration().is_none()),
                "a historical batch never gains a v3 arm by being read"
            );
        }
        Ok(())
    }

    /// The committed v3 fixture is exactly what the deterministic builder emits.
    #[test]
    fn t093_v3_fixture_matches_the_deterministic_builder() -> Result<(), Box<dyn std::error::Error>>
    {
        let committed_v3 = include_str!("../../../schemas/fixtures/signed-batch-v3.json");
        let document: FixtureDocument = serde_json::from_str(committed_v3)?;
        assert_eq!(document.fixture_version, FIXTURE_VERSION_V3);
        assert_eq!(fixture_json(&build_fixture_document()?)?, committed_v3);
        verify_fixture_document(&document)?;

        let authorization = fixture_device_authorization()?;
        let verified = verify_signed_batch(
            &hex::decode(&document.signed_batch_cbor_hex)?,
            &authorization,
        )?;
        assert_eq!(verified.source_schema_version(), EVENT_SCHEMA_VERSION_V3);
        let registered = verified
            .batch()
            .events
            .iter()
            .filter_map(|event| event.payload.registration().map(|view| view.kind))
            .collect::<Vec<_>>();
        assert_eq!(
            registered,
            academic_domain::V3_EVENT_KINDS.to_vec(),
            "the v3 fixture exercises every v3 arm exactly once, in tag order"
        );
        assert!(
            verified.batch().events.iter().any(|event| matches!(
                &event.payload,
                EventPayload::SnapshotRegistered(record) if record.source_digest.is_none()
            )) || verified.batch().events.iter().any(|event| event
                .payload
                .registration()
                .is_some_and(|_| matches!(
                    &event.payload,
                    EventPayload::AttemptRecorded(record) if record.source_digest.is_none()
                ))),
            "the v3 fixture must sign at least one arm with no source digest"
        );
        Ok(())
    }

    #[test]
    fn t010_v1_fixture_verification_is_constrained_to_the_exact_frozen_golden()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut document: FixtureDocument = serde_json::from_str(IMMUTABLE_V1_FIXTURE_JSON)?;
        document.name.push_str("-distinct");
        assert!(matches!(
            verify_fixture_document(&document),
            Err(CoreError::FixtureDrift)
        ));
        Ok(())
    }

    #[test]
    fn adr_003_has_at_least_twelve_bitemporal_examples() -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let authorization = fixture_device_authorization()?;
        let signed = hex::decode(&document.signed_batch_cbor_hex)?;
        let mut core = Core::new();
        let (verified, _) = core.accept_signed_batch(&signed, &authorization)?;
        let cases = time_travel_cases()?;
        assert!(cases.len() >= 12);

        for case in cases {
            let knowledge = core.ledger().knowledge_state_as_of(
                parse_id(CONCEPT_ID)?,
                parse_id(SCOPE_ID)?,
                PredicateId::parse("knowledge.mastery")?,
                PredicateId::parse("knowledge.freshness")?,
                case.valid_at,
                case.known_at,
            );
            let deadline = core.ledger().resolve(&ResolutionQuery {
                subject_entity_id: parse_id(DEADLINE_SUBJECT_ID)?,
                scope_id: parse_id(SCOPE_ID)?,
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
        assert_eq!(verified.batch().events.len(), 32);
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

    #[test]
    fn rust_fixture_wrapper_matches_schema_minimum_unique_and_exact_property_rules()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;

        let mut empty_name = document.clone();
        empty_name.name.clear();
        assert!(empty_name.validate_contract().is_err());

        let mut wrong_const = document.clone();
        wrong_const.contract.payload = "wrong".to_owned();
        assert!(wrong_const.validate_contract().is_err());

        let mut zero_minimum = document.clone();
        zero_minimum.expected_replay.accepted_events = 0;
        assert!(zero_minimum.validate_contract().is_err());

        let mut unsafe_integer = document.clone();
        unsafe_integer.expected_replay.accepted_events = JSON_SAFE_INTEGER_MAX + 1;
        assert!(unsafe_integer.validate_contract().is_err());

        let mut duplicate = document.clone();
        let first = duplicate.expected_replay.mastery_active_claim_ids[0];
        duplicate
            .expected_replay
            .mastery_active_claim_ids
            .push(first);
        assert!(duplicate.validate_contract().is_err());

        let mut missing_prediction_disclosure = document.clone();
        missing_prediction_disclosure
            .expected_replay
            .prediction_claims = None;
        assert!(missing_prediction_disclosure.validate_contract().is_err());

        let mut with_extra = serde_json::to_value(document)?;
        with_extra["unexpected"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<FixtureDocument>(with_extra).is_err());

        let integer_lexeme = fixture_json(&build_fixture_document()?)?.replacen(
            "\"accepted_events\": 32",
            "\"accepted_events\": 32.0",
            1,
        );
        let parsed = parse_fixture_document_json(integer_lexeme.as_bytes())?;
        assert_eq!(parsed.expected_replay.accepted_events, 32);

        for (needle, replacement) in [
            ("\"fixture_version\": 3", "\"fixture_version\": 3.0"),
            (
                "\"event_schema_version\": 3",
                "\"event_schema_version\": 3e0",
            ),
        ] {
            let version_lexeme =
                fixture_json(&build_fixture_document()?)?.replacen(needle, replacement, 1);
            let parsed = parse_fixture_document_json(version_lexeme.as_bytes())?;
            assert_eq!(parsed.fixture_version, FIXTURE_VERSION_V3);
            assert_eq!(
                parsed.contract.event_schema_version,
                EVENT_SCHEMA_VERSION_V3
            );
        }
        for replacement in ["3.5", "65536", "-1"] {
            let invalid = fixture_json(&build_fixture_document()?)?.replacen(
                "\"fixture_version\": 3",
                &format!("\"fixture_version\": {replacement}"),
                1,
            );
            assert!(parse_fixture_document_json(invalid.as_bytes()).is_err());
        }
        Ok(())
    }

    #[test]
    fn shared_prediction_metadata_corpus_matches_rust_wrapper_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Deserialize)]
        struct Corpus {
            schema_version: u8,
            cases: Vec<PredictionCase>,
        }
        #[derive(Deserialize)]
        struct PredictionCase {
            name: String,
            schema_valid: bool,
            semantic_valid: bool,
            disclosure: serde_json::Value,
        }

        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/prediction-metadata-parity-v1.json"
        ))?;
        assert_eq!(corpus.schema_version, 1);
        let base = serde_json::to_value(build_fixture_document()?)?;
        for case in corpus.cases {
            assert!(
                !case.semantic_valid || case.schema_valid,
                "semantically valid cases must also satisfy the JSON Schema: {}",
                case.name
            );
            let mut candidate = base.clone();
            candidate["expected_replay"]["prediction_claims"] =
                serde_json::Value::Array(vec![case.disclosure]);
            let bytes = serde_json::to_vec(&candidate)?;
            assert_eq!(
                parse_fixture_document_json(&bytes).is_ok(),
                case.semantic_valid,
                "{}",
                case.name
            );
        }
        Ok(())
    }

    #[test]
    fn shared_raw_and_integer_fixture_corpora_match_rust_typed_deserialization()
    -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Debug, Deserialize)]
        struct Corpus {
            schema_version: u8,
            cases: Vec<RawFixtureCase>,
        }
        #[derive(Debug, Deserialize)]
        struct RawFixtureCase {
            name: String,
            fixture: u16,
            valid: bool,
            replacements: Vec<RawReplacement>,
        }
        #[derive(Debug, Deserialize)]
        struct RawReplacement {
            needle: String,
            replacement: String,
        }

        let corpora: [Corpus; 2] = [
            serde_json::from_str(include_str!(
                "../../../schemas/fixtures/signed-batch-raw-parity-v1.json"
            ))?,
            serde_json::from_str(include_str!(
                "../../../schemas/fixtures/signed-batch-integer-lexeme-parity-v1.json"
            ))?,
        ];
        let fixture_v1 = include_str!("../../../schemas/fixtures/signed-batch-v1.json");
        let fixture_v2 = include_str!("../../../schemas/fixtures/signed-batch-v2.json");
        for corpus in corpora {
            assert_eq!(corpus.schema_version, 1);
            for case in corpus.cases {
                let mut raw = match case.fixture {
                    FIXTURE_VERSION_V1 => fixture_v1.to_owned(),
                    FIXTURE_VERSION_V2 => fixture_v2.to_owned(),
                    other => return Err(format!("unsupported corpus fixture {other}").into()),
                };
                for replacement in case.replacements {
                    let next = raw.replacen(&replacement.needle, &replacement.replacement, 1);
                    assert_ne!(next, raw, "{}: replacement must mutate fixture", case.name);
                    raw = next;
                }
                let rust_valid = parse_fixture_document_json(raw.as_bytes())
                    .and_then(|document| verify_fixture_document(&document).map(|_| document))
                    .is_ok();
                assert_eq!(rust_valid, case.valid, "{}", case.name);
            }
        }
        Ok(())
    }

    #[test]
    fn strict_utf8_byte_corpus_rejects_before_json_deserialization()
    -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Debug, Deserialize)]
        struct Corpus {
            schema_version: u8,
            cases: Vec<ByteFixtureCase>,
        }
        #[derive(Debug, Deserialize)]
        struct ByteFixtureCase {
            name: String,
            fixture: u16,
            valid: bool,
            replacements: Vec<ByteReplacement>,
        }
        #[derive(Debug, Deserialize)]
        struct ByteReplacement {
            needle_utf8: String,
            replacement_hex: String,
        }

        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/signed-batch-byte-parity-v1.json"
        ))?;
        assert_eq!(corpus.schema_version, 1);
        let fixture_v1 = include_bytes!("../../../schemas/fixtures/signed-batch-v1.json");
        let fixture_v2 = include_bytes!("../../../schemas/fixtures/signed-batch-v2.json");
        for case in corpus.cases {
            let mut bytes = match case.fixture {
                FIXTURE_VERSION_V1 => fixture_v1.to_vec(),
                FIXTURE_VERSION_V2 => fixture_v2.to_vec(),
                other => return Err(format!("unsupported corpus fixture {other}").into()),
            };
            for replacement in case.replacements {
                let needle = replacement.needle_utf8.as_bytes();
                let Some(position) = bytes
                    .windows(needle.len())
                    .position(|window| window == needle)
                else {
                    return Err(format!("{}: byte needle must exist", case.name).into());
                };
                if bytes[position + needle.len()..]
                    .windows(needle.len())
                    .any(|window| window == needle)
                {
                    return Err(format!("{}: byte needle must be unique", case.name).into());
                }
                let replacement_bytes = hex::decode(replacement.replacement_hex)?;
                bytes.splice(position..position + needle.len(), replacement_bytes);
            }
            let result = parse_fixture_document_json(&bytes);
            assert_eq!(result.is_ok(), case.valid, "{}", case.name);
            if !case.valid {
                assert!(matches!(result, Err(CoreError::Utf8(_))), "{}", case.name);
            }
        }
        Ok(())
    }

    #[test]
    fn fixture_replay_rejects_wrapper_supplied_trust_anchor()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut document = build_fixture_document()?;
        document.public_key_hex = "00".repeat(32);
        assert!(matches!(
            replay_fixture_document(&document, FINAL_VALID_AT, u64::MAX),
            Err(CoreError::FixtureTrustAnchorMismatch)
        ));
        Ok(())
    }

    #[test]
    fn equal_rank_disagreement_is_reported_as_conflict_without_a_false_winner()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let authorization = fixture_device_authorization()?;
        let signed = hex::decode(&document.signed_batch_cbor_hex)?;
        let mut core = Core::new();
        core.accept_signed_batch(&signed, &authorization)?;
        let deadline = core.ledger().resolve(&ResolutionQuery {
            subject_entity_id: parse_id(DEADLINE_SUBJECT_ID)?,
            scope_id: parse_id(SCOPE_ID)?,
            predicate_id: PredicateId::parse("academic.deadline")?,
            valid_at: TimestampMillis::new(400),
            known_at_accept_seq: 11,
            policy: AuthorityPolicy::OfficialFact,
        });
        assert!(deadline.active_claim_ids.is_empty());
        assert_eq!(
            deadline.conflicting_claim_ids,
            vec![parse_id(CLAIM_DEADLINE_OLD)?, parse_id(CLAIM_DEADLINE_NEW)?,]
        );
        Ok(())
    }

    #[test]
    fn committed_fixture_relation_event_round_trips_protobuf_losslessly()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = build_fixture_document()?;
        let authorization = fixture_device_authorization()?;
        let signed = hex::decode(&document.signed_batch_cbor_hex)?;
        let verified = verify_signed_batch(&signed, &authorization)?;
        let relation_event = verified
            .batch()
            .events
            .iter()
            .find(|event| matches!(&event.payload, EventPayload::ClaimRelated(_)))
            .ok_or(CoreError::MissingProjection("claim relation event"))?;
        let bytes = encode_claim_relation_event_proto(relation_event)?;
        assert_eq!(decode_claim_relation_event_proto(&bytes)?, *relation_event);
        Ok(())
    }
}
