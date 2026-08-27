#![cfg(feature = "phase1-fault-injection")]

#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use academic_domain::{
    ArtifactDescriptor, ArtifactId, Confidentiality, ContentDigest, DomainId, MediaType,
    PermissionLineageId, RetentionClass, VaultLocator,
};
use academic_vault::{
    DomainKeyring, ReconcileOptions, ReconcileState, SealDisposition, VAULT_FORMAT_VERSION, Vault,
};
use synthetic_artifacts::{
    ARTIFACT_ID, DOMAIN_ID, DOMAIN_KEY, PERMISSION_LINEAGE_ID, SyntheticTestRoot,
    create_private_test_root, large_artifact_bytes,
};

const CHILD_ENV: &str = "ACADEMIC_VAULT_TEST_CHILD";
const PROFILE_ENV: &str = "ACADEMIC_VAULT_TEST_PROFILE";
const FAULT_ENV: &str = "ACADEMIC_VAULT_TEST_FAULT";
const READY_ENV: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";

#[test]
fn fault_child_entrypoint() -> Result<(), Box<dyn Error>> {
    if env::var(CHILD_ENV).ok().as_deref() != Some("1") {
        return Ok(());
    }

    let root = env::var_os(PROFILE_ENV)
        .map(PathBuf::from)
        .ok_or("fault child profile path was not supplied")?;
    create_private_test_root(&root)?;
    let vault = open_vault(&root)?;
    let request = synthetic_artifacts::ingest_request()?;
    let bytes = large_artifact_bytes();
    let _receipt = vault.ingest(&request, bytes.as_slice())?;
    Err("selected vault fault did not terminate the child process".into())
}

#[test]
fn v01_v06_process_crash_matrix_recovers_without_false_receipts() -> Result<(), Box<dyn Error>> {
    for fault in ["V01", "V02", "V03", "V04", "V05", "V06"] {
        exercise_fault(fault)?;
    }
    Ok(())
}

fn exercise_fault(fault: &str) -> Result<(), Box<dyn Error>> {
    let root = SyntheticTestRoot::new(&format!("crash-{fault}"))?;
    let ready = root.path().join(format!("{fault}.ready"));
    let status = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("fault_child_entrypoint")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(PROFILE_ENV, root.path())
        .env(FAULT_ENV, fault)
        .env(READY_ENV, &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    assert!(!status.success(), "{fault} child unexpectedly succeeded");
    assert_eq!(fs::read_to_string(&ready)?, fault);

    let vault = open_vault(root.path())?;
    let bytes = large_artifact_bytes();
    let expected = expected_descriptor(&bytes)?;
    let retry_candidates = [expected.clone()];
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + Duration::from_secs(48 * 60 * 60))
            .with_retry_candidates(&retry_candidates)
            .with_temp_expiry(Duration::from_secs(60 * 60))
            .with_orphan_grace(Duration::from_secs(60 * 60)),
    )?;

    assert_eq!(count_extension(vault.layout().temp_dir(), "partial")?, 0);
    let pre_retry_objects = count_extension(vault.layout().objects_root(), "obj")?;
    if matches!(fault, "V05" | "V06") {
        assert_eq!(pre_retry_objects, 1, "{fault} lost its published object");
        assert!(report.records().iter().any(|record| {
            record.state() == ReconcileState::ValidOrphan
                && record.artifact_id() == Some(expected.id)
        }));
    } else {
        assert_eq!(pre_retry_objects, 0, "{fault} published before rename");
    }

    let receipt = vault.ingest(&synthetic_artifacts::ingest_request()?, bytes.as_slice())?;
    let expected_disposition = if matches!(fault, "V05" | "V06") {
        SealDisposition::AdoptedExisting
    } else {
        SealDisposition::PublishedNew
    };
    assert_eq!(receipt.disposition(), expected_disposition, "{fault}");
    assert_eq!(receipt.descriptor(), &expected);
    assert_eq!(fs::read(receipt.object_path())?, bytes);
    assert_eq!(count_extension(vault.layout().objects_root(), "obj")?, 1);
    Ok(())
}

fn open_vault(profile_root: &Path) -> Result<Vault, Box<dyn Error>> {
    let domain_id: DomainId = DOMAIN_ID.parse()?;
    let mut keyring = DomainKeyring::new();
    keyring.insert(domain_id, DOMAIN_KEY)?;
    Ok(Vault::open(profile_root, keyring)?)
}

fn expected_descriptor(bytes: &[u8]) -> Result<ArtifactDescriptor, Box<dyn Error>> {
    let id: ArtifactId = ARTIFACT_ID.parse()?;
    let domain_id: DomainId = DOMAIN_ID.parse()?;
    let permission_lineage_id: PermissionLineageId = PERMISSION_LINEAGE_ID.parse()?;
    let media_type = MediaType::parse("application/pdf")?;
    let content_digest = ContentDigest::sha256(bytes);
    let vault_locator = VaultLocator::derive(
        DOMAIN_KEY,
        VAULT_FORMAT_VERSION,
        &media_type,
        content_digest,
    )?;
    Ok(ArtifactDescriptor {
        id,
        content_digest,
        media_type,
        byte_length: u64::try_from(bytes.len())?,
        domain_id,
        confidentiality: Confidentiality::Restricted,
        retention_class: RetentionClass::UserManaged,
        permission_lineage_id,
        format_version: VAULT_FORMAT_VERSION,
        vault_locator,
        evidence_representations: Vec::new(),
    })
}

fn count_extension(root: &Path, extension: &str) -> Result<usize, Box<dyn Error>> {
    let mut count = 0_usize;
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_dir() {
            count = count.saturating_add(count_extension(&path, extension)?);
        } else if metadata.file_type().is_file()
            && path.extension().and_then(|value| value.to_str()) == Some(extension)
        {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}
