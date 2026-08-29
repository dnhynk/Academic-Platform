//! Canonical Phase 0 domain vocabulary.
//!
//! This crate intentionally contains no storage or network code. It makes the
//! evidence, authority, epistemic status, and time semantics executable before
//! a transactional store is selected.

pub mod predicates;

use std::{fmt, str::FromStr};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Largest integer that can cross the JSON contract without precision loss.
pub const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Failures raised while constructing canonical domain values.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// An identifier was not a UUIDv7 value.
    #[error("{kind} must be a UUIDv7 value: {value}")]
    InvalidId { kind: &'static str, value: String },
    /// A digest was not an algorithm-prefixed lowercase SHA-256 value.
    #[error("invalid SHA-256 digest: {0}")]
    InvalidDigest(String),
    /// A physical locator was malformed.
    #[error("invalid vault locator: {0}")]
    InvalidLocator(String),
    /// A confidence value exceeded the inclusive 0..=1000 range.
    #[error("confidence permille must be in 0..=1000, got {0}")]
    InvalidConfidence(u16),
    /// A half-open interval did not have a strictly increasing end.
    #[error("valid interval must satisfy from < to")]
    InvalidInterval,
    /// A byte or time range was empty or reversed.
    #[error("range must satisfy start < end")]
    InvalidRange,
    /// A decimal scale was outside the portable baseline.
    #[error("decimal scale must be in 0..=18, got {0}")]
    InvalidDecimalScale(u8),
    /// A decimal coefficient was not the canonical base-10 i128 spelling.
    #[error("invalid canonical decimal coefficient: {0}")]
    InvalidDecimalCoefficient(String),
    /// A media type did not meet the portable contract.
    #[error("invalid media type: {0}")]
    InvalidMediaType(String),
    /// A repository-relative logical path was unsafe or non-normalized.
    #[error("invalid normalized logical path: {0}")]
    InvalidLogicalPath(String),
    /// A stable predicate name was malformed.
    #[error("invalid predicate id: {0}")]
    InvalidPredicate(String),
    /// A required human-readable identifier was empty.
    #[error("{0} must not be empty")]
    EmptyValue(&'static str),
    /// A versioned contract used version zero.
    #[error("{0} version must be greater than zero")]
    InvalidVersion(&'static str),
    /// A claim combined incompatible status and authority values.
    #[error("claim status {status:?} is incompatible with authority {authority:?}")]
    StatusAuthorityMismatch {
        status: EpistemicStatus,
        authority: AuthorityClass,
    },
    /// The signed event actor is not permitted to assert the claim authority.
    #[error("actor {actor} cannot assert authority {authority:?}")]
    ActorAuthorityMismatch {
        actor: &'static str,
        authority: AuthorityClass,
    },
    /// An official or unknown claim carried an invalid confidence value.
    #[error("status {0:?} must not carry model confidence")]
    ConfidenceNotAllowed(EpistemicStatus),
    /// An active prediction omitted its required uncertainty value.
    #[error("active Prediction claims require confidence permille")]
    MissingPredictionConfidence,
    /// An active prediction omitted its evidence/sample disclosure.
    #[error("active Prediction claims require prediction metadata")]
    MissingPredictionMetadata,
    /// A non-prediction state carried prediction-only metadata.
    #[error("status {0:?} must not carry prediction metadata")]
    PredictionMetadataNotAllowed(EpistemicStatus),
    /// Prediction metadata used an unsupported semantic version.
    #[error("unsupported prediction metadata version {0}")]
    UnsupportedPredictionMetadataVersion(u16),
    /// A prediction observation window was open, empty, or reversed.
    #[error("prediction observation window must be bounded and satisfy from < to")]
    InvalidPredictionObservationWindow,
    /// Prediction metadata disclosed no positive samples.
    #[error("prediction positive sample count must be greater than zero")]
    InvalidPredictionSampleCount,
    /// A claim that requires evidence had no evidence links.
    #[error("status {0:?} requires at least one evidence link")]
    MissingEvidence(EpistemicStatus),
    /// A decision event was not authored by the user.
    #[error("user decisions must be authored by Actor::User")]
    DecisionActorNotUser,
    /// An event payload contained an invalid nested value.
    #[error("invalid event payload: {0}")]
    InvalidEventPayload(String),
    /// A user decision had an invalid replacement or reversal interval.
    #[error("invalid user decision: {0}")]
    InvalidDecision(String),
    /// A representation requiring byte resolution was not issued by a trusted verifier actor.
    #[error("partial or derived evidence representations require a deterministic verifier actor")]
    UntrustedEvidenceRepresentation,
    /// The event schema is unsupported and therefore fails closed.
    #[error("unsupported event schema version {0}")]
    UnsupportedSchemaVersion(u16),
    /// Empty batches are not admitted.
    #[error("event batch must not be empty")]
    EmptyBatch,
    /// The declared origin range did not match the event list.
    #[error("batch origin range does not match its event count")]
    InvalidOriginRange,
    /// Event origin sequence numbers were not contiguous.
    #[error("origin sequence must be contiguous: expected {expected}, got {actual}")]
    NonContiguousOrigin { expected: u64, actual: u64 },
}

macro_rules! uuid_id {
    ($name:ident, $kind:literal) => {
        #[doc = concat!("Opaque UUIDv7-backed ", $kind, " identifier.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Constructs the identifier only for RFC-variant UUID version seven values.
            pub fn try_from_uuid(value: Uuid) -> Result<Self, DomainError> {
                if value.get_variant() == uuid::Variant::RFC4122 && value.get_version_num() == 7 {
                    Ok(Self(value))
                } else {
                    Err(DomainError::InvalidId {
                        kind: $kind,
                        value: value.to_string(),
                    })
                }
            }

            /// Returns the underlying opaque UUID bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 16] {
                self.0.as_bytes()
            }

            /// Returns the underlying UUID without assigning ordering semantics to it.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = DomainError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let uuid = Uuid::parse_str(value).map_err(|_| DomainError::InvalidId {
                    kind: $kind,
                    value: value.to_owned(),
                })?;
                Self::try_from_uuid(uuid)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let encoded = String::deserialize(deserializer)?;
                let value = Uuid::parse_str(&encoded).map_err(de::Error::custom)?;
                if value.to_string() != encoded {
                    return Err(de::Error::custom(DomainError::InvalidId {
                        kind: $kind,
                        value: encoded,
                    }));
                }
                Self::try_from_uuid(value).map_err(de::Error::custom)
            }
        }
    };
}

uuid_id!(EntityId, "entity");
uuid_id!(EventId, "event");
uuid_id!(ClaimId, "claim");
uuid_id!(EvidenceId, "evidence");
uuid_id!(ArtifactId, "artifact");
uuid_id!(BatchId, "batch");
uuid_id!(DeviceId, "device");
uuid_id!(DomainId, "domain");
uuid_id!(DecisionId, "decision");
uuid_id!(PermissionLineageId, "permission lineage");
uuid_id!(ScopeId, "scope");

// Event schema v3 aggregate identifiers. Each names an aggregate registered by
// exactly one v3 arm, except `RepositoryId`, which is only ever a parent
// reference: the eighteen arms fixed by the Phase 2 plan register no repository.
uuid_id!(CurriculumVersionId, "curriculum version");
uuid_id!(CourseRevisionId, "course revision");
uuid_id!(OfferingId, "offering");
uuid_id!(AttemptId, "attempt");
uuid_id!(RequirementSetId, "requirement set");
uuid_id!(AuditId, "audit");
uuid_id!(CapturePermissionId, "capture permission");
uuid_id!(LectureSessionId, "lecture session");
uuid_id!(TranscriptVersionId, "transcript version");
uuid_id!(LectureDocumentId, "lecture document");
uuid_id!(RepositoryId, "repository");
uuid_id!(SnapshotId, "snapshot");
uuid_id!(FindingId, "finding");
uuid_id!(ModelRunId, "model run");
uuid_id!(ProposalId, "proposal");
uuid_id!(EgressDecisionId, "egress decision");
uuid_id!(ConsentId, "consent");
uuid_id!(EntityIdentityChangeId, "entity identity change");
uuid_id!(RetentionActionId, "retention action");

/// A UTC instant represented as Unix epoch milliseconds.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct TimestampMillis(i64);

impl TimestampMillis {
    /// Constructs an instant from Unix epoch milliseconds.
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// Returns the Unix epoch millisecond value.
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }
}

/// A half-open domain-valid interval `[from, to)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ValidInterval {
    from: TimestampMillis,
    to: Option<TimestampMillis>,
}

impl<'de> Deserialize<'de> for ValidInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireInterval {
            from: TimestampMillis,
            to: Option<TimestampMillis>,
        }

        let value = WireInterval::deserialize(deserializer)?;
        Self::new(value.from, value.to).map_err(de::Error::custom)
    }
}

impl ValidInterval {
    /// Constructs a validated half-open interval.
    pub fn new(from: TimestampMillis, to: Option<TimestampMillis>) -> Result<Self, DomainError> {
        if to.is_some_and(|end| from >= end) {
            return Err(DomainError::InvalidInterval);
        }
        Ok(Self { from, to })
    }

    /// Constructs an open-ended interval.
    #[must_use]
    pub const fn open_ended(from: TimestampMillis) -> Self {
        Self { from, to: None }
    }

    /// Returns the inclusive lower bound.
    #[must_use]
    pub const fn from(self) -> TimestampMillis {
        self.from
    }

    /// Returns the exclusive upper bound, when present.
    #[must_use]
    pub const fn to(self) -> Option<TimestampMillis> {
        self.to
    }

    /// Tests domain validity independently of ledger acceptance order.
    #[must_use]
    pub fn contains(self, instant: TimestampMillis) -> bool {
        self.from <= instant && self.to.is_none_or(|end| instant < end)
    }
}

/// Current semantic version of the prediction evidence/sample disclosure.
pub const PREDICTION_METADATA_VERSION_V1: u16 = 1;

/// A bounded half-open observation/history window `[from, to)` used as prediction evidence.
///
/// This coordinate is deliberately distinct from [`ValidInterval`], which says when a claim
/// applies in the domain rather than which historical observations produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PredictionObservationWindow {
    from: TimestampMillis,
    to: TimestampMillis,
}

impl<'de> Deserialize<'de> for PredictionObservationWindow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireWindow {
            from: TimestampMillis,
            to: TimestampMillis,
        }

        let value = WireWindow::deserialize(deserializer)?;
        Self::new(value.from, value.to).map_err(de::Error::custom)
    }
}

impl PredictionObservationWindow {
    /// Constructs a bounded, nonempty observation window.
    pub fn new(from: TimestampMillis, to: TimestampMillis) -> Result<Self, DomainError> {
        let window = Self { from, to };
        window.validate()?;
        Ok(window)
    }

    /// Revalidates the bounded, nonempty observation interval.
    pub fn validate(self) -> Result<(), DomainError> {
        if self.from >= self.to {
            return Err(DomainError::InvalidPredictionObservationWindow);
        }
        Ok(())
    }

    /// Returns the inclusive observation lower bound.
    #[must_use]
    pub const fn from(self) -> TimestampMillis {
        self.from
    }

    /// Returns the exclusive observation upper bound.
    #[must_use]
    pub const fn to(self) -> TimestampMillis {
        self.to
    }
}

/// Versioned disclosure of the bounded evidence interval and positive sample count for a prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PredictionMetadata {
    version: u16,
    observation_window: PredictionObservationWindow,
    positive_sample_count: u32,
}

impl<'de> Deserialize<'de> for PredictionMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireMetadata {
            version: u16,
            observation_window: PredictionObservationWindow,
            positive_sample_count: u32,
        }

        let value = WireMetadata::deserialize(deserializer)?;
        Self::from_version(
            value.version,
            value.observation_window,
            value.positive_sample_count,
        )
        .map_err(de::Error::custom)
    }
}

impl PredictionMetadata {
    /// Constructs current-version prediction metadata.
    pub fn new(
        observation_window: PredictionObservationWindow,
        positive_sample_count: u32,
    ) -> Result<Self, DomainError> {
        Self::from_version(
            PREDICTION_METADATA_VERSION_V1,
            observation_window,
            positive_sample_count,
        )
    }

    fn from_version(
        version: u16,
        observation_window: PredictionObservationWindow,
        positive_sample_count: u32,
    ) -> Result<Self, DomainError> {
        if version != PREDICTION_METADATA_VERSION_V1 {
            return Err(DomainError::UnsupportedPredictionMetadataVersion(version));
        }
        if positive_sample_count == 0 {
            return Err(DomainError::InvalidPredictionSampleCount);
        }
        Ok(Self {
            version,
            observation_window,
            positive_sample_count,
        })
    }

    /// Revalidates the complete metadata contract at an acceptance boundary.
    pub fn validate(self) -> Result<(), DomainError> {
        self.observation_window.validate()?;
        Self::from_version(
            self.version,
            self.observation_window,
            self.positive_sample_count,
        )
        .map(|_| ())
    }

    /// Returns the semantic metadata version.
    #[must_use]
    pub const fn version(self) -> u16 {
        self.version
    }

    /// Returns the evidence/history window that produced the prediction.
    #[must_use]
    pub const fn observation_window(self) -> PredictionObservationWindow {
        self.observation_window
    }

    /// Returns the disclosed positive sample count.
    #[must_use]
    pub const fn positive_sample_count(self) -> u32 {
        self.positive_sample_count
    }
}

/// SHA-256 identity for exact plaintext or canonical semantic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    /// Hashes exact bytes with SHA-256.
    #[must_use]
    pub fn sha256(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Constructs a digest from raw SHA-256 bytes.
    #[must_use]
    pub const fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "sha256:{}", hex::encode(self.0))
    }
}

impl FromStr for ContentDigest {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(hex_value) = value.strip_prefix("sha256:") else {
            return Err(DomainError::InvalidDigest(value.to_owned()));
        };
        if hex_value.len() != 64
            || !hex_value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidDigest(value.to_owned()));
        }
        let bytes = hex::decode(hex_value)
            .map_err(|_| DomainError::InvalidDigest(value.to_owned()))?
            .try_into()
            .map_err(|_| DomainError::InvalidDigest(value.to_owned()))?;
        Ok(Self(bytes))
    }
}

impl Serialize for ContentDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ContentDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}

/// A keyed, domain-scoped physical locator that does not expose plaintext equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VaultLocator([u8; 32]);

impl VaultLocator {
    /// Derives a physical locator from a domain-only secret key and logical descriptor.
    pub fn derive(
        domain_locator_key: &[u8],
        format_version: u16,
        media_type: &MediaType,
        digest: ContentDigest,
    ) -> Result<Self, DomainError> {
        if format_version == 0 {
            return Err(DomainError::InvalidVersion("artifact format"));
        }
        if domain_locator_key.is_empty() {
            return Err(DomainError::EmptyValue("domain locator key"));
        }
        let mut mac = HmacSha256::new_from_slice(domain_locator_key)
            .map_err(|_| DomainError::EmptyValue("domain locator key"))?;
        mac.update(&format_version.to_be_bytes());
        mac.update(media_type.as_str().as_bytes());
        mac.update(&[0]);
        mac.update(digest.as_bytes());
        Ok(Self(mac.finalize().into_bytes().into()))
    }

    /// Returns the locator bytes used for fan-out storage.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for VaultLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "locator:v1:{}", hex::encode(self.0))
    }
}

impl FromStr for VaultLocator {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(hex_value) = value.strip_prefix("locator:v1:") else {
            return Err(DomainError::InvalidLocator(value.to_owned()));
        };
        if hex_value.len() != 64
            || !hex_value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DomainError::InvalidLocator(value.to_owned()));
        }
        let bytes = hex::decode(hex_value)
            .map_err(|_| DomainError::InvalidLocator(value.to_owned()))?
            .try_into()
            .map_err(|_| DomainError::InvalidLocator(value.to_owned()))?;
        Ok(Self(bytes))
    }
}

impl Serialize for VaultLocator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for VaultLocator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(de::Error::custom)
    }
}

/// A normalized internet media type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MediaType(String);

impl MediaType {
    /// Parses a conservative `type/subtype` media type without parameters.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let mut parts = value.split('/');
        let Some(top) = parts.next() else {
            return Err(DomainError::InvalidMediaType(value));
        };
        let Some(subtype) = parts.next() else {
            return Err(DomainError::InvalidMediaType(value));
        };
        if top.is_empty()
            || subtype.is_empty()
            || parts.next().is_some()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'/' | b'+' | b'-' | b'.')
            })
        {
            return Err(DomainError::InvalidMediaType(value));
        }
        Ok(Self(value))
    }

    /// Returns the normalized media type.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for MediaType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// A normalized repository-relative logical path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LogicalPath(String);

impl LogicalPath {
    /// Parses a slash-separated relative path with no dot segments.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty()
            || value.starts_with('/')
            || value.contains('\\')
            || value.contains(':')
            || value.starts_with('~')
            || value.contains('\0')
            || value.chars().any(char::is_control)
            || value
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(DomainError::InvalidLogicalPath(value));
        }
        Ok(Self(value))
    }

    /// Returns the normalized portable path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LogicalPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// A stable predicate registry key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PredicateId(String);

impl PredicateId {
    /// Parses a lowercase, namespaced predicate identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.')
            || !value.contains('.')
            || value.starts_with('.')
            || value.ends_with('.')
            || value.contains("..")
        {
            return Err(DomainError::InvalidPredicate(value));
        }
        Ok(Self(value))
    }

    /// Returns the stable predicate name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PredicateId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// A bounded confidence value, distinct from authority or mastery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ConfidencePermille(u16);

impl ConfidencePermille {
    /// Constructs a confidence value from 0 through 1000 inclusive.
    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value > 1000 {
            return Err(DomainError::InvalidConfidence(value));
        }
        Ok(Self(value))
    }

    /// Returns the integer permille representation.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ConfidencePermille {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u16::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// An exact base-10 decimal, avoiding binary floating-point semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal {
    coefficient: i128,
    scale: u8,
}

impl Serialize for Decimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Decimal", 2)?;
        state.serialize_field("coefficient", &self.coefficient.to_string())?;
        state.serialize_field("scale", &self.scale)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireDecimal {
            coefficient: String,
            scale: u8,
        }

        let value = WireDecimal::deserialize(deserializer)?;
        let coefficient = value.coefficient.parse::<i128>().map_err(|_| {
            de::Error::custom(DomainError::InvalidDecimalCoefficient(
                value.coefficient.clone(),
            ))
        })?;
        if coefficient.to_string() != value.coefficient {
            return Err(de::Error::custom(DomainError::InvalidDecimalCoefficient(
                value.coefficient,
            )));
        }
        Self::new(coefficient, value.scale).map_err(de::Error::custom)
    }
}

impl Decimal {
    /// Constructs a decimal with a portable maximum scale of eighteen.
    pub fn new(coefficient: i128, scale: u8) -> Result<Self, DomainError> {
        if scale > 18 {
            return Err(DomainError::InvalidDecimalScale(scale));
        }
        Ok(Self { coefficient, scale })
    }

    /// Returns the signed integer coefficient.
    #[must_use]
    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// Returns the base-10 scale.
    #[must_use]
    pub const fn scale(self) -> u8 {
        self.scale
    }
}

/// Logical security and lifecycle domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DomainKind {
    University,
    Academic,
    Lecture,
    Knowledge,
    Competency,
    Evidence,
    Question,
    Project,
    Career,
    Personal,
}

/// Source confidentiality independent of epistemic authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidentiality {
    Public,
    Personal,
    Restricted,
    Secret,
}

/// Retention boundary used to prevent unsafe cross-policy deduplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetentionClass {
    Ephemeral,
    CourseTerm,
    UserManaged,
    LegalHold,
}

/// A scope that may be referenced by claims, relations, decisions, and queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeDescriptor {
    pub id: ScopeId,
    pub domain_id: DomainId,
    pub label: String,
}

impl ScopeDescriptor {
    /// Validates the human-readable scope provenance label.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.label.trim().is_empty() {
            return Err(DomainError::EmptyValue("scope label"));
        }
        Ok(())
    }
}

/// An immutable, explicitly locatable representation registered for evidence use.
///
/// The descriptor binds an exact locator to the digest and byte length of the
/// representation extracted at that locator. A ledger never guesses page,
/// timestamp, or repository bounds from a media type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRepresentation {
    pub locator: EvidenceLocator,
    pub content_digest: ContentDigest,
    pub byte_length: u64,
}

/// Logical content-addressed artifact descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDescriptor {
    pub id: ArtifactId,
    pub content_digest: ContentDigest,
    pub media_type: MediaType,
    pub byte_length: u64,
    pub domain_id: DomainId,
    pub confidentiality: Confidentiality,
    pub retention_class: RetentionClass,
    pub permission_lineage_id: PermissionLineageId,
    pub format_version: u16,
    pub vault_locator: VaultLocator,
    pub evidence_representations: Vec<ArtifactRepresentation>,
}

/// Enforces the raw JSON number-token profile for artifact descriptors.
///
/// Every number in this contract is an unsigned integer. Consumers must run
/// this check on the original JSON text before JSON parsing so decimal and
/// exponent spellings cannot be normalized into integers by JavaScript while
/// Rust observes a floating-point token. Range checks remain the responsibility
/// of typed deserialization, JSON Schema, and [`ArtifactDescriptor::validate`].
/// The same raw boundary must use Serde's typed struct deserializer, which
/// rejects duplicate fields and JSON strings that do not decode to Unicode
/// scalar values; the shared exact-raw corpus exercises the combined boundary.
pub fn validate_artifact_json_number_tokens(input: &str) -> Result<(), DomainError> {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'-' {
            return Err(DomainError::InvalidEventPayload(
                "artifact JSON numbers must use canonical unsigned integer tokens".to_owned(),
            ));
        }
        if byte.is_ascii_digit() {
            let first = byte;
            index += 1;
            if first == b'0' && index < bytes.len() && bytes[index].is_ascii_digit() {
                return Err(DomainError::InvalidEventPayload(
                    "artifact JSON numbers must use canonical unsigned integer tokens".to_owned(),
                ));
            }
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index < bytes.len() && matches!(bytes[index], b'.' | b'e' | b'E') {
                return Err(DomainError::InvalidEventPayload(
                    "artifact JSON numbers must use canonical unsigned integer tokens".to_owned(),
                ));
            }
            continue;
        }
        index += 1;
    }
    Ok(())
}

impl ArtifactDescriptor {
    /// Validates the descriptor fields that are fixed in Phase 0.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.format_version != 1 {
            return Err(DomainError::InvalidEventPayload(
                "unsupported artifact format version".to_owned(),
            ));
        }
        if self.byte_length > MAX_SAFE_JSON_INTEGER {
            return Err(DomainError::InvalidEventPayload(
                "artifact byte length exceeds the portable exact-integer range".to_owned(),
            ));
        }
        for (index, representation) in self.evidence_representations.iter().enumerate() {
            representation.locator.validate()?;
            if representation.byte_length > MAX_SAFE_JSON_INTEGER {
                return Err(DomainError::InvalidEventPayload(
                    "representation byte length exceeds the portable exact-integer range"
                        .to_owned(),
                ));
            }
            if self.evidence_representations[..index]
                .iter()
                .any(|prior| prior.locator == representation.locator)
            {
                return Err(DomainError::InvalidEventPayload(
                    "artifact evidence representation locators must be unique".to_owned(),
                ));
            }
            match &representation.locator {
                EvidenceLocator::TextBytes {
                    source_digest,
                    start,
                    end,
                } => {
                    if *source_digest != self.content_digest
                        || *end > self.byte_length
                        || representation.byte_length != *end - *start
                    {
                        return Err(DomainError::InvalidEventPayload(
                            "text representation must be bounded by the registered artifact bytes"
                                .to_owned(),
                        ));
                    }
                    if *start == 0
                        && *end == self.byte_length
                        && representation.content_digest != self.content_digest
                    {
                        return Err(DomainError::InvalidEventPayload(
                            "full-range text representation digest must equal the artifact content digest"
                                .to_owned(),
                        ));
                    }
                }
                EvidenceLocator::RepositoryBytes { start, end, .. } => {
                    if representation.byte_length != *end - *start {
                        return Err(DomainError::InvalidEventPayload(
                            "repository representation byte length must match its exact span"
                                .to_owned(),
                        ));
                    }
                }
                EvidenceLocator::Page { .. } | EvidenceLocator::TranscriptTime { .. } => {}
            }
        }
        Ok(())
    }

    /// Rejects derived representations until a byte-resolving verifier capability exists.
    ///
    /// An authenticated actor label is not proof that caller-supplied representation bytes
    /// were resolved from the artifact. Phase 0 therefore admits only the whole-artifact
    /// text representation whose digest is already bound by the artifact identity.
    pub fn validate_for_actor(&self, _actor: &Actor) -> Result<(), DomainError> {
        self.validate()?;
        let has_unverified_representation = self
            .evidence_representations
            .iter()
            .any(|representation| !self.is_artifact_digest_bound(representation));
        if has_unverified_representation {
            return Err(DomainError::UntrustedEvidenceRepresentation);
        }
        Ok(())
    }

    /// Finds exact immutable representation metadata for a locator.
    #[must_use]
    pub fn representation(&self, locator: &EvidenceLocator) -> Option<&ArtifactRepresentation> {
        self.evidence_representations
            .iter()
            .find(|representation| &representation.locator == locator)
    }

    /// Returns whether the representation is cryptographically identical to the whole artifact.
    #[must_use]
    pub fn is_artifact_digest_bound(&self, representation: &ArtifactRepresentation) -> bool {
        matches!(
            &representation.locator,
            EvidenceLocator::TextBytes {
                source_digest,
                start: 0,
                end,
            } if *source_digest == self.content_digest
                && *end == self.byte_length
                && representation.byte_length == self.byte_length
                && representation.content_digest == self.content_digest
        )
    }
}

/// Exact evidence location inside an immutable artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceLocator {
    Page {
        page_number: u32,
    },
    TextBytes {
        source_digest: ContentDigest,
        start: u64,
        end: u64,
    },
    TranscriptTime {
        start_ms: u64,
        end_ms: u64,
    },
    RepositoryBytes {
        snapshot_digest: ContentDigest,
        path: LogicalPath,
        start: u64,
        end: u64,
    },
}

impl EvidenceLocator {
    /// Validates non-empty spans while preserving the source identity.
    pub fn validate(&self) -> Result<(), DomainError> {
        let valid = match self {
            Self::Page { page_number } => *page_number > 0,
            Self::TextBytes { start, end, .. }
            | Self::TranscriptTime {
                start_ms: start,
                end_ms: end,
            }
            | Self::RepositoryBytes { start, end, .. } => {
                start < end && *start <= MAX_SAFE_JSON_INTEGER && *end <= MAX_SAFE_JSON_INTEGER
            }
        };
        if valid {
            Ok(())
        } else {
            Err(DomainError::InvalidRange)
        }
    }
}

/// How an evidence item relates to a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceRole {
    Supports,
    Contradicts,
    ContextOnly,
}

/// Strength of an evidence item without changing its authority class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceStrength {
    Direct,
    Corroborating,
    Weak,
}

/// Immutable evidence pointer into an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: EvidenceId,
    pub artifact_id: ArtifactId,
    pub locator: EvidenceLocator,
    pub excerpt_digest: ContentDigest,
    pub role: EvidenceRole,
    pub strength: EvidenceStrength,
    pub extraction_method: String,
    pub extractor_version: String,
}

impl EvidenceItem {
    /// Validates the exact locator and required provenance identifiers.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.locator.validate()?;
        if self.extraction_method.trim().is_empty() {
            return Err(DomainError::EmptyValue("extraction method"));
        }
        if self.extractor_version.trim().is_empty() {
            return Err(DomainError::EmptyValue("extractor version"));
        }
        Ok(())
    }
}

/// Epistemic status, kept separate from the source's authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpistemicStatus {
    OfficialConfirmed,
    UserConfirmed,
    CodeObserved,
    DeterministicDerived,
    AiInferred,
    Prediction,
    Disputed,
    Superseded,
    Unknown,
}

/// Authority class used by predicate-specific resolution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorityClass {
    Official,
    UserExplicit,
    DirectObservation,
    DeterministicEngine,
    Curated,
    ModelInference,
    Prediction,
    Unknown,
}

/// Observable mastery depth. Passage of time never mutates this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MasteryLevel {
    Unseen,
    Exposed,
    Understood,
    Practiced,
    Applied,
    Fluent,
}

/// Immediate retrieval readiness, projected independently of mastery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FreshnessBand {
    Unknown,
    Stale,
    Low,
    Moderate,
    High,
    VeryHigh,
}

/// Typed claim object. Binary floating point is intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimObject {
    Entity(EntityId),
    Text(String),
    Integer(i64),
    Boolean(bool),
    Decimal(Decimal),
    Instant(TimestampMillis),
    Interval(ValidInterval),
    Mastery(MasteryLevel),
    Freshness(FreshnessBand),
}

/// An atomic, append-only assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub subject_entity_id: EntityId,
    pub predicate_id: PredicateId,
    pub object: ClaimObject,
    pub scope_id: ScopeId,
    pub authority_class: AuthorityClass,
    pub epistemic_status: EpistemicStatus,
    pub confidence: Option<ConfidencePermille>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_metadata: Option<PredictionMetadata>,
    pub valid_time: ValidInterval,
    pub evidence_ids: Vec<EvidenceId>,
}

impl Claim {
    /// Validates authority/status separation and the minimum evidence rule.
    pub fn validate(&self) -> Result<(), DomainError> {
        let compatible = matches!(
            (self.epistemic_status, self.authority_class),
            (EpistemicStatus::OfficialConfirmed, AuthorityClass::Official)
                | (EpistemicStatus::UserConfirmed, AuthorityClass::UserExplicit)
                | (
                    EpistemicStatus::CodeObserved,
                    AuthorityClass::DirectObservation
                )
                | (
                    EpistemicStatus::DeterministicDerived,
                    AuthorityClass::DeterministicEngine
                )
                | (
                    EpistemicStatus::DeterministicDerived,
                    AuthorityClass::Curated
                )
                | (EpistemicStatus::AiInferred, AuthorityClass::ModelInference)
                | (EpistemicStatus::Prediction, AuthorityClass::Prediction)
                | (EpistemicStatus::Disputed, _)
                | (EpistemicStatus::Superseded, _)
                | (EpistemicStatus::Unknown, AuthorityClass::Unknown)
        );
        if !compatible {
            return Err(DomainError::StatusAuthorityMismatch {
                status: self.epistemic_status,
                authority: self.authority_class,
            });
        }
        if matches!(
            self.epistemic_status,
            EpistemicStatus::OfficialConfirmed | EpistemicStatus::Unknown
        ) && self.confidence.is_some()
        {
            return Err(DomainError::ConfidenceNotAllowed(self.epistemic_status));
        }
        if self.epistemic_status == EpistemicStatus::Prediction && self.confidence.is_none() {
            return Err(DomainError::MissingPredictionConfidence);
        }
        match (self.epistemic_status, self.prediction_metadata) {
            (EpistemicStatus::Prediction, Some(metadata)) => metadata.validate()?,
            (EpistemicStatus::Prediction, None) => {
                return Err(DomainError::MissingPredictionMetadata);
            }
            (status, Some(_)) => return Err(DomainError::PredictionMetadataNotAllowed(status)),
            (_, None) => {}
        }
        if !matches!(self.epistemic_status, EpistemicStatus::Unknown)
            && self.evidence_ids.is_empty()
        {
            return Err(DomainError::MissingEvidence(self.epistemic_status));
        }
        Ok(())
    }

    /// Enforces the fail-closed actor/authority/status matrix for signed events.
    pub fn validate_for_actor(&self, actor: &Actor) -> Result<(), DomainError> {
        self.validate()?;
        let permitted = match actor {
            Actor::User { .. } => self.authority_class == AuthorityClass::UserExplicit,
            Actor::DeterministicEngine { .. } => {
                self.authority_class == AuthorityClass::DeterministicEngine
            }
            Actor::ModelRun { .. } => matches!(
                self.authority_class,
                AuthorityClass::ModelInference | AuthorityClass::Prediction
            ),
            Actor::Importer { .. } => matches!(
                self.authority_class,
                AuthorityClass::Official
                    | AuthorityClass::DirectObservation
                    | AuthorityClass::Curated
                    | AuthorityClass::Unknown
            ),
        };
        if permitted {
            Ok(())
        } else {
            Err(DomainError::ActorAuthorityMismatch {
                actor: actor.kind_name(),
                authority: self.authority_class,
            })
        }
    }
}

/// Provenance-bearing actor, never inferred from wall-clock or process identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Actor {
    User { user_id: EntityId },
    DeterministicEngine { name: String, version: String },
    ModelRun { run_id: EntityId },
    Importer { name: String, version: String },
}

impl Actor {
    /// Validates actor labels used in signed bytes.
    pub fn validate(&self) -> Result<(), DomainError> {
        match self {
            Self::User { .. } | Self::ModelRun { .. } => Ok(()),
            Self::DeterministicEngine { name, version } | Self::Importer { name, version } => {
                if name.trim().is_empty() {
                    return Err(DomainError::EmptyValue("actor name"));
                }
                if version.trim().is_empty() {
                    return Err(DomainError::EmptyValue("actor version"));
                }
                Ok(())
            }
        }
    }

    /// Returns the stable actor variant name used in validation diagnostics.
    #[must_use]
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::User { .. } => "USER",
            Self::DeterministicEngine { .. } => "DETERMINISTIC_ENGINE",
            Self::ModelRun { .. } => "MODEL_RUN",
            Self::Importer { .. } => "IMPORTER",
        }
    }
}

/// Append-only relationship between two claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRelation {
    pub source_claim_id: ClaimId,
    pub target_claim_id: ClaimId,
    pub kind: ClaimRelationKind,
    pub scope_id: ScopeId,
}

/// Meaning of an immutable claim relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimRelationKind {
    Supports,
    Contradicts,
    Supersedes,
    Retracts,
    Duplicates,
}

/// User-owned decision action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionAction {
    Confirm,
    Reject,
    Replace { replacement_claim_id: ClaimId },
}

/// Semantic identity against which user decisions remain durable across claim IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionSlot {
    pub subject_entity_id: EntityId,
    pub predicate_id: PredicateId,
    pub scope_id: ScopeId,
}

/// Immutable explicit user decision that automated inference cannot erase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDecision {
    pub id: DecisionId,
    pub target_claim_id: ClaimId,
    pub target_object: ClaimObject,
    pub resolution_slot: ResolutionSlot,
    pub action: DecisionAction,
    pub valid_time: ValidInterval,
    pub rationale_evidence_ids: Vec<EvidenceId>,
    pub decided_at: TimestampMillis,
    pub reversible_until: Option<TimestampMillis>,
}

impl UserDecision {
    /// Validates replacement identity and the optional half-open reversal window.
    pub fn validate(&self) -> Result<(), DomainError> {
        if let DecisionAction::Replace {
            replacement_claim_id,
        } = &self.action
            && *replacement_claim_id == self.target_claim_id
        {
            return Err(DomainError::InvalidDecision(
                "replacement claim must differ from target".to_owned(),
            ));
        }
        if self
            .reversible_until
            .is_some_and(|until| until <= self.decided_at)
        {
            return Err(DomainError::InvalidDecision(
                "reversible_until must be later than decided_at".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Declares one event schema v3 aggregate registration record.
///
/// Every v3 arm registers an aggregate at the same depth: its own identity, the
/// domain and scope it belongs to, the parent aggregate it hangs from where one
/// exists, an optional digest of the provenance artifact it was ingested from,
/// and the interval over which the registration is effective or observed. The
/// aggregate's own attributes are not part of the signed arm: disputable facts
/// arrive as `CLAIM_ASSERTED`, and everything else becomes typed closure-table
/// columns fixed by the task that owns that aggregate.
macro_rules! aggregate_registration {
    ($name:ident, $id:ty, $kind:literal $(, $parent_field:ident: $parent:ty)?) => {
        #[doc = concat!("Canonical registration of one ", $kind, " aggregate.")]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub id: $id,
            $(pub $parent_field: $parent,)?
            pub domain_id: DomainId,
            pub scope_id: ScopeId,
            pub source_digest: Option<ContentDigest>,
            pub valid_time: ValidInterval,
        }

        impl $name {
            #[doc = concat!("Validates the ", $kind, " registration against its own event's domain.")]
            pub fn validate(&self, event_domain_id: DomainId) -> Result<(), DomainError> {
                if self.domain_id != event_domain_id {
                    return Err(DomainError::InvalidEventPayload(concat!(
                        $kind,
                        " domain must match event domain"
                    ).to_owned()));
                }
                Ok(())
            }
        }
    };
}

aggregate_registration!(
    CurriculumVersionRegistration,
    CurriculumVersionId,
    "curriculum version"
);
aggregate_registration!(
    CourseRevisionRegistration,
    CourseRevisionId,
    "course revision",
    curriculum_version_id: CurriculumVersionId
);
aggregate_registration!(
    OfferingRegistration,
    OfferingId,
    "offering",
    course_revision_id: CourseRevisionId
);
aggregate_registration!(AttemptRegistration, AttemptId, "attempt", offering_id: OfferingId);
aggregate_registration!(
    RequirementSetRegistration,
    RequirementSetId,
    "requirement set",
    curriculum_version_id: CurriculumVersionId
);
aggregate_registration!(
    AuditRegistration,
    AuditId,
    "audit",
    requirement_set_id: RequirementSetId
);
aggregate_registration!(
    CapturePermissionRegistration,
    CapturePermissionId,
    "capture permission",
    offering_id: OfferingId
);
aggregate_registration!(
    LectureSessionRegistration,
    LectureSessionId,
    "lecture session",
    offering_id: OfferingId
);
aggregate_registration!(
    TranscriptVersionRegistration,
    TranscriptVersionId,
    "transcript version",
    lecture_session_id: LectureSessionId
);
aggregate_registration!(
    LectureDocumentRegistration,
    LectureDocumentId,
    "lecture document",
    lecture_session_id: LectureSessionId
);
aggregate_registration!(
    SnapshotRegistration,
    SnapshotId,
    "snapshot",
    repository_id: RepositoryId
);
aggregate_registration!(FindingRegistration, FindingId, "finding", snapshot_id: SnapshotId);
aggregate_registration!(ModelRunRegistration, ModelRunId, "model run");
aggregate_registration!(
    ProposalDispositionRegistration,
    ProposalId,
    "proposal disposition",
    model_run_id: ModelRunId
);
aggregate_registration!(
    EgressDecisionRegistration,
    EgressDecisionId,
    "egress decision"
);
aggregate_registration!(ConsentRegistration, ConsentId, "consent");
aggregate_registration!(
    EntityIdentityChangeRegistration,
    EntityIdentityChangeId,
    "entity identity change",
    entity_id: EntityId
);
aggregate_registration!(
    RetentionActionRegistration,
    RetentionActionId,
    "retention action"
);

/// Event kind discriminants introduced by event schema v3, in Proto tag order 16..=33.
///
/// A payload authenticated as v1 or v2 may not carry any of these; the legacy
/// source projections reject them so a v3 arm can never be smuggled into bytes
/// that claim an older schema version.
pub const V3_EVENT_KINDS: [&str; 18] = [
    "CURRICULUM_VERSION_PUBLISHED",
    "COURSE_REVISION_PUBLISHED",
    "OFFERING_OBSERVED",
    "ATTEMPT_RECORDED",
    "REQUIREMENT_SET_PUBLISHED",
    "AUDIT_COMPUTED",
    "CAPTURE_PERMISSION_RECORDED",
    "LECTURE_SESSION_RECORDED",
    "TRANSCRIPT_VERSION_ADDED",
    "LECTURE_DOCUMENT_PUBLISHED",
    "SNAPSHOT_REGISTERED",
    "FINDING_PUBLISHED",
    "MODEL_RUN_RECORDED",
    "PROPOSAL_DISPOSED",
    "EGRESS_DECIDED",
    "CONSENT_RECORDED",
    "ENTITY_IDENTITY_CHANGED",
    "RETENTION_ACTION_RECORDED",
];

/// Canonical event payloads admitted by the Phase 0 pure ledger.
///
/// Tags 10..=15 are the v1/v2 arms and never change. The v3 arms below are
/// additive and occupy Proto tags 16..=33 in declaration order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventPayload {
    ScopeRegistered(ScopeDescriptor),
    ArtifactRegistered(ArtifactDescriptor),
    EvidenceRegistered(EvidenceItem),
    ClaimAsserted(Claim),
    ClaimRelated(ClaimRelation),
    DecisionRecorded(UserDecision),
    CurriculumVersionPublished(CurriculumVersionRegistration),
    CourseRevisionPublished(CourseRevisionRegistration),
    OfferingObserved(OfferingRegistration),
    AttemptRecorded(AttemptRegistration),
    RequirementSetPublished(RequirementSetRegistration),
    AuditComputed(AuditRegistration),
    CapturePermissionRecorded(CapturePermissionRegistration),
    LectureSessionRecorded(LectureSessionRegistration),
    TranscriptVersionAdded(TranscriptVersionRegistration),
    LectureDocumentPublished(LectureDocumentRegistration),
    SnapshotRegistered(SnapshotRegistration),
    FindingPublished(FindingRegistration),
    ModelRunRecorded(ModelRunRegistration),
    ProposalDisposed(ProposalDispositionRegistration),
    EgressDecided(EgressDecisionRegistration),
    ConsentRecorded(ConsentRegistration),
    EntityIdentityChanged(EntityIdentityChangeRegistration),
    RetentionActionRecorded(RetentionActionRegistration),
}

/// The identity and closure fields every event schema v3 registration arm shares.
///
/// Consumers that only need to place an aggregate in its domain and scope read
/// this instead of matching all eighteen arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregateRegistrationRef {
    /// Wire discriminant of the arm that registered the aggregate.
    pub kind: &'static str,
    /// Opaque aggregate identifier, already constrained to an RFC-variant UUIDv7.
    pub id: Uuid,
    /// Domain the aggregate belongs to; equal to its event's domain.
    pub domain_id: DomainId,
    /// Scope the aggregate is registered under.
    pub scope_id: ScopeId,
}

impl EventPayload {
    /// Returns the shared registration view for a v3 arm, or `None` for a v1/v2 arm.
    #[must_use]
    pub fn registration(&self) -> Option<AggregateRegistrationRef> {
        macro_rules! view {
            ($record:expr) => {
                Some(AggregateRegistrationRef {
                    kind: self.kind(),
                    id: $record.id.as_uuid(),
                    domain_id: $record.domain_id,
                    scope_id: $record.scope_id,
                })
            };
        }
        match self {
            Self::ScopeRegistered(_)
            | Self::ArtifactRegistered(_)
            | Self::EvidenceRegistered(_)
            | Self::ClaimAsserted(_)
            | Self::ClaimRelated(_)
            | Self::DecisionRecorded(_) => None,
            Self::CurriculumVersionPublished(record) => view!(record),
            Self::CourseRevisionPublished(record) => view!(record),
            Self::OfferingObserved(record) => view!(record),
            Self::AttemptRecorded(record) => view!(record),
            Self::RequirementSetPublished(record) => view!(record),
            Self::AuditComputed(record) => view!(record),
            Self::CapturePermissionRecorded(record) => view!(record),
            Self::LectureSessionRecorded(record) => view!(record),
            Self::TranscriptVersionAdded(record) => view!(record),
            Self::LectureDocumentPublished(record) => view!(record),
            Self::SnapshotRegistered(record) => view!(record),
            Self::FindingPublished(record) => view!(record),
            Self::ModelRunRecorded(record) => view!(record),
            Self::ProposalDisposed(record) => view!(record),
            Self::EgressDecided(record) => view!(record),
            Self::ConsentRecorded(record) => view!(record),
            Self::EntityIdentityChanged(record) => view!(record),
            Self::RetentionActionRecorded(record) => view!(record),
        }
    }

    /// Returns the wire discriminant this payload serializes as.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ScopeRegistered(_) => "SCOPE_REGISTERED",
            Self::ArtifactRegistered(_) => "ARTIFACT_REGISTERED",
            Self::EvidenceRegistered(_) => "EVIDENCE_REGISTERED",
            Self::ClaimAsserted(_) => "CLAIM_ASSERTED",
            Self::ClaimRelated(_) => "CLAIM_RELATED",
            Self::DecisionRecorded(_) => "DECISION_RECORDED",
            Self::CurriculumVersionPublished(_) => "CURRICULUM_VERSION_PUBLISHED",
            Self::CourseRevisionPublished(_) => "COURSE_REVISION_PUBLISHED",
            Self::OfferingObserved(_) => "OFFERING_OBSERVED",
            Self::AttemptRecorded(_) => "ATTEMPT_RECORDED",
            Self::RequirementSetPublished(_) => "REQUIREMENT_SET_PUBLISHED",
            Self::AuditComputed(_) => "AUDIT_COMPUTED",
            Self::CapturePermissionRecorded(_) => "CAPTURE_PERMISSION_RECORDED",
            Self::LectureSessionRecorded(_) => "LECTURE_SESSION_RECORDED",
            Self::TranscriptVersionAdded(_) => "TRANSCRIPT_VERSION_ADDED",
            Self::LectureDocumentPublished(_) => "LECTURE_DOCUMENT_PUBLISHED",
            Self::SnapshotRegistered(_) => "SNAPSHOT_REGISTERED",
            Self::FindingPublished(_) => "FINDING_PUBLISHED",
            Self::ModelRunRecorded(_) => "MODEL_RUN_RECORDED",
            Self::ProposalDisposed(_) => "PROPOSAL_DISPOSED",
            Self::EgressDecided(_) => "EGRESS_DECIDED",
            Self::ConsentRecorded(_) => "CONSENT_RECORDED",
            Self::EntityIdentityChanged(_) => "ENTITY_IDENTITY_CHANGED",
            Self::RetentionActionRecorded(_) => "RETENTION_ACTION_RECORDED",
        }
    }

    /// Returns the lowest event schema version whose arm table can carry this payload.
    #[must_use]
    pub const fn minimum_schema_version(&self) -> u16 {
        match self {
            Self::ScopeRegistered(_)
            | Self::ArtifactRegistered(_)
            | Self::EvidenceRegistered(_)
            | Self::ClaimAsserted(_)
            | Self::ClaimRelated(_)
            | Self::DecisionRecorded(_) => EVENT_SCHEMA_VERSION_V1,
            Self::CurriculumVersionPublished(_)
            | Self::CourseRevisionPublished(_)
            | Self::OfferingObserved(_)
            | Self::AttemptRecorded(_)
            | Self::RequirementSetPublished(_)
            | Self::AuditComputed(_)
            | Self::CapturePermissionRecorded(_)
            | Self::LectureSessionRecorded(_)
            | Self::TranscriptVersionAdded(_)
            | Self::LectureDocumentPublished(_)
            | Self::SnapshotRegistered(_)
            | Self::FindingPublished(_)
            | Self::ModelRunRecorded(_)
            | Self::ProposalDisposed(_)
            | Self::EgressDecided(_)
            | Self::ConsentRecorded(_)
            | Self::EntityIdentityChanged(_)
            | Self::RetentionActionRecorded(_) => EVENT_SCHEMA_VERSION_V3,
        }
    }
}

/// Origin-authored event. `accepted_seq` is deliberately absent and assigned by a vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub origin_seq: u64,
    pub origin_observed_at: TimestampMillis,
    pub actor: Actor,
    pub domain_id: DomainId,
    pub payload: EventPayload,
}

impl Event {
    /// Validates nested payload invariants before signing or acceptance.
    pub fn validate(&self) -> Result<(), DomainError> {
        self.actor.validate()?;
        match &self.payload {
            EventPayload::ScopeRegistered(scope) => {
                if scope.domain_id != self.domain_id {
                    return Err(DomainError::InvalidEventPayload(
                        "scope domain must match event domain".to_owned(),
                    ));
                }
                scope.validate()
            }
            EventPayload::ArtifactRegistered(descriptor) => {
                if descriptor.domain_id != self.domain_id {
                    return Err(DomainError::InvalidEventPayload(
                        "artifact domain must match event domain".to_owned(),
                    ));
                }
                descriptor.validate_for_actor(&self.actor)
            }
            EventPayload::EvidenceRegistered(evidence) => evidence.validate(),
            EventPayload::ClaimAsserted(claim) => claim.validate_for_actor(&self.actor),
            EventPayload::ClaimRelated(relation) => {
                if relation.source_claim_id == relation.target_claim_id {
                    Err(DomainError::InvalidEventPayload(
                        "claim relation cannot target itself".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            EventPayload::DecisionRecorded(decision) => {
                if matches!(&self.actor, Actor::User { .. }) {
                    decision.validate()
                } else {
                    Err(DomainError::DecisionActorNotUser)
                }
            }
            EventPayload::CurriculumVersionPublished(record) => record.validate(self.domain_id),
            EventPayload::CourseRevisionPublished(record) => record.validate(self.domain_id),
            EventPayload::OfferingObserved(record) => record.validate(self.domain_id),
            EventPayload::AttemptRecorded(record) => record.validate(self.domain_id),
            EventPayload::RequirementSetPublished(record) => record.validate(self.domain_id),
            EventPayload::AuditComputed(record) => record.validate(self.domain_id),
            EventPayload::CapturePermissionRecorded(record) => record.validate(self.domain_id),
            EventPayload::LectureSessionRecorded(record) => record.validate(self.domain_id),
            EventPayload::TranscriptVersionAdded(record) => record.validate(self.domain_id),
            EventPayload::LectureDocumentPublished(record) => record.validate(self.domain_id),
            EventPayload::SnapshotRegistered(record) => record.validate(self.domain_id),
            EventPayload::FindingPublished(record) => record.validate(self.domain_id),
            EventPayload::ModelRunRecorded(record) => record.validate(self.domain_id),
            EventPayload::ProposalDisposed(record) => record.validate(self.domain_id),
            EventPayload::EgressDecided(record) => record.validate(self.domain_id),
            EventPayload::ConsentRecorded(record) => record.validate(self.domain_id),
            EventPayload::EntityIdentityChanged(record) => record.validate(self.domain_id),
            EventPayload::RetentionActionRecorded(record) => record.validate(self.domain_id),
        }
    }
}

/// Legacy signed-batch semantic version accepted only through deterministic upcasting.
pub const EVENT_SCHEMA_VERSION_V1: u16 = 1;
/// Legacy signed-batch semantic version with durable user-decision applicability.
pub const EVENT_SCHEMA_VERSION_V2: u16 = 2;
/// Current signed-batch semantic version whose arm table carries [`V3_EVENT_KINDS`].
pub const EVENT_SCHEMA_VERSION_V3: u16 = 3;
/// Signed-batch semantic version emitted by current writers.
pub const EVENT_SCHEMA_VERSION: u16 = EVENT_SCHEMA_VERSION_V3;

/// An origin-authored batch before canonical framing and signature verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsignedBatch {
    pub schema_version: u16,
    pub batch_id: BatchId,
    pub device_id: DeviceId,
    pub origin_seq_start: u64,
    pub origin_seq_end: u64,
    pub previous_batch_hash: Option<ContentDigest>,
    pub origin_created_at: TimestampMillis,
    pub events: Vec<Event>,
}

impl UnsignedBatch {
    /// Revalidates the full decoded batch before signing or acceptance.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.schema_version != EVENT_SCHEMA_VERSION {
            return Err(DomainError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.events.is_empty() {
            return Err(DomainError::EmptyBatch);
        }
        let expected_count = self
            .origin_seq_end
            .checked_sub(self.origin_seq_start)
            .and_then(|difference| difference.checked_add(1))
            .ok_or(DomainError::InvalidOriginRange)?;
        if usize::try_from(expected_count).ok() != Some(self.events.len()) {
            return Err(DomainError::InvalidOriginRange);
        }
        for (offset, event) in self.events.iter().enumerate() {
            let expected = self
                .origin_seq_start
                .checked_add(u64::try_from(offset).map_err(|_| DomainError::InvalidOriginRange)?)
                .ok_or(DomainError::InvalidOriginRange)?;
            if event.origin_seq != expected {
                return Err(DomainError::NonContiguousOrigin {
                    expected,
                    actual: event.origin_seq,
                });
            }
            event.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn id<T: FromStr<Err = DomainError>>(suffix: u32) -> Result<T, DomainError> {
        format!("01900000-0000-7000-8000-{suffix:012x}").parse()
    }

    #[test]
    fn uuid_ids_require_rfc_variant_and_version_seven_at_every_constructor() {
        let invalid = [
            "00000000-0000-4000-8000-000000000000",
            "01900000-0000-7000-0000-000000000001",
            "01900000-0000-7000-c000-000000000001",
            "01900000-0000-7000-e000-000000000001",
        ];

        macro_rules! assert_constructor_rejects {
            ($id:ty) => {
                for encoded in invalid {
                    let value = Uuid::parse_str(encoded).expect("test UUID must parse");
                    assert!(matches!(
                        <$id>::try_from_uuid(value),
                        Err(DomainError::InvalidId { .. })
                    ));
                    assert!(encoded.parse::<$id>().is_err());
                    assert!(serde_json::from_str::<$id>(&format!("\"{encoded}\"")).is_err());
                }
            };
        }

        assert_constructor_rejects!(EntityId);
        assert_constructor_rejects!(EventId);
        assert_constructor_rejects!(ClaimId);
        assert_constructor_rejects!(EvidenceId);
        assert_constructor_rejects!(ArtifactId);
        assert_constructor_rejects!(BatchId);
        assert_constructor_rejects!(DeviceId);
        assert_constructor_rejects!(DomainId);
        assert_constructor_rejects!(DecisionId);
        assert_constructor_rejects!(PermissionLineageId);
        assert_constructor_rejects!(ScopeId);
    }

    #[test]
    fn digest_requires_canonical_lowercase_prefix() {
        let digest = ContentDigest::sha256(b"synthetic fixture");
        let encoded = digest.to_string();
        assert_eq!(encoded.parse::<ContentDigest>(), Ok(digest));
        assert!(encoded.to_uppercase().parse::<ContentDigest>().is_err());
    }

    #[test]
    fn keyed_locator_changes_across_domains() -> Result<(), DomainError> {
        let media_type = MediaType::parse("text/plain")?;
        let digest = ContentDigest::sha256(b"same plaintext");
        let first = VaultLocator::derive(b"domain-a", 1, &media_type, digest)?;
        let second = VaultLocator::derive(b"domain-b", 1, &media_type, digest)?;
        assert_ne!(first, second);
        assert!(!first.to_string().contains(&hex::encode(digest.as_bytes())));
        Ok(())
    }

    #[test]
    fn mastery_and_freshness_are_independent_types() {
        let mastery = ClaimObject::Mastery(MasteryLevel::Applied);
        let freshness = ClaimObject::Freshness(FreshnessBand::Stale);
        assert_ne!(mastery, freshness);
    }

    #[test]
    fn constrained_deserialization_cannot_bypass_constructors() {
        assert!(
            serde_json::from_str::<EntityId>("\"00000000-0000-4000-8000-000000000000\"").is_err()
        );
        assert!(
            serde_json::from_str::<EntityId>("\"01900000-0000-7000-8000-0000000000AA\"").is_err()
        );
        assert!(serde_json::from_str::<ValidInterval>("{\"from\":7,\"to\":7}").is_err());
        assert!(
            serde_json::from_str::<PredictionObservationWindow>("{\"from\":7,\"to\":7}").is_err()
        );
        assert!(
            serde_json::from_str::<PredictionMetadata>(
                "{\"version\":2,\"observation_window\":{\"from\":1,\"to\":2},\"positive_sample_count\":1}"
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<PredictionMetadata>(
                "{\"version\":1,\"observation_window\":{\"from\":1,\"to\":2},\"positive_sample_count\":0}"
            )
            .is_err()
        );
        assert!(serde_json::from_str::<MediaType>("\"Text/Plain\"").is_err());
        assert!(serde_json::from_str::<LogicalPath>("\"../escape\"").is_err());
        assert!(serde_json::from_str::<PredicateId>("\"missing_namespace\"").is_err());
        assert!(serde_json::from_str::<ConfidencePermille>("1001").is_err());
        assert!(serde_json::from_str::<Decimal>("{\"coefficient\":\"1\",\"scale\":19}").is_err());
    }

    #[test]
    fn decimal_uses_canonical_string_coefficients_at_i128_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        for coefficient in [i128::MIN, -1, 0, 1, i128::MAX] {
            let value = Decimal::new(coefficient, 18)?;
            let json = serde_json::to_string(&value)?;
            assert!(json.contains(&format!("\"coefficient\":\"{coefficient}\"")));
            assert_eq!(serde_json::from_str::<Decimal>(&json)?, value);
        }
        for invalid in [
            "",
            "+1",
            "01",
            "-0",
            " 1",
            "1 ",
            "170141183460469231731687303715884105728",
            "-170141183460469231731687303715884105729",
        ] {
            let json = format!("{{\"coefficient\":\"{invalid}\",\"scale\":0}}");
            assert!(serde_json::from_str::<Decimal>(&json).is_err(), "{invalid}");
        }
        assert!(serde_json::from_str::<Decimal>("{\"coefficient\":1,\"scale\":0}").is_err());
        Ok(())
    }

    #[test]
    fn logical_path_rejects_all_platform_absolute_and_uri_forms() {
        for invalid in [
            "/etc/passwd",
            "C:/Windows/System32",
            "c:relative.txt",
            "C:\\Windows\\System32",
            "\\\\server\\share\\file",
            "\\\\?\\C:\\device",
            "\\\\.\\PhysicalDrive0",
            "file:///tmp/data",
            "https://example.invalid/a",
            "urn:academic:test",
            "~/private",
            "src//lib.rs",
            "src/",
        ] {
            assert!(LogicalPath::parse(invalid).is_err(), "{invalid}");
        }
        for valid in ["src/domain.rs", "docs/설계.md", "a/b-c_1.txt"] {
            assert_eq!(
                LogicalPath::parse(valid).map(|path| path.0),
                Ok(valid.to_owned())
            );
        }
    }

    #[test]
    fn t007_artifact_descriptor_mutation_corpus_matches_rust_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Deserialize)]
        struct Corpus {
            base: serde_json::Value,
            raw_number_cases: Vec<RawNumberCase>,
            raw_json_cases: Vec<RawJsonCase>,
            cases: Vec<Case>,
        }
        #[derive(Deserialize)]
        struct Case {
            name: String,
            mutations: Vec<Mutation>,
            schema_valid: bool,
            semantic_valid: bool,
        }
        #[derive(Deserialize)]
        struct RawNumberCase {
            name: String,
            mutations: Vec<Mutation>,
            path: String,
            token: String,
            valid: bool,
        }
        #[derive(Deserialize)]
        struct RawJsonCase {
            name: String,
            raw_json: String,
            valid: bool,
        }
        #[derive(Clone, Deserialize)]
        struct Mutation {
            op: String,
            path: String,
            value: serde_json::Value,
        }

        fn apply_mutations(
            base: &serde_json::Value,
            mutations: &[Mutation],
            name: &str,
        ) -> Result<serde_json::Value, String> {
            let mut candidate = base.clone();
            for mutation in mutations {
                match mutation.op.as_str() {
                    "replace" | "append" => {
                        let target = candidate.pointer_mut(&mutation.path).ok_or_else(|| {
                            format!("{name}: invalid mutation path {}", mutation.path)
                        })?;
                        if mutation.op == "replace" {
                            *target = mutation.value.clone();
                        } else {
                            target
                                .as_array_mut()
                                .ok_or_else(|| format!("{name}: append target is not an array"))?
                                .push(mutation.value.clone());
                        }
                    }
                    "add" => {
                        let (parent_path, encoded_key) =
                            mutation.path.rsplit_once('/').ok_or_else(|| {
                                format!("{name}: add path must identify an object property")
                            })?;
                        let parent = if parent_path.is_empty() {
                            &mut candidate
                        } else {
                            candidate.pointer_mut(parent_path).ok_or_else(|| {
                                format!("{name}: invalid mutation parent {parent_path}")
                            })?
                        };
                        let key = encoded_key.replace("~1", "/").replace("~0", "~");
                        let object = parent
                            .as_object_mut()
                            .ok_or_else(|| format!("{name}: add target parent is not an object"))?;
                        if object.insert(key, mutation.value.clone()).is_some() {
                            return Err(format!("{name}: add target already exists"));
                        }
                    }
                    other => return Err(format!("{name}: unknown mutation op {other}")),
                }
            }
            Ok(candidate)
        }

        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/artifact-descriptor-parity-v1.json"
        ))?;
        for case in &corpus.cases {
            let candidate = apply_mutations(&corpus.base, &case.mutations, &case.name)?;
            let rust_valid = serde_json::from_value::<ArtifactDescriptor>(candidate)
                .is_ok_and(|descriptor| descriptor.validate().is_ok());
            assert_eq!(
                rust_valid,
                case.schema_valid && case.semantic_valid,
                "artifact parity corpus disagreement: {}",
                case.name
            );
        }
        for case in &corpus.raw_number_cases {
            let mut candidate = apply_mutations(&corpus.base, &case.mutations, &case.name)?;
            let target = candidate
                .pointer_mut(&case.path)
                .ok_or_else(|| format!("{}: invalid raw number path {}", case.name, case.path))?;
            *target = serde_json::Value::String("__RAW_INTEGER_TOKEN__".to_owned());
            let template = serde_json::to_string(&candidate)?;
            let raw = template.replacen("\"__RAW_INTEGER_TOKEN__\"", &case.token, 1);
            let rust_valid = validate_artifact_json_number_tokens(&raw).is_ok()
                && serde_json::from_str::<ArtifactDescriptor>(&raw)
                    .is_ok_and(|descriptor| descriptor.validate().is_ok());
            assert_eq!(
                rust_valid, case.valid,
                "raw artifact number parity disagreement: {} ({})",
                case.name, case.token,
            );
        }
        for case in &corpus.raw_json_cases {
            let rust_valid = validate_artifact_json_number_tokens(&case.raw_json).is_ok()
                && serde_json::from_str::<ArtifactDescriptor>(&case.raw_json)
                    .is_ok_and(|descriptor| descriptor.validate().is_ok());
            assert_eq!(
                rust_valid, case.valid,
                "raw artifact JSON parity disagreement: {}",
                case.name,
            );
        }
        Ok(())
    }

    #[test]
    fn claim_actor_authority_status_matrix_fails_closed() -> Result<(), Box<dyn std::error::Error>>
    {
        let user = Actor::User { user_id: id(1)? };
        let engine = Actor::DeterministicEngine {
            name: "engine".to_owned(),
            version: "1".to_owned(),
        };
        let model = Actor::ModelRun { run_id: id(2)? };
        let importer = Actor::Importer {
            name: "importer".to_owned(),
            version: "1".to_owned(),
        };
        let actors = [&user, &engine, &model, &importer];
        let rows = [
            (
                AuthorityClass::Official,
                EpistemicStatus::OfficialConfirmed,
                [false, false, false, true],
            ),
            (
                AuthorityClass::UserExplicit,
                EpistemicStatus::UserConfirmed,
                [true, false, false, false],
            ),
            (
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                [false, false, false, true],
            ),
            (
                AuthorityClass::DeterministicEngine,
                EpistemicStatus::DeterministicDerived,
                [false, true, false, false],
            ),
            (
                AuthorityClass::Curated,
                EpistemicStatus::DeterministicDerived,
                [false, false, false, true],
            ),
            (
                AuthorityClass::ModelInference,
                EpistemicStatus::AiInferred,
                [false, false, true, false],
            ),
            (
                AuthorityClass::Prediction,
                EpistemicStatus::Prediction,
                [false, false, true, false],
            ),
            (
                AuthorityClass::Unknown,
                EpistemicStatus::Unknown,
                [false, false, false, true],
            ),
        ];
        for (row_index, (authority, status, expected)) in rows.into_iter().enumerate() {
            let is_prediction = status == EpistemicStatus::Prediction;
            let claim = Claim {
                id: id(100 + u32::try_from(row_index)?)?,
                subject_entity_id: id(10)?,
                predicate_id: PredicateId::parse("test.value")?,
                object: ClaimObject::Text("synthetic".to_owned()),
                scope_id: id(11)?,
                authority_class: authority,
                epistemic_status: status,
                confidence: is_prediction
                    .then(|| ConfidencePermille::new(500))
                    .transpose()?,
                prediction_metadata: is_prediction
                    .then(|| {
                        PredictionMetadata::new(
                            PredictionObservationWindow::new(
                                TimestampMillis::new(-10),
                                TimestampMillis::new(0),
                            )?,
                            1,
                        )
                    })
                    .transpose()?,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
                evidence_ids: if status == EpistemicStatus::Unknown {
                    Vec::new()
                } else {
                    vec![id(12)?]
                },
            };
            for (actor, permitted) in actors.into_iter().zip(expected) {
                assert_eq!(claim.validate_for_actor(actor).is_ok(), permitted);
            }
        }
        Ok(())
    }

    #[test]
    fn prediction_requires_confidence_and_typed_metadata_without_changing_ai_inference()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = Actor::ModelRun { run_id: id(160)? };
        let mut claim = Claim {
            id: id(161)?,
            subject_entity_id: id(162)?,
            predicate_id: PredicateId::parse("academic.course.offering")?,
            object: ClaimObject::Boolean(true),
            scope_id: id(163)?,
            authority_class: AuthorityClass::Prediction,
            epistemic_status: EpistemicStatus::Prediction,
            confidence: None,
            prediction_metadata: Some(PredictionMetadata::new(
                PredictionObservationWindow::new(
                    TimestampMillis::new(i64::MIN),
                    TimestampMillis::new(i64::MAX),
                )?,
                u32::MAX,
            )?),
            valid_time: ValidInterval::new(
                TimestampMillis::new(10),
                Some(TimestampMillis::new(20)),
            )?,
            evidence_ids: vec![id(164)?],
        };
        assert_eq!(
            claim.validate_for_actor(&model),
            Err(DomainError::MissingPredictionConfidence)
        );

        claim.confidence = Some(ConfidencePermille::new(0)?);
        claim.prediction_metadata = None;
        assert_eq!(
            claim.validate_for_actor(&model),
            Err(DomainError::MissingPredictionMetadata)
        );

        claim.prediction_metadata = Some(PredictionMetadata::new(
            PredictionObservationWindow::new(TimestampMillis::new(-20), TimestampMillis::new(0))?,
            1,
        )?);
        assert!(claim.validate_for_actor(&model).is_ok());
        assert_ne!(
            claim
                .prediction_metadata
                .map(PredictionMetadata::observation_window),
            Some(PredictionObservationWindow::new(
                claim.valid_time.from(),
                claim
                    .valid_time
                    .to()
                    .ok_or("test valid_time must be bounded")?,
            )?)
        );

        claim.authority_class = AuthorityClass::ModelInference;
        claim.epistemic_status = EpistemicStatus::AiInferred;
        claim.confidence = None;
        claim.prediction_metadata = None;
        assert!(claim.validate_for_actor(&model).is_ok());

        claim.prediction_metadata = Some(PredictionMetadata::new(
            PredictionObservationWindow::new(TimestampMillis::new(0), TimestampMillis::new(1))?,
            1,
        )?);
        assert_eq!(
            claim.validate_for_actor(&model),
            Err(DomainError::PredictionMetadataNotAllowed(
                EpistemicStatus::AiInferred
            ))
        );
        Ok(())
    }

    #[test]
    fn t013_curated_deterministic_claims_require_importer_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let importer = Actor::Importer {
            name: "curated-ontology".to_owned(),
            version: "1".to_owned(),
        };
        let mut claim = Claim {
            id: id(180)?,
            subject_entity_id: id(181)?,
            predicate_id: PredicateId::parse("knowledge.prerequisite")?,
            object: ClaimObject::Text("synthetic curated relation".to_owned()),
            scope_id: id(182)?,
            authority_class: AuthorityClass::Curated,
            epistemic_status: EpistemicStatus::DeterministicDerived,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
            evidence_ids: vec![id(183)?],
        };
        assert!(claim.validate_for_actor(&importer).is_ok());
        for actor in [
            Actor::User { user_id: id(184)? },
            Actor::DeterministicEngine {
                name: "engine".to_owned(),
                version: "1".to_owned(),
            },
            Actor::ModelRun { run_id: id(185)? },
        ] {
            assert!(
                matches!(
                    claim.validate_for_actor(&actor),
                    Err(DomainError::ActorAuthorityMismatch { .. })
                ),
                "only an importer may author Curated + DeterministicDerived"
            );
        }
        for status in [
            EpistemicStatus::OfficialConfirmed,
            EpistemicStatus::UserConfirmed,
            EpistemicStatus::CodeObserved,
            EpistemicStatus::AiInferred,
            EpistemicStatus::Prediction,
            EpistemicStatus::Unknown,
        ] {
            claim.epistemic_status = status;
            claim.evidence_ids = if status == EpistemicStatus::Unknown {
                Vec::new()
            } else {
                vec![id(183)?]
            };
            assert!(
                matches!(
                    claim.validate_for_actor(&importer),
                    Err(DomainError::StatusAuthorityMismatch { .. })
                ),
                "Curated must not pair with active status {status:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn model_event_cannot_impersonate_user_confirmed_claim()
    -> Result<(), Box<dyn std::error::Error>> {
        let event = Event {
            id: id(200)?,
            origin_seq: 1,
            origin_observed_at: TimestampMillis::new(0),
            actor: Actor::ModelRun { run_id: id(201)? },
            domain_id: id(202)?,
            payload: EventPayload::ClaimAsserted(Claim {
                id: id(203)?,
                subject_entity_id: id(204)?,
                predicate_id: PredicateId::parse("test.value")?,
                object: ClaimObject::Text("synthetic".to_owned()),
                scope_id: id(205)?,
                authority_class: AuthorityClass::UserExplicit,
                epistemic_status: EpistemicStatus::UserConfirmed,
                confidence: None,
                prediction_metadata: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
                evidence_ids: vec![id(206)?],
            }),
        };
        assert!(matches!(
            event.validate(),
            Err(DomainError::ActorAuthorityMismatch { .. })
        ));
        Ok(())
    }

    proptest! {
        #[test]
        fn half_open_interval_contains_only_its_domain(from in -1_000_000_i64..1_000_000_i64, width in 1_i64..10_000_i64) {
            let to = from + width;
            let interval = ValidInterval::new(TimestampMillis::new(from), Some(TimestampMillis::new(to)))?;
            prop_assert!(interval.contains(TimestampMillis::new(from)));
            prop_assert!(interval.contains(TimestampMillis::new(to - 1)));
            prop_assert!(!interval.contains(TimestampMillis::new(to)));
        }

        #[test]
        fn invalid_intervals_are_rejected(from in -1_000_000_i64..1_000_000_i64, delta in -10_000_i64..=0_i64) {
            let result = ValidInterval::new(
                TimestampMillis::new(from),
                Some(TimestampMillis::new(from + delta)),
            );
            prop_assert_eq!(result, Err(DomainError::InvalidInterval));
        }
    }
}
