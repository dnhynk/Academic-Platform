use std::{error::Error, time::Duration};

use academic_daemon::{RunningDaemon, SESSION_NONCE_CAPABILITY_PREFIX};
use academic_rpc::{
    FrameClass,
    generated::{LocalCoreEnvelope, MutationStatus, local_core_envelope},
    read_envelope, write_envelope,
};
use academic_store::queries::canonical_snapshot;

pub mod support;

use support::{TestEnvironment, client_exchange, complete_handshake, connect, handshake, request};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_clients_same_idempotency_get_same_receipt() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let request = request(11, Some(0))?;
    let first = client_exchange(daemon.endpoint(), daemon.session_nonce(), request.clone()).await?;
    let second = client_exchange(daemon.endpoint(), daemon.session_nonce(), request).await?;
    assert_eq!(first.status, MutationStatus::Accepted as i32);
    assert_eq!(second.status, MutationStatus::Duplicate as i32);
    assert_eq!(first.receipt, second.receipt);
    daemon.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_clients_revision_conflict_is_explicit() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let first = client_exchange(
        daemon.endpoint(),
        daemon.session_nonce(),
        request(21, Some(0))?,
    )
    .await?;
    let second = client_exchange(
        daemon.endpoint(),
        daemon.session_nonce(),
        request(22, Some(0))?,
    )
    .await?;
    assert_eq!(first.status, MutationStatus::Accepted as i32);
    assert_eq!(second.status, MutationStatus::Rejected as i32);
    assert_eq!(second.reason, "REVISION_CONFLICT");
    assert_eq!(second.profile_revision, 1);
    daemon.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn desktop_client_disconnect_does_not_stop_daemon() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let request = request(31, Some(0))?;
    let mut stream = connect(daemon.endpoint()).await?;
    complete_handshake(
        &mut stream,
        handshake(daemon.session_nonce().capability_id()),
    )
    .await?;
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableRequest(
            request.clone(),
        )),
    };
    write_envelope(&mut stream, &envelope, FrameClass::Command).await?;
    drop(stream);

    let mut accepted = false;
    for _ in 0..50 {
        let reader = daemon.readers().open()?;
        if canonical_snapshot(&reader)?.profile_revision == 1 {
            accepted = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(accepted);
    let response =
        client_exchange(daemon.endpoint(), daemon.session_nonce(), request.clone()).await?;
    assert_eq!(response.status, MutationStatus::Duplicate as i32);
    daemon.shutdown().await?;

    let restarted = RunningDaemon::start(environment.config(&profile)).await?;
    let replay = client_exchange(restarted.endpoint(), restarted.session_nonce(), request).await?;
    assert_eq!(replay.status, MutationStatus::Duplicate as i32);
    assert_eq!(replay.receipt, response.receipt);
    restarted.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reader_never_obtains_write_authority() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let reader = daemon.readers().open()?;
    let pragmas = reader.pragma_snapshot()?;
    assert!(pragmas.query_only);
    assert!(
        reader
            .execute_batch("CREATE TABLE forbidden_by_reader(value INTEGER);")
            .is_err()
    );
    daemon.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_nonce_mismatch_is_rejected() -> Result<(), Box<dyn Error>> {
    let environment = TestEnvironment::new()?;
    let profile = environment.profile("profile")?;
    let daemon = RunningDaemon::start(environment.config(&profile)).await?;
    let mut stream = connect(daemon.endpoint()).await?;
    let wrong = format!("{SESSION_NONCE_CAPABILITY_PREFIX}{}", "00".repeat(32));
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ClientHandshake(handshake(
            wrong,
        ))),
    };
    write_envelope(&mut stream, &envelope, FrameClass::Handshake).await?;
    assert!(
        read_envelope(&mut stream, FrameClass::Handshake)
            .await
            .is_err()
    );
    daemon.shutdown().await?;
    Ok(())
}
