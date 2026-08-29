//! Forward-only, pre-listen store migration and schema identity verification.

use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};

#[cfg(not(feature = "sqlcipher-store"))]
use crate::PHASE1_STORAGE_POLICY;
use crate::{
    SQLITE_APPLICATION_ID, STORE_FORMAT_UUID, STORE_MINIMUM_READER_PROTOCOL,
    STORE_MINIMUM_WRITER_PROTOCOL, STORE_SCHEMA_SEMVER, STORE_SCHEMA_VERSION,
    connection::{
        PragmaSnapshot, configure_migration_connection, disable_checkpoint_on_close,
        enable_checkpoint_on_close, read_pragma_snapshot, verify_fts5, verify_migration_pragmas,
        verify_writer_pragmas,
    },
    error::{StoreError, StoreResult},
    schema_fingerprint::{user_schema_object_count, verify_store_schema_fingerprint},
};

/// First and only S1 migration, embedded byte-for-byte from the ordered migration directory.
pub const MIGRATION_0001_SQL: &str = include_str!("../../../migrations/store/0001_phase1_core.sql");

/// Typed canonical closure tables for the eighteen event schema v3 registration arms.
///
/// Applies on top of store schema version 2. Migration `0003` establishes that
/// version, its identity, and the encrypted lane; this migration reads and
/// writes no part of that identity, so it extends what a schema-2 profile holds
/// without changing which format that profile is.
///
/// The encrypted lane's [`STORE_MIGRATION_SQL`] ends with it, so an encrypted
/// profile carries these tables from creation and admission fingerprints them.
pub const MIGRATION_0004_SQL: &str =
    include_str!("../../../migrations/store/0004_phase2_canonical_aggregates.sql");

/// `P2-K5`'s typed columns for the `RETENTION_ACTION_RECORDED` aggregate.
///
/// Migration `0004` states the rule this file follows: the v3 registration
/// frame carries no typed aggregate attributes and each aggregate owner adds
/// its own in a later migration. This is that migration for `P2-K5`, and it is
/// what lets a re-sealed object's canonical reference move without an edit to
/// the signed `artifact_descriptor` row.
pub const MIGRATION_0005_SQL: &str =
    include_str!("../../../migrations/store/0005_phase2_descriptor_migration.sql");

/// The Phase 2 encrypted-profile identity migration, embedded byte-for-byte.
///
/// It replaces the Phase 1 identity singleton with the schema-2 one. The
/// encrypted lane applies it; the plaintext lane never does, and
/// [`STORE_MIGRATION_SQL`] is what proves that. It is also available to this
/// crate's own unit tests in either lane, because migration `0004` layers on
/// the schema-2 base and its tests have to build the real one rather than
/// something that resembles it. A `cfg(test)` item reaches no product binary,
/// so the plaintext product graph still carries neither this text nor the
/// SQLCipher build it belongs to.
#[cfg(any(feature = "sqlcipher-store", test))]
pub const MIGRATION_0003_SQL: &str =
    include_str!("../../../migrations/store/0003_phase2_encrypted_identity.sql");

/// The ordered migration set this binary's lane applies to an empty database.
///
/// Both the runtime migration and the reference fingerprint execute exactly
/// this sequence, so the schema authority stays the committed SQL rather than
/// a second hand-maintained description of it. That is also why the set is what
/// decides whether a profile is admitted: the fingerprint is exact structural
/// equality against the reference this sequence produces, never a subset test,
/// so a profile carrying an object this sequence does not create is refused.
#[cfg(not(feature = "sqlcipher-store"))]
pub const STORE_MIGRATION_SQL: &[&str] = &[MIGRATION_0001_SQL];
/// The ordered migration set this binary's lane applies to an empty database.
///
/// `0004` belongs to store schema version 2, not to a version after it: t068
/// section 3.8 has `0003` establish the version and `0004` add the aggregates.
/// Both therefore run in this one creation transaction, and both are inside the
/// admission fingerprint. `0004` stamps no identity of its own, so the frozen
/// schema-2 identity is entirely `0003`'s.
#[cfg(feature = "sqlcipher-store")]
pub const STORE_MIGRATION_SQL: &[&str] = &[
    MIGRATION_0001_SQL,
    MIGRATION_0003_SQL,
    MIGRATION_0004_SQL,
    MIGRATION_0005_SQL,
];

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
    ///
    /// Absent from the schema-2 singleton for the same reason as the two
    /// fields below: t068 section 3.1 emits `data_policy`,
    /// `production_data_allowed`, and `product_network` together and only
    /// after the admission verifier succeeds, so all three are `P2-K6`'s
    /// runtime output and none is a fact about the file.
    #[cfg(not(feature = "sqlcipher-store"))]
    pub data_policy: String,
    /// Plaintext temporary storage mode.
    pub storage_mode: String,
    /// Encryption declaration, always `NONE` in S1.
    pub storage_encryption: String,
    /// Whether production input is permitted, always false in S1.
    #[cfg(not(feature = "sqlcipher-store"))]
    pub production_data_allowed: bool,
    /// Product network declaration, always `NONE` in S1.
    #[cfg(not(feature = "sqlcipher-store"))]
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
    // Admission is deliberately read-only. In particular, journal_mode is not
    // changed to WAL until a database is proven empty or exactly current, so a
    // rejected foreign/tampered database keeps its exact main-database and
    // committed-WAL bytes. Closing a read-write handle would otherwise
    // checkpoint an uncheckpointed WAL into the main database and rewrite it
    // even on the rejection path, so checkpoint-on-close is disabled here and
    // restored only once admission has decided. SQLite's own read and recovery
    // path may still create or refresh the rebuildable `-wal`/`-shm` sidecars;
    // that is outside the claim.
    //
    // A hot `-journal` is the one input this handle cannot leave untouched:
    // SQLite rolls a rollback journal back on the first read of a read-write
    // handle, before any admission statement runs. A rejected input in that
    // state is therefore restored to its last committed main-database bytes
    // and loses its `-journal`; no committed content is lost, and the
    // read-only reader path leaves even that input byte-identical.
    disable_checkpoint_on_close(connection)?;
    let admission = inspect_schema_before_mutation(connection)?;
    verify_fts5(connection)?;
    configure_migration_connection(connection)?;
    let configured = read_pragma_snapshot(connection)?;
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

    if admission == SchemaAdmission::Current {
        verify_writer_pragmas(&configured)?;
        verify_current_schema(connection, &configured)?;
        enable_checkpoint_on_close(connection)?;
        return Ok(MigrationStatus::AlreadyCurrent);
    }
    verify_migration_pragmas(&configured)?;

    let created_at_unix_ms = unix_time_millis()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    for step in STORE_MIGRATION_SQL {
        transaction.execute_batch(step)?;
    }
    insert_schema_identity(&transaction, creating_build_digest, created_at_unix_ms)?;
    transaction.pragma_update(None, "application_id", SQLITE_APPLICATION_ID)?;
    transaction.pragma_update(None, "user_version", STORE_SCHEMA_VERSION)?;
    transaction.commit()?;

    let after = read_pragma_snapshot(connection)?;
    verify_writer_pragmas(&after)?;
    verify_current_schema(connection, &after)?;
    enable_checkpoint_on_close(connection)?;
    Ok(MigrationStatus::Applied)
}

/// Applies migrations `0004` and `0005` to a schema-2 connection without them.
///
/// Profile creation does not come through here: [`STORE_MIGRATION_SQL`] ends
/// with `0005` in the encrypted lane, so a profile carries the aggregates and
/// `P2-K5`'s typed columns from the creation transaction. What is left for this
/// entry point is a schema-2 base assembled from the migration text rather than
/// by that runner, which is how the aggregate suite builds one in either lane.
///
/// The two files are one step here because `0005` is `0004`'s continuation: it
/// adds the typed columns `0004` deliberately left to each aggregate owner, and
/// a base carrying one without the other is not a schema this build admits.
///
/// It is the same narrow, forward-only, pre-listen boundary as
/// [`migrate_open_connection_pre_listen`]: the caller owns opening and keying the
/// handle, and this function owns the migration SQL, the foreign-key discipline
/// that the `ledger_event` rebuild requires on a base that may already hold
/// rows, and the pre-commit integrity checks. It installs no authorizer and must
/// run before any daemon listener exists. Against a connection that already
/// carries `0004` it fails, which is what makes the migration forward-only for
/// a created profile too.
///
/// The migration is a delta on the canonical core, not on the schema identity
/// singleton: it neither reads nor writes `schema_meta`, `application_id`, or
/// `user_version`, so applying it cannot turn one profile format into another.
pub fn apply_aggregate_migration_pre_listen(connection: &mut Connection) -> StoreResult<()> {
    // `PRAGMA foreign_keys` is a no-op inside a transaction, and SQLite's
    // documented table-rebuild procedure needs it off while the old
    // `ledger_event` is dropped and its replacement is renamed into place.
    // `foreign_key_check` inside the transaction is what proves no reference was
    // left dangling, and it runs before the commit.
    connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let applied = apply_aggregate_migration_in_transaction(connection);
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    applied
}

fn apply_aggregate_migration_in_transaction(connection: &mut Connection) -> StoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    transaction.execute_batch(MIGRATION_0004_SQL)?;
    transaction.execute_batch(MIGRATION_0005_SQL)?;
    verify_integrity(&transaction)?;
    transaction.commit()?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaAdmission {
    Empty,
    Current,
}

fn inspect_schema_before_mutation(connection: &Connection) -> StoreResult<SchemaAdmission> {
    let observed = read_pragma_snapshot(connection)?;
    verify_integrity(connection)?;
    if observed.application_id == i64::from(SQLITE_APPLICATION_ID)
        && observed.user_version > i64::from(STORE_SCHEMA_VERSION)
    {
        return Err(StoreError::NewerSchema {
            found: observed_version_for_error(observed.user_version),
            supported: STORE_SCHEMA_VERSION,
        });
    }
    if observed.application_id == i64::from(SQLITE_APPLICATION_ID)
        && observed.user_version == i64::from(STORE_SCHEMA_VERSION)
    {
        verify_current_schema(connection, &observed)?;
        return Ok(SchemaAdmission::Current);
    }
    if observed.application_id == 0
        && observed.user_version == 0
        && user_schema_object_count(connection)? == 0
    {
        return Ok(SchemaAdmission::Empty);
    }
    Err(StoreError::UnsupportedMigrationState {
        application_id: observed.application_id,
        user_version: observed.user_version,
    })
}

pub(crate) fn verify_current_schema(
    connection: &Connection,
    pragmas: &PragmaSnapshot,
) -> StoreResult<()> {
    let identity = read_schema_identity(connection)?;
    verify_schema_identity(&identity, pragmas)?;
    verify_store_schema_fingerprint(connection, STORE_MIGRATION_SQL)?;
    verify_integrity(connection)
}

/// Writes the one schema identity row this lane is allowed to create.
#[cfg(not(feature = "sqlcipher-store"))]
fn insert_schema_identity(
    transaction: &rusqlite::Transaction<'_>,
    creating_build_digest: [u8; 32],
    created_at_unix_ms: i64,
) -> StoreResult<()> {
    transaction.execute(
        "INSERT INTO schema_meta (\
             singleton, format_uuid, schema_version, schema_semver,\
             minimum_reader_protocol_major, minimum_reader_protocol_minor,\
             minimum_writer_protocol_major, minimum_writer_protocol_minor,\
             data_policy, storage_mode, storage_encryption,\
             production_data_allowed, product_network, creating_build_digest, created_at_unix_ms\
         ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            STORE_FORMAT_UUID.as_slice(),
            i64::from(STORE_SCHEMA_VERSION),
            STORE_SCHEMA_SEMVER,
            i64::from(STORE_MINIMUM_READER_PROTOCOL.0),
            i64::from(STORE_MINIMUM_READER_PROTOCOL.1),
            i64::from(STORE_MINIMUM_WRITER_PROTOCOL.0),
            i64::from(STORE_MINIMUM_WRITER_PROTOCOL.1),
            PHASE1_STORAGE_POLICY.data_policy,
            PHASE1_STORAGE_POLICY.storage_mode,
            PHASE1_STORAGE_POLICY.storage_encryption,
            PHASE1_STORAGE_POLICY.production_data_allowed,
            PHASE1_STORAGE_POLICY.product_network,
            creating_build_digest.as_slice(),
            created_at_unix_ms,
        ],
    )?;
    Ok(())
}

/// Writes the one schema identity row this lane is allowed to create.
#[cfg(feature = "sqlcipher-store")]
fn insert_schema_identity(
    transaction: &rusqlite::Transaction<'_>,
    creating_build_digest: [u8; 32],
    created_at_unix_ms: i64,
) -> StoreResult<()> {
    transaction.execute(
        "INSERT INTO schema_meta (\
             singleton, format_uuid, schema_version, schema_semver,\
             minimum_reader_protocol_major, minimum_reader_protocol_minor,\
             minimum_writer_protocol_major, minimum_writer_protocol_minor,\
             storage_mode, storage_encryption,\
             creating_build_digest, created_at_unix_ms\
         ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            STORE_FORMAT_UUID.as_slice(),
            i64::from(STORE_SCHEMA_VERSION),
            STORE_SCHEMA_SEMVER,
            i64::from(STORE_MINIMUM_READER_PROTOCOL.0),
            i64::from(STORE_MINIMUM_READER_PROTOCOL.1),
            i64::from(STORE_MINIMUM_WRITER_PROTOCOL.0),
            i64::from(STORE_MINIMUM_WRITER_PROTOCOL.1),
            crate::cipher::ENCRYPTED_STORE_STORAGE_MODE,
            crate::cipher::ENCRYPTED_STORE_STORAGE_ENCRYPTION,
            creating_build_digest.as_slice(),
            created_at_unix_ms,
        ],
    )?;
    Ok(())
}

/// Reads the singleton without assuming that its checks are sufficient by themselves.
#[cfg(not(feature = "sqlcipher-store"))]
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

/// Reads the singleton without assuming that its checks are sufficient by themselves.
#[cfg(feature = "sqlcipher-store")]
pub fn read_schema_identity(connection: &Connection) -> StoreResult<SchemaIdentity> {
    let raw = connection.query_row(
        concat!(
            "SELECT format_uuid, schema_version, schema_semver, ",
            "minimum_reader_protocol_major, minimum_reader_protocol_minor, ",
            "minimum_writer_protocol_major, minimum_writer_protocol_minor, ",
            "storage_mode, storage_encryption, ",
            "creating_build_digest, created_at_unix_ms ",
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
                row.get::<_, Vec<u8>>(9)?,
                row.get::<_, i64>(10)?,
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
        storage_mode: raw.7,
        storage_encryption: raw.8,
        creating_build_digest: fixed_bytes::<32>("creating_build_digest", raw.9)?,
        created_at_unix_ms: raw.10,
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
        format!(
            "{}.{}",
            STORE_MINIMUM_READER_PROTOCOL.0, STORE_MINIMUM_READER_PROTOCOL.1
        ),
        format!(
            "{}.{}",
            identity.minimum_reader_protocol.0, identity.minimum_reader_protocol.1
        ),
    )?;
    identity_exact(
        "schema_meta.minimum_writer_protocol",
        format!(
            "{}.{}",
            STORE_MINIMUM_WRITER_PROTOCOL.0, STORE_MINIMUM_WRITER_PROTOCOL.1
        ),
        format!(
            "{}.{}",
            identity.minimum_writer_protocol.0, identity.minimum_writer_protocol.1
        ),
    )?;
    verify_storage_declaration(identity)?;
    if identity.created_at_unix_ms < 0 {
        return Err(StoreError::SchemaIdentityMismatch {
            component: "schema_meta.created_at_unix_ms",
            expected: "non-negative".to_owned(),
            actual: identity.created_at_unix_ms.to_string(),
        });
    }
    Ok(())
}

/// Verifies the storage declaration columns this lane pins in the singleton.
#[cfg(not(feature = "sqlcipher-store"))]
fn verify_storage_declaration(identity: &SchemaIdentity) -> StoreResult<()> {
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
    )
}

/// Verifies the storage declaration columns this lane pins in the singleton.
///
/// Both are physical facts about the file. The schema-2 singleton records no
/// policy column at all, so there is nothing here to compare against a posture.
#[cfg(feature = "sqlcipher-store")]
fn verify_storage_declaration(identity: &SchemaIdentity) -> StoreResult<()> {
    identity_exact(
        "schema_meta.storage_mode",
        crate::cipher::ENCRYPTED_STORE_STORAGE_MODE.to_owned(),
        identity.storage_mode.clone(),
    )?;
    identity_exact(
        "schema_meta.storage_encryption",
        crate::cipher::ENCRYPTED_STORE_STORAGE_ENCRYPTION.to_owned(),
        identity.storage_encryption.clone(),
    )
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
