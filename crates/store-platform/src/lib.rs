//! Reviewed native path capabilities for the synthetic-only store.
//!
//! This private workspace crate is the only place where the store may obtain
//! operating-system path identity, locality, and access-control facts. Its
//! public API is safe and never exposes a raw handle or descriptor.

use std::{
    fmt,
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// Host-independent classification of a Windows path spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowsPathKind {
    /// A drive-qualified absolute path such as `C:\\profile`.
    DriveAbsolute,
    /// A UNC or slash-normalized network share.
    Unc,
    /// A Win32 verbatim, device, DOS-device, or NT-device spelling.
    Device,
    /// A drive-relative spelling such as `C:profile`.
    DriveRelative,
    /// A spelling that is not one of the Windows forms above.
    Other,
}

/// Drive type returned by an injected or native Windows API seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowsDriveKind {
    Fixed,
    Remote,
    Removable,
    Optical,
    RamDisk,
    NoRoot,
    Unknown,
}

/// Remote-device characteristic returned by an injected or native volume seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RemoteDeviceFact {
    Local,
    Remote,
    Unknown,
}

/// Why a storage probe could not prove that a path is local.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalityUnknown {
    UnsupportedDriveKind(WindowsDriveKind),
    RemoteDeviceUnavailable,
    NoCoveringMount,
    UnrecognizedFileSystem(String),
    /// The volume backing the probed descriptor is removable media.
    ///
    /// macOS produces this when `fstatfs` on the pinned descriptor reports a
    /// mount that is otherwise local -- `MNT_LOCAL` set and `f_fstypename` not
    /// a known network filesystem -- but that also carries `MNT_REMOVABLE`.
    /// Removable media can vanish under an open profile, so it is never
    /// promoted to `Local`.
    RemovableVolume,
    UnsupportedPlatform,
}

/// Storage locality. Unknown is intentionally distinct from local.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StorageLocality {
    Local,
    Remote,
    Unknown(LocalityUnknown),
}

/// Current state of the requested final component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RootState {
    Missing,
    EmptyDirectory,
    NonEmptyDirectory,
    NotDirectory,
}

/// Verified access boundary of the requested directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DirectoryAccess {
    /// Current user and, on Windows, LocalSystem are the only allow trustees.
    OwnerOnly,
    /// The final component is missing and must use protected creation.
    RequiresProtectedCreation,
    /// The directory contains an access grant or mode broader than policy.
    Broad,
    /// The access boundary could not be proved.
    Unknown,
}

/// Result of the final handle/descriptor identity check.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FinalPathStatus {
    /// The existing final object was reopened without following links and its
    /// stable identity matched. The path came from the pinned native handle.
    Verified(PathBuf),
    /// The final component is absent; the nearest existing ancestor is pinned.
    Missing,
    /// The path name no longer resolved to the pinned object.
    Changed,
    /// The current Unix target cannot recover a final path from a descriptor.
    Unknown,
}

/// Complete native facts consumed by the store's host-independent `PathProbe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCapabilities {
    pub canonical_existing_ancestor: PathBuf,
    pub root_state: RootState,
    pub storage_locality: StorageLocality,
    pub access: DirectoryAccess,
    pub final_path: FinalPathStatus,
}

/// Stable category for native-boundary failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathCapabilityErrorCode {
    EmptyPath,
    NonUnicodePath,
    RelativePath,
    TraversalComponent,
    NetworkShare,
    DevicePath,
    DriveRelativePath,
    LinkOrReparsePoint,
    NotDirectoryAncestor,
    ParentMissing,
    AlreadyExists,
    RemoteStorage,
    UnknownStorage,
    BroadAccess,
    IdentityChanged,
    UnsupportedPlatform,
    OperatingSystem,
}

/// Privacy-bounded error returned by the native facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCapabilityError {
    pub code: PathCapabilityErrorCode,
    pub operation: &'static str,
    pub os_code: Option<i64>,
}

impl PathCapabilityError {
    pub(crate) const fn new(
        code: PathCapabilityErrorCode,
        operation: &'static str,
        os_code: Option<i64>,
    ) -> Self {
        Self {
            code,
            operation,
            os_code,
        }
    }
}

impl fmt::Display for PathCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} while {}", self.code, self.operation)?;
        if let Some(code) = self.os_code {
            write!(formatter, " (os code {code})")?;
        }
        Ok(())
    }
}

impl std::error::Error for PathCapabilityError {}

/// Classifies a Windows spelling without touching the host operating system.
#[must_use]
pub fn classify_windows_path(value: &str) -> WindowsPathKind {
    let normalized = value.replace('/', "\\");
    let lowercase = normalized.to_ascii_lowercase();
    if lowercase.starts_with("\\\\?\\")
        || lowercase.starts_with("\\\\.\\")
        || lowercase.starts_with("\\\\??\\")
        || lowercase.starts_with("\\??\\")
        || lowercase.starts_with("\\device\\")
        || lowercase.starts_with("\\dosdevices\\")
        || lowercase.starts_with("\\global??\\")
        || lowercase.starts_with("globalroot\\")
    {
        return WindowsPathKind::Device;
    }
    if normalized.starts_with("\\\\") {
        return WindowsPathKind::Unc;
    }
    let bytes = normalized.as_bytes();
    let device_components =
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            &normalized[2..]
        } else {
            &normalized
        };
    if contains_windows_device_component(device_components) {
        return WindowsPathKind::Device;
    }
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        if bytes.get(2) == Some(&b'\\') {
            WindowsPathKind::DriveAbsolute
        } else {
            WindowsPathKind::DriveRelative
        }
    } else {
        WindowsPathKind::Other
    }
}

fn contains_windows_device_component(value: &str) -> bool {
    for component in value.split('\\').filter(|part| !part.is_empty()) {
        let trimmed = component.trim_end_matches([' ', '.']);
        if trimmed.is_empty() || trimmed != component || trimmed.contains(':') {
            return true;
        }
        let stem = trimmed.split('.').next().unwrap_or_default();
        let folded = stem.to_ascii_lowercase();
        if matches!(
            folded.as_str(),
            "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
        ) || is_numbered_windows_device(&folded, "com")
            || is_numbered_windows_device(&folded, "lpt")
        {
            return true;
        }
    }
    false
}

fn is_numbered_windows_device(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    })
}

/// Combines independent Windows drive and volume-device facts fail closed.
#[must_use]
pub fn classify_windows_storage(
    drive: WindowsDriveKind,
    remote_device: RemoteDeviceFact,
) -> StorageLocality {
    if drive == WindowsDriveKind::Remote || remote_device == RemoteDeviceFact::Remote {
        return StorageLocality::Remote;
    }
    match (drive, remote_device) {
        (WindowsDriveKind::Fixed, RemoteDeviceFact::Local) => StorageLocality::Local,
        (WindowsDriveKind::Fixed, RemoteDeviceFact::Unknown) => {
            StorageLocality::Unknown(LocalityUnknown::RemoteDeviceUnavailable)
        }
        (kind, _) => StorageLocality::Unknown(LocalityUnknown::UnsupportedDriveKind(kind)),
    }
}

/// Classifies a Linux/Unix mount type without consulting the host mount table.
#[must_use]
pub fn classify_unix_filesystem_type(filesystem_type: &str) -> StorageLocality {
    let normalized = filesystem_type.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "apfs" | "bcachefs" | "btrfs" | "erofs" | "ext2" | "ext3" | "ext4" | "f2fs" | "jfs"
        | "nilfs2" | "ntfs3" | "tmpfs" | "ubifs" | "ufs" | "vfat" | "xfs" | "zfs" => {
            StorageLocality::Local
        }
        "9p" | "afs" | "ceph" | "cifs" | "davfs" | "drvfs" | "glusterfs" | "lustre" | "nfs"
        | "nfs4" | "smb" | "smb2" | "smb3" | "sshfs" | "vboxsf" | "virtiofs" | "vmhgfs" => {
            StorageLocality::Remote
        }
        value if value.starts_with("fuse.") => StorageLocality::Remote,
        _ => StorageLocality::Unknown(LocalityUnknown::UnrecognizedFileSystem(normalized)),
    }
}

/// Inspects a path without creating it or following a link/reparse component.
pub fn inspect_path(path: &Path) -> Result<PathCapabilities, PathCapabilityError> {
    validate_common_path(path)?;
    #[cfg(unix)]
    {
        unix::inspect_path(path)
    }
    #[cfg(windows)]
    {
        windows::inspect_path(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(PathCapabilityError::new(
            PathCapabilityErrorCode::UnsupportedPlatform,
            "inspect path",
            None,
        ))
    }
}

/// Creates exactly one missing final directory with the protected platform ACL/mode.
///
/// The function reopens and verifies the directory before returning. It never
/// creates missing intermediate components.
pub fn create_owner_only_directory(path: &Path) -> Result<PathCapabilities, PathCapabilityError> {
    validate_common_path(path)?;
    #[cfg(unix)]
    {
        unix::create_owner_only_directory(path)
    }
    #[cfg(windows)]
    {
        windows::create_owner_only_directory(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(PathCapabilityError::new(
            PathCapabilityErrorCode::UnsupportedPlatform,
            "create protected directory",
            None,
        ))
    }
}

fn validate_common_path(path: &Path) -> Result<(), PathCapabilityError> {
    let value = path.to_str().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::NonUnicodePath,
            "validate path spelling",
            None,
        )
    })?;
    if value.is_empty() {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::EmptyPath,
            "validate path spelling",
            None,
        ));
    }
    #[cfg(windows)]
    {
        match classify_windows_path(value) {
            WindowsPathKind::Unc => {
                return Err(PathCapabilityError::new(
                    PathCapabilityErrorCode::NetworkShare,
                    "validate path namespace",
                    None,
                ));
            }
            WindowsPathKind::Device => {
                return Err(PathCapabilityError::new(
                    PathCapabilityErrorCode::DevicePath,
                    "validate path namespace",
                    None,
                ));
            }
            WindowsPathKind::DriveRelative => {
                return Err(PathCapabilityError::new(
                    PathCapabilityErrorCode::DriveRelativePath,
                    "validate path namespace",
                    None,
                ));
            }
            WindowsPathKind::DriveAbsolute | WindowsPathKind::Other => {}
        }
    }
    if !path.is_absolute() {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::RelativePath,
            "validate absolute path",
            None,
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::TraversalComponent,
            "validate path components",
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_namespace_classification_is_cross_host() {
        assert_eq!(
            classify_windows_path(r"C:\profile"),
            WindowsPathKind::DriveAbsolute
        );
        assert_eq!(
            classify_windows_path("C:/profile"),
            WindowsPathKind::DriveAbsolute
        );
        assert_eq!(
            classify_windows_path(r"C:profile"),
            WindowsPathKind::DriveRelative
        );
        assert_eq!(
            classify_windows_path(r"\\server\share"),
            WindowsPathKind::Unc
        );
        assert_eq!(
            classify_windows_path("//server/share"),
            WindowsPathKind::Unc
        );
        for value in [
            r"\\?\C:\profile",
            r"\\.\C:\profile",
            r"\??\C:\profile",
            r"\Device\HarddiskVolume1\profile",
            "//?/UNC/server/share",
            r"\\??\\C:\profile",
            r"\DosDevices\C:\profile",
            r"C:\NUL.txt",
            r"C:\profile\COM1",
            r"C:\profile\LPT².log",
            r"C:\profile:stream",
            r"C:\profile. ",
        ] {
            assert_eq!(
                classify_windows_path(value),
                WindowsPathKind::Device,
                "{value}"
            );
        }
    }

    #[test]
    fn injected_windows_storage_facts_fail_closed() {
        assert_eq!(
            classify_windows_storage(WindowsDriveKind::Fixed, RemoteDeviceFact::Local),
            StorageLocality::Local
        );
        assert_eq!(
            classify_windows_storage(WindowsDriveKind::Remote, RemoteDeviceFact::Unknown),
            StorageLocality::Remote
        );
        assert_eq!(
            classify_windows_storage(WindowsDriveKind::Fixed, RemoteDeviceFact::Remote),
            StorageLocality::Remote
        );
        assert_eq!(
            classify_windows_storage(WindowsDriveKind::Fixed, RemoteDeviceFact::Unknown),
            StorageLocality::Unknown(LocalityUnknown::RemoteDeviceUnavailable)
        );
        assert!(matches!(
            classify_windows_storage(WindowsDriveKind::Removable, RemoteDeviceFact::Local),
            StorageLocality::Unknown(LocalityUnknown::UnsupportedDriveKind(
                WindowsDriveKind::Removable
            ))
        ));
    }

    #[test]
    fn injected_unix_mount_types_cover_local_remote_and_unknown() {
        assert_eq!(
            classify_unix_filesystem_type("ext4"),
            StorageLocality::Local
        );
        for filesystem in ["nfs", "nfs4", "cifs", "smb3", "fuse.sshfs", "fuse.rclone"] {
            assert_eq!(
                classify_unix_filesystem_type(filesystem),
                StorageLocality::Remote,
                "{filesystem}"
            );
        }
        assert_eq!(
            classify_unix_filesystem_type("futurefs"),
            StorageLocality::Unknown(LocalityUnknown::UnrecognizedFileSystem(
                "futurefs".to_owned()
            ))
        );
    }
}
