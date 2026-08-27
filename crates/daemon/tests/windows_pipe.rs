#![cfg(windows)]

use std::{error::Error, io};

use academic_daemon::{LocalEndpoint, RunningDaemon};
use tokio::net::windows::named_pipe::ClientOptions;

pub mod support;

use support::TestEnvironment;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_pipe_acl_current_user_only() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    // Startup calls GetSecurityInfo and refuses the listener unless its DACL
    // is protected and contains exactly one current-token SID allow ACE.
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    assert!(matches!(daemon.endpoint(), LocalEndpoint::NamedPipe(_)));
    daemon.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_pipe_rejects_remote_clients() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let name = match daemon.endpoint() {
        LocalEndpoint::NamedPipe(name) => name,
        LocalEndpoint::UnixSocket(_) => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not a named pipe").into());
        }
    };
    let remote = name.replacen(r"\\.\pipe\", r"\\localhost\pipe\", 1);
    assert!(ClientOptions::new().open(remote).is_err());
    daemon.shutdown().await?;
    Ok(())
}
