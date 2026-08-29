//! Canonical database access, deterministic row projection, and signed replay.
//!
//! Every row read here is ordered by its canonical identifier, never by SQLite
//! rowid or insertion order, so the same committed watermark produces the same
//! bytes on Windows and Linux. Nothing in this module reads the disposable
//! projection sidecar: projections are never export or restore authority.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use academic_contracts::{
    DeviceAuthorization, decode_canonical_claim_object, decode_canonical_evidence_ids,
    decode_canonical_evidence_locator, encode_canonical_actor, encode_canonical_event_payload,
    verify_signed_batch,
};
use academic_domain::{
    ArtifactDescriptor, ArtifactId, ArtifactRepresentation, ClaimObject, Confidentiality,
    ContentDigest, DomainError, DomainId, EventPayload, EvidenceLocator, MediaType,
    PermissionLineageId, RetentionClass, VaultLocator,
};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::{
    PortabilityError, PortabilityResult,
    checksum::{CanonicalDigest, decode_hex, encode_hex},
};

/// Domain separator for the complete canonical-state digest.
pub const CANONICAL_SEMANTIC_DIGEST_DOMAIN: &str = "learning-platform.phase1.canonical-semantic.v1";

/// One read-only canonical SQLite handle owned by the portability boundary.
///
/// A live profile is admitted only after the guarded store reader has already
/// accepted its schema, pragmas, and FTS5 availability. A database that this
/// crate itself copied is opened read-write and immediately constrained to
/// `query_only`, because a freshly copied file may still need the write
/// capability that WAL initialization requires.
#[derive(Debug)]
pub struct CanonicalDatabase {
    connection: Connection,
    path: PathBuf,
}

impl CanonicalDatabase {
    /// Opens a live profile database through the guarded store boundary first.
    #[cfg(not(feature = "encrypted-portability"))]
    pub fn open_source(database_path: &Path) -> PortabilityResult<Self> {
        let guarded = academic_store::connection::open_reader(database_path)?;
        drop(guarded);
        let connection = Connection::open_with_flags(
            database_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::constrain(connection, database_path)
    }

    /// Opens a live encrypted profile through the guarded keyed boundary first.
    ///
    /// The guarded reader admits the schema and pragmas exactly as it does in
    /// the plaintext lane; this handle then re-applies the key as its own first
    /// statement, because SQLCipher needs it before the first page is read.
    #[cfg(feature = "encrypted-portability")]
    pub fn open_source(
        database_path: &Path,
        key: &academic_crypto::StoreKey,
    ) -> PortabilityResult<Self> {
        let guarded = academic_store::connection::open_keyed_reader(database_path, key)?;
        drop(guarded);
        let connection = Connection::open_with_flags(
            database_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        academic_store::cipher::apply_store_key(&connection, key, database_path)?;
        Self::constrain(connection, database_path)
    }

    /// Opens a database this crate copied, without granting a write capability.
    #[cfg(not(feature = "encrypted-portability"))]
    pub fn open_copy(database_path: &Path) -> PortabilityResult<Self> {
        let connection = Connection::open_with_flags(
            database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::constrain(connection, database_path)
    }

    /// Opens an encrypted database this crate copied, keyed before first read.
    #[cfg(feature = "encrypted-portability")]
    pub fn open_copy(
        database_path: &Path,
        key: &academic_crypto::StoreKey,
    ) -> PortabilityResult<Self> {
        let connection = Connection::open_with_flags(
            database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        academic_store::cipher::apply_store_key(&connection, key, database_path)?;
        Self::constrain(connection, database_path)
    }

    fn constrain(connection: Connection, database_path: &Path) -> PortabilityResult<Self> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;\
             PRAGMA trusted_schema = OFF;\
             PRAGMA busy_timeout = 250;\
             PRAGMA temp_store = MEMORY;\
             PRAGMA query_only = ON;",
        )?;
        let query_only: i64 = connection.query_row("PRAGMA query_only", [], |row| row.get(0))?;
        if query_only != 1 {
            return Err(PortabilityError::mismatch(
                "portability reader query_only",
                1,
                query_only,
            ));
        }
        let application_id: i64 =
            connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
        let expected = i64::from(academic_store::SQLITE_APPLICATION_ID);
        if application_id != expected {
            return Err(PortabilityError::mismatch(
                "SQLite application identifier",
                expected,
                application_id,
            ));
        }
        Ok(Self {
            connection,
            path: database_path.to_path_buf(),
        })
    }

    /// Returns the database path without exposing a raw handle to callers.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opens one deferred read transaction, or joins the one already open.
    ///
    /// SQLite gives every autocommit statement its own read snapshot, so a
    /// sequence of `SELECT`s against a live profile can observe rows a writer
    /// committed between two of them. Every read *set* this crate publishes is
    /// a claim about one committed state — an export manifest's watermark,
    /// counts, and rows have to describe the same commit or they describe none
    /// — so the whole set runs inside one transaction. Deferred is the exact
    /// mode: the snapshot is taken by the first read, not by `BEGIN`, and a
    /// `query_only` connection may open one.
    ///
    /// The guard joins an already-open transaction rather than nesting, so a
    /// caller needing a wider snapshot than one read function opens the outer
    /// one and the inner guards leave it alone.
    pub fn begin_read(&self) -> PortabilityResult<ReadSnapshot<'_>> {
        if !self.connection.is_autocommit() {
            return Ok(ReadSnapshot { owned: None });
        }
        self.connection.execute_batch("BEGIN DEFERRED")?;
        Ok(ReadSnapshot {
            owned: Some(&self.connection),
        })
    }

    pub(crate) const fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Runs `PRAGMA cipher_integrity_check` and fails closed on any report.
    ///
    /// SQLite's own `integrity_check` walks the decrypted pages; this walks the
    /// per-page HMACs, so a page whose ciphertext was edited is reported even
    /// when the b-tree it decrypts to is still well formed.
    #[cfg(feature = "encrypted-portability")]
    pub fn cipher_integrity_check(&self) -> PortabilityResult<()> {
        let reported = academic_store::cipher::cipher_integrity_report(&self.connection)?;
        if reported.is_empty() {
            return Ok(());
        }
        Err(PortabilityError::DatabaseCheckFailed {
            check: "cipher_integrity_check",
            detail: reported.join("; "),
        })
    }

    /// Runs `PRAGMA integrity_check` and fails closed on anything but `ok`.
    pub fn integrity_check(&self) -> PortabilityResult<()> {
        let mut statement = self.connection.prepare("PRAGMA integrity_check")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        if messages.len() == 1 && messages[0] == "ok" {
            return Ok(());
        }
        Err(PortabilityError::DatabaseCheckFailed {
            check: "integrity_check",
            detail: messages.join("; "),
        })
    }

    /// Runs `PRAGMA foreign_key_check` and fails closed on any reported row.
    pub fn foreign_key_check(&self) -> PortabilityResult<()> {
        let mut statement = self.connection.prepare("PRAGMA foreign_key_check")?;
        let rows = statement.query_map([], |row| {
            Ok(format!(
                "{} -> {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(2)?
            ))
        })?;
        let mut violations = Vec::new();
        for row in rows {
            violations.push(row?);
        }
        if violations.is_empty() {
            return Ok(());
        }
        Err(PortabilityError::DatabaseCheckFailed {
            check: "foreign_key_check",
            detail: violations.join("; "),
        })
    }
}

/// Frozen synthetic-only policy repeated by every portable manifest.
#[cfg(not(feature = "encrypted-portability"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBlock {
    pub data_policy: String,
    pub storage_mode: String,
    pub storage_encryption: String,
    pub production_data_allowed: bool,
    pub product_network: String,
}

/// The posture an encrypted backup is allowed to claim.
///
/// The schema-2 singleton records the *format* and nothing else: t068 section
/// 3.1 emits `data_policy`, `production_data_allowed`, and `product_network`
/// only when `AdmissionVerifier::verify()` succeeds, which `P2-K6` has not
/// shipped and `P2-H1` has not signed. So a manifest may not read those three
/// out of the database, and it may not invent them either. The two booleans
/// below are compiled facts about *this build*, asserted on write and on read.
#[cfg(feature = "encrypted-portability")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBlock {
    pub storage_mode: String,
    pub storage_encryption: String,
    pub production_data_allowed: bool,
    pub adr_002_accepted: bool,
}

#[cfg(feature = "encrypted-portability")]
impl PolicyBlock {
    /// Returns the exact posture an encrypted backup may claim today.
    #[must_use]
    pub fn encrypted_v2() -> Self {
        Self {
            storage_mode: academic_store::cipher::ENCRYPTED_STORE_STORAGE_MODE.to_owned(),
            storage_encryption: academic_store::cipher::ENCRYPTED_STORE_STORAGE_ENCRYPTION
                .to_owned(),
            production_data_allowed: false,
            adr_002_accepted: false,
        }
    }

    /// Rejects any policy block that claims more than this build may claim.
    pub fn require_encrypted_v2(&self) -> PortabilityResult<()> {
        let expected = Self::encrypted_v2();
        if self.storage_mode != expected.storage_mode {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.storage_mode",
            });
        }
        if self.storage_encryption != expected.storage_encryption {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.storage_encryption",
            });
        }
        if self.production_data_allowed {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.production_data_allowed",
            });
        }
        if self.adr_002_accepted {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.adr_002_accepted",
            });
        }
        Ok(())
    }
}

#[cfg(not(feature = "encrypted-portability"))]
impl PolicyBlock {
    /// Returns the exact frozen Phase 1 policy.
    #[must_use]
    pub fn phase1() -> Self {
        let policy = academic_store::PHASE1_STORAGE_POLICY;
        Self {
            data_policy: policy.data_policy.to_owned(),
            storage_mode: policy.storage_mode.to_owned(),
            storage_encryption: policy.storage_encryption.to_owned(),
            production_data_allowed: policy.production_data_allowed,
            product_network: policy.product_network.to_owned(),
        }
    }

    /// Rejects any policy block that is not the frozen synthetic-only posture.
    pub fn require_phase1(&self) -> PortabilityResult<()> {
        let expected = Self::phase1();
        if self.data_policy != expected.data_policy {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.data_policy",
            });
        }
        if self.storage_mode != expected.storage_mode {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.storage_mode",
            });
        }
        if self.storage_encryption != expected.storage_encryption {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.storage_encryption",
            });
        }
        if self.production_data_allowed {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.production_data_allowed",
            });
        }
        if self.product_network != expected.product_network {
            return Err(PortabilityError::ManifestRejected {
                field: "policy.product_network",
            });
        }
        Ok(())
    }
}

/// Physical store identity copied from the `schema_meta` singleton.
#[cfg(not(feature = "encrypted-portability"))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreSchemaIdentity {
    pub format_uuid: String,
    pub schema_version: u32,
    pub schema_semver: String,
    pub minimum_reader_protocol_major: u32,
    pub minimum_reader_protocol_minor: u32,
    pub minimum_writer_protocol_major: u32,
    pub minimum_writer_protocol_minor: u32,
    pub policy: PolicyBlock,
}

/// Physical store identity copied from the schema-2 `schema_meta` singleton.
///
/// The field list is exactly the columns migration `0003` creates. The three
/// posture columns the Phase 1 singleton carried are absent from the table, so
/// they are absent here; `policy` is this build's compiled posture rather than
/// a database read.
#[cfg(feature = "encrypted-portability")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreSchemaIdentity {
    pub format_uuid: String,
    pub schema_version: u32,
    pub schema_semver: String,
    pub minimum_reader_protocol_major: u32,
    pub minimum_reader_protocol_minor: u32,
    pub minimum_writer_protocol_major: u32,
    pub minimum_writer_protocol_minor: u32,
    pub policy: PolicyBlock,
}

/// Replica-local acceptance coordinates that fix one snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalWatermark {
    pub next_accept_seq: u64,
    pub profile_revision: u64,
    pub accept_seq_head: u64,
    pub outbox_head: u64,
}

/// Exact canonical row counts compared by backup and restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalCounts {
    pub batches: u64,
    pub events: u64,
    pub scopes: u64,
    pub artifacts: u64,
    pub artifact_representations: u64,
    pub evidence: u64,
    pub claims: u64,
    pub claim_evidence_links: u64,
    pub relations: u64,
    pub decisions: u64,
    pub outbox: u64,
    pub command_receipts: u64,
    pub device_heads: u64,
}

/// One device origin-chain head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceHeadRow {
    pub device_id: String,
    pub next_origin_seq: u64,
    pub head_batch_id: String,
    pub head_envelope_sha256: String,
    pub updated_at_unix_ms: i64,
}

/// One accepted signed batch without its original bytes.
///
/// The original envelope is exported byte-for-byte as its own file; this record
/// carries only the replica-local acceptance coordinates and exact digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRow {
    pub batch_id: String,
    pub device_id: String,
    pub origin_seq_start: u64,
    pub origin_seq_end: u64,
    pub previous_envelope_sha256: Option<String>,
    pub origin_created_at_unix_ms: i64,
    pub event_schema_version: u32,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
    pub accepted_at_unix_ms: i64,
    pub envelope_sha256: String,
    pub envelope_byte_length: u64,
    pub deterministic_payload_sha256: String,
    pub deterministic_payload_byte_length: u64,
    pub signing_public_key: String,
    pub signature: String,
}

/// One accepted canonical event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRow {
    pub event_id: String,
    pub batch_id: String,
    pub origin_seq: u64,
    pub origin_observed_at_unix_ms: i64,
    pub accept_seq: u64,
    pub actor_kind: String,
    pub actor_canonical_sha256: String,
    pub domain_id: String,
    pub event_kind: String,
    pub canonical_payload_sha256: String,
    pub canonical_payload_byte_length: u64,
}

/// One registered scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeRow {
    pub scope_id: String,
    pub created_event_id: String,
    pub domain_id: String,
    pub label: String,
}

/// One immutable representation registered for an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepresentationRow {
    pub representation_index: u64,
    pub locator_kind: String,
    pub locator: EvidenceLocator,
    pub content_digest: String,
    pub byte_length: u64,
}

/// One registered artifact descriptor and its ordered representations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRow {
    pub artifact_id: String,
    pub registered_event_id: String,
    pub content_digest: String,
    pub media_type: String,
    pub byte_length: u64,
    pub domain_id: String,
    pub confidentiality: String,
    pub retention_class: String,
    pub permission_lineage_id: String,
    pub format_version: u32,
    pub vault_locator: String,
    pub representations: Vec<RepresentationRow>,
}

/// One registered evidence item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRow {
    pub evidence_id: String,
    pub registered_event_id: String,
    pub artifact_id: String,
    pub representation_index: u64,
    pub excerpt_digest: String,
    pub evidence_role: String,
    pub evidence_strength: String,
    pub extraction_method: String,
    pub extractor_version: String,
}

/// One asserted claim with its ordered evidence links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRow {
    pub claim_id: String,
    pub assertion_event_id: String,
    pub subject_entity_id: String,
    pub predicate_id: String,
    pub scope_id: String,
    pub object_kind: String,
    pub object_entity_id: Option<String>,
    pub object_text: Option<String>,
    pub object_integer: Option<i64>,
    pub object_decimal_coefficient: Option<String>,
    pub object_decimal_scale: Option<u32>,
    pub object_interval_from: Option<i64>,
    pub object_interval_to: Option<i64>,
    pub authority_class: String,
    pub epistemic_status: String,
    pub confidence_permille: Option<u32>,
    pub prediction_metadata_version: Option<u32>,
    pub prediction_observation_from: Option<i64>,
    pub prediction_observation_to: Option<i64>,
    pub prediction_sample_count: Option<u64>,
    pub valid_from_unix_ms: i64,
    pub valid_to_unix_ms: Option<i64>,
    pub evidence_ids: Vec<String>,
}

/// One canonical claim relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationRow {
    pub relation_event_id: String,
    pub source_claim_id: String,
    pub target_claim_id: String,
    pub scope_id: String,
    pub relation_kind: String,
    pub actor_kind: String,
}

/// One recorded user decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRow {
    pub decision_id: String,
    pub decision_event_id: String,
    pub target_claim_id: String,
    pub target_object: ClaimObject,
    pub target_object_canonical_sha256: String,
    pub resolution_subject_entity_id: String,
    pub resolution_predicate_id: String,
    pub resolution_scope_id: String,
    pub action: String,
    pub replacement_claim_id: Option<String>,
    pub valid_from_unix_ms: i64,
    pub valid_to_unix_ms: Option<i64>,
    pub rationale_evidence_ids: Vec<String>,
    pub decided_at_unix_ms: i64,
    pub reversible_until_unix_ms: Option<i64>,
}

/// One projection outbox row.
///
/// Outbox rows are canonical acceptance bookkeeping, not projection content, and
/// can never lead the canonical commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutboxRow {
    pub outbox_seq: u64,
    pub accepted_batch_id: String,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
    pub canonical_revision: u64,
    pub event_kind_mask: String,
    pub payload_digest: String,
    pub created_at_unix_ms: i64,
}

/// One immutable command receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandReceiptRow {
    pub client_instance_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub expected_revision: Option<u64>,
    pub committed_revision: u64,
    pub response_sha256: String,
    pub response_byte_length: u64,
    pub created_at_unix_ms: i64,
}

/// Complete canonical state of one committed watermark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalRows {
    pub schema: StoreSchemaIdentity,
    pub watermark: CanonicalWatermark,
    pub counts: CanonicalCounts,
    pub device_heads: Vec<DeviceHeadRow>,
    pub batches: Vec<BatchRow>,
    pub events: Vec<EventRow>,
    pub scopes: Vec<ScopeRow>,
    pub artifacts: Vec<ArtifactRow>,
    pub evidence: Vec<EvidenceRow>,
    pub claims: Vec<ClaimRow>,
    pub relations: Vec<RelationRow>,
    pub decisions: Vec<DecisionRow>,
    pub outbox: Vec<OutboxRow>,
    pub command_receipts: Vec<CommandReceiptRow>,
}

impl CanonicalRows {
    /// Hashes the complete canonical state with stable field ordering.
    ///
    /// The digest never observes wall-clock generation time, filesystem
    /// metadata, or projection state, so two snapshots of the same committed
    /// watermark agree byte-for-byte across hosts.
    pub fn semantic_digest(&self) -> PortabilityResult<ContentDigest> {
        let mut digest = CanonicalDigest::new(CANONICAL_SEMANTIC_DIGEST_DOMAIN);
        digest.text("schema").field(&canonical_json(&self.schema)?);
        digest
            .text("watermark")
            .field(&canonical_json(&self.watermark)?);
        digest.text("counts").field(&canonical_json(&self.counts)?);
        append_section(&mut digest, "device_heads", &self.device_heads)?;
        append_section(&mut digest, "batches", &self.batches)?;
        append_section(&mut digest, "events", &self.events)?;
        append_section(&mut digest, "scopes", &self.scopes)?;
        append_section(&mut digest, "artifacts", &self.artifacts)?;
        append_section(&mut digest, "evidence", &self.evidence)?;
        append_section(&mut digest, "claims", &self.claims)?;
        append_section(&mut digest, "relations", &self.relations)?;
        append_section(&mut digest, "decisions", &self.decisions)?;
        append_section(&mut digest, "outbox", &self.outbox)?;
        append_section(&mut digest, "command_receipts", &self.command_receipts)?;
        Ok(digest.finish())
    }
}

fn append_section<T: Serialize>(
    digest: &mut CanonicalDigest,
    name: &str,
    rows: &[T],
) -> PortabilityResult<()> {
    digest.text(name);
    digest.unsigned(count_of(rows.len()));
    for row in rows {
        digest.field(&canonical_json(row)?);
    }
    Ok(())
}

/// Serializes one value into compact, deterministic JSON bytes.
///
/// Struct field order is fixed by declaration order and no value in this crate
/// is a floating-point number, so the bytes are identical on every host.
pub fn canonical_json<T: Serialize>(value: &T) -> PortabilityResult<Vec<u8>> {
    serde_json::to_vec(value).map_err(|source| PortabilityError::Json {
        operation: "serialize canonical record",
        source,
    })
}

/// One deferred read transaction pinning a single snapshot for a read set.
///
/// Created by [`CanonicalDatabase::begin_read`]. Ending it is the whole
/// contract, so it ends on drop; a read transaction has nothing to commit,
/// which makes rollback its exact end rather than a discarded outcome.
#[derive(Debug)]
pub struct ReadSnapshot<'a> {
    /// The connection this guard must end the transaction on, or `None` when
    /// an outer guard owns it and is still reading through it.
    owned: Option<&'a Connection>,
}

impl Drop for ReadSnapshot<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.owned {
            // The only way this fails is a transaction that is already closed,
            // which is the state the drop is trying to reach.
            let _ = connection.execute_batch("ROLLBACK");
        }
    }
}

/// Reads the complete canonical state at the database's committed watermark.
pub fn read_canonical_rows(database: &CanonicalDatabase) -> PortabilityResult<CanonicalRows> {
    // Every table below is read through one snapshot: the counts, the
    // watermark, and the rows have to describe the same commit.
    let _snapshot = database.begin_read()?;
    let connection = database.connection();
    let schema = read_schema_identity(connection)?;
    let watermark = read_watermark(connection)?;
    let device_heads = read_device_heads(connection)?;
    let batches = read_batches(connection)?;
    let events = read_events(connection)?;
    let scopes = read_scopes(connection)?;
    let artifacts = read_artifacts(connection)?;
    let evidence = read_evidence(connection)?;
    let claims = read_claims(connection)?;
    let relations = read_relations(connection)?;
    let decisions = read_decisions(connection)?;
    let outbox = read_outbox(connection)?;
    let command_receipts = read_command_receipts(connection)?;
    let representation_total = artifacts.iter().fold(0_u64, |total, artifact| {
        total.saturating_add(count_of(artifact.representations.len()))
    });
    let evidence_links = claims.iter().fold(0_u64, |total, claim| {
        total.saturating_add(count_of(claim.evidence_ids.len()))
    });
    let counts = CanonicalCounts {
        batches: count_of(batches.len()),
        events: count_of(events.len()),
        scopes: count_of(scopes.len()),
        artifacts: count_of(artifacts.len()),
        artifact_representations: representation_total,
        evidence: count_of(evidence.len()),
        claims: count_of(claims.len()),
        claim_evidence_links: evidence_links,
        relations: count_of(relations.len()),
        decisions: count_of(decisions.len()),
        outbox: count_of(outbox.len()),
        command_receipts: count_of(command_receipts.len()),
        device_heads: count_of(device_heads.len()),
    };
    Ok(CanonicalRows {
        schema,
        watermark,
        counts,
        device_heads,
        batches,
        events,
        scopes,
        artifacts,
        evidence,
        claims,
        relations,
        decisions,
        outbox,
        command_receipts,
    })
}

pub(crate) fn count_of(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(not(feature = "encrypted-portability"))]
type SchemaRaw = (
    Vec<u8>,
    i64,
    String,
    i64,
    i64,
    i64,
    i64,
    String,
    String,
    String,
    i64,
    String,
);

#[cfg(feature = "encrypted-portability")]
type EncryptedSchemaRaw = (Vec<u8>, i64, String, i64, i64, i64, i64, String, String);

#[cfg(feature = "encrypted-portability")]
fn read_schema_identity(connection: &Connection) -> PortabilityResult<StoreSchemaIdentity> {
    let raw: EncryptedSchemaRaw = connection.query_row(
        concat!(
            "SELECT format_uuid, schema_version, schema_semver, ",
            "minimum_reader_protocol_major, minimum_reader_protocol_minor, ",
            "minimum_writer_protocol_major, minimum_writer_protocol_minor, ",
            "storage_mode, storage_encryption FROM schema_meta WHERE singleton = 1"
        ),
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        },
    )?;
    let (
        format_uuid,
        schema_version,
        schema_semver,
        reader_major,
        reader_minor,
        writer_major,
        writer_minor,
        storage_mode,
        storage_encryption,
    ) = raw;
    let policy = PolicyBlock {
        storage_mode,
        storage_encryption,
        production_data_allowed: false,
        adr_002_accepted: false,
    };
    // The physical facts are compared against what this build knows the
    // encrypted lane writes, so a manifest can never record a storage mode the
    // running binary does not implement.
    policy.require_encrypted_v2()?;
    Ok(StoreSchemaIdentity {
        format_uuid: encode_hex(&format_uuid),
        schema_version: bounded_u32(schema_version, "store schema version")?,
        schema_semver,
        minimum_reader_protocol_major: bounded_u32(reader_major, "minimum reader major")?,
        minimum_reader_protocol_minor: bounded_u32(reader_minor, "minimum reader minor")?,
        minimum_writer_protocol_major: bounded_u32(writer_major, "minimum writer major")?,
        minimum_writer_protocol_minor: bounded_u32(writer_minor, "minimum writer minor")?,
        policy,
    })
}

#[cfg(not(feature = "encrypted-portability"))]
fn read_schema_identity(connection: &Connection) -> PortabilityResult<StoreSchemaIdentity> {
    let raw: SchemaRaw = connection.query_row(
        concat!(
            "SELECT format_uuid, schema_version, schema_semver, ",
            "minimum_reader_protocol_major, minimum_reader_protocol_minor, ",
            "minimum_writer_protocol_major, minimum_writer_protocol_minor, ",
            "data_policy, storage_mode, storage_encryption, production_data_allowed, ",
            "product_network FROM schema_meta WHERE singleton = 1"
        ),
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        },
    )?;
    let (
        format_uuid,
        schema_version,
        schema_semver,
        reader_major,
        reader_minor,
        writer_major,
        writer_minor,
        data_policy,
        storage_mode,
        storage_encryption,
        production_data_allowed,
        product_network,
    ) = raw;
    Ok(StoreSchemaIdentity {
        format_uuid: encode_hex(&format_uuid),
        schema_version: bounded_u32(schema_version, "store schema version")?,
        schema_semver,
        minimum_reader_protocol_major: bounded_u32(reader_major, "minimum reader major")?,
        minimum_reader_protocol_minor: bounded_u32(reader_minor, "minimum reader minor")?,
        minimum_writer_protocol_major: bounded_u32(writer_major, "minimum writer major")?,
        minimum_writer_protocol_minor: bounded_u32(writer_minor, "minimum writer minor")?,
        policy: PolicyBlock {
            data_policy,
            storage_mode,
            storage_encryption,
            production_data_allowed: production_data_allowed != 0,
            product_network,
        },
    })
}

fn read_watermark(connection: &Connection) -> PortabilityResult<CanonicalWatermark> {
    let raw: (i64, i64, i64, i64) = connection.query_row(
        concat!(
            "SELECT r.next_accept_seq, r.profile_revision, ",
            "coalesce((SELECT max(accept_seq) FROM ledger_event), 0), ",
            "coalesce((SELECT max(outbox_seq) FROM projection_outbox), 0) ",
            "FROM replica_state r WHERE r.singleton = 1"
        ),
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let (next_accept_seq, profile_revision, accept_seq_head, outbox_head) = raw;
    Ok(CanonicalWatermark {
        next_accept_seq: nonnegative(next_accept_seq, "next acceptance sequence")?,
        profile_revision: nonnegative(profile_revision, "profile revision")?,
        accept_seq_head: nonnegative(accept_seq_head, "acceptance head")?,
        outbox_head: nonnegative(outbox_head, "outbox head")?,
    })
}

fn read_device_heads(connection: &Connection) -> PortabilityResult<Vec<DeviceHeadRow>> {
    collect(
        connection,
        concat!(
            "SELECT device_id, next_origin_seq, head_batch_id, head_envelope_hash, updated_at ",
            "FROM device_head ORDER BY device_id"
        ),
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
        |(device_id, next_origin_seq, head_batch_id, head_envelope_hash, updated_at)| {
            Ok(DeviceHeadRow {
                device_id: uuid_text(&device_id)?,
                next_origin_seq: nonnegative(next_origin_seq, "device next origin sequence")?,
                head_batch_id: uuid_text(&head_batch_id)?,
                head_envelope_sha256: encode_hex(&head_envelope_hash),
                updated_at_unix_ms: updated_at,
            })
        },
    )
}

type BatchRaw = (
    Vec<u8>,
    Vec<u8>,
    i64,
    i64,
    Option<Vec<u8>>,
    i64,
    i64,
    i64,
    i64,
    i64,
    Vec<u8>,
    i64,
    Vec<u8>,
    i64,
    Vec<u8>,
    Vec<u8>,
);

fn read_batches(connection: &Connection) -> PortabilityResult<Vec<BatchRow>> {
    collect(
        connection,
        concat!(
            "SELECT batch_id, device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
            "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, ",
            "accepted_at, envelope_hash, length(signed_envelope), deterministic_payload_hash, ",
            "length(deterministic_payload), signing_public_key, signature ",
            "FROM ledger_batch ORDER BY batch_id"
        ),
        |row| -> rusqlite::Result<BatchRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
            ))
        },
        |raw| {
            let (
                batch_id,
                device_id,
                origin_seq_start,
                origin_seq_end,
                previous_batch_hash,
                origin_created_at,
                event_schema_version,
                accept_seq_start,
                accept_seq_end,
                accepted_at,
                envelope_hash,
                envelope_length,
                payload_hash,
                payload_length,
                signing_public_key,
                signature,
            ) = raw;
            Ok(BatchRow {
                batch_id: uuid_text(&batch_id)?,
                device_id: uuid_text(&device_id)?,
                origin_seq_start: nonnegative(origin_seq_start, "origin sequence start")?,
                origin_seq_end: nonnegative(origin_seq_end, "origin sequence end")?,
                previous_envelope_sha256: previous_batch_hash.as_deref().map(encode_hex),
                origin_created_at_unix_ms: origin_created_at,
                event_schema_version: bounded_u32(event_schema_version, "event schema version")?,
                accept_seq_start: nonnegative(accept_seq_start, "acceptance start")?,
                accept_seq_end: nonnegative(accept_seq_end, "acceptance end")?,
                accepted_at_unix_ms: accepted_at,
                envelope_sha256: encode_hex(&envelope_hash),
                envelope_byte_length: nonnegative(envelope_length, "envelope byte length")?,
                deterministic_payload_sha256: encode_hex(&payload_hash),
                deterministic_payload_byte_length: nonnegative(
                    payload_length,
                    "payload byte length",
                )?,
                signing_public_key: encode_hex(&signing_public_key),
                signature: encode_hex(&signature),
            })
        },
    )
}

type EventRaw = (
    Vec<u8>,
    Vec<u8>,
    i64,
    i64,
    i64,
    String,
    Vec<u8>,
    Vec<u8>,
    String,
    Vec<u8>,
    i64,
);

fn read_events(connection: &Connection) -> PortabilityResult<Vec<EventRow>> {
    collect(
        connection,
        concat!(
            "SELECT event_id, batch_id, origin_seq, origin_observed_at, accept_seq, actor_kind, ",
            "actor_canonical, domain_id, event_kind, payload_hash, length(canonical_payload) ",
            "FROM ledger_event ORDER BY event_id"
        ),
        |row| -> rusqlite::Result<EventRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        },
        |raw| {
            let (
                event_id,
                batch_id,
                origin_seq,
                origin_observed_at,
                accept_seq,
                actor_kind,
                actor_canonical,
                domain_id,
                event_kind,
                payload_hash,
                payload_length,
            ) = raw;
            Ok(EventRow {
                event_id: uuid_text(&event_id)?,
                batch_id: uuid_text(&batch_id)?,
                origin_seq: nonnegative(origin_seq, "event origin sequence")?,
                origin_observed_at_unix_ms: origin_observed_at,
                accept_seq: nonnegative(accept_seq, "event acceptance sequence")?,
                actor_kind,
                actor_canonical_sha256: digest_hex(&actor_canonical),
                domain_id: uuid_text(&domain_id)?,
                event_kind,
                canonical_payload_sha256: encode_hex(&payload_hash),
                canonical_payload_byte_length: nonnegative(
                    payload_length,
                    "event payload byte length",
                )?,
            })
        },
    )
}

fn read_scopes(connection: &Connection) -> PortabilityResult<Vec<ScopeRow>> {
    collect(
        connection,
        "SELECT scope_id, created_event_id, domain_id, label FROM scope ORDER BY scope_id",
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
        |(scope_id, created_event_id, domain_id, label)| {
            Ok(ScopeRow {
                scope_id: uuid_text(&scope_id)?,
                created_event_id: uuid_text(&created_event_id)?,
                domain_id: uuid_text(&domain_id)?,
                label,
            })
        },
    )
}

type ArtifactRaw = (
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    String,
    i64,
    Vec<u8>,
    String,
    String,
    Vec<u8>,
    i64,
    Vec<u8>,
);

fn read_artifacts(connection: &Connection) -> PortabilityResult<Vec<ArtifactRow>> {
    let mut representations: BTreeMap<Vec<u8>, Vec<RepresentationRow>> = BTreeMap::new();
    let mut statement = connection.prepare(concat!(
        "SELECT artifact_id, representation_index, locator_kind, locator_payload, ",
        "content_digest, byte_length FROM artifact_representation ",
        "ORDER BY artifact_id, representation_index"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    for row in rows {
        let (artifact_id, index, locator_kind, locator_payload, content_digest, byte_length) = row?;
        let locator = decode_locator(&locator_kind, &locator_payload)?;
        representations
            .entry(artifact_id)
            .or_default()
            .push(RepresentationRow {
                representation_index: nonnegative(index, "representation index")?,
                locator_kind,
                locator,
                content_digest: encode_hex(&content_digest),
                byte_length: nonnegative(byte_length, "representation byte length")?,
            });
    }
    collect(
        connection,
        concat!(
            "SELECT artifact_id, registered_event_id, content_digest, media_type, byte_length, ",
            "domain_id, confidentiality, retention_class, permission_lineage_id, format_version, ",
            "vault_locator FROM artifact_descriptor ORDER BY artifact_id"
        ),
        |row| -> rusqlite::Result<ArtifactRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        },
        |raw| {
            let (
                artifact_id,
                registered_event_id,
                content_digest,
                media_type,
                byte_length,
                domain_id,
                confidentiality,
                retention_class,
                permission_lineage_id,
                format_version,
                vault_locator,
            ) = raw;
            let owned = representations
                .get(&artifact_id)
                .cloned()
                .unwrap_or_default();
            Ok(ArtifactRow {
                artifact_id: uuid_text(&artifact_id)?,
                registered_event_id: uuid_text(&registered_event_id)?,
                content_digest: encode_hex(&content_digest),
                media_type,
                byte_length: nonnegative(byte_length, "artifact byte length")?,
                domain_id: uuid_text(&domain_id)?,
                confidentiality,
                retention_class,
                permission_lineage_id: uuid_text(&permission_lineage_id)?,
                format_version: bounded_u32(format_version, "artifact format version")?,
                vault_locator: encode_hex(&vault_locator),
                representations: owned,
            })
        },
    )
}

type EvidenceRaw = (
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    i64,
    Vec<u8>,
    String,
    String,
    String,
    String,
);

fn read_evidence(connection: &Connection) -> PortabilityResult<Vec<EvidenceRow>> {
    collect(
        connection,
        concat!(
            "SELECT evidence_id, registered_event_id, artifact_id, representation_index, ",
            "excerpt_digest, evidence_role, evidence_strength, extraction_method, ",
            "extractor_version FROM evidence_item ORDER BY evidence_id"
        ),
        |row| -> rusqlite::Result<EvidenceRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        },
        |raw| {
            let (
                evidence_id,
                registered_event_id,
                artifact_id,
                representation_index,
                excerpt_digest,
                evidence_role,
                evidence_strength,
                extraction_method,
                extractor_version,
            ) = raw;
            Ok(EvidenceRow {
                evidence_id: uuid_text(&evidence_id)?,
                registered_event_id: uuid_text(&registered_event_id)?,
                artifact_id: uuid_text(&artifact_id)?,
                representation_index: nonnegative(
                    representation_index,
                    "evidence representation index",
                )?,
                excerpt_digest: encode_hex(&excerpt_digest),
                evidence_role,
                evidence_strength,
                extraction_method,
                extractor_version,
            })
        },
    )
}

type ClaimRaw = (
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    String,
    Vec<u8>,
    String,
    Option<Vec<u8>>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    i64,
    Option<i64>,
);

fn read_claims(connection: &Connection) -> PortabilityResult<Vec<ClaimRow>> {
    let mut links: BTreeMap<Vec<u8>, Vec<String>> = BTreeMap::new();
    let mut statement = connection.prepare(
        "SELECT claim_id, evidence_id FROM claim_evidence ORDER BY claim_id, evidence_ordinal",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    for row in rows {
        let (claim_id, evidence_id) = row?;
        links
            .entry(claim_id)
            .or_default()
            .push(uuid_text(&evidence_id)?);
    }
    collect(
        connection,
        concat!(
            "SELECT claim_id, assertion_event_id, subject_entity_id, predicate_id, scope_id, ",
            "object_kind, object_entity_id, object_text, object_integer, ",
            "object_decimal_coefficient, object_decimal_scale, object_interval_from, ",
            "object_interval_to, authority_class, epistemic_status, confidence_permille, ",
            "prediction_metadata_version, prediction_observation_from, ",
            "prediction_observation_to, prediction_sample_count, valid_from, valid_to ",
            "FROM claim ORDER BY claim_id"
        ),
        |row| -> rusqlite::Result<ClaimRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
                row.get(16)?,
                row.get(17)?,
                row.get(18)?,
                row.get(19)?,
                row.get(20)?,
                row.get(21)?,
            ))
        },
        |raw| {
            let (
                claim_id,
                assertion_event_id,
                subject_entity_id,
                predicate_id,
                scope_id,
                object_kind,
                object_entity_id,
                object_text,
                object_integer,
                object_decimal_coefficient,
                object_decimal_scale,
                object_interval_from,
                object_interval_to,
                authority_class,
                epistemic_status,
                confidence_permille,
                prediction_metadata_version,
                prediction_observation_from,
                prediction_observation_to,
                prediction_sample_count,
                valid_from,
                valid_to,
            ) = raw;
            let evidence_ids = links.get(&claim_id).cloned().unwrap_or_default();
            Ok(ClaimRow {
                claim_id: uuid_text(&claim_id)?,
                assertion_event_id: uuid_text(&assertion_event_id)?,
                subject_entity_id: uuid_text(&subject_entity_id)?,
                predicate_id,
                scope_id: uuid_text(&scope_id)?,
                object_kind,
                object_entity_id: object_entity_id.as_deref().map(uuid_text).transpose()?,
                object_text,
                object_integer,
                object_decimal_coefficient,
                object_decimal_scale: object_decimal_scale
                    .map(|value| bounded_u32(value, "decimal scale"))
                    .transpose()?,
                object_interval_from,
                object_interval_to,
                authority_class,
                epistemic_status,
                confidence_permille: confidence_permille
                    .map(|value| bounded_u32(value, "confidence permille"))
                    .transpose()?,
                prediction_metadata_version: prediction_metadata_version
                    .map(|value| bounded_u32(value, "prediction metadata version"))
                    .transpose()?,
                prediction_observation_from,
                prediction_observation_to,
                prediction_sample_count: prediction_sample_count
                    .map(|value| nonnegative(value, "prediction sample count"))
                    .transpose()?,
                valid_from_unix_ms: valid_from,
                valid_to_unix_ms: valid_to,
                evidence_ids,
            })
        },
    )
}

fn read_relations(connection: &Connection) -> PortabilityResult<Vec<RelationRow>> {
    collect(
        connection,
        concat!(
            "SELECT relation_event_id, source_claim_id, target_claim_id, scope_id, ",
            "relation_kind, actor_kind FROM claim_relation ORDER BY relation_event_id"
        ),
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
        |(
            relation_event_id,
            source_claim_id,
            target_claim_id,
            scope_id,
            relation_kind,
            actor_kind,
        )| {
            Ok(RelationRow {
                relation_event_id: uuid_text(&relation_event_id)?,
                source_claim_id: uuid_text(&source_claim_id)?,
                target_claim_id: uuid_text(&target_claim_id)?,
                scope_id: uuid_text(&scope_id)?,
                relation_kind,
                actor_kind,
            })
        },
    )
}

type DecisionRaw = (
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    String,
    Vec<u8>,
    String,
    Option<Vec<u8>>,
    i64,
    Option<i64>,
    Vec<u8>,
    i64,
    Option<i64>,
);

fn read_decisions(connection: &Connection) -> PortabilityResult<Vec<DecisionRow>> {
    collect(
        connection,
        concat!(
            "SELECT decision_id, decision_event_id, target_claim_id, target_object_canonical, ",
            "resolution_subject_entity_id, resolution_predicate_id, resolution_scope_id, action, ",
            "replacement_claim_id, valid_from, valid_to, rationale_evidence_ids_canonical, ",
            "decided_at, reversible_until FROM user_decision ORDER BY decision_id"
        ),
        |row| -> rusqlite::Result<DecisionRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
            ))
        },
        |raw| {
            let (
                decision_id,
                decision_event_id,
                target_claim_id,
                target_object_canonical,
                resolution_subject_entity_id,
                resolution_predicate_id,
                resolution_scope_id,
                action,
                replacement_claim_id,
                valid_from,
                valid_to,
                rationale_canonical,
                decided_at,
                reversible_until,
            ) = raw;
            let target_object = decode_canonical_claim_object(&target_object_canonical)?;
            let rationale = decode_canonical_evidence_ids(&rationale_canonical)?;
            Ok(DecisionRow {
                decision_id: uuid_text(&decision_id)?,
                decision_event_id: uuid_text(&decision_event_id)?,
                target_claim_id: uuid_text(&target_claim_id)?,
                target_object,
                target_object_canonical_sha256: digest_hex(&target_object_canonical),
                resolution_subject_entity_id: uuid_text(&resolution_subject_entity_id)?,
                resolution_predicate_id,
                resolution_scope_id: uuid_text(&resolution_scope_id)?,
                action,
                replacement_claim_id: replacement_claim_id.as_deref().map(uuid_text).transpose()?,
                valid_from_unix_ms: valid_from,
                valid_to_unix_ms: valid_to,
                rationale_evidence_ids: rationale
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>(),
                decided_at_unix_ms: decided_at,
                reversible_until_unix_ms: reversible_until,
            })
        },
    )
}

type OutboxRaw = (i64, Vec<u8>, i64, i64, i64, Vec<u8>, Vec<u8>, i64);

fn read_outbox(connection: &Connection) -> PortabilityResult<Vec<OutboxRow>> {
    collect(
        connection,
        concat!(
            "SELECT outbox_seq, accepted_batch_id, accept_seq_start, accept_seq_end, ",
            "canonical_revision, event_kind_mask, payload_digest, created_at ",
            "FROM projection_outbox ORDER BY outbox_seq"
        ),
        |row| -> rusqlite::Result<OutboxRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        },
        |raw| {
            let (
                outbox_seq,
                accepted_batch_id,
                accept_seq_start,
                accept_seq_end,
                canonical_revision,
                event_kind_mask,
                payload_digest,
                created_at,
            ) = raw;
            Ok(OutboxRow {
                outbox_seq: nonnegative(outbox_seq, "outbox sequence")?,
                accepted_batch_id: uuid_text(&accepted_batch_id)?,
                accept_seq_start: nonnegative(accept_seq_start, "outbox acceptance start")?,
                accept_seq_end: nonnegative(accept_seq_end, "outbox acceptance end")?,
                canonical_revision: nonnegative(canonical_revision, "canonical revision")?,
                event_kind_mask: encode_hex(&event_kind_mask),
                payload_digest: encode_hex(&payload_digest),
                created_at_unix_ms: created_at,
            })
        },
    )
}

type ReceiptRaw = (
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Option<i64>,
    i64,
    i64,
    Vec<u8>,
    i64,
);

fn read_command_receipts(connection: &Connection) -> PortabilityResult<Vec<CommandReceiptRow>> {
    collect(
        connection,
        concat!(
            "SELECT client_instance_id, idempotency_key, request_hash, expected_revision, ",
            "committed_revision, length(response_bytes), response_hash, created_at ",
            "FROM command_receipt ORDER BY client_instance_id, idempotency_key"
        ),
        |row| -> rusqlite::Result<ReceiptRaw> {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        },
        |raw| {
            let (
                client_instance_id,
                idempotency_key,
                request_hash,
                expected_revision,
                committed_revision,
                response_length,
                response_hash,
                created_at,
            ) = raw;
            Ok(CommandReceiptRow {
                client_instance_id: encode_hex(&client_instance_id),
                idempotency_key: encode_hex(&idempotency_key),
                request_hash: encode_hex(&request_hash),
                expected_revision: expected_revision
                    .map(|value| nonnegative(value, "expected revision"))
                    .transpose()?,
                committed_revision: nonnegative(committed_revision, "committed revision")?,
                response_sha256: encode_hex(&response_hash),
                response_byte_length: nonnegative(response_length, "receipt response length")?,
                created_at_unix_ms: created_at,
            })
        },
    )
}

/// Reads every registered artifact descriptor in canonical identifier order.
///
/// The descriptors are revalidated by the same domain rules that admitted them,
/// so a corrupted normalized row can never silently become an object reference.
///
/// The locator each descriptor carries is the *current* one: a `P2-K5` rotation
/// appends its move to `artifact_descriptor_migration` rather than editing the
/// signed row, and this resolves that chain. Every caller — backup, restore,
/// export — therefore names the object the profile can actually open. A profile
/// with no migrations, and the whole plaintext lane, resolve to the signed
/// locator unchanged.
pub fn read_artifact_descriptors(
    database: &CanonicalDatabase,
) -> PortabilityResult<Vec<ArtifactDescriptor>> {
    let rows = read_artifacts(database.connection())?;
    let mut descriptors = Vec::with_capacity(rows.len());
    for row in &rows {
        descriptors.push(artifact_descriptor(row)?);
    }
    academic_store::descriptor_migration::resolve_with_stored_migrations(
        database.connection(),
        &mut descriptors,
    )
    .map_err(PortabilityError::Store)?;
    Ok(descriptors)
}

fn artifact_descriptor(row: &ArtifactRow) -> PortabilityResult<ArtifactDescriptor> {
    let descriptor = ArtifactDescriptor {
        id: parse_id::<ArtifactId>(&row.artifact_id)?,
        content_digest: parse_digest(&row.content_digest)?,
        media_type: MediaType::parse(row.media_type.clone())?,
        byte_length: row.byte_length,
        domain_id: parse_id::<DomainId>(&row.domain_id)?,
        confidentiality: parse_confidentiality(&row.confidentiality)?,
        retention_class: parse_retention(&row.retention_class)?,
        permission_lineage_id: parse_id::<PermissionLineageId>(&row.permission_lineage_id)?,
        format_version: u16::try_from(row.format_version).map_err(|_| {
            PortabilityError::mismatch("artifact format version", "1..=65535", row.format_version)
        })?,
        vault_locator: VaultLocator::from_str(&format!("locator:v1:{}", row.vault_locator))?,
        evidence_representations: row
            .representations
            .iter()
            .map(|representation| {
                Ok(ArtifactRepresentation {
                    locator: representation.locator.clone(),
                    content_digest: parse_digest(&representation.content_digest)?,
                    byte_length: representation.byte_length,
                })
            })
            .collect::<PortabilityResult<Vec<_>>>()?,
    };
    descriptor.validate()?;
    Ok(descriptor)
}

/// Report of one complete signed replay over a canonical database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayReport {
    pub verified_batches: u64,
    pub verified_events: u64,
    pub device_heads: u64,
}

/// Re-verifies every stored signed envelope against independent trust anchors.
///
/// The signing key carried inside a stored envelope is never trusted on its own:
/// each batch must match a caller-supplied [`DeviceAuthorization`], exactly as
/// fixture verification does. The replay then re-derives every normalized event
/// coordinate, canonical payload digest, acceptance range, and device origin
/// chain and compares them with the stored rows, so a tampered database fails
/// before any restore destination can be published.
pub fn replay_signed_batches(
    database: &CanonicalDatabase,
    authorizations: &[DeviceAuthorization],
) -> PortabilityResult<ReplayReport> {
    let connection = database.connection();
    let anchors: BTreeMap<[u8; 16], &DeviceAuthorization> = authorizations
        .iter()
        .map(|authorization| (*authorization.device_id().as_bytes(), authorization))
        .collect();
    let batches = read_batches(connection)?;
    let events = read_events(connection)?;
    let mut events_by_batch: BTreeMap<&str, Vec<&EventRow>> = BTreeMap::new();
    for event in &events {
        events_by_batch
            .entry(event.batch_id.as_str())
            .or_default()
            .push(event);
    }

    let mut ordered: Vec<&BatchRow> = batches.iter().collect();
    ordered.sort_by_key(|batch| batch.accept_seq_start);
    let mut expected_accept_seq = 1_u64;
    let mut device_chain: BTreeMap<String, DeviceChain> = BTreeMap::new();
    let mut verified_events = 0_u64;

    for batch in ordered {
        let envelope = read_batch_envelope(connection, &batch.batch_id)?;
        let device_bytes = uuid_bytes(&batch.device_id)?;
        let authorization = anchors.get(&device_bytes).copied().ok_or_else(|| {
            PortabilityError::MissingAuthorization {
                device_id: batch.device_id.clone(),
            }
        })?;
        let verified = verify_signed_batch(&envelope, authorization)?;
        let semantic = verified.batch();

        require(
            "batch identifier",
            &batch.batch_id,
            &semantic.batch_id.to_string(),
        )?;
        require(
            "batch device",
            &batch.device_id,
            &semantic.device_id.to_string(),
        )?;
        require_u64(
            "batch origin start",
            batch.origin_seq_start,
            semantic.origin_seq_start,
        )?;
        require_u64(
            "batch origin end",
            batch.origin_seq_end,
            semantic.origin_seq_end,
        )?;
        require_i64(
            "batch origin creation",
            batch.origin_created_at_unix_ms,
            semantic.origin_created_at.value(),
        )?;
        // The stored column is the version the batch was AUTHENTICATED as, not
        // the version reading it upcasts to. Those coincide only while the
        // writer version is also the newest readable source version; a v2
        // envelope read by a v3 reader has source 2 and batch 3.
        require_u64(
            "batch schema version",
            u64::from(batch.event_schema_version),
            u64::from(verified.source_schema_version()),
        )?;
        require(
            "batch envelope digest",
            &batch.envelope_sha256,
            &encode_hex(verified.envelope_hash().as_bytes().as_slice()),
        )?;
        require(
            "batch payload digest",
            &batch.deterministic_payload_sha256,
            &encode_hex(verified.payload_hash().as_bytes().as_slice()),
        )?;
        require(
            "batch signature",
            &batch.signature,
            &encode_hex(verified.signature_bytes()),
        )?;
        require(
            "batch signing key",
            &batch.signing_public_key,
            &encode_hex(authorization.verifying_key().as_bytes().as_slice()),
        )?;
        require_u64(
            "batch envelope byte length",
            batch.envelope_byte_length,
            count_of(envelope.len()),
        )?;
        require_u64(
            "batch payload byte length",
            batch.deterministic_payload_byte_length,
            count_of(verified.source_payload().len()),
        )?;
        let expected_previous = semantic
            .previous_batch_hash
            .map(|hash| encode_hex(hash.as_bytes().as_slice()));
        if batch.previous_envelope_sha256 != expected_previous {
            return Err(PortabilityError::replay(
                "batch previous-envelope link",
                batch.batch_id.clone(),
            ));
        }

        if batch.accept_seq_start != expected_accept_seq {
            return Err(PortabilityError::replay(
                "acceptance sequence contiguity",
                format!(
                    "batch {} starts at {} but {expected_accept_seq} was expected",
                    batch.batch_id, batch.accept_seq_start
                ),
            ));
        }
        let event_count = count_of(semantic.events.len());
        let expected_end = batch
            .accept_seq_start
            .checked_add(event_count.saturating_sub(1))
            .ok_or(PortabilityError::IntegerOutOfRange {
                subject: "acceptance range end",
                value: i64::MAX,
            })?;
        require_u64("batch acceptance end", batch.accept_seq_end, expected_end)?;
        expected_accept_seq =
            expected_end
                .checked_add(1)
                .ok_or(PortabilityError::IntegerOutOfRange {
                    subject: "next acceptance sequence",
                    value: i64::MAX,
                })?;

        let stored_events = events_by_batch
            .get(batch.batch_id.as_str())
            .cloned()
            .unwrap_or_default();
        if count_of(stored_events.len()) != event_count {
            return Err(PortabilityError::replay(
                "batch event count",
                batch.batch_id.clone(),
            ));
        }
        let mut stored_by_id: BTreeMap<&str, &EventRow> = BTreeMap::new();
        for event in stored_events {
            stored_by_id.insert(event.event_id.as_str(), event);
        }
        for (offset, event) in semantic.events.iter().enumerate() {
            let identifier = event.id.to_string();
            let stored = stored_by_id
                .get(identifier.as_str())
                .copied()
                .ok_or_else(|| {
                    PortabilityError::replay("stored event is absent", identifier.clone())
                })?;
            require_u64("event origin sequence", stored.origin_seq, event.origin_seq)?;
            require_i64(
                "event observation time",
                stored.origin_observed_at_unix_ms,
                event.origin_observed_at.value(),
            )?;
            require(
                "event actor kind",
                &stored.actor_kind,
                event.actor.kind_name(),
            )?;
            require(
                "event domain",
                &stored.domain_id,
                &event.domain_id.to_string(),
            )?;
            require("event kind", &stored.event_kind, event_kind(&event.payload))?;
            let canonical_payload = encode_canonical_event_payload(event)?;
            require(
                "event canonical payload digest",
                &stored.canonical_payload_sha256,
                &digest_hex(&canonical_payload),
            )?;
            require_u64(
                "event canonical payload length",
                stored.canonical_payload_byte_length,
                count_of(canonical_payload.len()),
            )?;
            let canonical_actor = encode_canonical_actor(&event.actor)?;
            require(
                "event canonical actor digest",
                &stored.actor_canonical_sha256,
                &digest_hex(&canonical_actor),
            )?;
            require_u64(
                "event acceptance sequence",
                stored.accept_seq,
                batch.accept_seq_start.saturating_add(count_of(offset)),
            )?;
            verified_events = verified_events.saturating_add(1);
        }

        let chain = device_chain
            .entry(batch.device_id.clone())
            .or_insert_with(DeviceChain::new);
        if chain.next_origin_seq != batch.origin_seq_start {
            return Err(PortabilityError::replay(
                "device origin-chain gap",
                format!(
                    "device {} expected origin {} but batch {} starts at {}",
                    batch.device_id, chain.next_origin_seq, batch.batch_id, batch.origin_seq_start
                ),
            ));
        }
        let expected_link = if batch.origin_seq_start == 1 {
            None
        } else {
            Some(chain.head_envelope_sha256.clone())
        };
        if batch.previous_envelope_sha256 != expected_link {
            return Err(PortabilityError::replay(
                "device origin-chain fork",
                batch.batch_id.clone(),
            ));
        }
        chain.next_origin_seq = batch.origin_seq_end.saturating_add(1);
        chain
            .head_envelope_sha256
            .clone_from(&batch.envelope_sha256);
    }

    let heads = read_device_heads(connection)?;
    for head in &heads {
        let chain = device_chain.get(&head.device_id).ok_or_else(|| {
            PortabilityError::replay("device head without batches", head.device_id.clone())
        })?;
        require_u64(
            "device head origin",
            head.next_origin_seq,
            chain.next_origin_seq,
        )?;
        require(
            "device head envelope digest",
            &head.head_envelope_sha256,
            &chain.head_envelope_sha256,
        )?;
    }
    if heads.len() != device_chain.len() {
        return Err(PortabilityError::replay(
            "device head coverage",
            format!("{} heads for {} devices", heads.len(), device_chain.len()),
        ));
    }

    Ok(ReplayReport {
        verified_batches: count_of(batches.len()),
        verified_events,
        device_heads: count_of(heads.len()),
    })
}

#[derive(Debug)]
struct DeviceChain {
    next_origin_seq: u64,
    head_envelope_sha256: String,
}

impl DeviceChain {
    const fn new() -> Self {
        Self {
            next_origin_seq: 1,
            head_envelope_sha256: String::new(),
        }
    }
}

/// Reads the exact original signed envelope bytes for one batch identifier.
pub fn read_batch_envelope(connection: &Connection, batch_id: &str) -> PortabilityResult<Vec<u8>> {
    let bytes = uuid_bytes(batch_id)?;
    connection
        .query_row(
            "SELECT signed_envelope FROM ledger_batch WHERE batch_id = ?1",
            [bytes.as_slice()],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map_err(PortabilityError::from)
}

const fn event_kind(payload: &EventPayload) -> &'static str {
    payload.kind()
}

fn digest_hex(bytes: &[u8]) -> String {
    encode_hex(ContentDigest::sha256(bytes).as_bytes().as_slice())
}

fn decode_locator(kind: &str, bytes: &[u8]) -> PortabilityResult<EvidenceLocator> {
    let locator = decode_canonical_evidence_locator(bytes)?;
    let actual = match &locator {
        EvidenceLocator::TextBytes { .. } => "TEXT_BYTES",
        EvidenceLocator::Page { .. } => "PAGE",
        EvidenceLocator::TranscriptTime { .. } => "TRANSCRIPT_TIME",
        EvidenceLocator::RepositoryBytes { .. } => "REPOSITORY_BYTES",
    };
    if actual == kind {
        Ok(locator)
    } else {
        Err(PortabilityError::mismatch(
            "evidence locator kind",
            kind,
            actual,
        ))
    }
}

fn parse_confidentiality(value: &str) -> PortabilityResult<Confidentiality> {
    match value {
        "PUBLIC" => Ok(Confidentiality::Public),
        "PERSONAL" => Ok(Confidentiality::Personal),
        "RESTRICTED" => Ok(Confidentiality::Restricted),
        "SECRET" => Ok(Confidentiality::Secret),
        other => Err(PortabilityError::mismatch(
            "artifact confidentiality",
            "a registered confidentiality",
            other,
        )),
    }
}

fn parse_retention(value: &str) -> PortabilityResult<RetentionClass> {
    match value {
        "EPHEMERAL" => Ok(RetentionClass::Ephemeral),
        "COURSE_TERM" => Ok(RetentionClass::CourseTerm),
        "USER_MANAGED" => Ok(RetentionClass::UserManaged),
        "LEGAL_HOLD" => Ok(RetentionClass::LegalHold),
        other => Err(PortabilityError::mismatch(
            "artifact retention class",
            "a registered retention class",
            other,
        )),
    }
}

fn parse_digest(hex: &str) -> PortabilityResult<ContentDigest> {
    ContentDigest::from_str(&format!("sha256:{hex}")).map_err(PortabilityError::from)
}

/// Parses one security-domain identifier out of a manifest field.
#[cfg(feature = "encrypted-portability")]
pub(crate) fn parse_domain_id(text: &str) -> PortabilityResult<DomainId> {
    parse_id(text)
}

fn parse_id<T>(text: &str) -> PortabilityResult<T>
where
    T: FromStr<Err = DomainError>,
{
    T::from_str(text).map_err(PortabilityError::from)
}

fn collect<Raw, Row, Read, Map>(
    connection: &Connection,
    sql: &str,
    read: Read,
    map: Map,
) -> PortabilityResult<Vec<Row>>
where
    Read: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Raw>,
    Map: Fn(Raw) -> PortabilityResult<Row>,
{
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([], read)?;
    let mut mapped = Vec::new();
    for row in rows {
        mapped.push(map(row?)?);
    }
    Ok(mapped)
}

fn require(subject: &'static str, expected: &str, actual: &str) -> PortabilityResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(PortabilityError::mismatch(subject, expected, actual))
    }
}

fn require_u64(subject: &'static str, expected: u64, actual: u64) -> PortabilityResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(PortabilityError::mismatch(subject, expected, actual))
    }
}

fn require_i64(subject: &'static str, expected: i64, actual: i64) -> PortabilityResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(PortabilityError::mismatch(subject, expected, actual))
    }
}

fn nonnegative(value: i64, subject: &'static str) -> PortabilityResult<u64> {
    u64::try_from(value).map_err(|_| PortabilityError::IntegerOutOfRange { subject, value })
}

fn bounded_u32(value: i64, subject: &'static str) -> PortabilityResult<u32> {
    u32::try_from(value).map_err(|_| PortabilityError::IntegerOutOfRange { subject, value })
}

pub(crate) fn uuid_text(bytes: &[u8]) -> PortabilityResult<String> {
    let bytes: [u8; 16] = bytes
        .try_into()
        .map_err(|_| PortabilityError::mismatch("canonical identifier length", 16, bytes.len()))?;
    let hex = encode_hex(&bytes);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

pub(crate) fn uuid_bytes(text: &str) -> PortabilityResult<[u8; 16]> {
    let compact = text.replace('-', "");
    let decoded = decode_hex(&compact)
        .ok_or_else(|| PortabilityError::mismatch("canonical identifier", "hexadecimal", text))?;
    decoded
        .try_into()
        .map_err(|_| PortabilityError::mismatch("canonical identifier length", 16, text.len()))
}

#[cfg(all(test, not(feature = "encrypted-portability")))]
mod tests {
    use super::*;

    #[test]
    fn identifier_text_and_bytes_round_trip() -> PortabilityResult<()> {
        let bytes = [
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ];
        let text = uuid_text(&bytes)?;
        assert_eq!(text, "01900000-0000-7000-8000-000000000001");
        assert_eq!(uuid_bytes(&text)?, bytes);
        Ok(())
    }

    #[test]
    fn phase1_policy_block_rejects_production_posture() {
        let mut policy = PolicyBlock::phase1();
        assert!(policy.require_phase1().is_ok());
        policy.production_data_allowed = true;
        assert!(policy.require_phase1().is_err());
    }
}
