use rusqlite::{Connection, OptionalExtension as _, Row, Transaction, params};

use super::{BrokerError, ContentDigest, PermissionBroker, ReasonCode, push_string, sqlite_u64};

const MAX_SQLITE_SEQUENCE: u64 = 9_223_372_036_854_775_807;

/// The provider surface is part of provider identity, not descriptive metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSurface {
    /// Contracted enterprise or API access.
    EnterpriseApi,
    /// Consumer-facing chat or application access.
    ConsumerUi,
}

impl ProviderSurface {
    const fn as_str(self) -> &'static str {
        match self {
            Self::EnterpriseApi => "ENTERPRISE_API",
            Self::ConsumerUi => "CONSUMER_UI",
        }
    }

    fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "ENTERPRISE_API" => Ok(Self::EnterpriseApi),
            "CONSUMER_UI" => Ok(Self::ConsumerUi),
            _ => Err(BrokerError::CorruptProviderRecord),
        }
    }
}

/// Stable provider identity: the ordered `(vendor_id, surface)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderIdentity {
    vendor_id: String,
    surface: ProviderSurface,
}

impl ProviderIdentity {
    /// Creates a provider identity without collapsing two surfaces from one vendor.
    pub fn new(
        vendor_id: impl Into<String>,
        surface: ProviderSurface,
    ) -> Result<Self, BrokerError> {
        let vendor_id = vendor_id.into();
        if !valid_text(&vendor_id) {
            return Err(BrokerError::InvalidProviderPolicy);
        }
        Ok(Self { vendor_id, surface })
    }

    /// Stable vendor identifier.
    #[must_use]
    pub fn vendor_id(&self) -> &str {
        &self.vendor_id
    }

    /// Contract surface participating in identity.
    #[must_use]
    pub const fn surface(&self) -> ProviderSurface {
        self.surface
    }

    /// Canonical broker destination derived from both identity components.
    #[must_use]
    pub fn destination_id(&self) -> String {
        let mut bytes = b"academic-provider-identity-v1\0".to_vec();
        push_string(&mut bytes, &self.vendor_id);
        push_string(&mut bytes, self.surface.as_str());
        format!("provider:{}", ContentDigest::of(&bytes).as_str())
    }
}

/// Registration input. Every field is optional so omission can fail closed at the boundary.
#[derive(Debug, Clone, Default)]
pub struct ProviderPolicyDraft {
    /// Provider identity, including its surface.
    pub identity: Option<ProviderIdentity>,
    /// Whether provider-side training is enabled by contract.
    pub training_use_enabled: Option<bool>,
    /// Whether the applicable contract records an effective training opt-out.
    pub training_opt_out_applied: Option<bool>,
    /// Maximum server retention in milliseconds; zero is a valid declared fact.
    pub server_retention_millis: Option<u64>,
    /// Whether abuse monitoring may retain/log submitted content.
    pub abuse_logging_enabled: Option<bool>,
    /// Every region in which the provider may process or retain the input.
    pub residency_regions: Option<Vec<String>>,
    /// Declared subprocessors; an explicitly empty list is valid.
    pub subprocessors: Option<Vec<String>>,
    /// Whether encryption in transit is declared for this surface.
    pub transit_encryption_declared: Option<bool>,
    /// Whether encryption at rest is declared for this surface.
    pub at_rest_encryption_declared: Option<bool>,
    /// Whether the surface exposes a deletion API.
    pub deletion_api_available: Option<bool>,
    /// Whether a deletion request can yield a receipt.
    pub deletion_receipt_capable: Option<bool>,
    /// Largest input accepted by this surface.
    pub maximum_input_bytes: Option<u64>,
    /// Exact provider logging configuration identifier.
    pub logging_configuration: Option<String>,
    /// Digest of the policy source that was reviewed.
    pub policy_source_digest: Option<ContentDigest>,
    /// Last time the source policy was verified.
    pub last_verified_at: Option<u64>,
    /// Explicit freshness lifetime. There is deliberately no default.
    pub ttl_millis: Option<u64>,
}

/// One immutable version of provider policy facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPolicySnapshot {
    snapshot_digest: ContentDigest,
    identity: ProviderIdentity,
    destination_id: String,
    training_use_enabled: bool,
    training_opt_out_applied: bool,
    server_retention_millis: u64,
    abuse_logging_enabled: bool,
    residency_regions: Vec<String>,
    subprocessors: Vec<String>,
    transit_encryption_declared: bool,
    at_rest_encryption_declared: bool,
    deletion_api_available: bool,
    deletion_receipt_capable: bool,
    maximum_input_bytes: u64,
    logging_configuration: String,
    policy_source_digest: ContentDigest,
    last_verified_at: u64,
    ttl_millis: u64,
    registered_at: u64,
}

impl ProviderPolicySnapshot {
    fn from_draft(draft: ProviderPolicyDraft) -> Result<Self, BrokerError> {
        let identity = required(draft.identity, "identity")?;
        let training_use_enabled = required(draft.training_use_enabled, "training_use_enabled")?;
        let training_opt_out_applied =
            required(draft.training_opt_out_applied, "training_opt_out_applied")?;
        let server_retention_millis =
            required(draft.server_retention_millis, "server_retention_millis")?;
        let abuse_logging_enabled = required(draft.abuse_logging_enabled, "abuse_logging_enabled")?;
        let residency_regions =
            canonical_set(required(draft.residency_regions, "residency_regions")?)?;
        let subprocessors = canonical_set(required(draft.subprocessors, "subprocessors")?)?;
        let transit_encryption_declared = required(
            draft.transit_encryption_declared,
            "transit_encryption_declared",
        )?;
        let at_rest_encryption_declared = required(
            draft.at_rest_encryption_declared,
            "at_rest_encryption_declared",
        )?;
        let deletion_api_available =
            required(draft.deletion_api_available, "deletion_api_available")?;
        let deletion_receipt_capable =
            required(draft.deletion_receipt_capable, "deletion_receipt_capable")?;
        let maximum_input_bytes = required(draft.maximum_input_bytes, "maximum_input_bytes")?;
        let logging_configuration = required(draft.logging_configuration, "logging_configuration")?;
        let policy_source_digest = required(draft.policy_source_digest, "policy_source_digest")?;
        let last_verified_at = required(draft.last_verified_at, "last_verified_at")?;
        let ttl_millis = required(draft.ttl_millis, "ttl_millis")?;

        if residency_regions.is_empty()
            || maximum_input_bytes == 0
            || !valid_text(&logging_configuration)
            || ttl_millis == 0
            || last_verified_at.checked_add(ttl_millis).is_none()
            || (deletion_receipt_capable && !deletion_api_available)
        {
            return Err(BrokerError::InvalidProviderPolicy);
        }

        let destination_id = identity.destination_id();
        let mut snapshot = Self {
            snapshot_digest: ContentDigest::of(b"uninitialized-provider-policy"),
            identity,
            destination_id,
            training_use_enabled,
            training_opt_out_applied,
            server_retention_millis,
            abuse_logging_enabled,
            residency_regions,
            subprocessors,
            transit_encryption_declared,
            at_rest_encryption_declared,
            deletion_api_available,
            deletion_receipt_capable,
            maximum_input_bytes,
            logging_configuration,
            policy_source_digest,
            last_verified_at,
            ttl_millis,
            registered_at: 0,
        };
        snapshot.snapshot_digest = ContentDigest::of(&snapshot.canonical_bytes());
        Ok(snapshot)
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = b"academic-provider-policy-snapshot-v1\0".to_vec();
        push_string(&mut bytes, self.identity.vendor_id());
        push_string(&mut bytes, self.identity.surface().as_str());
        bytes.push(u8::from(self.training_use_enabled));
        bytes.push(u8::from(self.training_opt_out_applied));
        bytes.extend_from_slice(&self.server_retention_millis.to_be_bytes());
        bytes.push(u8::from(self.abuse_logging_enabled));
        push_set(&mut bytes, &self.residency_regions);
        push_set(&mut bytes, &self.subprocessors);
        bytes.push(u8::from(self.transit_encryption_declared));
        bytes.push(u8::from(self.at_rest_encryption_declared));
        bytes.push(u8::from(self.deletion_api_available));
        bytes.push(u8::from(self.deletion_receipt_capable));
        bytes.extend_from_slice(&self.maximum_input_bytes.to_be_bytes());
        push_string(&mut bytes, &self.logging_configuration);
        push_string(&mut bytes, self.policy_source_digest.as_str());
        bytes.extend_from_slice(&self.last_verified_at.to_be_bytes());
        bytes.extend_from_slice(&self.ttl_millis.to_be_bytes());
        bytes
    }

    /// Digest of the canonical provider-policy snapshot encoding.
    #[must_use]
    pub const fn snapshot_digest(&self) -> &ContentDigest {
        &self.snapshot_digest
    }

    /// Identity tuple represented by this version.
    #[must_use]
    pub const fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    /// Exact destination string used by permission rules and grants.
    #[must_use]
    pub fn destination_id(&self) -> &str {
        &self.destination_id
    }

    /// Whether provider-side training is contractually enabled.
    #[must_use]
    pub const fn training_use_enabled(&self) -> bool {
        self.training_use_enabled
    }

    /// Whether the reviewed contract records an effective opt-out.
    #[must_use]
    pub const fn training_opt_out_applied(&self) -> bool {
        self.training_opt_out_applied
    }

    /// Declared maximum server retention.
    #[must_use]
    pub const fn server_retention_millis(&self) -> u64 {
        self.server_retention_millis
    }

    /// Whether abuse logging is enabled.
    #[must_use]
    pub const fn abuse_logging_enabled(&self) -> bool {
        self.abuse_logging_enabled
    }

    /// Canonically sorted residency regions.
    #[must_use]
    pub fn residency_regions(&self) -> &[String] {
        &self.residency_regions
    }

    /// Canonically sorted subprocessors.
    #[must_use]
    pub fn subprocessors(&self) -> &[String] {
        &self.subprocessors
    }

    /// Whether encryption in transit is declared.
    #[must_use]
    pub const fn transit_encryption_declared(&self) -> bool {
        self.transit_encryption_declared
    }

    /// Whether encryption at rest is declared.
    #[must_use]
    pub const fn at_rest_encryption_declared(&self) -> bool {
        self.at_rest_encryption_declared
    }

    /// Whether a deletion API is available.
    #[must_use]
    pub const fn deletion_api_available(&self) -> bool {
        self.deletion_api_available
    }

    /// Whether the deletion API yields a receipt.
    #[must_use]
    pub const fn deletion_receipt_capable(&self) -> bool {
        self.deletion_receipt_capable
    }

    /// Provider-declared maximum input size.
    #[must_use]
    pub const fn maximum_input_bytes(&self) -> u64 {
        self.maximum_input_bytes
    }

    /// Exact logging configuration identifier.
    #[must_use]
    pub fn logging_configuration(&self) -> &str {
        &self.logging_configuration
    }

    /// Digest of the source policy reviewed for this snapshot.
    #[must_use]
    pub const fn policy_source_digest(&self) -> &ContentDigest {
        &self.policy_source_digest
    }

    /// Last verification time.
    #[must_use]
    pub const fn last_verified_at(&self) -> u64 {
        self.last_verified_at
    }

    /// Explicit provider-policy TTL.
    #[must_use]
    pub const fn ttl_millis(&self) -> u64 {
        self.ttl_millis
    }

    /// Exclusive freshness boundary.
    #[must_use]
    pub fn verified_until(&self) -> u64 {
        self.last_verified_at.saturating_add(self.ttl_millis)
    }

    /// Broker retention hash that an egress rule must pin.
    #[must_use]
    pub fn retention_terms_hash(&self) -> ContentDigest {
        let mut bytes = b"academic-provider-retention-terms-v1\0".to_vec();
        bytes.extend_from_slice(&self.server_retention_millis.to_be_bytes());
        bytes.push(u8::from(self.abuse_logging_enabled));
        push_string(&mut bytes, &self.logging_configuration);
        ContentDigest::of(&bytes)
    }

    fn effective_training_use(&self) -> bool {
        self.training_use_enabled && !self.training_opt_out_applied
    }
}

/// Explicit user constraints stored by the registry. No row is created by default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderUserPolicy {
    /// Stable decision identifier.
    pub policy_id: String,
    /// Exact provider surface this decision concerns.
    pub provider_identity: ProviderIdentity,
    /// Exact provider-policy version reviewed by the user.
    pub provider_policy_snapshot_digest: ContentDigest,
    /// Regions explicitly accepted for this decision.
    pub allowed_residency_regions: Vec<String>,
    /// Explicit handling when the provider has no deletion API.
    pub allow_without_deletion_api: bool,
    /// Whether the user requires a transit-encryption declaration.
    pub require_transit_encryption: bool,
    /// Whether the user requires an at-rest-encryption declaration.
    pub require_at_rest_encryption: bool,
    /// Evidence for the user-authored decision.
    pub decision_evidence_id: String,
    /// Decision recording time.
    pub recorded_at: u64,
}

/// Provider deletion receipt metadata. Receipt content is represented only by a digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionReceiptDraft {
    /// Provider-issued receipt identifier.
    pub receipt_id: String,
    /// Grant whose transmission is being deleted.
    pub grant_id: String,
    /// Allow audit row for the exact transmission.
    pub egress_audit_seq: u64,
    /// Digest of the provider receipt bytes.
    pub provider_receipt_digest: ContentDigest,
    /// Deletion request time.
    pub requested_at: u64,
    /// Receipt observation time.
    pub received_at: u64,
}

/// Persisted immutable deletion-receipt row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionReceiptRow {
    /// Append order.
    pub receipt_seq: u64,
    /// Provider-issued receipt identifier.
    pub receipt_id: String,
    /// Linked egress grant.
    pub grant_id: String,
    /// Linked allow audit row.
    pub egress_audit_seq: u64,
    /// Provider-policy version under which the transmission occurred.
    pub provider_policy_snapshot_digest: ContentDigest,
    /// Digest of the provider receipt bytes.
    pub provider_receipt_digest: ContentDigest,
    /// Deletion request time.
    pub requested_at: u64,
    /// Receipt observation time.
    pub received_at: u64,
}

impl PermissionBroker {
    /// Registers one immutable provider-policy version without performing network I/O.
    pub fn register_provider_policy(
        &self,
        draft: ProviderPolicyDraft,
        registered_at: u64,
    ) -> Result<ProviderPolicySnapshot, BrokerError> {
        let mut snapshot = ProviderPolicySnapshot::from_draft(draft)?;
        if registered_at < snapshot.last_verified_at {
            return Err(BrokerError::NonMonotonicProviderRegistration);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        if let Some(existing) =
            load_provider_snapshot(&connection, snapshot.snapshot_digest.as_str())?
        {
            return Ok(existing);
        }
        let latest_registered_at = connection
            .query_row(
                "SELECT registered_at FROM provider_policy_snapshot WHERE destination_id = ?1 ORDER BY registered_at DESC, snapshot_seq DESC LIMIT 1",
                [snapshot.destination_id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(provider_u64)
            .transpose()?;
        if latest_registered_at.is_some_and(|latest| registered_at <= latest) {
            return Err(BrokerError::NonMonotonicProviderRegistration);
        }
        snapshot.registered_at = registered_at;
        let transaction = connection.transaction()?;
        insert_provider_snapshot(&transaction, &snapshot)?;
        transaction.commit()?;
        Ok(snapshot)
    }

    /// Returns the provider-policy version in force for an identity at a time.
    pub fn current_provider_policy(
        &self,
        identity: &ProviderIdentity,
        at: u64,
    ) -> Result<Option<ProviderPolicySnapshot>, BrokerError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        load_current_provider_snapshot(&connection, &identity.destination_id(), at)
    }

    /// Returns every immutable version for an exact provider identity.
    pub fn provider_policy_versions(
        &self,
        identity: &ProviderIdentity,
    ) -> Result<Vec<ProviderPolicySnapshot>, BrokerError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let mut statement = connection.prepare(
            "SELECT snapshot_digest FROM provider_policy_snapshot WHERE destination_id = ?1 ORDER BY snapshot_seq",
        )?;
        let digests = statement
            .query_map([identity.destination_id()], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        digests
            .into_iter()
            .map(|digest| {
                load_provider_snapshot(&connection, &digest)?
                    .ok_or(BrokerError::CorruptProviderRecord)
            })
            .collect()
    }

    /// Appends an explicit user policy for one exact provider-policy version.
    pub fn record_provider_user_policy(
        &self,
        mut policy: ProviderUserPolicy,
    ) -> Result<(), BrokerError> {
        if !valid_text(&policy.policy_id) || !valid_text(&policy.decision_evidence_id) {
            return Err(BrokerError::InvalidProviderUserPolicy);
        }
        policy.allowed_residency_regions = canonical_set(policy.allowed_residency_regions)?;
        if policy.allowed_residency_regions.is_empty() {
            return Err(BrokerError::InvalidProviderUserPolicy);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let snapshot =
            load_provider_snapshot(&connection, policy.provider_policy_snapshot_digest.as_str())?
                .ok_or(BrokerError::InvalidProviderUserPolicy)?;
        if snapshot.identity != policy.provider_identity
            || policy.recorded_at < snapshot.registered_at
        {
            return Err(BrokerError::InvalidProviderUserPolicy);
        }
        let latest = connection
            .query_row(
                "SELECT recorded_at FROM provider_user_policy WHERE provider_policy_snapshot_digest = ?1 ORDER BY recorded_at DESC, user_policy_seq DESC LIMIT 1",
                [policy.provider_policy_snapshot_digest.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(provider_u64)
            .transpose()?;
        if latest.is_some_and(|at| policy.recorded_at <= at) {
            return Err(BrokerError::NonMonotonicProviderRegistration);
        }
        let transaction = connection.transaction()?;
        transaction.execute(
            concat!(
                "INSERT INTO provider_user_policy (policy_id, destination_id, ",
                "provider_policy_snapshot_digest, allow_without_deletion_api, ",
                "require_transit_encryption, require_at_rest_encryption, ",
                "decision_evidence_id, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ),
            params![
                policy.policy_id,
                snapshot.destination_id,
                policy.provider_policy_snapshot_digest.as_str(),
                policy.allow_without_deletion_api,
                policy.require_transit_encryption,
                policy.require_at_rest_encryption,
                policy.decision_evidence_id,
                sqlite_u64(policy.recorded_at)?,
            ],
        )?;
        for region in &policy.allowed_residency_regions {
            transaction.execute(
                "INSERT INTO provider_user_policy_residency (policy_id, region) VALUES (?1, ?2)",
                params![policy.policy_id, region],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Stores immutable deletion receipt metadata linked to a grant and its allow audit.
    pub fn store_deletion_receipt(
        &self,
        draft: DeletionReceiptDraft,
    ) -> Result<DeletionReceiptRow, BrokerError> {
        if !valid_text(&draft.receipt_id)
            || !valid_text(&draft.grant_id)
            || draft.received_at < draft.requested_at
        {
            return Err(BrokerError::InvalidDeletionReceipt);
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        let linked = connection
            .query_row(
                concat!(
                    "SELECT grant.provider_policy_snapshot_digest, grant.provider_id, ",
                    "audit.finished_at ",
                    "FROM egress_grant AS grant JOIN egress_audit AS audit ",
                    "ON audit.grant_id = grant.grant_id ",
                    "JOIN egress_consumption AS consumption ON ",
                    "consumption.grant_id = grant.grant_id ",
                    "AND consumption.egress_audit_seq = audit.audit_seq ",
                    "WHERE grant.grant_id = ?1 AND audit.audit_seq = ?2 ",
                    "AND audit.decision = 'ALLOW' AND audit.destination_id = grant.provider_id ",
                    "AND grant.consumed_at = audit.started_at ",
                    "AND consumption.consumed_at = grant.consumed_at"
                ),
                params![draft.grant_id, sqlite_u64(draft.egress_audit_seq)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(BrokerError::InvalidDeletionReceipt)?;
        if draft.requested_at < provider_u64(linked.2)? {
            return Err(BrokerError::InvalidDeletionReceipt);
        }
        let snapshot = load_provider_snapshot(&connection, &linked.0)?
            .ok_or(BrokerError::CorruptProviderRecord)?;
        if snapshot.destination_id != linked.1
            || !snapshot.deletion_api_available
            || !snapshot.deletion_receipt_capable
        {
            return Err(BrokerError::Denied(ReasonCode::NoDeletionReceipt));
        }
        let transaction = connection.transaction()?;
        transaction.execute(
            concat!(
                "INSERT INTO provider_deletion_receipt (receipt_id, grant_id, egress_audit_seq, ",
                "provider_policy_snapshot_digest, provider_receipt_digest, requested_at, received_at) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
            ),
            params![
                draft.receipt_id,
                draft.grant_id,
                sqlite_u64(draft.egress_audit_seq)?,
                snapshot.snapshot_digest.as_str(),
                draft.provider_receipt_digest.as_str(),
                sqlite_u64(draft.requested_at)?,
                sqlite_u64(draft.received_at)?,
            ],
        )?;
        let receipt_seq = u64::try_from(transaction.last_insert_rowid())
            .map_err(|_| BrokerError::CorruptProviderRecord)?;
        transaction.commit()?;
        Ok(DeletionReceiptRow {
            receipt_seq,
            receipt_id: draft.receipt_id,
            grant_id: draft.grant_id,
            egress_audit_seq: draft.egress_audit_seq,
            provider_policy_snapshot_digest: snapshot.snapshot_digest,
            provider_receipt_digest: draft.provider_receipt_digest,
            requested_at: draft.requested_at,
            received_at: draft.received_at,
        })
    }

    /// Reads one deletion receipt by provider-issued identifier.
    pub fn deletion_receipt(
        &self,
        receipt_id: &str,
    ) -> Result<Option<DeletionReceiptRow>, BrokerError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| BrokerError::LockPoisoned)?;
        connection
            .query_row(
                concat!(
                    "SELECT receipt_seq, receipt_id, grant_id, egress_audit_seq, ",
                    "provider_policy_snapshot_digest, provider_receipt_digest, requested_at, ",
                    "received_at FROM provider_deletion_receipt WHERE receipt_id = ?1"
                ),
                [receipt_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional()?
            .map(|row| {
                Ok(DeletionReceiptRow {
                    receipt_seq: provider_u64(row.0)?,
                    receipt_id: row.1,
                    grant_id: row.2,
                    egress_audit_seq: provider_u64(row.3)?,
                    provider_policy_snapshot_digest: ContentDigest::parse(row.4)?,
                    provider_receipt_digest: ContentDigest::parse(row.5)?,
                    requested_at: provider_u64(row.6)?,
                    received_at: provider_u64(row.7)?,
                })
            })
            .transpose()
    }
}

pub(crate) struct ProviderGrantFacts<'a> {
    pub destination_id: &'a str,
    pub provider_policy_snapshot_digest: &'a str,
    pub retention_terms_hash: &'a str,
    pub training_use_allowed: bool,
    pub byte_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProviderRegistryRevision {
    snapshot_seq: u64,
    user_policy_seq: u64,
}

impl ProviderRegistryRevision {
    pub(crate) const fn empty() -> Self {
        Self {
            snapshot_seq: 0,
            user_policy_seq: 0,
        }
    }
}

pub(crate) fn provider_registry_revision(
    connection: &Connection,
) -> Result<ProviderRegistryRevision, BrokerError> {
    let (snapshot_seq, user_policy_seq) = connection.query_row(
        concat!(
            "SELECT COALESCE((SELECT MAX(snapshot_seq) FROM provider_policy_snapshot), 0), ",
            "COALESCE((SELECT MAX(user_policy_seq) FROM provider_user_policy), 0)"
        ),
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(ProviderRegistryRevision {
        snapshot_seq: provider_u64(snapshot_seq)?,
        user_policy_seq: provider_u64(user_policy_seq)?,
    })
}

pub(crate) fn validate_provider_for_grant(
    connection: &Connection,
    facts: &ProviderGrantFacts<'_>,
    at: u64,
    revision: Option<ProviderRegistryRevision>,
) -> Result<u64, BrokerError> {
    let snapshot = load_current_provider_snapshot_at_revision(
        connection,
        facts.destination_id,
        at,
        revision.map_or(MAX_SQLITE_SEQUENCE, |value| value.snapshot_seq),
    )?
    .ok_or(BrokerError::Denied(ReasonCode::ProviderPolicyIncompatible))?;
    if snapshot.snapshot_digest.as_str() != facts.provider_policy_snapshot_digest {
        return Err(BrokerError::Denied(ReasonCode::ProviderPolicyIncompatible));
    }
    let verified_until = snapshot.verified_until();
    if at >= verified_until {
        return Err(BrokerError::Denied(ReasonCode::PolicyStale));
    }
    if snapshot.retention_terms_hash().as_str() != facts.retention_terms_hash
        || (!facts.training_use_allowed && snapshot.effective_training_use())
    {
        return Err(BrokerError::Denied(ReasonCode::ProviderPolicyIncompatible));
    }
    if facts.byte_count > snapshot.maximum_input_bytes {
        return Err(BrokerError::Denied(ReasonCode::Oversize));
    }

    let user_policy = load_current_user_policy(
        connection,
        snapshot.snapshot_digest.as_str(),
        at,
        revision.map_or(MAX_SQLITE_SEQUENCE, |value| value.user_policy_seq),
    )?;
    if let Some(policy) = user_policy.as_ref()
        && (policy.require_transit_encryption && !snapshot.transit_encryption_declared
            || policy.require_at_rest_encryption && !snapshot.at_rest_encryption_declared
            || snapshot
                .residency_regions
                .iter()
                .any(|region| !policy.allowed_residency_regions.contains(region)))
    {
        return Err(BrokerError::Denied(ReasonCode::ProviderPolicyIncompatible));
    }
    if !snapshot.deletion_api_available
        && !user_policy.is_some_and(|policy| policy.allow_without_deletion_api)
    {
        return Err(BrokerError::Denied(ReasonCode::NoDeletionReceipt));
    }
    Ok(verified_until)
}

fn insert_provider_snapshot(
    transaction: &Transaction<'_>,
    snapshot: &ProviderPolicySnapshot,
) -> Result<(), BrokerError> {
    transaction.execute(
        concat!(
            "INSERT INTO provider_policy_snapshot (snapshot_digest, destination_id, vendor_id, ",
            "surface, training_use_enabled, training_opt_out_applied, server_retention_millis, ",
            "abuse_logging_enabled, transit_encryption_declared, at_rest_encryption_declared, ",
            "deletion_api_available, deletion_receipt_capable, maximum_input_bytes, ",
            "logging_configuration, policy_source_digest, last_verified_at, ttl_millis, ",
            "registered_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ",
            "?13, ?14, ?15, ?16, ?17, ?18)"
        ),
        params![
            snapshot.snapshot_digest.as_str(),
            snapshot.destination_id,
            snapshot.identity.vendor_id,
            snapshot.identity.surface.as_str(),
            snapshot.training_use_enabled,
            snapshot.training_opt_out_applied,
            sqlite_u64(snapshot.server_retention_millis)?,
            snapshot.abuse_logging_enabled,
            snapshot.transit_encryption_declared,
            snapshot.at_rest_encryption_declared,
            snapshot.deletion_api_available,
            snapshot.deletion_receipt_capable,
            sqlite_u64(snapshot.maximum_input_bytes)?,
            snapshot.logging_configuration,
            snapshot.policy_source_digest.as_str(),
            sqlite_u64(snapshot.last_verified_at)?,
            sqlite_u64(snapshot.ttl_millis)?,
            sqlite_u64(snapshot.registered_at)?,
        ],
    )?;
    for region in &snapshot.residency_regions {
        transaction.execute(
            "INSERT INTO provider_policy_residency (snapshot_digest, region) VALUES (?1, ?2)",
            params![snapshot.snapshot_digest.as_str(), region],
        )?;
    }
    for subprocessor in &snapshot.subprocessors {
        transaction.execute(
            "INSERT INTO provider_policy_subprocessor (snapshot_digest, subprocessor) VALUES (?1, ?2)",
            params![snapshot.snapshot_digest.as_str(), subprocessor],
        )?;
    }
    Ok(())
}

fn load_current_provider_snapshot(
    connection: &Connection,
    destination_id: &str,
    at: u64,
) -> Result<Option<ProviderPolicySnapshot>, BrokerError> {
    load_current_provider_snapshot_at_revision(connection, destination_id, at, MAX_SQLITE_SEQUENCE)
}

fn load_current_provider_snapshot_at_revision(
    connection: &Connection,
    destination_id: &str,
    at: u64,
    maximum_snapshot_seq: u64,
) -> Result<Option<ProviderPolicySnapshot>, BrokerError> {
    let digest = connection
        .query_row(
            concat!(
                "SELECT snapshot_digest FROM provider_policy_snapshot ",
                "WHERE destination_id = ?1 AND registered_at <= ?2 AND snapshot_seq <= ?3 ",
                "ORDER BY registered_at DESC, snapshot_seq DESC LIMIT 1"
            ),
            params![
                destination_id,
                sqlite_u64(at)?,
                sqlite_u64(maximum_snapshot_seq)?,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    digest
        .map(|digest| {
            load_provider_snapshot(connection, &digest)?.ok_or(BrokerError::CorruptProviderRecord)
        })
        .transpose()
}

fn load_provider_snapshot(
    connection: &Connection,
    digest: &str,
) -> Result<Option<ProviderPolicySnapshot>, BrokerError> {
    let scalar = connection
        .query_row(
            concat!(
                "SELECT snapshot_digest, destination_id, vendor_id, surface, ",
                "training_use_enabled, training_opt_out_applied, server_retention_millis, ",
                "abuse_logging_enabled, transit_encryption_declared, at_rest_encryption_declared, ",
                "deletion_api_available, deletion_receipt_capable, maximum_input_bytes, ",
                "logging_configuration, policy_source_digest, last_verified_at, ttl_millis, ",
                "registered_at FROM provider_policy_snapshot WHERE snapshot_digest = ?1"
            ),
            [digest],
            provider_scalar_row,
        )
        .optional()?;
    scalar
        .map(|scalar| scalar.into_snapshot(connection))
        .transpose()
}

#[derive(Debug)]
struct ProviderScalarRow {
    snapshot_digest: String,
    destination_id: String,
    vendor_id: String,
    surface: String,
    training_use_enabled: bool,
    training_opt_out_applied: bool,
    server_retention_millis: i64,
    abuse_logging_enabled: bool,
    transit_encryption_declared: bool,
    at_rest_encryption_declared: bool,
    deletion_api_available: bool,
    deletion_receipt_capable: bool,
    maximum_input_bytes: i64,
    logging_configuration: String,
    policy_source_digest: String,
    last_verified_at: i64,
    ttl_millis: i64,
    registered_at: i64,
}

impl ProviderScalarRow {
    fn into_snapshot(self, connection: &Connection) -> Result<ProviderPolicySnapshot, BrokerError> {
        let residency_regions = load_strings(
            connection,
            "SELECT region FROM provider_policy_residency WHERE snapshot_digest = ?1 ORDER BY region",
            &self.snapshot_digest,
        )?;
        let subprocessors = load_strings(
            connection,
            "SELECT subprocessor FROM provider_policy_subprocessor WHERE snapshot_digest = ?1 ORDER BY subprocessor",
            &self.snapshot_digest,
        )?;
        let snapshot = ProviderPolicySnapshot {
            snapshot_digest: ContentDigest::parse(self.snapshot_digest)?,
            identity: ProviderIdentity::new(
                self.vendor_id,
                ProviderSurface::parse(&self.surface)?,
            )?,
            destination_id: self.destination_id,
            training_use_enabled: self.training_use_enabled,
            training_opt_out_applied: self.training_opt_out_applied,
            server_retention_millis: provider_u64(self.server_retention_millis)?,
            abuse_logging_enabled: self.abuse_logging_enabled,
            residency_regions,
            subprocessors,
            transit_encryption_declared: self.transit_encryption_declared,
            at_rest_encryption_declared: self.at_rest_encryption_declared,
            deletion_api_available: self.deletion_api_available,
            deletion_receipt_capable: self.deletion_receipt_capable,
            maximum_input_bytes: provider_u64(self.maximum_input_bytes)?,
            logging_configuration: self.logging_configuration,
            policy_source_digest: ContentDigest::parse(self.policy_source_digest)?,
            last_verified_at: provider_u64(self.last_verified_at)?,
            ttl_millis: provider_u64(self.ttl_millis)?,
            registered_at: provider_u64(self.registered_at)?,
        };
        if snapshot.identity.destination_id() != snapshot.destination_id
            || ContentDigest::of(&snapshot.canonical_bytes()) != snapshot.snapshot_digest
        {
            return Err(BrokerError::CorruptProviderRecord);
        }
        Ok(snapshot)
    }
}

fn provider_scalar_row(row: &Row<'_>) -> rusqlite::Result<ProviderScalarRow> {
    Ok(ProviderScalarRow {
        snapshot_digest: row.get(0)?,
        destination_id: row.get(1)?,
        vendor_id: row.get(2)?,
        surface: row.get(3)?,
        training_use_enabled: row.get(4)?,
        training_opt_out_applied: row.get(5)?,
        server_retention_millis: row.get(6)?,
        abuse_logging_enabled: row.get(7)?,
        transit_encryption_declared: row.get(8)?,
        at_rest_encryption_declared: row.get(9)?,
        deletion_api_available: row.get(10)?,
        deletion_receipt_capable: row.get(11)?,
        maximum_input_bytes: row.get(12)?,
        logging_configuration: row.get(13)?,
        policy_source_digest: row.get(14)?,
        last_verified_at: row.get(15)?,
        ttl_millis: row.get(16)?,
        registered_at: row.get(17)?,
    })
}

#[derive(Debug)]
struct StoredUserPolicy {
    allow_without_deletion_api: bool,
    require_transit_encryption: bool,
    require_at_rest_encryption: bool,
    allowed_residency_regions: Vec<String>,
}

fn load_current_user_policy(
    connection: &Connection,
    snapshot_digest: &str,
    at: u64,
    maximum_user_policy_seq: u64,
) -> Result<Option<StoredUserPolicy>, BrokerError> {
    let scalar = connection
        .query_row(
            concat!(
                "SELECT policy_id, allow_without_deletion_api, require_transit_encryption, ",
                "require_at_rest_encryption FROM provider_user_policy ",
                "WHERE provider_policy_snapshot_digest = ?1 AND recorded_at <= ?2 ",
                "AND user_policy_seq <= ?3 ",
                "ORDER BY recorded_at DESC, user_policy_seq DESC LIMIT 1"
            ),
            params![
                snapshot_digest,
                sqlite_u64(at)?,
                sqlite_u64(maximum_user_policy_seq)?,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()?;
    scalar
        .map(|(policy_id, allow, require_transit, require_at_rest)| {
            Ok(StoredUserPolicy {
                allow_without_deletion_api: allow,
                require_transit_encryption: require_transit,
                require_at_rest_encryption: require_at_rest,
                allowed_residency_regions: load_strings(
                    connection,
                    "SELECT region FROM provider_user_policy_residency WHERE policy_id = ?1 ORDER BY region",
                    &policy_id,
                )?,
            })
        })
        .transpose()
}

fn load_strings(connection: &Connection, sql: &str, key: &str) -> Result<Vec<String>, BrokerError> {
    let mut statement = connection.prepare(sql)?;
    Ok(statement
        .query_map([key], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn required<T>(value: Option<T>, field: &'static str) -> Result<T, BrokerError> {
    value.ok_or(BrokerError::MissingProviderPrivacyField(field))
}

fn canonical_set(mut values: Vec<String>) -> Result<Vec<String>, BrokerError> {
    if values.iter().any(|value| !valid_text(value)) {
        return Err(BrokerError::InvalidProviderPolicy);
    }
    values.sort();
    values.dedup();
    Ok(values)
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && !value.contains('\0')
}

fn push_set(bytes: &mut Vec<u8>, values: &[String]) {
    bytes.extend_from_slice(
        &u64::try_from(values.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for value in values {
        push_string(bytes, value);
    }
}

fn provider_u64(value: i64) -> Result<u64, BrokerError> {
    u64::try_from(value).map_err(|_| BrokerError::CorruptProviderRecord)
}
