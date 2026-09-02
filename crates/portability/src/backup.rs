//! Plaintext synthetic backup at a fixed commit watermark.
//!
//! The database is copied with the SQLite Online Backup API into a temporary
//! directory, every reachable sealed object is copied beside it, a versioned
//! manifest records hashes, counts, device heads, and the watermark, everything
//! is synchronized, and the directory is published with one rename.
//!
//! This backup is **plaintext**. It protects nothing, it is synthetic-only, and
//! it is not evidence for ADR-002, ADR-005, or ADR-012. The SQLite file itself
//! need not be byte-identical across runs; its integrity and its canonical
//! semantic digest must be.

use std::{
    fs,
    path::{Path, PathBuf},
};

use academic_vault::{DomainKeyring, Vault};
use rusqlite::{
    Connection,
    backup::{Backup, StepResult},
};

use crate::{
    PHASE1_BACKUP_FORMAT, PHASE1_BACKUP_MANIFEST_VERSION, PHASE1_BACKUP_PLAINTEXT_WARNING,
    PHASE1_PORTABILITY_GENERATOR, PortabilityError, PortabilityResult,
    checksum::{encode_hex, hash_file},
    directory,
    fault::{self, PortabilityFaultPoint},
    manifest::{BackupManifest, BackupSemantic, FileEntry, ObjectEntry},
    verify::{
        CanonicalDatabase, CanonicalRows, PolicyBlock, read_artifact_descriptors,
        read_canonical_rows,
    },
};

/// Exact JSON Schema shipped inside every backup directory.
pub const BACKUP_MANIFEST_SCHEMA: &str =
    include_str!("../../../schemas/jsonschema/phase1-backup-v1.schema.json");

/// Relative path of the manifest inside a backup directory.
pub const MANIFEST_FILE: &str = "manifest.json";
/// Relative path of the embedded manifest schema.
pub const MANIFEST_SCHEMA_FILE: &str = "schemas/phase1-backup-v1.schema.json";
/// Relative directory holding the copied canonical database.
pub const DATABASE_DIRECTORY: &str = "store";
/// Relative directory holding one exact plaintext object per registered artifact.
///
/// The backup deliberately does not mirror the vault's deep policy-namespaced
/// fan-out. Restore re-derives the canonical vault path from the signed
/// descriptor, which is both stronger and far shorter than trusting a path
/// recorded in a manifest.
pub const OBJECTS_DIRECTORY: &str = "objects";

/// Pages copied per Online Backup step.
///
/// Stepping keeps the copy interruptible at a named fault point. SQLite restarts
/// the copy transparently if the source is written between steps, and the
/// watermark comparison after the copy rejects any snapshot that moved.
const BACKUP_PAGES_PER_STEP: i32 = 8;

/// Receipt returned by a completed backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReceipt {
    pub destination: PathBuf,
    pub manifest: BackupManifest,
}

/// Copies one synthetic profile into a new published backup directory.
pub fn backup_profile(
    profile_root: &Path,
    destination: &Path,
    keyring: DomainKeyring,
) -> PortabilityResult<BackupReceipt> {
    directory::require_absent(destination)?;
    // Before anything is read and before the posture is computed: this
    // opens the database file directly instead of going through
    // `open_synthetic_profile`, so it is outside the marker rule unless it
    // runs the rule itself.
    academic_store::profile::require_profile_format(profile_root)?;
    let database_path = profile_root.join(academic_store::STORE_DATABASE_FILE);
    let source = CanonicalDatabase::open_source(&database_path)?;
    let source_rows = read_canonical_rows(&source)?;
    source_rows.schema.policy.require_phase1()?;
    let source_digest = source_rows.semantic_digest()?;
    let vault = Vault::open(profile_root, keyring)?;

    let staging = directory::reserve_staging_path(destination, "backup-staging")?;
    directory::create_new_directory(&staging)?;

    let copied_database = staging
        .join(DATABASE_DIRECTORY)
        .join(academic_store::STORE_DATABASE_FILE);
    directory::create_directories(&staging.join(DATABASE_DIRECTORY))?;
    copy_database(source.connection(), &copied_database)?;
    drop(source);

    let snapshot = CanonicalDatabase::open_copy(&copied_database)?;
    snapshot.integrity_check()?;
    snapshot.foreign_key_check()?;
    let snapshot_rows = read_canonical_rows(&snapshot)?;
    require_same_snapshot(&source_rows, &snapshot_rows)?;
    let snapshot_digest = snapshot_rows.semantic_digest()?;
    if snapshot_digest != source_digest {
        return Err(PortabilityError::mismatch(
            "backup canonical semantic digest",
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
        let sealed = vault.verify_sealed_object(descriptor)?;
        let relative = format!("{OBJECTS_DIRECTORY}/{}.bin", descriptor.id);
        directory::check_relative_path(&relative)?;
        let path = directory::resolve_relative(&staging, &relative)?;
        if let Some(parent) = path.parent() {
            directory::create_directories(parent)?;
        }
        let (digest, byte_length) = directory::copy_new_file(sealed.object_path(), &path)?;
        if digest != descriptor.content_digest || byte_length != descriptor.byte_length {
            return Err(PortabilityError::mismatch(
                "backed-up artifact object",
                descriptor.content_digest,
                digest,
            ));
        }
        objects.push(ObjectEntry {
            artifact_id: descriptor.id.to_string(),
            domain_id: descriptor.domain_id.to_string(),
            retention_class: retention_name(descriptor.retention_class).to_owned(),
            permission_lineage_id: descriptor.permission_lineage_id.to_string(),
            vault_locator: encode_hex(descriptor.vault_locator.as_bytes().as_slice()),
            path: relative,
            byte_length,
            plaintext_sha256: encode_hex(digest.as_bytes().as_slice()),
        });
    }

    directory::create_directories(&staging.join("schemas"))?;
    directory::write_new_file(
        &staging.join(MANIFEST_SCHEMA_FILE),
        BACKUP_MANIFEST_SCHEMA.as_bytes(),
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
    let database_relative = format!(
        "{DATABASE_DIRECTORY}/{}",
        academic_store::STORE_DATABASE_FILE
    );
    let database = files
        .iter()
        .find(|entry| entry.path == database_relative)
        .cloned()
        .ok_or_else(|| {
            PortabilityError::mismatch("backup database entry", database_relative, "absent")
        })?;

    let semantic = BackupSemantic {
        format: PHASE1_BACKUP_FORMAT.to_owned(),
        manifest_version: PHASE1_BACKUP_MANIFEST_VERSION,
        generator: PHASE1_PORTABILITY_GENERATOR.to_owned(),
        policy: PolicyBlock::phase1(),
        encrypted: false,
        plaintext_warning: PHASE1_BACKUP_PLAINTEXT_WARNING.to_owned(),
        store_schema: snapshot_rows.schema.clone(),
        watermark: snapshot_rows.watermark,
        counts: snapshot_rows.counts,
        device_heads: snapshot_rows.device_heads.clone(),
        canonical_semantic_digest: encode_hex(snapshot_digest.as_bytes().as_slice()),
        database,
        objects,
        files,
    };
    let manifest = BackupManifest::seal(semantic, directory::now_unix_millis()?)?;
    manifest.require_phase1_contract()?;
    directory::write_new_file(&staging.join(MANIFEST_FILE), &manifest.to_json_bytes()?)?;
    directory::sync_tree(&staging)?;

    fault::trip(PortabilityFaultPoint::Bk04);

    directory::publish(&staging, destination)?;
    Ok(BackupReceipt {
        destination: destination.to_path_buf(),
        manifest,
    })
}

fn copy_database(source: &Connection, destination_path: &Path) -> PortabilityResult<()> {
    directory::require_absent(destination_path)?;
    let mut destination = Connection::open(destination_path)?;
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

/// Removes the write-ahead journal sidecars left beside a cleanly closed copy.
///
/// A cleanly closed SQLite connection already checkpoints and deletes them; this
/// keeps the backup inventory exact when a host leaves a zero-length residue.
fn remove_sidecar_journals(database_path: &Path) -> PortabilityResult<()> {
    let Some(name) = database_path.file_name().and_then(|value| value.to_str()) else {
        return Err(PortabilityError::UnsafeEntry(database_path.to_path_buf()));
    };
    let Some(parent) = database_path.parent() else {
        return Err(PortabilityError::UnsafeEntry(database_path.to_path_buf()));
    };
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = parent.join(format!("{name}{suffix}"));
        match fs::remove_file(&path) {
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
            "backup canonical counts",
            source.counts.events,
            snapshot.counts.events,
        ));
    }
    if source.device_heads != snapshot.device_heads {
        return Err(PortabilityError::mismatch(
            "backup device heads",
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

/// One backup directory verified without a repository, network, or database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBackup {
    pub root: PathBuf,
    pub manifest: BackupManifest,
}

/// Re-reads and fully verifies a published backup directory.
///
/// Every listed file must exist with the exact recorded digest and length, no
/// unlisted file may exist beside the manifest, and the semantic digest must
/// recompute. A backup whose manifest is absent is rejected rather than
/// partially trusted.
pub fn verify_backup_directory(root: &Path) -> PortabilityResult<VerifiedBackup> {
    let manifest_path = root.join(MANIFEST_FILE);
    let bytes = fs::read(&manifest_path)
        .map_err(|source| PortabilityError::io("read backup manifest", &manifest_path, source))?;
    let manifest = BackupManifest::from_json_bytes(&bytes)?;
    manifest.require_phase1_contract()?;
    manifest.verify_semantic_digest()?;

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
            "backup directory inventory",
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
                "backup file digest",
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
        if entry.sha256 != object.plaintext_sha256 || entry.byte_length != object.byte_length {
            return Err(PortabilityError::mismatch(
                "backup object binding",
                &object.plaintext_sha256,
                &entry.sha256,
            ));
        }
    }

    Ok(VerifiedBackup {
        root: root.to_path_buf(),
        manifest,
    })
}

/// Lists unpublished backup staging directories left beside a destination.
pub fn find_unpublished_backups(destination: &Path) -> PortabilityResult<Vec<PathBuf>> {
    directory::find_staging_directories(destination)
}

/// Removes one unpublished backup staging directory.
pub fn remove_unpublished_backup(destination: &Path, staging: &Path) -> PortabilityResult<()> {
    directory::remove_staging_directory(destination, staging)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_schema_matches_the_frozen_backup_contract() -> PortabilityResult<()> {
        let schema: serde_json::Value =
            serde_json::from_str(BACKUP_MANIFEST_SCHEMA).map_err(|source| {
                PortabilityError::Json {
                    operation: "parse embedded backup schema",
                    source,
                }
            })?;
        let semantic = &schema["properties"]["semantic"]["properties"];
        assert_eq!(semantic["format"]["const"], PHASE1_BACKUP_FORMAT);
        assert_eq!(
            semantic["manifest_version"]["const"],
            PHASE1_BACKUP_MANIFEST_VERSION
        );
        assert_eq!(semantic["encrypted"]["const"], false);
        assert_eq!(
            semantic["plaintext_warning"]["const"],
            PHASE1_BACKUP_PLAINTEXT_WARNING
        );
        Ok(())
    }

    #[test]
    fn backup_step_size_allows_an_interruptible_copy() {
        const {
            assert!(BACKUP_PAGES_PER_STEP > 0);
        }
    }
}
