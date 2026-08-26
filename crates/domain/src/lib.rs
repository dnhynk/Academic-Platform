//! Canonical Phase 0 domain vocabulary.
//!
//! This crate intentionally contains no storage or network code. It makes the
//! evidence, authority, epistemic status, and time semantics executable before
//! a transactional store is selected.

use std::{fmt, str::FromStr};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeStruct};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

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
            /// Constructs the identifier only when the UUID version is seven.
            pub fn try_from_uuid(value: Uuid) -> Result<Self, DomainError> {
                if value.get_version_num() == 7 {
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

impl ArtifactDescriptor {
    /// Validates the descriptor fields that are fixed in Phase 0.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.format_version != 1 {
            return Err(DomainError::InvalidEventPayload(
                "unsupported artifact format version".to_owned(),
            ));
        }
        for (index, representation) in self.evidence_representations.iter().enumerate() {
            representation.locator.validate()?;
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

    /// Returns whether exact descriptor metadata proves the evidence locator and digest.
    #[must_use]
    pub fn supports_evidence(&self, evidence: &EvidenceItem) -> bool {
        evidence.artifact_id == self.id
            && self.evidence_representations.iter().any(|representation| {
                representation.locator == evidence.locator
                    && representation.content_digest == evidence.excerpt_digest
            })
    }
}

/// Exact evidence location inside an immutable artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
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
            | Self::RepositoryBytes { start, end, .. } => start < end,
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

/// Immutable explicit user decision that automated inference cannot erase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDecision {
    pub id: DecisionId,
    pub target_claim_id: ClaimId,
    pub action: DecisionAction,
    pub scope_id: ScopeId,
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

/// Canonical event payloads admitted by the Phase 0 pure ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventPayload {
    ScopeRegistered(ScopeDescriptor),
    ArtifactRegistered(ArtifactDescriptor),
    EvidenceRegistered(EvidenceItem),
    ClaimAsserted(Claim),
    ClaimRelated(ClaimRelation),
    DecisionRecorded(UserDecision),
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
                descriptor.validate()
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
        }
    }
}

/// Signed batch semantic version implemented by the Phase 0 ledger.
pub const EVENT_SCHEMA_VERSION: u16 = 1;

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
    fn uuid_id_rejects_non_v7() {
        let result = EntityId::try_from_uuid(Uuid::nil());
        assert!(matches!(result, Err(DomainError::InvalidId { .. })));
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
                EpistemicStatus::Disputed,
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
            let claim = Claim {
                id: id(100 + u32::try_from(row_index)?)?,
                subject_entity_id: id(10)?,
                predicate_id: PredicateId::parse("test.value")?,
                object: ClaimObject::Text("synthetic".to_owned()),
                scope_id: id(11)?,
                authority_class: authority,
                epistemic_status: status,
                confidence: None,
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
