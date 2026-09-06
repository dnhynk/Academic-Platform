//! Native regressions for ownership across select! cancellation and handoff.

use super::*;
use std::{
    future::{Future, poll_fn},
    pin::pin,
    task::Poll,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::ClientOptions,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn paths(temp: &tempfile::TempDir) -> RuntimePaths {
    RuntimePaths {
        profile_key: "synthetic".into(),
        directory: temp.path().to_owned(),
        metadata: temp.path().join("session.meta"),
        endpoint: LocalEndpoint::NamedPipe(format!(
            r"\\.\pipe\academic-t120-listener-{}-{}",
            std::process::id(),
            temp.path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )),
    }
}

async fn poll_pending(future: std::pin::Pin<&mut impl Future>) {
    let mut future = future;
    poll_fn(|context| {
        assert!(future.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn cancelled_accept_preserves_endpoint_and_already_open_client() -> TestResult {
    for open_before_cancel in [false, true] {
        let temp = tempfile::tempdir()?;
        let mut listener = LocalListener::bind(&paths(&temp))?;
        let pipe_name = listener.name.clone();
        let mut client = None;
        {
            let mut accepting = pin!(listener.accept());
            poll_pending(accepting.as_mut()).await;
            if open_before_cancel {
                client = Some(ClientOptions::new().open(&pipe_name)?);
            }
            // Drop exactly the future select! drops when join_next wins. No
            // timer or stress loop is needed to hit the cancellation window.
        }
        let mut client = match client {
            Some(client) => client,
            None => ClientOptions::new().open(&pipe_name)?,
        };
        client.write_all(b"kept").await?;
        let mut stream = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await??;
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).await?;
        assert_eq!(&bytes, b"kept");
        verify_pipe_acl(&stream, &listener.security.sid)?;
    }
    Ok(())
}

#[tokio::test]
async fn successor_is_openable_before_first_accept_resumes() -> TestResult {
    let temp = tempfile::tempdir()?;
    let mut listener = LocalListener::bind(&paths(&temp))?;
    let pipe_name = listener.name.clone();
    let mut accepting = Box::pin(listener.accept());
    poll_pending(accepting.as_mut()).await;
    let mut first = ClientOptions::new().open(&pipe_name)?;
    // Hold the accept future unpolled after the first open: the old listener
    // cannot create its successor here and deterministically returns 231.
    let mut second = ClientOptions::new().open(&pipe_name)?;
    first.write_all(b"1").await?;
    second.write_all(b"2").await?;
    let mut first_server = tokio::time::timeout(Duration::from_secs(2), accepting).await??;
    let mut second_server =
        tokio::time::timeout(Duration::from_secs(2), listener.accept()).await??;
    let mut byte = [0];
    first_server.read_exact(&mut byte).await?;
    assert_eq!(&byte, b"1");
    second_server.read_exact(&mut byte).await?;
    assert_eq!(&byte, b"2");
    verify_pipe_acl(&second_server, &listener.security.sid)?;
    Ok(())
}

#[tokio::test]
async fn instance_ceiling_preserves_existing_accept_and_recovers() -> TestResult {
    let temp = tempfile::tempdir()?;
    let mut listener = LocalListener::bind(&paths(&temp))?;
    let pipe_name = listener.name.clone();
    let first = ClientOptions::new().open(&pipe_name)?;
    let first_server = listener.accept().await?;
    let mut held = Vec::new();
    // Fill the real max_instances(64) pool with idle synthetic servers.
    for _ in 0..62 {
        held.push(create_pipe(&pipe_name, &mut listener.security, false)?);
    }
    assert_eq!(
        create_pipe(&pipe_name, &mut listener.security, false)
            .err()
            .and_then(|error| error.raw_os_error()),
        Some(231)
    );
    let second = ClientOptions::new().open(&pipe_name)?;
    let second_server = listener.accept().await?;
    drop((held, first, first_server, second, second_server));
    {
        let mut accepting = pin!(listener.accept());
        poll_pending(accepting.as_mut()).await;
    }
    let _client = ClientOptions::new().open(&pipe_name)?;
    let stream = tokio::time::timeout(Duration::from_secs(2), listener.accept()).await??;
    verify_pipe_acl(&stream, &listener.security.sid)?;
    Ok(())
}
