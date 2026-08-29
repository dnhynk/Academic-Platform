//! Unix-domain socket, peer-credential, and file-lock implementation.

use std::{
    fs::{self, Permissions},
    io,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::Path,
};

use rustix::{
    fd::OwnedFd,
    fs::{self as rfs, FlockOperation, Mode, OFlags},
    process::{Uid, getuid},
};
use tokio::net::{UnixListener, UnixStream};

use super::{
    LocalEndpoint, PRODUCT_RUNTIME_DIR, RuntimeLayoutError, RuntimePaths, SINGLETON_LOCK_FILE,
    profile_key,
};

/// Bytes of `sockaddr_un::sun_path`, which has to hold the whole absolute
/// endpoint path and its terminating NUL. Linux and Android carry 108; macOS
/// and the BSDs carry 104.
#[cfg(any(target_os = "linux", target_os = "android"))]
const SUN_PATH_CAPACITY: usize = 108;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
const SUN_PATH_CAPACITY: usize = 104;

/// Endpoint file below the per-profile runtime directory. Every byte of it is
/// charged against [`MAX_UNIX_ENDPOINT_PATH_LEN`].
const ENDPOINT_FILE: &str = "d.sock";

/// Longest Unix endpoint path this platform can address.
///
/// The endpoint is the caller's runtime root followed by the fixed
/// `academic-os/<profile key>/d.sock` suffix, and the whole absolute path has
/// to fit `sun_path`. A runtime root that leaves no room for that suffix is
/// refused by [`prepare_runtime`] with the measured length and the offending
/// path, because the alternative is an unexplained `InvalidInput` from `bind`.
pub const MAX_UNIX_ENDPOINT_PATH_LEN: usize = SUN_PATH_CAPACITY - 1;

#[derive(Debug)]
pub(crate) struct SingletonGuard {
    _lock: OwnedFd,
}

impl SingletonGuard {
    pub(crate) fn acquire(paths: &RuntimePaths) -> io::Result<Self> {
        let directory = rfs::open(
            &paths.directory,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let lock = rfs::openat(
            &directory,
            SINGLETON_LOCK_FILE,
            OFlags::CREATE | OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?;
        rfs::fchmod(&lock, Mode::RUSR | Mode::WUSR)?;
        rfs::flock(&lock, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
            if error == rustix::io::Errno::WOULDBLOCK {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "profile daemon already running",
                )
            } else {
                io::Error::from_raw_os_error(error.raw_os_error())
            }
        })?;
        Ok(Self { _lock: lock })
    }
}

#[derive(Debug)]
pub(crate) struct LocalListener {
    listener: UnixListener,
    expected_uid: Uid,
}

impl LocalListener {
    pub(crate) fn bind(paths: &RuntimePaths) -> io::Result<Self> {
        Self::bind_with_expected_uid(paths, getuid())
    }

    pub(crate) fn bind_with_expected_uid(
        paths: &RuntimePaths,
        expected_uid: Uid,
    ) -> io::Result<Self> {
        let path = match &paths.endpoint {
            LocalEndpoint::UnixSocket(path) => path.clone(),
            LocalEndpoint::NamedPipe(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not a Unix endpoint",
                ));
            }
        };
        remove_stale_socket(&path)?;
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, Permissions::from_mode(0o600))?;
        verify_socket(&path)?;
        Ok(Self {
            listener,
            expected_uid,
        })
    }

    pub(crate) async fn accept(&mut self) -> io::Result<UnixStream> {
        let (stream, _) = self.listener.accept().await?;
        // Peer credentials describe one connection, not the endpoint. macOS
        // reads them with `LOCAL_PEEREPID` and `getpeereid`, which need a
        // still-connected peer, so a client that closed between connect and
        // accept fails this call with `ENOTCONN` where Linux answers from what
        // `SO_PEERCRED` recorded at connect time. Either way the connection is
        // refused and never served, so this is the same current-user decision
        // as a UID mismatch; ending the listener for it would take local IPC
        // down for every client until the daemon restarts.
        let credentials = stream.peer_cred().map_err(|source| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("Unix peer credentials are unreadable: {source}"),
            )
        })?;
        if credentials.uid() != self.expected_uid.as_raw() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unix peer UID does not match daemon UID",
            ));
        }
        Ok(stream)
    }
}

/// Accept errors that describe one connection or a momentarily unavailable
/// resource rather than a dead endpoint.
///
/// `ECONNABORTED` is a client that vanished between connect and accept, and
/// descriptor exhaustion clears as live connections finish. Ending the listener
/// for either would convert transient backpressure into a silent, permanent
/// loss of local IPC for every client.
pub(crate) fn accept_error_is_transient(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::ConnectionAborted {
        return true;
    }
    let code = error.raw_os_error();
    [rustix::io::Errno::MFILE, rustix::io::Errno::NFILE]
        .into_iter()
        .any(|transient| code == Some(transient.raw_os_error()))
}

pub(crate) fn prepare_runtime(
    runtime_root: &Path,
    profile_root: &Path,
) -> Result<RuntimePaths, RuntimeLayoutError> {
    require_plain_directory(runtime_root, false)?;
    let key = profile_key(profile_root)?;
    let product = runtime_root.join(PRODUCT_RUNTIME_DIR);
    let directory = product.join(&key);
    let endpoint = directory.join(ENDPOINT_FILE);
    // The address bound decides the whole layout, so it is answered from the
    // assembled path before any directory is created and long before `bind`.
    let length = path_len(&endpoint);
    if length > MAX_UNIX_ENDPOINT_PATH_LEN {
        return Err(RuntimeLayoutError::EndpointPathTooLong {
            limit: MAX_UNIX_ENDPOINT_PATH_LEN,
            length,
            path: endpoint,
        });
    }
    ensure_private_directory(&product)?;
    ensure_private_directory(&directory)?;
    Ok(RuntimePaths {
        profile_key: key,
        metadata: directory.join("session.meta"),
        endpoint: LocalEndpoint::UnixSocket(endpoint),
        directory,
    })
}

/// Length of a path in the bytes the kernel copies into `sun_path`, which is
/// what the bound is stated in. Character counts would understate any
/// non-ASCII component.
fn path_len(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().len()
}

fn verify_socket(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "endpoint is not a socket",
        ));
    }
    if metadata.uid() != getuid().as_raw() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket is not owned by the current UID with mode 0600",
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "socket has no parent directory",
        )
    })?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    if parent_metadata.file_type().is_symlink()
        || !parent_metadata.is_dir()
        || parent_metadata.uid() != getuid().as_raw()
        || parent_metadata.permissions().mode() & 0o777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket parent is not current-UID mode 0700",
        ));
    }
    Ok(())
}

pub(crate) fn cleanup_endpoint(paths: &RuntimePaths) {
    if let LocalEndpoint::UnixSocket(path) = &paths.endpoint {
        let _ignored = remove_stale_socket(path);
    }
}

pub(crate) fn secure_metadata(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != getuid().as_raw()
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "session metadata is not current-UID mode 0600",
        ));
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => require_plain_directory(path, false)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(path)?,
        Err(error) => return Err(error),
    }
    fs::set_permissions(path, Permissions::from_mode(0o700))?;
    require_plain_directory(path, true)
}

fn require_plain_directory(path: &Path, private: bool) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime path is not a plain directory",
        ));
    }
    if metadata.uid() != getuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory UID mismatch",
        ));
    }
    if private && metadata.permissions().mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory mode is not 0700",
        ));
    }
    Ok(())
}

fn remove_stale_socket(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_socket()
                && !metadata.file_type().is_symlink()
                && metadata.uid() == getuid().as_raw() =>
        {
            fs::remove_file(path)
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to replace non-socket or foreign endpoint",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
