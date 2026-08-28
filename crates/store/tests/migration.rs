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

#[derive(Debug, PartialEq, Eq)]
struct DatabaseFamilySnapshot(Vec<(String, Option<Vec<u8>>)>);

fn database_family_snapshot(path: &Path) -> Result<DatabaseFamilySnapshot, Box<dyn Error>> {
    let mut members = Vec::new();
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let member = if suffix.is_empty() {
            path.to_path_buf()
        } else {
            let mut value = path.as_os_str().to_os_string();
            value.push(suffix);
            PathBuf::from(value)
        };
        let bytes = if member.try_exists()? {
            Some(fs::read(&member)?)
        } else {
            None
        };
        members.push((suffix.to_owned(), bytes));
    }
    Ok(DatabaseFamilySnapshot(members))
}

fn assert_maintenance_and_reader_reject_without_mutation(
    path: &Path,
) -> Result<(StoreError, StoreError), Box<dyn Error>> {
    let before_maintenance = database_family_snapshot(path)?;
    let maintenance_error = match migrate_pre_listen(path, BUILD_DIGEST) {
        Ok(status) => {
            return Err(std::io::Error::other(format!(
                "maintenance unexpectedly admitted schema as {status:?}"
            ))
            .into());
        }
        Err(error) => error,
    };
    assert_eq!(
        database_family_snapshot(path)?,
        before_maintenance,
        "maintenance rejection changed the database family"
    );

    let before_reader = database_family_snapshot(path)?;
    let reader_error = match open_reader(path) {
        Ok(_) => {
            return Err(
                std::io::Error::other("read-only reader unexpectedly admitted schema").into(),
            );
        }
        Err(error) => error,
    };
    assert_eq!(
        database_family_snapshot(path)?,
        before_reader,
        "reader rejection changed the database family"
    );
    Ok((maintenance_error, reader_error))
}

fn use_delete_journal(connection: &Connection) -> Result<(), Box<dyn Error>> {
    let journal_mode = connection.query_row("PRAGMA journal_mode = DELETE", [], |row| {
        row.get::<_, String>(0)
    })?;
    assert_eq!(journal_mode.to_ascii_lowercase(), "delete");
    Ok(())
}

fn replace_schema_sql(
    connection: &Connection,
    object_type: &str,
    name: &str,
    from: &str,
    to: &str,
) -> Result<(), Box<dyn Error>> {
    let original = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2",
        [object_type, name],
        |row| row.get::<_, String>(0),
    )?;
    let replacement = original.replacen(from, to, 1);
    assert_ne!(replacement, original, "schema mutation anchor must exist");
    connection.execute_batch("PRAGMA writable_schema = ON;")?;
    connection.execute(
        "UPDATE sqlite_schema SET sql = ?3 WHERE type = ?1 AND name = ?2",
        (object_type, name, replacement),
    )?;
    connection.execute_batch("PRAGMA writable_schema = OFF;")?;
    let schema_version =
        connection.query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))?;
    connection.pragma_update(None, "schema_version", schema_version + 1)?;
    Ok(())
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
fn canonical_schema_fingerprint_admits_maintenance_and_reader() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("canonical-fingerprint")?;
    assert_eq!(
        migrate_pre_listen(database.path(), BUILD_DIGEST)?,
        MigrationStatus::Applied
    );
    assert_eq!(
        migrate_pre_listen(database.path(), [0x7c; 32])?,
        MigrationStatus::AlreadyCurrent
    );
    let reader = open_reader(database.path())?;
    assert_eq!(
        reader.pragma_snapshot()?.user_version,
        i64::from(STORE_SCHEMA_VERSION)
    );
    Ok(())
}

#[test]
fn formatting_only_trigger_definition_is_canonically_admitted() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("canonical-formatting")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let raw = Connection::open(database.path())?;
    raw.execute_batch(concat!(
        "DROP TRIGGER guard_claim_delete;",
        "CREATE TRIGGER \"guard_claim_delete\" before delete ON \"claim\" ",
        "BEGIN /* formatting-only */ SELECT raise(abort, ",
        "'canonical table is append-only'); END;"
    ))?;
    drop(raw);

    assert_eq!(
        migrate_pre_listen(database.path(), BUILD_DIGEST)?,
        MigrationStatus::AlreadyCurrent
    );
    let _reader = open_reader(database.path())?;
    Ok(())
}

#[test]
fn version_zero_sqlite_x_user_object_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("version-zero-sqlite-x")?;
    let raw = Connection::open(database.path())?;
    raw.execute_batch("CREATE TABLE sqliteXforeign(value INTEGER) STRICT;")?;
    drop(raw);

    let (maintenance_error, _) =
        assert_maintenance_and_reader_reject_without_mutation(database.path())?;
    assert!(matches!(
        maintenance_error,
        StoreError::UnsupportedMigrationState {
            application_id: 0,
            user_version: 0
        }
    ));
    Ok(())
}

#[test]
fn current_schema_extra_sqlite_x_object_is_rejected_without_mutation() -> Result<(), Box<dyn Error>>
{
    let database = TemporaryDatabase::new("current-sqlite-x")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let raw = Connection::open(database.path())?;
    use_delete_journal(&raw)?;
    raw.execute_batch("CREATE TABLE sqliteXshadow(value INTEGER) STRICT;")?;
    drop(raw);

    let (maintenance_error, reader_error) =
        assert_maintenance_and_reader_reject_without_mutation(database.path())?;
    assert!(matches!(
        maintenance_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    assert!(matches!(
        reader_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    Ok(())
}

#[test]
fn same_name_changed_trigger_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("changed-trigger")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let raw = Connection::open(database.path())?;
    use_delete_journal(&raw)?;
    raw.execute_batch(concat!(
        "DROP TRIGGER guard_claim_delete;",
        "CREATE TRIGGER guard_claim_delete BEFORE DELETE ON claim ",
        "BEGIN SELECT 1; END;"
    ))?;
    drop(raw);

    let (maintenance_error, reader_error) =
        assert_maintenance_and_reader_reject_without_mutation(database.path())?;
    assert!(matches!(
        maintenance_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    assert!(matches!(
        reader_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    Ok(())
}

#[test]
fn same_name_changed_index_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("changed-index")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let raw = Connection::open(database.path())?;
    use_delete_journal(&raw)?;
    raw.execute_batch(concat!(
        "DROP INDEX idx_ledger_event_domain_accept;",
        "CREATE UNIQUE INDEX idx_ledger_event_domain_accept ",
        "ON ledger_event(lower(event_kind), accept_seq) WHERE accept_seq > 0;"
    ))?;
    drop(raw);

    let (maintenance_error, reader_error) =
        assert_maintenance_and_reader_reject_without_mutation(database.path())?;
    assert!(matches!(
        maintenance_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    assert!(matches!(
        reader_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    Ok(())
}

#[test]
fn same_name_changed_table_definition_is_rejected_without_mutation() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new("changed-table")?;
    migrate_pre_listen(database.path(), BUILD_DIGEST)?;
    let raw = Connection::open(database.path())?;
    use_delete_journal(&raw)?;
    replace_schema_sql(
        &raw,
        "table",
        "ingest_lease",
        "lease_id BLOB PRIMARY KEY CHECK",
        "lease_id BLOB NOT NULL UNIQUE CHECK",
    )?;
    replace_schema_sql(
        &raw,
        "table",
        "ingest_lease",
        "owner_instance_id BLOB NOT NULL CHECK (",
        "owner_instance_id TEXT DEFAULT 'synthetic' CHECK (",
    )?;
    replace_schema_sql(
        &raw,
        "table",
        "ingest_lease",
        "expires_at > acquired_at",
        "expires_at >= acquired_at",
    )?;
    replace_schema_sql(
        &raw,
        "table",
        "claim_evidence",
        "REFERENCES claim(claim_id) ON UPDATE RESTRICT ON DELETE RESTRICT",
        "REFERENCES claim(claim_id) ON UPDATE CASCADE ON DELETE RESTRICT",
    )?;
    drop(raw);

    let (maintenance_error, reader_error) =
        assert_maintenance_and_reader_reject_without_mutation(database.path())?;
    assert!(matches!(
        maintenance_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
    assert!(matches!(
        reader_error,
        StoreError::SchemaIdentityMismatch { .. }
    ));
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
        "SELECT name FROM sqlite_schema WHERE type = 'table' \
         AND substr(name, 1, length('sqlite_')) COLLATE NOCASE <> 'sqlite_' ORDER BY name",
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
        "SELECT count(*) FROM pragma_table_list WHERE schema = 'main' \
         AND substr(name, 1, length('sqlite_')) COLLATE NOCASE <> 'sqlite_' AND strict <> 1",
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
