//! Encrypted backup at a fixed commit watermark.
//!
//! The order is the Phase 1 order, because the `BK` failpoints name positions
//! in it: fix the watermark, snapshot the database, close over every reachable
//! object, write the inventory, synchronize, publish with one rename. What
//! changed is what the bytes are — a SQLCipher snapshot and `AEAD_CHUNKED_V2`
//! objects — and that the manifest is sealed under a key the device wrapper
//! cannot produce.

use std::path::{Path, PathBuf};

use academic_crypto::{StoreKey, VaultMasterKey};
use academic_recovery::{BackupMasterKey, BackupRecipientSet, BackupSetId, RecoveryProfile};
use academic_vault::{EncryptedVault, SealedObjectVerifier as _};
use rusqlite::{
    Connection,
    backup::{Backup, StepResult},
};

use crate::{
    PortabilityError, PortabilityResult,
    checksum::{encode_hex, hash_file},
    directory,
    encrypted::{
        DATABASE_DIRECTORY, FORMAT_MARKER_CONTENTS, FORMAT_MARKER_FILE, MANIFEST_FILE, ProfileKeys,
        RECIPIENTS_FILE,
        manifest::{
            BACKUP_FORMAT, BACKUP_GENERATOR, BACKUP_MANIFEST_VERSION, EncryptedBackupManifest,
            EncryptedBackupSemantic, EncryptedObjectEntry,
        },
        object_relative_path, require_unreadable_without_key,
    },
    fault::{self, PortabilityFaultPoint},
    manifest::FileEntry,
    verify::{
        CanonicalDatabase, CanonicalRows, PolicyBlock, read_artifact_descriptors,
        read_canonical_rows,
    },
};

/// Pages copied per Online Backup step, matching the Phase 1 lane so `BK01`
/// still lands midway through a copy rather than before or after it.
const BACKUP_PAGES_PER_STEP: i32 = 8;

/// Everything one backup needs that is not in the profile.
#[derive(Debug)]
pub struct BackupPlan<'a> {
    /// The recovery profile in force. `DEVICE_ONLY` is refused here.
    pub recovery_profile: RecoveryProfile,
    /// The backup root, wrapped only by recovery-class recipients.
    pub backup_root: &'a BackupMasterKey,
    /// The wrapped copies of that root, written into the backup.
    pub backup_recipients: &'a BackupRecipientSet,
    /// The profile's own recovery-class recipient records, canonical CBOR.
    ///
    /// A restore on a fresh machine recovers the Vault Master Key from these,
    /// so a backup without them is not restorable and is refused.
    pub profile_recovery_recipients: &'a [u8],
}

/// Receipt returned by a completed backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedBackupReceipt {
    pub destination: PathBuf,
    pub manifest: EncryptedBackupManifest,
    pub backup_set_id: BackupSetId,
}

/// Copies one encrypted profile into a new published backup directory.
pub fn backup_encrypted_profile(
    profile_root: &Path,
    destination: &Path,
    master: &VaultMasterKey,
    keys: &ProfileKeys,
    plan: &BackupPlan<'_>,
) -> PortabilityResult<EncryptedBackupReceipt> {
    if !plan.recovery_profile.supports_independent_backup() {
        return Err(PortabilityError::RecoveryProfile(
            academic_recovery::RecoveryProfileError::NoIndependentBackupRecipient {
                profile: plan.recovery_profile.as_str(),
                statement: plan.recovery_profile.loss_statement(),
            },
        ));
    }
    if plan.profile_recovery_recipients.is_empty() {
        return Err(PortabilityError::ManifestRejected {
            field: "profile_recovery_recipients",
        });
    }
    directory::require_absent(destination)?;

    let database_path = profile_root.join(academic_store::STORE_DATABASE_FILE);
    let source = CanonicalDatabase::open_source(&database_path, keys.store_key())?;
    let source_rows = read_canonical_rows(&source)?;
    source_rows.schema.policy.require_encrypted_v2()?;
    let source_digest = source_rows.semantic_digest()?;
    let vault = EncryptedVault::open(profile_root, keys.keyring(master)?)?;

    let staging = directory::reserve_staging_path(destination, "backup-staging")?;
    directory::create_new_directory(&staging)?;

    let copied_database = staging
        .join(DATABASE_DIRECTORY)
        .join(academic_store::STORE_DATABASE_FILE);
    directory::create_directories(&staging.join(DATABASE_DIRECTORY))?;
    copy_encrypted_database(source.connection(), &copied_database, keys.store_key())?;
    drop(source);
    require_unreadable_without_key(&copied_database)?;

    let snapshot = CanonicalDatabase::open_copy(&copied_database, keys.store_key())?;
    snapshot.cipher_integrity_check()?;
    snapshot.integrity_check()?;
    snapshot.foreign_key_check()?;
    let snapshot_rows = read_canonical_rows(&snapshot)?;
    require_same_snapshot(&source_rows, &snapshot_rows)?;
    let snapshot_digest = snapshot_rows.semantic_digest()?;
    if snapshot_digest != source_digest {
        return Err(PortabilityError::mismatch(
            "encrypted backup canonical semantic digest",
            source_digest,
            snapshot_digest,
        ));
    }
    let descriptors = read_artifact_descriptors(&snapshot)?;
    drop(snapshot);
    remove_sidecar_journals(&copied_database)?;

    fault::trip(PortabilityFaultPoint::Bk02);

    let mut objects = Vec::with_capacity(descriptors.len());
    for (index, descriptor) in descriptors.iter().enumerate() {
        if index == 1 {
            fault::trip(PortabilityFaultPoint::Bk03);
        }
        // Reading the object back through the vault authenticates its header
        // and every chunk, so a backup never copies an object that would fail
        // to open on restore.
        let sealed = vault.verify_sealed_object(descriptor)?;
        let relative = object_relative_path(&descriptor.id.to_string())?;
        let path = directory::resolve_relative(&staging, &relative)?;
        if let Some(parent) = path.parent() {
            directory::create_directories(parent)?;
        }
        let (ciphertext_digest, byte_length) =
            directory::copy_new_file(sealed.object_path(), &path)?;
        objects.push(EncryptedObjectEntry {
            artifact_id: descriptor.id.to_string(),
            domain_id: descriptor.domain_id.to_string(),
            retention_class: retention_name(descriptor.retention_class).to_owned(),
            permission_lineage_id: descriptor.permission_lineage_id.to_string(),
            vault_locator: encode_hex(descriptor.vault_locator.as_bytes().as_slice()),
            format_version: u32::from(descriptor.format_version),
            path: relative,
            byte_length,
            ciphertext_sha256: encode_hex(ciphertext_digest.as_bytes().as_slice()),
            plaintext_sha256: encode_hex(descriptor.content_digest.as_bytes().as_slice()),
            plaintext_byte_length: descriptor.byte_length,
        });
    }

    directory::create_directories(&staging.join("keys"))?;
    directory::write_new_file(
        &staging.join(RECIPIENTS_FILE),
        &plan.backup_recipients.to_canonical_cbor()?,
    )?;
    directory::write_new_file(
        &staging.join(FORMAT_MARKER_FILE),
        FORMAT_MARKER_CONTENTS.as_bytes(),
    )?;

    let mut files = Vec::new();
    for relative in directory::list_files(&staging)? {
        directory::check_relative_path(&relative)?;
        let path = directory::resolve_relative(&staging, &relative)?;
        let (digest, byte_length) = hash_file(&path)
            .map_err(|source| PortabilityError::io("hash backup file", &path, source))?;
        files.push(FileEntry {
            path: relative,
            byte_length,
            sha256: encode_hex(digest.as_bytes().as_slice()),
        });
    }
    files.sort();
    let database_relative = crate::encrypted::database_relative_path();
    let database = files
        .iter()
        .find(|entry| entry.path == database_relative)
        .cloned()
        .ok_or_else(|| {
            PortabilityError::mismatch("backup database entry", database_relative, "absent")
        })?;

    let set_id = plan.backup_recipients.set_id();
    let semantic = EncryptedBackupSemantic {
        format: BACKUP_FORMAT.to_owned(),
        manifest_version: BACKUP_MANIFEST_VERSION,
        generator: BACKUP_GENERATOR.to_owned(),
        policy: PolicyBlock::encrypted_v2(),
        encrypted: true,
        object_format: academic_vault::ENCRYPTED_OBJECT_FORMAT.to_owned(),
        recovery_profile: plan.recovery_profile.as_str().to_owned(),
        backup_set_id: encode_hex(set_id.as_bytes().as_slice()),
        profile_id: encode_hex(keys.profile_id().as_bytes().as_slice()),
        profile_recovery_recipients: encode_hex(plan.profile_recovery_recipients),
        store_schema: snapshot_rows.schema.clone(),
        watermark: snapshot_rows.watermark,
        counts: snapshot_rows.counts,
        device_heads: snapshot_rows.device_heads.clone(),
        canonical_semantic_digest: encode_hex(snapshot_digest.as_bytes().as_slice()),
        database,
        objects,
        files,
    };
    let manifest = EncryptedBackupManifest::new(semantic, directory::now_unix_millis()?)?;
    manifest.require_contract()?;
    directory::write_new_file(
        &staging.join(MANIFEST_FILE),
        &manifest.seal(plan.backup_root, set_id)?,
    )?;
    directory::sync_tree(&staging)?;

    fault::trip(PortabilityFaultPoint::Bk04);

    directory::publish(&staging, destination)?;
    Ok(EncryptedBackupReceipt {
        destination: destination.to_path_buf(),
        manifest,
        backup_set_id: set_id,
    })
}

/// One backup directory verified without a repository, network, or database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedEncryptedBackup {
    pub root: PathBuf,
    pub manifest: EncryptedBackupManifest,
    pub recipients: BackupRecipientSet,
}

/// Re-reads and fully verifies a published backup directory.
///
/// Opening the manifest needs the backup root, so this is the point at which a
/// caller without a recovery secret stops: a backup is inert without one, and
/// the failure says so rather than reporting a corrupt directory.
pub fn verify_encrypted_backup_directory(
    root: &Path,
    backup_root: &BackupMasterKey,
) -> PortabilityResult<VerifiedEncryptedBackup> {
    let marker_path = root.join(FORMAT_MARKER_FILE);
    let marker = std::fs::read(&marker_path).map_err(|source| {
        PortabilityError::io("read backup format marker", &marker_path, source)
    })?;
    if marker != FORMAT_MARKER_CONTENTS.as_bytes() {
        return Err(PortabilityError::ManifestRejected {
            field: "BACKUP_FORMAT_V2",
        });
    }

    let recipients_path = root.join(RECIPIENTS_FILE);
    let recipients =
        BackupRecipientSet::from_canonical_cbor(&std::fs::read(&recipients_path).map_err(
            |source| PortabilityError::io("read backup recipients", &recipients_path, source),
        )?)?;

    let manifest_path = root.join(MANIFEST_FILE);
    let sealed = std::fs::read(&manifest_path)
        .map_err(|source| PortabilityError::io("read backup manifest", &manifest_path, source))?;
    EncryptedBackupManifest::verify_signature(&sealed)?;
    let manifest = EncryptedBackupManifest::open(&sealed, backup_root)?;
    if manifest.semantic.backup_set_id != encode_hex(recipients.set_id().as_bytes().as_slice()) {
        return Err(PortabilityError::ManifestRejected {
            field: "backup_set_id",
        });
    }

    let mut expected: Vec<&str> = manifest
        .semantic
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    expected.push(MANIFEST_FILE);
    expected.sort_unstable();
    let observed = directory::list_files(root)?;
    if observed != expected {
        return Err(PortabilityError::mismatch(
            "encrypted backup directory inventory",
            expected.join(", "),
            observed.join(", "),
        ));
    }

    for entry in &manifest.semantic.files {
        let path = directory::resolve_relative(root, &entry.path)?;
        let (digest, byte_length) = hash_file(&path)
            .map_err(|source| PortabilityError::io("hash backup file", &path, source))?;
        if encode_hex(digest.as_bytes().as_slice()) != entry.sha256
            || byte_length != entry.byte_length
        {
            return Err(PortabilityError::mismatch(
                "encrypted backup file digest",
                &entry.sha256,
                encode_hex(digest.as_bytes().as_slice()),
            ));
        }
    }

    for object in &manifest.semantic.objects {
        let entry = manifest
            .semantic
            .files
            .iter()
            .find(|file| file.path == object.path)
            .ok_or_else(|| PortabilityError::MissingObject {
                artifact_id: object.artifact_id.clone(),
            })?;
        if entry.sha256 != object.ciphertext_sha256 || entry.byte_length != object.byte_length {
            return Err(PortabilityError::mismatch(
                "encrypted backup object binding",
                &object.ciphertext_sha256,
                &entry.sha256,
            ));
        }
    }

    Ok(VerifiedEncryptedBackup {
        root: root.to_path_buf(),
        manifest,
        recipients,
    })
}

/// Reads the wrapped backup key set without opening anything else.
pub fn read_backup_recipients(root: &Path) -> PortabilityResult<BackupRecipientSet> {
    let path = root.join(RECIPIENTS_FILE);
    let bytes = std::fs::read(&path)
        .map_err(|source| PortabilityError::io("read backup recipients", &path, source))?;
    Ok(BackupRecipientSet::from_canonical_cbor(&bytes)?)
}

/// Lists unpublished backup staging directories left beside a destination.
pub fn find_unpublished_backups(destination: &Path) -> PortabilityResult<Vec<PathBuf>> {
    directory::find_staging_directories(destination)
}

/// Removes one unpublished backup staging directory.
pub fn remove_unpublished_backup(destination: &Path, staging: &Path) -> PortabilityResult<()> {
    directory::remove_staging_directory(destination, staging)
}

/// Copies an encrypted database into a keyed destination.
///
/// The destination is keyed **before** the first page is written. A SQLite
/// Online Backup into an unkeyed handle writes plaintext pages, so the key
/// application here is not a convenience: it is the difference between a backup
/// and a disclosure.
fn copy_encrypted_database(
    source: &Connection,
    destination_path: &Path,
    key: &StoreKey,
) -> PortabilityResult<()> {
    directory::require_absent(destination_path)?;
    let mut destination = Connection::open(destination_path)?;
    academic_store::cipher::apply_store_key(&destination, key, destination_path)?;
    {
        let backup = Backup::new(source, &mut destination)?;
        let mut stepped = false;
        loop {
            match backup.step(BACKUP_PAGES_PER_STEP)? {
                StepResult::Done => break,
                StepResult::More => {
                    if !stepped {
                        stepped = true;
                        fault::trip(PortabilityFaultPoint::Bk01);
                    }
                }
                StepResult::Busy | StepResult::Locked => {
                    return Err(PortabilityError::DatabaseCheckFailed {
                        check: "online backup",
                        detail: "the source database was locked during the snapshot".to_owned(),
                    });
                }
                _ => {
                    return Err(PortabilityError::DatabaseCheckFailed {
                        check: "online backup",
                        detail: "the SQLite backup API returned an unknown step result".to_owned(),
                    });
                }
            }
        }
    }
    destination.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    destination
        .close()
        .map_err(|(_, error)| PortabilityError::from(error))?;
    Ok(())
}

fn remove_sidecar_journals(database_path: &Path) -> PortabilityResult<()> {
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
                    "remove backup journal sidecar",
                    path,
                    source,
                ));
            }
        }
    }
    Ok(())
}

fn require_same_snapshot(
    source: &CanonicalRows,
    snapshot: &CanonicalRows,
) -> PortabilityResult<()> {
    if source.watermark != snapshot.watermark {
        return Err(PortabilityError::WatermarkMoved {
            expected: source.watermark.accept_seq_head,
            actual: snapshot.watermark.accept_seq_head,
        });
    }
    if source.counts != snapshot.counts {
        return Err(PortabilityError::mismatch(
            "encrypted backup canonical counts",
            source.counts.events,
            snapshot.counts.events,
        ));
    }
    if source.device_heads != snapshot.device_heads {
        return Err(PortabilityError::mismatch(
            "encrypted backup device heads",
            source.device_heads.len(),
            snapshot.device_heads.len(),
        ));
    }
    Ok(())
}

const fn retention_name(value: academic_domain::RetentionClass) -> &'static str {
    match value {
        academic_domain::RetentionClass::Ephemeral => "EPHEMERAL",
        academic_domain::RetentionClass::CourseTerm => "COURSE_TERM",
        academic_domain::RetentionClass::UserManaged => "USER_MANAGED",
        academic_domain::RetentionClass::LegalHold => "LEGAL_HOLD",
    }
}
