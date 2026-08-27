use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_store::{
    SQLITE_APPLICATION_ID, SQLITE_BUSY_TIMEOUT_MILLIS, STORE_SCHEMA_VERSION,
    connection::{open_reader, open_writer},
    migration::migrate_pre_listen,
};
use rusqlite::Connection;

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const BUILD_DIGEST: [u8; 32] = [0x29; 32];

#[derive(Debug)]
struct TemporaryDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl TemporaryDatabase {
    fn new(label: &str) -> std::io::Result<Self> {
        let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-sql-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let path = root.join("store.sqlite3");
        Ok(Self { root, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

#[test]
fn sqlite_pragmas_are_exact() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("pragmas")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let writer = open_writer(database.path())?;
    let pragmas = writer.pragma_snapshot()?;
    assert_eq!(pragmas.application_id, i64::from(SQLITE_APPLICATION_ID));
    assert_eq!(pragmas.user_version, i64::from(STORE_SCHEMA_VERSION));
    assert_eq!(pragmas.journal_mode, "wal");
    assert_eq!(pragmas.synchronous, 2);
    assert!(pragmas.foreign_keys);
    assert!(!pragmas.trusted_schema);
    assert_eq!(
        pragmas.busy_timeout_millis,
        i64::try_from(SQLITE_BUSY_TIMEOUT_MILLIS)?
    );
    assert_eq!(pragmas.temp_store, 2);
    assert!(!pragmas.query_only);
    assert!(pragmas.recursive_triggers);
    Ok(())
}

#[test]
fn reader_is_query_only() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("reader")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let reader = open_reader(database.path())?;
    let pragmas = reader.pragma_snapshot()?;
    assert!(pragmas.query_only);
    assert!(pragmas.foreign_keys);
    assert!(!pragmas.trusted_schema);
    assert_eq!(pragmas.busy_timeout_millis, 250);
    assert_eq!(
        reader.query_row("SELECT profile_revision FROM replica_state", [], |row| row
            .get::<_, i64>(
            0
        ))?,
        0
    );
    assert!(
        reader
            .execute("UPDATE replica_state SET profile_revision = 1", [])
            .is_err()
    );
    assert!(
        reader
            .execute_batch("CREATE TABLE forbidden(value INTEGER);")
            .is_err()
    );
    assert!(reader.execute_batch("PRAGMA query_only = OFF;").is_err());
    assert!(reader.pragma_snapshot()?.query_only);
    Ok(())
}

#[test]
fn canonical_update_delete_are_denied() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("append-only")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let writer = open_writer(database.path())?;

    assert_eq!(
        writer.execute("UPDATE replica_state SET profile_revision = 1", [])?,
        1
    );
    assert!(
        writer
            .execute(
                "UPDATE schema_meta SET created_at_unix_ms = created_at_unix_ms + 1",
                []
            )
            .is_err()
    );
    assert!(
        writer
            .execute("DELETE FROM schema_meta WHERE singleton = 1", [])
            .is_err()
    );
    assert!(
        writer
            .execute(
                concat!(
                    "INSERT OR REPLACE INTO schema_meta ",
                    "SELECT singleton, format_uuid, schema_version, schema_semver, ",
                    "minimum_reader_protocol_major, minimum_reader_protocol_minor, ",
                    "minimum_writer_protocol_major, minimum_writer_protocol_minor, ",
                    "data_policy, storage_mode, storage_encryption, production_data_allowed, ",
                    "product_network, creating_build_digest, created_at_unix_ms + 1 ",
                    "FROM schema_meta WHERE singleton = 1"
                ),
                [],
            )
            .is_err()
    );
    assert!(
        writer
            .execute_batch("PRAGMA recursive_triggers = OFF;")
            .is_err()
    );
    assert!(writer.pragma_snapshot()?.recursive_triggers);
    assert!(
        writer
            .execute_batch("PRAGMA writable_schema = ON;")
            .is_err()
    );
    for table in [
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
    ] {
        assert!(
            writer
                .execute_batch(&format!("DELETE FROM {table};"))
                .is_err(),
            "authorizer allowed canonical delete for {table}"
        );
    }
    assert!(writer.execute_batch("DROP TABLE claim;").is_err());
    assert!(
        writer
            .execute_batch("DROP INDEX idx_claim_resolution;")
            .is_err()
    );
    assert!(
        writer
            .execute_batch("ALTER TABLE claim ADD COLUMN forbidden TEXT;")
            .is_err()
    );
    let trigger_count = writer.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'trigger' AND name LIKE 'guard_%'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(trigger_count, 26);
    drop(writer);

    let maintenance = Connection::open(database.path())?;
    assert!(
        maintenance
            .execute(
                "UPDATE schema_meta SET created_at_unix_ms = created_at_unix_ms + 1",
                []
            )
            .is_err()
    );
    assert!(
        maintenance
            .execute("DELETE FROM schema_meta WHERE singleton = 1", [])
            .is_err()
    );
    Ok(())
}
