//! The durable import session: the `IN04` shape.
//!
//! Section 7's `IN04` row is "kill mid import" with the required outcome "no
//! partial attempt set; lease released; resumable". Three mechanisms produce
//! that, and each is one sentence:
//!
//! - **No partial set.** Both durable files arrive by rename over a fully
//!   written, fsynced temporary. A reader sees a complete file or no file; it
//!   never sees a truncated one.
//! - **Lease.** The session directory holds one `session.lock`, created with
//!   `create_new`, so two *live* sessions cannot both hold it. It is not an
//!   operating-system advisory lock: a killed holder leaves the file behind,
//!   and [`ImportSession::resume`] is what releases it. That is the whole
//!   claim, and it is deliberately weaker than "a crashed process releases its
//!   lease", which would not be true.
//! - **Resumable.** [`inspect`] reports which of the three durable states a
//!   session is in, and `resume` re-enters an unpublished one.

use std::{
    fs::{self, File},
    io::Write as _,
    path::{Path, PathBuf},
};

use academic_domain::TranscriptVersionId;

use crate::{
    TranscriptError,
    admission::AdmittedImport,
    fault::{self, FaultPoint},
    reconcile::ReconciledTranscript,
    record::TranscriptField,
};

/// Directory, relative to the profile root, holding every import session.
pub const SESSIONS_RELATIVE_PATH: &str = "transcript/sessions";
/// The lease file inside one session directory.
pub const LEASE_FILE_NAME: &str = "session.lock";
/// The staged, not-yet-published row set.
pub const STAGING_FILE_NAME: &str = "staging.part";
/// The published confirmed row set.
pub const CONFIRMED_FILE_NAME: &str = "confirmed.set";
/// Label the confirmed set opens with.
pub const CONFIRMED_SET_LABEL: &[u8] = b"ACADEMIC-TRANSCRIPT-CONFIRMED-SET-V1";

/// Which durable state one import session is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// No session directory exists.
    Absent,
    /// A session exists and nothing is staged.
    Started {
        /// Whether a lease file is present.
        lease_held: bool,
    },
    /// A complete row set is staged and nothing is published.
    Staged {
        /// Whether a lease file is present.
        lease_held: bool,
    },
    /// A complete row set is published.
    Published {
        /// Whether a lease file is still present.
        lease_held: bool,
    },
}

impl SessionState {
    /// Whether a confirmed row set is durable.
    #[must_use]
    pub const fn is_published(self) -> bool {
        matches!(self, Self::Published { .. })
    }

    /// Whether a lease file is present.
    #[must_use]
    pub const fn lease_held(self) -> bool {
        match self {
            Self::Absent => false,
            Self::Started { lease_held }
            | Self::Staged { lease_held }
            | Self::Published { lease_held } => lease_held,
        }
    }
}

/// Returns the durable state of one import session without taking its lease.
pub fn inspect(
    profile_root: &Path,
    version_id: TranscriptVersionId,
) -> Result<SessionState, TranscriptError> {
    let directory = session_directory(profile_root, version_id);
    if !directory.is_dir() {
        return Ok(SessionState::Absent);
    }
    let lease_held = directory.join(LEASE_FILE_NAME).is_file();
    if directory.join(CONFIRMED_FILE_NAME).is_file() {
        return Ok(SessionState::Published { lease_held });
    }
    if directory.join(STAGING_FILE_NAME).is_file() {
        return Ok(SessionState::Staged { lease_held });
    }
    Ok(SessionState::Started { lease_held })
}

/// Returns the directory one import session owns.
#[must_use]
pub fn session_directory(profile_root: &Path, version_id: TranscriptVersionId) -> PathBuf {
    profile_root
        .join(SESSIONS_RELATIVE_PATH)
        .join(version_id.to_string())
}

/// One admitted, leased import session.
#[derive(Debug)]
pub struct ImportSession {
    directory: PathBuf,
    version_id: TranscriptVersionId,
}

impl ImportSession {
    /// Begins a new session, taking its lease.
    ///
    /// Requires an [`AdmittedImport`]. This is one of the two gated entry
    /// points in the crate; see [`crate::admission`] for where the gate is and
    /// where it deliberately is not.
    pub fn begin(
        _admitted: &AdmittedImport,
        profile_root: &Path,
        version_id: TranscriptVersionId,
    ) -> Result<Self, TranscriptError> {
        let directory = session_directory(profile_root, version_id);
        fs::create_dir_all(&directory).map_err(|source| TranscriptError::io(&directory, source))?;
        let lease = directory.join(LEASE_FILE_NAME);
        match File::options().write(true).create_new(true).open(&lease) {
            Ok(mut file) => {
                file.write_all(version_id.to_string().as_bytes())
                    .map_err(|source| TranscriptError::io(&lease, source))?;
                file.sync_all()
                    .map_err(|source| TranscriptError::io(&lease, source))?;
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(TranscriptError::SessionLeaseHeld { version_id });
            }
            Err(source) => return Err(TranscriptError::io(&lease, source)),
        }
        Ok(Self {
            directory,
            version_id,
        })
    }

    /// Re-enters an unpublished session, taking over its lease.
    ///
    /// This is what "the lease is released" means after a kill: the file the
    /// dead process left behind is removed here, by the process that takes the
    /// session over. A published session is refused, because re-entering one
    /// would be a second publication rather than a resumption.
    pub fn resume(
        _admitted: &AdmittedImport,
        profile_root: &Path,
        version_id: TranscriptVersionId,
    ) -> Result<Self, TranscriptError> {
        let directory = session_directory(profile_root, version_id);
        match inspect(profile_root, version_id)? {
            SessionState::Absent => return Err(TranscriptError::SessionAbsent { version_id }),
            SessionState::Published { .. } => {
                return Err(TranscriptError::SessionAlreadyPublished { version_id });
            }
            SessionState::Started { .. } | SessionState::Staged { .. } => {}
        }
        let lease = directory.join(LEASE_FILE_NAME);
        match fs::remove_file(&lease) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(TranscriptError::io(&lease, source)),
        }
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&lease)
            .map_err(|source| TranscriptError::io(&lease, source))?;
        file.write_all(version_id.to_string().as_bytes())
            .map_err(|source| TranscriptError::io(&lease, source))?;
        file.sync_all()
            .map_err(|source| TranscriptError::io(&lease, source))?;
        Ok(Self {
            directory,
            version_id,
        })
    }

    /// Returns the session's directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns the transcript version this session imports.
    #[must_use]
    pub const fn version_id(&self) -> TranscriptVersionId {
        self.version_id
    }

    /// Stages the complete reconciled row set.
    ///
    /// A reconciliation that halted produces no [`ReconciledTranscript`], so a
    /// halted import has nothing to pass here. That is `IN03`'s "nothing
    /// confirmed", carried by the type.
    pub fn stage(&self, reconciled: &ReconciledTranscript) -> Result<(), TranscriptError> {
        let bytes = encode_confirmed_set(self.version_id, reconciled);
        let temporary = self.directory.join("staging.tmp");
        write_durable(&temporary, &bytes)?;
        fault::trip(FaultPoint::StagingTemporaryWritten);
        let staging = self.directory.join(STAGING_FILE_NAME);
        fs::rename(&temporary, &staging).map_err(|source| TranscriptError::io(&staging, source))?;
        sync_directory(&self.directory)?;
        fault::trip(FaultPoint::SetStaged);
        Ok(())
    }

    /// Publishes the staged set and releases the lease.
    pub fn publish(self) -> Result<PathBuf, TranscriptError> {
        let staging = self.directory.join(STAGING_FILE_NAME);
        if !staging.is_file() {
            return Err(TranscriptError::NothingStaged {
                version_id: self.version_id,
            });
        }
        let confirmed = self.directory.join(CONFIRMED_FILE_NAME);
        fs::rename(&staging, &confirmed)
            .map_err(|source| TranscriptError::io(&confirmed, source))?;
        sync_directory(&self.directory)?;
        fault::trip(FaultPoint::SetPublished);
        self.release()?;
        Ok(confirmed)
    }

    /// Releases the lease without publishing.
    pub fn release(&self) -> Result<(), TranscriptError> {
        let lease = self.directory.join(LEASE_FILE_NAME);
        match fs::remove_file(&lease) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(TranscriptError::io(&lease, source)),
        }
        sync_directory(&self.directory)
    }
}

/// Encodes the confirmed row set exactly as it is written to disk.
///
/// Length-prefixed, so a truncated read cannot parse as a shorter valid set —
/// and identity-free, because a durable file beside the vault is one more place
/// the student number must not be.
///
/// The set names its own transcript version and the reconciliation's reference
/// digest. Both are inside the bytes rather than only in the directory path: a
/// file that only its location identified could be moved into another session's
/// directory and read there as that session's confirmed set.
#[must_use]
pub fn encode_confirmed_set(
    version_id: TranscriptVersionId,
    reconciled: &ReconciledTranscript,
) -> Vec<u8> {
    let rows = reconciled.transcript().rows();
    let mut out = Vec::new();
    out.extend_from_slice(CONFIRMED_SET_LABEL);
    out.extend_from_slice(version_id.as_bytes());
    out.extend_from_slice(reconciled.reference_identity_digest());
    out.extend_from_slice(&u32::try_from(rows.len()).unwrap_or(u32::MAX).to_le_bytes());
    for row in rows {
        out.extend_from_slice(&row.ordinal().to_le_bytes());
        for field in TranscriptField::ALL {
            let value = row.field(field);
            out.extend_from_slice(&u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
            out.extend_from_slice(value.as_bytes());
        }
    }
    out
}

fn write_durable(path: &Path, bytes: &[u8]) -> Result<(), TranscriptError> {
    let mut file = File::create(path).map_err(|source| TranscriptError::io(path, source))?;
    file.write_all(bytes)
        .map_err(|source| TranscriptError::io(path, source))?;
    file.sync_all()
        .map_err(|source| TranscriptError::io(path, source))
}

/// Makes a rename durable.
///
/// On Unix that is an fsync of the containing directory. Windows has no
/// directory handle to sync through `std`, and `MoveFileEx` without
/// `WRITE_THROUGH` is what the platform gives: the rename is atomic with
/// respect to a reader either way, which is the property `IN04` needs, and the
/// weaker durability against power loss is stated rather than papered over.
fn sync_directory(directory: &Path) -> Result<(), TranscriptError> {
    #[cfg(unix)]
    {
        File::open(directory)
            .and_then(|handle| handle.sync_all())
            .map_err(|source| TranscriptError::io(directory, source))
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Ok(())
    }
}
