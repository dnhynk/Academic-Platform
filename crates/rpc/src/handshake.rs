//! Deterministic same-major version and capability negotiation.

use std::collections::BTreeSet;

use crate::{
    PHASE1_PROTOCOL_POLICY,
    error::RpcError,
    generated::{
        self, ClientHandshake, MutableRequest, ProfileLockState, ServerHandshake,
        StorageSchemaVersion, SyntheticOnlyPolicy, WriteDisposition, mutable_request,
    },
    limits::{MAX_CAPABILITY_ID_BYTES, MAX_CAPABILITY_IDS, MAX_DAEMON_BUILD_BYTES},
};

/// Product-neutral protocol identifier used during local handshake.
pub const LOCAL_CORE_PROTOCOL_NAME: &str = "learning-platform.local-core";

/// Local IPC semantic version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion {
    /// Compatibility-breaking version.
    pub major: u16,
    /// Same-major additive version.
    pub minor: u16,
}

/// First local protocol version and minimum compatible client.
pub const LOCAL_CORE_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
/// Minimum client accepted by the Phase 1 daemon contract.
pub const MINIMUM_CLIENT_VERSION: ProtocolVersion = LOCAL_CORE_PROTOCOL_VERSION;

/// Exact capability names reserved for the synthetic-only local core.
pub const PHASE1_CAPABILITY_IDS: &[&str] = &[
    "learning-platform.local.diagnostics.v1",
    "learning-platform.local.synthetic-backup.v1",
    "learning-platform.local.synthetic-export.v1",
    "learning-platform.local.synthetic-ingest.v1",
    "learning-platform.local.synthetic-restore.v1",
];

/// Capabilities that cannot mutate canonical profile state.
pub const READ_ONLY_CAPABILITY_IDS: &[&str] = &[
    "learning-platform.local.diagnostics.v1",
    "learning-platform.local.synthetic-export.v1",
];

/// Capabilities admitted by the closed mutable request oneof.
pub const WRITE_CAPABILITY_IDS: &[&str] = &[
    "learning-platform.local.synthetic-backup.v1",
    "learning-platform.local.synthetic-ingest.v1",
    "learning-platform.local.synthetic-restore.v1",
];

/// Bounded server facts copied into a handshake response.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerHandshakeConfig {
    /// Stable build identity, not a wall-clock value.
    pub daemon_build: String,
    /// Projection builders and source watermarks visible at handshake time.
    pub projections: Vec<generated::ProjectionState>,
    /// Current profile lock state.
    pub lock_state: ProfileLockState,
}

impl Default for ServerHandshakeConfig {
    fn default() -> Self {
        Self {
            daemon_build: format!("academicd/{}", env!("CARGO_PKG_VERSION")),
            projections: Vec::new(),
            lock_state: ProfileLockState::Unlocked,
        }
    }
}

fn contains_capability(haystack: &[&str], needle: &str) -> bool {
    haystack.contains(&needle)
}

fn is_known_capability(capability: &str) -> bool {
    contains_capability(PHASE1_CAPABILITY_IDS, capability)
}

fn proto_version(version: ProtocolVersion) -> generated::ProtocolVersion {
    generated::ProtocolVersion {
        major: u32::from(version.major),
        minor: u32::from(version.minor),
    }
}

const fn client_is_below_minimum(client: ProtocolVersion, minimum: ProtocolVersion) -> bool {
    client.major == minimum.major && client.minor < minimum.minor
}

const fn negotiate_same_major_minor(
    client: ProtocolVersion,
    server: ProtocolVersion,
) -> ProtocolVersion {
    ProtocolVersion {
        major: server.major,
        minor: if client.minor < server.minor {
            client.minor
        } else {
            server.minor
        },
    }
}

pub(crate) fn protocol_version_from_proto(
    value: Option<&generated::ProtocolVersion>,
    field: &'static str,
) -> Result<ProtocolVersion, RpcError> {
    let value = value.ok_or(RpcError::MissingField { field })?;
    let major = u16::try_from(value.major).map_err(|_| RpcError::ProtocolVersionOutOfRange {
        field,
        major: value.major,
        minor: value.minor,
    })?;
    let minor = u16::try_from(value.minor).map_err(|_| RpcError::ProtocolVersionOutOfRange {
        field,
        major: value.major,
        minor: value.minor,
    })?;
    Ok(ProtocolVersion { major, minor })
}

pub(crate) fn validate_capability_list(capabilities: &[String]) -> Result<(), RpcError> {
    if capabilities.len() > MAX_CAPABILITY_IDS {
        return Err(RpcError::TooManyCapabilities {
            actual: capabilities.len(),
            maximum: MAX_CAPABILITY_IDS,
        });
    }
    for capability in capabilities {
        if capability.is_empty() || capability.len() > MAX_CAPABILITY_ID_BYTES {
            return Err(RpcError::InvalidCapabilityId {
                capability: capability.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_client_handshake(
    client: &ClientHandshake,
) -> Result<ProtocolVersion, RpcError> {
    if client.protocol_name != LOCAL_CORE_PROTOCOL_NAME {
        return Err(RpcError::ProtocolNameMismatch {
            actual: client.protocol_name.clone(),
        });
    }
    validate_capability_list(&client.capability_ids)?;
    protocol_version_from_proto(client.protocol_version.as_ref(), "client.protocol_version")
}

fn policy_message() -> SyntheticOnlyPolicy {
    SyntheticOnlyPolicy {
        data_policy: PHASE1_PROTOCOL_POLICY.data_policy.to_owned(),
        storage_mode: PHASE1_PROTOCOL_POLICY.storage_mode.to_owned(),
        storage_encryption: PHASE1_PROTOCOL_POLICY.storage_encryption.to_owned(),
        production_data_allowed: PHASE1_PROTOCOL_POLICY.production_data_allowed,
        product_network: PHASE1_PROTOCOL_POLICY.product_network.to_owned(),
    }
}

/// Negotiates a bounded capability intersection in canonical server order.
///
/// A major mismatch leaves only matching diagnostics/export capabilities. Any
/// unknown requested capability fails the complete write set closed.
pub fn negotiate_handshake(
    client: &ClientHandshake,
    config: &ServerHandshakeConfig,
) -> Result<ServerHandshake, RpcError> {
    let client_version = validate_client_handshake(client)?;
    if config.daemon_build.is_empty() || config.daemon_build.len() > MAX_DAEMON_BUILD_BYTES {
        return Err(RpcError::InvalidFieldValue {
            field: "server.daemon_build",
            reason: "must be nonempty and bounded",
        });
    }

    let requested = client
        .capability_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let has_unknown = requested
        .iter()
        .copied()
        .any(|capability| !is_known_capability(capability));
    let same_major = client_version.major == LOCAL_CORE_PROTOCOL_VERSION.major;
    let below_minimum = client_is_below_minimum(client_version, MINIMUM_CLIENT_VERSION);

    let (write_disposition, write_denial_reason) = if !same_major {
        (
            WriteDisposition::DeniedMajorVersion,
            "MAJOR_VERSION_MISMATCH",
        )
    } else if below_minimum {
        (
            WriteDisposition::DeniedClientTooOld,
            "CLIENT_VERSION_BELOW_MINIMUM",
        )
    } else if has_unknown {
        (
            WriteDisposition::DeniedUnknownCapability,
            "UNKNOWN_WRITE_CAPABILITY",
        )
    } else {
        (WriteDisposition::Allowed, "")
    };

    let capability_ids = PHASE1_CAPABILITY_IDS
        .iter()
        .copied()
        .filter(|capability| requested.contains(capability))
        .filter(|capability| {
            same_major || contains_capability(READ_ONLY_CAPABILITY_IDS, capability)
        })
        .map(str::to_owned)
        .collect();
    let negotiated_protocol_version = same_major.then(|| {
        proto_version(negotiate_same_major_minor(
            client_version,
            LOCAL_CORE_PROTOCOL_VERSION,
        ))
    });

    Ok(ServerHandshake {
        protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
        protocol_version: Some(proto_version(LOCAL_CORE_PROTOCOL_VERSION)),
        minimum_client_version: Some(proto_version(MINIMUM_CLIENT_VERSION)),
        daemon_build: config.daemon_build.clone(),
        storage_schema: Some(StorageSchemaVersion {
            number: 1,
            semantic_version: "1.0.0".to_owned(),
        }),
        vault_read_formats: vec!["PLAINTEXT_SYNTHETIC_V1".to_owned()],
        vault_write_format: "PLAINTEXT_SYNTHETIC_V1".to_owned(),
        projections: config.projections.clone(),
        lock_state: config.lock_state as i32,
        policy: Some(policy_message()),
        capability_ids,
        negotiated_protocol_version,
        write_disposition: write_disposition as i32,
        write_denial_reason: write_denial_reason.to_owned(),
    })
}

/// Returns the sole capability authorized to select a known write arm.
pub fn expected_capability_for_command(command: &mutable_request::Command) -> &'static str {
    match command {
        mutable_request::Command::SyntheticIngest(_) => {
            "learning-platform.local.synthetic-ingest.v1"
        }
        mutable_request::Command::SyntheticBackup(_) => {
            "learning-platform.local.synthetic-backup.v1"
        }
        mutable_request::Command::SyntheticRestore(_) => {
            "learning-platform.local.synthetic-restore.v1"
        }
    }
}

/// Checks the negotiated write disposition and binds capability to command.
pub fn authorize_mutable_request(
    handshake: &ServerHandshake,
    request: &MutableRequest,
) -> Result<(), RpcError> {
    let disposition = WriteDisposition::try_from(handshake.write_disposition).map_err(|_| {
        RpcError::UnknownEnumValue {
            field: "server.write_disposition",
            value: handshake.write_disposition,
        }
    })?;
    if disposition != WriteDisposition::Allowed {
        return Err(RpcError::WriteDenied {
            disposition: handshake.write_disposition,
            reason: handshake.write_denial_reason.clone(),
        });
    }

    let command = request.command.as_ref().ok_or(RpcError::MissingField {
        field: "mutable_request.command",
    })?;
    if !contains_capability(WRITE_CAPABILITY_IDS, &request.capability_id) {
        return Err(RpcError::UnknownWriteCapability {
            capability: request.capability_id.clone(),
        });
    }
    let expected = expected_capability_for_command(command);
    if request.capability_id != expected {
        return Err(RpcError::CapabilityCommandMismatch {
            capability: request.capability_id.clone(),
            expected,
        });
    }
    if !handshake
        .capability_ids
        .iter()
        .any(|capability| capability == expected)
    {
        return Err(RpcError::CapabilityNotNegotiated {
            capability: request.capability_id.clone(),
        });
    }
    Ok(())
}
