//! Fail-closed profile-root policy with an injectable operating-system probe.

use std::{
    fmt,
    path::{Component, Path, PathBuf},
};

#[cfg(windows)]
use academic_store_platform::{WindowsPathKind, classify_windows_path};

use crate::platform;

/// Whether the requested profile root currently exists and is usable as a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileRootState {
    /// The final profile component does not exist yet.
    Missing,
    /// The final component is an empty directory.
    EmptyDirectory,
    /// The final component is a non-empty directory.
    NonEmptyDirectory,
    /// The final component exists but is not a directory.
    NotDirectory,
}

/// Storage locality determined by a native or injected capability probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageLocality {
    /// A supported local filesystem/device.
    Local,
    /// A network, remote-device, or consumer remote filesystem.
    Remote,
    /// The probe could not prove local storage.
    Unknown,
}

/// Effective access state for the profile directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAccess {
    /// The existing directory is owned by and accessible only to the current user.
    OwnerOnly,
    /// The platform adapter can create a missing final directory as owner-only.
    OwnerOnlyOnCreate,
    /// The existing directory grants broader access than the profile policy allows.
    Broad,
    /// The probe could not prove the effective access boundary.
    Unknown,
}

/// Stable classification for a native probe failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathProbeFailureCode {
    /// The nearest existing ancestor could not be found or canonicalized.
    Canonicalization,
    /// Filesystem or device locality could not be inspected.
    StorageInspection,
    /// Ownership or access could not be inspected.
    AccessInspection,
    /// A required platform capability is unavailable.
    UnsupportedPlatform,
    /// Another operating-system I/O operation failed.
    OperatingSystem,
}

/// Bounded failure returned by [`PathProbe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathProbeFailure {
    /// Stable failure category.
    pub code: PathProbeFailureCode,
    /// Privacy-safe diagnostic detail.
    pub detail: String,
}

impl PathProbeFailure {
    /// Constructs a probe failure with a stable category.
    #[must_use]
    pub fn new(code: PathProbeFailureCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for PathProbeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.detail)
    }
}

/// Complete evidence consumed by the host-independent path-policy decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEvidence {
    /// Canonical nearest existing ancestor, obtained without trusting the final component.
    pub canonical_existing_ancestor: PathBuf,
    /// State of the requested final root.
    pub root_state: ProfileRootState,
    /// Proved storage locality.
    pub storage_locality: StorageLocality,
    /// Proved access state for the final directory or its secure creation.
    pub access: ProfileAccess,
    /// At least one existing component is a symbolic link or Windows reparse point.
    pub has_symlink_or_reparse_component: bool,
    /// The requested path falls within a configured consumer synchronization root.
    pub is_sync_folder: bool,
    /// An existing ancestor contains a `.git` directory or worktree file.
    pub has_git_ancestor: bool,
    /// An existing final directory resolves to the identity the probe opened and inspected.
    pub final_identity_matches: bool,
}

impl PathEvidence {
    /// Builds safe evidence for a missing root in deterministic injected tests.
    #[must_use]
    pub fn safe_missing(canonical_existing_ancestor: impl Into<PathBuf>) -> Self {
        Self {
            canonical_existing_ancestor: canonical_existing_ancestor.into(),
            root_state: ProfileRootState::Missing,
            storage_locality: StorageLocality::Local,
            access: ProfileAccess::OwnerOnlyOnCreate,
            has_symlink_or_reparse_component: false,
            is_sync_folder: false,
            has_git_ancestor: false,
            final_identity_matches: true,
        }
    }

    /// Builds safe evidence for an existing empty owner-only directory.
    #[must_use]
    pub fn safe_empty(canonical_root: impl Into<PathBuf>) -> Self {
        Self {
            canonical_existing_ancestor: canonical_root.into(),
            root_state: ProfileRootState::EmptyDirectory,
            storage_locality: StorageLocality::Local,
            access: ProfileAccess::OwnerOnly,
            has_symlink_or_reparse_component: false,
            is_sync_folder: false,
            has_git_ancestor: false,
            final_identity_matches: true,
        }
    }
}

/// Injectable boundary used to make every path-policy case executable on every host.
pub trait PathProbe: fmt::Debug + Send + Sync {
    /// Inspects a profile root without creating it or following a final link/reparse point.
    fn inspect(&self, requested_root: &Path) -> Result<PathEvidence, PathProbeFailure>;
}

/// Real current-host path probe.
#[derive(Clone, Default)]
pub struct NativePathProbe {
    configured_sync_roots: Vec<PathBuf>,
}

impl fmt::Debug for NativePathProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativePathProbe")
            .field(
                "configured_sync_root_count",
                &self.configured_sync_roots.len(),
            )
            .finish_non_exhaustive()
    }
}

impl NativePathProbe {
    /// Adds explicitly configured synchronization roots to the native provider checks.
    #[must_use]
    pub fn with_sync_roots(configured_sync_roots: Vec<PathBuf>) -> Self {
        Self {
            configured_sync_roots,
        }
    }
}

impl PathProbe for NativePathProbe {
    fn inspect(&self, requested_root: &Path) -> Result<PathEvidence, PathProbeFailure> {
        platform::inspect_profile_path(requested_root, &self.configured_sync_roots)
    }
}

/// Stable fail-closed reason returned by the host-independent policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathPolicyViolation {
    /// The supplied path was empty.
    EmptyPath,
    /// The path cannot be represented losslessly as Unicode for policy comparison.
    NonUnicodePath,
    /// The supplied path was not absolute on the current host.
    RelativePath,
    /// Dot or parent traversal appeared in the requested spelling.
    TraversalComponent,
    /// The path used a URI-like spelling rather than a local filesystem path.
    UriLikePath,
    /// A Windows UNC or slash-normalized network share was requested.
    NetworkShare,
    /// A Windows device/verbatim namespace was requested.
    DevicePath,
    /// A Windows drive spelling was supplied on a non-Windows host.
    ForeignDrivePath,
    /// The final root is not empty for new-profile creation.
    ProfileNotEmpty,
    /// The final component is not a directory.
    ProfileNotDirectory,
    /// The directory exists but holds no store database to read.
    MissingStoreDatabase,
    /// The probe proved a remote/network filesystem or device.
    RemoteStorage,
    /// The probe could not prove local storage.
    UnknownStorage,
    /// A symbolic link or reparse-point component was found.
    SymlinkOrReparsePoint,
    /// The path is inside a configured consumer synchronization root.
    SyncFolder,
    /// The path is inside a Git repository or linked worktree.
    GitWorktree,
    /// The existing directory grants access beyond the current user.
    BroadAccess,
    /// The probe could not prove an owner-only access boundary.
    UnknownAccess,
    /// The inspected final identity no longer matches the requested directory.
    FinalIdentityChanged,
    /// The native capability probe itself failed.
    ProbeFailed(PathProbeFailure),
}

impl fmt::Display for PathPolicyViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => formatter.write_str("empty path"),
            Self::NonUnicodePath => formatter.write_str("non-Unicode path"),
            Self::RelativePath => formatter.write_str("path is not absolute"),
            Self::TraversalComponent => formatter.write_str("path contains dot traversal"),
            Self::UriLikePath => formatter.write_str("URI-like path is not a profile root"),
            Self::NetworkShare => formatter.write_str("network share path is forbidden"),
            Self::DevicePath => formatter.write_str("device/verbatim path is forbidden"),
            Self::ForeignDrivePath => {
                formatter.write_str("Windows drive path is not native on this host")
            }
            Self::ProfileNotEmpty => formatter.write_str("new profile root is not empty"),
            Self::ProfileNotDirectory => formatter.write_str("profile root is not a directory"),
            Self::MissingStoreDatabase => {
                formatter.write_str("profile root holds no store database")
            }
            Self::RemoteStorage => formatter.write_str("remote storage is forbidden"),
            Self::UnknownStorage => formatter.write_str("local storage could not be proved"),
            Self::SymlinkOrReparsePoint => {
                formatter.write_str("symlink or reparse traversal is forbidden")
            }
            Self::SyncFolder => formatter.write_str("consumer sync folder is forbidden"),
            Self::GitWorktree => formatter.write_str("Git worktree profile is forbidden"),
            Self::BroadAccess => formatter.write_str("profile directory access is too broad"),
            Self::UnknownAccess => formatter.write_str("owner-only access could not be proved"),
            Self::FinalIdentityChanged => {
                formatter.write_str("final directory identity changed during inspection")
            }
            Self::ProbeFailed(source) => write!(formatter, "path probe failed: {source}"),
        }
    }
}

/// A profile path that passed the complete creation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProfilePath {
    requested_root: PathBuf,
    canonical_existing_ancestor: PathBuf,
    root_state: ProfileRootState,
}

impl ValidatedProfilePath {
    /// Returns the exact absolute root requested by the caller.
    #[must_use]
    pub fn requested_root(&self) -> &Path {
        &self.requested_root
    }

    /// Returns the canonical nearest ancestor used for policy decisions.
    #[must_use]
    pub fn canonical_existing_ancestor(&self) -> &Path {
        &self.canonical_existing_ancestor
    }

    /// Returns the root state observed by the probe.
    #[must_use]
    pub const fn root_state(&self) -> ProfileRootState {
        self.root_state
    }
}

/// Validates a path for creation of a new empty synthetic profile.
pub fn validate_new_profile_path<P: PathProbe + ?Sized>(
    requested_root: &Path,
    probe: &P,
) -> Result<ValidatedProfilePath, PathPolicyViolation> {
    validate_lexical_path(requested_root)?;
    let evidence = probe
        .inspect(requested_root)
        .map_err(PathPolicyViolation::ProbeFailed)?;
    validate_common_evidence(&evidence)?;
    match evidence.root_state {
        ProfileRootState::Missing => {
            if evidence.access != ProfileAccess::OwnerOnlyOnCreate {
                return Err(match evidence.access {
                    ProfileAccess::Broad => PathPolicyViolation::BroadAccess,
                    ProfileAccess::OwnerOnly
                    | ProfileAccess::OwnerOnlyOnCreate
                    | ProfileAccess::Unknown => PathPolicyViolation::UnknownAccess,
                });
            }
        }
        ProfileRootState::EmptyDirectory => {
            require_owner_only(evidence.access)?;
        }
        ProfileRootState::NonEmptyDirectory => {
            return Err(PathPolicyViolation::ProfileNotEmpty);
        }
        ProfileRootState::NotDirectory => {
            return Err(PathPolicyViolation::ProfileNotDirectory);
        }
    }
    Ok(validated(requested_root, evidence))
}

/// Revalidates the just-created, still-empty final directory before profile bytes are written.
pub fn validate_created_profile_path<P: PathProbe + ?Sized>(
    requested_root: &Path,
    probe: &P,
) -> Result<ValidatedProfilePath, PathPolicyViolation> {
    validate_lexical_path(requested_root)?;
    let evidence = probe
        .inspect(requested_root)
        .map_err(PathPolicyViolation::ProbeFailed)?;
    validate_common_evidence(&evidence)?;
    if evidence.root_state != ProfileRootState::EmptyDirectory {
        return Err(match evidence.root_state {
            ProfileRootState::Missing | ProfileRootState::NotDirectory => {
                PathPolicyViolation::ProfileNotDirectory
            }
            ProfileRootState::NonEmptyDirectory => PathPolicyViolation::ProfileNotEmpty,
            ProfileRootState::EmptyDirectory => PathPolicyViolation::ProfileNotEmpty,
        });
    }
    require_owner_only(evidence.access)?;
    Ok(validated(requested_root, evidence))
}

/// Validates the locality, identity, and access of an already populated profile root.
pub fn validate_existing_profile_path<P: PathProbe + ?Sized>(
    requested_root: &Path,
    probe: &P,
) -> Result<ValidatedProfilePath, PathPolicyViolation> {
    validate_lexical_path(requested_root)?;
    let evidence = probe
        .inspect(requested_root)
        .map_err(PathPolicyViolation::ProbeFailed)?;
    validate_common_evidence(&evidence)?;
    match evidence.root_state {
        ProfileRootState::EmptyDirectory | ProfileRootState::NonEmptyDirectory => {}
        ProfileRootState::Missing => return Err(PathPolicyViolation::ProfileNotDirectory),
        ProfileRootState::NotDirectory => return Err(PathPolicyViolation::ProfileNotDirectory),
    }
    require_owner_only(evidence.access)?;
    Ok(validated(requested_root, evidence))
}

fn validated(requested_root: &Path, evidence: PathEvidence) -> ValidatedProfilePath {
    ValidatedProfilePath {
        requested_root: requested_root.to_path_buf(),
        canonical_existing_ancestor: evidence.canonical_existing_ancestor,
        root_state: evidence.root_state,
    }
}

fn validate_common_evidence(evidence: &PathEvidence) -> Result<(), PathPolicyViolation> {
    match evidence.storage_locality {
        StorageLocality::Local => {}
        StorageLocality::Remote => return Err(PathPolicyViolation::RemoteStorage),
        StorageLocality::Unknown => return Err(PathPolicyViolation::UnknownStorage),
    }
    if evidence.has_symlink_or_reparse_component {
        return Err(PathPolicyViolation::SymlinkOrReparsePoint);
    }
    if evidence.is_sync_folder {
        return Err(PathPolicyViolation::SyncFolder);
    }
    if evidence.has_git_ancestor {
        return Err(PathPolicyViolation::GitWorktree);
    }
    if matches!(
        evidence.root_state,
        ProfileRootState::EmptyDirectory | ProfileRootState::NonEmptyDirectory
    ) && !evidence.final_identity_matches
    {
        return Err(PathPolicyViolation::FinalIdentityChanged);
    }
    Ok(())
}

fn require_owner_only(access: ProfileAccess) -> Result<(), PathPolicyViolation> {
    match access {
        ProfileAccess::OwnerOnly => Ok(()),
        ProfileAccess::Broad => Err(PathPolicyViolation::BroadAccess),
        ProfileAccess::OwnerOnlyOnCreate | ProfileAccess::Unknown => {
            Err(PathPolicyViolation::UnknownAccess)
        }
    }
}

fn validate_lexical_path(path: &Path) -> Result<(), PathPolicyViolation> {
    let text = path.to_str().ok_or(PathPolicyViolation::NonUnicodePath)?;
    if text.is_empty() {
        return Err(PathPolicyViolation::EmptyPath);
    }
    if text.chars().any(char::is_control) {
        return Err(PathPolicyViolation::DevicePath);
    }
    let slash_normalized = text.replace('\\', "/");
    let lowercase = slash_normalized.to_ascii_lowercase();
    if lowercase.contains("://") || lowercase.starts_with("file:") {
        return Err(PathPolicyViolation::UriLikePath);
    }
    if text.starts_with("\\\\?\\")
        || text.starts_with("\\\\.\\")
        || slash_normalized.starts_with("//?/")
        || slash_normalized.starts_with("//./")
    {
        return Err(PathPolicyViolation::DevicePath);
    }
    if text.starts_with("\\\\") || slash_normalized.starts_with("//") {
        return Err(PathPolicyViolation::NetworkShare);
    }
    if slash_normalized
        .split('/')
        .any(|component| matches!(component, "." | ".."))
    {
        return Err(PathPolicyViolation::TraversalComponent);
    }
    #[cfg(windows)]
    match classify_windows_path(text) {
        WindowsPathKind::Unc => return Err(PathPolicyViolation::NetworkShare),
        WindowsPathKind::Device => return Err(PathPolicyViolation::DevicePath),
        WindowsPathKind::DriveRelative => return Err(PathPolicyViolation::RelativePath),
        WindowsPathKind::DriveAbsolute | WindowsPathKind::Other => {}
        _ => return Err(PathPolicyViolation::DevicePath),
    }
    #[cfg(not(windows))]
    if looks_like_windows_drive(text) {
        return Err(PathPolicyViolation::ForeignDrivePath);
    }
    if !path.is_absolute() {
        return Err(PathPolicyViolation::RelativePath);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(PathPolicyViolation::TraversalComponent);
    }
    Ok(())
}

#[cfg(not(windows))]
fn looks_like_windows_drive(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}
