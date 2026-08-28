//! Fixed-watermark plaintext backup behaviour.

mod support;

use std::{collections::BTreeSet, fs};

use academic_portability::{
    PHASE1_BACKUP_PLAINTEXT_WARNING,
    backup::{
        DATABASE_DIRECTORY, MANIFEST_FILE, MANIFEST_SCHEMA_FILE, OBJECTS_DIRECTORY, backup_profile,
        verify_backup_directory,
    },
    export::export_profile,
    verify::{CanonicalDatabase, read_canonical_rows},
};
use support::{Fixture, TestResult, hex_lower};

#[test]
fn backup_snapshot_matches_fixed_watermark() -> TestResult {
    let mut fixture = Fixture::new("backup-watermark")?;
    let destination = fixture.work_path("backup");
    let receipt = backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    let fixed = receipt.manifest.semantic.watermark;
    assert_eq!(fixed.accept_seq_head, fixture.known_at_accept_seq());

    fixture.accept_additional_claim(1)?;
    assert!(fixture.known_at_accept_seq() > fixed.accept_seq_head);

    let verified = verify_backup_directory(&destination)?;
    assert_eq!(verified.manifest.semantic.watermark, fixed);
    assert_eq!(
        verified.manifest.semantic.counts,
        receipt.manifest.semantic.counts
    );
    assert_eq!(
        verified.manifest.semantic.device_heads,
        receipt.manifest.semantic.device_heads
    );

    let snapshot = CanonicalDatabase::open_copy(
        &destination
            .join(DATABASE_DIRECTORY)
            .join(academic_store::STORE_DATABASE_FILE),
    )?;
    snapshot.integrity_check()?;
    snapshot.foreign_key_check()?;
    let rows = read_canonical_rows(&snapshot)?;
    assert_eq!(
        rows.watermark, fixed,
        "the snapshot drifted from its watermark"
    );
    assert_eq!(
        hex_lower(rows.semantic_digest()?.as_bytes()),
        receipt.manifest.semantic.canonical_semantic_digest,
        "the snapshot's canonical semantic digest is not the manifested one"
    );
    Ok(())
}

#[test]
fn backup_object_closure_is_complete() -> TestResult {
    let fixture = Fixture::new("backup-closure")?;
    let destination = fixture.work_path("backup");
    let receipt = backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    let semantic = &receipt.manifest.semantic;
    assert!(
        semantic.objects.len() >= 2,
        "the closure test needs at least two reachable objects"
    );
    assert_eq!(
        u64::try_from(semantic.objects.len())?,
        semantic.counts.artifacts,
        "backup object closure did not cover every registered artifact"
    );

    let listed: BTreeSet<&str> = semantic
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    for object in &semantic.objects {
        assert!(
            object.path.starts_with(OBJECTS_DIRECTORY),
            "object {} left the exported object namespace",
            object.artifact_id
        );
        assert!(listed.contains(object.path.as_str()));
        let bytes = fs::read(destination.join(&object.path))?;
        assert_eq!(u64::try_from(bytes.len())?, object.byte_length);
        assert_eq!(
            hex_lower(academic_domain::ContentDigest::sha256(&bytes).as_bytes()),
            object.plaintext_sha256,
            "copied object bytes disagree with the manifested plaintext digest"
        );
    }

    assert!(
        !destination.join("vault").exists(),
        "backup mirrored the vault's deep policy namespace instead of a flat object set"
    );
    assert!(listed.contains(MANIFEST_SCHEMA_FILE));
    verify_backup_directory(&destination)?;
    Ok(())
}

#[test]
fn backup_manifest_never_claims_confidentiality() -> TestResult {
    let fixture = Fixture::new("backup-warning")?;
    let destination = fixture.work_path("backup");
    let receipt = backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    assert!(!receipt.manifest.semantic.encrypted);
    assert_eq!(
        receipt.manifest.semantic.plaintext_warning,
        PHASE1_BACKUP_PLAINTEXT_WARNING
    );
    assert!(!receipt.manifest.semantic.policy.production_data_allowed);
    assert_eq!(receipt.manifest.semantic.policy.storage_encryption, "NONE");

    let manifest = fs::read_to_string(destination.join(MANIFEST_FILE))?;
    assert!(manifest.contains("NOT ENCRYPTED"));
    assert!(manifest.contains("NOT CONFIDENTIAL"));
    Ok(())
}

#[test]
fn backup_refuses_an_existing_destination() -> TestResult {
    let fixture = Fixture::new("backup-existing")?;
    let destination = fixture.work_path("backup");
    backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    assert!(
        backup_profile(fixture.profile_root(), &destination, fixture.keyring()?).is_err(),
        "backup overwrote an existing destination"
    );
    verify_backup_directory(&destination)?;
    Ok(())
}

#[test]
fn backup_verification_rejects_a_corrupt_file() -> TestResult {
    let fixture = Fixture::new("backup-corrupt")?;
    let destination = fixture.work_path("backup");
    let receipt = backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    verify_backup_directory(&destination)?;

    let object = receipt
        .manifest
        .semantic
        .objects
        .first()
        .ok_or("backup carried no objects")?;
    let path = destination.join(&object.path);
    let mut bytes = fs::read(&path)?;
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(&path, &bytes)?;
    assert!(
        verify_backup_directory(&destination).is_err(),
        "a corrupt object survived backup verification"
    );
    Ok(())
}

#[test]
fn backup_and_export_agree_on_canonical_semantics() -> TestResult {
    let fixture = Fixture::new("backup-export-agree")?;
    let backup = backup_profile(
        fixture.profile_root(),
        &fixture.work_path("backup"),
        fixture.keyring()?,
    )?;
    let export = export_profile(
        fixture.profile_root(),
        &fixture.work_path("export"),
        fixture.keyring()?,
    )?;
    assert_eq!(
        backup.manifest.semantic.canonical_semantic_digest,
        export.manifest.semantic.canonical_semantic_digest,
        "backup and export disagreed about the same committed watermark"
    );
    assert_eq!(
        backup.manifest.semantic.watermark,
        export.manifest.semantic.watermark
    );
    assert_eq!(
        backup.manifest.semantic.counts,
        export.manifest.semantic.counts
    );
    Ok(())
}

/// Linux permission case: a destination whose parent forbids creation fails
/// closed, publishes nothing, and leaves no staging directory behind.
#[cfg(unix)]
#[test]
fn backup_fails_closed_on_a_read_only_destination_parent() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("backup-permission")?;
    let parent = fixture.work_path("locked");
    fs::create_dir(&parent)?;
    let destination = parent.join("backup");
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500))?;

    let result = backup_profile(fixture.profile_root(), &destination, fixture.keyring()?);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))?;
    assert!(
        result.is_err(),
        "backup published into a destination parent it could not write"
    );
    assert!(!destination.exists());
    assert!(
        fs::read_dir(&parent)?.next().is_none(),
        "backup left residue"
    );

    backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
    verify_backup_directory(&destination)?;
    Ok(())
}
