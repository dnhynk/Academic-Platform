use academic_rpc::{
    FrameClass, committed_proto_codegen_matches,
    convert::{ValidatedWriteCommand, validate_mutable_request, validate_mutable_response},
    decode_envelope_frame, encode_envelope_frame,
    generated::{
        AcceptanceRange, ImmutableReceipt, LocalCoreEnvelope, MutableRequest, MutableResponse,
        MutationStatus, SyntheticIngestCommand, local_core_envelope, mutable_request,
    },
};

const REQUEST_FRAME_HEX: &str = "000000b61ab3010a10000102030405060708090a0b0c0d0e0f1210101112131415161718191a1b1c1d1e1f1a20202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f2220404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f28ffffffffffffffffff01322b6c6561726e696e672d706c6174666f726d2e6c6f63616c2e73796e7468657469632d696e676573742e763152110a0f7369676e65642d62617463682d7632";
const RESPONSE_FRAME_HEX: &str = "000001062283020a10000102030405060708090a0b0c0d0e0f10011a084143434550544544229d010a10606162636465666768696a6b6c6d6e6f1210000102030405060708090a0b0c0d0e0f1a10101112131415161718191a1b1c1d1e1f2220202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f2a20404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f30ffffffffffffffffff013a1608feffffffffffffffff0110ffffffffffffffffff0128ffffffffffffffffff01321608feffffffffffffffff0110ffffffffffffffffff013a20808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";

fn byte_sequence(start: u8, length: usize) -> Vec<u8> {
    (start..=u8::MAX).take(length).collect()
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn request_envelope() -> LocalCoreEnvelope {
    LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableRequest(
            MutableRequest {
                request_id: byte_sequence(0, 16),
                client_instance_id: byte_sequence(16, 16),
                idempotency_key: byte_sequence(32, 32),
                request_digest: byte_sequence(64, 32),
                expected_profile_revision: Some(u64::MAX),
                capability_id: "learning-platform.local.synthetic-ingest.v1".to_owned(),
                command: Some(mutable_request::Command::SyntheticIngest(
                    SyntheticIngestCommand {
                        synthetic_fixture_id: "signed-batch-v2".to_owned(),
                    },
                )),
            },
        )),
    }
}

fn response_envelope() -> LocalCoreEnvelope {
    let acceptance_range = AcceptanceRange {
        accept_seq_start: u64::MAX - 1,
        accept_seq_end: u64::MAX,
    };
    LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableResponse(
            MutableResponse {
                request_id: byte_sequence(0, 16),
                status: MutationStatus::Accepted as i32,
                reason: "ACCEPTED".to_owned(),
                receipt: Some(ImmutableReceipt {
                    receipt_id: byte_sequence(96, 16),
                    request_id: byte_sequence(0, 16),
                    client_instance_id: byte_sequence(16, 16),
                    idempotency_key: byte_sequence(32, 32),
                    request_digest: byte_sequence(64, 32),
                    profile_revision: u64::MAX,
                    acceptance_range: Some(acceptance_range),
                }),
                profile_revision: u64::MAX,
                acceptance_range: Some(acceptance_range),
                response_digest: byte_sequence(128, 32),
            },
        )),
    }
}

#[test]
fn ipc_golden_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let source = request_envelope();
    let frame = encode_envelope_frame(&source, FrameClass::Command)?;
    assert_eq!(lowercase_hex(&frame), REQUEST_FRAME_HEX);
    let decoded = decode_envelope_frame(&frame, FrameClass::Command)?;
    assert_eq!(decoded, source);

    let Some(local_core_envelope::Payload::MutableRequest(request)) = decoded.payload else {
        return Err("golden decoded into the wrong envelope arm".into());
    };
    let validated = validate_mutable_request(&request)?;
    assert_eq!(validated.expected_profile_revision, Some(u64::MAX));
    assert_eq!(
        &validated.idempotency_key.as_bytes()[..],
        byte_sequence(32, 32).as_slice()
    );
    assert_eq!(
        &validated.request_digest.as_bytes()[..],
        byte_sequence(64, 32).as_slice()
    );
    assert_eq!(
        validated.command,
        ValidatedWriteCommand::SyntheticIngest {
            fixture_id: "signed-batch-v2".to_owned(),
        }
    );
    Ok(())
}

#[test]
fn receipt_fields_are_lossless() -> Result<(), Box<dyn std::error::Error>> {
    let source = response_envelope();
    let frame = encode_envelope_frame(&source, FrameClass::Command)?;
    assert_eq!(lowercase_hex(&frame), RESPONSE_FRAME_HEX);
    let decoded = decode_envelope_frame(&frame, FrameClass::Command)?;
    assert_eq!(decoded, source);

    let Some(local_core_envelope::Payload::MutableResponse(response)) = decoded.payload else {
        return Err("receipt golden decoded into the wrong envelope arm".into());
    };
    let validated = validate_mutable_response(&response)?;
    assert_eq!(validated.profile_revision, u64::MAX);
    assert_eq!(validated.receipt.profile_revision, u64::MAX);
    assert_eq!(
        &validated.receipt.receipt_id.as_bytes()[..],
        byte_sequence(96, 16).as_slice()
    );
    assert_eq!(
        &validated.receipt.idempotency_key.as_bytes()[..],
        byte_sequence(32, 32).as_slice()
    );
    assert_eq!(
        &validated.receipt.request_digest.as_bytes()[..],
        byte_sequence(64, 32).as_slice()
    );
    assert_eq!(
        &validated.response_digest.as_bytes()[..],
        byte_sequence(128, 32).as_slice()
    );
    let range = validated
        .acceptance_range
        .ok_or("golden acceptance range was lost")?;
    assert_eq!(range.start, u64::MAX - 1);
    assert_eq!(range.end, u64::MAX);
    assert_eq!(validated.receipt.acceptance_range, Some(range));
    Ok(())
}

#[test]
fn proto_codegen_has_no_drift() {
    assert!(committed_proto_codegen_matches());
}

#[test]
fn optional_expected_revision_preserves_explicit_zero() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = request_envelope();
    let Some(local_core_envelope::Payload::MutableRequest(request)) = source.payload.as_mut()
    else {
        return Err("request fixture selected the wrong envelope arm".into());
    };
    request.expected_profile_revision = Some(0);
    let frame = encode_envelope_frame(&source, FrameClass::Command)?;
    let decoded = decode_envelope_frame(&frame, FrameClass::Command)?;
    let Some(local_core_envelope::Payload::MutableRequest(request)) = decoded.payload else {
        return Err("request fixture decoded into the wrong envelope arm".into());
    };
    assert_eq!(request.expected_profile_revision, Some(0));
    Ok(())
}
