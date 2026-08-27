//! Semantic conversion and strict closed-write wire validation.

use std::collections::BTreeSet;

use academic_domain::ContentDigest;

use crate::{
    PHASE1_PROTOCOL_POLICY,
    error::RpcError,
    generated::{
        self, LocalCoreEnvelope, MutableRequest, MutableResponse, MutationStatus, ProfileLockState,
        ServerHandshake, WriteDisposition, local_core_envelope, mutable_request,
    },
    handshake::{
        LOCAL_CORE_PROTOCOL_NAME, LOCAL_CORE_PROTOCOL_VERSION, MINIMUM_CLIENT_VERSION,
        PHASE1_CAPABILITY_IDS, READ_ONLY_CAPABILITY_IDS, WRITE_CAPABILITY_IDS,
        expected_capability_for_command, protocol_version_from_proto, validate_capability_list,
        validate_client_handshake,
    },
    limits::{
        FrameClass, IDEMPOTENCY_KEY_BYTES, MAX_DAEMON_BUILD_BYTES, MAX_PROJECTION_STATES,
        MAX_PROTOBUF_NESTING_DEPTH, MAX_RESPONSE_REASON_BYTES, MAX_SYNTHETIC_FIXTURE_ID_BYTES,
        OPAQUE_ID_BYTES, SHA256_DIGEST_BYTES,
    },
};

/// Exact 16-byte identifier with no numeric or textual narrowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpaqueId([u8; OPAQUE_ID_BYTES]);

impl OpaqueId {
    /// Constructs an opaque identifier without assigning ordering semantics.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; OPAQUE_ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact wire bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; OPAQUE_ID_BYTES] {
        &self.0
    }
}

/// Exact 32-byte retry key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IdempotencyKey([u8; IDEMPOTENCY_KEY_BYTES]);

impl IdempotencyKey {
    /// Constructs the key from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; IDEMPOTENCY_KEY_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact wire bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; IDEMPOTENCY_KEY_BYTES] {
        &self.0
    }
}

/// Validated inclusive replica-local acceptance sequence range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedAcceptanceRange {
    /// First accepted sequence.
    pub start: u64,
    /// Last accepted sequence.
    pub end: u64,
}

/// Closed mutable command after semantic conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedWriteCommand {
    /// Accept one repository-allowlisted synthetic fixture.
    SyntheticIngest { fixture_id: String },
    /// Create a synthetic-only backup.
    SyntheticBackup,
    /// Restore the synthetic-only backup identified by an immutable receipt.
    SyntheticRestore { backup_receipt_id: OpaqueId },
}

/// Mutable request whose exact-width and capability invariants passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMutableRequest {
    /// Request correlation ID.
    pub request_id: OpaqueId,
    /// Stable client-process instance ID.
    pub client_instance_id: OpaqueId,
    /// Exact retry key.
    pub idempotency_key: IdempotencyKey,
    /// SHA-256 digest of the request contract bytes.
    pub request_digest: ContentDigest,
    /// Optimistic concurrency guard, including an explicit zero when present.
    pub expected_profile_revision: Option<u64>,
    /// Capability bound to the selected command arm.
    pub capability_id: String,
    /// Known closed command.
    pub command: ValidatedWriteCommand,
}

/// Immutable receipt fields after cross-copy consistency checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedReceipt {
    /// Receipt identity.
    pub receipt_id: OpaqueId,
    /// Original request identity.
    pub request_id: OpaqueId,
    /// Original client instance identity.
    pub client_instance_id: OpaqueId,
    /// Original idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Original request digest.
    pub request_digest: ContentDigest,
    /// Committed profile revision.
    pub profile_revision: u64,
    /// Replica-local acceptance range when applicable.
    pub acceptance_range: Option<ValidatedAcceptanceRange>,
}

/// Mutable response whose receipt, revision, range, and digest are lossless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMutableResponse {
    /// Request correlation ID.
    pub request_id: OpaqueId,
    /// Closed status discriminant.
    pub status: MutationStatus,
    /// Stable machine-readable reason.
    pub reason: String,
    /// Immutable request/commit receipt.
    pub receipt: ValidatedReceipt,
    /// Current profile revision.
    pub profile_revision: u64,
    /// Replica-local acceptance range when applicable.
    pub acceptance_range: Option<ValidatedAcceptanceRange>,
    /// SHA-256 digest of the response contract bytes.
    pub response_digest: ContentDigest,
}

fn exact_array<const WIDTH: usize>(
    bytes: &[u8],
    field: &'static str,
) -> Result<[u8; WIDTH], RpcError> {
    let actual = bytes.len();
    bytes.try_into().map_err(|_| RpcError::InvalidFieldLength {
        field,
        expected: WIDTH,
        actual,
    })
}

fn opaque_id(bytes: &[u8], field: &'static str) -> Result<OpaqueId, RpcError> {
    Ok(OpaqueId::from_bytes(exact_array(bytes, field)?))
}

fn idempotency_key(bytes: &[u8], field: &'static str) -> Result<IdempotencyKey, RpcError> {
    Ok(IdempotencyKey::from_bytes(exact_array(bytes, field)?))
}

fn digest(bytes: &[u8], field: &'static str) -> Result<ContentDigest, RpcError> {
    Ok(ContentDigest::from_sha256_bytes(exact_array::<
        SHA256_DIGEST_BYTES,
    >(bytes, field)?))
}

fn validate_nonempty_bounded(
    value: &str,
    field: &'static str,
    maximum: usize,
) -> Result<(), RpcError> {
    if value.is_empty() || value.len() > maximum {
        return Err(RpcError::InvalidFieldValue {
            field,
            reason: "must be nonempty and within the fixed UTF-8 byte limit",
        });
    }
    Ok(())
}

fn acceptance_range(
    value: &generated::AcceptanceRange,
) -> Result<ValidatedAcceptanceRange, RpcError> {
    if value.accept_seq_start > value.accept_seq_end {
        return Err(RpcError::InvalidAcceptanceRange {
            start: value.accept_seq_start,
            end: value.accept_seq_end,
        });
    }
    Ok(ValidatedAcceptanceRange {
        start: value.accept_seq_start,
        end: value.accept_seq_end,
    })
}

/// Validates exact-width metadata and the closed capability/command binding.
pub fn validate_mutable_request(
    request: &MutableRequest,
) -> Result<ValidatedMutableRequest, RpcError> {
    let command = request.command.as_ref().ok_or(RpcError::MissingField {
        field: "mutable_request.command",
    })?;
    let expected_capability = expected_capability_for_command(command);
    if !WRITE_CAPABILITY_IDS.contains(&request.capability_id.as_str()) {
        return Err(RpcError::UnknownWriteCapability {
            capability: request.capability_id.clone(),
        });
    }
    if request.capability_id != expected_capability {
        return Err(RpcError::CapabilityCommandMismatch {
            capability: request.capability_id.clone(),
            expected: expected_capability,
        });
    }

    let command = match command {
        mutable_request::Command::SyntheticIngest(command) => {
            validate_nonempty_bounded(
                &command.synthetic_fixture_id,
                "mutable_request.synthetic_ingest.synthetic_fixture_id",
                MAX_SYNTHETIC_FIXTURE_ID_BYTES,
            )?;
            ValidatedWriteCommand::SyntheticIngest {
                fixture_id: command.synthetic_fixture_id.clone(),
            }
        }
        mutable_request::Command::SyntheticBackup(_) => ValidatedWriteCommand::SyntheticBackup,
        mutable_request::Command::SyntheticRestore(command) => {
            ValidatedWriteCommand::SyntheticRestore {
                backup_receipt_id: opaque_id(
                    &command.backup_receipt_id,
                    "mutable_request.synthetic_restore.backup_receipt_id",
                )?,
            }
        }
    };

    Ok(ValidatedMutableRequest {
        request_id: opaque_id(&request.request_id, "mutable_request.request_id")?,
        client_instance_id: opaque_id(
            &request.client_instance_id,
            "mutable_request.client_instance_id",
        )?,
        idempotency_key: idempotency_key(
            &request.idempotency_key,
            "mutable_request.idempotency_key",
        )?,
        request_digest: digest(&request.request_digest, "mutable_request.request_digest")?,
        expected_profile_revision: request.expected_profile_revision,
        capability_id: request.capability_id.clone(),
        command,
    })
}

fn validate_receipt(receipt: &generated::ImmutableReceipt) -> Result<ValidatedReceipt, RpcError> {
    Ok(ValidatedReceipt {
        receipt_id: opaque_id(&receipt.receipt_id, "receipt.receipt_id")?,
        request_id: opaque_id(&receipt.request_id, "receipt.request_id")?,
        client_instance_id: opaque_id(&receipt.client_instance_id, "receipt.client_instance_id")?,
        idempotency_key: idempotency_key(&receipt.idempotency_key, "receipt.idempotency_key")?,
        request_digest: digest(&receipt.request_digest, "receipt.request_digest")?,
        profile_revision: receipt.profile_revision,
        acceptance_range: receipt
            .acceptance_range
            .as_ref()
            .map(acceptance_range)
            .transpose()?,
    })
}

/// Validates response status and immutable receipt/revision/range copies.
pub fn validate_mutable_response(
    response: &MutableResponse,
) -> Result<ValidatedMutableResponse, RpcError> {
    let status =
        MutationStatus::try_from(response.status).map_err(|_| RpcError::UnknownEnumValue {
            field: "mutable_response.status",
            value: response.status,
        })?;
    if status == MutationStatus::Unspecified {
        return Err(RpcError::UnknownEnumValue {
            field: "mutable_response.status",
            value: response.status,
        });
    }
    validate_nonempty_bounded(
        &response.reason,
        "mutable_response.reason",
        MAX_RESPONSE_REASON_BYTES,
    )?;

    let request_id = opaque_id(&response.request_id, "mutable_response.request_id")?;
    let response_range = response
        .acceptance_range
        .as_ref()
        .map(acceptance_range)
        .transpose()?;
    let receipt = validate_receipt(response.receipt.as_ref().ok_or(RpcError::MissingField {
        field: "mutable_response.receipt",
    })?)?;
    if receipt.request_id != request_id {
        return Err(RpcError::InconsistentReceipt {
            field: "request_id",
        });
    }
    if receipt.profile_revision != response.profile_revision {
        return Err(RpcError::InconsistentReceipt {
            field: "profile_revision",
        });
    }
    if receipt.acceptance_range != response_range {
        return Err(RpcError::InconsistentReceipt {
            field: "acceptance_range",
        });
    }

    Ok(ValidatedMutableResponse {
        request_id,
        status,
        reason: response.reason.clone(),
        receipt,
        profile_revision: response.profile_revision,
        acceptance_range: response_range,
        response_digest: digest(
            &response.response_digest,
            "mutable_response.response_digest",
        )?,
    })
}

fn validate_server_handshake(server: &ServerHandshake) -> Result<(), RpcError> {
    if server.protocol_name != LOCAL_CORE_PROTOCOL_NAME {
        return Err(RpcError::ProtocolNameMismatch {
            actual: server.protocol_name.clone(),
        });
    }
    let protocol =
        protocol_version_from_proto(server.protocol_version.as_ref(), "server.protocol_version")?;
    let minimum = protocol_version_from_proto(
        server.minimum_client_version.as_ref(),
        "server.minimum_client_version",
    )?;
    if protocol != LOCAL_CORE_PROTOCOL_VERSION || minimum != MINIMUM_CLIENT_VERSION {
        return Err(RpcError::InvalidFieldValue {
            field: "server.protocol_versions",
            reason: "must match the frozen local v1 contract",
        });
    }
    validate_nonempty_bounded(
        &server.daemon_build,
        "server.daemon_build",
        MAX_DAEMON_BUILD_BYTES,
    )?;
    let storage = server
        .storage_schema
        .as_ref()
        .ok_or(RpcError::MissingField {
            field: "server.storage_schema",
        })?;
    if storage.number != 1 || storage.semantic_version != "1.0.0" {
        return Err(RpcError::InvalidFieldValue {
            field: "server.storage_schema",
            reason: "must be schema 1 / 1.0.0",
        });
    }
    if server.vault_read_formats.as_slice() != ["PLAINTEXT_SYNTHETIC_V1"]
        || server.vault_write_format != "PLAINTEXT_SYNTHETIC_V1"
    {
        return Err(RpcError::InvalidFieldValue {
            field: "server.vault_formats",
            reason: "must retain the Phase 1 plaintext synthetic format",
        });
    }
    if server.projections.len() > MAX_PROJECTION_STATES {
        return Err(RpcError::InvalidFieldValue {
            field: "server.projections",
            reason: "projection state count exceeds handshake limit",
        });
    }
    let mut projection_ids = BTreeSet::new();
    for projection in &server.projections {
        validate_nonempty_bounded(
            &projection.projection_id,
            "server.projection.projection_id",
            MAX_SYNTHETIC_FIXTURE_ID_BYTES,
        )?;
        if !projection_ids.insert(projection.projection_id.as_str()) {
            return Err(RpcError::InvalidFieldValue {
                field: "server.projections",
                reason: "projection identifiers must be unique",
            });
        }
        if projection.builder_schema_version == 0 {
            return Err(RpcError::InvalidFieldValue {
                field: "server.projection.builder_schema_version",
                reason: "must be positive",
            });
        }
        let _ = digest(
            &projection.builder_digest,
            "server.projection.builder_digest",
        )?;
    }
    let lock_state =
        ProfileLockState::try_from(server.lock_state).map_err(|_| RpcError::UnknownEnumValue {
            field: "server.lock_state",
            value: server.lock_state,
        })?;
    if lock_state == ProfileLockState::Unspecified {
        return Err(RpcError::UnknownEnumValue {
            field: "server.lock_state",
            value: server.lock_state,
        });
    }
    let policy = server.policy.as_ref().ok_or(RpcError::MissingField {
        field: "server.policy",
    })?;
    if policy.data_policy != PHASE1_PROTOCOL_POLICY.data_policy
        || policy.storage_mode != PHASE1_PROTOCOL_POLICY.storage_mode
        || policy.storage_encryption != PHASE1_PROTOCOL_POLICY.storage_encryption
        || policy.production_data_allowed != PHASE1_PROTOCOL_POLICY.production_data_allowed
        || policy.product_network != PHASE1_PROTOCOL_POLICY.product_network
    {
        return Err(RpcError::InvalidFieldValue {
            field: "server.policy",
            reason: "synthetic-only policy bytes drifted",
        });
    }

    validate_capability_list(&server.capability_ids)?;
    for capability in &server.capability_ids {
        if !PHASE1_CAPABILITY_IDS.contains(&capability.as_str()) {
            return Err(RpcError::InvalidCapabilityId {
                capability: capability.clone(),
            });
        }
    }
    for pair in server.capability_ids.windows(2) {
        let [left, right] = pair else {
            continue;
        };
        if left >= right {
            return Err(RpcError::InvalidFieldValue {
                field: "server.capability_ids",
                reason: "must be sorted and unique",
            });
        }
    }

    let disposition = WriteDisposition::try_from(server.write_disposition).map_err(|_| {
        RpcError::UnknownEnumValue {
            field: "server.write_disposition",
            value: server.write_disposition,
        }
    })?;
    let expected_reason = match disposition {
        WriteDisposition::Allowed => "",
        WriteDisposition::DeniedMajorVersion => "MAJOR_VERSION_MISMATCH",
        WriteDisposition::DeniedUnknownCapability => "UNKNOWN_WRITE_CAPABILITY",
        WriteDisposition::DeniedClientTooOld => "CLIENT_VERSION_BELOW_MINIMUM",
        WriteDisposition::Unspecified => {
            return Err(RpcError::UnknownEnumValue {
                field: "server.write_disposition",
                value: server.write_disposition,
            });
        }
    };
    if server.write_denial_reason != expected_reason {
        return Err(RpcError::InvalidFieldValue {
            field: "server.write_denial_reason",
            reason: "does not match the closed write disposition",
        });
    }
    if disposition == WriteDisposition::DeniedMajorVersion {
        if server.negotiated_protocol_version.is_some()
            || server
                .capability_ids
                .iter()
                .any(|value| !READ_ONLY_CAPABILITY_IDS.contains(&value.as_str()))
        {
            return Err(RpcError::InvalidFieldValue {
                field: "server.major_mismatch",
                reason: "may expose only bounded read-only capabilities",
            });
        }
    } else {
        let negotiated = protocol_version_from_proto(
            server.negotiated_protocol_version.as_ref(),
            "server.negotiated_protocol_version",
        )?;
        if negotiated.major != protocol.major || negotiated.minor > protocol.minor {
            return Err(RpcError::InvalidFieldValue {
                field: "server.negotiated_protocol_version",
                reason: "must be a same-major bounded minor",
            });
        }
    }
    Ok(())
}

/// Validates all semantic fields and returns the envelope's required frame cap.
pub fn validate_envelope(envelope: &LocalCoreEnvelope) -> Result<FrameClass, RpcError> {
    match envelope
        .payload
        .as_ref()
        .ok_or(RpcError::MissingEnvelopePayload)?
    {
        local_core_envelope::Payload::ClientHandshake(client) => {
            let _ = validate_client_handshake(client)?;
            Ok(FrameClass::Handshake)
        }
        local_core_envelope::Payload::ServerHandshake(server) => {
            validate_server_handshake(server)?;
            Ok(FrameClass::Handshake)
        }
        local_core_envelope::Payload::MutableRequest(request) => {
            let _ = validate_mutable_request(request)?;
            Ok(FrameClass::Command)
        }
        local_core_envelope::Payload::MutableResponse(response) => {
            let _ = validate_mutable_response(response)?;
            Ok(FrameClass::Command)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WireField<'a> {
    tag: u32,
    wire_type: u8,
    bytes: Option<&'a [u8]>,
}

fn malformed(detail: impl Into<String>) -> RpcError {
    RpcError::MalformedData {
        detail: detail.into(),
    }
}

fn read_varint(bytes: &[u8], cursor: &mut usize) -> Result<u64, RpcError> {
    let mut value = 0_u64;
    for octet_index in 0..10 {
        let octet = *bytes
            .get(*cursor)
            .ok_or_else(|| malformed("truncated varint"))?;
        *cursor += 1;
        if octet_index == 9 && octet > 1 {
            return Err(malformed("varint overflow"));
        }
        value |= u64::from(octet & 0x7f) << (octet_index * 7);
        if octet & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(malformed("unterminated varint"))
}

fn read_key(bytes: &[u8], cursor: &mut usize) -> Result<(u32, u8), RpcError> {
    let key = read_varint(bytes, cursor)?;
    let tag = key >> 3;
    if tag == 0 || tag > 0x1fff_ffff {
        return Err(malformed("invalid Protobuf field number"));
    }
    let tag = u32::try_from(tag).map_err(|_| malformed("field number overflow"))?;
    let wire_type = u8::try_from(key & 0x07).map_err(|_| malformed("wire type overflow"))?;
    Ok((tag, wire_type))
}

fn take_bytes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], RpcError> {
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| malformed("length-delimited field overflow"))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| malformed("truncated fixed or length-delimited field"))?;
    *cursor = end;
    Ok(value)
}

fn skip_group(
    bytes: &[u8],
    cursor: &mut usize,
    opening_tag: u32,
    depth: usize,
) -> Result<(), RpcError> {
    if depth > MAX_PROTOBUF_NESTING_DEPTH {
        return Err(RpcError::ProtobufNestingLimitExceeded {
            maximum: MAX_PROTOBUF_NESTING_DEPTH,
        });
    }
    loop {
        if *cursor == bytes.len() {
            return Err(malformed("unterminated legacy group"));
        }
        let (tag, wire_type) = read_key(bytes, cursor)?;
        match wire_type {
            3 => skip_group(bytes, cursor, tag, depth + 1)?,
            4 if tag == opening_tag => return Ok(()),
            4 => return Err(malformed("mismatched legacy group terminator")),
            _ => {
                let _ = read_wire_value(bytes, cursor, wire_type, tag, depth)?;
            }
        }
    }
}

fn read_wire_value<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    wire_type: u8,
    tag: u32,
    depth: usize,
) -> Result<Option<&'a [u8]>, RpcError> {
    match wire_type {
        0 => {
            let _ = read_varint(bytes, cursor)?;
            Ok(None)
        }
        1 => {
            let _ = take_bytes(bytes, cursor, 8)?;
            Ok(None)
        }
        2 => {
            let length = usize::try_from(read_varint(bytes, cursor)?)
                .map_err(|_| malformed("length-delimited field exceeds host size"))?;
            Ok(Some(take_bytes(bytes, cursor, length)?))
        }
        3 => {
            skip_group(bytes, cursor, tag, depth + 1)?;
            Ok(None)
        }
        4 => Err(malformed("unexpected legacy group terminator")),
        5 => {
            let _ = take_bytes(bytes, cursor, 4)?;
            Ok(None)
        }
        _ => Err(malformed("unknown Protobuf wire type")),
    }
}

fn next_wire_field<'a>(bytes: &'a [u8], cursor: &mut usize) -> Result<WireField<'a>, RpcError> {
    let (tag, wire_type) = read_key(bytes, cursor)?;
    let value = read_wire_value(bytes, cursor, wire_type, tag, 0)?;
    Ok(WireField {
        tag,
        wire_type,
        bytes: value,
    })
}

fn expect_wire(field: WireField<'_>, expected: u8) -> Result<(), RpcError> {
    if field.wire_type != expected {
        return Err(malformed(format!(
            "field {} used wire type {}, expected {}",
            field.tag, field.wire_type, expected
        )));
    }
    Ok(())
}

fn nested_bytes<'a>(field: WireField<'a>) -> Result<&'a [u8], RpcError> {
    expect_wire(field, 2)?;
    field
        .bytes
        .ok_or_else(|| malformed("length-delimited field had no payload"))
}

fn insert_once(seen: &mut BTreeSet<u32>, message: &'static str, tag: u32) -> Result<(), RpcError> {
    if !seen.insert(tag) {
        return Err(RpcError::DuplicateField { message, tag });
    }
    Ok(())
}

fn validate_ingest_command_wire(bytes: &[u8]) -> Result<(), RpcError> {
    let mut cursor = 0;
    let mut seen = BTreeSet::new();
    while cursor < bytes.len() {
        let field = next_wire_field(bytes, &mut cursor)?;
        if field.tag != 1 {
            return Err(RpcError::UnknownMutableRequestField { tag: field.tag });
        }
        insert_once(&mut seen, "SyntheticIngestCommand", field.tag)?;
        expect_wire(field, 2)?;
    }
    Ok(())
}

fn validate_backup_command_wire(bytes: &[u8]) -> Result<(), RpcError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let mut cursor = 0;
    let field = next_wire_field(bytes, &mut cursor)?;
    Err(RpcError::UnknownMutableRequestField { tag: field.tag })
}

fn validate_restore_command_wire(bytes: &[u8]) -> Result<(), RpcError> {
    let mut cursor = 0;
    let mut seen = BTreeSet::new();
    while cursor < bytes.len() {
        let field = next_wire_field(bytes, &mut cursor)?;
        if field.tag != 1 {
            return Err(RpcError::UnknownMutableRequestField { tag: field.tag });
        }
        insert_once(&mut seen, "SyntheticRestoreCommand", field.tag)?;
        expect_wire(field, 2)?;
    }
    Ok(())
}

fn validate_mutable_request_wire(bytes: &[u8]) -> Result<(), RpcError> {
    let mut cursor = 0;
    let mut seen = BTreeSet::new();
    let mut command_count = 0_usize;
    while cursor < bytes.len() {
        let field = next_wire_field(bytes, &mut cursor)?;
        insert_once(&mut seen, "MutableRequest", field.tag)?;
        match field.tag {
            1 | 2 | 3 | 4 | 6 => expect_wire(field, 2)?,
            5 => expect_wire(field, 0)?,
            10 => {
                command_count += 1;
                validate_ingest_command_wire(nested_bytes(field)?)?;
            }
            11 => {
                command_count += 1;
                validate_backup_command_wire(nested_bytes(field)?)?;
            }
            12 => {
                command_count += 1;
                validate_restore_command_wire(nested_bytes(field)?)?;
            }
            13..=31 => return Err(RpcError::UnknownWriteCommand { tag: field.tag }),
            _ => return Err(RpcError::UnknownMutableRequestField { tag: field.tag }),
        }
    }
    match command_count {
        0 => Err(RpcError::MissingField {
            field: "mutable_request.command",
        }),
        1 => Ok(()),
        _ => Err(RpcError::AmbiguousWriteCommand),
    }
}

/// Rejects unknown/duplicate envelope fields and any unknown write command tag.
///
/// Prost intentionally drops unknown fields. This bounded preflight therefore
/// runs on the original payload before generated decoding at the write boundary.
pub fn validate_closed_envelope_wire(bytes: &[u8]) -> Result<(), RpcError> {
    let mut cursor = 0;
    let mut payload_tag = None;
    while cursor < bytes.len() {
        let field = next_wire_field(bytes, &mut cursor)?;
        if !(1..=4).contains(&field.tag) {
            return Err(RpcError::UnknownEnvelopeField { tag: field.tag });
        }
        if payload_tag.replace(field.tag).is_some() {
            return Err(RpcError::AmbiguousEnvelopePayload);
        }
        let payload = nested_bytes(field)?;
        if field.tag == 3 {
            validate_mutable_request_wire(payload)?;
        }
    }
    payload_tag
        .map(|_| ())
        .ok_or(RpcError::MissingEnvelopePayload)
}
