//! The append-only progress journal shared by rotation and retention.
//!
//! t068 section 3.2 gives the profile one `keys/rotation-journal.jsonl`, and
//! `P2-K5` adds `retention/deletion-journal.jsonl` beside it for the deletion
//! half. Both are the same physical contract, so they are one type.
//!
//! # Record shape
//!
//! One JSON object per line, UTF-8, newline-terminated, no trailing spaces:
//!
//! ```json
//! {"journal_version":1,"sequence":0,"previous_digest":"00..00","entry":{"kind":"..."},"entry_digest":"<64 hex>"}
//! ```
//!
//! `entry_digest` is `SHA-256` over the exact bytes
//! `"academic-os/journal/v1" | LE64(sequence) | previous_digest | entry_json`,
//! where `entry_json` is the serialized `entry` value of that same line. Record
//! zero's `previous_digest` is 64 zero hex characters. Each later record's
//! `previous_digest` is the previous record's `entry_digest`.
//!
//! That chain is what makes "append-only" checkable rather than asserted: a
//! removed line breaks the sequence, a rewritten line breaks its own digest,
//! and a reordered pair breaks the link. [`AppendOnlyJournal::open`] verifies
//! the whole chain before it will append, so a tampered journal cannot be
//! extended.
//!
//! # What a journal must never carry
//!
//! Nothing here is encrypted, so nothing here may be private. Entries carry
//! locators, which are already the on-disk filenames, digests of them, key
//! *generation names* (which are one-way commitments, not keys), recipient
//! identifiers, and reason codes. They carry no media type, no content digest,
//! no plaintext length, no artifact identity, and no key byte.

use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::entry::JournalEntry;

/// Frozen journal record version.
pub const JOURNAL_VERSION: u8 = 1;

/// Domain separator mixed into every record digest.
pub const JOURNAL_DIGEST_DOMAIN: &[u8] = b"academic-os/journal/v1";

/// Relative path of the rotation journal inside a profile.
pub const ROTATION_JOURNAL_RELATIVE_PATH: &str = "keys/rotation-journal.jsonl";

/// Relative path of the deletion journal inside a profile.
pub const DELETION_JOURNAL_RELATIVE_PATH: &str = "retention/deletion-journal.jsonl";

/// Largest journal this build will read, in bytes.
///
/// A journal is one line per rotation unit and one line per retention action,
/// so this bound is far above any real profile and exists only so a corrupt or
/// hostile file cannot be read into unbounded memory.
pub const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;

/// Why a journal could not be read, verified, or extended.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum JournalError {
    /// The journal file could not be read or written.
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
    /// A line is not a well-formed journal record.
    #[error("journal record {sequence} of {path} is not a well-formed record")]
    Malformed {
        /// Zero-based line number.
        sequence: u64,
        /// Path involved.
        path: PathBuf,
    },
    /// A record declares a version this build does not implement.
    #[error("journal record {sequence} declares unsupported version {version}")]
    UnsupportedVersion {
        /// Zero-based line number.
        sequence: u64,
        /// Declared version.
        version: u8,
    },
    /// The chain does not hold, so the file was not only appended to.
    ///
    /// This is the append-only violation. It is one variant on purpose: a
    /// removed line, a rewritten line, and a reordered pair are the same fact.
    #[error(
        "journal {path} is not append-only: record {sequence} does not continue \
         the chain, so a line was rewritten, removed, or reordered"
    )]
    ChainBroken {
        /// Zero-based index of the first record that does not continue the chain.
        sequence: u64,
        /// Path involved.
        path: PathBuf,
    },
    /// The file is larger than this build will read.
    #[error("journal {path} is larger than the {MAX_JOURNAL_BYTES} byte read bound")]
    TooLarge {
        /// Path involved.
        path: PathBuf,
    },
    /// A record could not be encoded.
    #[error("a journal record could not be encoded")]
    Encode,
}

fn io(operation: &'static str, path: &Path, source: std::io::Error) -> JournalError {
    JournalError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

/// One line of a journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalRecord {
    /// Frozen record version.
    pub journal_version: u8,
    /// Zero-based position; strictly increasing by one.
    pub sequence: u64,
    /// Previous record's `entry_digest`, or 64 zero hex characters at record zero.
    pub previous_digest: String,
    /// The recorded fact.
    pub entry: JournalEntry,
    /// `SHA-256` over the domain separator, sequence, previous digest, and entry.
    pub entry_digest: String,
}

/// The all-zero digest that record zero links to.
#[must_use]
pub fn genesis_digest() -> String {
    "0".repeat(64)
}

fn entry_json(entry: &JournalEntry) -> Result<String, JournalError> {
    serde_json::to_string(entry).map_err(|_| JournalError::Encode)
}

/// Computes the digest a record at `sequence` must carry.
fn record_digest(
    sequence: u64,
    previous_digest: &str,
    entry: &JournalEntry,
) -> Result<String, JournalError> {
    let mut hasher = Sha256::new();
    hasher.update(JOURNAL_DIGEST_DOMAIN);
    hasher.update(sequence.to_le_bytes());
    hasher.update(previous_digest.as_bytes());
    hasher.update(entry_json(entry)?.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// An opened journal whose whole chain verified.
///
/// The file handle is kept open in append mode for the life of this value, so
/// there is no code path in this type that can position a write anywhere but
/// the end of the file.
#[derive(Debug)]
pub struct AppendOnlyJournal {
    path: PathBuf,
    file: File,
    records: Vec<JournalRecord>,
}

impl AppendOnlyJournal {
    /// Opens, creating an empty journal if none exists, and verifies the chain.
    pub fn open(path: &Path) -> Result<Self, JournalError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| io("create journal directory", parent, source))?;
        }
        let records = read_verified(path)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| io("open journal for append", path, source))?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
            records,
        })
    }

    /// Returns the verified records in file order.
    #[must_use]
    pub fn records(&self) -> &[JournalRecord] {
        &self.records
    }

    /// Returns the journal path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the recorded entries in file order.
    pub fn entries(&self) -> impl Iterator<Item = &JournalEntry> {
        self.records.iter().map(|record| &record.entry)
    }

    /// Appends one entry and makes it durable before returning.
    ///
    /// The line, its parent directory, and the file's metadata are all synced,
    /// so a caller that sees this return knows the record survives a kill.
    pub fn append(&mut self, entry: JournalEntry) -> Result<&JournalRecord, JournalError> {
        let sequence = u64::try_from(self.records.len()).map_err(|_| JournalError::Encode)?;
        let previous_digest = self
            .records
            .last()
            .map_or_else(genesis_digest, |record| record.entry_digest.clone());
        let entry_digest = record_digest(sequence, &previous_digest, &entry)?;
        let record = JournalRecord {
            journal_version: JOURNAL_VERSION,
            sequence,
            previous_digest,
            entry,
            entry_digest,
        };
        let mut line = serde_json::to_string(&record).map_err(|_| JournalError::Encode)?;
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .map_err(|source| io("append journal record", &self.path, source))?;
        self.file
            .sync_all()
            .map_err(|source| io("synchronize journal", &self.path, source))?;
        if let Some(parent) = self.path.parent() {
            sync_directory(parent)?;
        }
        self.records.push(record);
        // `push` above guarantees a last element.
        self.records.last().ok_or(JournalError::Encode)
    }
}

/// Reads and verifies a journal without opening it for append.
pub fn read_verified(path: &Path) -> Result<Vec<JournalRecord>, JournalError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io("open journal", path, source)),
    };
    let length = file
        .metadata()
        .map_err(|source| io("inspect journal", path, source))?
        .len();
    if length > MAX_JOURNAL_BYTES {
        return Err(JournalError::TooLarge {
            path: path.to_path_buf(),
        });
    }

    let mut records = Vec::new();
    let mut previous_digest = genesis_digest();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let sequence = u64::try_from(index).map_err(|_| JournalError::Encode)?;
        let line = line.map_err(|source| io("read journal record", path, source))?;
        let record: JournalRecord =
            serde_json::from_str(&line).map_err(|_| JournalError::Malformed {
                sequence,
                path: path.to_path_buf(),
            })?;
        if record.journal_version != JOURNAL_VERSION {
            return Err(JournalError::UnsupportedVersion {
                sequence,
                version: record.journal_version,
            });
        }
        let expected = record_digest(sequence, &previous_digest, &record.entry)?;
        if record.sequence != sequence
            || record.previous_digest != previous_digest
            || record.entry_digest != expected
        {
            return Err(JournalError::ChainBroken {
                sequence,
                path: path.to_path_buf(),
            });
        }
        previous_digest.clone_from(&record.entry_digest);
        records.push(record);
    }
    Ok(records)
}

/// Flushes a directory entry where the host allows it.
///
/// The content of every record is made durable by `sync_all` on the handle
/// that wrote it, which is the guarantee the rotation invariant rests on. This
/// is the weaker, additional barrier for the *directory entry* of a
/// newly-created file.
///
/// It is best effort on purpose. Windows refuses `FlushFileBuffers` on a
/// directory handle on several supported filesystems, and even opening one
/// needs `FILE_FLAG_BACKUP_SEMANTICS`, which is FFI this crate must not
/// contain: `unsafe_code = "forbid"` applies here and the reviewed
/// directory-barrier boundary is `academic-vault`'s platform leaf. So a
/// refusal to open or flush a directory is not treated as a failure, exactly
/// as `academic-recovery` writes its receipt without one. On Linux and macOS
/// the flush happens and is real.
pub(crate) fn sync_directory(path: &Path) -> Result<(), JournalError> {
    let directory = match File::open(path) {
        Ok(directory) => directory,
        Err(source)
            if matches!(
                source.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidInput
            ) =>
        {
            return Ok(());
        }
        Err(source) => return Err(io("open directory", path, source)),
    };
    match directory.sync_all() {
        Ok(()) => Ok(()),
        Err(source)
            if matches!(
                source.kind(),
                std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::InvalidInput
            ) =>
        {
            Ok(())
        }
        Err(source) => Err(io("synchronize directory", path, source)),
    }
}
