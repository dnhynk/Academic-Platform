//! Reviewed Windows path and access-control implementation.
//!
//! Every unsafe block is confined to a small private FFI function with a
//! concrete invariant. No raw handle, SID, ACL, or security descriptor crosses
//! this module boundary.

use std::{
    ffi::{OsStr, OsString, c_void},
    mem::{MaybeUninit, offset_of, size_of},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};

use windows_sys::Wdk::{
    Foundation::OBJECT_ATTRIBUTES,
    Storage::FileSystem::{
        FILE_CREATE, FILE_DIRECTORY_FILE, FILE_DIRECTORY_INFORMATION, FILE_OPEN_REPARSE_POINT,
        FILE_SYNCHRONOUS_IO_NONALERT, FileDirectoryInformation, FileFsDeviceInformation,
        NtCreateFile, NtQueryDirectoryFile, NtQueryVolumeInformationFile,
    },
    System::SystemServices::{FILE_FS_DEVICE_INFORMATION, FILE_REMOTE_DEVICE},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS,
        GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, LocalFree,
        OBJ_CASE_INSENSITIVE, STATUS_NO_MORE_FILES, STATUS_NO_SUCH_FILE,
        STATUS_OBJECT_NAME_COLLISION, UNICODE_STRING,
    },
    Security::{
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL_SIZE_INFORMATION, AclSizeInformation,
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetSecurityInfo, SDDL_REVISION_1, SE_FILE_OBJECT,
        },
        CreateWellKnownSid, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetAclInformation,
        GetLengthSid, GetSecurityDescriptorControl, GetTokenInformation, INHERITED_ACE, IsValidSid,
        OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, TOKEN_QUERY,
        TOKEN_USER, TokenUser, WinLocalSystemSid,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_ID_INFO, FILE_LIST_DIRECTORY, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileAttributeTagInfo, FileIdInfo,
        GetDriveTypeW, GetFileInformationByHandleEx, GetFinalPathNameByHandleW, GetVolumePathNameW,
        OPEN_EXISTING, READ_CONTROL, VOLUME_NAME_DOS,
    },
    System::{
        IO::IO_STATUS_BLOCK,
        SystemServices::ACCESS_ALLOWED_ACE_TYPE,
        Threading::{GetCurrentProcess, OpenProcessToken},
        WindowsProgramming::{
            DRIVE_CDROM, DRIVE_FIXED, DRIVE_NO_ROOT_DIR, DRIVE_RAMDISK, DRIVE_REMOTE,
            DRIVE_REMOVABLE,
        },
    },
};

use crate::{
    DirectoryAccess, FinalPathStatus, PathCapabilities, PathCapabilityError,
    PathCapabilityErrorCode, RemoteDeviceFact, RootState, StorageLocality, WindowsDriveKind,
    classify_windows_storage,
};

const FILE_DEVICE_NETWORK_FILE_SYSTEM: u32 = 0x0000_0014;

#[derive(Debug)]
struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: `OwnedHandle` is constructed only from one successful Win32
        // handle-returning call, is never cloned, and closes that handle once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[derive(Debug)]
struct LocalAllocation(*mut c_void);

impl LocalAllocation {
    fn as_security_descriptor(&self) -> PSECURITY_DESCRIPTOR {
        self.0
    }
}

impl Drop for LocalAllocation {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: the pointer was returned by a documented LocalAlloc-backed
        // Windows conversion/security API and remains owned by this wrapper.
        unsafe {
            if !self.0.is_null() {
                let _ = LocalFree(self.0);
            }
        }
    }
}

#[derive(Debug)]
struct UserSid {
    _storage: Vec<usize>,
    sid: PSID,
}

#[derive(Debug)]
struct SystemSid {
    _storage: Vec<usize>,
    sid: PSID,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileIdentity {
    volume_serial: u64,
    file_id: [u8; 16],
}

#[derive(Debug)]
struct OpenedComponent {
    handle: OwnedHandle,
    identity: FileIdentity,
    final_path: PathBuf,
    attributes: u32,
}

#[derive(Debug)]
struct Walk {
    components: Vec<OpenedComponent>,
    root_state: RootState,
    missing_components: usize,
}

pub(super) fn inspect_path(path: &Path) -> Result<PathCapabilities, PathCapabilityError> {
    let first = walk_once(path)?;
    recheck_reparse_state(&first)?;
    let second = walk_once(path)?;
    if !same_walk_identity(&first, &second) {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::IdentityChanged,
            "recheck Windows path identity",
            None,
        ));
    }
    capabilities_from_walk(first)
}

pub(super) fn create_owner_only_directory(
    path: &Path,
) -> Result<PathCapabilities, PathCapabilityError> {
    let before = walk_once(path)?;
    recheck_reparse_state(&before)?;
    let confirmation = walk_once(path)?;
    if !same_walk_identity(&before, &confirmation) {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::IdentityChanged,
            "recheck protected Windows parent identity",
            None,
        ));
    }
    if before.root_state != RootState::Missing {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::AlreadyExists,
            "create protected Windows directory",
            None,
        ));
    }
    if before.missing_components != 1 {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::ParentMissing,
            "create protected Windows directory",
            None,
        ));
    }
    let parent = before.components.last().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::ParentMissing,
            "retain protected Windows parent handle",
            None,
        )
    })?;
    let drive_kind = drive_kind(&parent.final_path)?;
    match classify_windows_storage(drive_kind, remote_device_fact(&parent.handle)) {
        StorageLocality::Local => {}
        StorageLocality::Remote => {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::RemoteStorage,
                "create protected Windows directory",
                None,
            ));
        }
        StorageLocality::Unknown(_) => {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::UnknownStorage,
                "create protected Windows directory",
                None,
            ));
        }
    }

    let user_sid = current_user_sid()?;
    let descriptor = protected_security_descriptor(&user_sid)?;
    let name = path.file_name().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::ParentMissing,
            "resolve protected Windows directory name",
            None,
        )
    })?;
    let created = create_directory_relative(&parent.handle, name, &descriptor)?;
    if created.attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
        != FILE_ATTRIBUTE_DIRECTORY
        || directory_is_empty(&created.handle)? != RootState::EmptyDirectory
        || verify_directory_dacl(&created.handle)? != DirectoryAccess::OwnerOnly
    {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::BroadAccess,
            "verify protected Windows directory handle after creation",
            None,
        ));
    }

    let after = walk_once(path)?;
    recheck_reparse_state(&after)?;
    let after_confirmation = walk_once(path)?;
    if !same_walk_identity(&after, &after_confirmation)
        || after
            .components
            .last()
            .is_none_or(|component| component.identity != created.identity)
    {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::IdentityChanged,
            "verify created Windows directory name and handle identity",
            None,
        ));
    }
    let facts = capabilities_from_walk(after)?;
    if facts.root_state != RootState::EmptyDirectory
        || facts.access != DirectoryAccess::OwnerOnly
        || !matches!(facts.final_path, FinalPathStatus::Verified(_))
    {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::BroadAccess,
            "verify protected Windows directory after creation",
            None,
        ));
    }
    Ok(facts)
}

fn walk_once(path: &Path) -> Result<Walk, PathCapabilityError> {
    let mut paths: Vec<PathBuf> = path.ancestors().map(Path::to_path_buf).collect();
    paths.reverse();
    let mut components = Vec::with_capacity(paths.len());
    for (index, component_path) in paths.iter().enumerate() {
        let opened = match open_existing_component(component_path) {
            Ok(opened) => opened,
            Err(error)
                if error.os_code == Some(i64::from(ERROR_FILE_NOT_FOUND))
                    || error.os_code == Some(i64::from(ERROR_PATH_NOT_FOUND)) =>
            {
                return Ok(Walk {
                    components,
                    root_state: RootState::Missing,
                    missing_components: paths.len() - index,
                });
            }
            Err(error) => return Err(error),
        };
        if opened.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::LinkOrReparsePoint,
                "inspect Windows path component",
                None,
            ));
        }
        let is_last = index + 1 == paths.len();
        if opened.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 && !is_last {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::NotDirectoryAncestor,
                "walk Windows path components",
                None,
            ));
        }
        components.push(opened);
    }

    let final_component = components.last().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "retain Windows path root",
            None,
        )
    })?;
    let root_state = if final_component.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        RootState::NotDirectory
    } else {
        directory_is_empty(&final_component.handle)?
    };
    Ok(Walk {
        components,
        root_state,
        missing_components: 0,
    })
}

fn capabilities_from_walk(walk: Walk) -> Result<PathCapabilities, PathCapabilityError> {
    let nearest = walk.components.last().ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "retain nearest existing Windows path",
            None,
        )
    })?;
    let drive_kind = drive_kind(&nearest.final_path)?;
    let remote_device = remote_device_fact(&nearest.handle);
    let storage_locality = classify_windows_storage(drive_kind, remote_device);
    let access = match walk.root_state {
        RootState::Missing => DirectoryAccess::RequiresProtectedCreation,
        RootState::EmptyDirectory | RootState::NonEmptyDirectory => {
            verify_directory_dacl(&nearest.handle)?
        }
        RootState::NotDirectory => DirectoryAccess::Unknown,
    };
    let final_path = match walk.root_state {
        RootState::Missing => FinalPathStatus::Missing,
        RootState::EmptyDirectory | RootState::NonEmptyDirectory | RootState::NotDirectory => {
            FinalPathStatus::Verified(nearest.final_path.clone())
        }
    };
    Ok(PathCapabilities {
        canonical_existing_ancestor: nearest.final_path.clone(),
        root_state: walk.root_state,
        storage_locality,
        access,
        final_path,
    })
}

fn same_walk_identity(left: &Walk, right: &Walk) -> bool {
    left.root_state == right.root_state
        && left.missing_components == right.missing_components
        && left.components.len() == right.components.len()
        && left
            .components
            .iter()
            .zip(&right.components)
            .all(|(left, right)| {
                left.identity == right.identity
                    && paths_equal_case_insensitive(&left.final_path, &right.final_path)
            })
}

fn recheck_reparse_state(walk: &Walk) -> Result<(), PathCapabilityError> {
    for component in &walk.components {
        if file_attributes(&component.handle)? & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::LinkOrReparsePoint,
                "recheck Windows reparse component",
                None,
            ));
        }
    }
    Ok(())
}

#[allow(unsafe_code)]
fn directory_is_empty(handle: &OwnedHandle) -> Result<RootState, PathCapabilityError> {
    const BUFFER_BYTES: usize = 4_096;
    let mut restart_scan = true;
    loop {
        let mut storage = vec![0_usize; BUFFER_BYTES.div_ceil(size_of::<usize>())];
        let mut status_block = MaybeUninit::<IO_STATUS_BLOCK>::zeroed();
        // SAFETY: the synchronous directory handle has FILE_LIST_DIRECTORY;
        // the aligned buffer and status block are writable for their declared
        // sizes, and all optional async/filter pointers are null.
        let status = unsafe {
            NtQueryDirectoryFile(
                handle.raw(),
                null_mut(),
                None,
                null(),
                status_block.as_mut_ptr(),
                storage.as_mut_ptr().cast(),
                u32::try_from(BUFFER_BYTES).unwrap_or(0),
                FileDirectoryInformation,
                true,
                null(),
                restart_scan,
            )
        };
        if status == STATUS_NO_MORE_FILES || status == STATUS_NO_SUCH_FILE {
            return Ok(RootState::EmptyDirectory);
        }
        if status < 0 {
            return Err(ntstatus_error("enumerate pinned Windows directory", status));
        }
        // SAFETY: synchronous NtQueryDirectoryFile success initializes the
        // complete IO_STATUS_BLOCK supplied for this exact operation.
        let written = unsafe { status_block.assume_init() }.Information;
        let name_offset = offset_of!(FILE_DIRECTORY_INFORMATION, FileName);
        if written < name_offset || written > BUFFER_BYTES {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "validate Windows directory query bounds",
                None,
            ));
        }
        // SAFETY: a successful single-entry query initialized at least one
        // FILE_DIRECTORY_INFORMATION at the suitably aligned buffer start,
        // and IO_STATUS_BLOCK proved the fixed fields fit.
        let information = unsafe { &*storage.as_ptr().cast::<FILE_DIRECTORY_INFORMATION>() };
        let name_bytes = usize::try_from(information.FileNameLength).map_err(|_| {
            PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "validate Windows directory entry length",
                None,
            )
        })?;
        if name_bytes % size_of::<u16>() != 0 || name_bytes > written.saturating_sub(name_offset) {
            return Err(PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "validate Windows directory entry bounds",
                None,
            ));
        }
        // SAFETY: the kernel-reported UTF-16 byte length was checked against
        // the initialized query buffer before constructing this slice.
        let name = unsafe {
            std::slice::from_raw_parts(information.FileName.as_ptr(), name_bytes / size_of::<u16>())
        };
        if name != [u16::from(b'.')] && name != [u16::from(b'.'), u16::from(b'.')] {
            return Ok(RootState::NonEmptyDirectory);
        }
        restart_scan = false;
    }
}

#[allow(unsafe_code)]
fn open_existing_component(path: &Path) -> Result<OpenedComponent, PathCapabilityError> {
    let wide = wide_path(path)?;
    // SAFETY: `wide` is NUL-terminated and lives through the call; all output
    // pointers are null; the returned handle is checked before ownership.
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(last_error("open non-inheritable Windows path handle"));
    }
    let handle = OwnedHandle(raw);
    verify_non_inheritable(&handle)?;
    let attributes = file_attributes(&handle)?;
    let identity = file_identity(&handle)?;
    let final_path = final_path(&handle)?;
    Ok(OpenedComponent {
        handle,
        identity,
        final_path,
        attributes,
    })
}

#[allow(unsafe_code)]
fn verify_non_inheritable(handle: &OwnedHandle) -> Result<(), PathCapabilityError> {
    let mut flags = 0;
    // SAFETY: `handle` is live and `flags` is a valid writable `u32`.
    let flag_result = unsafe { GetHandleInformation(handle.raw(), &mut flags) };
    if flag_result == 0 {
        return Err(last_error("inspect Windows handle inheritance"));
    }
    if flags & HANDLE_FLAG_INHERIT != 0 {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "reject inheritable Windows path handle",
            None,
        ));
    }
    Ok(())
}

#[allow(unsafe_code)]
fn file_attributes(handle: &OwnedHandle) -> Result<u32, PathCapabilityError> {
    let mut information = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: the handle is live and the output buffer has the exact structure
    // size required by `FileAttributeTagInfo`.
    let result = unsafe {
        GetFileInformationByHandleEx(
            handle.raw(),
            FileAttributeTagInfo,
            (&raw mut information).cast(),
            u32::try_from(size_of::<FILE_ATTRIBUTE_TAG_INFO>()).map_err(|_| {
                PathCapabilityError::new(
                    PathCapabilityErrorCode::OperatingSystem,
                    "size Windows attribute buffer",
                    None,
                )
            })?,
        )
    };
    if result == 0 {
        Err(last_error("inspect Windows reparse attributes"))
    } else {
        Ok(information.FileAttributes)
    }
}

#[allow(unsafe_code)]
fn file_identity(handle: &OwnedHandle) -> Result<FileIdentity, PathCapabilityError> {
    let mut information = FILE_ID_INFO::default();
    // SAFETY: the handle is live and the output buffer has the exact structure
    // size required by FileIdInfo's 128-bit stable file identity query.
    let result = unsafe {
        GetFileInformationByHandleEx(
            handle.raw(),
            FileIdInfo,
            (&raw mut information).cast(),
            u32::try_from(size_of::<FILE_ID_INFO>()).map_err(|_| {
                PathCapabilityError::new(
                    PathCapabilityErrorCode::OperatingSystem,
                    "size Windows file identity buffer",
                    None,
                )
            })?,
        )
    };
    if result == 0 {
        return Err(last_error("inspect Windows file identity"));
    }
    Ok(FileIdentity {
        volume_serial: information.VolumeSerialNumber,
        file_id: information.FileId.Identifier,
    })
}

#[allow(unsafe_code)]
fn final_path(handle: &OwnedHandle) -> Result<PathBuf, PathCapabilityError> {
    // SAFETY: the handle is live; a null buffer with length zero is the
    // documented size query and has no writable memory requirement.
    let required = unsafe {
        GetFinalPathNameByHandleW(
            handle.raw(),
            null_mut(),
            0,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    };
    if required == 0 {
        return Err(last_error("size final Windows handle path"));
    }
    let capacity = usize::try_from(required).map_err(|_| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size final Windows handle path",
            None,
        )
    })?;
    let mut buffer = vec![0_u16; capacity];
    // SAFETY: `buffer` is writable for `required` UTF-16 units and the handle
    // stays live for the complete query.
    let written = unsafe {
        GetFinalPathNameByHandleW(
            handle.raw(),
            buffer.as_mut_ptr(),
            required,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    };
    if written == 0 || written >= required {
        return Err(last_error("read final Windows handle path"));
    }
    buffer.truncate(usize::try_from(written).map_err(|_| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "decode final Windows handle path",
            None,
        )
    })?);
    Ok(normalize_final_path(PathBuf::from(OsString::from_wide(
        &buffer,
    ))))
}

#[allow(unsafe_code)]
fn drive_kind(path: &Path) -> Result<WindowsDriveKind, PathCapabilityError> {
    let wide = wide_path(path)?;
    let mut volume_path = vec![0_u16; 32_768];
    let length = u32::try_from(volume_path.len()).map_err(|_| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size Windows volume path buffer",
            None,
        )
    })?;
    // SAFETY: both UTF-16 buffers are valid and the output capacity is passed
    // exactly; `wide` remains alive throughout the call.
    let result = unsafe { GetVolumePathNameW(wide.as_ptr(), volume_path.as_mut_ptr(), length) };
    if result == 0 {
        return Ok(WindowsDriveKind::Unknown);
    }
    // SAFETY: `GetVolumePathNameW` wrote a NUL-terminated path within the
    // supplied buffer, which remains alive through `GetDriveTypeW`.
    let drive = unsafe { GetDriveTypeW(volume_path.as_ptr()) };
    Ok(match drive {
        DRIVE_FIXED => WindowsDriveKind::Fixed,
        DRIVE_REMOTE => WindowsDriveKind::Remote,
        DRIVE_REMOVABLE => WindowsDriveKind::Removable,
        DRIVE_CDROM => WindowsDriveKind::Optical,
        DRIVE_RAMDISK => WindowsDriveKind::RamDisk,
        DRIVE_NO_ROOT_DIR => WindowsDriveKind::NoRoot,
        _ => WindowsDriveKind::Unknown,
    })
}

#[allow(unsafe_code)]
fn remote_device_fact(handle: &OwnedHandle) -> RemoteDeviceFact {
    let mut status_block = MaybeUninit::<IO_STATUS_BLOCK>::zeroed();
    let mut information = FILE_FS_DEVICE_INFORMATION::default();
    // SAFETY: the live directory handle is valid for a volume information
    // query and both output buffers have the exact documented layout/size.
    let status = unsafe {
        NtQueryVolumeInformationFile(
            handle.raw(),
            status_block.as_mut_ptr(),
            (&raw mut information).cast(),
            u32::try_from(size_of::<FILE_FS_DEVICE_INFORMATION>()).unwrap_or(0),
            FileFsDeviceInformation,
        )
    };
    if status < 0 {
        RemoteDeviceFact::Unknown
    } else if information.Characteristics & FILE_REMOTE_DEVICE != 0
        || information.DeviceType == FILE_DEVICE_NETWORK_FILE_SYSTEM
    {
        RemoteDeviceFact::Remote
    } else {
        RemoteDeviceFact::Local
    }
}

#[allow(unsafe_code)]
fn current_user_sid() -> Result<UserSid, PathCapabilityError> {
    let mut token = INVALID_HANDLE_VALUE;
    // SAFETY: the pseudo process handle is always valid for the current
    // process and `token` is a writable handle slot.
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if opened == 0 {
        return Err(last_error("open current process token"));
    }
    let token = OwnedHandle(token);
    let mut required = 0;
    // SAFETY: a null output with zero length is the documented sizing call;
    // `required` is a valid writable length slot and the token remains live.
    let _ = unsafe { GetTokenInformation(token.raw(), TokenUser, null_mut(), 0, &mut required) };
    if required == 0 {
        return Err(last_error("size current-user token information"));
    }
    let bytes = usize::try_from(required).map_err(|_| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size current-user token information",
            None,
        )
    })?;
    let words = bytes.div_ceil(size_of::<usize>());
    let mut storage = vec![0_usize; words];
    // SAFETY: `storage` is suitably aligned for `TOKEN_USER`, writable for at
    // least `required` bytes, and is not moved or resized during the call.
    let read = unsafe {
        GetTokenInformation(
            token.raw(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    };
    if read == 0 {
        return Err(last_error("read current-user token information"));
    }
    // SAFETY: the successful call initialized a `TOKEN_USER` at the aligned
    // buffer start, and its SID pointer remains backed by `storage`.
    let sid = unsafe { (*(storage.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    // SAFETY: the SID pointer is returned by the trusted token API and remains
    // within the live token-information allocation.
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "validate current-user SID",
            None,
        ));
    }
    Ok(UserSid {
        _storage: storage,
        sid,
    })
}

#[allow(unsafe_code)]
fn system_sid() -> Result<SystemSid, PathCapabilityError> {
    let mut required = 0;
    // SAFETY: a null SID buffer with zero length is the documented sizing call
    // and `required` is a valid writable length slot.
    let _ = unsafe { CreateWellKnownSid(WinLocalSystemSid, null_mut(), null_mut(), &mut required) };
    if required == 0 {
        return Err(last_error("size LocalSystem SID"));
    }
    let bytes = usize::try_from(required).map_err(|_| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size LocalSystem SID",
            None,
        )
    })?;
    let mut storage = vec![0_usize; bytes.div_ceil(size_of::<usize>())];
    let sid = storage.as_mut_ptr().cast();
    // SAFETY: `storage` is aligned and writable for at least `required` bytes;
    // the null domain SID requests the process-local well-known SID.
    let created = unsafe { CreateWellKnownSid(WinLocalSystemSid, null_mut(), sid, &mut required) };
    if created == 0 {
        return Err(last_error("create LocalSystem SID"));
    }
    Ok(SystemSid {
        _storage: storage,
        sid,
    })
}

#[allow(unsafe_code)]
fn sid_string(sid: PSID) -> Result<String, PathCapabilityError> {
    let mut raw = null_mut();
    // SAFETY: `sid` is a validated SID backed by a live allocation and `raw`
    // is a writable pointer slot for the LocalAlloc-backed string.
    let converted = unsafe { ConvertSidToStringSidW(sid, &mut raw) };
    if converted == 0 || raw.is_null() {
        return Err(last_error("convert current-user SID to SDDL"));
    }
    let allocation = LocalAllocation(raw.cast());
    let mut length = 0;
    // SAFETY: the conversion API returned a NUL-terminated UTF-16 string;
    // scanning stops at that terminator while the LocalAllocation is live.
    unsafe {
        while *raw.add(length) != 0 {
            length += 1;
        }
    }
    // SAFETY: `length` was bounded by the terminator in the live allocation.
    let value = unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(raw, length)) };
    drop(allocation);
    Ok(value)
}

#[allow(unsafe_code)]
fn protected_security_descriptor(user: &UserSid) -> Result<LocalAllocation, PathCapabilityError> {
    let user_text = sid_string(user.sid)?;
    let system = system_sid()?;
    // SAFETY: both SIDs are validated and backed by live allocations.
    let user_is_system = unsafe { EqualSid(user.sid, system.sid) } != 0;
    let sddl = if user_is_system {
        format!("O:{user_text}D:P(A;;FA;;;{user_text})")
    } else {
        // LocalSystem is the sole narrowly justified system trustee: it keeps
        // OS backup/recovery and security services operable without granting
        // Administrators, Users, Everyone, or inherited parent trustees.
        format!("O:{user_text}D:P(A;;FA;;;{user_text})(A;;FA;;;SY)")
    };
    let wide = wide_text(OsStr::new(&sddl))?;
    let mut descriptor = null_mut();
    // SAFETY: `wide` is a valid NUL-terminated SDDL string, `descriptor` is a
    // writable result slot, and the API reports a LocalAlloc-backed result.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 || descriptor.is_null() {
        return Err(last_error("build protected Windows security descriptor"));
    }
    Ok(LocalAllocation(descriptor))
}

#[allow(unsafe_code)]
fn create_directory_relative(
    parent: &OwnedHandle,
    name: &OsStr,
    descriptor: &LocalAllocation,
) -> Result<OpenedComponent, PathCapabilityError> {
    let mut wide = wide_text(name)?;
    let name_units = wide.len().checked_sub(1).ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size relative Windows directory name",
            None,
        )
    })?;
    let name_bytes = name_units.checked_mul(size_of::<u16>()).ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size relative Windows directory name",
            None,
        )
    })?;
    let maximum_bytes = wide.len().checked_mul(size_of::<u16>()).ok_or_else(|| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size relative Windows directory buffer",
            None,
        )
    })?;
    let unicode_name = UNICODE_STRING {
        Length: u16::try_from(name_bytes).map_err(|_| {
            PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "bound relative Windows directory name",
                None,
            )
        })?,
        MaximumLength: u16::try_from(maximum_bytes).map_err(|_| {
            PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "bound relative Windows directory buffer",
                None,
            )
        })?,
        Buffer: wide.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(size_of::<OBJECT_ATTRIBUTES>()).map_err(|_| {
            PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "size Windows object attributes",
                None,
            )
        })?,
        RootDirectory: parent.raw(),
        ObjectName: &raw const unicode_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: descriptor.as_security_descriptor().cast(),
        SecurityQualityOfService: null(),
    };
    let mut status_block = MaybeUninit::<IO_STATUS_BLOCK>::zeroed();
    let mut raw = INVALID_HANDLE_VALUE;
    // SAFETY: RootDirectory is a live, reparse-checked parent handle; the
    // relative UNICODE_STRING and self-relative protected descriptor remain
    // live for the call; FILE_CREATE prevents replacement, no OBJ_INHERIT is
    // set, and the returned handle slot/status block are writable.
    let status = unsafe {
        NtCreateFile(
            &mut raw,
            FILE_ALL_ACCESS,
            &attributes,
            status_block.as_mut_ptr(),
            null(),
            FILE_ATTRIBUTE_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            null(),
            0,
        )
    };
    if status == STATUS_OBJECT_NAME_COLLISION {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::AlreadyExists,
            "create protected Windows directory relative to pinned parent",
            Some(i64::from(status)),
        ));
    }
    if status < 0 || raw == INVALID_HANDLE_VALUE {
        return Err(ntstatus_error(
            "create protected Windows directory relative to pinned parent",
            status,
        ));
    }
    let handle = OwnedHandle(raw);
    verify_non_inheritable(&handle)?;
    let attributes = file_attributes(&handle)?;
    let identity = file_identity(&handle)?;
    let final_path = final_path(&handle)?;
    Ok(OpenedComponent {
        handle,
        identity,
        final_path,
        attributes,
    })
}

#[allow(unsafe_code)]
fn verify_directory_dacl(handle: &OwnedHandle) -> Result<DirectoryAccess, PathCapabilityError> {
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: the handle was opened with READ_CONTROL; all requested output
    // pointers are valid and the returned descriptor is LocalAlloc-backed.
    let result = unsafe {
        GetSecurityInfo(
            handle.raw(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if result != ERROR_SUCCESS || descriptor.is_null() {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "read Windows directory DACL",
            Some(i64::from(result)),
        ));
    }
    let allocation = LocalAllocation(descriptor);
    let user = current_user_sid()?;
    let system = system_sid()?;
    if owner.is_null() || dacl.is_null() {
        return Ok(DirectoryAccess::Broad);
    }
    // SAFETY: owner, user, and system are validated SIDs backed by live
    // security/token allocations.
    if unsafe { EqualSid(owner, user.sid) } == 0 {
        return Ok(DirectoryAccess::Broad);
    }
    let mut control = 0;
    let mut revision = 0;
    // SAFETY: the LocalAllocation keeps a valid security descriptor live and
    // both scalar outputs are writable for the documented call.
    if unsafe { GetSecurityDescriptorControl(allocation.0, &mut control, &mut revision) } == 0 {
        return Err(last_error("inspect Windows DACL protection"));
    }
    if control & SE_DACL_PROTECTED == 0 {
        return Ok(DirectoryAccess::Broad);
    }
    let mut size = ACL_SIZE_INFORMATION::default();
    let size_length = u32::try_from(size_of::<ACL_SIZE_INFORMATION>()).map_err(|_| {
        PathCapabilityError::new(
            PathCapabilityErrorCode::OperatingSystem,
            "size Windows ACL information",
            None,
        )
    })?;
    // SAFETY: `dacl` points inside the live descriptor and `size` is the exact
    // writable structure for `AclSizeInformation`.
    if unsafe {
        GetAclInformation(
            dacl,
            (&raw mut size).cast(),
            size_length,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(last_error("inspect Windows DACL entries"));
    }
    // SAFETY: both validated SID pointers remain live for this comparison.
    let user_is_system = unsafe { EqualSid(user.sid, system.sid) } != 0;
    let expected_aces = if user_is_system { 1 } else { 2 };
    if size.AceCount != expected_aces {
        return Ok(DirectoryAccess::Broad);
    }
    let mut saw_user = false;
    let mut saw_system = false;
    for index in 0..size.AceCount {
        let mut raw_ace = null_mut();
        // SAFETY: `dacl` is live and `index` is strictly less than the
        // OS-reported ACE count; `raw_ace` is a writable pointer slot.
        if unsafe { GetAce(dacl, index, &mut raw_ace) } == 0 || raw_ace.is_null() {
            return Err(last_error("read Windows DACL entry"));
        }
        // SAFETY: a successful GetAce returns at least an ACE_HEADER within
        // the live ACL allocation; no larger structure is referenced yet.
        let header = unsafe { &*raw_ace.cast::<ACE_HEADER>() };
        let sid_offset = offset_of!(ACCESS_ALLOWED_ACE, SidStart);
        let minimum_sid_bytes = 8;
        if header.AceType != ACCESS_ALLOWED_ACE_TYPE as u8
            || usize::from(header.AceSize) < sid_offset + minimum_sid_bytes
            || u32::from(header.AceFlags) & INHERITED_ACE != 0
        {
            return Ok(DirectoryAccess::Broad);
        }
        // SAFETY: AceSize proved that the live ACE allocation contains the
        // fixed ACCESS_ALLOWED_ACE fields and the minimum SID header.
        let ace = unsafe { &*raw_ace.cast::<ACCESS_ALLOWED_ACE>() };
        if ace.Mask != FILE_ALL_ACCESS {
            return Ok(DirectoryAccess::Broad);
        }
        let sid: PSID = (&raw const ace.SidStart).cast_mut().cast();
        // SAFETY: ACCESS_ALLOWED_ACE stores its SID inline at SidStart and the
        // ACL remains live for the complete comparison.
        if unsafe { IsValidSid(sid) } == 0 {
            return Ok(DirectoryAccess::Broad);
        }
        // SAFETY: IsValidSid accepted the inline SID; GetLengthSid reads that
        // validated header while the containing ACL allocation remains live.
        let sid_length = unsafe { GetLengthSid(sid) };
        let Some(sid_end) = sid_offset.checked_add(usize::try_from(sid_length).map_err(|_| {
            PathCapabilityError::new(
                PathCapabilityErrorCode::OperatingSystem,
                "bound Windows DACL SID length",
                None,
            )
        })?) else {
            return Ok(DirectoryAccess::Broad);
        };
        if sid_end > usize::from(header.AceSize) {
            return Ok(DirectoryAccess::Broad);
        }
        // SAFETY: all compared SIDs are validated and backed by live storage.
        if unsafe { EqualSid(sid, user.sid) } != 0 {
            if saw_user {
                return Ok(DirectoryAccess::Broad);
            }
            saw_user = true;
        } else if unsafe { EqualSid(sid, system.sid) } != 0 {
            if saw_system {
                return Ok(DirectoryAccess::Broad);
            }
            saw_system = true;
        } else {
            return Ok(DirectoryAccess::Broad);
        }
    }
    if saw_user && (user_is_system || saw_system) {
        Ok(DirectoryAccess::OwnerOnly)
    } else {
        Ok(DirectoryAccess::Broad)
    }
}

fn wide_path(path: &Path) -> Result<Vec<u16>, PathCapabilityError> {
    wide_text(path.as_os_str())
}

fn wide_text(value: &OsStr) -> Result<Vec<u16>, PathCapabilityError> {
    let mut wide: Vec<u16> = value.encode_wide().collect();
    if wide.contains(&0) {
        return Err(PathCapabilityError::new(
            PathCapabilityErrorCode::DevicePath,
            "encode Windows path",
            None,
        ));
    }
    wide.push(0);
    Ok(wide)
}

fn normalize_final_path(path: PathBuf) -> PathBuf {
    let value = path.as_os_str().to_string_lossy();
    if let Some(value) = value.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{value}"))
    } else if let Some(value) = value.strip_prefix(r"\\?\") {
        PathBuf::from(value)
    } else {
        path
    }
}

fn paths_equal_case_insensitive(left: &Path, right: &Path) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

fn last_error(operation: &'static str) -> PathCapabilityError {
    let error = std::io::Error::last_os_error();
    PathCapabilityError::new(
        PathCapabilityErrorCode::OperatingSystem,
        operation,
        error.raw_os_error().map(i64::from),
    )
}

fn ntstatus_error(operation: &'static str, status: i32) -> PathCapabilityError {
    PathCapabilityError::new(
        PathCapabilityErrorCode::OperatingSystem,
        operation,
        Some(i64::from(status)),
    )
}
