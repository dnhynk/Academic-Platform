//! Listener coordination and deterministic graceful shutdown.

use std::{io, sync::Arc, time::Duration};

use academic_admission::Posture;
use tokio::{
    sync::{oneshot, watch},
    task::{JoinError, JoinSet},
};

use crate::{
    DaemonError, MAX_CONCURRENT_CONNECTIONS, SessionNonce, WriterQueue, daemon_io,
    runtime_meta::remove_metadata,
    service::{RunningDaemon, serve_connection},
    transport::{self, LocalListener, RuntimePaths, SingletonGuard},
};

/// Pause before retrying an accept that failed transiently.
///
/// A transient transport error such as a momentarily full Windows named-pipe
/// instance ceiling must not stop the listener, and retrying it with no pause
/// would spin one core for as long as the condition lasts.
const TRANSIENT_ACCEPT_BACKOFF: Duration = Duration::from_millis(50);

impl RunningDaemon {
    /// Stops accepting connections, lets admitted connection work finish,
    /// drains the writer queue, removes metadata/socket state, and releases the
    /// singleton last.
    pub async fn shutdown(mut self) -> Result<(), DaemonError> {
        if let Some(stop) = self.stop.take() {
            let _ignored = stop.send(());
        }
        if let Some(task) = self.listener_task.take() {
            join_listener(task.await)??;
        }
        self.writer.shutdown();
        remove_metadata(&self.metadata_path)?;
        transport::cleanup_endpoint(&self.runtime_paths);
        Ok(())
    }
}

impl Drop for RunningDaemon {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ignored = stop.send(());
        }
    }
}

fn join_listener(
    result: Result<Result<(), DaemonError>, JoinError>,
) -> Result<Result<(), DaemonError>, DaemonError> {
    result.map_err(|error| DaemonError::ListenerTask(error.to_string()))
}

#[derive(Debug)]
pub(crate) struct ListenerConfig {
    pub writer: Arc<WriterQueue>,
    pub nonce: SessionNonce,
    pub posture: Posture,
    pub frame_timeout: Duration,
}

pub(crate) async fn listener_loop(
    mut listener: LocalListener,
    config: ListenerConfig,
    mut stop: oneshot::Receiver<()>,
    _singleton: SingletonGuard,
    runtime_paths: RuntimePaths,
) -> Result<(), DaemonError> {
    let _cleanup = RuntimeCleanup(runtime_paths);
    let mut connections = JoinSet::new();
    let (connection_stop, _) = watch::channel(false);
    let mut listener_error = None;
    let mut backoff = false;
    loop {
        if backoff {
            backoff = false;
            tokio::select! {
                _ = &mut stop => {
                    let _previous = connection_stop.send_replace(true);
                    break;
                },
                () = tokio::time::sleep(TRANSIENT_ACCEPT_BACKOFF) => {}
            }
        }
        // At the concurrency bound the listener reaps finished work instead of
        // accepting, so a client that connects and holds cannot grow the live
        // serve tasks, descriptors, and transport instances without limit.
        if connections.len() >= MAX_CONCURRENT_CONNECTIONS {
            tokio::select! {
                _ = &mut stop => {
                    let _previous = connection_stop.send_replace(true);
                    break;
                },
                Some(result) = connections.join_next() => join_connection(result)?,
            }
            continue;
        }
        tokio::select! {
            _ = &mut stop => {
                let _previous = connection_stop.send_replace(true);
                break;
            },
            Some(result) = connections.join_next() => join_connection(result)?,
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let connection_writer = Arc::clone(&config.writer);
                    let connection_nonce = config.nonce.clone();
                    let connection_posture = config.posture.clone();
                    let connection_shutdown = connection_stop.subscribe();
                    let frame_timeout = config.frame_timeout;
                    connections.spawn(async move {
                        serve_connection(
                            stream,
                            connection_writer,
                            &connection_nonce,
                            &connection_posture,
                            frame_timeout,
                            connection_shutdown,
                        )
                        .await
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => continue,
                // A transient transport error describes one connection or a
                // momentarily unavailable instance, not a dead endpoint. Tearing
                // the listener down for it turns backpressure into a permanent
                // outage that no client can observe until shutdown.
                Err(error) if transport::accept_error_is_transient(&error) => backoff = true,
                Err(source) => {
                    let _previous = connection_stop.send_replace(true);
                    listener_error = Some(daemon_io(
                        "accept local connection",
                        "local endpoint",
                        source,
                    ));
                    break;
                }
            },
        }
    }
    while let Some(result) = connections.join_next().await {
        join_connection(result)?;
    }
    match listener_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn join_connection(result: Result<Result<(), DaemonError>, JoinError>) -> Result<(), DaemonError> {
    match result {
        Ok(Ok(())) | Ok(Err(_)) => Ok(()),
        Err(error) => Err(DaemonError::ListenerTask(error.to_string())),
    }
}

#[derive(Debug)]
struct RuntimeCleanup(RuntimePaths);

impl Drop for RuntimeCleanup {
    fn drop(&mut self) {
        let _ignored = remove_metadata(&self.0.metadata);
        transport::cleanup_endpoint(&self.0);
    }
}
