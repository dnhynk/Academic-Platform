//! Host-dispatched durable directory, no-replace publication, and temp-lock operations.

use std::{fs::File, path::Path};

use crate::{VaultError, VaultResult};

#[cfg(unix)]
#[path = "platform/unix.rs"]
mod host;
#[cfg(windows)]
#[path = "platform/windows.rs"]
mod host;

#[cfg(not(any(unix, windows)))]
mod host {
    use std::{fs::File, io, path::Path};

    #[derive(Debug)]
    pub(crate) struct LockedTemp(File);

    impl LockedTemp {
        pub(crate) fn file_mut(&mut self) -> &mut File {
            &mut self.0
        }
    }

    pub(crate) fn create_locked_temp(_path: &Path) -> io::Result<LockedTemp> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }

    pub(crate) fn sync_directory(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }

    pub(crate) fn open_readonly_no_follow(_path: &Path) -> io::Result<File> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }

    pub(crate) fn symlink_metadata_no_follow(_path: &Path) -> io::Result<std::fs::Metadata> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }

    pub(crate) fn publish_no_replace(_source: &Path, _destination: &Path) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }

    pub(crate) fn publish_locked_no_replace(
        _temp: &mut LockedTemp,
        _source: &Path,
        _destination: &Path,
    ) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }

    pub(crate) fn try_remove_unlocked(_path: &Path) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported vault host",
        ))
    }
}

pub(crate) use host::LockedTemp;

pub(crate) fn create_locked_temp(path: &Path) -> VaultResult<LockedTemp> {
    host::create_locked_temp(path)
        .map_err(|source| VaultError::io("create locked vault temp", path, source))
}

pub(crate) fn sync_directory(path: &Path) -> VaultResult<()> {
    host::sync_directory(path)
        .map_err(|source| VaultError::io("synchronize vault directory", path, source))
}

pub(crate) fn open_readonly_no_follow(path: &Path) -> VaultResult<File> {
    host::open_readonly_no_follow(path).map_err(|source| {
        VaultError::io(
            "open sealed vault object without link traversal",
            path,
            source,
        )
    })
}

pub(crate) fn symlink_metadata_no_follow(path: &Path) -> std::io::Result<std::fs::Metadata> {
    host::symlink_metadata_no_follow(path)
}

/// Publishes an ingest temp while retaining its platform exclusion handle.
pub(crate) fn publish_locked_no_replace(
    temp: &mut LockedTemp,
    source: &Path,
    destination: &Path,
) -> VaultResult<bool> {
    host::publish_locked_no_replace(temp, source, destination).map_err(|source_error| {
        VaultError::io(
            "publish locked vault object without replacement",
            destination,
            source_error,
        )
    })
}

/// Returns `true` when this call published and `false` when the destination already existed.
pub(crate) fn publish_no_replace(source: &Path, destination: &Path) -> VaultResult<bool> {
    host::publish_no_replace(source, destination).map_err(|source_error| {
        VaultError::io(
            "publish vault object without replacement",
            destination,
            source_error,
        )
    })
}

/// Removes an expired temp while holding the platform exclusion primitive.
///
/// `true` means removed (or concurrently absent); `false` means another live owner held it.
pub(crate) fn try_remove_unlocked(path: &Path) -> VaultResult<bool> {
    host::try_remove_unlocked(path)
        .map_err(|source| VaultError::io("remove expired unlocked vault temp", path, source))
}
