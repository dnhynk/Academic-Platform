//! BK01-BK04 and RS01-RS04 process-kill matrix.
//!
//! Every outcome must be either unpublished partial output or a complete
//! verified destination. A partially activated profile or a half-published
//! backup is never allowed.

#![cfg(all(feature = "phase1-fault-injection", feature = "plaintext-portability"))]

mod support;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use academic_portability::{
    RESTORE_INCOMPLETE_MARKER,
    backup::{
        DATABASE_DIRECTORY, MANIFEST_FILE, OBJECTS_DIRECTORY, backup_profile,
        find_unpublished_backups, remove_unpublished_backup, verify_backup_directory,
    },
    fault::{FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE},
    manifest::BackupManifest,
    restore::{
        RESTORED_VAULT_OBJECTS_DIRECTORY, RestorePlan, find_unpublished_restores,
        remove_unpublished_restore, restore_profile,
    },
    verify::{CanonicalDatabase, read_canonical_rows},
};
use academic_store::{
    INCOMPLETE_PROFILE_MARKER, path_policy::NativePathProbe, profile::open_synthetic_profile,
};
use support::{
    Fixture, TestResult, hex_lower, projection_builder_digest, projection_config_hash,
    synthetic_authorizations, synthetic_keyring, synthetic_policies, synthetic_projection_targets,
};

const CHILD_VARIABLE: &str = "ACADEMIC_PORTABILITY_TEST_CHILD";
const OPERATION_VARIABLE: &str = "ACADEMIC_PORTABILITY_TEST_OPERATION";
const SOURCE_VARIABLE: &str = "ACADEMIC_PORTABILITY_TEST_SOURCE";
const DESTINATION_VARIABLE: &str = "ACADEMIC_PORTABILITY_TEST_DESTINATION";

#[test]
fn fault_child_entrypoint() -> TestResult {
    if env::var(CHILD_VARIABLE).ok().as_deref() != Some("1") {
        return Ok(());
    }
    let source = env::var_os(SOURCE_VARIABLE)
        .map(PathBuf::from)
        .ok_or("fault child source path was not supplied")?;
    let destination = env::var_os(DESTINATION_VARIABLE)
        .map(PathBuf::from)
        .ok_or("fault child destination path was not supplied")?;
    match env::var(OPERATION_VARIABLE).ok().as_deref() {
        Some("backup") => {
            backup_profile(&source, &destination, synthetic_keyring()?)?;
        }
        Some("restore") => {
            let manifest = BackupManifest::from_json_bytes(&fs::read(source.join(MANIFEST_FILE))?)?;
            let authorizations = synthetic_authorizations()?;
            let targets =
                synthetic_projection_targets(manifest.semantic.watermark.accept_seq_head)?;
            let policies = synthetic_policies()?;
            restore_profile(
                &source,
                &destination,
                &NativePathProbe::default(),
                synthetic_keyring()?,
                &RestorePlan {
                    authorizations: &authorizations,
                    projections: &targets,
                    predicate_policies: Some(&policies),
                    projection_builder_digest: projection_builder_digest(),
                    projection_config_hash: projection_config_hash(),
                },
            )?;
        }
        _ => return Err("fault child operation was not supplied".into()),
    }
    Err("the selected portability fault did not terminate the child process".into())
}

fn run_child(operation: &str, source: &Path, destination: &Path, fault: &str) -> TestResult {
    let ready = destination
        .parent()
        .ok_or("destination has no parent")?
        .join(format!("{fault}.ready"));
    let status = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("fault_child_entrypoint")
        .arg("--nocapture")
        .env(CHILD_VARIABLE, "1")
        .env(OPERATION_VARIABLE, operation)
        .env(SOURCE_VARIABLE, source)
        .env(DESTINATION_VARIABLE, destination)
        .env(FAULT_SELECTION_VARIABLE, fault)
        .env(FAULT_READY_MARKER_VARIABLE, &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    assert!(!status.success(), "{fault} child unexpectedly succeeded");
    assert_eq!(
        fs::read_to_string(&ready)?,
        fault,
        "{fault} child did not reach its named checkpoint"
    );
    Ok(())
}

fn canonical_digest(profile_root: &Path) -> TestResult<String> {
    let database =
        CanonicalDatabase::open_source(&profile_root.join(academic_store::STORE_DATABASE_FILE))?;
    let rows = read_canonical_rows(&database)?;
    Ok(hex_lower(rows.semantic_digest()?.as_bytes()))
}

fn single_staging(destination: &Path, staged: Vec<PathBuf>) -> TestResult<PathBuf> {
    assert_eq!(
        staged.len(),
        1,
        "expected exactly one unpublished staging directory beside {}",
        destination.display()
    );
    staged.into_iter().next().ok_or_else(|| {
        Box::<dyn std::error::Error>::from("staging directory disappeared between checks")
    })
}

fn count_files(root: &Path) -> TestResult<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let mut total = 0;
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            total += count_files(&path)?;
        } else {
            total += 1;
        }
    }
    Ok(total)
}

#[test]
fn bk01_bk04_leave_no_partially_published_backup() -> TestResult {
    for fault in ["BK01", "BK02", "BK03", "BK04"] {
        let fixture = Fixture::new(&format!("crash-{fault}"))?;
        let before = canonical_digest(fixture.profile_root())?;
        let destination = fixture.work_path("backup");
        run_child("backup", fixture.profile_root(), &destination, fault)?;

        assert!(
            !destination.exists(),
            "{fault} published a backup destination"
        );
        let staging = single_staging(&destination, find_unpublished_backups(&destination)?)?;
        assert_eq!(
            canonical_digest(fixture.profile_root())?,
            before,
            "{fault} mutated the source profile"
        );

        let manifest_present = staging.join(MANIFEST_FILE).is_file();
        let objects = count_files(&staging.join(OBJECTS_DIRECTORY))?;
        match fault {
            "BK01" | "BK02" => {
                assert!(!manifest_present, "{fault} wrote a manifest too early");
                assert_eq!(objects, 0, "{fault} copied objects too early");
                assert!(
                    verify_backup_directory(&staging).is_err(),
                    "{fault} produced an acceptable backup"
                );
            }
            "BK03" => {
                assert!(!manifest_present, "{fault} wrote a manifest too early");
                assert_eq!(objects, 1, "{fault} was not interrupted midway");
                assert!(
                    verify_backup_directory(&staging).is_err(),
                    "{fault} produced an acceptable backup"
                );
            }
            _ => {
                assert!(manifest_present, "{fault} lost its synced manifest");
                verify_backup_directory(&staging)?;
            }
        }
        if fault == "BK01" {
            assert!(
                staging
                    .join(DATABASE_DIRECTORY)
                    .join(academic_store::STORE_DATABASE_FILE)
                    .exists(),
                "BK01 did not begin an online backup"
            );
        }

        remove_unpublished_backup(&destination, &staging)?;
        assert!(find_unpublished_backups(&destination)?.is_empty());
        let receipt = backup_profile(fixture.profile_root(), &destination, fixture.keyring()?)?;
        verify_backup_directory(&destination)?;
        assert_eq!(
            receipt.manifest.semantic.canonical_semantic_digest, before,
            "{fault} left the source unable to produce its original snapshot"
        );
    }
    Ok(())
}

#[test]
fn rs01_rs04_leave_no_partially_activated_profile() -> TestResult {
    for fault in ["RS01", "RS02", "RS03", "RS04"] {
        let fixture = Fixture::new(&format!("crash-{fault}"))?;
        let backup = fixture.work_path("backup");
        backup_profile(fixture.profile_root(), &backup, fixture.keyring()?)?;
        let backup_before = verify_backup_directory(&backup)?;
        let profile_before = canonical_digest(fixture.profile_root())?;

        let destination = fixture.work_path("restored");
        run_child("restore", &backup, &destination, fault)?;

        assert!(
            !destination.exists(),
            "{fault} published a restore destination"
        );
        assert!(
            open_synthetic_profile(&destination, &NativePathProbe::default()).is_err(),
            "{fault} left an openable profile at the destination"
        );
        let staging = single_staging(&destination, find_unpublished_restores(&destination)?)?;
        let database = staging.join(academic_store::STORE_DATABASE_FILE);
        let objects = count_files(&staging.join(RESTORED_VAULT_OBJECTS_DIRECTORY))?;
        match fault {
            "RS01" => {
                assert!(
                    staging.join(RESTORE_INCOMPLETE_MARKER).is_file(),
                    "RS01 left no recognizable restore marker"
                );
                assert!(staging.join(INCOMPLETE_PROFILE_MARKER).is_file());
                assert!(!database.exists(), "RS01 copied a database too early");
            }
            "RS02" => {
                assert!(database.is_file(), "RS02 did not copy the database");
                assert_eq!(objects, 0, "RS02 copied objects too early");
                assert!(staging.join(INCOMPLETE_PROFILE_MARKER).is_file());
            }
            "RS03" => {
                assert!(database.is_file());
                assert_eq!(
                    u64::try_from(objects)?,
                    u64::try_from(backup_before.manifest.semantic.objects.len())?,
                    "RS03 did not copy every object before its checkpoint"
                );
                assert!(staging.join(INCOMPLETE_PROFILE_MARKER).is_file());
            }
            _ => {
                assert!(database.is_file());
                assert!(
                    !staging.join(INCOMPLETE_PROFILE_MARKER).exists(),
                    "RS04 should have completed every check before publication"
                );
            }
        }
        if matches!(fault, "RS01" | "RS02" | "RS03") {
            assert!(
                open_synthetic_profile(&staging, &NativePathProbe::default()).is_err(),
                "{fault} left an openable incomplete profile"
            );
        }

        let backup_after = verify_backup_directory(&backup)?;
        assert_eq!(
            backup_after.manifest, backup_before.manifest,
            "{fault} mutated its backup source"
        );
        assert_eq!(
            canonical_digest(fixture.profile_root())?,
            profile_before,
            "{fault} mutated the current profile"
        );

        remove_unpublished_restore(&destination, &staging)?;
        assert!(find_unpublished_restores(&destination)?.is_empty());

        let authorizations = fixture.authorizations();
        let targets = fixture.projection_targets()?;
        let policies = fixture.policies()?;
        let receipt = restore_profile(
            &backup,
            &destination,
            &NativePathProbe::default(),
            fixture.keyring()?,
            &RestorePlan {
                authorizations: &authorizations,
                projections: &targets,
                predicate_policies: Some(&policies),
                projection_builder_digest: projection_builder_digest(),
                projection_config_hash: projection_config_hash(),
            },
        )?;
        assert_eq!(receipt.canonical_semantic_digest, profile_before);
        open_synthetic_profile(&destination, &NativePathProbe::default())?;
    }
    Ok(())
}
