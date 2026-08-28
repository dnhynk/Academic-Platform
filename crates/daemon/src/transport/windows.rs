//! Windows named-pipe, session mutex, and current-user ACL implementation.

use std::{
    ffi::{OsStr, c_void},
    fs::{File, OpenOptions},
    io,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
    ptr,
};

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_BROKEN_PIPE, ERROR_LOCK_VIOLATION, ERROR_NO_DATA, ERROR_PIPE_BUSY,
        ERROR_SUCCESS, HANDLE, LocalFree,
    },
    Security::{
        ACCESS_ALLOWED_ACE, ACL,
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetNamedSecurityInfoW, GetSecurityInfo, SE_FILE_OBJECT, SE_KERNEL_OBJECT,
            SetNamedSecurityInfoW,
        },
        DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetSecurityDescriptorControl,
        GetSecurityDescriptorDacl, GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    },
    Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
        LockFileEx,
    },
    System::{
        IO::OVERLAPPED,
        RemoteDesktop::ProcessIdToSessionId,
        SystemServices::ACCESS_ALLOWED_ACE_TYPE,
        Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken},
    },
};

use super::{LocalEndpoint, RuntimePaths, SINGLETON_LOCK_FILE, profile_key};

#[derive(Debug)]
struct OwnedHandle(HANDLE);

// A Win32 kernel handle value may be closed from another process thread.
unsafe impl Send for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this type exclusively owns the non-null Win32 handle.
        unsafe { CloseHandle(self.0) };
    }
}

#[derive(Debug)]
pub(crate) struct SecurityDescriptor {
    descriptor: PSECURITY_DESCRIPTOR,
    sid: Vec<u8>,
}

// The descriptor is immutable after construction and remains alive around
// each synchronous Win32 creation call.
unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

impl SecurityDescriptor {
    pub(crate) fn current_user_only() -> io::Result<Self> {
        let sid = current_user_sid()?;
        let sid_text = sid_string(sid.as_ptr().cast_mut().cast())?;
        let sddl = wide(&format!("D:P(A;;GA;;;{sid_text})"));
        let mut descriptor = ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated and output ownership is transferred here.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { descriptor, sid })
    }

    fn attributes(&mut self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
            lpSecurityDescriptor: self.descriptor,
            bInheritHandle: 0,
        }
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: descriptor was allocated by LocalAlloc inside the conversion API.
        unsafe { LocalFree(self.descriptor.cast()) };
    }
}

#[derive(Debug)]
pub(crate) struct SingletonGuard {
    _lock: File,
}

impl SingletonGuard {
    /// Binds the singleton to the profile runtime directory instead of the
    /// logon session.
    ///
    /// A `Local\` named mutex cannot carry this guarantee: that namespace is
    /// per-logon-session, so two concurrent sessions of the same user — RDP
    /// beside the console, or fast user switching — each create their own,
    /// neither observes `ERROR_ALREADY_EXISTS`, and both acquire. An exclusive
    /// byte-range lock is taken on a file object instead: one profile resolves
    /// to one file from every session on the machine, the kernel releases it
    /// when the owning process dies so no lock is orphaned, and a lock file that
    /// cannot be opened or locked refuses startup, so the failure mode is
    /// closed. This is the same shape as the Unix `flock` guard.
    ///
    /// It guarantees at most one daemon per profile runtime directory on this
    /// machine, across logon sessions. It does not couple distinct profiles,
    /// which hash to distinct directories and stay independent by design, and it
    /// does not make one daemon reachable from another logon session, because
    /// the named-pipe endpoint still carries the session id.
    pub(crate) fn acquire(paths: &RuntimePaths) -> io::Result<Self> {
        // The containing directory already carries a protected current-user-only
        // DACL, so no other principal can reach this file. The reparse-point flag
        // is the Windows analogue of `O_NOFOLLOW`: a planted link is opened as
        // itself instead of redirecting the lock to another file.
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(paths.directory.join(SINGLETON_LOCK_FILE))?;
        let handle = lock.as_raw_handle() as HANDLE;
        let mut overlapped = OVERLAPPED::default();
        // SAFETY: the handle is owned by `lock` and `overlapped` outlives the call.
        let locked = unsafe {
            LockFileEx(
                handle,
                LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        };
        if locked == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == i32::try_from(ERROR_LOCK_VIOLATION).ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "profile daemon already running",
                ));
            }
            return Err(error);
        }
        Ok(Self { _lock: lock })
    }
}

#[derive(Debug)]
pub(crate) struct LocalListener {
    name: String,
    next: Option<NamedPipeServer>,
    security: SecurityDescriptor,
}

impl LocalListener {
    pub(crate) fn bind(paths: &RuntimePaths) -> io::Result<Self> {
        let name = match &paths.endpoint {
            LocalEndpoint::NamedPipe(name) => name.clone(),
            LocalEndpoint::UnixSocket(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not a Windows endpoint",
                ));
            }
        };
        let mut security = SecurityDescriptor::current_user_only()?;
        let next = create_pipe(&name, &mut security, true)?;
        verify_pipe_acl(&next, &security.sid)?;
        Ok(Self {
            name,
            next: Some(next),
            security,
        })
    }

    pub(crate) async fn accept(&mut self) -> io::Result<NamedPipeServer> {
        let pending = match self.next.take() {
            Some(pending) => pending,
            // The previous accept could not pre-create the replacement because
            // the instance ceiling was momentarily full. Re-creating it here
            // means one transient failure never costs the endpoint permanently.
            None => create_pipe(&self.name, &mut self.security, false)?,
        };
        pending.connect().await?;
        // Pre-create the next instance so another client can connect while this
        // one is served. Failing here is transient and must not drop the client
        // that already connected: the instance is re-created on the next accept.
        self.next = create_pipe(&self.name, &mut self.security, false).ok();
        Ok(pending)
    }
}

/// Accept errors that describe one connection or a momentarily full instance
/// ceiling rather than a dead endpoint.
///
/// `ERROR_PIPE_BUSY` is reported when every instance of the pipe is in use,
/// which is exactly the state a client that connects and holds produces. Ending
/// the listener for it removes the pending instance and the session metadata
/// while the process stays alive, so every later client — including legitimate
/// writers — silently fails to connect.
pub(crate) fn accept_error_is_transient(error: &io::Error) -> bool {
    let code = error.raw_os_error();
    [ERROR_PIPE_BUSY, ERROR_NO_DATA, ERROR_BROKEN_PIPE]
        .into_iter()
        .any(|transient| code == i32::try_from(transient).ok())
}

pub(crate) fn prepare_runtime(
    runtime_root: &Path,
    profile_root: &Path,
) -> io::Result<RuntimePaths> {
    let metadata = std::fs::symlink_metadata(runtime_root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime root is not a plain directory",
        ));
    }
    let product = runtime_root.join("academic-os");
    ensure_private_directory(&product)?;
    let key = profile_key(profile_root)?;
    let directory = product.join(&key);
    ensure_private_directory(&directory)?;
    let mut session = 0_u32;
    // SAFETY: output points at an initialized u32 for the current process ID.
    if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(RuntimePaths {
        profile_key: key.clone(),
        metadata: directory.join("session.meta"),
        endpoint: LocalEndpoint::NamedPipe(format!(r"\\.\pipe\academic-os\{session}\{key}")),
        directory,
    })
}

pub(crate) fn cleanup_endpoint(_paths: &RuntimePaths) {}

pub(crate) fn secure_metadata(path: &Path) -> io::Result<()> {
    secure_path(path)
}

fn ensure_private_directory(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "runtime directory is a link or non-directory",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            std::fs::create_dir(path)?;
        }
        Err(error) => return Err(error),
    }
    secure_path(path)
}

fn create_pipe(
    name: &str,
    security: &mut SecurityDescriptor,
    first: bool,
) -> io::Result<NamedPipeServer> {
    let mut attributes = security.attributes();
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .max_instances(64);
    // SAFETY: attributes and its descriptor remain valid for this synchronous creation call.
    unsafe {
        options.create_with_security_attributes_raw(name, ptr::addr_of_mut!(attributes).cast())
    }
}

fn verify_pipe_acl(server: &NamedPipeServer, expected_sid: &[u8]) -> io::Result<()> {
    let handle = server.as_raw_handle() as HANDLE;
    let mut dacl: *mut ACL = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: handle is a live pipe server; output pointers remain valid until LocalFree.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(status).unwrap_or(i32::MAX),
        ));
    }
    let result = verify_descriptor(descriptor, dacl, expected_sid);
    // SAFETY: descriptor was allocated by the security API.
    unsafe { LocalFree(descriptor.cast()) };
    result
}

fn secure_path(path: &Path) -> io::Result<()> {
    let security = SecurityDescriptor::current_user_only()?;
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl: *mut ACL = ptr::null_mut();
    // SAFETY: the descriptor is live and the output pointers are initialized.
    if unsafe {
        GetSecurityDescriptorDacl(security.descriptor, &mut present, &mut dacl, &mut defaulted)
    } == 0
        || present == 0
        || dacl.is_null()
    {
        return Err(io::Error::last_os_error());
    }
    let path_wide = wide_os(path.as_os_str());
    // SAFETY: path and DACL remain valid for this synchronous call.
    let status = unsafe {
        SetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            dacl,
            ptr::null(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(status).unwrap_or(i32::MAX),
        ));
    }
    verify_named_path(path, &security.sid)
}

fn verify_named_path(path: &Path, expected_sid: &[u8]) -> io::Result<()> {
    let path_wide = wide_os(path.as_os_str());
    let mut dacl: *mut ACL = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: path is NUL-terminated and output pointers are valid.
    let status = unsafe {
        GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(status).unwrap_or(i32::MAX),
        ));
    }
    let result = verify_descriptor(descriptor, dacl, expected_sid);
    // SAFETY: descriptor was allocated by the security API.
    unsafe { LocalFree(descriptor.cast()) };
    result
}

fn verify_descriptor(
    descriptor: PSECURITY_DESCRIPTOR,
    dacl: *mut ACL,
    expected_sid: &[u8],
) -> io::Result<()> {
    if descriptor.is_null() || dacl.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "pipe has no DACL",
        ));
    }
    let mut control = 0_u16;
    let mut revision = 0_u32;
    // SAFETY: descriptor is a live self-relative security descriptor.
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
        || control & SE_DACL_PROTECTED == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "pipe DACL is not protected",
        ));
    }
    // SAFETY: dacl was returned from GetSecurityInfo and is live.
    let ace_count = unsafe { (*dacl).AceCount };
    if ace_count != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "pipe DACL is not current-user-only",
        ));
    }
    let mut ace: *mut c_void = ptr::null_mut();
    // SAFETY: index zero exists because AceCount is exactly one.
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 || ace.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: SDDL created an access-allowed ACE and the header is readable.
    let allowed = unsafe { &*(ace.cast::<ACCESS_ALLOWED_ACE>()) };
    if u32::from(allowed.Header.AceType) != ACCESS_ALLOWED_ACE_TYPE {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "pipe ACE is not allow-only",
        ));
    }
    let actual_sid = ptr::addr_of!(allowed.SidStart).cast_mut().cast();
    // SAFETY: both SID buffers are validated outputs from Windows security APIs.
    if unsafe { EqualSid(actual_sid, expected_sid.as_ptr().cast_mut().cast()) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "pipe ACE SID mismatch",
        ));
    }
    Ok(())
}

fn current_user_sid() -> io::Result<Vec<u8>> {
    let mut token = ptr::null_mut();
    // SAFETY: output receives a process token handle on success.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = OwnedHandle(token);
    let mut length = 0_u32;
    // SAFETY: null buffer is the documented size query.
    let _ignored =
        unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut length) };
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0_u8; usize::try_from(length).unwrap_or(usize::MAX)];
    // SAFETY: buffer has the exact size reported by the preceding call.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful TokenUser output begins with TOKEN_USER.
    let token_user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
    sid_bytes(token_user.User.Sid)
}

fn sid_bytes(sid: PSID) -> io::Result<Vec<u8>> {
    use windows_sys::Win32::Security::{GetLengthSid, IsValidSid};
    // SAFETY: SID is borrowed from a successful TokenUser query.
    if unsafe { IsValidSid(sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "current token SID is invalid",
        ));
    }
    // SAFETY: validated SID has a readable length.
    let length = unsafe { GetLengthSid(sid) };
    // SAFETY: SID contains `length` bytes by contract.
    Ok(unsafe {
        std::slice::from_raw_parts(
            sid.cast::<u8>(),
            usize::try_from(length).unwrap_or(usize::MAX),
        )
    }
    .to_vec())
}

fn sid_string(sid: PSID) -> io::Result<String> {
    let mut value = ptr::null_mut();
    // SAFETY: SID was validated; output is LocalAlloc-owned UTF-16.
    if unsafe { ConvertSidToStringSidW(sid, &mut value) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut length = 0_usize;
    // SAFETY: output is NUL-terminated by the API.
    while unsafe { *value.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: the preceding loop found the terminator.
    let text = String::from_utf16(unsafe { std::slice::from_raw_parts(value, length) })
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "SID text is not UTF-16"));
    // SAFETY: value was allocated by LocalAlloc.
    unsafe { LocalFree(value.cast()) };
    text
}

fn wide(value: &str) -> Vec<u16> {
    wide_os(OsStr::new(value))
}

fn wide_os(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
