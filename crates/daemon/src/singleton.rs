//! Per-current-user, per-profile singleton acquisition.

use std::io;

use crate::{
    DaemonError, daemon_io,
    transport::{RuntimePaths, SingletonGuard},
};

pub(crate) fn acquire(paths: &RuntimePaths) -> Result<SingletonGuard, DaemonError> {
    SingletonGuard::acquire(paths).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            DaemonError::AlreadyRunning
        } else {
            daemon_io("acquire profile singleton", &paths.profile_key, source)
        }
    })
}
