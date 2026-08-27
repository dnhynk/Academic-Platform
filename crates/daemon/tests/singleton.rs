use std::error::Error;

use academic_daemon::{DaemonError, RunningDaemon};

pub mod support;

use support::TestEnvironment;

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
