// The plaintext synthetic lane only. The encrypted lane cannot link this lane's
// profile API at all (t068 section 2.3-13), so under `sqlcipher-store` this file
// compiles to nothing and `tests/encrypted_profile.rs` carries the equivalent
// coverage against the schema-2 profile.
#![cfg(not(feature = "sqlcipher-store"))]

use std::{
    error::Error,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CRATE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct DownstreamCrate {
    root: PathBuf,
}

impl DownstreamCrate {
    fn new(label: &str, source: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_CRATE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-api-boundary-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("src"))?;

        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .ok_or("store manifest must be two levels below the workspace root")?;
        let store_path = toml_path(&workspace_root.join("crates/store"));
        let vault_path = toml_path(&workspace_root.join("crates/vault"));
        fs::write(
            root.join("Cargo.toml"),
            format!(
                concat!(
                    "[package]\n",
                    "name = \"academic-api-boundary-{label}\"\n",
                    "version = \"0.0.0\"\n",
                    "edition = \"2024\"\n",
                    "rust-version = \"1.98\"\n\n",
                    "[workspace]\n\n",
                    "[dependencies]\n",
                    "academic-store = {{ path = '{store_path}' }}\n",
                    "academic-vault = {{ path = '{vault_path}' }}\n",
                ),
                label = label,
                store_path = store_path,
                vault_path = vault_path,
            ),
        )?;
        fs::write(root.join("src/main.rs"), source)?;
        Ok(Self { root })
    }

    fn cargo_check(&self) -> Result<Output, Box<dyn Error>> {
        let configured_cargo = std::env::var_os("CARGO");
        let cargo = configured_cargo
            .clone()
            .unwrap_or_else(|| OsString::from("cargo"));
        let mut lock_command = Command::new(&cargo);
        if configured_cargo.is_none() {
            lock_command.arg("+1.98.0");
        }
        let lock_output = lock_command
            .args(["generate-lockfile", "--offline"])
            .current_dir(&self.root)
            .env("CARGO_NET_OFFLINE", "true")
            .output()?;
        if !lock_output.status.success() {
            return Ok(lock_output);
        }

        let mut check_command = Command::new(cargo);
        if configured_cargo.is_none() {
            check_command.arg("+1.98.0");
        }
        Ok(check_command
            .args(["check", "--locked", "--offline", "--quiet"])
            .current_dir(&self.root)
            .env("CARGO_NET_OFFLINE", "true")
            .output()?)
    }
}

impl Drop for DownstreamCrate {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!(
                "test cleanup failed for downstream crate {}: {error}",
                self.root.display()
            );
        }
    }
}

#[test]
fn downstream_crate_cannot_open_raw_writer_or_execute_sql() -> Result<(), Box<dyn Error>> {
    let downstream = DownstreamCrate::new(
        "writer",
        r#"
use academic_store::{
    connection::{WriterConnection, open_writer},
    profile::SyntheticProfile,
};

fn attempt(profile: &SyntheticProfile) {
    let _writer_type: Option<WriterConnection> = None;
    let _writer = open_writer(profile.database_path());
    let _second_writer = profile.open_writer();
    let mut acceptance = profile.open_acceptance_store().unwrap();
    let _ = acceptance.execute("INSERT INTO command_receipt DEFAULT VALUES", []);
    let _ = acceptance.execute_batch("DELETE FROM ledger_batch;");
}

fn main() {}
"#,
    )?;
    let output = downstream.cargo_check()?;
    assert!(
        !output.status.success(),
        "raw writer probe unexpectedly compiled"
    );
    let stderr = String::from_utf8(output.stderr)?;
    for denied_name in [
        "WriterConnection",
        "open_writer",
        "open_writer()",
        "execute",
        "execute_batch",
    ] {
        assert!(
            stderr.contains(denied_name),
            "compiler did not report the denied `{denied_name}` API:\n{stderr}"
        );
    }
    Ok(())
}

#[test]
fn external_crate_cannot_mint_sealed_object_capability() -> Result<(), Box<dyn Error>> {
    let downstream = DownstreamCrate::new(
        "capability",
        r#"
use academic_store::{SealedObjectReceipt, SealedObjectVerifier};
use academic_vault::{SealDisposition, SealedArtifactReceipt, SealedObjectCapability};

fn attempt() {
    let _capability = SealedObjectCapability::new(
        todo!(),
        todo!(),
        SealDisposition::PublishedNew,
    );
    let _receipt = SealedArtifactReceipt {};
}

fn main() {}
"#,
    )?;
    let output = downstream.cargo_check()?;
    assert!(
        !output.status.success(),
        "sealed-object capability minting probe unexpectedly compiled"
    );
    let stderr = String::from_utf8(output.stderr)?;
    for denied_name in [
        "SealedObjectReceipt",
        "SealedObjectVerifier",
        "SealedObjectCapability",
        "private associated function",
        "private fields",
    ] {
        assert!(
            stderr.contains(denied_name),
            "compiler did not report the denied `{denied_name}` seam:\n{stderr}"
        );
    }
    Ok(())
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
