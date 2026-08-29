#![cfg(unix)]

use std::{
    error::Error,
    fs, io,
    os::unix::fs::{MetadataExt, PermissionsExt},
};

use academic_daemon::{DaemonError, LocalEndpoint, MAX_UNIX_ENDPOINT_PATH_LEN, RunningDaemon};
use academic_rpc::{
    FrameClass,
    generated::{LocalCoreEnvelope, local_core_envelope},
    read_envelope, write_envelope,
};

pub mod support;

use support::{TestEnvironment, connect, handshake, path_len};

/// macOS canonicalizes `$TMPDIR` to the fixed-format 56-byte private path
/// `/private/var/folders/<2>/<30>/T`, and `~/Library/Application Support`
/// resolves comparably. This is the ordinary macOS runtime root the product has
/// to be able to host a profile below.
const MACOS_RUNTIME_ROOT_LEN: usize = 56;

fn socket_path(daemon: &RunningDaemon) -> Result<&std::path::Path, Box<dyn Error>> {
    match daemon.endpoint() {
        LocalEndpoint::UnixSocket(path) => Ok(path),
        LocalEndpoint::NamedPipe(_) => {
            Err(io::Error::new(io::ErrorKind::InvalidData, "not a Unix socket").into())
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unix_socket_parent_0700_socket_0600() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let path = match daemon.endpoint() {
        LocalEndpoint::UnixSocket(path) => path,
        LocalEndpoint::NamedPipe(_) => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not a Unix socket").into());
        }
    };
    let socket = fs::symlink_metadata(path)?;
    let parent =
        fs::symlink_metadata(path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Unix socket has no parent")
        })?)?;
    let metadata = fs::symlink_metadata(daemon.metadata_path())?;
    assert_eq!(socket.uid(), rustix::process::getuid().as_raw());
    assert_eq!(socket.permissions().mode() & 0o777, 0o600);
    assert_eq!(parent.uid(), rustix::process::getuid().as_raw());
    assert_eq!(parent.permissions().mode() & 0o777, 0o700);
    assert_eq!(metadata.uid(), rustix::process::getuid().as_raw());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    daemon.shutdown().await?;

    let symlink_environment = TestEnvironment::new()?;
    let symlink_profile = symlink_environment.profile("profile")?;
    let destination = symlink_environment.root.path().join("link-target");
    fs::create_dir(&destination)?;
    std::os::unix::fs::symlink(
        &destination,
        symlink_environment.runtime_root.join("academic-os"),
    )?;
    assert!(
        RunningDaemon::start(symlink_environment.config(&symlink_profile))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unix_peer_uid_must_match() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let current = rustix::process::getuid().as_raw();
    let candidate = current.wrapping_add(1);
    let wrong = if candidate == u32::MAX { 1 } else { candidate };
    let daemon = RunningDaemon::start(
        environment
            .config(&profile)
            .with_required_unix_peer_uid(wrong),
    )
    .await?;
    let mut client = connect(daemon.endpoint()).await?;
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ClientHandshake(handshake(
            daemon.session_nonce().capability_id(),
        ))),
    };
    let rejected = match write_envelope(&mut client, &envelope, FrameClass::Handshake).await {
        Err(_) => true,
        Ok(()) => read_envelope(&mut client, FrameClass::Handshake)
            .await
            .is_err(),
    };
    assert!(rejected);
    daemon.shutdown().await?;
    Ok(())
}

/// `sun_path` bounds the whole assembled endpoint path, so the product owns
/// that budget explicitly: a path of exactly the limit binds, and the next byte
/// is refused with the limit, the measured length and the offending path before
/// anything is created. Measuring the fixed suffix from a real start keeps this
/// exact on every platform instead of restating the layout here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_endpoint_path_bound_is_enforced_before_binding() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let measured = RunningDaemon::start(environment.config(&profile)).await?;
    let suffix = path_len(socket_path(&measured)?) - path_len(&environment.runtime_root);
    measured.shutdown().await?;

    let at_limit = environment.runtime_root_of_length(MAX_UNIX_ENDPOINT_PATH_LEN - suffix)?;
    let daemon = RunningDaemon::start(environment.config_at(&profile, &at_limit)).await?;
    assert_eq!(path_len(socket_path(&daemon)?), MAX_UNIX_ENDPOINT_PATH_LEN);
    daemon.shutdown().await?;

    let over_limit = environment.runtime_root_of_length(MAX_UNIX_ENDPOINT_PATH_LEN - suffix + 1)?;
    let outcome = RunningDaemon::start(environment.config_at(&profile, &over_limit)).await;
    let Err(DaemonError::EndpointPathTooLong {
        limit,
        length,
        ref path,
    }) = outcome
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "an endpoint past the platform bound was not refused",
        )
        .into());
    };
    assert_eq!(limit, MAX_UNIX_ENDPOINT_PATH_LEN);
    assert_eq!(length, MAX_UNIX_ENDPOINT_PATH_LEN + 1);
    // The refused path is reported whole, so an operator reads the real cause.
    assert_eq!(path.len(), length);
    assert!(path.starts_with(over_limit.to_string_lossy().as_ref()));
    // Nothing below the refused root was created, so no truncated endpoint and
    // no orphaned singleton directory can exist.
    assert!(fs::symlink_metadata(over_limit.join("academic-os")).is_err());
    Ok(())
}

/// An ordinary macOS runtime root hosts a profile, which is the product defect
/// this bound was stated for: before it, the fixed suffix alone spent more of
/// `sun_path` than such a root leaves.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ordinary_macos_runtime_root_hosts_a_profile() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let root = environment.runtime_root_of_length(MACOS_RUNTIME_ROOT_LEN)?;
    let daemon = RunningDaemon::start(environment.config_at(&profile, &root)).await?;

    let path = socket_path(&daemon)?;
    assert!(
        path_len(path) <= MAX_UNIX_ENDPOINT_PATH_LEN,
        "a {MACOS_RUNTIME_ROOT_LEN}-byte runtime root produced a {}-byte endpoint",
        path_len(path)
    );
    // The audited ownership and permission properties hold at this shape too.
    let socket = fs::symlink_metadata(path)?;
    let parent = fs::symlink_metadata(
        path.parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "socket has no parent"))?,
    )?;
    assert_eq!(socket.uid(), rustix::process::getuid().as_raw());
    assert_eq!(socket.permissions().mode() & 0o777, 0o600);
    assert_eq!(parent.uid(), rustix::process::getuid().as_raw());
    assert_eq!(parent.permissions().mode() & 0o777, 0o700);
    daemon.shutdown().await?;
    Ok(())
}
