//! Forward-only, pre-listen store migration and schema identity verification.

use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};

use crate::{
    PHASE1_STORAGE_POLICY, SQLITE_APPLICATION_ID, STORE_FORMAT_UUID, STORE_SCHEMA_SEMVER,
    STORE_SCHEMA_VERSION,
    connection::{
        PragmaSnapshot, configure_migration_connection, read_pragma_snapshot, verify_fts5,
        verify_migration_pragmas, verify_writer_pragmas,
    },
    error::{StoreError, StoreResult},
};

/// First and only S1 migration, embedded byte-for-byte from the ordered migration directory.
pub const MIGRATION_0001_SQL: &str = include_str!("../../../migrations/store/0001_phase1_core.sql");

/// Result of invoking the forward-only migration runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationStatus {
    /// A new database advanced from version zero to version one.
    Applied,
    /// The exact version-one identity already existed and no migration SQL ran.
    AlreadyCurrent,
}

/// Exact singleton identity stored alongside SQLite's two numeric identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIdentity {
    /// Stable binary format UUID.
    pub format_uuid: [u8; 16],
    /// Physical schema version repeated in the singleton.
    pub schema_version: u32,
    /// Semantic store schema version.
    pub schema_semver: String,
    /// Minimum reader protocol `(major, minor)`.
    pub minimum_reader_protocol: (u32, u32),
    /// Minimum writer protocol `(major, minor)`.
    pub minimum_writer_protocol: (u32, u32),
    /// Synthetic-only data policy.
    pub data_policy: String,
    /// Plaintext temporary storage mode.
    pub storage_mode: String,
    /// Encryption declaration, always `NONE` in S1.
    pub storage_encryption: String,
    /// Whether production input is permitted, always false in S1.
    pub production_data_allowed: bool,
    /// Product network declaration, always `NONE` in S1.
    pub product_network: String,
    /// Digest of the binary/build that created the schema.
    pub creating_build_digest: [u8; 32],
    /// Creation time in Unix milliseconds.
    pub created_at_unix_ms: i64,
}

/// Migrates only a new database or verifies the exact already-current schema.
///
/// This function is the maintenance connection: callers must invoke it before any
/// daemon listener exists. It deliberately does not install the product authorizer.
pub fn migrate_pre_listen(
    database_path: &Path,
    creating_build_digest: [u8; 32],
) -> StoreResult<MigrationStatus> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut connection = Connection::open_with_flags(database_path, flags)?;
    migrate_open_connection_pre_listen(&mut connection, creating_build_digest)
}

/// Migrates or verifies an already-opened maintenance connection.
///
/// This is the narrow boundary used by evidence-only encrypted-store probes:
/// an encrypted caller must apply its key as the first SQLite statement after
/// opening the handle, then call this function. The exact S1 connection policy,
/// migration SQL, identity checks, and integrity checks remain centralized here.
/// Product callers should normally use [`migrate_pre_listen`] instead.
pub fn migrate_open_connection_pre_listen(
    connection: &mut Connection,
    creating_build_digest: [u8; 32],
) -> StoreResult<MigrationStatus> {
    configure_migration_connection(connection)?;
    let before = read_pragma_snapshot(connection)?;
    verify_migration_pragmas(&before)?;
    verify_fts5(connection)?;
    connection.execute_batch("PRAGMA locking_mode = EXCLUSIVE;")?;
    let locking_mode = connection
        .query_row("PRAGMA locking_mode", [], |row| row.get::<_, String>(0))?
        .to_ascii_lowercase();
    if locking_mode != "exclusive" {
        return Err(StoreError::PragmaMismatch {
            pragma: "locking_mode",
            expected: "exclusive".to_owned(),
            actual: locking_mode,
        });
    }
    verify_integrity(connection)?;
    if before.application_id == i64::from(SQLITE_APPLICATION_ID)
        && before.user_version > i64::from(STORE_SCHEMA_VERSION)
    {
        return Err(StoreError::NewerSchema {
            found: observed_version_for_error(before.user_version),
            supported: STORE_SCHEMA_VERSION,
        });
    }
    if before.application_id == i64::from(SQLITE_APPLICATION_ID)
        && before.user_version == i64::from(STORE_SCHEMA_VERSION)
    {
        verify_writer_pragmas(&before)?;
        verify_current_schema(connection, &before)?;
        return Ok(MigrationStatus::AlreadyCurrent);
    }
    if before.application_id != 0 || before.user_version != 0 || user_table_count(connection)? != 0
    {
        return Err(StoreError::UnsupportedMigrationState {
            application_id: before.application_id,
            user_version: before.user_version,
        });
    }

    let created_at_unix_ms = unix_time_millis()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    transaction.execute_batch(MIGRATION_0001_SQL)?;
    transaction.execute(
        "INSERT INTO schema_meta (\
             singleton, format_uuid, schema_version, schema_semver,\
             minimum_reader_protocol_major, minimum_reader_protocol_minor,\
             minimum_writer_protocol_major, minimum_writer_protocol_minor,\
             data_policy, storage_mode, storage_encryption,\
             production_data_allowed, product_network, creating_build_digest, created_at_unix_ms\
         ) VALUES (1, ?1, ?2, ?3, 1, 0, 1, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            STORE_FORMAT_UUID.as_slice(),
            i64::from(STORE_SCHEMA_VERSION),
            STORE_SCHEMA_SEMVER,
            PHASE1_STORAGE_POLICY.data_policy,
            PHASE1_STORAGE_POLICY.storage_mode,
            PHASE1_STORAGE_POLICY.storage_encryption,
            PHASE1_STORAGE_POLICY.production_data_allowed,
            PHASE1_STORAGE_POLICY.product_network,
            creating_build_digest.as_slice(),
            created_at_unix_ms,
        ],
    )?;
    transaction.pragma_update(None, "application_id", SQLITE_APPLICATION_ID)?;
    transaction.pragma_update(None, "user_version", STORE_SCHEMA_VERSION)?;
    transaction.commit()?;

    let after = read_pragma_snapshot(connection)?;
    verify_writer_pragmas(&after)?;
    verify_current_schema(connection, &after)?;
    Ok(MigrationStatus::Applied)
}

pub(crate) fn verify_current_schema(
    connection: &Connection,
    pragmas: &PragmaSnapshot,
) -> StoreResult<()> {
    let identity = read_schema_identity(connection)?;
    verify_schema_identity(&identity, pragmas)?;
    verify_schema_shape(connection)?;
    verify_integrity(connection)
}

/// Reads the singleton without assuming that its checks are sufficient by themselves.
pub fn read_schema_identity(connection: &Connection) -> StoreResult<SchemaIdentity> {
    let raw = connection.query_row(
        concat!(
            "SELECT format_uuid, schema_version, schema_semver, ",
            "minimum_reader_protocol_major, minimum_reader_protocol_minor, ",
            "minimum_writer_protocol_major, minimum_writer_protocol_minor, ",
            "data_policy, storage_mode, storage_encryption, production_data_allowed, ",
            "product_network, creating_build_digest, created_at_unix_ms ",
            "FROM schema_meta WHERE singleton = 1"
        ),
        [],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, Vec<u8>>(12)?,
                row.get::<_, i64>(13)?,
            ))
        },
    )?;
    Ok(SchemaIdentity {
        format_uuid: fixed_bytes::<16>("format_uuid", raw.0)?,
        schema_version: nonnegative_u32("schema_version", raw.1)?,
        schema_semver: raw.2,
        minimum_reader_protocol: (
            nonnegative_u32("minimum_reader_protocol_major", raw.3)?,
            nonnegative_u32("minimum_reader_protocol_minor", raw.4)?,
        ),
        minimum_writer_protocol: (
            nonnegative_u32("minimum_writer_protocol_major", raw.5)?,
            nonnegative_u32("minimum_writer_protocol_minor", raw.6)?,
        ),
        data_policy: raw.7,
        storage_mode: raw.8,
        storage_encryption: raw.9,
        production_data_allowed: raw.10 != 0,
        product_network: raw.11,
        creating_build_digest: fixed_bytes::<32>("creating_build_digest", raw.12)?,
        created_at_unix_ms: raw.13,
    })
}

/// Verifies that `application_id`, `user_version`, and `schema_meta` are one identity.
pub fn verify_schema_identity(
    identity: &SchemaIdentity,
    pragmas: &PragmaSnapshot,
) -> StoreResult<()> {
    identity_exact(
        "application_id",
        SQLITE_APPLICATION_ID.to_string(),
        pragmas.application_id.to_string(),
    )?;
    identity_exact(
        "user_version",
        STORE_SCHEMA_VERSION.to_string(),
        pragmas.user_version.to_string(),
    )?;
    identity_exact(
        "schema_meta.schema_version",
        STORE_SCHEMA_VERSION.to_string(),
        identity.schema_version.to_string(),
    )?;
    identity_exact(
        "schema_meta.format_uuid",
        hex_bytes(&STORE_FORMAT_UUID),
        hex_bytes(&identity.format_uuid),
    )?;
    identity_exact(
        "schema_meta.schema_semver",
        STORE_SCHEMA_SEMVER.to_owned(),
        identity.schema_semver.clone(),
    )?;
    identity_exact(
        "schema_meta.minimum_reader_protocol",
        "1.0".to_owned(),
        format!(
            "{}.{}",
            identity.minimum_reader_protocol.0, identity.minimum_reader_protocol.1
        ),
    )?;
    identity_exact(
        "schema_meta.minimum_writer_protocol",
        "1.0".to_owned(),
        format!(
            "{}.{}",
            identity.minimum_writer_protocol.0, identity.minimum_writer_protocol.1
        ),
    )?;
    identity_exact(
        "schema_meta.data_policy",
        PHASE1_STORAGE_POLICY.data_policy.to_owned(),
        identity.data_policy.clone(),
    )?;
    identity_exact(
        "schema_meta.storage_mode",
        PHASE1_STORAGE_POLICY.storage_mode.to_owned(),
        identity.storage_mode.clone(),
    )?;
    identity_exact(
        "schema_meta.storage_encryption",
        PHASE1_STORAGE_POLICY.storage_encryption.to_owned(),
        identity.storage_encryption.clone(),
    )?;
    identity_exact(
        "schema_meta.production_data_allowed",
        "false".to_owned(),
        identity.production_data_allowed.to_string(),
    )?;
    identity_exact(
        "schema_meta.product_network",
        PHASE1_STORAGE_POLICY.product_network.to_owned(),
        identity.product_network.clone(),
    )?;
    if identity.created_at_unix_ms < 0 {
        return Err(StoreError::SchemaIdentityMismatch {
            component: "schema_meta.created_at_unix_ms",
            expected: "non-negative".to_owned(),
            actual: identity.created_at_unix_ms.to_string(),
        });
    }
    Ok(())
}

/// Converts an unsigned domain integer into SQLite's signed integer domain.
pub fn checked_sqlite_integer(value: u64) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::UnsignedIntegerOverflow(value))
}

fn verify_integrity(connection: &Connection) -> StoreResult<()> {
    let integrity =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?;
    if integrity != "ok" {
        return Err(StoreError::SchemaIdentityMismatch {
            component: "integrity_check",
            expected: "ok".to_owned(),
            actual: integrity,
        });
    }
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    if rows.next()?.is_some() {
        return Err(StoreError::SchemaIdentityMismatch {
            component: "foreign_key_check",
            expected: "no rows".to_owned(),
            actual: "violation present".to_owned(),
        });
    }
    Ok(())
}

fn verify_schema_shape(connection: &Connection) -> StoreResult<()> {
    const TABLES: &[&str] = &[
        "schema_meta",
        "ledger_batch",
        "ledger_event",
        "scope",
        "artifact_descriptor",
        "artifact_representation",
        "evidence_item",
        "claim",
        "claim_evidence",
        "claim_relation",
        "user_decision",
        "projection_outbox",
        "command_receipt",
        "replica_state",
        "device_head",
        "projection_cursor",
        "projection_active",
        "ingest_lease",
    ];
    const INDEXES: &[&str] = &[
        "idx_ledger_batch_device_range",
        "idx_ledger_event_batch_origin",
        "idx_ledger_event_domain_accept",
        "idx_scope_domain",
        "idx_artifact_domain",
        "idx_evidence_artifact",
        "idx_claim_resolution",
        "idx_claim_assertion_event",
        "idx_claim_evidence_evidence",
        "idx_claim_relation_target",
        "idx_user_decision_slot",
        "idx_projection_outbox_accept",
        "idx_ingest_lease_expiry",
    ];
    const TRIGGERS: &[&str] = &[
        "guard_schema_meta_update",
        "guard_schema_meta_delete",
        "guard_ledger_batch_update",
        "guard_ledger_batch_delete",
        "guard_ledger_event_update",
        "guard_ledger_event_delete",
        "guard_scope_update",
        "guard_scope_delete",
        "guard_artifact_descriptor_update",
        "guard_artifact_descriptor_delete",
        "guard_artifact_representation_update",
        "guard_artifact_representation_delete",
        "guard_evidence_item_update",
        "guard_evidence_item_delete",
        "guard_claim_update",
        "guard_claim_delete",
        "guard_claim_evidence_update",
        "guard_claim_evidence_delete",
        "guard_claim_relation_update",
        "guard_claim_relation_delete",
        "guard_user_decision_update",
        "guard_user_decision_delete",
        "guard_projection_outbox_update",
        "guard_projection_outbox_delete",
        "guard_command_receipt_update",
        "guard_command_receipt_delete",
    ];

    verify_object_set(connection, "table", TABLES)?;
    for table in TABLES {
        let strict = connection.query_row(
            "SELECT strict FROM pragma_table_list WHERE schema = 'main' AND name = ?1",
            [table],
            |row| row.get::<_, i64>(0),
        )?;
        if strict != 1 {
            return Err(StoreError::SchemaIdentityMismatch {
                component: "schema.strict_table",
                expected: format!("{table}=1"),
                actual: format!("{table}={strict}"),
            });
        }
    }
    verify_object_set(connection, "index", INDEXES)?;
    verify_object_set(connection, "trigger", TRIGGERS)
}

fn verify_object_set(
    connection: &Connection,
    object_type: &'static str,
    expected_names: &[&str],
) -> StoreResult<()> {
    for name in expected_names {
        let count = connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = ?1 AND name = ?2",
            [object_type, name],
            |row| row.get::<_, i64>(0),
        )?;
        if count != 1 {
            return Err(StoreError::SchemaIdentityMismatch {
                component: "schema.required_object",
                expected: format!("one {object_type} named {name}"),
                actual: format!("{count} matching objects"),
            });
        }
    }
    let actual_count = connection.query_row(
        "SELECT count(*) FROM sqlite_schema \
         WHERE type = ?1 AND name NOT LIKE 'sqlite_%'",
        [object_type],
        |row| row.get::<_, i64>(0),
    )?;
    let expected_count =
        i64::try_from(expected_names.len()).map_err(|_| StoreError::SchemaIdentityMismatch {
            component: "schema.object_count",
            expected: "object count fitting signed 64-bit".to_owned(),
            actual: expected_names.len().to_string(),
        })?;
    if actual_count == expected_count {
        Ok(())
    } else {
        Err(StoreError::SchemaIdentityMismatch {
            component: "schema.object_count",
            expected: format!("{expected_count} {object_type} objects"),
            actual: format!("{actual_count} {object_type} objects"),
        })
    }
}

fn user_table_count(connection: &Connection) -> StoreResult<i64> {
    connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
}

fn unix_time_millis() -> StoreResult<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        StoreError::InvalidProfileState {
            path: Path::new("<system-clock>").to_path_buf(),
            reason: "system time is before the Unix epoch",
        }
    })?;
    i64::try_from(duration.as_millis()).map_err(|_| StoreError::InvalidProfileState {
        path: Path::new("<system-clock>").to_path_buf(),
        reason: "system time does not fit SQLite signed milliseconds",
    })
}

fn fixed_bytes<const N: usize>(component: &'static str, bytes: Vec<u8>) -> StoreResult<[u8; N]> {
    let actual_length = bytes.len();
    bytes
        .try_into()
        .map_err(|_| StoreError::SchemaIdentityMismatch {
            component,
            expected: format!("{N} bytes"),
            actual: format!("{actual_length} bytes"),
        })
}

fn nonnegative_u32(component: &'static str, value: i64) -> StoreResult<u32> {
    u32::try_from(value).map_err(|_| StoreError::SchemaIdentityMismatch {
        component,
        expected: "unsigned 32-bit integer".to_owned(),
        actual: value.to_string(),
    })
}

fn observed_version_for_error(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn identity_exact(component: &'static str, expected: String, actual: String) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::SchemaIdentityMismatch {
            component,
            expected,
            actual,
        })
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
