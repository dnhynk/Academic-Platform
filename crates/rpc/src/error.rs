//! Typed local RPC rejection reasons.

use std::{error::Error, fmt, io};

use crate::limits::FrameClass;

/// Frame portion in which an EOF occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSection {
    /// Four-byte length prefix.
    Prefix,
    /// Length-delimited Protobuf payload.
    Payload,
}

/// Fail-closed framing, wire, conversion, and negotiation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// An in-memory async reader or writer failed.
    Io { kind: io::ErrorKind },
    /// A frame declared an empty payload.
    ZeroLengthFrame,
    /// A host-size payload cannot be represented by the u32 frame prefix.
    FrameLengthOverflow { declared: usize },
    /// The declared payload exceeds the selected pre-allocation limit.
    FrameTooLarge { declared: usize, maximum: usize },
    /// A bounded allocation could not be reserved.
    FrameAllocationFailed { requested: usize },
    /// EOF occurred before the selected frame portion was complete.
    TruncatedFrame {
        section: FrameSection,
        expected: usize,
        received: usize,
    },
    /// Exact-one-frame input contained bytes after the declared payload.
    TrailingFrameData { trailing: usize },
    /// The decoded envelope belongs to a different protocol phase.
    FrameClassMismatch {
        expected: FrameClass,
        actual: FrameClass,
    },
    /// Prost could not encode a message.
    EncodeFailure { detail: String },
    /// Protobuf bytes were malformed or used a forbidden wire construction.
    MalformedData { detail: String },
    /// Legacy group nesting exceeded the fixed parser budget.
    ProtobufNestingLimitExceeded { maximum: usize },
    /// The envelope oneof was absent.
    MissingEnvelopePayload,
    /// More than one envelope oneof arm appeared on the wire.
    AmbiguousEnvelopePayload,
    /// The envelope contained an unknown field.
    UnknownEnvelopeField { tag: u32 },
    /// A singular field appeared more than once on the strict write boundary.
    DuplicateField { message: &'static str, tag: u32 },
    /// A mutable request contained an unknown non-command field.
    UnknownMutableRequestField { tag: u32 },
    /// A mutable request selected an unrecognized command tag.
    UnknownWriteCommand { tag: u32 },
    /// More than one write command appeared on the wire.
    AmbiguousWriteCommand,
    /// A required semantic field was absent.
    MissingField { field: &'static str },
    /// A byte field did not preserve its exact contract width.
    InvalidFieldLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    /// A field was present but outside its semantic domain.
    InvalidFieldValue {
        field: &'static str,
        reason: &'static str,
    },
    /// A Protobuf enum carried an unknown or unspecified discriminant.
    UnknownEnumValue { field: &'static str, value: i32 },
    /// A protocol version could not be represented by the local u16 contract.
    ProtocolVersionOutOfRange {
        field: &'static str,
        major: u32,
        minor: u32,
    },
    /// The peer used a different protocol identity.
    ProtocolNameMismatch { actual: String },
    /// A handshake attempted to exceed its bounded capability list.
    TooManyCapabilities { actual: usize, maximum: usize },
    /// A capability identifier was empty or too long.
    InvalidCapabilityId { capability: String },
    /// A mutable request named a capability that cannot authorize a write.
    UnknownWriteCapability { capability: String },
    /// The known write command and declared capability disagreed.
    CapabilityCommandMismatch {
        capability: String,
        expected: &'static str,
    },
    /// The request capability was not part of the negotiated intersection.
    CapabilityNotNegotiated { capability: String },
    /// Handshake negotiation denied every write.
    WriteDenied { disposition: i32, reason: String },
    /// Receipt and response copies of an immutable field disagreed.
    InconsistentReceipt { field: &'static str },
    /// An inclusive acceptance range was reversed.
    InvalidAcceptanceRange { start: u64, end: u64 },
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { kind } => write!(formatter, "in-memory I/O failed: {kind:?}"),
            Self::ZeroLengthFrame => formatter.write_str("zero-length frame is forbidden"),
            Self::FrameLengthOverflow { declared } => {
                write!(
                    formatter,
                    "frame length {declared} cannot fit the u32 prefix"
                )
            }
            Self::FrameTooLarge { declared, maximum } => {
                write!(formatter, "frame length {declared} exceeds limit {maximum}")
            }
            Self::FrameAllocationFailed { requested } => {
                write!(
                    formatter,
                    "could not reserve bounded frame allocation {requested}"
                )
            }
            Self::TruncatedFrame {
                section,
                expected,
                received,
            } => write!(
                formatter,
                "truncated frame {section:?}: expected {expected} bytes, received {received}",
            ),
            Self::TrailingFrameData { trailing } => {
                write!(formatter, "exact frame has {trailing} trailing bytes")
            }
            Self::FrameClassMismatch { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected:?} envelope, decoded {actual:?}"
                )
            }
            Self::EncodeFailure { detail } => write!(formatter, "Protobuf encode failed: {detail}"),
            Self::MalformedData { detail } => write!(formatter, "malformed Protobuf: {detail}"),
            Self::ProtobufNestingLimitExceeded { maximum } => {
                write!(formatter, "Protobuf nesting exceeds fixed limit {maximum}")
            }
            Self::MissingEnvelopePayload => formatter.write_str("envelope payload is missing"),
            Self::AmbiguousEnvelopePayload => {
                formatter.write_str("multiple envelope payload arms are forbidden")
            }
            Self::UnknownEnvelopeField { tag } => {
                write!(formatter, "unknown envelope field tag {tag}")
            }
            Self::DuplicateField { message, tag } => {
                write!(formatter, "duplicate {message} field tag {tag}")
            }
            Self::UnknownMutableRequestField { tag } => {
                write!(formatter, "unknown mutable request field tag {tag}")
            }
            Self::UnknownWriteCommand { tag } => {
                write!(formatter, "unknown write command tag {tag}")
            }
            Self::AmbiguousWriteCommand => {
                formatter.write_str("multiple write command arms are forbidden")
            }
            Self::MissingField { field } => write!(formatter, "missing required field {field}"),
            Self::InvalidFieldLength {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "field {field} must be {expected} bytes, got {actual}",
            ),
            Self::InvalidFieldValue { field, reason } => {
                write!(formatter, "invalid field {field}: {reason}")
            }
            Self::UnknownEnumValue { field, value } => {
                write!(formatter, "unknown {field} enum discriminant {value}")
            }
            Self::ProtocolVersionOutOfRange {
                field,
                major,
                minor,
            } => write!(
                formatter,
                "protocol version {field}={major}.{minor} exceeds u16 bounds",
            ),
            Self::ProtocolNameMismatch { actual } => {
                write!(formatter, "unexpected local protocol name {actual}")
            }
            Self::TooManyCapabilities { actual, maximum } => write!(
                formatter,
                "handshake has {actual} capabilities; maximum is {maximum}",
            ),
            Self::InvalidCapabilityId { capability } => {
                write!(formatter, "invalid capability identifier {capability}")
            }
            Self::UnknownWriteCapability { capability } => {
                write!(formatter, "unknown write capability {capability}")
            }
            Self::CapabilityCommandMismatch {
                capability,
                expected,
            } => write!(
                formatter,
                "write command requires {expected}, not {capability}",
            ),
            Self::CapabilityNotNegotiated { capability } => {
                write!(formatter, "capability was not negotiated: {capability}")
            }
            Self::WriteDenied {
                disposition,
                reason,
            } => write!(
                formatter,
                "handshake denied writes ({disposition}): {reason}",
            ),
            Self::InconsistentReceipt { field } => {
                write!(formatter, "receipt field {field} disagrees with response")
            }
            Self::InvalidAcceptanceRange { start, end } => {
                write!(formatter, "acceptance range is reversed: {start}..={end}")
            }
        }
    }
}

impl Error for RpcError {}

impl From<io::Error> for RpcError {
    fn from(value: io::Error) -> Self {
        Self::Io { kind: value.kind() }
    }
}

impl From<prost::DecodeError> for RpcError {
    fn from(value: prost::DecodeError) -> Self {
        Self::MalformedData {
            detail: value.to_string(),
        }
    }
}

impl From<prost::EncodeError> for RpcError {
    fn from(value: prost::EncodeError) -> Self {
        Self::EncodeFailure {
            detail: value.to_string(),
        }
    }
}
