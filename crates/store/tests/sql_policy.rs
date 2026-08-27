use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_store::{
    SQLITE_APPLICATION_ID, SQLITE_BUSY_TIMEOUT_MILLIS, STORE_SCHEMA_VERSION,
    connection::open_reader,
    path_policy::NativePathProbe,
    profile::{SyntheticProfile, create_synthetic_profile},
};

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const BUILD_DIGEST: [u8; 32] = [0x29; 32];

#[derive(Debug)]
struct TemporaryDatabase {
    root: PathBuf,
    profile: SyntheticProfile,
}

impl TemporaryDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-sql-{label}-{}-{sequence}",
            std::process::id()
        ));
        let profile = create_synthetic_profile(&root, &NativePathProbe::default(), BUILD_DIGEST)?;
        Ok(Self { root, profile })
    }

    fn path(&self) -> &Path {
        self.profile.database_path()
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
    let store = database.profile.open_acceptance_store()?;
    let pragmas = store.pragma_snapshot()?;
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
