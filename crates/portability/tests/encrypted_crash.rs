//! `BK01`-`BK04` and `RS01`-`RS04` re-run under encryption.
//!
//! These are the Phase 1 fault identifiers, at the Phase 1 positions, against
//! an encrypted profile: a SQLCipher snapshot and `AEAD_CHUNKED_V2` objects
//! under a manifest sealed with a backup key the device wrapper cannot produce.
//! The required outcome is unchanged — unpublished partial output, or a
//! complete verified destination, never a normal-looking partial state.
//!
//! **`BK03` is reached here rather than pointed at.** It fires on the second
//! object copy, so it needs two registered artifacts. Phase 1's daemon exit
//! corpus registers one and records `BK03` as `NOT_RUN` pointing at
//! `tests/crash.rs`, which does assert it. This corpus registers two, so the
//! checkpoint is hit directly and the assertion below observes exactly one
//! copied object.
//!
//! Run with:
//! `cargo test -p academic-portability --no-default-features --features encrypted-portability,phase2-fault-injection --test encrypted_crash`

#![cfg(all(feature = "encrypted-portability", feature = "phase2-fault-injection"))]

mod encrypted_support;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use academic_portability::{
    encrypted::{
        DATABASE_DIRECTORY, MANIFEST_FILE, OBJECTS_DIRECTORY, ProfileKeys,
        RESTORE_INCOMPLETE_MARKER, RESTORED_VAULT_OBJECTS_DIRECTORY,
        backup::{
            BackupPlan, backup_encrypted_profile, find_unpublished_backups,
            remove_unpublished_backup, verify_encrypted_backup_directory,
        },
        restore::{
            EncryptedRestorePlan, find_unpublished_restores, open_backup_with_secret,
            recover_profile_keys, remove_unpublished_restore, restore_encrypted_profile,
        },
    },
    fault::{FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE},
    verify::{CanonicalDatabase, read_canonical_rows},
};
use academic_recovery::{BackupRecipientKind, RecoveryProfile};
use academic_store::{INCOMPLETE_PROFILE_MARKER, path_policy::NativePathProbe};
use encrypted_support::{
    EncryptedFixture, TestResult, backup_key_set, domain_id, hex_lower, read_recipients,
    recovery_secret, unlock_master_with_phrase,
};

const CHILD_VARIABLE: &str = "ACADEMIC_ENCRYPTED_PORTABILITY_TEST_CHILD";
const OPERATION_VARIABLE: &str = "ACADEMIC_ENCRYPTED_PORTABILITY_TEST_OPERATION";
const SOURCE_VARIABLE: &str = "ACADEMIC_ENCRYPTED_PORTABILITY_TEST_SOURCE";
const DESTINATION_VARIABLE: &str = "ACADEMIC_ENCRYPTED_PORTABILITY_TEST_DESTINATION";

/// The child process. It reconstructs every key from the profile's own
/// recipient file and the fixed phrase, so it shares no process state with the
/// parent beyond four paths.
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
            let master = unlock_master_with_phrase(&read_recipients(&source)?)?;
            let keys =
                ProfileKeys::derive(&master, encrypted_support::PROFILE_ID, &[domain_id()?])?;
            let recovery_recipients =
                encrypted_support::recovery_only_recipients(&read_recipients(&source)?)?;
            let (root, recipients) = backup_key_set()?;
            backup_encrypted_profile(
                &source,
                &destination,
                &master,
                &keys,
                &BackupPlan {
                    recovery_profile: RecoveryProfile::DevicePlusPhrase,
                    backup_root: &root,
                    backup_recipients: &recipients,
                    profile_recovery_recipients: &recovery_recipients,
                },
            )?;
        }
        Some("restore") => {
            let (backup_root, _) = open_backup_with_secret(
                &source,
                BackupRecipientKind::RecoveryPhrase,
                &recovery_secret(),
            )?;
            let verified = verify_encrypted_backup_directory(&source, &backup_root)?;
            let recovered = recover_profile_keys(&verified, &recovery_secret(), 0)?;
            restore_encrypted_profile(
                &source,
                &destination,
                &NativePathProbe::default(),
                &backup_root,
                &recovered,
                &EncryptedRestorePlan {
                    authorizations: &encrypted_support::authorizations()?,
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

fn canonical_digest(profile_root: &Path, keys: &ProfileKeys) -> TestResult<String> {
    let database = CanonicalDatabase::open_source(
        &profile_root.join(academic_store::STORE_DATABASE_FILE),
        keys.store_key(),
    )?;
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

/// `BK01`-`BK04`: a terminated backup leaves an unpublished staging directory
/// and an untouched source profile, never a half-published backup.
#[test]
fn bk01_bk04_leave_no_partially_published_encrypted_backup() -> TestResult {
    for fault in ["BK01", "BK02", "BK03", "BK04"] {
        let fixture = EncryptedFixture::new(&format!("crash-{fault}"))?;
        let before = canonical_digest(fixture.profile_root(), fixture.keys())?;
        let destination = fixture.work_path("backup");
        run_child("backup", fixture.profile_root(), &destination, fault)?;

        assert!(
            !destination.exists(),
            "{fault} published a backup destination"
        );
        let staging = single_staging(&destination, find_unpublished_backups(&destination)?)?;
        assert_eq!(
            canonical_digest(fixture.profile_root(), fixture.keys())?,
            before,
            "{fault} mutated the source profile"
        );

        let manifest_present = staging.join(MANIFEST_FILE).is_file();
        let objects = count_files(&staging.join(OBJECTS_DIRECTORY))?;
        match fault {
            "BK01" | "BK02" => {
                assert!(!manifest_present, "{fault} wrote a manifest too early");
                assert_eq!(objects, 0, "{fault} copied objects too early");
            }
            "BK03" => {
                assert!(!manifest_present, "{fault} wrote a manifest too early");
                // The reachable-object copy was interrupted between the first
                // and the second object. A one-artifact corpus never reaches
                // this checkpoint, which is why Phase 1's exit corpus records
                // BK03 as NOT_RUN and defers to its own crash suite.
                assert_eq!(objects, 1, "{fault} was not interrupted midway");
            }
            _ => {
                assert!(manifest_present, "{fault} lost its synced manifest");
                // The child sealed with a root only the phrase opens, and that
                // root is recoverable from the staged key set, so the parent
                // can verify a backup it did not itself produce.
                let (staged_root, _) = open_backup_with_secret(
                    &staging,
                    BackupRecipientKind::RecoveryPhrase,
                    &recovery_secret(),
                )?;
                verify_encrypted_backup_directory(&staging, &staged_root)?;
            }
        }
        if matches!(fault, "BK01" | "BK02" | "BK03") {
            // A staging directory that is not a backup must not read as one,
            // whether or not the caller holds a key.
            let attempted = open_backup_with_secret(
                &staging,
                BackupRecipientKind::RecoveryPhrase,
                &recovery_secret(),
            );
            match attempted {
                Ok((root, _)) => assert!(
                    verify_encrypted_backup_directory(&staging, &root).is_err(),
                    "{fault} produced an acceptable backup"
                ),
                Err(_) => {}
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

        // The source is still able to produce its original snapshot.
        let (root, recipients) = backup_key_set()?;
        let recovery_recipients = fixture.recovery_recipients_cbor()?;
        let receipt = backup_encrypted_profile(
            fixture.profile_root(),
            &destination,
            fixture.master(),
            fixture.keys(),
            &BackupPlan {
                recovery_profile: RecoveryProfile::DevicePlusPhrase,
                backup_root: &root,
                backup_recipients: &recipients,
                profile_recovery_recipients: &recovery_recipients,
            },
        )?;
        verify_encrypted_backup_directory(&destination, &root)?;
        assert_eq!(
            receipt.manifest.semantic.canonical_semantic_digest, before,
            "{fault} left the source unable to produce its original snapshot"
        );
    }
    Ok(())
}

/// `RS01`-`RS04`: a terminated restore leaves an unpublished staging directory,
/// an untouched backup, and an untouched current profile. The destination is
/// either absent or a completely verified profile.
#[test]
fn rs01_rs04_leave_no_partially_activated_encrypted_profile() -> TestResult {
    for fault in ["RS01", "RS02", "RS03", "RS04"] {
        let fixture = EncryptedFixture::new(&format!("crash-{fault}"))?;
        let backup = fixture.work_path("backup");
        let (backup_root, recipients) = backup_key_set()?;
        let recovery_recipients = fixture.recovery_recipients_cbor()?;
        backup_encrypted_profile(
            fixture.profile_root(),
            &backup,
            fixture.master(),
            fixture.keys(),
            &BackupPlan {
                recovery_profile: RecoveryProfile::DevicePlusPhrase,
                backup_root: &backup_root,
                backup_recipients: &recipients,
                profile_recovery_recipients: &recovery_recipients,
            },
        )?;
        let backup_before = verify_encrypted_backup_directory(&backup, &backup_root)?;
        let profile_before = canonical_digest(fixture.profile_root(), fixture.keys())?;

        let destination = fixture.work_path("restored");
        fs::create_dir(&destination)?;
        run_child("restore", &backup, &destination, fault)?;

        assert!(
            !destination
                .join(academic_store::STORE_DATABASE_FILE)
                .exists(),
            "{fault} published a restored database"
        );
        assert!(
            academic_store::cipher::open_encrypted_profile(
                &destination,
                &NativePathProbe::default(),
                fixture.keys().store_key()
            )
            .is_err(),
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
                academic_store::cipher::open_encrypted_profile(
                    &staging,
                    &NativePathProbe::default(),
                    fixture.keys().store_key()
                )
                .is_err(),
                "{fault} left an openable incomplete profile"
            );
        }

        let backup_after = verify_encrypted_backup_directory(&backup, &backup_root)?;
        assert_eq!(
            backup_after.manifest, backup_before.manifest,
            "{fault} mutated its backup source"
        );
        assert_eq!(
            canonical_digest(fixture.profile_root(), fixture.keys())?,
            profile_before,
            "{fault} mutated the current profile"
        );

        remove_unpublished_restore(&destination, &staging)?;
        assert!(find_unpublished_restores(&destination)?.is_empty());

        let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
        let recovered = recover_profile_keys(&verified, &recovery_secret(), 0)?;
        let authorizations = fixture.authorizations();
        let receipt = restore_encrypted_profile(
            &backup,
            &destination,
            &NativePathProbe::default(),
            &backup_root,
            &recovered,
            &EncryptedRestorePlan {
                authorizations: &authorizations,
            },
        )?;
        assert_eq!(receipt.canonical_semantic_digest, profile_before);
        academic_store::cipher::open_encrypted_profile(
            &destination,
            &NativePathProbe::default(),
            fixture.keys().store_key(),
        )?;
    }
    Ok(())
}
