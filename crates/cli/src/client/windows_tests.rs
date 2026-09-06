//! Synthetic native-pipe controls; no daemon fault harness retry is involved.

use super::*;
use std::{
    future::{Future, poll_fn},
    pin::pin,
    task::Poll,
    time::{Duration, Instant},
};
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn endpoint(temp: &tempfile::TempDir) -> LocalEndpoint {
    LocalEndpoint::NamedPipe(format!(
        r"\\.\pipe\academic-t120-{}-{}",
        std::process::id(),
        temp.path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    ))
}

fn name(endpoint: &LocalEndpoint) -> io::Result<&str> {
    match endpoint {
        LocalEndpoint::NamedPipe(name) => Ok(name),
        _ => Err(io::Error::other("expected a pipe")),
    }
}

async fn assert_pending(future: std::pin::Pin<&mut impl Future>) {
    let mut future = future;
    poll_fn(|context| {
        assert!(future.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn delayed_endpoint_and_forced_busy_gap_recover_before_sending() -> TestResult {
    for busy in [false, true] {
        let temp = tempfile::tempdir()?;
        let endpoint = endpoint(&temp);
        let pipe_name = name(&endpoint)?;
        let occupied = if busy {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_name)?;
            let client = ClientOptions::new().open(pipe_name)?;
            server.connect().await?;
            Some((server, client))
        } else {
            None
        };
        // Observe the real OS failure, then force connect() to attempt that
        // same unavailable state before publishing a replacement. No timing
        // assumption decides whether the first product open saw the gap.
        assert_eq!(
            ClientOptions::new()
                .open(pipe_name)
                .err()
                .and_then(|error| error.raw_os_error()),
            Some(if busy { 231 } else { 2 })
        );
        let mut connecting = pin!(connect(&endpoint));
        assert_pending(connecting.as_mut()).await;
        drop(occupied);
        let mut server = ServerOptions::new().create(pipe_name)?;
        let mut stream = connecting.await?;
        server.connect().await?;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream.write_all(b"one").await?;
        let mut bytes = [0; 3];
        server.read_exact(&mut bytes).await?;
        assert_eq!(&bytes, b"one");
    }
    Ok(())
}

#[tokio::test]
async fn absent_endpoint_keeps_exact_public_unreachable_class() -> TestResult {
    let temp = tempfile::tempdir()?;
    let metadata = SessionMetadata {
        endpoint: endpoint(&temp),
        nonce_capability: "synthetic".into(),
        path: temp.path().join("session.meta"),
    };
    let started = Instant::now();
    let result = handshake_only(&metadata, &[]).await;
    let error = result.err().ok_or("absent endpoint succeeded")?;
    assert_eq!(error.class(), ExitClass::Unavailable);
    assert_eq!(error.class().code(), 14);
    assert_eq!(error.reason(), "DAEMON_UNREACHABLE");
    // Scheduling slack is not the policy proof; the injected-clock test
    // separately proves the exact 500 ms deadline and 25-open ceiling.
    assert!(started.elapsed() < Duration::from_secs(2));
    Ok(())
}

#[tokio::test]
async fn native_access_denied_is_returned_without_waiting_for_availability() -> TestResult {
    let temp = tempfile::tempdir()?;
    let endpoint = endpoint(&temp);
    let _server = ServerOptions::new()
        .first_pipe_instance(true)
        .access_outbound(false)
        .create(name(&endpoint)?)?;
    let mut connecting = pin!(connect(&endpoint));
    // An immediate Ready proves no retry sleep was introduced for OS error 5.
    let result = poll_fn(|context| {
        let result = connecting.as_mut().poll(context);
        assert!(result.is_ready(), "access denied was retried");
        result
    })
    .await;
    assert_eq!(result.err().and_then(|error| error.raw_os_error()), Some(5));
    Ok(())
}

#[tokio::test]
async fn handshake_and_ambiguous_mutation_failures_never_reconnect() -> TestResult {
    use academic_rpc::generated::{SyntheticIngestCommand, mutable_request};
    const CAPABILITY: &str = "learning-platform.local.synthetic-ingest.v1";
    for after_mutation in [false, true] {
        let temp = tempfile::tempdir()?;
        let endpoint = endpoint(&temp);
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(name(&endpoint)?)?;
        let spare = ServerOptions::new().create(name(&endpoint)?)?;
        let metadata = SessionMetadata {
            endpoint,
            nonce_capability: "synthetic".into(),
            path: temp.path().join("session.meta"),
        };
        let client = async {
            if after_mutation {
                send_mutation(
                    &metadata,
                    CAPABILITY,
                    MutableRequest {
                        request_id: vec![1; 16],
                        client_instance_id: vec![2; 16],
                        idempotency_key: vec![3; 32],
                        request_digest: vec![4; 32],
                        expected_profile_revision: None,
                        capability_id: CAPABILITY.into(),
                        command: Some(mutable_request::Command::SyntheticIngest(
                            SyntheticIngestCommand {
                                synthetic_fixture_id: "phase0-synthetic-bitemporal-ledger-v2"
                                    .into(),
                            },
                        )),
                    },
                )
                .await
                .map(|_| ())
            } else {
                handshake_only(&metadata, &[]).await.map(|_| ())
            }
        };
        let peer = async {
            server.connect().await?;
            let handshake = read_envelope(&mut server, FrameClass::Handshake).await?;
            assert!(matches!(
                handshake.payload,
                Some(local_core_envelope::Payload::ClientHandshake(_))
            ));
            if after_mutation {
                write_envelope(
                    &mut server,
                    &LocalCoreEnvelope {
                        payload: Some(local_core_envelope::Payload::ServerHandshake(
                            academic_rpc::negotiate_handshake(
                                &client_handshake(&[CAPABILITY], "synthetic"),
                                &academic_rpc::ServerHandshakeConfig::default(),
                            )?,
                        )),
                    },
                    FrameClass::Handshake,
                )
                .await?;
                let request = read_envelope(&mut server, FrameClass::Command).await?;
                assert!(matches!(
                    request.payload,
                    Some(local_core_envelope::Payload::MutableRequest(_))
                ));
            }
            // The mutation may already have committed. Losing its response
            // must remain an error, even with a second instance ready to open.
            drop(server);
            Ok::<(), Box<dyn std::error::Error>>(())
        };
        let (result, served) =
            tokio::time::timeout(Duration::from_secs(2), async { tokio::join!(client, peer) })
                .await?;
        served?;
        let error = result
            .err()
            .ok_or("closed response unexpectedly succeeded")?;
        assert_eq!(
            error.reason(),
            if after_mutation {
                "RESPONSE_READ_FAILED"
            } else {
                "HANDSHAKE_READ_FAILED"
            }
        );
        assert_pending(pin!(spare.connect()).as_mut()).await;
    }
    Ok(())
}
