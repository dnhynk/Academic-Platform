//! Deterministic synthetic export, plaintext backup, and empty-profile restore.
//!
//! The open export is a directory, not an archive: two exports taken at the same
//! canonical watermark produce identical per-file hashes and an identical
//! semantic manifest digest on Windows and Linux. Filesystem metadata is never
//! part of that contract.
//!
//! The Phase 1 backup is plaintext and synthetic-only. It proves watermark
//! fixing, reachable-object closure, and atomic publication; it is not
//! confidential, is not ADR-002 or ADR-012 acceptance evidence, and must never
//! be described as a secure backup.

#[cfg(feature = "plaintext-portability")]
pub mod backup;
pub mod checksum;
#[cfg(feature = "encrypted-portability")]
pub mod encrypted;
#[cfg(feature = "plaintext-portability")]
pub mod export;
pub mod fault;
pub mod manifest;
#[cfg(feature = "plaintext-portability")]
pub mod restore;
pub mod verify;

use std::{error::Error, fmt, io, path::PathBuf};

use academic_contracts::ContractError;
use academic_domain::DomainError;
#[cfg(feature = "plaintext-portability")]
use academic_projections::runner::ProjectionError;
use academic_store::{error::StoreError, queries::QueryError};
use academic_vault::VaultError;

/// Deterministic open-directory export contract name.
pub const PHASE1_EXPORT_FORMAT: &str = "learning-platform-phase1-export-v1";
/// Synthetic-only backup manifest contract name.
pub const PHASE1_BACKUP_FORMAT: &str = "learning-platform-phase1-backup-v1";
/// Restore is allowed only into a new empty profile.
pub const RESTORE_REQUIRES_EMPTY_PROFILE: bool = true;
/// Projections are disposable and excluded from the canonical export by default.
pub const EXPORT_INCLUDES_PROJECTIONS_BY_DEFAULT: bool = false;

/// Exact export manifest version.
pub const PHASE1_EXPORT_MANIFEST_VERSION: u32 = 1;
/// Exact backup manifest version.
pub const PHASE1_BACKUP_MANIFEST_VERSION: u32 = 1;
/// Stable identity of the writer that produced an export or backup directory.
pub const PHASE1_PORTABILITY_GENERATOR: &str = "learning-platform.phase1-portability.v1";
/// Unavoidable statement that the Phase 1 backup format protects nothing.
pub const PHASE1_BACKUP_PLAINTEXT_WARNING: &str =
    "PLAINTEXT SYNTHETIC-ONLY BACKUP — NOT ENCRYPTED, NOT CONFIDENTIAL, NOT PRODUCTION EVIDENCE";
/// Marker written into every unpublished restore staging directory.
pub const RESTORE_INCOMPLETE_MARKER: &str = ".academic-restore-incomplete";

/// Longest relative path an export or backup directory may contain.
///
/// This is the portable budget the *format* owns. Absolute length is the
/// caller's choice of destination root, and a produced relative path is the
/// only part this crate can bound. Keeping every relative path inside this
/// budget means a destination root that fits the classic Windows limit still
/// yields classic-safe paths, and the produced names avoid reserved device
/// names, trailing dots or spaces, and characters Windows refuses.
pub const MAX_PORTABLE_RELATIVE_PATH_BYTES: usize = 160;

/// Fail-closed error boundary for export, backup, and restore.
#[derive(Debug)]
#[non_exhaustive]
pub enum PortabilityError {
    /// A filesystem operation failed.
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    /// The guarded store boundary refused the database.
    Store(StoreError),
    /// A `P2-K5` backup tombstone could not be read or re-applied.
    ///
    /// The detail is `academic-retention`'s own message. A restore that cannot
    /// re-apply a deletion fails rather than publishing a profile in which the
    /// deletion silently did not happen.
    Tombstone(String),
    /// A canonical query failed.
    Query(QueryError),
    /// The synthetic vault refused an object.
    Vault(VaultError),
    /// Rebuilding disposable projections failed.
    #[cfg(feature = "plaintext-portability")]
    Projection(ProjectionError),
    /// A backup key, recipient record, or sealed manifest was refused.
    #[cfg(feature = "encrypted-portability")]
    BackupKey(academic_recovery::BackupKeyError),
    /// The sealed backup manifest could not be produced, verified, or opened.
    #[cfg(feature = "encrypted-portability")]
    SealedManifest(academic_recovery::SealedManifestError),
    /// The selected recovery profile refused the operation.
    #[cfg(feature = "encrypted-portability")]
    RecoveryProfile(academic_recovery::RecoveryProfileError),
    /// The key schedule refused to derive a key.
    #[cfg(feature = "encrypted-portability")]
    KeySchedule,
    /// No recovery recipient in the backup opened with the presented secret.
    ///
    /// The reason is the *last* recipient's, and every recipient reports the
    /// same refusal for a wrong secret, so nothing here can be read as an
    /// oracle about which recipient or which part of the secret was wrong.
    #[cfg(feature = "encrypted-portability")]
    RecoveryUnlockRefused { reason: String },
    /// A stored signed envelope failed independent verification.
    Contract(ContractError),
    /// A stored canonical row failed domain validation.
    Domain(DomainError),
    /// A direct SQLite statement failed.
    Sqlite(rusqlite::Error),
    /// Manifest serialization or parsing failed.
    Json {
        operation: &'static str,
        source: serde_json::Error,
    },
    /// A destination directory already exists.
    DestinationExists(PathBuf),
    /// A restore destination was not a new empty directory.
    DestinationNotEmpty(PathBuf),
    /// A produced path exceeded the portable path budget.
    PathTooLong { path: PathBuf, limit: usize },
    /// A directory entry had an unsafe or unsupported physical shape.
    UnsafeEntry(PathBuf),
    /// A manifest field did not match the frozen Phase 1 contract.
    ManifestRejected { field: &'static str },
    /// A stored or copied value did not match its expected exact form.
    IntegrityMismatch {
        subject: &'static str,
        expected: String,
        actual: String,
    },
    /// A referenced sealed object was absent from a backup or restore source.
    MissingObject { artifact_id: String },
    /// A restore input carried no independent authorization for a stored device.
    MissingAuthorization { device_id: String },
    /// The signed replay disagreed with a stored canonical row.
    ReplayMismatch {
        subject: &'static str,
        detail: String,
    },
    /// `integrity_check` or `foreign_key_check` reported a problem.
    DatabaseCheckFailed { check: &'static str, detail: String },
    /// The canonical watermark moved while a snapshot was being taken.
    WatermarkMoved { expected: u64, actual: u64 },
    /// A stored coordinate did not fit the portable exact-integer range.
    IntegerOutOfRange { subject: &'static str, value: i64 },
    /// The system clock could not be represented in stable milliseconds.
    ClockUnavailable,
}

impl PortabilityError {
    pub(crate) fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn mismatch(
        subject: &'static str,
        expected: impl fmt::Display,
        actual: impl fmt::Display,
    ) -> Self {
        Self::IntegrityMismatch {
            subject,
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    }

    pub(crate) fn replay(subject: &'static str, detail: impl fmt::Display) -> Self {
        Self::ReplayMismatch {
            subject,
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for PortabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} at {}: {source}", path.display()),
            Self::Store(source) => write!(formatter, "canonical store boundary: {source}"),
            Self::Tombstone(detail) => write!(
                formatter,
                "a backup tombstone could not be re-applied to the restored objects: {detail}"
            ),
            Self::Query(source) => write!(formatter, "canonical query: {source}"),
            Self::Vault(source) => write!(formatter, "synthetic vault: {source}"),
            #[cfg(feature = "plaintext-portability")]
            Self::Projection(source) => write!(formatter, "projection rebuild: {source}"),
            #[cfg(feature = "encrypted-portability")]
            Self::BackupKey(source) => write!(formatter, "backup key material: {source}"),
            #[cfg(feature = "encrypted-portability")]
            Self::SealedManifest(source) => write!(formatter, "sealed backup manifest: {source}"),
            #[cfg(feature = "encrypted-portability")]
            Self::RecoveryProfile(source) => write!(formatter, "recovery profile: {source}"),
            #[cfg(feature = "encrypted-portability")]
            Self::KeySchedule => formatter.write_str("the key schedule refused a derivation"),
            #[cfg(feature = "encrypted-portability")]
            Self::RecoveryUnlockRefused { reason } => write!(
                formatter,
                "no recovery recipient in this backup opened with the presented                  secret: {reason}"
            ),
            Self::Contract(source) => write!(formatter, "signed envelope replay: {source}"),
            Self::Domain(source) => write!(formatter, "canonical row validation: {source}"),
            Self::Sqlite(source) => write!(formatter, "SQLite portability statement: {source}"),
            Self::Json { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::DestinationExists(path) => write!(
                formatter,
                "destination {} already exists; portability never overwrites",
                path.display()
            ),
            Self::DestinationNotEmpty(path) => write!(
                formatter,
                "restore requires a new empty destination; {} is not empty",
                path.display()
            ),
            Self::PathTooLong { path, limit } => write!(
                formatter,
                "produced path {} exceeds the portable {limit}-byte budget",
                path.display()
            ),
            Self::UnsafeEntry(path) => write!(
                formatter,
                "unsafe or unsupported portability entry at {}",
                path.display()
            ),
            Self::ManifestRejected { field } => {
                write!(formatter, "manifest field {field} is not admitted")
            }
            Self::IntegrityMismatch {
                subject,
                expected,
                actual,
            } => write!(
                formatter,
                "{subject} mismatch: expected {expected}, observed {actual}"
            ),
            Self::MissingObject { artifact_id } => write!(
                formatter,
                "artifact {artifact_id} has no reachable sealed object"
            ),
            Self::MissingAuthorization { device_id } => write!(
                formatter,
                "no independent device authorization was supplied for device {device_id}"
            ),
            Self::ReplayMismatch { subject, detail } => {
                write!(formatter, "signed replay rejected {subject}: {detail}")
            }
            Self::DatabaseCheckFailed { check, detail } => {
                write!(formatter, "{check} failed: {detail}")
            }
            Self::WatermarkMoved { expected, actual } => write!(
                formatter,
                "canonical watermark moved during the snapshot: expected {expected}, observed \
                 {actual}"
            ),
            Self::IntegerOutOfRange { subject, value } => write!(
                formatter,
                "{subject} value {value} is outside the portable exact-integer range"
            ),
            Self::ClockUnavailable => {
                formatter.write_str("system clock is unavailable for portability bookkeeping")
            }
        }
    }
}

impl Error for PortabilityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Store(source) => Some(source),
            Self::Query(source) => Some(source),
            Self::Vault(source) => Some(source),
            #[cfg(feature = "plaintext-portability")]
            Self::Projection(source) => Some(source),
            #[cfg(feature = "encrypted-portability")]
            Self::BackupKey(source) => Some(source),
            #[cfg(feature = "encrypted-portability")]
            Self::SealedManifest(source) => Some(source),
            #[cfg(feature = "encrypted-portability")]
            Self::RecoveryProfile(source) => Some(source),
            Self::Contract(source) => Some(source),
            Self::Domain(source) => Some(source),
            Self::Sqlite(source) => Some(source),
            Self::Json { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<StoreError> for PortabilityError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

impl From<QueryError> for PortabilityError {
    fn from(value: QueryError) -> Self {
        Self::Query(value)
    }
}

impl From<VaultError> for PortabilityError {
    fn from(value: VaultError) -> Self {
        Self::Vault(value)
    }
}

#[cfg(feature = "plaintext-portability")]
impl From<ProjectionError> for PortabilityError {
    fn from(value: ProjectionError) -> Self {
        Self::Projection(value)
    }
}

#[cfg(feature = "encrypted-portability")]
impl From<academic_recovery::BackupKeyError> for PortabilityError {
    fn from(value: academic_recovery::BackupKeyError) -> Self {
        Self::BackupKey(value)
    }
}

#[cfg(feature = "encrypted-portability")]
impl From<academic_recovery::SealedManifestError> for PortabilityError {
    fn from(value: academic_recovery::SealedManifestError) -> Self {
        Self::SealedManifest(value)
    }
}

#[cfg(feature = "encrypted-portability")]
impl From<academic_recovery::RecoveryProfileError> for PortabilityError {
    fn from(value: academic_recovery::RecoveryProfileError) -> Self {
        Self::RecoveryProfile(value)
    }
}

#[cfg(feature = "encrypted-portability")]
impl From<academic_crypto::KeyScheduleError> for PortabilityError {
    fn from(_: academic_crypto::KeyScheduleError) -> Self {
        Self::KeySchedule
    }
}

impl From<ContractError> for PortabilityError {
    fn from(value: ContractError) -> Self {
        Self::Contract(value)
    }
}

impl From<DomainError> for PortabilityError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

impl From<rusqlite::Error> for PortabilityError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

/// Result type for every portability operation.
pub type PortabilityResult<T> = Result<T, PortabilityError>;

/// Durable directory plumbing shared by export, backup, and restore.
///
/// Publication is always the same shape: build a sibling staging directory,
/// synchronize every written file, synchronize the directory tree, and then
/// rename the staging root onto the never-previously-existing destination. A
/// process that dies before that rename leaves an unpublished staging root and
/// an untouched destination.
pub(crate) mod directory {
    use std::{
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        path::{Component, Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use academic_domain::ContentDigest;
    use sha2::{Digest, Sha256};

    use super::{MAX_PORTABLE_RELATIVE_PATH_BYTES, PortabilityError, PortabilityResult};

    const COPY_CHUNK_BYTES: usize = 64 * 1024;
    static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

    /// Rejects a produced relative path that exceeds the portable budget.
    ///
    /// Reserved Windows device names, trailing dots or spaces, and characters
    /// Windows refuses in a path component also fail closed here, so a format
    /// change that would make a directory unwritable on Windows is caught on
    /// every host rather than only on Windows.
    pub(crate) fn check_relative_path(relative: &str) -> PortabilityResult<()> {
        if relative.len() > MAX_PORTABLE_RELATIVE_PATH_BYTES {
            return Err(PortabilityError::PathTooLong {
                path: PathBuf::from(relative),
                limit: MAX_PORTABLE_RELATIVE_PATH_BYTES,
            });
        }
        for component in relative.split('/') {
            if component.is_empty()
                || component.ends_with('.')
                || component.ends_with(' ')
                || component.bytes().any(|byte| {
                    matches!(byte, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*') || byte < 0x20
                })
            {
                return Err(PortabilityError::UnsafeEntry(PathBuf::from(relative)));
            }
            let stem = component.split('.').next().unwrap_or(component);
            if WINDOWS_RESERVED_NAMES
                .iter()
                .any(|reserved| reserved.eq_ignore_ascii_case(stem))
            {
                return Err(PortabilityError::UnsafeEntry(PathBuf::from(relative)));
            }
        }
        Ok(())
    }

    /// Path components Windows refuses regardless of directory or extension.
    const WINDOWS_RESERVED_NAMES: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    /// Returns a relative, forward-slash path string for manifests.
    ///
    /// The separator is normalized so a Windows-produced manifest and a
    /// Linux-produced manifest are byte-identical.
    pub(crate) fn relative_path_string(root: &Path, path: &Path) -> PortabilityResult<String> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| PortabilityError::UnsafeEntry(path.to_path_buf()))?;
        let mut parts = Vec::new();
        for component in relative.components() {
            match component {
                Component::Normal(part) => {
                    let part = part
                        .to_str()
                        .ok_or_else(|| PortabilityError::UnsafeEntry(path.to_path_buf()))?;
                    parts.push(part.to_owned());
                }
                _ => return Err(PortabilityError::UnsafeEntry(path.to_path_buf())),
            }
        }
        if parts.is_empty() {
            return Err(PortabilityError::UnsafeEntry(path.to_path_buf()));
        }
        Ok(parts.join("/"))
    }

    /// Resolves a manifest-relative forward-slash path inside one root.
    ///
    /// Absolute paths, drive letters, parent traversal, empty components, and
    /// backslashes fail closed so a hostile manifest cannot escape the root.
    pub(crate) fn resolve_relative(root: &Path, relative: &str) -> PortabilityResult<PathBuf> {
        if relative.is_empty() || relative.contains('\\') || relative.contains('\0') {
            return Err(PortabilityError::UnsafeEntry(PathBuf::from(relative)));
        }
        let mut resolved = root.to_path_buf();
        for part in relative.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return Err(PortabilityError::UnsafeEntry(PathBuf::from(relative)));
            }
            resolved.push(part);
        }
        Ok(resolved)
    }

    /// Reserves an absent sibling staging path next to a destination.
    pub(crate) fn reserve_staging_path(
        destination: &Path,
        suffix: &str,
    ) -> PortabilityResult<PathBuf> {
        let parent = destination
            .parent()
            .ok_or_else(|| PortabilityError::UnsafeEntry(destination.to_path_buf()))?;
        let name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| PortabilityError::UnsafeEntry(destination.to_path_buf()))?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PortabilityError::ClockUnavailable)?
            .as_nanos();
        for _ in 0..64 {
            let sequence = NEXT_STAGING.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!(
                "{name}.{suffix}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::symlink_metadata(&candidate) {
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(candidate);
                }
                Ok(_) => {}
                Err(source) => {
                    return Err(PortabilityError::io(
                        "inspect staging directory",
                        candidate,
                        source,
                    ));
                }
            }
        }
        Err(PortabilityError::UnsafeEntry(destination.to_path_buf()))
    }

    /// Creates one new directory that must not already exist.
    pub(crate) fn create_new_directory(path: &Path) -> PortabilityResult<()> {
        create_private_directory(path)
    }

    /// Creates every missing directory below a root, refusing traversal.
    pub(crate) fn create_directories(path: &Path) -> PortabilityResult<()> {
        let mut current = PathBuf::new();
        for component in path.components() {
            current.push(component.as_os_str());
            if current.parent().is_none() {
                continue;
            }
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_dir() => {}
                Ok(_) => return Err(PortabilityError::UnsafeEntry(current.clone())),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    create_private_directory(&current)?;
                }
                Err(source) => {
                    return Err(PortabilityError::io(
                        "inspect portability directory",
                        current,
                        source,
                    ));
                }
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    fn create_private_directory(path: &Path) -> PortabilityResult<()> {
        use std::os::unix::fs::DirBuilderExt;

        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(path)
            .map_err(|source| PortabilityError::io("create portability directory", path, source))
    }

    #[cfg(not(unix))]
    fn create_private_directory(path: &Path) -> PortabilityResult<()> {
        fs::create_dir(path)
            .map_err(|source| PortabilityError::io("create portability directory", path, source))
    }

    /// Writes one new file and synchronizes its bytes before returning.
    pub(crate) fn write_new_file(path: &Path, bytes: &[u8]) -> PortabilityResult<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|source| PortabilityError::io("create portability file", path, source))?;
        file.write_all(bytes)
            .map_err(|source| PortabilityError::io("write portability file", path, source))?;
        file.sync_all()
            .map_err(|source| PortabilityError::io("synchronize portability file", path, source))
    }

    /// Copies one file, synchronizes it, and returns its exact digest and length.
    pub(crate) fn copy_new_file(
        source: &Path,
        destination: &Path,
    ) -> PortabilityResult<(ContentDigest, u64)> {
        let mut input = File::open(source)
            .map_err(|error| PortabilityError::io("open portability source", source, error))?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .map_err(|error| PortabilityError::io("create portability copy", destination, error))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; COPY_CHUNK_BYTES];
        let mut length = 0_u64;
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| PortabilityError::io("read portability source", source, error))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            output.write_all(&buffer[..read]).map_err(|error| {
                PortabilityError::io("write portability copy", destination, error)
            })?;
            length = length.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        }
        output.sync_all().map_err(|error| {
            PortabilityError::io("synchronize portability copy", destination, error)
        })?;
        Ok((
            ContentDigest::from_sha256_bytes(hasher.finalize().into()),
            length,
        ))
    }

    /// Synchronizes one directory entry.
    ///
    /// Unix flushes the directory handle itself. Windows exposes no directory
    /// flush through the standard library, and the vault's native barrier writes
    /// a helper file that a byte-exact export directory must not contain; every
    /// portability file is therefore `sync_all`ed before publication and the
    /// publish rename is ordered by the filesystem's own metadata journal. The
    /// directory shape is still revalidated so a replaced entry fails closed.
    pub(crate) fn sync_directory(path: &Path) -> PortabilityResult<()> {
        let metadata = fs::symlink_metadata(path).map_err(|source| {
            PortabilityError::io("inspect portability directory", path, source)
        })?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(PortabilityError::UnsafeEntry(path.to_path_buf()));
        }
        #[cfg(unix)]
        {
            let directory = File::open(path).map_err(|source| {
                PortabilityError::io("open portability directory", path, source)
            })?;
            directory.sync_all().map_err(|source| {
                PortabilityError::io("synchronize portability directory", path, source)
            })?;
        }
        Ok(())
    }

    /// Synchronizes a complete directory tree, deepest entries first.
    pub(crate) fn sync_tree(root: &Path) -> PortabilityResult<()> {
        for entry in read_directory(root)? {
            let metadata = fs::symlink_metadata(&entry).map_err(|source| {
                PortabilityError::io("inspect portability entry", &entry, source)
            })?;
            if metadata.file_type().is_symlink() {
                return Err(PortabilityError::UnsafeEntry(entry));
            }
            if metadata.file_type().is_dir() {
                sync_tree(&entry)?;
            }
        }
        sync_directory(root)
    }

    /// Lists one directory's immediate children in stable sorted order.
    pub(crate) fn read_directory(root: &Path) -> PortabilityResult<Vec<PathBuf>> {
        let mut entries = Vec::new();
        let listing = fs::read_dir(root).map_err(|source| {
            PortabilityError::io("enumerate portability directory", root, source)
        })?;
        for entry in listing {
            let entry = entry.map_err(|source| {
                PortabilityError::io("read portability directory entry", root, source)
            })?;
            entries.push(entry.path());
        }
        entries.sort();
        Ok(entries)
    }

    /// Lists every regular file below a root as sorted relative forward-slash paths.
    pub(crate) fn list_files(root: &Path) -> PortabilityResult<Vec<String>> {
        let mut files = Vec::new();
        collect_files(root, root, &mut files)?;
        files.sort();
        Ok(files)
    }

    fn collect_files(
        root: &Path,
        current: &Path,
        files: &mut Vec<String>,
    ) -> PortabilityResult<()> {
        for entry in read_directory(current)? {
            let metadata = fs::symlink_metadata(&entry).map_err(|source| {
                PortabilityError::io("inspect portability entry", &entry, source)
            })?;
            if metadata.file_type().is_symlink() {
                return Err(PortabilityError::UnsafeEntry(entry));
            }
            if metadata.file_type().is_dir() {
                collect_files(root, &entry, files)?;
            } else if metadata.file_type().is_file() {
                files.push(relative_path_string(root, &entry)?);
            } else {
                return Err(PortabilityError::UnsafeEntry(entry));
            }
        }
        Ok(())
    }

    /// Renames a fully synchronized staging root onto an absent destination.
    pub(crate) fn publish(staging: &Path, destination: &Path) -> PortabilityResult<()> {
        require_absent(destination)?;
        fs::rename(staging, destination).map_err(|source| {
            PortabilityError::io("publish portability directory", destination, source)
        })?;
        if let Some(parent) = destination.parent() {
            sync_directory(parent)?;
        }
        Ok(())
    }

    /// Publishes onto a destination that is absent or an existing empty directory.
    ///
    /// An existing empty directory is removed immediately before the rename so
    /// the published profile keeps the staging root's owner-only creation
    /// identity instead of an inherited one.
    pub(crate) fn publish_over_empty(staging: &Path, destination: &Path) -> PortabilityResult<()> {
        match fs::symlink_metadata(destination) {
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(PortabilityError::io(
                    "inspect restore destination",
                    destination,
                    source,
                ));
            }
            Ok(metadata) => {
                if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                    return Err(PortabilityError::DestinationNotEmpty(
                        destination.to_path_buf(),
                    ));
                }
                if !read_directory(destination)?.is_empty() {
                    return Err(PortabilityError::DestinationNotEmpty(
                        destination.to_path_buf(),
                    ));
                }
                fs::remove_dir(destination).map_err(|source| {
                    PortabilityError::io("remove empty restore destination", destination, source)
                })?;
            }
        }
        fs::rename(staging, destination).map_err(|source| {
            PortabilityError::io("publish restored profile", destination, source)
        })?;
        if let Some(parent) = destination.parent() {
            sync_directory(parent)?;
        }
        Ok(())
    }

    /// Fails closed unless the path does not exist at all.
    pub(crate) fn require_absent(path: &Path) -> PortabilityResult<()> {
        match fs::symlink_metadata(path) {
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok(_) => Err(PortabilityError::DestinationExists(path.to_path_buf())),
            Err(source) => Err(PortabilityError::io(
                "inspect portability destination",
                path,
                source,
            )),
        }
    }

    /// Accepts only an absent path or an existing empty directory.
    pub(crate) fn require_new_empty_directory(path: &Path) -> PortabilityResult<()> {
        match fs::symlink_metadata(path) {
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(PortabilityError::io(
                "inspect restore destination",
                path,
                source,
            )),
            Ok(metadata) => {
                if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                    return Err(PortabilityError::DestinationNotEmpty(path.to_path_buf()));
                }
                if read_directory(path)?.is_empty() {
                    Ok(())
                } else {
                    Err(PortabilityError::DestinationNotEmpty(path.to_path_buf()))
                }
            }
        }
    }

    /// Marker embedded in every staging directory name this crate creates.
    pub(crate) const STAGING_NAME_MARKER: &str = "-staging-";

    /// Lists unpublished staging directories left beside one destination.
    ///
    /// Only directories this crate itself named are ever reported.
    pub(crate) fn find_staging_directories(destination: &Path) -> PortabilityResult<Vec<PathBuf>> {
        let Some(parent) = destination.parent() else {
            return Ok(Vec::new());
        };
        let Some(name) = destination.file_name().and_then(|value| value.to_str()) else {
            return Ok(Vec::new());
        };
        let prefix = format!("{name}.");
        let mut staged = Vec::new();
        for entry in read_directory(parent)? {
            let Some(candidate) = entry.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if candidate.starts_with(&prefix) && candidate.contains(STAGING_NAME_MARKER) {
                staged.push(entry);
            }
        }
        Ok(staged)
    }

    /// Removes an unpublished staging tree that this crate created.
    ///
    /// The path must be a real directory sitting beside `destination` whose name
    /// this crate produced, so the recursive removal can never reach an
    /// unrelated location, a symlinked target, or the destination itself.
    pub(crate) fn remove_staging_directory(
        destination: &Path,
        staging: &Path,
    ) -> PortabilityResult<()> {
        let expected_parent = destination.parent();
        if staging.parent() != expected_parent {
            return Err(PortabilityError::UnsafeEntry(staging.to_path_buf()));
        }
        let name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| PortabilityError::UnsafeEntry(destination.to_path_buf()))?;
        let candidate = staging
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| PortabilityError::UnsafeEntry(staging.to_path_buf()))?;
        if !candidate.starts_with(&format!("{name}.")) || !candidate.contains(STAGING_NAME_MARKER) {
            return Err(PortabilityError::UnsafeEntry(staging.to_path_buf()));
        }
        let metadata = fs::symlink_metadata(staging)
            .map_err(|source| PortabilityError::io("inspect staging directory", staging, source))?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            return Err(PortabilityError::UnsafeEntry(staging.to_path_buf()));
        }
        remove_tree(staging)
    }

    fn remove_tree(path: &Path) -> PortabilityResult<()> {
        for entry in read_directory(path)? {
            let metadata = fs::symlink_metadata(&entry)
                .map_err(|source| PortabilityError::io("inspect staging entry", &entry, source))?;
            if metadata.file_type().is_symlink() {
                return Err(PortabilityError::UnsafeEntry(entry));
            }
            if metadata.file_type().is_dir() {
                remove_tree(&entry)?;
            } else if metadata.file_type().is_file() {
                fs::remove_file(&entry).map_err(|source| {
                    PortabilityError::io("remove staging file", &entry, source)
                })?;
            } else {
                return Err(PortabilityError::UnsafeEntry(entry));
            }
        }
        fs::remove_dir(path)
            .map_err(|source| PortabilityError::io("remove staging directory", path, source))
    }

    /// Returns the current instant in stable Unix milliseconds.
    pub(crate) fn now_unix_millis() -> PortabilityResult<i64> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PortabilityError::ClockUnavailable)?;
        i64::try_from(elapsed.as_millis()).map_err(|_| PortabilityError::ClockUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portability_contract_preserves_projection_non_authority() {
        const {
            assert!(RESTORE_REQUIRES_EMPTY_PROFILE);
            assert!(!EXPORT_INCLUDES_PROJECTIONS_BY_DEFAULT);
        }
    }

    #[test]
    fn produced_relative_paths_stay_windows_safe() {
        let long = format!(
            "objects/{}.bin",
            "a".repeat(MAX_PORTABLE_RELATIVE_PATH_BYTES)
        );
        assert!(matches!(
            directory::check_relative_path(&long),
            Err(PortabilityError::PathTooLong { .. })
        ));
        for rejected in [
            "ledger/CON.cbor",
            "ledger/nul",
            "canonical/lpt1.jsonl",
            "objects/trailing.",
            "objects/trailing ",
            "objects/a:b.bin",
            "objects/a?b.bin",
            "objects/a|b.bin",
        ] {
            assert!(
                matches!(
                    directory::check_relative_path(rejected),
                    Err(PortabilityError::UnsafeEntry(_))
                ),
                "{rejected} was accepted as a portable relative path"
            );
        }
        for accepted in [
            "manifest.json",
            "ledger/batches/01900000-0000-7000-8000-00000b000001.cbor",
            "objects/01900000-0000-7000-8000-000000000101/             01900000-0000-7000-8000-000000000201.bin",
        ] {
            assert!(
                directory::check_relative_path(accepted).is_ok(),
                "{accepted} was refused as a portable relative path"
            );
        }
    }

    #[test]
    fn backup_format_never_claims_confidentiality() {
        assert!(PHASE1_BACKUP_PLAINTEXT_WARNING.contains("NOT ENCRYPTED"));
        assert!(PHASE1_BACKUP_PLAINTEXT_WARNING.contains("NOT CONFIDENTIAL"));
        assert!(!PHASE1_BACKUP_FORMAT.contains("secure"));
        assert!(!PHASE1_EXPORT_FORMAT.contains("secure"));
    }
}
