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

/// Paths resolved before singleton acquisition and listener creation.
#[derive(Debug, Clone)]
pub(crate) struct RuntimePaths {
    pub(crate) profile_key: String,
    pub(crate) directory: PathBuf,
    pub(crate) metadata: PathBuf,
    pub(crate) endpoint: LocalEndpoint,
}

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
    // A 160-bit profile namespace keeps Unix socket paths below SUN_LEN
    // while retaining collision resistance far beyond the per-user scope.
    Ok(hex::encode(&digest[..20]))
}

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
