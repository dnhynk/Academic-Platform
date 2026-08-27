//! Version and capability names for the future local IPC boundary.
//!
//! F0 contains no framing implementation, listener, socket, pipe, or client.

/// Protobuf package reserved by `schemas/proto/academic/v1/local_core.proto`.
pub const LOCAL_CORE_PROTO_PACKAGE: &str = "academic.v1";
/// Product-neutral protocol identifier used during local handshake.
pub const LOCAL_CORE_PROTOCOL_NAME: &str = "learning-platform.local-core";
/// Four-byte unsigned big-endian length prefix.
pub const FRAME_PREFIX_BYTES: usize = 4;
/// Maximum encoded handshake frame.
pub const MAX_HANDSHAKE_FRAME_BYTES: usize = 64 * 1024;
/// Maximum encoded command frame.
pub const MAX_COMMAND_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Unavoidable warning printed before future human-readable data commands.
pub const PHASE1_POLICY_BANNER: &str =
    "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN";

/// Local IPC semantic version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion {
    pub major: u16,
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

/// Policy object repeated in every future handshake and command response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase1ProtocolPolicy {
    pub data_policy: &'static str,
    pub storage_mode: &'static str,
    pub storage_encryption: &'static str,
    pub production_data_allowed: bool,
    pub product_network: &'static str,
}

/// Exact Phase 1 handshake posture.
pub const PHASE1_PROTOCOL_POLICY: Phase1ProtocolPolicy = Phase1ProtocolPolicy {
    data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
    storage_mode: "PLAINTEXT_TEMPORARY_SQLITE",
    storage_encryption: "NONE",
    production_data_allowed: false,
    product_network: "NONE",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_protocol_names_and_limits_are_frozen() {
        assert_eq!(LOCAL_CORE_PROTO_PACKAGE, "academic.v1");
        assert_eq!(LOCAL_CORE_PROTOCOL_VERSION.major, 1);
        assert_eq!(FRAME_PREFIX_BYTES, 4);
        assert_eq!(MAX_HANDSHAKE_FRAME_BYTES, 65_536);
        assert_eq!(MAX_COMMAND_FRAME_BYTES, 8_388_608);
        assert!(PHASE1_CAPABILITY_IDS.is_sorted());
    }
}
