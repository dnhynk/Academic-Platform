//! Exact SQLite connection policy for the one writer and query-only readers.

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Params, Row, Transaction, TransactionBehavior,
    config::DbConfig,
};

use crate::{
    SQLITE_APPLICATION_ID, SQLITE_BUSY_TIMEOUT_MILLIS, STORE_SCHEMA_VERSION,
    authorizer::{install_canonical_authorizer, install_reader_authorizer},
    error::{StoreError, StoreResult},
    migration::verify_current_schema,
};

const SQLITE_SYNCHRONOUS_FULL: i64 = 2;
const SQLITE_TEMP_STORE_MEMORY: i64 = 2;

/// Read-back of every connection PRAGMA fixed by the S1 policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaSnapshot {
    /// SQLite file application identity.
    pub application_id: i64,
    /// Physical schema number.
    pub user_version: i64,
    /// Persistent journaling mode, normalized to lowercase.
    pub journal_mode: String,
    /// SQLite synchronous level (`2` is `FULL`).
    pub synchronous: i64,
    /// Whether foreign-key enforcement is active on this connection.
    pub foreign_keys: bool,
    /// Whether application schemas are trusted on this connection.
    pub trusted_schema: bool,
    /// Busy timeout in milliseconds.
    pub busy_timeout_millis: i64,
    /// Temporary storage mode (`2` is memory).
    pub temp_store: i64,
    /// Whether SQLite itself rejects every data-changing statement.
    pub query_only: bool,
    /// Whether replace-style implicit deletes execute persisted delete triggers.
    pub recursive_triggers: bool,
}

/// The product writer connection with canonical-table authorizer installed.
pub(crate) struct WriterConnection {
    connection: Connection,
    database_path: PathBuf,
    acceptance_authorized: Arc<AtomicBool>,
    admitted_schema_version: i64,
}

impl fmt::Debug for WriterConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriterConnection")
            .field("database_path", &self.database_path)
            .field("canonical_authorizer", &true)
            .finish_non_exhaustive()
    }
}

impl WriterConnection {
    /// Enables the exact acceptance mutation set until the returned guard is dropped.
    pub(crate) fn authorize_acceptance(&self) -> AcceptanceAuthorization {
        let previously_authorized = self.acceptance_authorized.swap(true, Ordering::AcqRel);
        AcceptanceAuthorization {
            acceptance_authorized: Arc::clone(&self.acceptance_authorized),
            previously_authorized,
        }
    }

    /// Schema cookie SQLite reported when this writer passed admission.
    ///
    /// Acceptance compares it again inside the write transaction, so the value
    /// must be read out before the transaction borrows the connection.
    pub(crate) const fn admitted_schema_version(&self) -> i64 {
        self.admitted_schema_version
    }

    /// Starts the one allowed acceptance transaction with an eager write lock.
    ///
    /// This crate-private capability keeps the raw SQLite connection hidden while
    /// statically preventing nested transactions through the mutable borrow.
    pub(crate) fn begin_immediate(&mut self) -> StoreResult<Transaction<'_>> {
        self.connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
    }

    /// Runs a bounded repository preflight against the guarded connection.
    ///
    /// The higher-ranked callback cannot return a borrowed connection, and the
    /// installed authorizer remains active. S2 uses this only to resolve the
    /// complete sealed-artifact reference closure before `BEGIN IMMEDIATE`.
    pub(crate) fn with_preflight_reader<T>(
        &self,
        operation: impl for<'connection> FnOnce(&'connection Connection) -> T,
    ) -> T {
        operation(&self.connection)
    }

    /// Executes SQL through the guarded product writer.
    #[cfg(all(test, not(feature = "sqlcipher-store")))]
    pub(crate) fn execute<P: Params>(&self, sql: &str, params: P) -> StoreResult<usize> {
        self.connection
            .execute(sql, params)
            .map_err(StoreError::from)
    }

    /// Executes a SQL batch through the guarded product writer.
    #[cfg(all(test, not(feature = "sqlcipher-store")))]
    pub(crate) fn execute_batch(&self, sql: &str) -> StoreResult<()> {
        self.connection.execute_batch(sql).map_err(StoreError::from)
    }

    /// Runs one query row without exposing the underlying connection capability.
    #[cfg(all(test, not(feature = "sqlcipher-store")))]
    pub(crate) fn query_row<T, P, F>(&self, sql: &str, params: P, mapper: F) -> StoreResult<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    {
        self.connection
            .query_row(sql, params, mapper)
            .map_err(StoreError::from)
    }

    /// Reads back the exact active PRAGMAs.
    pub(crate) fn pragma_snapshot(&self) -> StoreResult<PragmaSnapshot> {
        read_pragma_snapshot(&self.connection)
    }

    /// Returns the database path without exposing a raw SQLite handle.
    #[must_use]
    pub(crate) fn database_path(&self) -> &Path {
        &self.database_path
    }
}

/// Restores the writer's fail-closed authorizer state on every return path.
pub(crate) struct AcceptanceAuthorization {
    acceptance_authorized: Arc<AtomicBool>,
    previously_authorized: bool,
}

impl fmt::Debug for AcceptanceAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptanceAuthorization")
            .field("previously_authorized", &self.previously_authorized)
            .finish_non_exhaustive()
    }
}

impl Drop for AcceptanceAuthorization {
    fn drop(&mut self) {
        self.acceptance_authorized
            .store(self.previously_authorized, Ordering::Release);
    }
}

/// A filesystem-read-only and SQLite-`query_only` connection.
pub struct ReaderConnection {
    connection: Connection,
    database_path: PathBuf,
}

impl fmt::Debug for ReaderConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReaderConnection")
            .field("database_path", &self.database_path)
            .field("query_only", &true)
            .finish_non_exhaustive()
    }
}

impl ReaderConnection {
    /// Starts one deferred read transaction for a multi-statement canonical snapshot.
    ///
    /// The transaction remains crate-private so consumers receive typed query DTOs
    /// rather than a raw SQLite capability.
    pub(crate) fn begin_deferred(&mut self) -> StoreResult<Transaction<'_>> {
        self.connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(StoreError::from)
    }

    /// Runs an internal typed query against the guarded read-only connection.
    pub(crate) fn with_query_connection<T>(
        &self,
        operation: impl for<'connection> FnOnce(&'connection Connection) -> T,
    ) -> T {
        operation(&self.connection)
    }

    /// Runs one query row without exposing the underlying connection capability.
    pub fn query_row<T, P, F>(&self, sql: &str, params: P, mapper: F) -> StoreResult<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    {
        self.connection
            .query_row(sql, params, mapper)
            .map_err(StoreError::from)
    }

    /// Collects a read-only result set without exposing the SQLite connection.
    ///
    /// S2 bitemporal resolution needs more than one normalized row. Keeping the
    /// statement and iterator inside this boundary preserves the OS/query-only
    /// reader capability while avoiding a raw-connection escape hatch.
    pub(crate) fn query_collect<T, P, F>(
        &self,
        sql: &str,
        params: P,
        mapper: F,
    ) -> StoreResult<Vec<T>>
    where
        P: Params,
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params, mapper)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    }

    /// Attempts one statement through both OS read-only and SQLite query-only enforcement.
    ///
    /// This method exists so callers can receive the real denial rather than assuming a
    /// high-level wrapper is the only protection.
    pub fn execute<P: Params>(&self, sql: &str, params: P) -> StoreResult<usize> {
        self.connection
            .execute(sql, params)
            .map_err(StoreError::from)
    }

    /// Attempts a batch through both reader protections.
    pub fn execute_batch(&self, sql: &str) -> StoreResult<()> {
        self.connection.execute_batch(sql).map_err(StoreError::from)
    }

    /// Reads back the exact active PRAGMAs.
    pub fn pragma_snapshot(&self) -> StoreResult<PragmaSnapshot> {
        read_pragma_snapshot(&self.connection)
    }

    /// Returns the database path without exposing a raw SQLite handle.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }
}

/// Opens the existing schema as the sole guarded product writer.
#[cfg(not(feature = "sqlcipher-store"))]
pub(crate) fn open_writer(database_path: &Path) -> StoreResult<WriterConnection> {
    open_writer_prepared(database_path, |_| Ok(()))
}

/// Opens the existing schema as the sole guarded product writer, keyed.
///
/// The key is applied by `prepare` as the first statement issued on the fresh
/// handle, before any admission read, so no page is touched unkeyed.
#[cfg(feature = "sqlcipher-store")]
pub(crate) fn open_keyed_writer(
    database_path: &Path,
    key: &academic_crypto::StoreKey,
) -> StoreResult<WriterConnection> {
    open_writer_prepared(database_path, |connection| {
        crate::cipher::apply_store_key(connection, key, database_path)
    })
}

/// The one writer-admission sequence, parameterized only by how the handle is
/// prepared before its first page access.
fn open_writer_prepared(
    database_path: &Path,
    prepare: impl FnOnce(&Connection) -> StoreResult<()>,
) -> StoreResult<WriterConnection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(database_path, flags)?;
    prepare(&connection)?;
    // Reject a foreign or tampered schema before journal-mode or any other
    // writer configuration can change the database family, and before closing
    // this read-write handle can checkpoint an uncheckpointed WAL into the
    // main database.
    disable_checkpoint_on_close(&connection)?;
    let admission_pragmas = read_pragma_snapshot(&connection)?;
    verify_current_schema(&connection, &admission_pragmas)?;
    verify_fts5(&connection)?;
    configure_writer_connection(&connection)?;
    let pragmas = read_pragma_snapshot(&connection)?;
    verify_writer_pragmas(&pragmas)?;
    verify_current_schema(&connection, &pragmas)?;
    // Admission is complete: this handle can no longer reject, so it must
    // checkpoint on close exactly as it did before the rejection protection.
    enable_checkpoint_on_close(&connection)?;
    let admitted_schema_version = read_schema_version(&connection)?;
    let acceptance_authorized = Arc::new(AtomicBool::new(false));
    install_canonical_authorizer(&connection, Arc::clone(&acceptance_authorized))?;
    Ok(WriterConnection {
        connection,
        database_path: database_path.to_path_buf(),
        acceptance_authorized,
        admitted_schema_version,
    })
}

/// Opens an existing database with OS read-only flags and SQLite `query_only=ON`.
#[cfg(not(feature = "sqlcipher-store"))]
pub fn open_reader(database_path: &Path) -> StoreResult<ReaderConnection> {
    open_reader_prepared(database_path, |_| Ok(()))
}

/// Opens an existing encrypted database read-only with its raw store key.
///
/// The encrypted lane has no unkeyed reader: [`open_reader`] is compiled out,
/// so no call site can reach an encrypted database without supplying a key.
#[cfg(feature = "sqlcipher-store")]
pub fn open_keyed_reader(
    database_path: &Path,
    key: &academic_crypto::StoreKey,
) -> StoreResult<ReaderConnection> {
    open_reader_prepared(database_path, |connection| {
        crate::cipher::apply_store_key(connection, key, database_path)
    })
}

/// The one reader-admission sequence, parameterized only by how the handle is
/// prepared before its first page access.
fn open_reader_prepared(
    database_path: &Path,
    prepare: impl FnOnce(&Connection) -> StoreResult<()>,
) -> StoreResult<ReaderConnection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(database_path, flags)?;
    prepare(&connection)?;
    let admission_pragmas = read_pragma_snapshot(&connection)?;
    verify_current_schema(&connection, &admission_pragmas)?;
    configure_reader_connection(&connection)?;
    let pragmas = read_pragma_snapshot(&connection)?;
    verify_reader_pragmas(&pragmas)?;
    install_reader_authorizer(&connection)?;
    Ok(ReaderConnection {
        connection,
        database_path: database_path.to_path_buf(),
    })
}

/// Disables SQLite's checkpoint-on-close for the admission window only.
///
/// Rejecting a database must leave its exact main-database and committed-WAL
/// bytes. Without this, closing the handle checkpoints an uncheckpointed WAL
/// into the main database and rewrites it after admission already failed. The
/// rebuildable `-shm` index and an empty `-wal` that SQLite's own read path
/// creates carry no committed content and are outside that claim.
///
/// The window ends where admission does: every caller that reaches an admitted
/// handle restores the default through [`enable_checkpoint_on_close`], so
/// normal operation keeps bringing the main database current at a clean close.
pub(crate) fn disable_checkpoint_on_close(connection: &Connection) -> StoreResult<()> {
    let disabled = connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
    if disabled {
        Ok(())
    } else {
        Err(StoreError::PragmaMismatch {
            pragma: "db_config.no_ckpt_on_close",
            expected: "1".to_owned(),
            actual: "0".to_owned(),
        })
    }
}

/// Restores SQLite's checkpoint-on-close once admission has succeeded.
///
/// The protection above is needed only while a rejection is still possible. An
/// admitted handle that kept it would never bring the main database current
/// again, so a clean close would leave every committed byte in `-wal` until the
/// autocheckpoint threshold, and any consumer of the main database alone would
/// read an empty database. Restoring the default here keeps the rejection path
/// byte-preserving and the admitted path's steady state unchanged.
pub(crate) fn enable_checkpoint_on_close(connection: &Connection) -> StoreResult<()> {
    let disabled = connection.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, false)?;
    if disabled {
        Err(StoreError::PragmaMismatch {
            pragma: "db_config.no_ckpt_on_close",
            expected: "0".to_owned(),
            actual: "1".to_owned(),
        })
    } else {
        Ok(())
    }
}

/// Reads SQLite's schema cookie, which changes whenever the schema changes.
pub(crate) fn read_schema_version(connection: &Connection) -> StoreResult<i64> {
    pragma_i64(connection, "schema_version")
}

/// Fails an acceptance closed when the schema changed under an admitted writer.
///
/// The structural fingerprint is verified when the writer is opened and never
/// again, but SQLite reloads `sqlite_schema` on an already-open connection
/// whenever the cookie changes, so a same-user process that edits the schema
/// after admission would otherwise reach the inside of a receipted acceptance.
/// The cookie is the same signal SQLite itself uses for that reload, and it is
/// compared after `BEGIN IMMEDIATE` has taken the write lock, so an acceptance
/// can only commit against the exact schema that was admitted.
pub(crate) fn verify_admitted_schema_version(
    connection: &Connection,
    admitted: i64,
) -> StoreResult<()> {
    let observed = read_schema_version(connection)?;
    if observed == admitted {
        Ok(())
    } else {
        Err(StoreError::SchemaIdentityMismatch {
            component: "schema.cookie",
            expected: admitted.to_string(),
            actual: observed.to_string(),
        })
    }
}

pub(crate) fn configure_migration_connection(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA synchronous = FULL;\
         PRAGMA foreign_keys = ON;\
         PRAGMA trusted_schema = OFF;\
         PRAGMA busy_timeout = 250;\
         PRAGMA temp_store = MEMORY;\
         PRAGMA recursive_triggers = ON;\
         PRAGMA query_only = OFF;",
    )?;
    Ok(())
}

fn configure_writer_connection(connection: &Connection) -> StoreResult<()> {
    configure_migration_connection(connection)
}

fn configure_reader_connection(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "PRAGMA query_only = ON;\
         PRAGMA foreign_keys = ON;\
         PRAGMA trusted_schema = OFF;\
         PRAGMA busy_timeout = 250;\
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(())
}

pub(crate) fn read_pragma_snapshot(connection: &Connection) -> StoreResult<PragmaSnapshot> {
    Ok(PragmaSnapshot {
        application_id: pragma_i64(connection, "application_id")?,
        user_version: pragma_i64(connection, "user_version")?,
        journal_mode: pragma_text(connection, "journal_mode")?.to_ascii_lowercase(),
        synchronous: pragma_i64(connection, "synchronous")?,
        foreign_keys: pragma_i64(connection, "foreign_keys")? == 1,
        trusted_schema: pragma_i64(connection, "trusted_schema")? == 1,
        busy_timeout_millis: pragma_i64(connection, "busy_timeout")?,
        temp_store: pragma_i64(connection, "temp_store")?,
        query_only: pragma_i64(connection, "query_only")? == 1,
        recursive_triggers: pragma_i64(connection, "recursive_triggers")? == 1,
    })
}

pub(crate) fn verify_fts5(connection: &Connection) -> StoreResult<()> {
    let compile_option = connection
        .query_row(
            "SELECT 1 FROM pragma_compile_options WHERE compile_options = 'ENABLE_FTS5'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if compile_option != Some(1) {
        return Err(StoreError::UnsupportedSqliteBuild(
            "ENABLE_FTS5 compile option is absent",
        ));
    }
    connection.execute_batch(
        "SAVEPOINT academic_fts5_probe;\
         CREATE VIRTUAL TABLE temp.academic_fts5_probe USING fts5(body);\
         INSERT INTO temp.academic_fts5_probe(body) VALUES ('synthetic probe');",
    )?;
    let matches = connection.query_row(
        "SELECT count(*) FROM temp.academic_fts5_probe WHERE body MATCH 'synthetic'",
        [],
        |row| row.get::<_, i64>(0),
    );
    let cleanup = connection.execute_batch(
        "DROP TABLE IF EXISTS temp.academic_fts5_probe;\
         RELEASE academic_fts5_probe;",
    );
    let matches = matches?;
    cleanup?;
    if matches != 1 {
        return Err(StoreError::UnsupportedSqliteBuild(
            "FTS5 executable create/query probe failed",
        ));
    }
    Ok(())
}

pub(crate) fn verify_writer_pragmas(pragmas: &PragmaSnapshot) -> StoreResult<()> {
    verify_identity_pragmas(pragmas)?;
    verify_migration_pragmas(pragmas)
}

pub(crate) fn verify_migration_pragmas(pragmas: &PragmaSnapshot) -> StoreResult<()> {
    verify_writer_operational_pragmas(pragmas)?;
    exact("query_only", "0", bool_i64(pragmas.query_only).to_string())
}

fn verify_reader_pragmas(pragmas: &PragmaSnapshot) -> StoreResult<()> {
    exact(
        "application_id",
        &SQLITE_APPLICATION_ID.to_string(),
        pragmas.application_id.to_string(),
    )?;
    exact(
        "user_version",
        &STORE_SCHEMA_VERSION.to_string(),
        pragmas.user_version.to_string(),
    )?;
    exact("journal_mode", "wal", pragmas.journal_mode.clone())?;
    exact(
        "foreign_keys",
        "1",
        bool_i64(pragmas.foreign_keys).to_string(),
    )?;
    exact(
        "trusted_schema",
        "0",
        bool_i64(pragmas.trusted_schema).to_string(),
    )?;
    exact(
        "busy_timeout",
        &SQLITE_BUSY_TIMEOUT_MILLIS.to_string(),
        pragmas.busy_timeout_millis.to_string(),
    )?;
    exact("query_only", "1", bool_i64(pragmas.query_only).to_string())
}

fn verify_identity_pragmas(pragmas: &PragmaSnapshot) -> StoreResult<()> {
    exact(
        "application_id",
        &SQLITE_APPLICATION_ID.to_string(),
        pragmas.application_id.to_string(),
    )?;
    exact(
        "user_version",
        &STORE_SCHEMA_VERSION.to_string(),
        pragmas.user_version.to_string(),
    )
}

fn verify_writer_operational_pragmas(pragmas: &PragmaSnapshot) -> StoreResult<()> {
    exact("journal_mode", "wal", pragmas.journal_mode.clone())?;
    exact(
        "synchronous",
        &SQLITE_SYNCHRONOUS_FULL.to_string(),
        pragmas.synchronous.to_string(),
    )?;
    exact(
        "foreign_keys",
        "1",
        bool_i64(pragmas.foreign_keys).to_string(),
    )?;
    exact(
        "trusted_schema",
        "0",
        bool_i64(pragmas.trusted_schema).to_string(),
    )?;
    exact(
        "busy_timeout",
        &SQLITE_BUSY_TIMEOUT_MILLIS.to_string(),
        pragmas.busy_timeout_millis.to_string(),
    )?;
    exact(
        "temp_store",
        &SQLITE_TEMP_STORE_MEMORY.to_string(),
        pragmas.temp_store.to_string(),
    )?;
    exact(
        "recursive_triggers",
        "1",
        bool_i64(pragmas.recursive_triggers).to_string(),
    )
}

fn exact(pragma: &'static str, expected: &str, actual: String) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::PragmaMismatch {
            pragma,
            expected: expected.to_owned(),
            actual,
        })
    }
}

fn pragma_i64(connection: &Connection, name: &'static str) -> StoreResult<i64> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(StoreError::from)
}

fn pragma_text(connection: &Connection, name: &'static str) -> StoreResult<String> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(StoreError::from)
}

const fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

// The plaintext-lane connection policy. The encrypted lane opens every handle
// keyed and is covered by `tests/encrypted_profile.rs` instead.
#[cfg(all(test, not(feature = "sqlcipher-store")))]
mod tests {
    use std::{
        error::Error,
        fs,
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };

    use crate::migration::migrate_pre_listen;

    use super::*;

    static NEXT_TEST_DATABASE: AtomicU64 = AtomicU64::new(0);

    const CANONICAL_FRAGMENT: &str = concat!(
        "INSERT INTO command_receipt (",
        "client_instance_id, idempotency_key, request_hash, expected_revision, ",
        "committed_revision, response_bytes, response_hash, created_at",
        ") VALUES (zeroblob(16), zeroblob(32), zeroblob(32), NULL, 1, x'00', ",
        "zeroblob(32), 0)"
    );

    #[test]
    fn public_writer_rejects_canonical_insert_outside_acceptance() -> Result<(), Box<dyn Error>> {
        let (root, path) = migrated_database("canonical-insert")?;
        let writer = open_writer(&path)?;

        assert!(writer.execute(CANONICAL_FRAGMENT, []).is_err());
        assert_eq!(
            writer.query_row("SELECT count(*) FROM command_receipt", [], |row| row
                .get::<_, i64>(0))?,
            0
        );
        assert!(
            writer
                .execute_batch("UPDATE replica_state SET profile_revision = 1;")
                .is_err()
        );

        drop(writer);
        remove_test_root(&root);
        Ok(())
    }

    #[test]
    fn acceptance_authorization_allows_only_append_and_head_updates() -> Result<(), Box<dyn Error>>
    {
        let (root, path) = migrated_database("exact-authority")?;
        let writer = open_writer(&path)?;
        let authorization = writer.authorize_acceptance();

        assert_eq!(
            writer.execute(
                "UPDATE replica_state SET profile_revision = profile_revision WHERE singleton = 1",
                [],
            )?,
            1
        );
        assert!(writer.execute("DELETE FROM command_receipt", []).is_err());
        assert!(
            writer
                .execute("UPDATE schema_meta SET singleton = 1", [])
                .is_err()
        );
        assert!(writer.execute_batch("DROP TABLE claim;").is_err());

        drop(authorization);
        assert!(
            writer
                .execute(
                    "UPDATE replica_state SET profile_revision = profile_revision WHERE singleton = 1",
                    [],
                )
                .is_err()
        );
        drop(writer);
        remove_test_root(&root);
        Ok(())
    }

    #[test]
    fn migration_snapshot_allows_uninitialized_identity_but_writer_snapshot_does_not() {
        let snapshot = expected_migration_snapshot();

        assert!(verify_migration_pragmas(&snapshot).is_ok());
        assert!(matches!(
            verify_writer_pragmas(&snapshot),
            Err(StoreError::PragmaMismatch {
                pragma: "application_id",
                ..
            })
        ));
    }

    #[test]
    fn migration_snapshot_rejects_every_operational_mismatch() {
        let expected = expected_migration_snapshot();
        let mismatches = [
            (
                "journal_mode",
                PragmaSnapshot {
                    journal_mode: "delete".to_owned(),
                    ..expected.clone()
                },
            ),
            (
                "synchronous",
                PragmaSnapshot {
                    synchronous: 1,
                    ..expected.clone()
                },
            ),
            (
                "foreign_keys",
                PragmaSnapshot {
                    foreign_keys: false,
                    ..expected.clone()
                },
            ),
            (
                "trusted_schema",
                PragmaSnapshot {
                    trusted_schema: true,
                    ..expected.clone()
                },
            ),
            (
                "busy_timeout",
                PragmaSnapshot {
                    busy_timeout_millis: 0,
                    ..expected.clone()
                },
            ),
            (
                "temp_store",
                PragmaSnapshot {
                    temp_store: 0,
                    ..expected.clone()
                },
            ),
            (
                "recursive_triggers",
                PragmaSnapshot {
                    recursive_triggers: false,
                    ..expected.clone()
                },
            ),
            (
                "query_only",
                PragmaSnapshot {
                    query_only: true,
                    ..expected
                },
            ),
        ];

        for (expected_pragma, snapshot) in mismatches {
            assert!(matches!(
                verify_migration_pragmas(&snapshot),
                Err(StoreError::PragmaMismatch { pragma, .. }) if pragma == expected_pragma
            ));
        }
    }

    #[test]
    fn admitted_writer_close_checkpoints_the_main_database() -> Result<(), Box<dyn Error>> {
        let (root, path) = migrated_database("checkpoint-on-close")?;
        let writer = open_writer(&path)?;
        // The rejection protection is lifted once admission has succeeded, so
        // this handle is the last-connection checkpointer again.
        assert!(
            !writer
                .connection
                .db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE)?
        );
        drop(writer);

        let mut wal = path.as_os_str().to_os_string();
        wal.push("-wal");
        assert!(
            !PathBuf::from(wal).try_exists()?,
            "a clean writer close must checkpoint and remove the WAL"
        );
        let main_length = fs::metadata(&path)?.len();
        assert!(
            main_length > 4096,
            "the main database must hold the checkpointed schema, saw {main_length} bytes"
        );

        remove_test_root(&root);
        Ok(())
    }

    #[test]
    fn admitted_writer_records_the_schema_cookie_it_was_admitted_with() -> Result<(), Box<dyn Error>>
    {
        let (root, path) = migrated_database("admitted-cookie")?;
        let writer = open_writer(&path)?;
        let admitted = writer.admitted_schema_version();
        assert_eq!(read_schema_version(&writer.connection)?, admitted);
        assert!(verify_admitted_schema_version(&writer.connection, admitted).is_ok());
        assert!(matches!(
            verify_admitted_schema_version(&writer.connection, admitted + 1),
            Err(StoreError::SchemaIdentityMismatch {
                component: "schema.cookie",
                ..
            })
        ));

        drop(writer);
        remove_test_root(&root);
        Ok(())
    }

    fn expected_migration_snapshot() -> PragmaSnapshot {
        PragmaSnapshot {
            application_id: 0,
            user_version: 0,
            journal_mode: "wal".to_owned(),
            synchronous: SQLITE_SYNCHRONOUS_FULL,
            foreign_keys: true,
            trusted_schema: false,
            busy_timeout_millis: 250,
            temp_store: SQLITE_TEMP_STORE_MEMORY,
            query_only: false,
            recursive_triggers: true,
        }
    }

    fn migrated_database(label: &str) -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
        let sequence = NEXT_TEST_DATABASE.fetch_add(1, AtomicOrdering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-connection-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let path = root.join("store.sqlite3");
        migrate_pre_listen(&path, [0x8d; 32])?;
        Ok((root, path))
    }

    fn remove_test_root(root: &Path) {
        if let Err(error) = fs::remove_dir_all(root) {
            eprintln!("test cleanup failed for {}: {error}", root.display());
        }
    }
}
