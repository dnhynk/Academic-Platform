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
//! That chain is what makes "append-only" checkable rather than asserted for
//! every edit *inside* the file: a rewritten line breaks its own digest, a
//! reordered pair breaks the link, and a line removed from the middle breaks
//! the sequence of everything after it. [`AppendOnlyJournal::open`] verifies
//! the whole chain before it will append.
//!
//! # The head file, and what a backward chain cannot see
//!
//! A backward chain cannot see its own tail being cut off: drop the last `k`
//! records and the remaining prefix still verifies. So the record count and
//! the head digest are also written beside the journal, in
//! `<journal>.head`, after every append:
//!
//! ```json
//! {"journal_version":1,"record_count":3,"head_digest":"<64 hex>"}
//! ```
//!
//! A journal holding fewer records than its head declares, or holding a
//! different digest at the position the head names, is [`JournalError::HeadMismatch`]
//! and neither reads nor extends. The asymmetry is deliberate: the head is
//! written after the record it names is already durable, so a kill between the
//! two leaves the journal *ahead* of the head, which is a crash and is
//! repaired on the next open. A journal *behind* its head is a removal.
//!
//! This is a consistency anchor, not a MAC. Nothing in a profile's cleartext
//! journal is keyed, so an adversary who can write both files can rewrite both
//! consistently and is not detected here. What the head closes is the one case
//! the chain alone could not see at all.
//!
//! # A torn final line is a crash, not tampering
//!
//! [`AppendOnlyJournal::append`] writes one whole line and syncs it before it
//! returns, so a record that ends without its newline was never durable and
//! never reported. Such a trailing fragment is dropped and the file is
//! truncated back to the last complete record when the journal is opened for
//! append, which is what lets an interrupted rotation resume. A *complete*
//! line that does not parse is still [`JournalError::Malformed`]: the two are
//! different facts.
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
    /// The journal does not hold the head its own head file names.
    ///
    /// This is the tail-removal violation the backward chain cannot see. A
    /// journal that holds *more* records than the head names is not this: that
    /// is a kill between an append and the head write, and it is repaired.
    #[error(
        "journal {path} holds {observed} records ending at {observed_digest}, but its head \
         file names {declared} records ending at {declared_digest}, so the tail was removed"
    )]
    HeadMismatch {
        /// Path of the journal, not of the head file.
        path: PathBuf,
        /// Records the head file declares.
        declared: u64,
        /// Digest the head file declares for record `declared - 1`.
        declared_digest: String,
        /// Records the journal holds.
        observed: u64,
        /// Digest the journal holds at the position the head names, or `absent`.
        observed_digest: String,
    },
    /// The head file exists but is not a well-formed head record.
    #[error("journal head file {path} is not a well-formed head record")]
    MalformedHead {
        /// Path of the head file.
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

/// Filename suffix of the head file written beside a journal.
pub const JOURNAL_HEAD_SUFFIX: &str = ".head";

/// The head file's single record.
///
/// It names how many records the journal held when it was last appended to and
/// the digest of the last of them. See the module documentation for exactly
/// what that does and does not detect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalHead {
    /// Frozen record version, shared with [`JournalRecord`].
    pub journal_version: u8,
    /// How many records the journal held.
    pub record_count: u64,
    /// `entry_digest` of record `record_count - 1`.
    pub head_digest: String,
}

/// Returns the head file that belongs to one journal path.
#[must_use]
pub fn head_path(journal: &Path) -> PathBuf {
    let mut name = journal.as_os_str().to_os_string();
    name.push(JOURNAL_HEAD_SUFFIX);
    PathBuf::from(name)
}

fn read_head(journal: &Path) -> Result<Option<JournalHead>, JournalError> {
    let path = head_path(journal);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io("read journal head", &path, source)),
    };
    let head: JournalHead =
        serde_json::from_slice(&bytes).map_err(|_| JournalError::MalformedHead { path })?;
    if head.journal_version != JOURNAL_VERSION {
        return Err(JournalError::UnsupportedVersion {
            sequence: head.record_count,
            version: head.journal_version,
        });
    }
    Ok(Some(head))
}

/// Writes the head file for a journal that now holds `records`.
///
/// The write is a temp file and a rename, so a kill leaves either the previous
/// head or the new one and never a half-written line.
fn write_head(journal: &Path, records: &[JournalRecord]) -> Result<(), JournalError> {
    let Some(last) = records.last() else {
        return Ok(());
    };
    let head = JournalHead {
        journal_version: JOURNAL_VERSION,
        record_count: u64::try_from(records.len()).map_err(|_| JournalError::Encode)?,
        head_digest: last.entry_digest.clone(),
    };
    let path = head_path(journal);
    let temp = {
        let mut name = path.as_os_str().to_os_string();
        name.push(".partial");
        PathBuf::from(name)
    };
    let mut bytes = serde_json::to_vec(&head).map_err(|_| JournalError::Encode)?;
    bytes.push(b'\n');
    let mut file =
        File::create(&temp).map_err(|source| io("create journal head", &temp, source))?;
    file.write_all(&bytes)
        .map_err(|source| io("write journal head", &temp, source))?;
    file.sync_all()
        .map_err(|source| io("synchronize journal head", &temp, source))?;
    drop(file);
    fs::rename(&temp, &path).map_err(|source| io("publish journal head", &path, source))?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

/// Refuses a journal that holds fewer records than its head declares.
fn require_head_covered(
    path: &Path,
    records: &[JournalRecord],
    head: Option<&JournalHead>,
) -> Result<(), JournalError> {
    let Some(head) = head else {
        return Ok(());
    };
    let observed = u64::try_from(records.len()).map_err(|_| JournalError::Encode)?;
    let named = usize::try_from(head.record_count.saturating_sub(1)).unwrap_or(usize::MAX);
    let observed_digest = records
        .get(named)
        .map(|record| record.entry_digest.clone())
        .unwrap_or_else(|| "absent".to_owned());
    if observed < head.record_count || observed_digest != head.head_digest {
        return Err(JournalError::HeadMismatch {
            path: path.to_path_buf(),
            declared: head.record_count,
            declared_digest: head.head_digest.clone(),
            observed,
            observed_digest,
        });
    }
    Ok(())
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
        let (records, complete_bytes) = read_complete_records(path)?;
        require_head_covered(path, &records, read_head(path)?.as_ref())?;
        // A torn final line is dropped before the append handle exists.
        // Truncating it is what makes a rotation resumable: the next append
        // starts at a record boundary rather than extending a fragment. The
        // handle is a separate, short-lived write handle, because an
        // append-mode handle cannot shorten a file on Windows and the append
        // handle this journal keeps must never be able to.
        if let Ok(existing) = OpenOptions::new().write(true).open(path) {
            let length = existing
                .metadata()
                .map_err(|source| io("inspect journal", path, source))?
                .len();
            if length != complete_bytes {
                existing
                    .set_len(complete_bytes)
                    .map_err(|source| io("truncate torn journal record", path, source))?;
                existing
                    .sync_all()
                    .map_err(|source| io("synchronize journal", path, source))?;
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| io("open journal for append", path, source))?;
        // A kill between an append and its head write leaves the head behind.
        // Re-writing it here from the verified records is the repair.
        write_head(path, &records)?;
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
        // The head names a record that is already durable, so it is written
        // after the line it names and never before it.
        write_head(&self.path, &self.records)?;
        // `push` above guarantees a last element.
        self.records.last().ok_or(JournalError::Encode)
    }
}

/// Reads and verifies a journal without opening it for append.
///
/// A trailing fragment with no newline is dropped before verification, and the
/// head file is checked afterwards, so this refuses a journal whose tail was
/// removed as well as one whose interior was edited.
pub fn read_verified(path: &Path) -> Result<Vec<JournalRecord>, JournalError> {
    let (records, _complete_bytes) = read_complete_records(path)?;
    require_head_covered(path, &records, read_head(path)?.as_ref())?;
    Ok(records)
}

/// Verifies every complete line and reports how many bytes they occupy.
///
/// The byte count is what an open-for-append truncates to, so a torn final
/// line cannot become the prefix of the next record.
fn read_complete_records(path: &Path) -> Result<(Vec<JournalRecord>, u64), JournalError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), 0));
        }
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
    let mut complete_bytes: u64 = 0;
    let mut index: usize = 0;
    let mut reader = BufReader::new(file);
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|source| io("read journal record", path, source))?;
        if read == 0 {
            break;
        }
        if !line.ends_with('\n') {
            // The writer syncs one whole line before it returns, so a fragment
            // without its newline was never a durable record. It is dropped
            // rather than refused; `AppendOnlyJournal::open` truncates it away.
            break;
        }
        let sequence = u64::try_from(index).map_err(|_| JournalError::Encode)?;
        let record: JournalRecord =
            serde_json::from_str(line.trim_end_matches('\n')).map_err(|_| {
                JournalError::Malformed {
                    sequence,
                    path: path.to_path_buf(),
                }
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
        complete_bytes = complete_bytes
            .saturating_add(u64::try_from(line.len()).map_err(|_| JournalError::Encode)?);
        index = index.saturating_add(1);
    }
    Ok((records, complete_bytes))
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
