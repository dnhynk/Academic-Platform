use std::{error::Error, time::Duration};

use academic_daemon::{MAX_CONCURRENT_CONNECTIONS, RunningDaemon};
use academic_rpc::generated::{MutableRequest, MutableResponse, MutationStatus};
use tokio::io::AsyncReadExt;

pub mod support;

use support::{ClientStream, TestEnvironment, client_exchange, connect, request};

/// More attempts than the concurrency bound and the 64-instance Windows
/// named-pipe ceiling combined, so the listener must both refuse unbounded
/// growth and keep creating replacement instances under the pressure.
const HELD_CONNECTION_ATTEMPTS: usize = 128;

async fn exchange_until_served(
    daemon: &RunningDaemon,
    request: MutableRequest,
) -> Result<MutableResponse, Box<dyn Error>> {
    let mut last = None;
    for _ in 0..100 {
        match client_exchange(daemon.endpoint(), daemon.session_nonce(), request.clone()).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                last = Some(error);
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
    Err(last.unwrap_or_else(|| "no exchange was attempted".into()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn held_open_connections_do_not_stop_the_listener() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;

    let mut held: Vec<Box<dyn ClientStream>> = Vec::new();
    for _ in 0..HELD_CONNECTION_ATTEMPTS {
        match connect(daemon.endpoint()).await {
            Ok(stream) => held.push(stream),
            // Refusing one excess connection is the bounded outcome. The listener
            // itself must survive it.
            Err(_) => tokio::time::sleep(Duration::from_millis(5)).await,
        }
    }
    assert!(
        held.len() >= MAX_CONCURRENT_CONNECTIONS,
        "only {} connections were established",
        held.len()
    );
    drop(held);

    // A legitimate writer still reaches the daemon after the pressure clears.
    let response = exchange_until_served(&daemon, request(41, Some(0))?).await?;
    assert_eq!(response.status, MutationStatus::Accepted as i32);
    // A listener that stopped on a transport error stores it until here.
    daemon.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_read_timeout_closes_the_connection() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(
        environment
            .config(&profile)
            .with_client_frame_timeout(Duration::from_millis(250)),
    )
    .await?;

    let mut silent = connect(daemon.endpoint()).await?;
    let mut byte = [0_u8; 1];
    let outcome = tokio::time::timeout(Duration::from_secs(5), silent.read(&mut byte)).await;
    // A closed connection reports end of stream on Unix and a broken pipe on
    // Windows; both prove the deadline released the served slot.
    assert!(
        matches!(outcome, Ok(Ok(0)) | Ok(Err(_))),
        "a silent client was not closed by the bounded frame deadline"
    );
    drop(silent);

    let response = exchange_until_served(&daemon, request(42, Some(0))?).await?;
    assert_eq!(response.status, MutationStatus::Accepted as i32);
    daemon.shutdown().await?;
    Ok(())
}
