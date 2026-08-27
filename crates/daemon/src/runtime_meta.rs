//! Per-process nonce generation and protected runtime metadata publication.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

use academic_rpc::generated::ClientHandshake;

use crate::{
    DaemonError, SESSION_NONCE_CAPABILITY_PREFIX, daemon_io,
    transport::{self, RuntimePaths},
};

/// Unpredictable nonce bound to exactly one daemon lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNonce([u8; 32]);

impl SessionNonce {
    pub(crate) fn fresh() -> Result<Self, DaemonError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|error| {
            daemon_io(
                "generate session nonce",
                "current process",
                io::Error::other(error.to_string()),
            )
        })?;
        Ok(Self(bytes))
    }

    /// Returns the P1 handshake capability carrying this nonce.
    #[must_use]
    pub fn capability_id(&self) -> String {
        format!("{SESSION_NONCE_CAPABILITY_PREFIX}{}", hex::encode(self.0))
    }

    /// Returns lowercase hexadecimal for current-user metadata readers.
    #[must_use]
    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }
}

pub(crate) fn authenticate_session(
    mut client: ClientHandshake,
    nonce: &SessionNonce,
) -> Result<ClientHandshake, DaemonError> {
    let expected = nonce.capability_id();
    let mut matching = 0_usize;
    let mut nonce_count = 0_usize;
    client.capability_ids.retain(|capability| {
        if capability.starts_with(SESSION_NONCE_CAPABILITY_PREFIX) {
            nonce_count += 1;
            if capability == &expected {
                matching += 1;
            }
            false
        } else {
            true
        }
    });
    if nonce_count != 1 || matching != 1 {
        return Err(DaemonError::InvalidSessionMetadata(
            "session nonce is missing, stale, duplicated, or mismatched",
        ));
    }
    Ok(client)
}

pub(crate) fn publish_metadata(
    paths: &RuntimePaths,
    nonce: &SessionNonce,
) -> Result<(), DaemonError> {
    remove_metadata(&paths.metadata)?;
    let temporary = paths
        .directory
        .join(format!("session-{}.tmp", nonce.as_hex()));
    let contents = format!(
        "version=1\nendpoint={}\nnonce={}\n",
        paths.endpoint.display_value(),
        nonce.as_hex()
    );
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|source| daemon_io("create session metadata", temporary.display(), source))?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|source| daemon_io("persist session metadata", temporary.display(), source))?;
    drop(file);
    fs::rename(&temporary, &paths.metadata).map_err(|source| {
        daemon_io("publish session metadata", paths.metadata.display(), source)
    })?;
    if let Err(source) = transport::secure_metadata(&paths.metadata) {
        let _ignored = remove_metadata(&paths.metadata);
        return Err(daemon_io(
            "secure session metadata",
            paths.metadata.display(),
            source,
        ));
    }
    Ok(())
}

pub(crate) fn remove_metadata(path: &Path) -> Result<(), DaemonError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            fs::remove_file(path)
                .map_err(|source| daemon_io("remove session metadata", path.display(), source))
        }
        Ok(_) => Err(DaemonError::InvalidSessionMetadata(
            "refusing to replace a link or non-file metadata entry",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(daemon_io(
            "inspect session metadata",
            path.display(),
            source,
        )),
    }
}
