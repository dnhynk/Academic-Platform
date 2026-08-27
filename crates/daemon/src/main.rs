use std::{env, error::Error, io, path::PathBuf};

use academic_daemon::{DaemonConfig, RunningDaemon};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("{}", academic_rpc::PHASE1_POLICY_BANNER);
    let mut arguments = env::args_os().skip(1);
    let profile_root = arguments.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: academicd <profile-root> <runtime-root>",
        )
    })?;
    let runtime_root = arguments.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: academicd <profile-root> <runtime-root>",
        )
    })?;
    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: academicd <profile-root> <runtime-root>",
        )
        .into());
    }
    let daemon = RunningDaemon::start(DaemonConfig::new(profile_root, runtime_root)).await?;
    println!("READY endpoint={}", daemon.endpoint().display_value());
    tokio::signal::ctrl_c().await?;
    daemon.shutdown().await?;
    Ok(())
}
