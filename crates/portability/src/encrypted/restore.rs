//! Independent restore of an encrypted backup into a new empty profile.
//!
//! Restore never writes into a live profile and never re-uses the machine's
//! device keystore. It recovers the Vault Master Key from the profile's own
//! recovery-class recipient records — which travel inside the sealed manifest —
//! so a fresh machine with the recovery phrase and the backup directory is
//! enough, and a fresh machine with the backup directory alone is not.
//!
//! The order is fixed: accept only a new empty destination that is not inside
//! the backup, open the manifest, verify file digests, stage a profile, copy
//! the database, check cipher and b-tree integrity, compare schema, watermark,
//! counts and heads, replay every signed batch against independent trust
//! anchors, close over every object and authenticate it, re-apply every
//! tombstone the backup carries, then publish with one rename.
//!
//! # Where the tombstones land in that order
//!
//! A `P2-K5` crypto-shred destroys the live key slot and writes a tombstone
//! into each backup that still holds a copy. Re-deletion happens here, in the
//! staging tree, after every object has been authenticated and before the
//! rename that publishes it: a restore therefore never publishes a profile
//! holding a key slot the profile it came from had destroyed, and the failure
//! of a tombstone to apply is a failed restore rather than a silent
//! resurrection. Authenticating first is what keeps "the backup was intact"
//! and "the deletion was re-applied" two separate, observable facts.

use std::path::{Path, PathBuf};

use academic_contracts::DeviceAuthorization;
use academic_crypto::{
    ProfileId, RecipientSet, RecoverySecret, UnlockThrottle, VaultMasterKey, unlock_with_recovery,
};
use academic_recovery::{
    BackupMasterKey, BackupRecipientKind, BackupRecipientSet, RecoveryProfile,
};
use academic_retention::engine::{AppliedTombstones, SparedObject};
use academic_store::{
    INCOMPLETE_PROFILE_MARKER, cipher::prepare_encrypted_profile, path_policy::PathProbe,
};
use academic_vault::{EncryptedVault, SealedObjectVerifier as _};

use crate::{
    PortabilityError, PortabilityResult, RESTORE_INCOMPLETE_MARKER,
    checksum::{decode_hex, encode_hex},
    directory,
    encrypted::{
        ProfileKeys,
        backup::{
            VerifiedEncryptedBackup, read_backup_recipients, verify_encrypted_backup_directory,
        },
        manifest::EncryptedBackupManifest,
        require_unreadable_without_key,
    },
    fault::{self, PortabilityFaultPoint},
    verify::{CanonicalDatabase, ReplayReport, read_artifact_descriptors, read_canonical_rows},
};

/// Everything a restore needs that does not come out of the backup.
///
/// Authorizations are deliberately not read from the backup: a signing key
/// carried inside a restored envelope would authenticate nothing.
#[derive(Debug)]
pub struct EncryptedRestorePlan<'a> {
    pub authorizations: &'a [DeviceAuthorization],
}

/// Receipt returned by a published restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedRestoreReceipt {
    pub destination: PathBuf,
    pub manifest: EncryptedBackupManifest,
    pub replay: ReplayReport,
    pub canonical_semantic_digest: String,
    pub restored_object_count: u64,
    /// Locators re-deleted from the restored tree by the backup's tombstones.
    ///
    /// Sorted. Empty when the backup carries no tombstone, which is the case
    /// for a backup taken from a profile that never deleted an artifact.
    pub re_deleted_locators: Vec<String>,
    /// Objects a tombstone's locator reached whose artifact it does not name.
    ///
    /// Sorted, and empty for every backup whose profile never registered the
    /// same bytes twice in one domain. A locator carries no permission lineage,
    /// so two such registrations share one; these are the copies the deletion
    /// deliberately left readable, and naming them is how a restore says which
    /// artifact it destroyed and which it did not.
    pub spared_objects: Vec<SparedObject>,
    /// Tombstoned locators that reached no object in the restored tree.
    ///
    /// Sorted. A tombstone names the artifact it was written for, the locator
    /// the live shred destroyed, and every locator the artifact's chain moved
    /// through before it, so a copy under any of the artifact's names is
    /// reached; an entry here is a deletion this backup could not carry out,
    /// and it is reported rather than dropped. The ordinary cause is a backup
    /// taken before the artifact was registered.
    pub absent_locators: Vec<String>,
}

/// The key material a restore recovered from a backup and one secret.
///
/// This is the whole chain a fresh machine walks: phrase, backup root, sealed
/// manifest, the profile's recovery recipients, the Vault Master Key. Nothing
/// in it touches an operating-system broker.
#[derive(Debug)]
pub struct RecoveredProfileKeys {
    pub master: VaultMasterKey,
    pub profile_id: ProfileId,
    pub recovery_profile: RecoveryProfile,
}

/// Recovers the backup root from a recovery secret alone.
pub fn open_backup_with_secret(
    backup_root_directory: &Path,
    kind: BackupRecipientKind,
    secret: &RecoverySecret,
) -> PortabilityResult<(BackupMasterKey, BackupRecipientSet)> {
    let recipients = read_backup_recipients(backup_root_directory)?;
    let root = recipients.open(kind, secret)?;
    Ok((root, recipients))
}

/// Recovers the Vault Master Key from a verified backup and one recovery secret.
///
/// The recipient records come out of the *sealed* manifest, so this cannot
/// succeed for a caller who could not already open the manifest.
pub fn recover_profile_keys(
    verified: &VerifiedEncryptedBackup,
    secret: &RecoverySecret,
    now_ms: u64,
) -> PortabilityResult<RecoveredProfileKeys> {
    let semantic = &verified.manifest.semantic;
    let recipient_bytes = decode_hex(&semantic.profile_recovery_recipients).ok_or(
        PortabilityError::ManifestRejected {
            field: "profile_recovery_recipients",
        },
    )?;
    let set = RecipientSet::from_canonical_cbor(&recipient_bytes).map_err(|_| {
        PortabilityError::ManifestRejected {
            field: "profile_recovery_recipients",
        }
    })?;
    let profile_bytes = decode_hex(&semantic.profile_id)
        .and_then(|bytes| <[u8; 16]>::try_from(bytes.as_slice()).ok())
        .ok_or(PortabilityError::ManifestRejected {
            field: "profile_id",
        })?;
    let profile_id = ProfileId::from_bytes(profile_bytes);
    let recovery_profile = RecoveryProfile::parse(&semantic.recovery_profile).ok_or(
        PortabilityError::ManifestRejected {
            field: "recovery_profile",
        },
    )?;

    let mut throttle = UnlockThrottle::new();
    let mut last: Option<PortabilityError> = None;
    for record in set.records() {
        if record.kind() != academic_crypto::RecipientKind::RecoverySecret {
            // A device recipient in a backup would be a recipient that cannot
            // exist on the machine the backup is being restored onto.
            return Err(PortabilityError::ManifestRejected {
                field: "profile_recovery_recipients",
            });
        }
        match unlock_with_recovery(record, profile_id, secret, &mut throttle, now_ms) {
            Ok(master) => {
                return Ok(RecoveredProfileKeys {
                    master,
                    profile_id,
                    recovery_profile,
                });
            }
            Err(error) => {
                last = Some(PortabilityError::RecoveryUnlockRefused {
                    reason: error.to_string(),
                });
            }
        }
    }
    Err(last.unwrap_or(PortabilityError::ManifestRejected {
        field: "profile_recovery_recipients",
    }))
}

/// Restores one verified backup into a new empty profile directory.
pub fn restore_encrypted_profile<P: PathProbe + ?Sized>(
    backup_root_directory: &Path,
    destination: &Path,
    probe: &P,
    backup_root: &BackupMasterKey,
    recovered: &RecoveredProfileKeys,
    plan: &EncryptedRestorePlan<'_>,
) -> PortabilityResult<EncryptedRestoreReceipt> {
    directory::require_new_empty_directory(destination)?;
    require_outside_backup(backup_root_directory, destination)?;
    let verified = verify_encrypted_backup_directory(backup_root_directory, backup_root)?;

    let staging = directory::reserve_staging_path(destination, "restore-staging")?;
    let incomplete = prepare_encrypted_profile(&staging, probe)?;
    let restore_marker = staging.join(RESTORE_INCOMPLETE_MARKER);
    directory::write_new_file(
        &restore_marker,
        restore_marker_contents(destination).as_bytes(),
    )?;
    directory::sync_directory(&staging)?;
    drop(incomplete);

    fault::trip(PortabilityFaultPoint::Rs01);

    let outcome = build_restored_profile(&staging, &verified, recovered, plan);
    let (replay, canonical_semantic_digest, restored_object_count) = outcome?;
    let tombstoned = apply_backup_tombstones(backup_root_directory, &staging)?;

    remove_marker(&staging, RESTORE_INCOMPLETE_MARKER)?;
    remove_marker(&staging, INCOMPLETE_PROFILE_MARKER)?;
    directory::sync_tree(&staging)?;

    fault::trip(PortabilityFaultPoint::Rs04);

    directory::publish_over_empty(&staging, destination)?;
    Ok(EncryptedRestoreReceipt {
        destination: destination.to_path_buf(),
        manifest: verified.manifest,
        replay,
        canonical_semantic_digest,
        restored_object_count,
        re_deleted_locators: tombstoned.applied,
        spared_objects: tombstoned.spared,
        absent_locators: tombstoned.absent,
    })
}

/// Refuses a destination inside the backup being restored.
///
/// A restore into the backup's own tree publishes a directory the backup does
/// not list, so the backup stops verifying against its own manifest — the
/// deletion is silent and permanent, and the destination looked like a
/// perfectly good new empty directory on the way in. The comparison is over
/// canonical paths, and the destination does not exist yet, so its nearest
/// existing ancestor is what is compared.
fn require_outside_backup(
    backup_root_directory: &Path,
    destination: &Path,
) -> PortabilityResult<()> {
    let backup = std::fs::canonicalize(backup_root_directory).map_err(|source| {
        PortabilityError::io(
            "resolve backup root for containment",
            backup_root_directory,
            source,
        )
    })?;
    let mut candidate = destination.to_path_buf();
    loop {
        match std::fs::canonicalize(&candidate) {
            Ok(resolved) => {
                if resolved == backup || resolved.starts_with(&backup) {
                    return Err(PortabilityError::UnsafeEntry(destination.to_path_buf()));
                }
                return Ok(());
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = candidate.parent().map(Path::to_path_buf) else {
                    return Ok(());
                };
                if parent == candidate {
                    return Ok(());
                }
                candidate = parent;
            }
            Err(source) => {
                return Err(PortabilityError::io(
                    "resolve restore destination for containment",
                    destination,
                    source,
                ));
            }
        }
    }
}

/// Re-applies every tombstone the backup carries to the staged object tree.
///
/// This is the restore half of a `P2-K5` deletion and it is the product path:
/// `academic-retention` owns the tombstone record and the positioned write, and
/// this calls it. The re-deletion needs no key — the locator is cleartext at a
/// fixed header offset — so what it proves is that a deletion reaches the
/// copies a backup holds, not merely that a record of one was filed.
///
/// A tombstone names the artifact it was written for, the locator the live shred
/// destroyed, and every locator the artifact's reference chain moved through
/// before it, so a backup taken before a rotation — which holds the object under
/// an older name — is reached by the same record. Whichever of an artifact's
/// names this backup happens to hold, one tombstone reaches it, and an object
/// that merely shares a locator because it holds the same bytes under another
/// lineage is left intact and reported.
///
/// A tombstone that reached no object at all is not an error: the artifact may
/// have been registered after the backup was taken, or shredded before it. It
/// is returned in `absent` and carried on the receipt, because a deletion this
/// backup could not carry out is a fact the caller has to be told. A tombstone
/// that names an object that *is* present and cannot be shredded is a failed
/// restore.
fn apply_backup_tombstones(
    backup_root_directory: &Path,
    staging: &Path,
) -> PortabilityResult<AppliedTombstones> {
    let tombstones = academic_retention::tombstone::read_from_backup(backup_root_directory)
        .map_err(|source| PortabilityError::Tombstone(source.to_string()))?;
    if tombstones.is_empty() {
        return Ok(AppliedTombstones::default());
    }
    let objects_root = staging.join(crate::encrypted::RESTORED_VAULT_OBJECTS_DIRECTORY);
    let applied = academic_retention::engine::apply_tombstones(&objects_root, &tombstones)
        .map_err(|source| PortabilityError::Tombstone(source.to_string()))?;
    Ok(applied)
}

type RestoreOutcome = (ReplayReport, String, u64);

fn build_restored_profile(
    staging: &Path,
    verified: &VerifiedEncryptedBackup,
    recovered: &RecoveredProfileKeys,
    plan: &EncryptedRestorePlan<'_>,
) -> PortabilityResult<RestoreOutcome> {
    let semantic = &verified.manifest.semantic;
    if semantic.profile_id != encode_hex(recovered.profile_id.as_bytes().as_slice()) {
        return Err(PortabilityError::mismatch(
            "restored profile identity",
            &semantic.profile_id,
            encode_hex(recovered.profile_id.as_bytes().as_slice()),
        ));
    }

    let database_source = directory::resolve_relative(&verified.root, &semantic.database.path)?;
    let database_path = staging.join(academic_store::STORE_DATABASE_FILE);
    let (digest, byte_length) = directory::copy_new_file(&database_source, &database_path)?;
    if encode_hex(digest.as_bytes().as_slice()) != semantic.database.sha256
        || byte_length != semantic.database.byte_length
    {
        return Err(PortabilityError::mismatch(
            "restored database digest",
            &semantic.database.sha256,
            encode_hex(digest.as_bytes().as_slice()),
        ));
    }
    require_unreadable_without_key(&database_path)?;

    fault::trip(PortabilityFaultPoint::Rs02);

    let mut domains: Vec<academic_domain::DomainId> = semantic
        .objects
        .iter()
        .map(|object| crate::verify::parse_domain_id(&object.domain_id))
        .collect::<PortabilityResult<Vec<_>>>()?;
    domains.sort_unstable();
    domains.dedup();
    let keys = ProfileKeys::derive(&recovered.master, recovered.profile_id, &domains)?;

    let database = CanonicalDatabase::open_copy(&database_path, keys.store_key())?;
    database.cipher_integrity_check()?;
    database.integrity_check()?;
    database.foreign_key_check()?;
    let rows = read_canonical_rows(&database)?;
    rows.schema.policy.require_encrypted_v2()?;
    if rows.schema != semantic.store_schema {
        return Err(PortabilityError::mismatch(
            "restored store schema",
            semantic.store_schema.schema_semver.clone(),
            rows.schema.schema_semver.clone(),
        ));
    }
    if rows.watermark != semantic.watermark {
        return Err(PortabilityError::WatermarkMoved {
            expected: semantic.watermark.accept_seq_head,
            actual: rows.watermark.accept_seq_head,
        });
    }
    if rows.counts != semantic.counts {
        return Err(PortabilityError::mismatch(
            "restored canonical counts",
            semantic.counts.events,
            rows.counts.events,
        ));
    }
    if rows.device_heads != semantic.device_heads {
        return Err(PortabilityError::mismatch(
            "restored device heads",
            semantic.device_heads.len(),
            rows.device_heads.len(),
        ));
    }
    let canonical_semantic_digest = encode_hex(rows.semantic_digest()?.as_bytes().as_slice());
    if canonical_semantic_digest != semantic.canonical_semantic_digest {
        return Err(PortabilityError::mismatch(
            "restored canonical semantic digest",
            &semantic.canonical_semantic_digest,
            canonical_semantic_digest,
        ));
    }

    let replay = crate::verify::replay_signed_batches(&database, plan.authorizations)?;
    if replay.verified_batches != rows.counts.batches
        || replay.verified_events != rows.counts.events
        || replay.device_heads != rows.counts.device_heads
    {
        return Err(PortabilityError::replay(
            "replay coverage",
            format!(
                "{} batches and {} events verified for {} stored batches and {} stored events",
                replay.verified_batches,
                replay.verified_events,
                rows.counts.batches,
                rows.counts.events
            ),
        ));
    }

    let descriptors = read_artifact_descriptors(&database)?;
    drop(database);
    remove_journal_sidecars(&database_path)?;

    let vault = EncryptedVault::open(staging, keys.keyring(&recovered.master)?)?;
    if descriptors.len() != semantic.objects.len() {
        return Err(PortabilityError::mismatch(
            "restored object closure",
            semantic.objects.len(),
            descriptors.len(),
        ));
    }
    for descriptor in &descriptors {
        let identifier = descriptor.id.to_string();
        let declared = semantic
            .objects
            .iter()
            .find(|object| object.artifact_id == identifier)
            .ok_or_else(|| PortabilityError::MissingObject {
                artifact_id: identifier.clone(),
            })?;
        let source = directory::resolve_relative(&verified.root, &declared.path)?;
        // The canonical vault path is re-derived from the signed descriptor and
        // this profile's own locator key, so a manifest can never place an
        // object where the store would not look for it.
        let target = vault.layout().object_path(descriptor)?;
        if let Some(parent) = target.parent() {
            directory::create_directories(parent)?;
        }
        let (digest, byte_length) = directory::copy_new_file(&source, &target)?;
        if encode_hex(digest.as_bytes().as_slice()) != declared.ciphertext_sha256
            || byte_length != declared.byte_length
        {
            return Err(PortabilityError::mismatch(
                "restored object ciphertext digest",
                &declared.ciphertext_sha256,
                encode_hex(digest.as_bytes().as_slice()),
            ));
        }
        if encode_hex(descriptor.content_digest.as_bytes().as_slice()) != declared.plaintext_sha256
            || descriptor.byte_length != declared.plaintext_byte_length
        {
            return Err(PortabilityError::mismatch(
                "restored object plaintext digest",
                &declared.plaintext_sha256,
                encode_hex(descriptor.content_digest.as_bytes().as_slice()),
            ));
        }
    }

    fault::trip(PortabilityFaultPoint::Rs03);

    // Authentication happens after every object is in place, so a restore that
    // is interrupted midway leaves no object that has been declared good. A
    // crypto-shredded object authenticates nothing because its key slot was
    // destroyed on purpose; its bytes were still digest-checked above, which is
    // the whole of what a backup can promise about it.
    for descriptor in &descriptors {
        match vault.verify_sealed_object(descriptor) {
            Ok(_) => {}
            Err(source) if crate::encrypted::backup::may_be_shredded(&source) => {
                vault.verify_shredded_object(descriptor)?;
            }
            Err(source) => return Err(source.into()),
        }
    }

    Ok((
        replay,
        canonical_semantic_digest,
        crate::verify::count_of(descriptors.len()),
    ))
}

fn remove_journal_sidecars(database_path: &Path) -> PortabilityResult<()> {
    let Some(name) = database_path.file_name().and_then(|value| value.to_str()) else {
        return Err(PortabilityError::UnsafeEntry(database_path.to_path_buf()));
    };
    let Some(parent) = database_path.parent() else {
        return Err(PortabilityError::UnsafeEntry(database_path.to_path_buf()));
    };
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = parent.join(format!("{name}{suffix}"));
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(PortabilityError::io(
                    "remove restore journal sidecar",
                    path,
                    source,
                ));
            }
        }
    }
    Ok(())
}

fn remove_marker(staging: &Path, marker: &str) -> PortabilityResult<()> {
    let path = staging.join(marker);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PortabilityError::io("remove restore marker", path, source)),
    }
}

fn restore_marker_contents(destination: &Path) -> String {
    format!(
        "ACADEMIC_PLATFORM_PHASE2_ENCRYPTED_RESTORE_INCOMPLETE\ndestination={}\n",
        destination.display()
    )
}

/// Lists unpublished restore staging directories left beside a destination.
pub fn find_unpublished_restores(destination: &Path) -> PortabilityResult<Vec<PathBuf>> {
    directory::find_staging_directories(destination)
}

/// Removes one unpublished restore staging directory.
pub fn remove_unpublished_restore(destination: &Path, staging: &Path) -> PortabilityResult<()> {
    directory::remove_staging_directory(destination, staging)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_restore_marker_records_its_intended_destination() {
        let contents = restore_marker_contents(Path::new("profile"));
        assert!(contents.starts_with("ACADEMIC_PLATFORM_PHASE2_ENCRYPTED_RESTORE_INCOMPLETE\n"));
        assert!(contents.contains("destination=profile"));
        assert!(contents.ends_with('\n'));
    }
}
