//! Acceptance evidence for the `P2-K4` backup and restore half.
//!
//! Four of the eight named tests live here, because what they assert is about
//! an actual encrypted snapshot: what a restore refuses, what it verifies, what
//! a fresh machine can do with a phrase, and what two backups of one watermark
//! must agree on. The other four are about keys, recovery profiles, and the
//! ingest gate and live in `academic-recovery`.
//!
//! This whole file compiles only in the encrypted lane:
//! `cargo test -p academic-portability --no-default-features --features encrypted-portability`.

#![cfg(feature = "encrypted-portability")]

mod encrypted_support;

use std::fs;

use academic_portability::{
    PortabilityError,
    encrypted::{
        MANIFEST_FILE, ProfileKeys,
        backup::{BackupPlan, backup_encrypted_profile, verify_encrypted_backup_directory},
        restore::{
            EncryptedRestorePlan, find_unpublished_restores, open_backup_with_secret,
            recover_profile_keys, restore_encrypted_profile,
        },
    },
};
use academic_recovery::{BackupRecipientKind, RecoveryProfile};
use academic_store::path_policy::NativePathProbe;
use encrypted_support::{
    EncryptedFixture, TestResult, backup_key_set, backup_set_id, domain_id, hex_lower,
    read_recipients, recovery_secret, unlock_master_with_phrase,
};

/// Builds one backup of the fixture's profile at its current watermark.
fn take_backup(
    fixture: &EncryptedFixture,
    destination_name: &str,
) -> TestResult<academic_portability::encrypted::backup::EncryptedBackupReceipt> {
    let (root, recipients) = backup_key_set()?;
    let recovery_recipients = fixture.recovery_recipients_cbor()?;
    Ok(backup_encrypted_profile(
        fixture.profile_root(),
        &fixture.work_path(destination_name),
        fixture.master(),
        fixture.keys(),
        &BackupPlan {
            recovery_profile: RecoveryProfile::DevicePlusPhrase,
            backup_root: &root,
            backup_recipients: &recipients,
            profile_recovery_recipients: &recovery_recipients,
        },
    )?)
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// Restore activates only over a destination that is new and empty. A live
/// profile, an occupied directory, and a directory holding a single hidden file
/// are all refused before any key is derived or any byte is written.
#[test]
fn restore_rejects_nonempty_target() -> TestResult {
    let fixture = EncryptedFixture::new("restore-nonempty")?;
    let backup = fixture.work_path("backup");
    take_backup(&fixture, "backup")?;

    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 0)?;
    let authorizations = fixture.authorizations();
    let plan = EncryptedRestorePlan {
        authorizations: &authorizations,
    };

    // The live profile itself.
    let error = restore_encrypted_profile(
        &backup,
        fixture.profile_root(),
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &plan,
    )
    .err()
    .ok_or("restore activated over the live profile")?;
    assert!(
        matches!(error, PortabilityError::DestinationNotEmpty(_)),
        "restore over a live profile reported {error:?}"
    );
    // The refusal came before any work. A restore that discovered the
    // destination was occupied only when it tried to publish would already
    // have copied the whole database into a staging directory beside it.
    assert!(
        find_unpublished_restores(fixture.profile_root())?.is_empty(),
        "restore staged work beside a destination it refused"
    );
    // The live profile is untouched: it still opens and still holds its rows.
    let digest_after = canonical_digest(&fixture)?;
    assert_eq!(digest_after, canonical_digest(&fixture)?);

    // A directory holding one ordinary file.
    let occupied = fixture.work_path("occupied");
    fs::create_dir(&occupied)?;
    fs::write(occupied.join("notes.txt"), b"not empty\n")?;
    let error = restore_encrypted_profile(
        &backup,
        &occupied,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &plan,
    )
    .err()
    .ok_or("restore activated over an occupied directory")?;
    assert!(matches!(error, PortabilityError::DestinationNotEmpty(_)));
    // Nothing was written into the directory, and nothing was staged beside it.
    let entries: Vec<_> = fs::read_dir(&occupied)?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(entries.len(), 1, "restore wrote into a refused destination");
    assert!(
        find_unpublished_restores(&occupied)?.is_empty(),
        "restore staged work beside an occupied destination"
    );

    // A directory holding one dot-prefixed file, which a naive emptiness check
    // built on a shell glob would miss.
    let hidden = fixture.work_path("hidden");
    fs::create_dir(&hidden)?;
    fs::write(hidden.join(".keep"), b"")?;
    let error = restore_encrypted_profile(
        &backup,
        &hidden,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &plan,
    )
    .err()
    .ok_or("restore activated over a directory holding a dotfile")?;
    assert!(matches!(error, PortabilityError::DestinationNotEmpty(_)));
    assert!(
        find_unpublished_restores(&hidden)?.is_empty(),
        "restore staged work beside a destination holding a dotfile"
    );

    // A new empty directory is accepted, so the refusals above are about
    // emptiness rather than about a restore that cannot work at all.
    let fresh = fixture.work_path("fresh");
    fs::create_dir(&fresh)?;
    let receipt = restore_encrypted_profile(
        &backup,
        &fresh,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &plan,
    )?;
    assert_eq!(receipt.restored_object_count, 2);
    Ok(())
}

/// A restore does not trust its manifest. It re-derives the ledger chain, the
/// object closure, and the counts from the restored bytes, and refuses when any
/// of the three disagrees — including when the manifest was re-sealed with the
/// real backup key so that its signature verifies.
#[test]
fn restore_verifies_ledger_object_and_count_closure() -> TestResult {
    let fixture = EncryptedFixture::new("restore-closure")?;
    let backup = fixture.work_path("backup");
    let receipt = take_backup(&fixture, "backup")?;
    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 0)?;
    let authorizations = fixture.authorizations();

    // The honest restore succeeds and closes over both objects.
    let destination = fixture.work_path("restored");
    fs::create_dir(&destination)?;
    let restored = restore_encrypted_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &authorizations,
        },
    )?;
    assert_eq!(
        restored.canonical_semantic_digest,
        receipt.manifest.semantic.canonical_semantic_digest
    );
    assert_eq!(restored.restored_object_count, 2);
    assert_eq!(
        restored.replay.verified_events,
        receipt.manifest.semantic.counts.events
    );

    // Ledger closure: without the independent trust anchor the signed batches
    // cannot be replayed, so nothing is activated.
    let foreign = vec![fixture.foreign_authorization()?];
    let empty_destination = fixture.work_path("no-anchor");
    fs::create_dir(&empty_destination)?;
    let error = restore_encrypted_profile(
        &backup,
        &empty_destination,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &foreign,
        },
    )
    .err()
    .ok_or("restore accepted a backup it could not replay")?;
    assert!(
        matches!(
            error,
            PortabilityError::MissingAuthorization { .. } | PortabilityError::ReplayMismatch { .. }
        ),
        "replay refusal reported {error:?}"
    );
    assert!(
        !empty_destination
            .join(academic_store::STORE_DATABASE_FILE)
            .exists(),
        "a refused restore published a database"
    );

    // Object closure: a flipped ciphertext byte in one sealed object is caught
    // by the manifest's own file inventory before a restore even begins.
    let tampered_backup = fixture.work_path("tampered-object");
    copy_tree(&backup, &tampered_backup)?;
    let object_path = tampered_backup.join(&receipt.manifest.semantic.objects[0].path);
    let mut object_bytes = fs::read(&object_path)?;
    let last = object_bytes.len() - 1;
    object_bytes[last] ^= 0x01;
    fs::write(&object_path, &object_bytes)?;
    let error = verify_encrypted_backup_directory(&tampered_backup, &backup_root)
        .err()
        .ok_or("a tampered object passed backup verification")?;
    assert!(
        matches!(error, PortabilityError::IntegrityMismatch { .. }),
        "object tampering reported {error:?}"
    );

    // Count closure: the manifest is re-sealed with the real backup key after
    // its event count is altered, so the signature verifies and the digest
    // recomputes. The restore still refuses, because the counts it re-derives
    // from the restored database disagree.
    let forged_backup = fixture.work_path("forged-counts");
    copy_tree(&backup, &forged_backup)?;
    let mut forged = receipt.manifest.clone();
    forged.semantic.counts.events = forged.semantic.counts.events.saturating_add(1);
    let resealed = academic_portability::encrypted::manifest::EncryptedBackupManifest::new(
        forged.semantic,
        forged.volatile.generated_at_unix_ms,
    )?;
    fs::write(
        forged_backup.join(MANIFEST_FILE),
        resealed.seal(&backup_root, backup_set_id())?,
    )?;
    let reverified = verify_encrypted_backup_directory(&forged_backup, &backup_root)?;
    assert_eq!(
        reverified.manifest.semantic.counts.events,
        receipt.manifest.semantic.counts.events + 1,
        "the forged manifest was not the one that verified"
    );
    let forged_destination = fixture.work_path("forged-restore");
    fs::create_dir(&forged_destination)?;
    let error = restore_encrypted_profile(
        &forged_backup,
        &forged_destination,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &authorizations,
        },
    )
    .err()
    .ok_or("restore accepted a manifest whose counts it could not reproduce")?;
    assert!(
        matches!(error, PortabilityError::IntegrityMismatch { .. }),
        "count refusal reported {error:?}"
    );
    assert!(
        !forged_destination
            .join(academic_store::STORE_DATABASE_FILE)
            .exists(),
        "a refused restore published a database"
    );
    Ok(())
}

/// A fresh machine has the backup directory and the printed phrase, and nothing
/// else: no device keystore, no profile, no recipient file. That is enough, and
/// the backup directory without the phrase is not.
///
/// **"Phrase" here is the whole 256-bit secret, not 24 words.** `P2-K4` ships no
/// wordlist codec — ADR-005 records why the list is still a user decision — so
/// this test is evidence that a fresh machine recovers from *the secret a phrase
/// encodes*, and it is not evidence that any codec works.
#[test]
fn fresh_machine_restore_with_phrase_only() -> TestResult {
    let fixture = EncryptedFixture::new("fresh-machine")?;
    let backup = fixture.work_path("backup");
    let receipt = take_backup(&fixture, "backup")?;
    let expected_digest = receipt.manifest.semantic.canonical_semantic_digest.clone();

    // The fresh machine. The device keystore file the original machine used is
    // removed, so nothing on this path can reach a device wrapping key.
    let device_key_file = fixture.work_path(encrypted_support::DEVICE_KEYSTORE_FILE);
    fs::remove_file(&device_key_file)?;
    assert!(!device_key_file.exists());

    // Without the phrase the backup is inert: the wrapped root does not open,
    // so the manifest cannot be read at all.
    let wrong = academic_crypto::RecoverySecret::from_entropy([0x00; 32]);
    assert!(
        open_backup_with_secret(&backup, BackupRecipientKind::RecoveryPhrase, &wrong).is_err(),
        "a wrong phrase opened the backup"
    );

    // With the phrase alone the whole chain runs: backup root, sealed manifest,
    // the profile's recovery recipients, the Vault Master Key, the store key,
    // the domain KEKs, and the restore.
    let (backup_root, recipients) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    assert_eq!(
        hex_lower(recipients.set_id().as_bytes().as_slice()),
        hex_lower(backup_set_id().as_bytes().as_slice())
    );
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;

    // Nothing the fresh machine cannot reach is in the chain. Every recipient
    // record the backup carries is recovery-class: a device record here would
    // name a broker this machine does not have, and a restore that happened to
    // succeed anyway would be succeeding by luck rather than by contract.
    let carried = academic_crypto::RecipientSet::from_canonical_cbor(
        &academic_portability::checksum::decode_hex(
            &verified.manifest.semantic.profile_recovery_recipients,
        )
        .ok_or("the manifest carried no recipient records")?,
    )?;
    assert!(!carried.records().is_empty());
    for record in carried.records() {
        assert_eq!(
            record.kind(),
            academic_crypto::RecipientKind::RecoverySecret,
            "the backup carried a recipient the fresh machine cannot open"
        );
    }

    let recovered = recover_profile_keys(&verified, &recovery_secret(), 0)?;
    assert_eq!(
        recovered.recovery_profile,
        RecoveryProfile::DevicePlusPhrase
    );

    let destination = fixture.work_path("fresh-profile");
    fs::create_dir(&destination)?;
    let authorizations = fixture.authorizations();
    let restored = restore_encrypted_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &authorizations,
        },
    )?;
    assert_eq!(restored.canonical_semantic_digest, expected_digest);
    assert_eq!(restored.restored_object_count, 2);

    // The restored profile is a real encrypted profile: it opens under the key
    // the phrase produced, and its rows are the ones the backup described.
    let keys = ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
    let opened = academic_store::cipher::open_encrypted_profile(
        &destination,
        &NativePathProbe::default(),
        keys.store_key(),
    )?;
    assert_eq!(opened.root(), destination.as_path());
    drop(opened);

    let database = academic_portability::verify::CanonicalDatabase::open_source(
        &destination.join(academic_store::STORE_DATABASE_FILE),
        keys.store_key(),
    )?;
    let rows = academic_portability::verify::read_canonical_rows(&database)?;
    assert_eq!(
        hex_lower(rows.semantic_digest()?.as_bytes()),
        expected_digest
    );
    Ok(())
}

/// Two backups taken at one committed watermark describe the same state. Their
/// bytes cannot be equal — SQLCipher re-encrypts every page it writes — so the
/// claim is about the semantic identity digest, and the test proves the bytes
/// really do differ so the equality is not a copy artifact.
#[test]
fn two_backups_at_same_watermark_have_equal_semantic_digest() -> TestResult {
    let mut fixture = EncryptedFixture::new("equal-digest")?;
    let first = take_backup(&fixture, "backup-one")?;
    let second = take_backup(&fixture, "backup-two")?;

    assert_eq!(
        first.manifest.semantic_identity_digest, second.manifest.semantic_identity_digest,
        "two backups of one watermark disagreed on their semantic identity"
    );
    assert_eq!(
        first.manifest.semantic.canonical_semantic_digest,
        second.manifest.semantic.canonical_semantic_digest
    );
    assert_eq!(
        first.manifest.semantic.watermark,
        second.manifest.semantic.watermark
    );
    assert_eq!(
        first.manifest.semantic.counts,
        second.manifest.semantic.counts
    );

    // The physical layer differs, which is why the whole-block digest is not
    // the digest this property is about.
    assert_ne!(
        first.manifest.semantic.database.sha256, second.manifest.semantic.database.sha256,
        "two SQLCipher snapshots produced identical bytes; the equality above \
         would then be trivially true"
    );
    assert_ne!(
        first.manifest.semantic_digest,
        second.manifest.semantic_digest
    );

    // Advancing the watermark changes the identity digest, so it is not a
    // constant that would compare equal for any two backups at all.
    fixture.accept_additional_claim(1)?;
    let third = take_backup(&fixture, "backup-three")?;
    assert_ne!(
        third.manifest.semantic_identity_digest, first.manifest.semantic_identity_digest,
        "the identity digest did not move with the watermark"
    );
    assert_ne!(
        third.manifest.semantic.canonical_semantic_digest,
        first.manifest.semantic.canonical_semantic_digest
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Supporting evidence
// ---------------------------------------------------------------------------

/// `DEVICE_ONLY` cannot produce a backup, and the refusal quotes its own loss
/// statement rather than a generic error.
#[test]
fn device_only_cannot_take_an_encrypted_backup() -> TestResult {
    let fixture = EncryptedFixture::new("device-only-backup")?;
    let (root, recipients) = backup_key_set()?;
    let recovery_recipients = fixture.recovery_recipients_cbor()?;
    let error = backup_encrypted_profile(
        fixture.profile_root(),
        &fixture.work_path("backup"),
        fixture.master(),
        fixture.keys(),
        &BackupPlan {
            recovery_profile: RecoveryProfile::DeviceOnly,
            backup_root: &root,
            backup_recipients: &recipients,
            profile_recovery_recipients: &recovery_recipients,
        },
    )
    .err()
    .ok_or("DEVICE_ONLY produced a backup")?;
    assert!(
        error
            .to_string()
            .contains("OS reimage or device loss is unrecoverable"),
        "the refusal did not state the loss behaviour: {error}"
    );
    Ok(())
}

/// The snapshot beside the manifest is ciphertext. An unkeyed reader gets
/// nothing out of it, and neither does a reader with the wrong key.
#[test]
fn a_backed_up_database_is_unreadable_without_its_key() -> TestResult {
    let fixture = EncryptedFixture::new("backup-ciphertext")?;
    let backup = fixture.work_path("backup");
    take_backup(&fixture, "backup")?;
    let database = backup.join(academic_portability::encrypted::database_relative_path());
    assert!(database.is_file());

    let unkeyed = rusqlite::Connection::open_with_flags(
        &database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    assert!(
        unkeyed
            .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
            .is_err(),
        "the backed-up database was readable without a key"
    );
    drop(unkeyed);

    let wrong_master = academic_crypto::VaultMasterKey::generate()?;
    let wrong_keys = ProfileKeys::derive(
        &wrong_master,
        encrypted_support::PROFILE_ID,
        &[domain_id()?],
    )?;
    assert!(
        academic_portability::verify::CanonicalDatabase::open_copy(
            &database,
            wrong_keys.store_key()
        )
        .is_err(),
        "the backed-up database opened under the wrong key"
    );
    Ok(())
}

/// The profile's recipient file is what a fresh machine unlocks from, and the
/// backup carries only its recovery-class records.
#[test]
fn a_backup_carries_no_device_recipient() -> TestResult {
    let fixture = EncryptedFixture::new("no-device-recipient")?;
    let backup = fixture.work_path("backup");
    take_backup(&fixture, "backup")?;

    let stored = read_recipients(fixture.profile_root())?;
    assert_eq!(stored.records().len(), 2, "the profile lost a recipient");
    assert!(
        unlock_master_with_phrase(&stored).is_ok(),
        "the stored recipient set does not open with the phrase"
    );

    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
    let carried = academic_crypto::RecipientSet::from_canonical_cbor(
        &academic_portability::checksum::decode_hex(
            &verified.manifest.semantic.profile_recovery_recipients,
        )
        .ok_or("the manifest carried no recipient records")?,
    )?;
    assert!(!carried.records().is_empty());
    for record in carried.records() {
        assert_eq!(
            record.kind(),
            academic_crypto::RecipientKind::RecoverySecret,
            "a backup carried a device recipient"
        );
    }
    Ok(())
}

fn canonical_digest(fixture: &EncryptedFixture) -> TestResult<String> {
    let database = academic_portability::verify::CanonicalDatabase::open_source(
        &fixture
            .profile_root()
            .join(academic_store::STORE_DATABASE_FILE),
        fixture.keys().store_key(),
    )?;
    let rows = academic_portability::verify::read_canonical_rows(&database)?;
    Ok(hex_lower(rows.semantic_digest()?.as_bytes()))
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) -> TestResult {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
