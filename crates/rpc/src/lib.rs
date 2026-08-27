//! Versioned, bounded in-memory local-core RPC contract.
//!
//! This crate defines only schema conversion, handshake negotiation, and byte
//! framing. It does not create a listener, endpoint, profile, or product network
//! behavior.

use prost::Message;
use tokio::io::{AsyncRead, AsyncWrite};

pub mod convert;
pub mod error;
pub mod frame;
pub mod generated;
pub mod handshake;
pub mod limits;

pub use error::{FrameSection, RpcError};
pub use handshake::{
    LOCAL_CORE_PROTOCOL_NAME, LOCAL_CORE_PROTOCOL_VERSION, MINIMUM_CLIENT_VERSION,
    PHASE1_CAPABILITY_IDS, ProtocolVersion, READ_ONLY_CAPABILITY_IDS, ServerHandshakeConfig,
    WRITE_CAPABILITY_IDS, authorize_mutable_request, expected_capability_for_command,
    negotiate_handshake,
};
pub use limits::{
    FRAME_PREFIX_BYTES, FrameClass, MAX_COMMAND_FRAME_BYTES, MAX_HANDSHAKE_FRAME_BYTES,
};

/// Protobuf package declared by `schemas/proto/academic/v1/local_core.proto`.
pub const LOCAL_CORE_PROTO_PACKAGE: &str = "academic.v1";
/// Unavoidable warning printed before future human-readable data commands.
pub const PHASE1_POLICY_BANNER: &str =
    "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN";

/// Policy object repeated in every local handshake and command response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase1ProtocolPolicy {
    /// Data admission posture.
    pub data_policy: &'static str,
    /// Current temporary store mode.
    pub storage_mode: &'static str,
    /// Explicit lack of at-rest encryption acceptance.
    pub storage_encryption: &'static str,
    /// Real or production data admission.
    pub production_data_allowed: bool,
    /// Product network posture.
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

fn require_frame_class(actual: FrameClass, expected: FrameClass) -> Result<(), RpcError> {
    if actual != expected {
        return Err(RpcError::FrameClassMismatch { expected, actual });
    }
    Ok(())
}

/// Encodes one validated envelope with the cap selected by the protocol phase.
pub fn encode_envelope_frame(
    envelope: &generated::LocalCoreEnvelope,
    class: FrameClass,
) -> Result<Vec<u8>, RpcError> {
    require_frame_class(convert::validate_envelope(envelope)?, class)?;
    frame::encode_message_frame(envelope, class)
}

/// Decodes and semantically validates one unframed envelope payload.
pub fn decode_envelope_payload(
    payload: &[u8],
    class: FrameClass,
) -> Result<generated::LocalCoreEnvelope, RpcError> {
    convert::validate_closed_envelope_wire(payload)?;
    let envelope = generated::LocalCoreEnvelope::decode(payload)?;
    require_frame_class(convert::validate_envelope(&envelope)?, class)?;
    Ok(envelope)
}

/// Decodes exactly one framed envelope and rejects any trailing bytes.
pub fn decode_envelope_frame(
    bytes: &[u8],
    class: FrameClass,
) -> Result<generated::LocalCoreEnvelope, RpcError> {
    decode_envelope_payload(frame::decode_exact_frame(bytes, class)?, class)
}

/// Reads one bounded envelope from an in-memory async stream.
pub async fn read_envelope<R>(
    reader: &mut R,
    class: FrameClass,
) -> Result<generated::LocalCoreEnvelope, RpcError>
where
    R: AsyncRead + Unpin,
{
    let payload = frame::read_frame(reader, class).await?;
    decode_envelope_payload(&payload, class)
}

/// Writes one validated envelope to an in-memory async stream.
pub async fn write_envelope<W>(
    writer: &mut W,
    envelope: &generated::LocalCoreEnvelope,
    class: FrameClass,
) -> Result<(), RpcError>
where
    W: AsyncWrite + Unpin,
{
    require_frame_class(convert::validate_envelope(envelope)?, class)?;
    frame::write_message(writer, envelope, class).await
}

/// Returns whether the committed Prost output exactly matches this build's schema output.
#[must_use]
pub fn committed_proto_codegen_matches() -> bool {
    generated::codegen_fingerprints_match()
}

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
