#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    path::Path,
    time::{Duration, SystemTime},
};

use academic_vault::{ReconcileOptions, ReconcileState, SealDisposition};
use synthetic_artifacts::{SAMPLE_BYTES, ingest_request, open_test_vault};

const FUTURE: Duration = Duration::from_secs(48 * 60 * 60);
const ONE_HOUR: Duration = Duration::from_secs(60 * 60);

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

#[cfg(windows)]
#[test]
fn retry_candidate_io_error_does_not_quarantine() -> Result<(), Box<dyn Error>> {
    let (_root, vault) = open_test_vault("retry-io-error")?;
    let sealed = vault.ingest(&ingest_request()?, SAMPLE_BYTES)?;
    let original = sealed.object_path().to_path_buf();
    let candidates = [sealed.descriptor().clone()];
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
