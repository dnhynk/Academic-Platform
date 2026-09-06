//! Stable mutable-request digest shared by clients and the sole writer.

use crate::{
    RpcError,
    convert::{ValidatedWriteCommand, validate_mutable_request},
    generated::MutableRequest,
};
use academic_domain::ContentDigest;

const REQUEST_DIGEST_DOMAIN: &[u8] = b"academic.local-mutable-request.v1\0";

/// Computes the stable non-self-referential digest of a P1 mutable request.
pub fn mutable_request_digest(request: &MutableRequest) -> Result<ContentDigest, RpcError> {
    let mut candidate = request.clone();
    candidate.request_digest = vec![0; 32];
    let validated = validate_mutable_request(&candidate)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(REQUEST_DIGEST_DOMAIN);
    bytes.extend_from_slice(validated.request_id.as_bytes());
    bytes.extend_from_slice(validated.client_instance_id.as_bytes());
    bytes.extend_from_slice(validated.idempotency_key.as_bytes());
    append_optional_u64(&mut bytes, validated.expected_profile_revision);
    append_bytes(&mut bytes, validated.capability_id.as_bytes());
    match validated.command {
        ValidatedWriteCommand::SyntheticIngest { fixture_id } => {
            bytes.push(1);
            append_bytes(&mut bytes, fixture_id.as_bytes());
        }
        ValidatedWriteCommand::SyntheticBackup => bytes.push(2),
        ValidatedWriteCommand::SyntheticRestore { backup_receipt_id } => {
            bytes.push(3);
            bytes.extend_from_slice(backup_receipt_id.as_bytes());
        }
    }
    Ok(ContentDigest::sha256(&bytes))
}

fn append_optional_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    target.extend_from_slice(value);
}
