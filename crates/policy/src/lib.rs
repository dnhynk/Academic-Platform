//! Default-deny permission decisions and runtime egress capabilities.
//!
//! The broker keeps policy evaluation, grant consumption, and audit writes in
//! one boundary. It never opens a socket and it never persists the payload it
//! temporarily hashes at runtime.

use std::{
    collections::HashMap,
    fmt,
    sync::{
        Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use rusqlite::{Connection, OptionalExtension as _, Transaction, params};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// SQL schema owned by the broker's operational store.
pub const POLICY_SCHEMA_SQL: &str = include_str!("schema.sql");
/// Default validity window for a minted one-use grant, in milliseconds.
pub const DEFAULT_GRANT_TTL_MILLIS: u64 = 60_000;

const MISSING_VALUE: &str = "<missing>";

/// A lowercase SHA-256 digest.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Hashes bytes into the digest used by policy, grant, and audit rows.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(lower_hex(&Sha256::digest(bytes)))
    }

    /// Parses an exact lowercase hexadecimal SHA-256 digest.
    pub fn parse(value: impl Into<String>) -> Result<Self, BrokerError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(BrokerError::InvalidDigest);
        }
        Ok(Self(value))
    }

    /// Returns the canonical lowercase hexadecimal representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ContentDigest")
            .field(&self.0)
            .finish()
    }
}

/// One half-open byte range and the digest of the exact bytes in that range.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRange {
    object_id: String,
    start: u64,
    end: u64,
    content_digest: ContentDigest,
}

impl ObjectRange {
    /// Constructs a non-empty half-open byte range.
    pub fn new(
        object_id: impl Into<String>,
        start: u64,
        end: u64,
        content_digest: ContentDigest,
    ) -> Result<Self, BrokerError> {
        let object_id = object_id.into();
        if object_id.is_empty() || start >= end {
            return Err(BrokerError::InvalidRange);
        }
        Ok(Self {
            object_id,
            start,
            end,
            content_digest,
        })
    }

    /// Stable object identifier.
    #[must_use]
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Inclusive range start.
    #[must_use]
    pub const fn start(&self) -> u64 {
        self.start
    }

    /// Exclusive range end.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.end
    }

    /// Digest of the exact bytes named by this range.
    #[must_use]
    pub const fn content_digest(&self) -> &ContentDigest {
        &self.content_digest
    }

    fn contains(&self, required: &Self) -> bool {
        self.object_id == required.object_id
            && self.start <= required.start
            && self.end >= required.end
    }

    fn byte_count(&self) -> u64 {
        self.end - self.start
    }
}

/// Hash of one immutable, canonically encoded policy snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolicyVersion(String);

impl PolicyVersion {
    /// Returns the lowercase SHA-256 policy hash.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One explicit per-tuple user rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressRule {
    /// Exact process class allowed to request the capability.
    pub actor_process_class: String,
    /// Exact data class configured by the user.
    pub data_class: String,
    /// Exact operation.
    pub operation: String,
    /// Exact declared purpose.
    pub purpose_id: String,
    /// Exact destination/provider.
    pub destination_id: String,
    /// Exact provider retention terms.
    pub retention_terms_hash: ContentDigest,
    /// Exact consent event/evidence.
    pub consent_evidence_id: String,
    /// First requested-at millisecond accepted by this rule.
    pub valid_from: u64,
    /// Exclusive requested-at millisecond upper bound.
    pub valid_until: u64,
    /// Smallest exact byte ranges the declared operation requires.
    pub minimal_ranges: Vec<ObjectRange>,
    /// Digest of the exact concatenated payload the runtime may receive.
    pub payload_digest: ContentDigest,
    /// Provider-policy snapshot fixed when the user configured the rule.
    pub provider_policy_snapshot_digest: ContentDigest,
    /// Whether provider training use is part of the explicit permission.
    pub training_use_allowed: bool,
    /// Exact redaction policy applied before this decision.
    pub redaction_policy_hash: ContentDigest,
}

impl EgressRule {
    fn is_structurally_valid(&self) -> bool {
        !self.actor_process_class.is_empty()
            && !self.data_class.is_empty()
            && !self.operation.is_empty()
            && !self.purpose_id.is_empty()
            && !self.destination_id.is_empty()
            && !self.consent_evidence_id.is_empty()
            && self.valid_from < self.valid_until
            && !self.minimal_ranges.is_empty()
            && canonical_ranges(&self.minimal_ranges).is_some()
    }

    fn matches_tuple(&self, request: &CompleteRequest<'_>) -> bool {
        self.actor_process_class == request.actor_process_class
            && self.data_class == request.data_class
            && self.operation == request.operation
            && self.purpose_id == request.purpose_id
            && self.destination_id == request.destination_id
            && &self.retention_terms_hash == request.retention_terms_hash
            && self.consent_evidence_id == request.consent_evidence_id
            && self.valid_from <= request.requested_at
            && request.requested_at < self.valid_until
    }

    fn requested_scope_contains_minimum(&self, requested: &[ObjectRange]) -> bool {
        self.minimal_ranges
            .iter()
            .all(|required| requested.iter().any(|range| range.contains(required)))
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_string(&mut bytes, &self.actor_process_class);
        push_string(&mut bytes, &self.data_class);
        push_string(&mut bytes, &self.operation);
        push_string(&mut bytes, &self.purpose_id);
        push_string(&mut bytes, &self.destination_id);
        push_string(&mut bytes, self.retention_terms_hash.as_str());
        push_string(&mut bytes, &self.consent_evidence_id);
        bytes.extend_from_slice(&self.valid_from.to_be_bytes());
        bytes.extend_from_slice(&self.valid_until.to_be_bytes());
        push_ranges(&mut bytes, &self.minimal_ranges);
        push_string(&mut bytes, self.payload_digest.as_str());
        push_string(&mut bytes, self.provider_policy_snapshot_digest.as_str());
        bytes.push(u8::from(self.training_use_allowed));
        push_string(&mut bytes, self.redaction_policy_hash.as_str());
        bytes
    }
}

/// Immutable policy input. Rule order does not affect its version hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicySnapshot {
    local_processing_preferred: bool,
    rules: Vec<EgressRule>,
}

impl PolicySnapshot {
    /// Creates a snapshot from explicit user rules.
    pub fn from_rules(mut rules: Vec<EgressRule>) -> Result<Self, BrokerError> {
        if rules.iter().any(|rule| !rule.is_structurally_valid()) {
            return Err(BrokerError::InvalidRule);
        }
        for rule in &mut rules {
            rule.minimal_ranges.sort();
        }
        rules.sort_by_key(EgressRule::canonical_bytes);
        Ok(Self {
            local_processing_preferred: true,
            rules,
        })
    }

    /// New-profile snapshot: local processing is preferred and no egress tuple is configured.
    #[must_use]
    pub const fn local_first_default_deny() -> Self {
        Self {
            local_processing_preferred: true,
            rules: Vec::new(),
        }
    }

    /// Whether the observable profile preference is local-first.
    #[must_use]
    pub const fn local_processing_preferred(&self) -> bool {
        self.local_processing_preferred
    }

    /// Number of explicitly configured egress rules.
    #[must_use]
    pub fn configured_egress_rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Deterministic SHA-256 over the canonical snapshot encoding.
    #[must_use]
    pub fn version(&self) -> PolicyVersion {
        PolicyVersion(lower_hex(&Sha256::digest(self.canonical_bytes())))
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = b"academic-policy-snapshot-v1\0".to_vec();
        bytes.push(u8::from(self.local_processing_preferred));
        bytes.extend_from_slice(
            &u64::try_from(self.rules.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        for rule in &self.rules {
            let encoded = rule.canonical_bytes();
            push_bytes(&mut bytes, &encoded);
        }
        bytes
    }
}

impl Default for PolicySnapshot {
    fn default() -> Self {
        Self::local_first_default_deny()
    }
}

/// The concrete §3.5 request fields. Every `Option` must resolve before any payload read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// Process class.
    pub actor_process_class: Option<String>,
    /// Data class.
    pub data_class: Option<String>,
    /// Requested object/range digest set.
    pub object_range_digest_set: Option<Vec<ObjectRange>>,
    /// Operation.
    pub operation: Option<String>,
    /// Purpose identifier.
    pub purpose_id: Option<String>,
    /// Destination/provider identifier.
    pub destination_id: Option<String>,
    /// Provider retention terms.
    pub retention_terms_hash: Option<ContentDigest>,
    /// Caller-recorded request time.
    pub requested_at: Option<u64>,
    /// Consent event/evidence identifier.
    pub consent_evidence_id: Option<String>,
    /// Pinned policy version.
    pub policy_version: Option<PolicyVersion>,
}

/// Closed reason-code enum fixed by §3.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonCode {
    /// No exact configured grant can be issued or found.
    NoGrant,
    /// The grant expired before use.
    GrantExpired,
    /// The one allowed use was already consumed.
    GrantConsumed,
    /// Requested/runtime scope is outside the configured capability.
    ScopeMismatch,
    /// The pinned policy version cannot be resolved.
    PolicyStale,
    /// Provider terms no longer match policy.
    ProviderPolicyIncompatible,
    /// Secret/DLP scanner failed.
    ScannerError,
    /// Secret pattern detected.
    SecretPattern,
    /// Secret entropy detected.
    SecretEntropy,
    /// Personal data detected.
    PiiDetected,
    /// Binary input could not be classified.
    UnknownBinary,
    /// Input exceeded a configured bound.
    Oversize,
    /// Required redaction destroyed meaning.
    RedactionDestroysMeaning,
    /// A canary appeared in a provider response.
    CanaryInResponse,
    /// Provider deletion receipt was required but absent.
    NoDeletionReceipt,
}

impl ReasonCode {
    /// Stable database spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoGrant => "NO_GRANT",
            Self::GrantExpired => "GRANT_EXPIRED",
            Self::GrantConsumed => "GRANT_CONSUMED",
            Self::ScopeMismatch => "SCOPE_MISMATCH",
            Self::PolicyStale => "POLICY_STALE",
            Self::ProviderPolicyIncompatible => "PROVIDER_POLICY_INCOMPATIBLE",
            Self::ScannerError => "SCANNER_ERROR",
            Self::SecretPattern => "SECRET_PATTERN",
            Self::SecretEntropy => "SECRET_ENTROPY",
            Self::PiiDetected => "PII_DETECTED",
            Self::UnknownBinary => "UNKNOWN_BINARY",
            Self::Oversize => "OVERSIZE",
            Self::RedactionDestroysMeaning => "REDACTION_DESTROYS_MEANING",
            Self::CanaryInResponse => "CANARY_IN_RESPONSE",
            Self::NoDeletionReceipt => "NO_DELETION_RECEIPT",
        }
    }

    fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "NO_GRANT" => Ok(Self::NoGrant),
            "GRANT_EXPIRED" => Ok(Self::GrantExpired),
            "GRANT_CONSUMED" => Ok(Self::GrantConsumed),
            "SCOPE_MISMATCH" => Ok(Self::ScopeMismatch),
            "POLICY_STALE" => Ok(Self::PolicyStale),
            "PROVIDER_POLICY_INCOMPATIBLE" => Ok(Self::ProviderPolicyIncompatible),
            "SCANNER_ERROR" => Ok(Self::ScannerError),
            "SECRET_PATTERN" => Ok(Self::SecretPattern),
            "SECRET_ENTROPY" => Ok(Self::SecretEntropy),
            "PII_DETECTED" => Ok(Self::PiiDetected),
            "UNKNOWN_BINARY" => Ok(Self::UnknownBinary),
            "OVERSIZE" => Ok(Self::Oversize),
            "REDACTION_DESTROYS_MEANING" => Ok(Self::RedactionDestroysMeaning),
            "CANARY_IN_RESPONSE" => Ok(Self::CanaryInResponse),
            "NO_DELETION_RECEIPT" => Ok(Self::NoDeletionReceipt),
            _ => Err(BrokerError::CorruptAuditReason),
        }
    }
}

/// Audit decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Capability issued or used.
    Allow,
    /// Capability refused.
    Deny,
}

impl Decision {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
        }
    }
}

/// Persisted grant shape fixed by §3.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRow {
    /// Opaque grant identifier.
    pub grant_id: String,
    /// Digest of the complete request tuple.
    pub request_digest: String,
    /// Digest of the exact payload.
    pub payload_digest: String,
    /// Canonical half-open byte ranges.
    pub byte_ranges_canonical: String,
    /// Purpose identifier.
    pub purpose_id: String,
    /// Provider identifier.
    pub provider_id: String,
    /// Provider-policy snapshot digest.
    pub provider_policy_snapshot_digest: String,
    /// Retention-terms digest.
    pub retention_terms_hash: String,
    /// Explicit training-use setting.
    pub training_use_allowed: bool,
    /// Redaction-policy digest.
    pub redaction_policy_hash: String,
    /// Issue time.
    pub issued_at: u64,
    /// Expiry time.
    pub expires_at: u64,
    /// Fixed at one.
    pub max_uses: u8,
    /// First successful runtime use, if any.
    pub consumed_at: Option<u64>,
    /// Consent event identifier.
    pub consent_event_id: String,
}

/// Persisted audit shape fixed by §3.5. It has digests and counts, never payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    /// Append order.
    pub audit_seq: u64,
    /// Related grant when one exists.
    pub grant_id: Option<String>,
    /// Allow/deny decision.
    pub decision: Decision,
    /// Denial code; allow rows use `None` because the fixed enum has no allow code.
    pub reason_code: Option<ReasonCode>,
    /// Process class, or the missing-field marker.
    pub actor_process_class: String,
    /// Payload digest when it was resolved without storing payload.
    pub payload_digest: Option<String>,
    /// Exact payload byte count, or zero before resolution.
    pub byte_count: u64,
    /// Destination identifier, or the missing-field marker.
    pub destination_id: String,
    /// Decision start.
    pub started_at: u64,
    /// Decision finish.
    pub finished_at: u64,
    /// Provider response digest when a later stage records one.
    pub provider_response_digest: Option<String>,
    /// Provider deletion receipt when a later stage records one.
    pub deletion_receipt_id: Option<String>,
}

/// Opaque single-use runtime capability. It cannot be constructed outside this crate.
pub struct CapabilityToken {
    grant_id: String,
    actor_process_class: String,
    operation: String,
    purpose_id: String,
    destination_id: String,
    ranges: Vec<ObjectRange>,
    payload_digest: ContentDigest,
}

impl fmt::Debug for CapabilityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CapabilityToken(<opaque>)")
    }
}

/// Deterministic decision material retained for replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionFingerprint {
    /// Request digest.
    pub request_digest: String,
    /// Pinned policy hash.
    pub policy_version: Option<String>,
    /// Decision.
    pub decision: Decision,
    /// Denial reason.
    pub reason_code: Option<ReasonCode>,
    /// Minimized ranges for an allow.
    pub byte_ranges_canonical: Option<String>,
    /// Exact payload digest for an allow.
    pub payload_digest: Option<String>,
}

/// Complete replay input and the decision originally observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionReceipt {
    request: PermissionRequest,
    fingerprint: DecisionFingerprint,
    grant_id: Option<String>,
}

impl DecisionReceipt {
    /// Original request.
    #[must_use]
    pub const fn request(&self) -> &PermissionRequest {
        &self.request
    }

    /// Original deterministic decision.
    #[must_use]
    pub const fn fingerprint(&self) -> &DecisionFingerprint {
        &self.fingerprint
    }

    /// Related grant identifier when the decision allowed issuance.
    #[must_use]
    pub fn grant_id(&self) -> Option<&str> {
        self.grant_id.as_deref()
    }
}

/// Result of one broker evaluation.
#[derive(Debug)]
pub struct DecisionOutcome {
    /// Deterministic replay receipt.
    pub receipt: DecisionReceipt,
    /// Opaque capability only on allow.
    pub capability: Option<CapabilityToken>,
}

/// Runtime call metadata plus the exact payload to hash at the boundary.
pub struct RuntimeToolCall<'a> {
    actor_process_class: String,
    operation: String,
    purpose_id: String,
    destination_id: String,
    ranges: Vec<ObjectRange>,
    payload: &'a [u8],
}

impl<'a> RuntimeToolCall<'a> {
    /// Constructs the only runtime input accepted by [`PermissionBroker::execute`].
    pub fn new(
        actor_process_class: impl Into<String>,
        operation: impl Into<String>,
        purpose_id: impl Into<String>,
        destination_id: impl Into<String>,
        ranges: Vec<ObjectRange>,
        payload: &'a [u8],
    ) -> Result<Self, BrokerError> {
        let call = Self {
            actor_process_class: actor_process_class.into(),
            operation: operation.into(),
            purpose_id: purpose_id.into(),
            destination_id: destination_id.into(),
            ranges,
            payload,
        };
        if call.actor_process_class.is_empty()
            || call.operation.is_empty()
            || call.purpose_id.is_empty()
            || call.destination_id.is_empty()
            || canonical_ranges(&call.ranges).is_none()
        {
            return Err(BrokerError::InvalidRuntimeCall);
        }
        Ok(call)
    }
}

impl fmt::Debug for RuntimeToolCall<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeToolCall")
            .field("actor_process_class", &self.actor_process_class)
            .field("operation", &self.operation)
            .field("purpose_id", &self.purpose_id)
            .field("destination_id", &self.destination_id)
            .field("ranges", &self.ranges)
            .field(
                "payload",
                &format_args!("<redacted:{} bytes>", self.payload.len()),
            )
            .finish()
    }
}

/// Payload view supplied to the tool only after the capability boundary succeeds.
pub struct AuthorizedToolCall<'a> {
    payload: &'a [u8],
    ranges: &'a [ObjectRange],
}

impl AuthorizedToolCall<'_> {
    /// Exact authorized bytes.
    #[must_use]
    pub const fn payload(&self) -> &[u8] {
        self.payload
    }

    /// Exact authorized ranges.
    #[must_use]
    pub const fn ranges(&self) -> &[ObjectRange] {
        self.ranges
    }
}

impl fmt::Debug for AuthorizedToolCall<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizedToolCall")
            .field(
                "payload",
                &format_args!("<redacted:{} bytes>", self.payload.len()),
            )
            .field("ranges", &self.ranges)
            .finish()
    }
}

/// Broker failure. Policy denials are typed separately from storage failures.
#[derive(Debug, Error)]
pub enum BrokerError {
    /// A SHA-256 digest was not exact lowercase hexadecimal.
    #[error("invalid SHA-256 digest")]
    InvalidDigest,
    /// A byte range was empty or malformed.
    #[error("invalid object range")]
    InvalidRange,
    /// An explicit policy rule was malformed.
    #[error("invalid policy rule")]
    InvalidRule,
    /// Runtime call metadata was malformed.
    #[error("invalid runtime tool call")]
    InvalidRuntimeCall,
    /// A wall-clock value did not fit the SQLite integer contract.
    #[error("time value is outside the SQLite integer range")]
    TimeOutOfRange,
    /// Internal policy/storage lock was poisoned.
    #[error("permission broker lock was poisoned")]
    LockPoisoned,
    /// The audit database contained a reason outside the closed enum.
    #[error("audit row contained an unknown reason code")]
    CorruptAuditReason,
    /// A capability decision denied the request or runtime call.
    #[error("permission denied: {0:?}")]
    Denied(ReasonCode),
    /// SQLite failure.
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// Default-deny policy registry, grant store, and runtime capability boundary.
pub struct PermissionBroker {
    connection: Mutex<Connection>,
    policies: RwLock<HashMap<PolicyVersion, PolicySnapshot>>,
    default_policy_version: PolicyVersion,
    grant_ttl_millis: u64,
    next_grant: AtomicU64,
}

impl fmt::Debug for PermissionBroker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PermissionBroker")
            .field("default_policy_version", &self.default_policy_version)
            .field("grant_ttl_millis", &self.grant_ttl_millis)
            .finish_non_exhaustive()
    }
}

impl PermissionBroker {
    /// Creates a new local-first profile whose observable egress policy is empty and deny-by-default.
    pub fn new_profile() -> Result<Self, BrokerError> {
        Self::new_profile_with_ttl(DEFAULT_GRANT_TTL_MILLIS)
    }

    /// Creates a new profile with an explicit testable token lifetime.
    pub fn new_profile_with_ttl(grant_ttl_millis: u64) -> Result<Self, BrokerError> {
        if grant_ttl_millis == 0 {
            return Err(BrokerError::InvalidRule);
        }
        let connection = Connection::open_in_memory()?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        connection.execute_batch(POLICY_SCHEMA_SQL)?;
        let default = PolicySnapshot::local_first_default_deny();
        let default_policy_version = default.version();
        let mut policies = HashMap::new();
        policies.insert(default_policy_version.clone(), default);
        Ok(Self {
            connection: Mutex::new(connection),
            policies: RwLock::new(policies),
            default_policy_version,
            grant_ttl_millis,
            next_grant: AtomicU64::new(1),
        })
    }

    /// Hash of the initial local-first default-deny snapshot.
    #[must_use]
    pub const fn default_policy_version(&self) -> &PolicyVersion {
        &self.default_policy_version
    }

    /// Returns the observable initial policy state.
    pub fn default_policy_snapshot(&self) -> Result<PolicySnapshot, BrokerError> {
        let policies = self
            .policies
            .read()
            .map_err(|_| BrokerError::LockPoisoned)?;
        policies
            .get(&self.default_policy_version)
            .cloned()
            .ok_or(BrokerError::LockPoisoned)
    }

    /// Installs an immutable explicit-user policy and returns its deterministic version.
    pub fn install_policy(&self, snapshot: PolicySnapshot) -> Result<PolicyVersion, BrokerError> {
        let version = snapshot.version();
        let mut policies = self
            .policies
            .write()
            .map_err(|_| BrokerError::LockPoisoned)?;
        if let Some(existing) = policies.get(&version) {
            if existing != &snapshot {
                return Err(BrokerError::InvalidRule);
            }
        } else {
            policies.insert(version.clone(), snapshot);
        }
        Ok(version)
    }

    /// Evaluates, audits, and when allowed mints one expiring capability.
    pub fn evaluate(
        &self,
        request: PermissionRequest,
        issued_at: u64,
    ) -> Result<DecisionOutcome, BrokerError> {
        let planned = self.plan(&request)?;
        match planned {
            PlannedDecision::Deny(reason) => {
                let fingerprint =
                    decision_fingerprint(&request, Decision::Deny, Some(reason), None);
                let mut connection = self
                    .connection
                    .lock()
                    .map_err(|_| BrokerError::LockPoisoned)?;
                let transaction = connection.transaction()?;
                insert_request_audit(
                    &transaction,
                    &request,
                    None,
                    Decision::Deny,
                    Some(reason),
                    None,
                    0,
                    issued_at,
                )?;
                transaction.commit()?;
                Ok(DecisionOutcome {
                    receipt: DecisionReceipt {
                        request,
                        fingerprint,
                        grant_id: None,
                    },
                    capability: None,
                })
            }
            PlannedDecision::Allow(rule) => self.issue(request, issued_at, &rule),
        }
    }

    /// Recomputes from the receipt's pinned version and appends an audit row for the replay decision.
    pub fn replay(
        &self,
        receipt: &DecisionReceipt,
        replayed_at: u64,
    ) -> Result<DecisionFingerprint, BrokerError> {
        let planned = self.plan(&receipt.request)?;
        let (decision, reason, rule) = match planned {
            PlannedDecision::Deny(reason) => (Decision::Deny, Some(reason), None),
            PlannedDecision::Allow(rule) => (Decision::Allow, None, Some(rule)),
        };
        let fingerprint = decision_fingerprint(&receipt.request, decision, reason, rule.as_deref());
        let (payload_digest, byte_count) = rule.as_ref().map_or((None, 0), |allowed| {
            (
                Some(allowed.payload_digest.as_str()),
                range_byte_count(&allowed.minimal_ranges).unwrap_or(0),
            )
        });
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let transaction = connection.transaction()?;
        insert_request_audit(
            &transaction,
            &receipt.request,
            receipt.grant_id.as_deref(),
            decision,
            reason,
            payload_digest,
            byte_count,
            replayed_at,
        )?;
        transaction.commit()?;
        Ok(fingerprint)
    }

    /// Checks actual payload bytes and exact scope, atomically consumes the token, audits, then calls the tool.
    pub fn execute<T>(
        &self,
        capability: &CapabilityToken,
        call: RuntimeToolCall<'_>,
        now: u64,
        tool: impl FnOnce(AuthorizedToolCall<'_>) -> T,
    ) -> Result<T, BrokerError> {
        let call_digest = ContentDigest::of(call.payload);
        let call_count =
            u64::try_from(call.payload.len()).map_err(|_| BrokerError::TimeOutOfRange)?;
        let scope_matches = capability.actor_process_class == call.actor_process_class
            && capability.operation == call.operation
            && capability.purpose_id == call.purpose_id
            && capability.destination_id == call.destination_id
            && capability.ranges == call.ranges
            && capability.payload_digest == call_digest
            && range_byte_count(&call.ranges) == Some(call_count);

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let transaction = connection.transaction()?;
        if !scope_matches {
            insert_runtime_audit(
                &transaction,
                capability,
                Decision::Deny,
                Some(ReasonCode::ScopeMismatch),
                Some(call_digest.as_str()),
                call_count,
                now,
            )?;
            transaction.commit()?;
            return Err(BrokerError::Denied(ReasonCode::ScopeMismatch));
        }

        let stored = load_runtime_grant(&transaction, &capability.grant_id)?;
        let Some(stored) = stored else {
            insert_runtime_audit(
                &transaction,
                capability,
                Decision::Deny,
                Some(ReasonCode::NoGrant),
                Some(call_digest.as_str()),
                call_count,
                now,
            )?;
            transaction.commit()?;
            return Err(BrokerError::Denied(ReasonCode::NoGrant));
        };
        if !stored.matches_capability(capability) {
            insert_runtime_audit(
                &transaction,
                capability,
                Decision::Deny,
                Some(ReasonCode::ScopeMismatch),
                Some(call_digest.as_str()),
                call_count,
                now,
            )?;
            transaction.commit()?;
            return Err(BrokerError::Denied(ReasonCode::ScopeMismatch));
        }
        if stored.consumed_at.is_some() {
            insert_runtime_audit(
                &transaction,
                capability,
                Decision::Deny,
                Some(ReasonCode::GrantConsumed),
                Some(call_digest.as_str()),
                call_count,
                now,
            )?;
            transaction.commit()?;
            return Err(BrokerError::Denied(ReasonCode::GrantConsumed));
        }
        if now >= stored.expires_at {
            insert_runtime_audit(
                &transaction,
                capability,
                Decision::Deny,
                Some(ReasonCode::GrantExpired),
                Some(call_digest.as_str()),
                call_count,
                now,
            )?;
            transaction.commit()?;
            return Err(BrokerError::Denied(ReasonCode::GrantExpired));
        }
        let consumed = transaction.execute(
            "UPDATE egress_grant SET consumed_at = ?1 WHERE grant_id = ?2 AND consumed_at IS NULL AND expires_at > ?1",
            params![sqlite_u64(now)?, capability.grant_id],
        )?;
        if consumed != 1 {
            insert_runtime_audit(
                &transaction,
                capability,
                Decision::Deny,
                Some(ReasonCode::GrantConsumed),
                Some(call_digest.as_str()),
                call_count,
                now,
            )?;
            transaction.commit()?;
            return Err(BrokerError::Denied(ReasonCode::GrantConsumed));
        }
        insert_runtime_audit(
            &transaction,
            capability,
            Decision::Allow,
            None,
            Some(call_digest.as_str()),
            call_count,
            now,
        )?;
        transaction.commit()?;
        drop(connection);
        Ok(tool(AuthorizedToolCall {
            payload: call.payload,
            ranges: &call.ranges,
        }))
    }

    /// Reads all audit metadata in append order.
    pub fn audit_rows(&self) -> Result<Vec<AuditRow>, BrokerError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let mut statement = connection.prepare(concat!(
            "SELECT audit_seq, grant_id, decision, reason_code, actor_process_class, ",
            "payload_digest, byte_count, destination_id, started_at, finished_at, ",
            "provider_response_digest, deletion_receipt_id FROM egress_audit ORDER BY audit_seq"
        ))?;
        let rows = statement
            .query_map([], |row| {
                let decision: String = row.get(2)?;
                let reason: Option<String> = row.get(3)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    decision,
                    reason,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(
                |(
                    audit_seq,
                    grant_id,
                    decision,
                    reason,
                    actor_process_class,
                    payload_digest,
                    byte_count,
                    destination_id,
                    started_at,
                    finished_at,
                    provider_response_digest,
                    deletion_receipt_id,
                )| {
                    Ok(AuditRow {
                        audit_seq: nonnegative(audit_seq)?,
                        grant_id,
                        decision: match decision.as_str() {
                            "ALLOW" => Decision::Allow,
                            "DENY" => Decision::Deny,
                            _ => return Err(BrokerError::CorruptAuditReason),
                        },
                        reason_code: reason.as_deref().map(ReasonCode::parse).transpose()?,
                        actor_process_class,
                        payload_digest,
                        byte_count: nonnegative(byte_count)?,
                        destination_id,
                        started_at: nonnegative(started_at)?,
                        finished_at: nonnegative(finished_at)?,
                        provider_response_digest,
                        deletion_receipt_id,
                    })
                },
            )
            .collect()
    }

    /// Reads one grant row for audit/testing without exposing a capability constructor.
    pub fn grant_row(&self, grant_id: &str) -> Result<Option<GrantRow>, BrokerError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        connection
            .query_row(
                concat!(
                    "SELECT grant_id, request_digest, payload_digest, byte_ranges_canonical, ",
                    "purpose_id, provider_id, provider_policy_snapshot_digest, retention_terms_hash, ",
                    "training_use_allowed, redaction_policy_hash, issued_at, expires_at, max_uses, ",
                    "consumed_at, consent_event_id FROM egress_grant WHERE grant_id = ?1"
                ),
                [grant_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, bool>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, u8>(12)?,
                        row.get::<_, Option<i64>>(13)?,
                        row.get::<_, String>(14)?,
                    ))
                },
            )
            .optional()?
            .map(
                |(
                    grant_id,
                    request_digest,
                    payload_digest,
                    byte_ranges_canonical,
                    purpose_id,
                    provider_id,
                    provider_policy_snapshot_digest,
                    retention_terms_hash,
                    training_use_allowed,
                    redaction_policy_hash,
                    issued_at,
                    expires_at,
                    max_uses,
                    consumed_at,
                    consent_event_id,
                )| {
                    Ok(GrantRow {
                        grant_id,
                        request_digest,
                        payload_digest,
                        byte_ranges_canonical,
                        purpose_id,
                        provider_id,
                        provider_policy_snapshot_digest,
                        retention_terms_hash,
                        training_use_allowed,
                        redaction_policy_hash,
                        issued_at: nonnegative(issued_at)?,
                        expires_at: nonnegative(expires_at)?,
                        max_uses,
                        consumed_at: consumed_at.map(nonnegative).transpose()?,
                        consent_event_id,
                    })
                },
            )
            .transpose()
    }

    fn plan(&self, request: &PermissionRequest) -> Result<PlannedDecision, BrokerError> {
        let complete = match CompleteRequest::resolve(request) {
            Ok(complete) => complete,
            Err(reason) => return Ok(PlannedDecision::Deny(reason)),
        };
        let policies = self
            .policies
            .read()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let Some(policy) = policies.get(complete.policy_version) else {
            return Ok(PlannedDecision::Deny(ReasonCode::PolicyStale));
        };
        let matching = policy
            .rules
            .iter()
            .filter(|rule| rule.matches_tuple(&complete))
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return Ok(PlannedDecision::Deny(ReasonCode::NoGrant));
        }
        let mut satisfying = matching
            .into_iter()
            .filter(|rule| rule.requested_scope_contains_minimum(complete.ranges))
            .cloned()
            .collect::<Vec<_>>();
        if satisfying.is_empty() {
            return Ok(PlannedDecision::Deny(ReasonCode::ScopeMismatch));
        }
        satisfying.sort_by(|left, right| {
            range_byte_count(&left.minimal_ranges)
                .cmp(&range_byte_count(&right.minimal_ranges))
                .then_with(|| left.canonical_bytes().cmp(&right.canonical_bytes()))
        });
        Ok(PlannedDecision::Allow(Box::new(satisfying.remove(0))))
    }

    fn issue(
        &self,
        request: PermissionRequest,
        issued_at: u64,
        rule: &EgressRule,
    ) -> Result<DecisionOutcome, BrokerError> {
        let expires_at = issued_at
            .checked_add(self.grant_ttl_millis)
            .ok_or(BrokerError::TimeOutOfRange)?;
        let request_digest = request_digest(&request);
        let sequence = self.next_grant.fetch_add(1, Ordering::Relaxed);
        let mut grant_material = b"academic-egress-grant-v1\0".to_vec();
        push_string(&mut grant_material, &request_digest);
        grant_material.extend_from_slice(&issued_at.to_be_bytes());
        grant_material.extend_from_slice(&sequence.to_be_bytes());
        let grant_id = lower_hex(&Sha256::digest(grant_material));
        let ranges = canonical_ranges(&rule.minimal_ranges).ok_or(BrokerError::InvalidRule)?;
        let byte_count = range_byte_count(&rule.minimal_ranges).ok_or(BrokerError::InvalidRule)?;
        let complete = CompleteRequest::resolve(&request).map_err(BrokerError::Denied)?;

        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            concat!(
                "INSERT INTO egress_grant (grant_id, request_digest, payload_digest, ",
                "byte_ranges_canonical, purpose_id, provider_id, provider_policy_snapshot_digest, ",
                "retention_terms_hash, training_use_allowed, redaction_policy_hash, issued_at, ",
                "expires_at, max_uses, consumed_at, consent_event_id) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, NULL, ?13)"
            ),
            params![
                grant_id,
                request_digest,
                rule.payload_digest.as_str(),
                ranges,
                complete.purpose_id,
                complete.destination_id,
                rule.provider_policy_snapshot_digest.as_str(),
                complete.retention_terms_hash.as_str(),
                rule.training_use_allowed,
                rule.redaction_policy_hash.as_str(),
                sqlite_u64(issued_at)?,
                sqlite_u64(expires_at)?,
                complete.consent_evidence_id,
            ],
        )?;
        insert_request_audit(
            &transaction,
            &request,
            Some(&grant_id),
            Decision::Allow,
            None,
            Some(rule.payload_digest.as_str()),
            byte_count,
            issued_at,
        )?;
        transaction.commit()?;
        drop(connection);

        let fingerprint = decision_fingerprint(&request, Decision::Allow, None, Some(rule));
        Ok(DecisionOutcome {
            receipt: DecisionReceipt {
                request,
                fingerprint,
                grant_id: Some(grant_id.clone()),
            },
            capability: Some(CapabilityToken {
                grant_id,
                actor_process_class: rule.actor_process_class.clone(),
                operation: rule.operation.clone(),
                purpose_id: rule.purpose_id.clone(),
                destination_id: rule.destination_id.clone(),
                ranges: rule.minimal_ranges.clone(),
                payload_digest: rule.payload_digest.clone(),
            }),
        })
    }
}

#[derive(Debug)]
enum PlannedDecision {
    Deny(ReasonCode),
    Allow(Box<EgressRule>),
}

#[derive(Debug)]
struct CompleteRequest<'a> {
    actor_process_class: &'a str,
    data_class: &'a str,
    ranges: &'a [ObjectRange],
    operation: &'a str,
    purpose_id: &'a str,
    destination_id: &'a str,
    retention_terms_hash: &'a ContentDigest,
    requested_at: u64,
    consent_evidence_id: &'a str,
    policy_version: &'a PolicyVersion,
}

impl<'a> CompleteRequest<'a> {
    fn resolve(request: &'a PermissionRequest) -> Result<Self, ReasonCode> {
        let actor_process_class = nonempty(request.actor_process_class.as_deref())?;
        let data_class = nonempty(request.data_class.as_deref())?;
        let ranges = request
            .object_range_digest_set
            .as_deref()
            .filter(|ranges| canonical_ranges(ranges).is_some())
            .ok_or(ReasonCode::NoGrant)?;
        let operation = nonempty(request.operation.as_deref())?;
        let purpose_id = nonempty(request.purpose_id.as_deref())?;
        let destination_id = nonempty(request.destination_id.as_deref())?;
        let retention_terms_hash = request
            .retention_terms_hash
            .as_ref()
            .ok_or(ReasonCode::NoGrant)?;
        let requested_at = request.requested_at.ok_or(ReasonCode::NoGrant)?;
        let consent_evidence_id = nonempty(request.consent_evidence_id.as_deref())?;
        let policy_version = request.policy_version.as_ref().ok_or(ReasonCode::NoGrant)?;
        Ok(Self {
            actor_process_class,
            data_class,
            ranges,
            operation,
            purpose_id,
            destination_id,
            retention_terms_hash,
            requested_at,
            consent_evidence_id,
            policy_version,
        })
    }
}

#[derive(Debug)]
struct RuntimeGrant {
    payload_digest: String,
    byte_ranges_canonical: String,
    purpose_id: String,
    provider_id: String,
    expires_at: u64,
    consumed_at: Option<u64>,
}

impl RuntimeGrant {
    fn matches_capability(&self, capability: &CapabilityToken) -> bool {
        self.payload_digest == capability.payload_digest.as_str()
            && canonical_ranges(&capability.ranges).as_deref()
                == Some(self.byte_ranges_canonical.as_str())
            && self.purpose_id == capability.purpose_id
            && self.provider_id == capability.destination_id
    }
}

fn load_runtime_grant(
    transaction: &Transaction<'_>,
    grant_id: &str,
) -> Result<Option<RuntimeGrant>, BrokerError> {
    transaction
        .query_row(
            concat!(
                "SELECT payload_digest, byte_ranges_canonical, purpose_id, provider_id, ",
                "expires_at, consumed_at FROM egress_grant WHERE grant_id = ?1"
            ),
            [grant_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()?
        .map(
            |(payload_digest, ranges, purpose_id, provider_id, expires_at, consumed_at)| {
                Ok(RuntimeGrant {
                    payload_digest,
                    byte_ranges_canonical: ranges,
                    purpose_id,
                    provider_id,
                    expires_at: nonnegative(expires_at)?,
                    consumed_at: consumed_at.map(nonnegative).transpose()?,
                })
            },
        )
        .transpose()
}

#[expect(
    clippy::too_many_arguments,
    reason = "the arguments map a request into the fixed audit row"
)]
fn insert_request_audit(
    transaction: &Transaction<'_>,
    request: &PermissionRequest,
    grant_id: Option<&str>,
    decision: Decision,
    reason: Option<ReasonCode>,
    payload_digest: Option<&str>,
    byte_count: u64,
    at: u64,
) -> Result<(), BrokerError> {
    insert_audit(
        transaction,
        grant_id,
        decision,
        reason,
        request
            .actor_process_class
            .as_deref()
            .unwrap_or(MISSING_VALUE),
        payload_digest,
        byte_count,
        request.destination_id.as_deref().unwrap_or(MISSING_VALUE),
        at,
    )
}

fn insert_runtime_audit(
    transaction: &Transaction<'_>,
    capability: &CapabilityToken,
    decision: Decision,
    reason: Option<ReasonCode>,
    payload_digest: Option<&str>,
    byte_count: u64,
    at: u64,
) -> Result<(), BrokerError> {
    insert_audit(
        transaction,
        Some(&capability.grant_id),
        decision,
        reason,
        &capability.actor_process_class,
        payload_digest,
        byte_count,
        &capability.destination_id,
        at,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the arguments mirror the fixed audit row"
)]
fn insert_audit(
    transaction: &Transaction<'_>,
    grant_id: Option<&str>,
    decision: Decision,
    reason: Option<ReasonCode>,
    actor_process_class: &str,
    payload_digest: Option<&str>,
    byte_count: u64,
    destination_id: &str,
    at: u64,
) -> Result<(), BrokerError> {
    transaction.execute(
        concat!(
            "INSERT INTO egress_audit (grant_id, decision, reason_code, actor_process_class, ",
            "payload_digest, byte_count, destination_id, started_at, finished_at, ",
            "provider_response_digest, deletion_receipt_id) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, NULL, NULL)"
        ),
        params![
            grant_id,
            decision.as_str(),
            reason.map(ReasonCode::as_str),
            actor_process_class,
            payload_digest,
            sqlite_u64(byte_count)?,
            destination_id,
            sqlite_u64(at)?,
        ],
    )?;
    Ok(())
}

fn decision_fingerprint(
    request: &PermissionRequest,
    decision: Decision,
    reason: Option<ReasonCode>,
    rule: Option<&EgressRule>,
) -> DecisionFingerprint {
    DecisionFingerprint {
        request_digest: request_digest(request),
        policy_version: request
            .policy_version
            .as_ref()
            .map(|version| version.as_str().to_owned()),
        decision,
        reason_code: reason,
        byte_ranges_canonical: rule.and_then(|allowed| canonical_ranges(&allowed.minimal_ranges)),
        payload_digest: rule.map(|allowed| allowed.payload_digest.as_str().to_owned()),
    }
}

fn request_digest(request: &PermissionRequest) -> String {
    let mut bytes = b"academic-permission-request-v1\0".to_vec();
    push_optional_string(&mut bytes, request.actor_process_class.as_deref());
    push_optional_string(&mut bytes, request.data_class.as_deref());
    match &request.object_range_digest_set {
        Some(ranges) => {
            bytes.push(1);
            push_ranges(&mut bytes, ranges);
        }
        None => bytes.push(0),
    }
    push_optional_string(&mut bytes, request.operation.as_deref());
    push_optional_string(&mut bytes, request.purpose_id.as_deref());
    push_optional_string(&mut bytes, request.destination_id.as_deref());
    push_optional_string(
        &mut bytes,
        request
            .retention_terms_hash
            .as_ref()
            .map(ContentDigest::as_str),
    );
    match request.requested_at {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
    push_optional_string(&mut bytes, request.consent_evidence_id.as_deref());
    push_optional_string(
        &mut bytes,
        request.policy_version.as_ref().map(PolicyVersion::as_str),
    );
    lower_hex(&Sha256::digest(bytes))
}

fn canonical_ranges(ranges: &[ObjectRange]) -> Option<String> {
    if ranges.is_empty() {
        return None;
    }
    let mut ordered = ranges.to_vec();
    ordered.sort();
    for window in ordered.windows(2) {
        if window[0].object_id == window[1].object_id && window[0].end > window[1].start {
            return None;
        }
    }
    Some(
        ordered
            .iter()
            .map(|range| {
                format!(
                    "{}:{}-{}@{}",
                    range.object_id,
                    range.start,
                    range.end,
                    range.content_digest.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn range_byte_count(ranges: &[ObjectRange]) -> Option<u64> {
    ranges
        .iter()
        .try_fold(0_u64, |total, range| total.checked_add(range.byte_count()))
}

fn nonempty(value: Option<&str>) -> Result<&str, ReasonCode> {
    value
        .filter(|candidate| !candidate.is_empty())
        .ok_or(ReasonCode::NoGrant)
}

fn sqlite_u64(value: u64) -> Result<i64, BrokerError> {
    i64::try_from(value).map_err(|_| BrokerError::TimeOutOfRange)
}

fn nonnegative(value: i64) -> Result<u64, BrokerError> {
    u64::try_from(value).map_err(|_| BrokerError::CorruptAuditReason)
}

fn push_optional_string(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            bytes.push(1);
            push_string(bytes, value);
        }
        None => bytes.push(0),
    }
}

fn push_ranges(bytes: &mut Vec<u8>, ranges: &[ObjectRange]) {
    let mut ordered = ranges.to_vec();
    ordered.sort();
    bytes.extend_from_slice(
        &u64::try_from(ordered.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for range in ordered {
        push_string(bytes, &range.object_id);
        bytes.extend_from_slice(&range.start.to_be_bytes());
        bytes.extend_from_slice(&range.end.to_be_bytes());
        push_string(bytes, range.content_digest.as_str());
    }
}

fn push_string(bytes: &mut Vec<u8>, value: &str) {
    push_bytes(bytes, value.as_bytes());
}

fn push_bytes(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    bytes.extend_from_slice(value);
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
