use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_store::{
    SQLITE_APPLICATION_ID, STORE_FORMAT_UUID, STORE_SCHEMA_SEMVER, STORE_SCHEMA_VERSION,
    connection::open_reader,
    error::StoreError,
    migration::{
        MigrationStatus, checked_sqlite_integer, migrate_pre_listen, read_schema_identity,
    },
};
use rusqlite::Connection;

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const BUILD_DIGEST: [u8; 32] = [0x5a; 32];

#[derive(Debug)]
struct TemporaryDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl TemporaryDatabase {
    fn new(label: &str) -> std::io::Result<Self> {
        let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-migration-{label}-{}-{sequence}",
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
fn schema_identity_triplet_agrees() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("identity")?;
    assert_eq!(
        migrate_pre_listen(database.path(), BUILD_DIGEST)?,
        MigrationStatus::Applied
    );
    let reader = open_reader(database.path())?;
    let pragmas = reader.pragma_snapshot()?;
    assert_eq!(pragmas.application_id, i64::from(SQLITE_APPLICATION_ID));
    assert_eq!(pragmas.user_version, i64::from(STORE_SCHEMA_VERSION));

    let raw = Connection::open(database.path())?;
    let identity = read_schema_identity(&raw)?;
    assert_eq!(identity.format_uuid, STORE_FORMAT_UUID);
    assert_eq!(identity.schema_version, STORE_SCHEMA_VERSION);
    assert_eq!(identity.schema_semver, STORE_SCHEMA_SEMVER);
    assert_eq!(identity.creating_build_digest, BUILD_DIGEST);
    assert!(!identity.production_data_allowed);
    Ok(())
}

#[test]
fn migration_is_idempotent_only_at_same_version() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("idempotence")?;
    assert_eq!(
        migrate_pre_listen(database.path(), BUILD_DIGEST)?,
        MigrationStatus::Applied
    );
    assert_eq!(
        migrate_pre_listen(database.path(), [0x6b; 32])?,
        MigrationStatus::AlreadyCurrent
    );

    let raw = Connection::open(database.path())?;
    raw.pragma_update(None, "user_version", STORE_SCHEMA_VERSION + 1)?;
    drop(raw);
    assert!(matches!(
        migrate_pre_listen(database.path(), BUILD_DIGEST),
        Err(StoreError::NewerSchema {
            found: 2,
            supported: 1
        })
    ));

    let foreign = TemporaryDatabase::new("foreign")?;
    let raw = Connection::open(foreign.path())?;
    raw.execute_batch("CREATE TABLE unexpected(value INTEGER) STRICT;")?;
    drop(raw);
    assert!(matches!(
        migrate_pre_listen(foreign.path(), BUILD_DIGEST),
        Err(StoreError::UnsupportedMigrationState { .. })
    ));

    let foreign_version = TemporaryDatabase::new("foreign-version")?;
    let raw = Connection::open(foreign_version.path())?;
    raw.pragma_update(None, "user_version", STORE_SCHEMA_VERSION + 1)?;
    drop(raw);
    assert!(matches!(
        migrate_pre_listen(foreign_version.path(), BUILD_DIGEST),
        Err(StoreError::UnsupportedMigrationState {
            application_id: 0,
            user_version: 2
        })
    ));

    let tampered = TemporaryDatabase::new("tampered")?;
    migrate_pre_listen(tampered.path(), BUILD_DIGEST)?;
    let raw = Connection::open(tampered.path())?;
    raw.execute_batch("DROP TRIGGER guard_claim_delete;")?;
    drop(raw);
    assert!(matches!(
        open_reader(tampered.path()),
        Err(StoreError::SchemaIdentityMismatch { .. })
    ));
    assert!(matches!(
        migrate_pre_listen(tampered.path(), BUILD_DIGEST),
        Err(StoreError::SchemaIdentityMismatch { .. })
    ));
    Ok(())
}

#[test]
fn u64_overflow_is_rejected() -> Result<(), Box<dyn Error>> {
    let maximum = u64::try_from(i64::MAX)?;
    assert_eq!(checked_sqlite_integer(maximum)?, i64::MAX);
    assert!(matches!(
        checked_sqlite_integer(maximum + 1),
        Err(StoreError::UnsignedIntegerOverflow(value)) if value == maximum + 1
    ));
    assert!(matches!(
        checked_sqlite_integer(u64::MAX),
        Err(StoreError::UnsignedIntegerOverflow(u64::MAX))
    ));
    Ok(())
}

#[test]
fn canonical_and_operational_schema_is_complete_and_strict() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("tables")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let raw = Connection::open(database.path())?;
    let mut statement = raw.prepare(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(
        names,
        [
            "artifact_descriptor",
            "artifact_representation",
            "claim",
            "claim_evidence",
            "claim_relation",
            "command_receipt",
            "device_head",
            "evidence_item",
            "ingest_lease",
            "ledger_batch",
            "ledger_event",
            "projection_active",
            "projection_cursor",
            "projection_outbox",
            "replica_state",
            "schema_meta",
            "scope",
            "user_decision",
        ]
    );
    let non_strict = raw.query_row(
        "SELECT count(*) FROM pragma_table_list WHERE schema = 'main' AND name NOT LIKE 'sqlite_%' AND strict <> 1",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    assert_eq!(non_strict, 0);
    let foreign_key_violations =
        raw.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get::<_, i64>(0)
        })?;
    assert_eq!(foreign_key_violations, 0);

    raw.execute_batch("PRAGMA foreign_keys = ON;")?;
    assert!(
        raw.execute(
            concat!(
                "INSERT INTO ledger_batch (",
                "batch_id, signed_envelope, envelope_hash, deterministic_payload, ",
                "deterministic_payload_hash, signing_public_key, signature, device_id, ",
                "origin_seq_start, origin_seq_end, previous_batch_hash, origin_created_at, ",
                "event_schema_version, accept_seq_start, accept_seq_end, accepted_at",
                ") VALUES (zeroblob(15), x'01', zeroblob(32), x'01', zeroblob(32), ",
                "zeroblob(32), zeroblob(64), zeroblob(16), 1, 1, NULL, 0, 1, 1, 1, 0)"
            ),
            [],
        )
        .is_err()
    );
    assert!(
        raw.execute(
            concat!(
                "INSERT INTO ledger_event (",
                "event_id, batch_id, origin_seq, origin_observed_at, accept_seq, actor_kind, ",
                "actor_canonical, domain_id, event_kind, canonical_payload, payload_hash",
                ") VALUES (x'00000000000000000000000000000001', ",
                "x'00000000000000000000000000000002', 1, 0, 1, 'USER', x'01', ",
                "x'00000000000000000000000000000003', 'synthetic.event', x'01', zeroblob(32))"
            ),
            [],
        )
        .is_err()
    );
    raw.execute(
        concat!(
            "INSERT INTO command_receipt (",
            "client_instance_id, idempotency_key, request_hash, expected_revision, ",
            "committed_revision, response_bytes, response_hash, created_at",
            ") VALUES (x'00000000000000000000000000000011', zeroblob(32), ",
            "x'0101010101010101010101010101010101010101010101010101010101010101', ",
            "NULL, 1, x'01', zeroblob(32), -1)"
        ),
        [],
    )?;
    raw.execute(
        concat!(
            "INSERT INTO command_receipt (",
            "client_instance_id, idempotency_key, request_hash, expected_revision, ",
            "committed_revision, response_bytes, response_hash, created_at",
            ") VALUES (x'00000000000000000000000000000012', zeroblob(32), ",
            "x'0202020202020202020202020202020202020202020202020202020202020202', ",
            "NULL, 1, x'01', zeroblob(32), 0)"
        ),
        [],
    )?;
    assert!(
        raw.execute(
            concat!(
                "INSERT INTO command_receipt (",
                "client_instance_id, idempotency_key, request_hash, expected_revision, ",
                "committed_revision, response_bytes, response_hash, created_at",
                ") VALUES (x'00000000000000000000000000000011', zeroblob(32), ",
                "x'0303030303030303030303030303030303030303030303030303030303030303', ",
                "NULL, 2, x'02', ",
                "x'0404040404040404040404040404040404040404040404040404040404040404', 1)"
            ),
            [],
        )
        .is_err()
    );
    Ok(())
}
