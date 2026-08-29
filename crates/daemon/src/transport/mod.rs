//! Current-user-only singleton and local IPC transport adapters.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

/// Concrete endpoint published in the current-user session metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEndpoint {
    /// Windows session-local named pipe.
    NamedPipe(String),
    /// Unix-domain socket below a mode-0700 parent.
    UnixSocket(PathBuf),
}

impl LocalEndpoint {
    /// Stable endpoint text used by session metadata.
    #[must_use]
    pub fn display_value(&self) -> String {
        match self {
            Self::NamedPipe(value) => value.clone(),
            Self::UnixSocket(value) => value.to_string_lossy().into_owned(),
        }
    }
}

/// Lock file backing the per-profile singleton on both platforms.
pub(crate) const SINGLETON_LOCK_FILE: &str = "academicd.lock";

/// Product namespace directory below the caller-supplied runtime root.
pub(crate) const PRODUCT_RUNTIME_DIR: &str = "academic-os";

/// Failure while resolving the runtime layout for one profile.
///
/// A layout failure is not always an operating-system error: on Unix the
/// assembled endpoint path has to fit the platform socket address, and that is
/// decided from the path alone, before anything is created or bound.
#[derive(Debug)]
pub(crate) enum RuntimeLayoutError {
    /// An operating-system boundary failed.
    Io(io::Error),
    /// The assembled Unix endpoint path exceeds the platform address bound.
    #[cfg(unix)]
    EndpointPathTooLong {
        /// Longest endpoint path the platform address can carry.
        limit: usize,
        /// Measured length of the assembled endpoint path.
        length: usize,
        /// The offending assembled path, never truncated to fit.
        path: PathBuf,
    },
}

impl From<io::Error> for RuntimeLayoutError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

/// Paths resolved before singleton acquisition and listener creation.
#[derive(Debug, Clone)]
pub(crate) struct RuntimePaths {
    pub(crate) profile_key: String,
    pub(crate) directory: PathBuf,
    pub(crate) metadata: PathBuf,
    pub(crate) endpoint: LocalEndpoint,
}

/// Digest bytes carried by [`profile_key`], hexadecimal-encoded to twice as
/// many path characters.
///
/// The key namespaces profiles inside one user's runtime area, so the bound
/// that matters is the birthday bound over the profile roots that user actually
/// hosts: with a 64-bit key, `n` distinct roots share one key with probability
/// about `n^2 / 2^65`, which is 3e-14 at a thousand roots and still 3e-12 at
/// ten thousand, both far past what one person reaches. Every character also
/// costs one byte of the Unix endpoint path, which `sun_path` bounds at 104
/// bytes on macOS, so the key is kept exactly as long as that namespace needs
/// and no longer.
const PROFILE_KEY_BYTES: usize = 8;

/// Maps one canonical profile root to the directory that carries its singleton
/// lock, its session metadata, and its endpoint.
///
/// The mapping is a pure function of the canonical root, so one profile always
/// resolves to one runtime directory and two profiles resolve to the same one
/// only on a [`PROFILE_KEY_BYTES`] digest-prefix collision.
fn profile_key(profile_root: &Path) -> io::Result<String> {
    let canonical = fs::canonicalize(profile_root)?;
    let mut hasher = Sha256::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(canonical.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        for unit in canonical.as_os_str().encode_wide() {
            hasher.update(unit.to_le_bytes());
        }
    }
    let digest = hasher.finalize();
    Ok(hex::encode(&digest[..PROFILE_KEY_BYTES]))
}

#[cfg(unix)]
pub use unix::MAX_UNIX_ENDPOINT_PATH_LEN;
#[cfg(unix)]
pub(crate) use unix::{
    LocalListener, SingletonGuard, accept_error_is_transient, cleanup_endpoint, prepare_runtime,
    secure_metadata,
};
#[cfg(windows)]
pub(crate) use windows::{
    LocalListener, SingletonGuard, accept_error_is_transient, cleanup_endpoint, prepare_runtime,
    secure_metadata,
};
