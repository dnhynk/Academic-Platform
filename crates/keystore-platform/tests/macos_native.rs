//! Explicit macOS-only execution; default tests never contact a Keychain.
#![cfg(target_os = "macos")]

use academic_keystore_platform::{
    KeystoreError, KeystoreErrorCode, KeystoreLabel, PROVIDER, PurgeOutcome, open, purge, seal,
};
use std::{error::Error, fs, path::PathBuf, process::Command};

type TestResult = Result<(), Box<dyn Error>>;
const CONTEXT: &str = "ACADEMIC_MACOS_KEYCHAIN_TEST_CONTEXT";
const SECRET: [u8; 32] = [0x71; 32];

fn require_context(expected: &str) -> TestResult {
    if std::env::var(CONTEXT).as_deref() != Ok(expected) {
        return Err("explicit disposable synthetic Keychain context required".into());
    }
    assert_eq!(PROVIDER.as_str(), "MACOS_KEYCHAIN_DATA_PROTECTION_V1");
    Ok(())
}

fn unique_label() -> Result<KeystoreLabel, Box<dyn Error>> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce)?;
    let suffix: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(KeystoreLabel::new(&format!(
        "academic-os:test:t251:{suffix}"
    ))?)
}

struct Item {
    label: KeystoreLabel,
    blob: Vec<u8>,
}

impl Drop for Item {
    fn drop(&mut self) {
        // Exact returned generation only, even when an assertion unwinds.
        if !self.blob.is_empty()
            && let Err(error) = purge(&self.label, &self.blob)
        {
            eprintln!("synthetic task-item cleanup failed: {error}");
        }
    }
}

#[test]
#[ignore = "explicit hosted disposable unprovisioned context only; refusal is not a positive broker result"]
fn macos_keychain_unprovisioned_identity_refuses_without_fallback() -> TestResult {
    require_context("disposable-unprovisioned-v1")?;
    let label = unique_label()?;
    match seal(&label, &SECRET) {
        Err(error) => {
            assert!(
                matches!(
                    error.code,
                    KeystoreErrorCode::Unavailable | KeystoreErrorCode::AccessDenied
                ),
                "unprovisioned context must refuse with a redacted availability/access category: {error}"
            );
            println!(
                "macOS refusal category={:?} os_code={:?}",
                error.code, error.os_code
            );
        }
        Ok(blob) => {
            // Unexpected success must fail this refusal test after exact cleanup.
            assert_eq!(purge(&label, &blob)?, PurgeOutcome::Removed);
            return Err(
                "host unexpectedly has a working identity; refusal evidence cannot pass".into(),
            );
        }
    }
    Ok(())
}

fn missing(result: Result<academic_keystore_platform::RecoveredSecret, KeystoreError>) {
    assert_eq!(
        result.err().map(|error| error.code),
        Some(KeystoreErrorCode::NotFound)
    );
}

#[test]
#[ignore = "requires separately reviewed provisioned identity in a disposable signed-in synthetic user context"]
fn macos_keychain_positive_contract_and_process_reopen() -> TestResult {
    require_context("disposable-provisioned-v1")?;
    let label = unique_label()?;
    let directory = std::env::temp_dir().join(label.as_str().replace(':', "-"));
    fs::create_dir(&directory)?;
    let artifact = OpaqueArtifact(directory.join("opaque.blob"));
    let executable = std::env::current_exe()?;
    // The writer exits before this process first opens the key. Only the opaque
    // non-secret blob crosses the process boundary; the synthetic fixture key
    // is a constant in the two copies of the same test binary.
    let status = Command::new(&executable)
        .args([
            "--exact",
            "macos_keychain_process_writer",
            "--ignored",
            "--test-threads=1",
        ])
        .env("ACADEMIC_MACOS_KEYCHAIN_CHILD_LABEL", label.as_str())
        .env("ACADEMIC_MACOS_KEYCHAIN_CHILD_BLOB", &artifact.0)
        .status()?;
    let item = Item {
        label,
        blob: fs::read(&artifact.0)?,
    };
    assert!(
        status.success(),
        "writer must exit successfully before reopen"
    );
    for _ in 0..17 {
        assert_eq!(open(&item.label, &item.blob)?.expose(), &SECRET);
    }
    let other = unique_label()?;
    assert_eq!(
        open(&other, &item.blob).err().map(|error| error.code),
        Some(KeystoreErrorCode::InvalidSealedBlob)
    );
    assert_eq!(
        purge(&other, &item.blob).err().map(|error| error.code),
        Some(KeystoreErrorCode::InvalidSealedBlob)
    );
    let mut foreign = item.blob.clone();
    foreign[5] = 1;
    assert_eq!(
        open(&item.label, &foreign).err().map(|error| error.code),
        Some(KeystoreErrorCode::ProviderMismatch)
    );
    assert_eq!(
        purge(&item.label, &foreign).err().map(|error| error.code),
        Some(KeystoreErrorCode::ProviderMismatch)
    );
    let mut corrupt = item.blob.clone();
    if let Some(last) = corrupt.last_mut() {
        *last ^= 1;
    }
    missing(open(&item.label, &corrupt));
    assert_eq!(purge(&item.label, &corrupt)?, PurgeOutcome::NothingStored);
    assert_eq!(
        seal(&item.label, &[0x72; 32]).err().map(|error| error.code),
        Some(KeystoreErrorCode::DuplicateLabel)
    );
    assert_eq!(open(&item.label, &item.blob)?.expose(), &SECRET);
    assert_eq!(purge(&item.label, &item.blob)?, PurgeOutcome::Removed);
    missing(open(&item.label, &item.blob));
    assert_eq!(purge(&item.label, &item.blob)?, PurgeOutcome::NothingStored);
    let replacement = Item {
        label: item.label.clone(),
        blob: seal(&item.label, &[0x73; 32])?,
    };
    missing(open(&item.label, &item.blob));
    assert_eq!(purge(&item.label, &item.blob)?, PurgeOutcome::NothingStored);
    assert_eq!(
        open(&replacement.label, &replacement.blob)?.expose(),
        &[0x73; 32]
    );
    assert_eq!(
        purge(&replacement.label, &replacement.blob)?,
        PurgeOutcome::Removed
    );
    missing(open(&replacement.label, &replacement.blob));
    Ok(())
}

struct OpaqueArtifact(PathBuf);

impl Drop for OpaqueArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        if let Some(directory) = self.0.parent() {
            let _ = fs::remove_dir(directory);
        }
    }
}

#[test]
#[ignore = "child of macos_keychain_positive_contract_and_process_reopen; not standalone evidence"]
fn macos_keychain_process_writer() -> TestResult {
    require_context("disposable-provisioned-v1")?;
    let label = KeystoreLabel::new(&std::env::var("ACADEMIC_MACOS_KEYCHAIN_CHILD_LABEL")?)?;
    assert!(label.as_str().starts_with("academic-os:test:t251:"));
    let path = PathBuf::from(std::env::var("ACADEMIC_MACOS_KEYCHAIN_CHILD_BLOB")?);
    let expected = std::env::temp_dir()
        .join(label.as_str().replace(':', "-"))
        .join("opaque.blob");
    assert_eq!(path, expected);
    let mut item = Item {
        blob: seal(&label, &SECRET)?,
        label,
    };
    // create_new prevents overwriting anything supplied at this path.
    use std::io::Write as _;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&item.blob)?;
    file.sync_all()?;
    // Intentionally persist the native item for the parent; all CF/ObjC
    // references were already released by seal. An empty blob cannot purge it.
    item.blob.clear();
    Ok(())
}
