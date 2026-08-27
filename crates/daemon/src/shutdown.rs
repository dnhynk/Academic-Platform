//! Listener coordination and deterministic graceful shutdown.

use std::{io, sync::Arc};

use tokio::{
    sync::{oneshot, watch},
    task::{JoinError, JoinSet},
};

use crate::{
    DaemonError, SessionNonce, WriterQueue, daemon_io,
    runtime_meta::remove_metadata,
    service::{RunningDaemon, serve_connection},
    transport::{self, LocalListener, RuntimePaths, SingletonGuard},
};

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

pub(crate) async fn listener_loop(
    mut listener: LocalListener,
    writer: Arc<WriterQueue>,
    nonce: SessionNonce,
    mut stop: oneshot::Receiver<()>,
    _singleton: SingletonGuard,
    runtime_paths: RuntimePaths,
) -> Result<(), DaemonError> {
    let _cleanup = RuntimeCleanup(runtime_paths);
    let mut connections = JoinSet::new();
    let (connection_stop, _) = watch::channel(false);
    let mut listener_error = None;
    loop {
        tokio::select! {
            _ = &mut stop => {
                let _previous = connection_stop.send_replace(true);
                break;
            },
            accepted = listener.accept() => match accepted {
                Ok(stream) => {
                    let connection_writer = Arc::clone(&writer);
                    let connection_nonce = nonce.clone();
                    let connection_shutdown = connection_stop.subscribe();
                    connections.spawn(async move {
                        serve_connection(
                            stream,
                            connection_writer,
                            &connection_nonce,
                            connection_shutdown,
                        )
                        .await
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => continue,
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
        match result {
            Ok(Ok(())) | Ok(Err(_)) => {}
            Err(error) => return Err(DaemonError::ListenerTask(error.to_string())),
        }
    }
    match listener_error {
        Some(error) => Err(error),
        None => Ok(()),
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
