use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use academic_store::{connection::open_writer, migration::migrate_pre_listen};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TemporaryDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl TemporaryDatabase {
    fn new() -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-s2-authorizer-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let path = root.join("store.sqlite3");
        migrate_pre_listen(&path, [0x82; 32])?;
        Ok(Self { root, path })
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
fn canonical_rows_remain_append_only_while_heads_are_operational() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new()?;
    let writer = open_writer(&database.path)?;

    assert!(
        writer.execute("DELETE FROM ledger_batch", []).is_err(),
        "canonical batch deletion must be denied even when the table is empty"
    );
    assert!(
        writer
            .execute("UPDATE projection_outbox SET created_at = 0", [])
            .is_err(),
        "canonical outbox mutation must be denied"
    );
    assert!(
        writer
            .execute("UPDATE command_receipt SET response_bytes = x'00'", [])
            .is_err(),
        "stored response bytes must be immutable"
    );

    assert_eq!(
        writer.execute(
            "UPDATE replica_state SET profile_revision = profile_revision WHERE singleton = 1",
            [],
        )?,
        1,
        "the transaction owner still needs the operational head update capability"
    );
    Ok(())
}
