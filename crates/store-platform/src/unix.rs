//! Safe Unix descriptor-relative implementation.

use std::{
    ffi::{OsStr, OsString},
    fs,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    path::{Component, Path, PathBuf},
};

use rustix::{
    fs::{self as rfs, AtFlags, Dir, FileType, Mode, OFlags},
    io::Errno,
    process,
};

use crate::{
    DirectoryAccess, FinalPathStatus, LocalityUnknown, PathCapabilities, PathCapabilityError,
    PathCapabilityErrorCode, RootState, StorageLocality, classify_unix_filesystem_type,
};

#[derive(Debug)]
struct Walk {
    handles: Vec<OwnedFd>,
    root_state: RootState,
    missing_components: usize,
}

pub(super) fn inspect_path(path: &Path) -> Result<PathCapabilities, PathCapabilityError> {
    let first = walk_once(path)?;
    let second = walk_once(path)?;
    if !same_walk_identity(&first, &second)? {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::IdentityChanged,
            "recheck descriptor-relative path identity",
            None,
        ));
    }
    capabilities_from_walk(first)
}

pub(super) fn create_owner_only_directory(
    path: &Path,
) -> Result<PathCapabilities, PathCapabilityError> {
    let before = walk_once(path)?;
    let confirmation = walk_once(path)?;
    if !same_walk_identity(&before, &confirmation)? {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::IdentityChanged,
            "recheck protected-directory parent identity",
            None,
        ));
    }
    if before.root_state != RootState::Missing {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::AlreadyExists,
            "create protected directory",
            None,
        ));
    }
    if before.missing_components != 1 {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::ParentMissing,
            "create protected directory",
            None,
        ));
    }
    let parent_handle = before.handles.last().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::ParentMissing,
            "retain protected-directory parent descriptor",
            None,
        )
    })?;
    let parent_final_path = descriptor_path(parent_handle)?;
    match mount_locality(&parent_final_path, parent_handle)? {
        StorageLocality::Local => {}
        StorageLocality::Remote => {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::RemoteStorage,
                "create protected directory",
                None,
            ));
        }
        StorageLocality::Unknown(_) => {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::UnknownStorage,
                "create protected directory",
                None,
            ));
        }
    }

    let name = path.file_name().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::ParentMissing,
            "resolve protected-directory name",
            None,
        )
    })?;
    rfs::mkdirat(parent_handle, name, Mode::RWXU)
        .map_err(|error| errno_error("create mode-0700 directory", error))?;

    let after = inspect_path(path)?;
    if after.root_state != RootState::EmptyDirectory
        || after.access != DirectoryAccess::OwnerOnly
        || !matches!(after.final_path, FinalPathStatus::Verified(_))
    {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::BroadAccess,
            "verify protected directory after creation",
            None,
        ));
    }
    Ok(after)
}

fn walk_once(path: &Path) -> Result<Walk, PathCapabilityError> {
    let names = normal_components(path)?;
    let mut handles = vec![
        rfs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| errno_error("open filesystem root without following links", error))?,
    ];

    for (index, name) in names.iter().enumerate() {
        let parent = handles.last().ok_or_else(|| {
            PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "retain descriptor-relative parent",
                None,
            )
        })?;
        let metadata = match rfs::statat(parent, *name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(metadata) => metadata,
            Err(Errno::NOENT) => {
                return Ok(Walk {
                    handles,
                    root_state: RootState::Missing,
                    missing_components: names.len() - index,
                });
            }
            Err(error) => {
                return Err(errno_error(
                    "inspect path component without following",
                    error,
                ));
            }
        };
        let file_type = FileType::from_raw_mode(metadata.st_mode);
        if file_type == FileType::Symlink {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::LinkOrReparsePoint,
                "inspect path component without following",
                None,
            ));
        }
        let is_last = index + 1 == names.len();
        if file_type != FileType::Directory {
            if is_last {
                return Ok(Walk {
                    handles,
                    root_state: RootState::NotDirectory,
                    missing_components: 0,
                });
            }
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::NotDirectoryAncestor,
                "walk descriptor-relative path",
                None,
            ));
        }
        let handle = rfs::openat(
            parent,
            *name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| map_open_error("open directory component without following", error))?;
        handles.push(handle);
    }

    let final_handle = handles.last().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "retain final directory descriptor",
            None,
        )
    })?;
    let root_state = if directory_is_empty(final_handle)? {
        RootState::EmptyDirectory
    } else {
        RootState::NonEmptyDirectory
    };
    Ok(Walk {
        handles,
        root_state,
        missing_components: 0,
    })
}

fn normal_components(path: &Path) -> Result<Vec<&OsStr>, PathCapabilityError> {
    let mut saw_root = false;
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => saw_root = true,
            Component::Normal(name) if saw_root => names.push(name),
            Component::Prefix(_)
            | Component::CurDir
            | Component::ParentDir
            | Component::Normal(_) => {
                return Err(PathCapabilityError::new(
                    PathCapabilityErrorCode::TraversalComponent,
                    "split descriptor-relative path",
                    None,
                ));
            }
        }
    }
    if saw_root {
        Ok(names)
    } else {
        Err(PathCapabilityError::new(
            PathCapabilityErrorCode::RelativePath,
            "split descriptor-relative path",
            None,
        ))
    }
}

fn directory_is_empty(handle: &OwnedFd) -> Result<bool, PathCapabilityError> {
    let mut directory = Dir::read_from(handle)
        .map_err(|error| errno_error("duplicate directory descriptor for enumeration", error))?;
    for entry in &mut directory {
        let entry = entry.map_err(|error| errno_error("enumerate directory descriptor", error))?;
        if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
            return Ok(false);
        }
    }
    Ok(true)
}

fn same_walk_identity(left: &Walk, right: &Walk) -> Result<bool, PathCapabilityError> {
    if left.root_state != right.root_state
        || left.missing_components != right.missing_components
        || left.handles.len() != right.handles.len()
    {
        return Ok(false);
    }
    for (left_handle, right_handle) in left.handles.iter().zip(&right.handles) {
        let left_stat = rfs::fstat(left_handle)
            .map_err(|error| errno_error("inspect pinned directory identity", error))?;
        let right_stat = rfs::fstat(right_handle)
            .map_err(|error| errno_error("reinspect directory identity", error))?;
        if left_stat.st_dev != right_stat.st_dev || left_stat.st_ino != right_stat.st_ino {
            return Ok(false);
        }
    }
    Ok(true)
}

fn capabilities_from_walk(walk: Walk) -> Result<PathCapabilities, PathCapabilityError> {
    let final_handle = walk.handles.last().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "retain nearest existing descriptor",
            None,
        )
    })?;
    let canonical_existing_ancestor = descriptor_path(final_handle)?;
    let storage_locality = mount_locality(&canonical_existing_ancestor, final_handle)?;
    let access = match walk.root_state {
        RootState::Missing => DirectoryAccess::RequiresProtectedCreation,
        RootState::EmptyDirectory | RootState::NonEmptyDirectory => directory_access(final_handle)?,
        RootState::NotDirectory => DirectoryAccess::Unknown,
    };
    let final_path = match walk.root_state {
        RootState::Missing => FinalPathStatus::Missing,
        RootState::EmptyDirectory | RootState::NonEmptyDirectory => {
            FinalPathStatus::Verified(canonical_existing_ancestor.clone())
        }
        RootState::NotDirectory => FinalPathStatus::Unknown,
    };
    Ok(PathCapabilities {
        canonical_existing_ancestor,
        root_state: walk.root_state,
        storage_locality,
        access,
        final_path,
    })
}

fn descriptor_path(handle: &OwnedFd) -> Result<PathBuf, PathCapabilityError> {
    #[cfg(target_os = "linux")]
    {
        let path =
            fs::read_link(format!("/proc/self/fd/{}", handle.as_raw_fd())).map_err(|error| {
                PathCapabilityError::new(
                    PathCapabilityErrorCode::OperatingSystem,
                    "read final path from pinned descriptor",
                    error.raw_os_error().map(i64::from),
                )
            })?;
        if path.as_os_str().as_bytes().ends_with(b" (deleted)") {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::IdentityChanged,
                "reject deleted pinned descriptor path",
                None,
            ));
        }
        Ok(path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = handle;
        Err(PathCapabilityError::new(
            PathCapabilityErrorCode::UnsupportedPlatform,
            "read final path from pinned descriptor",
            None,
        ))
    }
}

fn directory_access(handle: &OwnedFd) -> Result<DirectoryAccess, PathCapabilityError> {
    let metadata = rfs::fstat(handle)
        .map_err(|error| errno_error("inspect directory owner and mode", error))?;
    let owner_matches = metadata.st_uid == process::geteuid().as_raw();
    let owner_only_mode = metadata.st_mode & 0o077 == 0;
    Ok(if owner_matches && owner_only_mode {
        DirectoryAccess::OwnerOnly
    } else {
        DirectoryAccess::Broad
    })
}

#[cfg(target_os = "linux")]
fn mount_locality(
    canonical_path: &Path,
    handle: &OwnedFd,
) -> Result<StorageLocality, PathCapabilityError> {
    let metadata = rfs::fstat(handle)
        .map_err(|error| errno_error("inspect mounted filesystem identity", error))?;
    let expected_device = format!(
        "{}:{}",
        rfs::major(metadata.st_dev),
        rfs::minor(metadata.st_dev)
    );
    let bytes = fs::read("/proc/self/mountinfo").map_err(|error| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "read process mount table",
            error.raw_os_error().map(i64::from),
        )
    })?;
    let mut best: Option<(usize, String)> = None;
    for line in bytes.split(|byte| *byte == b'\n') {
        let Some(separator) = line.windows(3).position(|window| window == b" - ") else {
            continue;
        };
        let before: Vec<&[u8]> = line[..separator]
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect();
        let after: Vec<&[u8]> = line[separator + 3..]
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect();
        if before.len() < 5 || after.is_empty() || before[2] != expected_device.as_bytes() {
            continue;
        }
        let mount_point = PathBuf::from(OsString::from_vec(decode_mount_field(before[4])));
        if !canonical_path.starts_with(&mount_point) {
            continue;
        }
        let specificity = mount_point.components().count();
        let filesystem_type = String::from_utf8_lossy(after[0]).into_owned();
        if best
            .as_ref()
            .is_none_or(|(current, _)| specificity >= *current)
        {
            best = Some((specificity, filesystem_type));
        }
    }
    Ok(best.map_or(
        StorageLocality::Unknown(LocalityUnknown::NoCoveringMount),
        |(_, filesystem_type)| classify_unix_filesystem_type(&filesystem_type),
    ))
}

#[cfg(not(target_os = "linux"))]
fn mount_locality(
    _canonical_path: &Path,
    _handle: &OwnedFd,
) -> Result<StorageLocality, PathCapabilityError> {
    Ok(StorageLocality::Unknown(
        LocalityUnknown::UnsupportedPlatform,
    ))
}

fn decode_mount_field(value: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(value.len());
    let mut index = 0;
    while index < value.len() {
        if value[index] == b'\\' && index + 3 < value.len() {
            let octal = &value[index + 1..index + 4];
            if octal.iter().all(|byte| matches!(byte, b'0'..=b'7')) {
                output.push((octal[0] - b'0') * 64 + (octal[1] - b'0') * 8 + (octal[2] - b'0'));
                index += 4;
                continue;
            }
        }
        output.push(value[index]);
        index += 1;
    }
    output
}

fn map_open_error(operation: &'static str, error: Errno) -> PathCapabilityError {
    let code = if error == Errno::LOOP {
        PathCapabilityErrorCode::LinkOrReparsePoint
    } else if error == Errno::NOTDIR {
        PathCapabilityErrorCode::NotDirectoryAncestor
    } else {
        PathCapabilityErrorCode::OperatingSystem
    };
    PathCapabilityError::new(code, operation, Some(i64::from(error.raw_os_error())))
}

fn errno_error(operation: &'static str, error: Errno) -> PathCapabilityError {
    PathCapabilityError::new(
        PathCapabilityErrorCode::OperatingSystem,
        operation,
        Some(i64::from(error.raw_os_error())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_field_decoder_handles_kernel_octal_escapes() {
        assert_eq!(decode_mount_field(br"/tmp/a\040b\134c"), br"/tmp/a b\c");
    }
}
