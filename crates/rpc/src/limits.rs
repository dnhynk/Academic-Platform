//! Frozen local protocol limits.

/// Four-byte unsigned big-endian frame prefix.
pub const FRAME_PREFIX_BYTES: usize = 4;
/// Maximum encoded handshake envelope.
pub const MAX_HANDSHAKE_FRAME_BYTES: usize = 64 * 1024;
/// Maximum encoded command envelope.
pub const MAX_COMMAND_FRAME_BYTES: usize = 8 * 1024 * 1024;
/// Exact byte width of an opaque local identifier.
pub const OPAQUE_ID_BYTES: usize = 16;
/// Exact byte width of an idempotency key.
pub const IDEMPOTENCY_KEY_BYTES: usize = 32;
/// Exact byte width of a SHA-256 digest.
pub const SHA256_DIGEST_BYTES: usize = 32;
/// Maximum number of capability identifiers carried by one handshake.
pub const MAX_CAPABILITY_IDS: usize = 64;
/// Maximum UTF-8 byte length of one capability identifier.
pub const MAX_CAPABILITY_ID_BYTES: usize = 128;
/// Maximum UTF-8 byte length of a daemon build identifier.
pub const MAX_DAEMON_BUILD_BYTES: usize = 256;
/// Maximum UTF-8 byte length of a response reason.
pub const MAX_RESPONSE_REASON_BYTES: usize = 1_024;
/// Maximum UTF-8 byte length of a synthetic fixture allowlist identifier.
pub const MAX_SYNTHETIC_FIXTURE_ID_BYTES: usize = 256;
/// Maximum number of projection states returned by the bounded handshake.
pub const MAX_PROJECTION_STATES: usize = 256;
/// Maximum accepted nesting depth for legacy Protobuf group syntax.
pub const MAX_PROTOBUF_NESTING_DEPTH: usize = 16;

/// The protocol phase selecting the allocation cap for a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameClass {
    /// Client/server handshake envelope.
    Handshake,
    /// Mutable request/response envelope.
    Command,
}

impl FrameClass {
    /// Returns the exact maximum payload length for this frame class.
    #[must_use]
    pub const fn max_payload_bytes(self) -> usize {
        match self {
            Self::Handshake => MAX_HANDSHAKE_FRAME_BYTES,
            Self::Command => MAX_COMMAND_FRAME_BYTES,
        }
    }
}
