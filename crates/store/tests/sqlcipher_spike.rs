use std::{error::Error, fs, path::Path, process::Command};

#[cfg(feature = "sqlcipher-spike")]
use std::{
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

#[cfg(feature = "sqlcipher-spike")]
#[allow(dead_code)]
#[path = "../src/bin/sqlcipher_spike.rs"]
mod spike;

#[cfg(feature = "sqlcipher-spike")]
static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "sqlcipher-spike")]
#[derive(Debug)]
struct TempRoot {
    path: PathBuf,
}

#[cfg(feature = "sqlcipher-spike")]
impl TempRoot {
    fn new(label: &str) -> std::io::Result<Self> {
        let sequence = NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "academic-sqlcipher-e1-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(feature = "sqlcipher-spike")]
impl Drop for TempRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "SQLCipher test cleanup failed for {}: {error}",
                self.path.display()
            );
        }
    }
}

#[cfg(feature = "sqlcipher-spike")]
fn spike_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sqlcipher_spike"))
}

#[cfg(feature = "sqlcipher-spike")]
#[test]
fn sqlcipher_feature_is_explicit() -> Result<(), Box<dyn Error>> {
    assert_eq!(academic_store::SQLCIPHER_SPIKE_FEATURE, "sqlcipher-spike");
    assert_eq!(academic_store::BUNDLED_SQLITE_FEATURE, "bundled-sqlite");
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))?;
    assert!(manifest.contains("default = [\"bundled-sqlite\"]"));
    assert!(
        manifest.contains("sqlcipher-spike = [\"rusqlite/bundled-sqlcipher-vendored-openssl\"]")
    );
    assert!(!manifest.contains("default = [\"sqlcipher-spike\"]"));

    let root = TempRoot::new("feature")?;
    let database = root.path().join("explicit.sqlite3");
    let (cipher, pragmas) = spike::create_initialized_database(&database, false)?;
    assert!(cipher.cipher_version.starts_with("4.14.0"));
    assert_eq!(cipher.cipher_page_size, 4096);
    assert_eq!(cipher.kdf_iter, 256_000);
    assert_eq!(cipher.cipher_hmac_algorithm, "HMAC_SHA512");
    assert_eq!(cipher.cipher_kdf_algorithm, "PBKDF2_HMAC_SHA512");
    assert_eq!(pragmas.application_id, 0x4143_4144);
    assert_eq!(pragmas.user_version, 1);
    assert_eq!(pragmas.journal_mode, "wal");
    assert_eq!(pragmas.synchronous, 2);
    assert_eq!(pragmas.foreign_keys, 1);
    assert_eq!(pragmas.trusted_schema, 0);
    assert_eq!(pragmas.busy_timeout, 250);
    assert_eq!(pragmas.temp_store, 2);
    Ok(())
}

#[cfg(feature = "sqlcipher-spike")]
#[test]
fn sqlcipher_wrong_key_fails_closed() -> Result<(), Box<dyn Error>> {
    let root = TempRoot::new("wrong-key")?;
    let database = root.path().join("encrypted.sqlite3");
    spike::create_initialized_database(&database, false)?;
    assert!(spike::wrong_key_is_rejected(&database)?);

    let corrupt = root.path().join("corrupt-header.sqlite3");
    spike::corrupt_header_copy(&database, &corrupt)?;
    assert!(spike::corrupt_header_is_rejected(&corrupt)?);
    Ok(())
}

#[cfg(feature = "sqlcipher-spike")]
#[test]
fn sqlcipher_plaintext_canary_absent_from_all_artifacts() -> Result<(), Box<dyn Error>> {
    let root = TempRoot::new("canary")?;
    let artifact_root = root.path().join("artifacts");
    let receipt = spike::run_full_harness(&artifact_root)?;
    let expected = spike::load_canaries()?.len();
    assert_eq!(receipt.canary_count, expected);
    assert_eq!(receipt.restored_canary_count, expected);
    assert!(receipt.scan.findings.is_empty());
    assert!(receipt.scan.files_scanned >= 6);
    assert!(receipt.scan.bytes_scanned > 0);
    assert!(artifact_root.join("temp").is_dir());
    assert!(fs::read_dir(artifact_root.join("temp"))?.next().is_none());
    assert!(artifact_root.join("database/academic.sqlite3").is_file());
    assert!(
        artifact_root
            .join("backup/academic-backup.sqlite3")
            .is_file()
    );
    assert!(
        artifact_root
            .join("restore/academic-restore.sqlite3")
            .is_file()
    );
    assert!(
        artifact_root
            .join("crash-artifacts/database.sqlite3")
            .is_file()
    );
    assert!(
        artifact_root
            .join("crash-artifacts/database.sqlite3-wal")
            .is_file()
    );
    assert!(
        artifact_root
            .join("crash-artifacts/database.sqlite3-shm")
            .is_file()
    );
    Ok(())
}

#[cfg(feature = "sqlcipher-spike")]
#[test]
fn sqlcipher_wal_crash_recovers_or_fails_closed() -> Result<(), Box<dyn Error>> {
    let root = TempRoot::new("wal-crash")?;
    let mut committed_database = None;
    for checkpoint in ["DB01", "DB02", "DB03", "DB04", "DB05", "DB06", "DB07"] {
        let database = root.path().join(format!("{checkpoint}.sqlite3"));
        let output = Command::new(spike_binary())
            .arg("child-db-fault")
            .arg(checkpoint)
            .arg(&database)
            .output()?;
        assert_eq!(
            output.status.code(),
            Some(spike::db_fault_exit_code(checkpoint)?),
            "{checkpoint}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(database.is_file());

        let crash = root.path().join(format!("{checkpoint}-crash-artifacts"));
        spike::capture_crash_artifacts(&database, &crash)?;
        let outcome = spike::db_fault_outcome(&crash.join("database.sqlite3"), checkpoint)?;
        if checkpoint == "DB07" {
            assert_eq!(outcome, "COMMITTED_EXACT_RECEIPT_REPLAYABLE");
            assert!(PathBuf::from(format!("{}-wal", database.display())).is_file());
            assert!(PathBuf::from(format!("{}-shm", database.display())).is_file());
            committed_database = Some(database);
        } else {
            assert_eq!(outcome, "ROLLED_BACK_NO_SEQUENCE_CONSUMED");
        }
    }

    let database = committed_database.ok_or("DB07 database was not captured")?;
    assert_eq!(
        spike::wal_crash_exit_code(),
        spike::db_fault_exit_code("DB07")?
    );

    let truncated = root.path().join("truncated-wal");
    let truncated_database = spike::make_truncated_wal_snapshot(&database, &truncated)?;
    let outcome = spike::truncated_wal_outcome(&truncated_database)?;
    assert!(matches!(
        outcome.as_str(),
        "RECOVERED_COMPLETE"
            | "ATOMIC_PREVIOUS_STATE"
            | "FAIL_CLOSED_ON_OPEN"
            | "FAIL_CLOSED_ON_READ"
    ));

    let scan = spike::scan_artifacts(root.path(), &spike::load_canaries()?)?;
    assert!(
        scan.findings.is_empty(),
        "plaintext hits: {:?}",
        scan.findings
    );
    Ok(())
}

#[cfg(feature = "sqlcipher-spike")]
#[test]
fn sqlcipher_rekey_fault_has_one_documented_recovery_key() -> Result<(), Box<dyn Error>> {
    let root = TempRoot::new("rekey")?;
    let database = root.path().join("rekey.sqlite3");
    spike::create_initialized_database(&database, true)?;
    let before = root.path().join("rekey-before.marker");
    let after = root.path().join("rekey-after.marker");
    let mut child = Command::new(spike_binary())
        .arg("child-rekey")
        .arg(&database)
        .arg(&before)
        .arg(&after)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    assert!(
        spike::wait_for_marker(&before, 5_000),
        "rekey child did not reach the before-rewrite checkpoint"
    );
    thread::sleep(Duration::from_millis(20));
    let completed_before_kill = child.try_wait()?;
    if completed_before_kill.is_none() {
        child.kill()?;
    }
    let status = match completed_before_kill {
        Some(status) => status,
        None => child.wait()?,
    };
    if after.is_file() {
        assert_eq!(status.code(), Some(spike::rekey_complete_exit_code()));
    } else {
        assert!(!status.success());
    }

    let crash = root.path().join("rekey-crash-artifacts");
    spike::capture_crash_artifacts(&database, &crash)?;
    let recovery_key = spike::documented_recovery_key(&database)?;
    assert!(matches!(
        recovery_key.as_str(),
        "PRIMARY_PRE_REKEY_KEY" | "NEW_POST_REKEY_KEY"
    ));
    let scan = spike::scan_artifacts(&crash, &spike::load_canaries()?)?;
    assert!(
        scan.findings.is_empty(),
        "plaintext hits: {:?}",
        scan.findings
    );
    Ok(())
}

#[cfg(feature = "sqlcipher-spike")]
#[test]
fn sqlcipher_online_backup_restores_empty_profile() -> Result<(), Box<dyn Error>> {
    let root = TempRoot::new("backup")?;
    let source = root.path().join("source.sqlite3");
    let backup = root.path().join("backup.sqlite3");
    let restore = root.path().join("restored.sqlite3");
    spike::create_initialized_database(&source, false)?;
    let count = spike::online_backup_and_empty_restore(&source, &backup, &restore)?;
    assert_eq!(count, spike::load_canaries()?.len());
    assert_eq!(spike::canary_count_with_primary_key(&source)?, count);
    assert!(
        spike::online_backup_and_empty_restore(&source, &backup, &restore).is_err(),
        "restore accepted a non-empty destination"
    );
    let scan = spike::scan_artifacts(root.path(), &spike::load_canaries()?)?;
    assert!(
        scan.findings.is_empty(),
        "plaintext hits: {:?}",
        scan.findings
    );
    Ok(())
}

#[test]
fn plaintext_default_binary_has_no_cipher_claim() -> Result<(), Box<dyn Error>> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let store_manifest = fs::read_to_string(repository.join("crates/store/Cargo.toml"))?;
    assert!(store_manifest.contains("default = [\"bundled-sqlite\"]"));
    assert!(
        store_manifest
            .contains("sqlcipher-spike = [\"rusqlite/bundled-sqlcipher-vendored-openssl\"]")
    );
    assert!(store_manifest.contains("[[bin]]"));
    assert!(store_manifest.contains("name = \"sqlcipher_spike\""));
    assert!(store_manifest.contains("path = \"src/bin/sqlcipher_spike.rs\""));
    assert!(store_manifest.contains("required-features = [\"sqlcipher-spike\"]"));

    let tree = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(&repository)
        .args([
            "tree",
            "--locked",
            "--offline",
            "--package",
            "academic-daemon",
            "--edges",
            "features",
        ])
        .output()?;
    assert!(
        tree.status.success(),
        "default feature tree failed: {}",
        String::from_utf8_lossy(&tree.stderr)
    );
    let feature_tree = String::from_utf8(tree.stdout)?;
    let normalized = feature_tree.to_ascii_lowercase();
    assert!(!normalized.contains("bundled-sqlcipher"));
    assert!(!normalized.contains("openssl-src"));
    assert!(!normalized.contains("openssl-sys"));

    let daemon_main = fs::read_to_string(repository.join("crates/daemon/src/main.rs"))?;
    let rpc_policy = fs::read_to_string(repository.join("crates/rpc/src/lib.rs"))?;
    assert!(!daemon_main.contains("cipher_version"));
    assert!(rpc_policy.contains("storage_encryption: \"NONE\""));
    assert!(rpc_policy.contains("production_data_allowed: false"));
    Ok(())
}
