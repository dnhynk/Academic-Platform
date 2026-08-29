//! Backup tombstones: how a deletion reaches copies it cannot edit.
//!
//! A backup holds `AEAD_CHUNKED_V2` objects byte for byte. Crypto-shredding the
//! live object destroys the live key slot and nothing else, so the copy inside
//! an already-taken backup is still readable by whoever can open that backup.
//! A deletion that stopped there would be a deletion that did not happen.
//!
//! A tombstone is the record that closes that gap. It is written into the
//! backup directory beside the objects, and a restore applies every tombstone
//! it finds to the objects it materialises. The re-deletion needs **no key**:
//! the locator lives in the clear at a fixed header offset, and destroying a
//! key slot is a positioned write. So a restore that has not been unlocked yet
//! — the normal case, on a fresh machine — still re-deletes.
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
pub const TOMBSTONE_VERSION: u8 = 1;

/// Domain separator for the tombstone digest.
pub const TOMBSTONE_DIGEST_DOMAIN: &[u8] = b"academic-os/backup-tombstone/v1";

/// One recorded fact: this object's key slot was destroyed in the live profile.
///
/// It carries a locator, an action identity, and a time. It carries no artifact
/// identity, no media type, and no content digest, because a tombstone sits in
/// the clear inside a backup whose whole point is that it discloses nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupTombstone {
    /// Frozen version.
    pub tombstone_version: u8,
    /// 32 lowercase hex identity of the retention action that shredded it.
    pub action_id: String,
    /// 64 lowercase hex locator of the shredded object.
    pub locator: String,
    /// When the live shred was recorded, in milliseconds since the epoch.
    ///
    /// Supplied by the caller: nothing in this crate reads a clock, so a
    /// deletion replays to the same bytes.
    pub shredded_at_ms: u64,
}

impl BackupTombstone {
    /// Builds a tombstone for one locator.
    #[must_use]
    pub fn new(action_id: String, locator: [u8; 32], shredded_at_ms: u64) -> Self {
        Self {
            tombstone_version: TOMBSTONE_VERSION,
            action_id,
            locator: hex::encode(locator),
            shredded_at_ms,
        }
    }

    /// Returns the digest the destroyed key slot names.
    ///
    /// A shredded object therefore points at the record that explains it, and
    /// a tombstone that was altered no longer matches the slot it authorized.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(TOMBSTONE_DIGEST_DOMAIN);
        hasher.update([self.tombstone_version]);
        hasher.update([0]);
        hasher.update(self.action_id.as_bytes());
        hasher.update([0]);
        hasher.update(self.locator.as_bytes());
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
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(&self.locator, &mut bytes)
            .map_err(|_| TombstoneError::Malformed(self.locator.clone()))?;
        Ok(bytes)
    }
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
