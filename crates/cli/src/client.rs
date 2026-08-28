//! Local IPC client for the one-request-per-connection Phase 1 protocol.
//!
//! This module is the CLI's only path to a running daemon. It never opens the
//! canonical database and never constructs a writer: it reads the current-user
//! session metadata a daemon published, connects to that endpoint, completes the
//! versioned handshake, and optionally sends exactly one mutable request.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use academic_daemon::{LocalEndpoint, session_metadata_path};
use academic_rpc::{
    FrameClass, LOCAL_CORE_PROTOCOL_NAME, LOCAL_CORE_PROTOCOL_VERSION,
    generated::{
        ClientHandshake, LocalCoreEnvelope, MutableRequest, MutableResponse, ProtocolVersion,
        ServerHandshake, local_core_envelope,
    },
    read_envelope, write_envelope,
};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::output::{CliFailure, ExitClass};

/// Endpoint and session nonce a running daemon published for one profile.
#[derive(Debug, Clone)]
pub struct SessionMetadata {
    /// Bound local endpoint.
    pub endpoint: LocalEndpoint,
    /// Capability-carrying nonce required by the first handshake frame.
    pub nonce_capability: String,
    /// Path the metadata was read from.
    pub path: PathBuf,
}

const SESSION_NONCE_CAPABILITY_PREFIX: &str = academic_daemon::SESSION_NONCE_CAPABILITY_PREFIX;

/// Resolves the current-user runtime root when `--runtime` is not supplied.
///
/// The lookup fails closed. There is no world-writable fallback, because a
/// shared temporary directory would let another account present a socket or a
/// session file to this one.
pub fn default_runtime_root() -> Result<PathBuf, CliFailure> {
    #[cfg(windows)]
    let candidate = std::env::var_os("LOCALAPPDATA");
    #[cfg(unix)]
    let candidate = std::env::var_os("XDG_RUNTIME_DIR");

    #[cfg(windows)]
    const VARIABLE: &str = "LOCALAPPDATA";
    #[cfg(unix)]
    const VARIABLE: &str = "XDG_RUNTIME_DIR";

    candidate
        .map(PathBuf::from)
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| {
            CliFailure::new(
                ExitClass::Unavailable,
                "RUNTIME_ROOT_UNRESOLVED",
                format!("{VARIABLE} is not set; pass --runtime explicitly"),
            )
        })
}

fn parse_endpoint(value: &str) -> Result<LocalEndpoint, CliFailure> {
    if value.is_empty() {
        return Err(CliFailure::new(
            ExitClass::Internal,
            "SESSION_METADATA_INVALID",
            "session metadata recorded an empty endpoint",
        ));
    }
    #[cfg(windows)]
    {
        Ok(LocalEndpoint::NamedPipe(value.to_owned()))
    }
    #[cfg(unix)]
    {
        Ok(LocalEndpoint::UnixSocket(PathBuf::from(value)))
    }
}

/// Reads the session metadata a daemon published for one profile.
///
/// `Ok(None)` means no daemon currently owns the profile, which is a normal
/// state rather than a failure.
pub fn read_session_metadata(
    runtime_root: &Path,
    profile_root: &Path,
) -> Result<Option<SessionMetadata>, CliFailure> {
    // The runtime directory is keyed by the canonical profile root, so a root
    // that does not exist yet cannot be resolved. It also cannot be owned by a
    // daemon, which is exactly what the caller is asking, so report that
    // rather than failing: `restore` legitimately asks about an absent
    // destination.
    if !profile_root.exists() {
        return Ok(None);
    }
    let path = session_metadata_path(runtime_root, profile_root).map_err(|error| {
        CliFailure::new(
            ExitClass::Unavailable,
            "SESSION_METADATA_UNRESOLVED",
            format!("could not resolve the runtime directory for the profile: {error}"),
        )
    })?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CliFailure::new(
                ExitClass::Internal,
                "SESSION_METADATA_UNREADABLE",
                format!("{}: {error}", path.display()),
            ));
        }
    };

    let mut version = None;
    let mut endpoint = None;
    let mut nonce = None;
    for line in contents.lines() {
        match line.split_once('=') {
            Some(("version", value)) => version = Some(value.to_owned()),
            Some(("endpoint", value)) => endpoint = Some(value.to_owned()),
            Some(("nonce", value)) => nonce = Some(value.to_owned()),
            _ => {}
        }
    }
    if version.as_deref() != Some("1") {
        return Err(CliFailure::new(
            ExitClass::Incompatible,
            "SESSION_METADATA_VERSION",
            "session metadata is not version 1",
        ));
    }
    let (Some(endpoint), Some(nonce)) = (endpoint, nonce) else {
        return Err(CliFailure::new(
            ExitClass::Internal,
            "SESSION_METADATA_INVALID",
            "session metadata is missing an endpoint or nonce",
        ));
    };
    if nonce.is_empty() || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CliFailure::new(
            ExitClass::Internal,
            "SESSION_METADATA_INVALID",
            "session nonce is not lowercase hexadecimal",
        ));
    }
    Ok(Some(SessionMetadata {
        endpoint: parse_endpoint(&endpoint)?,
        nonce_capability: format!("{SESSION_NONCE_CAPABILITY_PREFIX}{nonce}"),
        path,
    }))
}

trait ClientStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> ClientStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

async fn connect(endpoint: &LocalEndpoint) -> io::Result<Box<dyn ClientStream>> {
    match endpoint {
        #[cfg(windows)]
        LocalEndpoint::NamedPipe(name) => {
            use tokio::net::windows::named_pipe::ClientOptions;
            Ok(Box::new(ClientOptions::new().open(name)?))
        }
        #[cfg(not(windows))]
        LocalEndpoint::NamedPipe(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows named pipes are not available on this host",
        )),
        #[cfg(unix)]
        LocalEndpoint::UnixSocket(path) => {
            Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
        }
        #[cfg(not(unix))]
        LocalEndpoint::UnixSocket(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Unix sockets are not available on this host",
        )),
    }
}

/// Reason reported when published metadata names an endpoint nothing answers.
///
/// A daemon that was terminated abruptly cannot remove its own session file, so
/// stale metadata is the normal state after a crash rather than an anomaly.
pub const DAEMON_UNREACHABLE: &str = "DAEMON_UNREACHABLE";

fn transport_failure(error: &io::Error) -> CliFailure {
    CliFailure::new(
        ExitClass::Unavailable,
        DAEMON_UNREACHABLE,
        format!("the published endpoint did not accept a connection: {error}"),
    )
}

fn client_handshake(capabilities: &[&str], nonce_capability: &str) -> ClientHandshake {
    let mut capability_ids = capabilities
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    capability_ids.push(nonce_capability.to_owned());
    ClientHandshake {
        protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
        protocol_version: Some(ProtocolVersion {
            major: u32::from(LOCAL_CORE_PROTOCOL_VERSION.major),
            minor: u32::from(LOCAL_CORE_PROTOCOL_VERSION.minor),
        }),
        capability_ids,
    }
}

async fn exchange_handshake(
    stream: &mut Box<dyn ClientStream>,
    handshake: ClientHandshake,
) -> Result<ServerHandshake, CliFailure> {
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ClientHandshake(handshake)),
    };
    write_envelope(stream, &envelope, FrameClass::Handshake)
        .await
        .map_err(|error| {
            CliFailure::new(
                ExitClass::Incompatible,
                "HANDSHAKE_WRITE_FAILED",
                error.to_string(),
            )
        })?;
    let response = read_envelope(stream, FrameClass::Handshake)
        .await
        .map_err(|error| {
            CliFailure::new(
                ExitClass::Incompatible,
                "HANDSHAKE_READ_FAILED",
                error.to_string(),
            )
        })?;
    match response.payload {
        Some(local_core_envelope::Payload::ServerHandshake(handshake)) => Ok(handshake),
        _ => Err(CliFailure::new(
            ExitClass::Incompatible,
            "HANDSHAKE_UNEXPECTED_FRAME",
            "the daemon did not return a server handshake",
        )),
    }
}

/// Completes a handshake and closes the connection without writing anything.
pub async fn handshake_only(
    metadata: &SessionMetadata,
    capabilities: &[&str],
) -> Result<ServerHandshake, CliFailure> {
    let mut stream = connect(&metadata.endpoint)
        .await
        .map_err(|error| transport_failure(&error))?;
    exchange_handshake(
        &mut stream,
        client_handshake(capabilities, &metadata.nonce_capability),
    )
    .await
}

/// Sends exactly one mutable request over a fresh authenticated connection.
pub async fn send_mutation(
    metadata: &SessionMetadata,
    capability: &str,
    request: MutableRequest,
) -> Result<(ServerHandshake, MutableResponse), CliFailure> {
    let mut stream = connect(&metadata.endpoint)
        .await
        .map_err(|error| transport_failure(&error))?;
    let handshake = exchange_handshake(
        &mut stream,
        client_handshake(&[capability], &metadata.nonce_capability),
    )
    .await?;
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableRequest(request)),
    };
    write_envelope(&mut stream, &envelope, FrameClass::Command)
        .await
        .map_err(|error| {
            CliFailure::new(
                ExitClass::Internal,
                "REQUEST_WRITE_FAILED",
                error.to_string(),
            )
        })?;
    let response = read_envelope(&mut stream, FrameClass::Command)
        .await
        .map_err(|error| {
            CliFailure::new(
                ExitClass::Internal,
                "RESPONSE_READ_FAILED",
                error.to_string(),
            )
        })?;
    match response.payload {
        Some(local_core_envelope::Payload::MutableResponse(response)) => Ok((handshake, response)),
        _ => Err(CliFailure::new(
            ExitClass::Internal,
            "RESPONSE_UNEXPECTED_FRAME",
            "the daemon did not return a mutable response",
        )),
    }
}
