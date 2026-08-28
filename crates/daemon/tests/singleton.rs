use std::{error::Error, fs, io, path::PathBuf};

use academic_daemon::{DaemonError, RunningDaemon};

pub mod support;

use support::TestEnvironment;

/// Matches the lock file both transports place in the profile runtime directory.
const SINGLETON_LOCK_FILE: &str = "academicd.lock";

/// Observes whether the guard handle is still open.
///
/// A daemon holds its lock file open for its whole lifetime, so an
/// exclusive-share open is refused until the guard is released.
#[cfg(windows)]
fn singleton_lock_is_held(path: &std::path::Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .is_err()
}

fn singleton_lock_path(daemon: &RunningDaemon) -> Result<PathBuf, Box<dyn Error>> {
    let directory = daemon.metadata_path().parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "session metadata has no runtime directory",
        )
    })?;
    Ok(directory.join(SINGLETON_LOCK_FILE))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_daemon_same_profile_is_rejected() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let second = RunningDaemon::start(environment.config(&profile)).await;
    assert!(matches!(second, Err(DaemonError::AlreadyRunning)));
    daemon.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn different_profiles_have_independent_singletons() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let first_profile = environment.profile("first")?;
    let second_profile = environment.profile("second")?;
    let first = RunningDaemon::start(environment.config(&first_profile)).await?;
    let second = RunningDaemon::start(environment.config(&second_profile)).await?;
    assert_ne!(first.endpoint(), second.endpoint());
    first.shutdown().await?;
    second.shutdown().await?;
    Ok(())
}

/// The singleton is a lock on a per-profile file rather than a per-logon-session
/// object, so a second acquisition for the same profile fails wherever it is
/// attempted while a different profile is unaffected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_singleton_is_a_lock_released_with_its_daemon() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let other_profile = environment.profile("other")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let lock = singleton_lock_path(&daemon)?;
    assert!(fs::symlink_metadata(&lock)?.is_file());
    #[cfg(windows)]
    assert!(singleton_lock_is_held(&lock));

    let second = RunningDaemon::start(environment.config(&profile)).await;
    assert!(matches!(second, Err(DaemonError::AlreadyRunning)));
    let other = RunningDaemon::start(environment.config(&other_profile)).await?;
    assert_ne!(singleton_lock_path(&other)?, lock);
    other.shutdown().await?;

    daemon.shutdown().await?;
    #[cfg(windows)]
    assert!(!singleton_lock_is_held(&lock));

    // The lock is released with the daemon that owned it, so the same profile
    // starts again without leaving an orphaned singleton behind.
    let restarted = RunningDaemon::start(environment.config(&profile)).await?;
    assert_eq!(singleton_lock_path(&restarted)?, lock);
    restarted.shutdown().await?;
    Ok(())
}
