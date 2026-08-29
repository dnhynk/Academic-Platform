//! Empty-profile restore behaviour and vendor-neutral round trip.

#![cfg(feature = "plaintext-portability")]

mod support;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use academic_portability::{
    PortabilityError,
    backup::{DATABASE_DIRECTORY, MANIFEST_FILE, backup_profile, verify_backup_directory},
    checksum::{encode_hex, hash_file},
    export::{export_profile, verify_export_directory},
    manifest::{BackupManifest, FileEntry},
    restore::{RestorePlan, find_unpublished_restores, restore_profile},
};
use academic_store::{path_policy::NativePathProbe, profile::open_synthetic_profile};
use support::{Fixture, TestResult, projection_builder_digest, projection_config_hash};

fn plan<'a>(
    authorizations: &'a [academic_contracts::DeviceAuthorization],
    projections: &'a [academic_portability::restore::ProjectionRebuildTarget],
    policies: &'a academic_projections::resolution::PredicatePolicies,
) -> RestorePlan<'a> {
    RestorePlan {
        authorizations,
        projections,
        predicate_policies: Some(policies),
        projection_builder_digest: projection_builder_digest(),
        projection_config_hash: projection_config_hash(),
    }
}

fn reseal_backup(root: &Path) -> TestResult {
    let manifest_path = root.join(MANIFEST_FILE);
    let mut manifest = BackupManifest::from_json_bytes(&fs::read(&manifest_path)?)?;
    let mut files = Vec::new();
    for entry in &manifest.semantic.files {
        let path = root.join(&entry.path);
        let (digest, byte_length) = hash_file(&path)?;
        files.push(FileEntry {
            path: entry.path.clone(),
            byte_length,
            sha256: encode_hex(digest.as_bytes()),
        });
    }
    let database_path = manifest.semantic.database.path.clone();
    manifest.semantic.database = files
        .iter()
        .find(|entry| entry.path == database_path)
        .cloned()
        .ok_or("resealed manifest lost its database entry")?;
    for object in &mut manifest.semantic.objects {
        let entry = files
            .iter()
            .find(|file| file.path == object.path)
            .ok_or("resealed manifest lost an object entry")?;
        object.byte_length = entry.byte_length;
        object.plaintext_sha256.clone_from(&entry.sha256);
    }
    manifest.semantic.files = files;
    let resealed = BackupManifest::seal(manifest.semantic, manifest.volatile.generated_at_unix_ms)?;
    fs::write(&manifest_path, resealed.to_json_bytes()?)?;
    verify_backup_directory(root)?;
    Ok(())
}

fn assert_unpublished(destination: &Path) -> TestResult {
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => return Err("a failed restore published its destination".into()),
    }
    Ok(())
}

#[test]
fn restore_rejects_nonempty_target() -> TestResult {
    let fixture = Fixture::new("restore-nonempty")?;
    let backup = fixture.work_path("backup");
    backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;

    let destination = fixture.work_path("restored");
    fs::create_dir(&destination)?;
    fs::write(destination.join("existing.txt"), b"not empty\n")?;

    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;
    let result = restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    );
    assert!(
        matches!(result, Err(PortabilityError::DestinationNotEmpty(_))),
        "restore accepted a non-empty destination: {result:?}"
    );
    assert!(destination.join("existing.txt").is_file());
    assert!(find_unpublished_restores(&destination)?.is_empty());

    fs::remove_file(destination.join("existing.txt"))?;
    let receipt = restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    )?;
    assert_eq!(receipt.destination, destination);
    Ok(())
}

#[test]
fn restore_checks_integrity_and_foreign_keys() -> TestResult {
    let fixture = Fixture::new("restore-integrity")?;
    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;

    let foreign_key_backup = fixture.work_path("backup-foreign-key");
    backup_profile(
        fixture.profile_root(),
        &foreign_key_backup,
        fixture.keyring()?,
    )?;
    {
        let path = foreign_key_backup
            .join(DATABASE_DIRECTORY)
            .join(academic_store::STORE_DATABASE_FILE);
        let connection = rusqlite::Connection::open(&path)?;
        connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
        connection.execute(
            "INSERT INTO claim_evidence (claim_id, evidence_id, evidence_ordinal) \
             VALUES (?1, ?2, 0)",
            rusqlite::params![vec![0x7a_u8; 16], vec![0x7b_u8; 16]],
        )?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        drop(connection);
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
            if sidecar.exists() {
                fs::remove_file(sidecar)?;
            }
        }
    }
    reseal_backup(&foreign_key_backup)?;
    let destination = fixture.work_path("restored-foreign-key");
    let result = restore_profile(
        &foreign_key_backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    );
    assert!(
        matches!(
            result,
            Err(PortabilityError::DatabaseCheckFailed {
                check: "foreign_key_check",
                ..
            })
        ),
        "restore accepted a foreign-key violation: {result:?}"
    );
    assert_unpublished(&destination)?;

    let integrity_backup = fixture.work_path("backup-integrity");
    backup_profile(
        fixture.profile_root(),
        &integrity_backup,
        fixture.keyring()?,
    )?;
    {
        let path = integrity_backup
            .join(DATABASE_DIRECTORY)
            .join(academic_store::STORE_DATABASE_FILE);
        let mut bytes = fs::read(&path)?;
        assert!(bytes.len() > 8_192, "the snapshot is too small to corrupt");
        let offset = bytes.len() / 2;
        for byte in bytes.iter_mut().skip(offset).take(64) {
            *byte ^= 0x5a;
        }
        fs::write(&path, &bytes)?;
    }
    reseal_backup(&integrity_backup)?;
    let corrupt_destination = fixture.work_path("restored-integrity");
    let result = restore_profile(
        &integrity_backup,
        &corrupt_destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    );
    assert!(
        result.is_err(),
        "restore accepted a structurally corrupt database"
    );
    assert_unpublished(&corrupt_destination)?;
    Ok(())
}

#[test]
fn restore_detects_missing_or_corrupt_object() -> TestResult {
    let fixture = Fixture::new("restore-objects")?;
    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;

    let missing = fixture.work_path("backup-missing");
    let receipt = backup_profile(fixture.profile_root(), &missing, fixture.keyring()?)?;
    let object = receipt
        .manifest
        .semantic
        .objects
        .first()
        .cloned()
        .ok_or("backup carried no objects")?;
    fs::remove_file(missing.join(&object.path))?;
    let destination = fixture.work_path("restored-missing");
    assert!(
        restore_profile(
            &missing,
            &destination,
            &NativePathProbe::default(),
            fixture.keyring()?,
            &plan(&authorizations, &targets, &policies),
        )
        .is_err(),
        "restore accepted a backup with a missing object"
    );
    assert_unpublished(&destination)?;

    let corrupt = fixture.work_path("backup-corrupt");
    backup_profile(fixture.profile_root(), &corrupt, fixture.keyring()?)?;
    let path = corrupt.join(&object.path);
    let mut bytes = fs::read(&path)?;
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    fs::write(&path, &bytes)?;
    reseal_backup(&corrupt)?;
    let corrupt_destination = fixture.work_path("restored-corrupt");
    let result = restore_profile(
        &corrupt,
        &corrupt_destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    );
    assert!(
        result.is_err(),
        "restore accepted an object whose bytes no longer match the signed descriptor"
    );
    assert_unpublished(&corrupt_destination)?;
    Ok(())
}

#[test]
fn restore_requires_independent_device_authorization() -> TestResult {
    let fixture = Fixture::new("restore-anchor")?;
    let backup = fixture.work_path("backup");
    backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;
    let foreign = vec![fixture.foreign_authorization()?];
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;
    let destination = fixture.work_path("restored");
    let result = restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&foreign, &targets, &policies),
    );
    assert!(
        matches!(result, Err(PortabilityError::MissingAuthorization { .. })),
        "restore replayed batches without an independent trust anchor: {result:?}"
    );
    assert_unpublished(&destination)?;
    Ok(())
}

#[test]
fn restore_rebuilds_projection_checksums() -> TestResult {
    let fixture = Fixture::new("restore-projections")?;
    let source: BTreeMap<String, String> =
        fixture.source_projection_checksums()?.into_iter().collect();
    assert_eq!(source.len(), 3);

    let backup = fixture.work_path("backup");
    backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;
    let verified = verify_backup_directory(&backup)?;
    for entry in &verified.manifest.semantic.files {
        assert!(
            !entry.path.contains("projection"),
            "the backup carried projection state at {}",
            entry.path
        );
    }

    let destination = fixture.work_path("restored");
    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;
    let receipt = restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    )?;

    let restored: BTreeMap<String, String> = receipt
        .projections
        .iter()
        .map(|generation| {
            (
                generation.kind.clone(),
                generation.canonical_checksum.clone(),
            )
        })
        .collect();
    assert_eq!(
        restored, source,
        "projections rebuilt from empty disagreed with the source generations"
    );
    assert!(receipt.projections.iter().all(|entry| entry.activated));
    assert_eq!(
        receipt.replay.verified_events,
        receipt.manifest.semantic.counts.events
    );
    Ok(())
}

#[test]
fn vendor_offline_round_trip() -> TestResult {
    let fixture = Fixture::new("round-trip")?;
    let source_export = fixture.work_path("export-source");
    let source_receipt =
        export_profile(fixture.profile_root(), &source_export, fixture.keyring()?)?;
    let verified_export = verify_export_directory(&source_export)?;
    assert_eq!(verified_export.manifest, source_receipt.manifest);

    let backup = fixture.work_path("backup");
    backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;
    verify_backup_directory(&backup)?;

    let destination = fixture.work_path("restored");
    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;
    let restore_receipt = restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    )?;
    assert_eq!(
        restore_receipt.canonical_semantic_digest,
        source_receipt.manifest.semantic.canonical_semantic_digest
    );

    let reopened = open_synthetic_profile(&destination, &NativePathProbe::default())?;
    assert_eq!(reopened.root(), destination.as_path());

    let restored_export = fixture.work_path("export-restored");
    let restored_receipt = export_profile(&destination, &restored_export, fixture.keyring()?)?;
    verify_export_directory(&restored_export)?;

    assert_eq!(
        restored_receipt.manifest.semantic_digest, source_receipt.manifest.semantic_digest,
        "a restored profile did not export the same canonical semantics"
    );
    assert_eq!(
        restored_receipt.manifest.semantic.files,
        source_receipt.manifest.semantic.files
    );
    for entry in &source_receipt.manifest.semantic.files {
        assert_eq!(
            fs::read(source_export.join(&entry.path))?,
            fs::read(restored_export.join(&entry.path))?,
            "exported file {} differed after a round trip",
            entry.path
        );
    }
    Ok(())
}

#[test]
fn restore_leaves_the_source_backup_untouched() -> TestResult {
    let fixture = Fixture::new("restore-source-intact")?;
    let backup = fixture.work_path("backup");
    let receipt = backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;
    let before: Vec<(String, String)> = receipt
        .manifest
        .semantic
        .files
        .iter()
        .map(|entry| (entry.path.clone(), entry.sha256.clone()))
        .collect();

    let destination = fixture.work_path("restored");
    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;
    restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    )?;

    let after = verify_backup_directory(&backup)?;
    let observed: Vec<(String, String)> = after
        .manifest
        .semantic
        .files
        .iter()
        .map(|entry| (entry.path.clone(), entry.sha256.clone()))
        .collect();
    assert_eq!(before, observed, "restore mutated its backup source");
    assert!(find_unpublished_restores(&destination)?.is_empty());
    Ok(())
}

/// Linux permission case: a published restored profile keeps the store's
/// owner-only creation identity, including its sealed object namespace.
#[cfg(unix)]
#[test]
fn restored_profile_is_owner_only_on_unix() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("restore-permission")?;
    let backup = fixture.work_path("backup");
    backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;
    let destination = fixture.work_path("restored");
    let authorizations = fixture.authorizations();
    let targets = fixture.projection_targets()?;
    let policies = fixture.policies()?;
    restore_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        fixture.keyring()?,
        &plan(&authorizations, &targets, &policies),
    )?;

    let mut checked = 0_usize;
    let mut pending = vec![destination.clone()];
    while let Some(directory) = pending.pop() {
        let mode = fs::symlink_metadata(&directory)?.permissions().mode();
        assert_eq!(
            mode & 0o077,
            0,
            "restored directory {} is group or world accessible",
            directory.display()
        );
        checked += 1;
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if fs::symlink_metadata(&path)?.is_dir() {
                pending.push(path);
            }
        }
    }
    assert!(checked > 5, "the restored profile had no vault namespace");
    open_synthetic_profile(&destination, &NativePathProbe::default())?;
    Ok(())
}
