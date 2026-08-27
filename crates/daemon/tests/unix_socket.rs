#![cfg(unix)]

use std::{
    error::Error,
    fs, io,
    os::unix::fs::{MetadataExt, PermissionsExt},
};

use academic_daemon::{LocalEndpoint, RunningDaemon};
use academic_rpc::{
    FrameClass,
    generated::{LocalCoreEnvelope, local_core_envelope},
    read_envelope, write_envelope,
};

pub mod support;

use support::{TestEnvironment, connect, handshake};

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
