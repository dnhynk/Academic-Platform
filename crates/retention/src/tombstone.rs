//! Backup tombstones: how a deletion reaches copies it cannot edit.
//!
//! A backup holds `AEAD_CHUNKED_V2` objects byte for byte. Crypto-shredding the
//! live object destroys the live key slot and nothing else, so the copy inside
//! an already-taken backup is still readable by whoever can open that backup.
//! A deletion that stopped there would be a deletion that did not happen.
//!
//! A tombstone is the record that closes that gap. It is written into the
//! backup directory beside the objects, and `restore_encrypted_profile` applies
//! every tombstone it finds to the objects it materialises — in the staging
//! tree, after each object has been authenticated and before the rename that
//! publishes the restore, so no published restore holds a key slot the profile
//! it came from had destroyed.
//!
//! The re-deletion needs **no key**: the artifact id and the locator both live
//! in the clear at fixed header offsets, and destroying a key slot is a
//! positioned write. That is what makes it work on a fresh machine, and it is
//! why [`apply_tombstones`] takes a directory rather than a vault.
//!
//! A locator alone does not name an artifact. It derives from the domain KEK
//! over the media type and the content digest, with no lineage and no retention
//! class in it, so inside one domain the same bytes registered in two
//! permission lineages get **one locator and two paths**. A record that named
//! only a locator would reach whichever of them the directory walk saw first —
//! destroying a key slot the profile never deleted, or leaving the deleted one
//! readable. So a tombstone names its artifact, and a re-deletion matches the
//! artifact id as well as the locator.
//!
//! `tombstones/` is the one path in a published backup the sealed manifest does
//! not list, because the manifest was sealed before this record existed and
//! re-sealing needs the backup root. The backup's verifier excludes it from the
//! inventory comparison and still requires every listed file.
//!
//! ```text
//! <backup>/
//!   objects/<artifact-id>.aobj
//!   tombstones/<locator>.tombstone      # one JSON object, one atomic write
//! ```

use std::{
    fs::{self, File},
    io::Write as _,
    path::{Path, PathBuf},
};

use academic_domain::ArtifactId;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{
    fault::{self, FaultPoint},
    journal::{JournalError, sync_directory},
};

/// Relative directory holding tombstones inside a backup.
pub const TOMBSTONE_DIRECTORY: &str = "tombstones";

/// File extension of one tombstone.
pub const TOMBSTONE_EXTENSION: &str = "tombstone";

/// Frozen tombstone version.
///
/// Version 1 named a locator and no artifact. A locator is shared by every
/// artifact in one domain that holds the same bytes, so such a record cannot be
/// applied to the artifact it was written for; [`read_from_backup`] refuses one
/// by version rather than applying it to whatever the walk reaches first.
pub const TOMBSTONE_VERSION: u8 = 2;

/// Domain separator for the tombstone digest.
pub const TOMBSTONE_DIGEST_DOMAIN: &[u8] = b"academic-os/backup-tombstone/v1";

/// One recorded fact: this object's key slot was destroyed in the live profile.
///
/// It carries the artifact it was written for, the locator that artifact had
/// when it was shredded, every locator its reference chain moved through before
/// that, an action identity, and a time. It carries no media type and no
/// content digest, because a tombstone sits in the clear inside a backup whose
/// whole point is that it discloses nothing — and both the locator and the
/// artifact id are already cleartext in the object headers it sits beside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupTombstone {
    /// Frozen version.
    pub tombstone_version: u8,
    /// 32 lowercase hex identity of the retention action that shredded it.
    pub action_id: String,
    /// 32 lowercase hex identity of the artifact that was shredded.
    ///
    /// This is what makes the record name an artifact rather than a set of
    /// bytes. It is at a fixed cleartext offset of every object header, so a
    /// re-deletion reads it with no key, and it is what
    /// [`apply_tombstones`](crate::engine::apply_tombstones) matches on
    /// alongside the locator.
    pub artifact_id: String,
    /// 64 lowercase hex locator of the shredded object.
    pub locator: String,
    /// 64 lowercase hex locators the same artifact was reachable under before.
    ///
    /// A locator derives from the domain KEK, so a rotation moves an artifact
    /// to a new one. A backup taken before a rotation holds the object under an
    /// older name, and a tombstone naming only the current one would not reach
    /// it. These are the artifact's earlier names, oldest first, read from the
    /// store's `artifact_descriptor_migration` chain when the deletion is
    /// planned.
    ///
    /// Absent from a tombstone written for an artifact that never moved, which
    /// is why it is skipped when empty: such a record is byte-for-byte the one
    /// this build wrote before the field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded_locators: Vec<String>,
    /// When the live shred was recorded, in milliseconds since the epoch.
    ///
    /// Supplied by the caller: nothing in this crate reads a clock, so a
    /// deletion replays to the same bytes.
    pub shredded_at_ms: u64,
}

impl BackupTombstone {
    /// Builds a tombstone for one artifact at one locator.
    #[must_use]
    pub fn new(
        action_id: String,
        artifact: ArtifactId,
        locator: [u8; 32],
        shredded_at_ms: u64,
    ) -> Self {
        Self::covering(action_id, artifact, locator, &[], shredded_at_ms)
    }

    /// Builds a tombstone that also names the locators the artifact moved from.
    ///
    /// `superseded` is the artifact's reference chain in chain order, which is
    /// what the store's `artifact_descriptor_migration` rows already hold. A
    /// caller that passes an empty slice gets exactly [`BackupTombstone::new`].
    #[must_use]
    pub fn covering(
        action_id: String,
        artifact: ArtifactId,
        locator: [u8; 32],
        superseded: &[[u8; 32]],
        shredded_at_ms: u64,
    ) -> Self {
        Self {
            tombstone_version: TOMBSTONE_VERSION,
            action_id,
            artifact_id: hex::encode(artifact.as_bytes()),
            locator: hex::encode(locator),
            superseded_locators: superseded.iter().map(hex::encode).collect(),
            shredded_at_ms,
        }
    }

    /// Returns the digest the destroyed key slot names.
    ///
    /// A shredded object therefore points at the record that explains it, and
    /// a tombstone that was altered no longer matches the slot it authorized.
    /// The artifact and the superseded names are inside it, so a record cannot
    /// be re-pointed or widened to reach an object it was not written to reach.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(TOMBSTONE_DIGEST_DOMAIN);
        hasher.update([self.tombstone_version]);
        hasher.update([0]);
        hasher.update(self.action_id.as_bytes());
        hasher.update([0]);
        hasher.update(self.artifact_id.as_bytes());
        hasher.update([0]);
        hasher.update(self.locator.as_bytes());
        hasher.update([0]);
        for superseded in &self.superseded_locators {
            hasher.update(superseded.as_bytes());
            hasher.update([0]);
        }
        hasher.update([0]);
        hasher.update(self.shredded_at_ms.to_le_bytes());
        hasher.finalize().into()
    }

    /// Returns the digest's hex spelling.
    #[must_use]
    pub fn digest_hex(&self) -> String {
        hex::encode(self.digest())
    }

    /// Returns the locator as bytes.
    pub fn locator_bytes(&self) -> Result<[u8; 32], TombstoneError> {
        decode_locator(&self.locator)
    }

    /// Returns the artifact identity as bytes.
    pub fn artifact_id_bytes(&self) -> Result<[u8; 16], TombstoneError> {
        let mut bytes = [0_u8; 16];
        hex::decode_to_slice(&self.artifact_id, &mut bytes)
            .map_err(|_| TombstoneError::Malformed(self.artifact_id.clone()))?;
        Ok(bytes)
    }

    /// Returns every locator this tombstone reaches, current one first.
    pub fn covered_locators(&self) -> Result<Vec<[u8; 32]>, TombstoneError> {
        let mut locators = Vec::with_capacity(1 + self.superseded_locators.len());
        locators.push(self.locator_bytes()?);
        for superseded in &self.superseded_locators {
            locators.push(decode_locator(superseded)?);
        }
        Ok(locators)
    }
}

fn decode_locator(value: &str) -> Result<[u8; 32], TombstoneError> {
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes)
        .map_err(|_| TombstoneError::Malformed(value.to_owned()))?;
    Ok(bytes)
}

/// Why a tombstone could not be written, read, or applied.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TombstoneError {
    /// The tombstone file could not be read or written.
    #[error("{operation} failed for {path}: {source}")]
    Io {
        /// What was being attempted.
        operation: &'static str,
        /// Path involved.
        path: PathBuf,
        /// Underlying failure.
        #[source]
        source: std::io::Error,
    },
    /// A stored tombstone is not a well-formed record.
    #[error("tombstone {0} is not a well-formed record")]
    Malformed(String),
    /// A stored tombstone declares an unsupported version.
    #[error("tombstone {path} declares unsupported version {version}")]
    UnsupportedVersion {
        /// Path involved.
        path: PathBuf,
        /// Declared version.
        version: u8,
    },
    /// The record could not be encoded.
    #[error("a tombstone could not be encoded")]
    Encode,
    /// The tombstone directory could not be made durable.
    #[error("the tombstone directory could not be synchronized: {0}")]
    Directory(#[from] JournalError),
}

fn io(operation: &'static str, path: &Path, source: std::io::Error) -> TombstoneError {
    TombstoneError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

/// Returns the tombstone directory inside a backup.
#[must_use]
pub fn tombstone_dir(backup_root: &Path) -> PathBuf {
    backup_root.join(TOMBSTONE_DIRECTORY)
}

/// Writes one tombstone into a backup with a single atomic write.
///
/// `RB02` requires a failed tombstone write to leave the deletion explicitly
/// incomplete rather than quietly complete, so the caller treats an error here
/// as `REPAIR_REQUIRED`. Re-writing an identical tombstone is idempotent.
pub fn write_into_backup(
    backup_root: &Path,
    tombstone: &BackupTombstone,
) -> Result<PathBuf, TombstoneError> {
    let directory = tombstone_dir(backup_root);
    fs::create_dir_all(&directory)
        .map_err(|source| io("create tombstone directory", &directory, source))?;
    let path = directory.join(format!("{}.{TOMBSTONE_EXTENSION}", tombstone.locator));
    let temp = directory.join(format!("{}.partial", tombstone.locator));

    let mut bytes = serde_json::to_vec(tombstone).map_err(|_| TombstoneError::Encode)?;
    bytes.push(b'\n');

    fault::trip(FaultPoint::Rb02BeforeTombstone);

    let mut file =
        File::create(&temp).map_err(|source| io("create tombstone temp", &temp, source))?;
    file.write_all(&bytes)
        .map_err(|source| io("write tombstone temp", &temp, source))?;
    file.sync_all()
        .map_err(|source| io("synchronize tombstone temp", &temp, source))?;
    drop(file);
    fs::rename(&temp, &path).map_err(|source| io("publish tombstone", &path, source))?;
    sync_directory(&directory)?;
    Ok(path)
}

/// Reads every tombstone a backup carries, sorted by locator.
pub fn read_from_backup(backup_root: &Path) -> Result<Vec<BackupTombstone>, TombstoneError> {
    let directory = tombstone_dir(backup_root);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io("enumerate tombstones", &directory, source)),
    };
    let mut tombstones = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| io("read tombstone entry", &directory, source))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some(TOMBSTONE_EXTENSION) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|source| io("read tombstone", &path, source))?;
        let tombstone: BackupTombstone = serde_json::from_slice(&bytes)
            .map_err(|_| TombstoneError::Malformed(path.display().to_string()))?;
        if tombstone.tombstone_version != TOMBSTONE_VERSION {
            return Err(TombstoneError::UnsupportedVersion {
                path,
                version: tombstone.tombstone_version,
            });
        }
        tombstones.push(tombstone);
    }
    tombstones.sort_by(|left, right| left.locator.cmp(&right.locator));
    Ok(tombstones)
}
