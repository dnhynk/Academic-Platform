//! Daemon startup and one-request-per-connection IPC service.

use std::{
    fmt, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use academic_core::local_service::{LocalServiceStartup, rejection_response};
use academic_rpc::{
    FrameClass, ServerHandshakeConfig, authorize_mutable_request,
    generated::{LocalCoreEnvelope, local_core_envelope},
    negotiate_handshake, read_envelope, write_envelope,
};
use academic_store::{
    path_policy::{NativePathProbe, PathProbe},
    profile::open_synthetic_profile,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{oneshot, watch},
    task::JoinHandle,
};

use crate::{
    AdmissionError, CLIENT_FRAME_TIMEOUT, DaemonError, LocalEndpoint, ReaderFactory, SessionNonce,
    WriterQueue, daemon_io, runtime_meta, shutdown, singleton, transport,
};

/// Startup inputs. Production uses the native fail-closed path probe.
#[derive(Clone)]
pub struct DaemonConfig {
    profile_root: PathBuf,
    runtime_root: PathBuf,
    probe: Arc<dyn PathProbe>,
    client_frame_timeout: Duration,
    #[cfg(unix)]
    required_peer_uid: Option<u32>,
}

impl fmt::Debug for DaemonConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DaemonConfig")
            .field("profile_root", &self.profile_root)
            .field("runtime_root", &self.runtime_root)
            .field("probe", &self.probe)
            .finish()
    }
}

impl DaemonConfig {
    /// Creates a native production configuration.
    #[must_use]
    pub fn new(profile_root: impl Into<PathBuf>, runtime_root: impl Into<PathBuf>) -> Self {
        Self {
            profile_root: profile_root.into(),
            runtime_root: runtime_root.into(),
            probe: Arc::new(NativePathProbe::default()),
            client_frame_timeout: CLIENT_FRAME_TIMEOUT,
            #[cfg(unix)]
            required_peer_uid: None,
        }
    }

    /// Injects path evidence for deterministic platform-policy tests.
    #[must_use]
    pub fn with_path_probe(mut self, probe: Arc<dyn PathProbe>) -> Self {
        self.probe = probe;
        self
    }

    /// Narrows the bounded wait for one client frame. It can only shorten the
    /// default [`CLIENT_FRAME_TIMEOUT`] and is exposed so timeout evidence runs
    /// deterministically instead of waiting out the product value.
    #[must_use]
    pub fn with_client_frame_timeout(mut self, timeout: Duration) -> Self {
        self.client_frame_timeout = timeout.min(CLIENT_FRAME_TIMEOUT);
        self
    }

    /// Requires a specific Unix peer UID. This can only narrow access below
    /// the default current-UID policy and is exposed for native policy
    /// evidence and constrained service wrappers.
    #[cfg(unix)]
    #[must_use]
    pub const fn with_required_unix_peer_uid(mut self, uid: u32) -> Self {
        self.required_peer_uid = Some(uid);
        self
    }
}

/// One fully started daemon. Dropping is fail-safe; [`Self::shutdown`] is the
/// graceful path that drains admitted work before releasing the singleton.
#[derive(Debug)]
pub struct RunningDaemon {
    endpoint: LocalEndpoint,
    nonce: SessionNonce,
    pub(crate) metadata_path: PathBuf,
    startup: LocalServiceStartup,
    readers: ReaderFactory,
    pub(crate) writer: Arc<WriterQueue>,
    pub(crate) stop: Option<oneshot::Sender<()>>,
    pub(crate) listener_task: Option<JoinHandle<Result<(), DaemonError>>>,
    pub(crate) runtime_paths: transport::RuntimePaths,
}

impl RunningDaemon {
    /// Completes validation, singleton acquisition, V1 reconciliation, writer
    /// ownership, nonce publication, and endpoint binding before returning.
    pub async fn start(config: DaemonConfig) -> Result<Self, DaemonError> {
        let profile = open_synthetic_profile(&config.profile_root, config.probe.as_ref())?;
        let readers = ReaderFactory::new(profile.clone());
        let runtime_paths = transport::prepare_runtime(&config.runtime_root, profile.root())
            .map_err(|source| {
                daemon_io("prepare runtime", config.runtime_root.display(), source)
            })?;
        let singleton = singleton::acquire(&runtime_paths)?;

        let (writer, startup) = WriterQueue::start(profile)?;
        let writer = Arc::new(writer);
        let nonce = SessionNonce::fresh()?;
        let listener = match bind_listener(&runtime_paths, &config) {
            Ok(listener) => listener,
            Err(source) => {
                writer.shutdown();
                return Err(daemon_io(
                    "bind local endpoint",
                    runtime_paths.endpoint.display_value(),
                    source,
                ));
            }
        };
        if let Err(error) = runtime_meta::publish_metadata(&runtime_paths, &nonce) {
            writer.shutdown();
            transport::cleanup_endpoint(&runtime_paths);
            return Err(error);
        }

        let endpoint = runtime_paths.endpoint.clone();
        let (stop, stop_receiver) = oneshot::channel();
        let listener_writer = Arc::clone(&writer);
        let listener_nonce = nonce.clone();
        let listener_paths = runtime_paths.clone();
        let frame_timeout = config.client_frame_timeout;
        let listener_task = tokio::spawn(async move {
            shutdown::listener_loop(
                listener,
                listener_writer,
                listener_nonce,
                frame_timeout,
                stop_receiver,
                singleton,
                listener_paths,
            )
            .await
        });
        Ok(Self {
            endpoint,
            nonce,
            metadata_path: runtime_paths.metadata.clone(),
            startup,
            readers,
            writer,
            stop: Some(stop),
            listener_task: Some(listener_task),
            runtime_paths,
        })
    }

    /// Returns the bound local endpoint.
    #[must_use]
    pub const fn endpoint(&self) -> &LocalEndpoint {
        &self.endpoint
    }

    /// Returns the fresh nonce expected in the first handshake frame.
    #[must_use]
    pub const fn session_nonce(&self) -> &SessionNonce {
        &self.nonce
    }

    /// Returns the protected session metadata path.
    #[must_use]
    pub fn metadata_path(&self) -> &Path {
        &self.metadata_path
    }

    /// Returns startup reconciliation evidence.
    #[must_use]
    pub const fn startup(&self) -> &LocalServiceStartup {
        &self.startup
    }

    /// Returns the read-only connection factory.
    #[must_use]
    pub const fn readers(&self) -> &ReaderFactory {
        &self.readers
    }

    /// Returns a shared handle to the bounded writer lane for diagnostic
    /// evidence and fault-boundary tests.
    #[must_use]
    pub fn writer(&self) -> Arc<WriterQueue> {
        Arc::clone(&self.writer)
    }
}

#[cfg(windows)]
fn bind_listener(
    paths: &transport::RuntimePaths,
    _config: &DaemonConfig,
) -> io::Result<transport::LocalListener> {
    transport::LocalListener::bind(paths)
}

#[cfg(unix)]
fn bind_listener(
    paths: &transport::RuntimePaths,
    config: &DaemonConfig,
) -> io::Result<transport::LocalListener> {
    match config.required_peer_uid {
        Some(uid) => transport::LocalListener::bind_with_expected_uid(
            paths,
            rustix::process::Uid::from_raw(uid),
        ),
        None => transport::LocalListener::bind(paths),
    }
}

/// Reads one client frame under a bounded deadline.
///
/// `Ok(None)` means graceful shutdown began first. An expired deadline is an
/// error so the caller closes the connection: without it a client that connects
/// and sends nothing parks a serve task and a transport instance forever.
async fn read_client_frame<S>(
    stream: &mut S,
    class: FrameClass,
    frame_timeout: Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<Option<LocalCoreEnvelope>, DaemonError>
where
    S: AsyncRead + Unpin,
{
    tokio::select! {
        received = tokio::time::timeout(frame_timeout, read_envelope(stream, class)) => match received {
            Ok(envelope) => Ok(Some(envelope?)),
            Err(_elapsed) => Err(daemon_io(
                "read client frame",
                "local endpoint",
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "client frame did not arrive within the bounded wait",
                ),
            )),
        },
        _ = shutdown.changed() => Ok(None),
    }
}

pub(crate) async fn serve_connection<S>(
    mut stream: S,
    writer: Arc<WriterQueue>,
    nonce: &SessionNonce,
    frame_timeout: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), DaemonError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Some(envelope) = read_client_frame(
        &mut stream,
        FrameClass::Handshake,
        frame_timeout,
        &mut shutdown,
    )
    .await?
    else {
        return Ok(());
    };
    let client = match envelope.payload {
        Some(local_core_envelope::Payload::ClientHandshake(client)) => client,
        _ => {
            return Err(DaemonError::InvalidSessionMetadata(
                "first frame is not a client handshake",
            ));
        }
    };
    let client = runtime_meta::authenticate_session(client, nonce)?;
    let handshake = negotiate_handshake(&client, &ServerHandshakeConfig::default())?;
    let response = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ServerHandshake(
            handshake.clone(),
        )),
    };
    write_envelope(&mut stream, &response, FrameClass::Handshake).await?;

    let Some(envelope) = read_client_frame(
        &mut stream,
        FrameClass::Command,
        frame_timeout,
        &mut shutdown,
    )
    .await?
    else {
        return Ok(());
    };
    let request = match envelope.payload {
        Some(local_core_envelope::Payload::MutableRequest(request)) => request,
        _ => {
            return Err(DaemonError::InvalidSessionMetadata(
                "second frame is not a mutable request",
            ));
        }
    };
    authorize_mutable_request(&handshake, &request)?;
    if *shutdown.borrow() {
        let response = rejection_response(&request, "SHUTTING_DOWN", writer.current_revision())?;
        let response = LocalCoreEnvelope {
            payload: Some(local_core_envelope::Payload::MutableResponse(response)),
        };
        let _ignored = write_envelope(&mut stream, &response, FrameClass::Command).await;
        return Ok(());
    }
    let response = match writer.try_admit(request.clone()) {
        Ok(admitted) => admitted
            .finish()
            .await
            .map_err(|error| DaemonError::ListenerTask(error.to_string()))?
            .map_err(DaemonError::from)?,
        Err(AdmissionError::ResourceExhausted) => {
            rejection_response(&request, "RESOURCE_EXHAUSTED", writer.current_revision())?
        }
        Err(error @ AdmissionError::ShuttingDown) => {
            return Err(DaemonError::ListenerTask(error.to_string()));
        }
    };
    writer.observe_revision(response.profile_revision);
    let response = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableResponse(response)),
    };
    write_envelope(&mut stream, &response, FrameClass::Command).await?;
    Ok(())
}
