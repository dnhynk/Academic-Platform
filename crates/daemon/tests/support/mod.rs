use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use academic_core::local_service::{PHASE1_SYNTHETIC_FIXTURE_ID, mutable_request_digest};
use academic_daemon::{DaemonConfig, LocalEndpoint, SessionNonce};
use academic_rpc::{
    FrameClass, LOCAL_CORE_PROTOCOL_NAME, LOCAL_CORE_PROTOCOL_VERSION,
    generated::{
        ClientHandshake, LocalCoreEnvelope, MutableRequest, MutableResponse, ProtocolVersion,
        SyntheticIngestCommand, local_core_envelope, mutable_request,
    },
    read_envelope, write_envelope,
};
use academic_store::{
    path_policy::{
        PathEvidence, PathProbe, PathProbeFailure, ProfileAccess, ProfileRootState, StorageLocality,
    },
    profile::{SyntheticProfile, create_synthetic_profile},
};
use tempfile::TempDir;
use tokio::io::{AsyncRead, AsyncWrite};

pub const INGEST_CAPABILITY: &str = "learning-platform.local.synthetic-ingest.v1";

pub trait ClientStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> ClientStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

#[derive(Debug, Clone, Copy)]
pub struct LocalTestProbe;

impl PathProbe for LocalTestProbe {
    fn inspect(&self, requested_root: &Path) -> Result<PathEvidence, PathProbeFailure> {
        let (root_state, canonical_existing_ancestor, access) =
            match fs::symlink_metadata(requested_root) {
                Ok(metadata) if !metadata.is_dir() => (
                    ProfileRootState::NotDirectory,
                    requested_root.to_path_buf(),
                    ProfileAccess::Unknown,
                ),
                Ok(_) => {
                    let mut entries = fs::read_dir(requested_root).map_err(probe_error)?;
                    let state = if entries.next().is_some() {
                        ProfileRootState::NonEmptyDirectory
                    } else {
                        ProfileRootState::EmptyDirectory
                    };
                    (
                        state,
                        fs::canonicalize(requested_root).map_err(probe_error)?,
                        ProfileAccess::OwnerOnly,
                    )
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let parent = requested_root.parent().ok_or_else(|| {
                        PathProbeFailure::new(
                            academic_store::path_policy::PathProbeFailureCode::Canonicalization,
                            "test profile path has no parent",
                        )
                    })?;
                    (
                        ProfileRootState::Missing,
                        fs::canonicalize(parent).map_err(probe_error)?,
                        ProfileAccess::OwnerOnlyOnCreate,
                    )
                }
                Err(error) => return Err(probe_error(error)),
            };
        Ok(PathEvidence {
            canonical_existing_ancestor,
            root_state,
            storage_locality: StorageLocality::Local,
            access,
            has_symlink_or_reparse_component: false,
            is_sync_folder: false,
            has_git_ancestor: false,
            final_identity_matches: root_state != ProfileRootState::NotDirectory,
        })
    }
}

fn probe_error(error: io::Error) -> PathProbeFailure {
    PathProbeFailure::new(
        academic_store::path_policy::PathProbeFailureCode::OperatingSystem,
        error.to_string(),
    )
}

#[derive(Debug)]
pub struct TestEnvironment {
    pub root: TempDir,
    pub runtime_root: PathBuf,
}

impl TestEnvironment {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let root = TempDir::new()?;
        let runtime_root = root.path().join("runtime");
        fs::create_dir(&runtime_root)?;
        Ok(Self { root, runtime_root })
    }

    pub fn profile(&self, name: &str) -> Result<SyntheticProfile, Box<dyn Error>> {
        let path = self.root.path().join(name);
        Ok(create_synthetic_profile(
            &path,
            &LocalTestProbe,
            [0xd1; 32],
        )?)
    }

    pub fn config(&self, profile: &SyntheticProfile) -> DaemonConfig {
        DaemonConfig::new(profile.root(), &self.runtime_root)
            .with_path_probe(Arc::new(LocalTestProbe))
    }
}

pub fn request(seed: u8, expected_revision: Option<u64>) -> Result<MutableRequest, Box<dyn Error>> {
    let mut request = MutableRequest {
        request_id: vec![seed; 16],
        client_instance_id: vec![0xc1; 16],
        idempotency_key: vec![seed; 32],
        request_digest: vec![0; 32],
        expected_profile_revision: expected_revision,
        capability_id: INGEST_CAPABILITY.to_owned(),
        command: Some(mutable_request::Command::SyntheticIngest(
            SyntheticIngestCommand {
                synthetic_fixture_id: PHASE1_SYNTHETIC_FIXTURE_ID.to_owned(),
            },
        )),
    };
    request.request_digest = mutable_request_digest(&request)?.as_bytes().to_vec();
    Ok(request)
}

pub fn handshake(nonce_capability: String) -> ClientHandshake {
    ClientHandshake {
        protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
        protocol_version: Some(ProtocolVersion {
            major: u32::from(LOCAL_CORE_PROTOCOL_VERSION.major),
            minor: u32::from(LOCAL_CORE_PROTOCOL_VERSION.minor),
        }),
        capability_ids: vec![INGEST_CAPABILITY.to_owned(), nonce_capability],
    }
}

pub async fn connect(endpoint: &LocalEndpoint) -> io::Result<Box<dyn ClientStream>> {
    match endpoint {
        #[cfg(windows)]
        LocalEndpoint::NamedPipe(name) => {
            use tokio::net::windows::named_pipe::ClientOptions;
            Ok(Box::new(ClientOptions::new().open(name)?))
        }
        #[cfg(not(windows))]
        LocalEndpoint::NamedPipe(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows named pipes are not available",
        )),
        #[cfg(unix)]
        LocalEndpoint::UnixSocket(path) => {
            Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
        }
        #[cfg(not(unix))]
        LocalEndpoint::UnixSocket(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Unix sockets are not available",
        )),
    }
}

pub async fn complete_handshake(
    stream: &mut Box<dyn ClientStream>,
    client: ClientHandshake,
) -> Result<(), Box<dyn Error>> {
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ClientHandshake(client)),
    };
    write_envelope(stream, &envelope, FrameClass::Handshake).await?;
    let response = read_envelope(stream, FrameClass::Handshake).await?;
    if !matches!(
        response.payload,
        Some(local_core_envelope::Payload::ServerHandshake(_))
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "server did not return a handshake",
        )
        .into());
    }
    Ok(())
}

pub async fn client_exchange(
    endpoint: &LocalEndpoint,
    nonce: &SessionNonce,
    request: MutableRequest,
) -> Result<MutableResponse, Box<dyn Error>> {
    let mut stream = connect(endpoint).await?;
    complete_handshake(&mut stream, handshake(nonce.capability_id())).await?;
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableRequest(request)),
    };
    write_envelope(&mut stream, &envelope, FrameClass::Command).await?;
    let response = read_envelope(&mut stream, FrameClass::Command).await?;
    match response.payload {
        Some(local_core_envelope::Payload::MutableResponse(response)) => Ok(response),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "server did not return a mutable response",
        )
        .into()),
    }
}
