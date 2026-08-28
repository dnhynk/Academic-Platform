#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    collections::BTreeSet,
    error::Error,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use academic_domain::RetentionClass;
use academic_vault::{
    ReconcileOptions, ReconcileReport, ReconcileState, SealDisposition, VaultError,
};
use synthetic_artifacts::{
    ARTIFACT_ID, DOMAIN_ID, PERMISSION_LINEAGE_ID, SAMPLE_BYTES, SECOND_ARTIFACT_ID,
    SECOND_DOMAIN_ID, SECOND_PERMISSION_LINEAGE_ID, ingest_request, open_test_vault, request_with,
};

const FUTURE: Duration = Duration::from_secs(48 * 60 * 60);
const ONE_HOUR: Duration = Duration::from_secs(60 * 60);
const FIXED_RECONCILE_MILLIS: u64 = 1_788_033_675_111;
const THIRD_ARTIFACT_ID: &str = "01900000-0000-7000-8000-000000000103";
const PREOCCUPIED_QUARANTINE_BYTES: &[u8] = b"preoccupied quarantine bytes\n";
const MAX_QUARANTINE_FILENAME_BYTES: usize = 157;

#[test]
fn same_policy_retry_adopts_sealed_orphan() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("adopt-valid-orphan")?;
    let request = ingest_request()?;
    let sealed = vault.ingest(&request, SAMPLE_BYTES)?;
    let candidates = [sealed.descriptor().clone()];
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + FUTURE)
            .with_retry_candidates(&candidates)
            .with_orphan_grace(ONE_HOUR),
    )?;

    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::ValidOrphan
            && record.artifact_id() == Some(sealed.descriptor().id)
    }));
    assert!(sealed.object_path().is_file());
    let adopted = vault.ingest(&request, SAMPLE_BYTES)?;
    assert_eq!(adopted.disposition(), SealDisposition::AdoptedExisting);
    assert_eq!(adopted.object_path(), sealed.object_path());
    Ok(())
}

#[test]
fn expired_temp_is_scavenged() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("expired-temp")?;
    let temp = vault.layout().temp_dir().join("expired.partial");
    fs::write(&temp, b"partial")?;
    let report = vault
        .reconcile(&ReconcileOptions::new(SystemTime::now() + FUTURE).with_temp_expiry(ONE_HOUR))?;

    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::TempExpiredRemoved && record.path() == temp
    }));
    assert!(!temp.exists());
    Ok(())
}

#[test]
fn live_temp_is_not_scavenged() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("live-temp")?;
    let temp = vault.layout().temp_dir().join("live.partial");
    fs::write(&temp, b"partial")?;
    let lock = hold_exclusive(&temp)?;
    let report = vault
        .reconcile(&ReconcileOptions::new(SystemTime::now() + FUTURE).with_temp_expiry(ONE_HOUR))?;

    assert!(
        report
            .records()
            .iter()
            .any(|record| { record.state() == ReconcileState::TempLive && record.path() == temp })
    );
    assert!(temp.is_file());
    drop(lock);
    Ok(())
}

#[test]
fn referenced_missing_object_is_repair_required() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("referenced-missing")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let object_path = sealed.object_path().to_path_buf();
    let referenced = [sealed.descriptor().clone()];
    fs::remove_file(&object_path)?;
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + FUTURE)
            .with_referenced(&referenced)
            .with_orphan_grace(Duration::ZERO),
    )?;

    assert!(report.repair_required());
    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::ReferencedMissingRepairRequired
            && record.artifact_id() == Some(sealed.descriptor().id)
    }));
    assert!(!object_path.exists());
    Ok(())
}

#[test]
fn referenced_corrupt_object_is_repair_required() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("referenced-corrupt")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let object_path = sealed.object_path().to_path_buf();
    let referenced = [sealed.descriptor().clone()];
    fs::write(&object_path, b"corrupt but still referenced")?;
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + FUTURE)
            .with_referenced(&referenced)
            .with_orphan_grace(Duration::ZERO),
    )?;

    assert!(report.repair_required());
    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::ReferencedCorruptRepairRequired
            && record.path() == object_path
    }));
    assert!(object_path.is_file());
    assert_eq!(fs::read(object_path)?, b"corrupt but still referenced");
    Ok(())
}

#[test]
fn unrecognized_expired_orphan_is_quarantined() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("quarantine-orphan")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let original = sealed.object_path().to_path_buf();
    drop(sealed);
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + FUTURE).with_orphan_grace(ONE_HOUR),
    )?;
    let quarantined = report
        .records()
        .iter()
        .find(|record| record.state() == ReconcileState::QuarantinedOrphan)
        .ok_or("orphan was not quarantined")?;

    assert!(!original.exists());
    assert!(quarantined.path().is_file());
    assert_eq!(quarantined.artifact_id(), None);
    Ok(())
}

#[test]
fn live_sealed_capability_defers_product_quarantine() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("live-capability-lease")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let original = sealed.object_path().to_path_buf();
    let options =
        ReconcileOptions::new(SystemTime::now() + FUTURE).with_orphan_grace(Duration::ZERO);

    let leased = vault.reconcile(&options)?;
    assert!(leased.records().iter().any(|record| {
        record.state() == ReconcileState::OrphanLeaseHeld && record.path() == original
    }));
    assert!(original.is_file());

    drop(sealed);
    let released = vault.reconcile(&options)?;
    assert!(released.records().iter().any(|record| {
        record.state() == ReconcileState::QuarantinedOrphan && record.path() != original
    }));
    assert!(!original.exists());
    Ok(())
}

#[test]
fn referenced_valid_object_survives_orphan_grace() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("referenced-valid")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let referenced = [sealed.descriptor().clone()];
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + FUTURE)
            .with_referenced(&referenced)
            .with_orphan_grace(Duration::ZERO),
    )?;

    assert!(!report.repair_required());
    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::ReferencedValid && record.path() == sealed.object_path()
    }));
    assert_eq!(fs::read(sealed.object_path())?, SAMPLE_BYTES);
    Ok(())
}

#[test]
fn audit_cross_policy_orphans_collide_in_flat_quarantine_namespace() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("audit-cross-policy-quarantine")?;
    let first = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;
    let second = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::LegalHold,
            PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;

    assert_eq!(
        first.descriptor().vault_locator,
        second.descriptor().vault_locator
    );
    assert_ne!(first.object_path(), second.object_path());
    let first_live_path = first.object_path().to_path_buf();
    let second_live_path = second.object_path().to_path_buf();
    drop(first);
    drop(second);
    let options = ReconcileOptions::new(fixed_reconcile_time()).with_orphan_grace(Duration::ZERO);

    let report = vault.reconcile(&options)?;
    let quarantined = quarantined_paths(&report);
    assert_eq!(quarantined.len(), 2);
    for path in &quarantined {
        assert_eq!(fs::read(path)?, SAMPLE_BYTES);
        assert_portable_quarantine_filename(path)?;
    }
    assert!(!first_live_path.exists());
    assert!(!second_live_path.exists());
    assert!(object_files(vault.layout().objects_root())?.is_empty());

    let repeated = vault.reconcile(&options)?;
    assert_eq!(quarantined_paths(&repeated), quarantined);
    assert!(object_files(vault.layout().objects_root())?.is_empty());
    Ok(())
}

#[test]
fn quarantine_identity_separates_permission_lineage_and_domain() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("quarantine-policy-identity")?;
    let first = vault.ingest(
        &request_with(
            ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;
    let second = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            SECOND_PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;
    let third = vault.ingest(
        &request_with(
            THIRD_ARTIFACT_ID,
            SECOND_DOMAIN_ID,
            RetentionClass::UserManaged,
            PERMISSION_LINEAGE_ID,
        )?,
        SAMPLE_BYTES,
    )?;

    let first_locator = first.descriptor().vault_locator.clone();
    let second_locator = second.descriptor().vault_locator.clone();
    let third_locator = third.descriptor().vault_locator.clone();
    assert_eq!(first_locator, second_locator);
    assert_ne!(first_locator, third_locator);
    drop(first);
    drop(second);
    drop(third);
    let report = vault.reconcile(
        &ReconcileOptions::new(fixed_reconcile_time()).with_orphan_grace(Duration::ZERO),
    )?;
    let quarantined = quarantined_paths(&report);
    assert_eq!(quarantined.len(), 3);
    for path in &quarantined {
        assert_eq!(fs::read(path)?, SAMPLE_BYTES);
        assert_portable_quarantine_filename(path)?;
    }
    assert!(object_files(vault.layout().objects_root())?.is_empty());
    Ok(())
}

#[test]
fn preoccupied_quarantine_destination_never_overwrites_bytes() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("preoccupied-quarantine")?;
    let request = ingest_request()?;
    let first = vault.ingest(&request, SAMPLE_BYTES)?;
    let live_path = first.object_path().to_path_buf();
    drop(first);
    let options = ReconcileOptions::new(fixed_reconcile_time()).with_orphan_grace(Duration::ZERO);
    let first_report = vault.reconcile(&options)?;
    let destination = quarantined_paths(&first_report)
        .into_iter()
        .next()
        .ok_or("first orphan was not quarantined")?;
    fs::write(&destination, PREOCCUPIED_QUARANTINE_BYTES)?;

    let republished = vault.ingest(&request, SAMPLE_BYTES)?;
    assert_eq!(republished.object_path(), live_path);
    drop(republished);
    let result = vault.reconcile(&options);
    let collision = match result {
        Err(VaultError::PathCollision(path)) => path,
        other => return Err(format!("expected PathCollision, observed {other:?}").into()),
    };

    assert_eq!(collision, destination);
    assert_eq!(fs::read(&destination)?, PREOCCUPIED_QUARANTINE_BYTES);
    assert_eq!(fs::read(&live_path)?, SAMPLE_BYTES);
    Ok(())
}

#[cfg(windows)]
#[test]
fn retry_candidate_io_error_does_not_quarantine() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("retry-io-error")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let original = sealed.object_path().to_path_buf();
    let candidates = [sealed.descriptor().clone()];
    drop(sealed);
    let lock = hold_exclusive(&original)?;
    let result = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + FUTURE)
            .with_retry_candidates(&candidates)
            .with_orphan_grace(Duration::ZERO),
    );

    assert!(matches!(result, Err(academic_vault::VaultError::Io { .. })));
    assert!(original.is_file());
    assert!(
        fs::read_dir(vault.layout().quarantine_dir())?
            .filter_map(Result::ok)
            .all(
                |entry| entry.path().extension().and_then(|value| value.to_str()) != Some("orphan")
            )
    );
    drop(lock);
    Ok(())
}

#[cfg(unix)]
fn hold_exclusive(path: &Path) -> Result<File, Box<dyn Error>> {
    use rustix::fs::{FlockOperation, flock};

    let file = OpenOptions::new().read(true).write(true).open(path)?;
    flock(&file, FlockOperation::NonBlockingLockExclusive)?;
    Ok(file)
}

#[cfg(windows)]
fn hold_exclusive(path: &Path) -> Result<File, Box<dyn Error>> {
    use std::os::windows::fs::OpenOptionsExt;

    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)?)
}

fn fixed_reconcile_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(FIXED_RECONCILE_MILLIS)
}

fn quarantined_paths(report: &ReconcileReport) -> BTreeSet<PathBuf> {
    report
        .records()
        .iter()
        .filter(|record| record.state() == ReconcileState::QuarantinedOrphan)
        .map(|record| record.path().to_path_buf())
        .collect()
}

fn assert_portable_quarantine_filename(path: &Path) -> Result<(), Box<dyn Error>> {
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("quarantine filename was not Unicode")?;
    assert!(filename.is_ascii());
    assert!(filename.len() <= MAX_QUARANTINE_FILENAME_BYTES);
    let stem = filename
        .strip_suffix(".orphan")
        .ok_or("quarantine filename had the wrong extension")?;
    let components = stem.split('-').collect::<Vec<_>>();
    assert_eq!(components.len(), 3);
    assert_eq!(components[0].len(), 13);
    assert_eq!(components[1].len(), 64);
    assert_eq!(components[2].len(), 64);
    assert!(components[0].bytes().all(|byte| byte.is_ascii_digit()));
    assert!(components[1].bytes().all(is_lower_hex));
    assert!(components[2].bytes().all(is_lower_hex));
    Ok(())
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn object_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_object_files(directory, &mut files)?;
    Ok(files)
}

fn collect_object_files(directory: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_object_files(&entry.path(), files)?;
        } else if file_type.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("obj")
        {
            files.push(entry.path());
        }
    }
    Ok(())
}
