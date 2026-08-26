//! Deterministic signed-batch contract.
//!
//! The signing surface is a strict CBOR profile made only of definite arrays,
//! integers, booleans, text, and bytes. JSON-shaped domain values are encoded
//! as tagged arrays, so map iteration order cannot affect signed bytes.

use std::io::Cursor;

use academic_domain::{ContentDigest, DomainError};
use academic_ledger::{LedgerError, UnsignedBatch};
use ciborium::value::{Integer, Value as CborValue};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use thiserror::Error;

/// Signed-envelope format version.
pub const SIGNED_ENVELOPE_VERSION: u16 = 1;

const JSON_NULL: u64 = 0;
const JSON_BOOL: u64 = 1;
const JSON_NUMBER: u64 = 2;
const JSON_STRING: u64 = 3;
const JSON_ARRAY: u64 = 4;
const JSON_OBJECT: u64 = 5;

/// Verified and decoded batch plus both semantic and envelope digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBatch {
    pub batch: UnsignedBatch,
    pub public_key: VerifyingKey,
    pub payload_hash: ContentDigest,
    pub envelope_hash: ContentDigest,
}

#[derive(Debug)]
struct DecodedEnvelope {
    payload: Vec<u8>,
    public_key: Vec<u8>,
    signature: Vec<u8>,
}

/// Contract encoding or verification failure.
#[derive(Debug, Error)]
pub enum ContractError {
    /// A domain invariant failed before or after transport.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A ledger semantic invariant failed before or after transport.
    #[error(transparent)]
    Ledger(#[from] LedgerError),
    /// JSON conversion failed.
    #[error("domain JSON conversion failed: {0}")]
    Json(#[from] serde_json::Error),
    /// CBOR serialization failed.
    #[error("CBOR encoding failed: {0}")]
    CborEncode(String),
    /// CBOR parsing failed.
    #[error("CBOR decoding failed: {0}")]
    CborDecode(String),
    /// A strict array or tagged value had the wrong shape.
    #[error("invalid deterministic CBOR shape: {0}")]
    InvalidShape(&'static str),
    /// A numeric JSON value was outside the integer-only contract.
    #[error("unsupported JSON number in deterministic contract: {0}")]
    UnsupportedNumber(String),
    /// An encoded number could not be parsed back to JSON.
    #[error("invalid encoded JSON number: {0}")]
    InvalidNumber(String),
    /// Bytes were well-formed but not the one canonical encoding.
    #[error("non-canonical deterministic CBOR bytes")]
    NonCanonicalEncoding,
    /// Extra bytes followed the single CBOR value.
    #[error("trailing bytes after CBOR value")]
    TrailingBytes,
    /// The signed envelope version is not supported.
    #[error("unsupported signed envelope version {0}")]
    UnsupportedEnvelopeVersion(u16),
    /// A public key or signature did not have the required length.
    #[error("invalid Ed25519 {kind} length: {actual}")]
    InvalidCryptoLength { kind: &'static str, actual: usize },
    /// The embedded key was not the independently expected device key.
    #[error("embedded signing key does not match the expected device key")]
    UnexpectedSigningKey,
    /// Ed25519 verification failed.
    #[error("Ed25519 signature verification failed")]
    InvalidSignature,
}

/// Encodes and signs a batch with an explicit Ed25519 key.
pub fn sign_batch(
    batch: &UnsignedBatch,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, ContractError> {
    let payload = encode_unsigned_batch(batch)?;
    let signature = signing_key.sign(&payload);
    encode_envelope(
        &payload,
        signing_key.verifying_key().as_bytes(),
        &signature.to_bytes(),
    )
}

/// Verifies canonical bytes, the independently anchored key, the signature,
/// and all nested semantic invariants before returning a batch.
pub fn verify_signed_batch(
    envelope_bytes: &[u8],
    expected_key: &VerifyingKey,
) -> Result<VerifiedBatch, ContractError> {
    let decoded = decode_envelope(envelope_bytes)?;
    let payload = decoded.payload;
    let public_key_bytes = decoded.public_key;
    let signature_bytes = decoded.signature;
    let public_key_array: [u8; 32] =
        public_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::InvalidCryptoLength {
                kind: "public key",
                actual: public_key_bytes.len(),
            })?;
    let public_key =
        VerifyingKey::from_bytes(&public_key_array).map_err(|_| ContractError::InvalidSignature)?;
    if public_key != *expected_key {
        return Err(ContractError::UnexpectedSigningKey);
    }
    let signature_array: [u8; 64] =
        signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::InvalidCryptoLength {
                kind: "signature",
                actual: signature_bytes.len(),
            })?;
    let signature = Signature::from_bytes(&signature_array);
    public_key
        .verify(&payload, &signature)
        .map_err(|_| ContractError::InvalidSignature)?;

    let batch = decode_unsigned_batch(&payload)?;
    Ok(VerifiedBatch {
        batch,
        public_key,
        payload_hash: ContentDigest::sha256(&payload),
        envelope_hash: ContentDigest::sha256(envelope_bytes),
    })
}

/// Encodes an unsigned batch into the deterministic CBOR signing payload.
pub fn encode_unsigned_batch(batch: &UnsignedBatch) -> Result<Vec<u8>, ContractError> {
    batch.validate()?;
    let json = serde_json::to_value(batch)?;
    encode_cbor_value(&json_to_cbor(&json)?)
}

/// Decodes and canonicality-checks an unsigned signing payload.
pub fn decode_unsigned_batch(bytes: &[u8]) -> Result<UnsignedBatch, ContractError> {
    let cbor = decode_single_cbor(bytes)?;
    let json = cbor_to_json(&cbor)?;
    let batch: UnsignedBatch = serde_json::from_value(json)?;
    batch.validate()?;
    if encode_unsigned_batch(&batch)? != bytes {
        return Err(ContractError::NonCanonicalEncoding);
    }
    Ok(batch)
}

fn encode_envelope(
    payload: &[u8],
    public_key: &[u8; 32],
    signature: &[u8; 64],
) -> Result<Vec<u8>, ContractError> {
    let envelope = CborValue::Array(vec![
        CborValue::Integer(Integer::from(u64::from(SIGNED_ENVELOPE_VERSION))),
        CborValue::Bytes(payload.to_vec()),
        CborValue::Bytes(public_key.to_vec()),
        CborValue::Bytes(signature.to_vec()),
    ]);
    encode_cbor_value(&envelope)
}

fn decode_envelope(bytes: &[u8]) -> Result<DecodedEnvelope, ContractError> {
    let cbor = decode_single_cbor(bytes)?;
    let CborValue::Array(mut fields) = cbor else {
        return Err(ContractError::InvalidShape("envelope must be an array"));
    };
    if fields.len() != 4 {
        return Err(ContractError::InvalidShape(
            "envelope array must contain four fields",
        ));
    }
    let signature = take_bytes(fields.pop(), "signature must be bytes")?;
    let public_key = take_bytes(fields.pop(), "public key must be bytes")?;
    let payload = take_bytes(fields.pop(), "payload must be bytes")?;
    let version = take_u64(fields.pop(), "envelope version must be an integer")?;
    let version = u16::try_from(version)
        .map_err(|_| ContractError::InvalidShape("envelope version exceeds u16"))?;
    if version != SIGNED_ENVELOPE_VERSION {
        return Err(ContractError::UnsupportedEnvelopeVersion(version));
    }
    if encode_envelope(
        &payload,
        public_key
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::InvalidCryptoLength {
                kind: "public key",
                actual: public_key.len(),
            })?,
        signature
            .as_slice()
            .try_into()
            .map_err(|_| ContractError::InvalidCryptoLength {
                kind: "signature",
                actual: signature.len(),
            })?,
    )? != bytes
    {
        return Err(ContractError::NonCanonicalEncoding);
    }
    Ok(DecodedEnvelope {
        payload,
        public_key,
        signature,
    })
}

fn encode_cbor_value(value: &CborValue) -> Result<Vec<u8>, ContractError> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes)
        .map_err(|error| ContractError::CborEncode(error.to_string()))?;
    Ok(bytes)
}

fn decode_single_cbor(bytes: &[u8]) -> Result<CborValue, ContractError> {
    let mut cursor = Cursor::new(bytes);
    let value = ciborium::de::from_reader(&mut cursor)
        .map_err(|error| ContractError::CborDecode(error.to_string()))?;
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| ContractError::InvalidShape("input length exceeds u64"))?
    {
        return Err(ContractError::TrailingBytes);
    }
    Ok(value)
}

fn json_to_cbor(value: &JsonValue) -> Result<CborValue, ContractError> {
    let tagged = match value {
        JsonValue::Null => vec![integer(JSON_NULL)],
        JsonValue::Bool(value) => vec![integer(JSON_BOOL), CborValue::Bool(*value)],
        JsonValue::Number(value) => {
            if !(value.is_i64() || value.is_u64()) {
                return Err(ContractError::UnsupportedNumber(value.to_string()));
            }
            vec![integer(JSON_NUMBER), CborValue::Text(value.to_string())]
        }
        JsonValue::String(value) => {
            vec![integer(JSON_STRING), CborValue::Text(value.clone())]
        }
        JsonValue::Array(values) => vec![
            integer(JSON_ARRAY),
            CborValue::Array(
                values
                    .iter()
                    .map(json_to_cbor)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        ],
        JsonValue::Object(values) => {
            let mut entries: Vec<(&String, &JsonValue)> = values.iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
            let encoded_entries = entries
                .into_iter()
                .map(|(key, nested)| {
                    Ok(CborValue::Array(vec![
                        CborValue::Text(key.clone()),
                        json_to_cbor(nested)?,
                    ]))
                })
                .collect::<Result<Vec<_>, ContractError>>()?;
            vec![integer(JSON_OBJECT), CborValue::Array(encoded_entries)]
        }
    };
    Ok(CborValue::Array(tagged))
}

fn cbor_to_json(value: &CborValue) -> Result<JsonValue, ContractError> {
    let CborValue::Array(fields) = value else {
        return Err(ContractError::InvalidShape(
            "tagged JSON value must be an array",
        ));
    };
    let Some(tag) = fields.first() else {
        return Err(ContractError::InvalidShape("tagged JSON array is empty"));
    };
    let tag = value_u64(tag, "tag must be an unsigned integer")?;
    match tag {
        JSON_NULL if fields.len() == 1 => Ok(JsonValue::Null),
        JSON_BOOL if fields.len() == 2 => match &fields[1] {
            CborValue::Bool(value) => Ok(JsonValue::Bool(*value)),
            _ => Err(ContractError::InvalidShape("boolean payload is invalid")),
        },
        JSON_NUMBER if fields.len() == 2 => match &fields[1] {
            CborValue::Text(value) => {
                let number = value
                    .parse::<JsonNumber>()
                    .map_err(|_| ContractError::InvalidNumber(value.clone()))?;
                if !(number.is_i64() || number.is_u64()) {
                    return Err(ContractError::InvalidNumber(value.clone()));
                }
                Ok(JsonValue::Number(number))
            }
            _ => Err(ContractError::InvalidShape("number payload is invalid")),
        },
        JSON_STRING if fields.len() == 2 => match &fields[1] {
            CborValue::Text(value) => Ok(JsonValue::String(value.clone())),
            _ => Err(ContractError::InvalidShape("string payload is invalid")),
        },
        JSON_ARRAY if fields.len() == 2 => match &fields[1] {
            CborValue::Array(values) => Ok(JsonValue::Array(
                values
                    .iter()
                    .map(cbor_to_json)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            _ => Err(ContractError::InvalidShape("array payload is invalid")),
        },
        JSON_OBJECT if fields.len() == 2 => decode_json_object(&fields[1]),
        _ => Err(ContractError::InvalidShape(
            "unknown tag or wrong tagged-array length",
        )),
    }
}

fn decode_json_object(value: &CborValue) -> Result<JsonValue, ContractError> {
    let CborValue::Array(entries) = value else {
        return Err(ContractError::InvalidShape(
            "object entries must be an array",
        ));
    };
    let mut result = JsonMap::new();
    let mut previous_key: Option<&str> = None;
    for entry in entries {
        let CborValue::Array(pair) = entry else {
            return Err(ContractError::InvalidShape("object entry must be a pair"));
        };
        if pair.len() != 2 {
            return Err(ContractError::InvalidShape(
                "object entry must contain two fields",
            ));
        }
        let CborValue::Text(key) = &pair[0] else {
            return Err(ContractError::InvalidShape("object key must be text"));
        };
        if previous_key.is_some_and(|previous| previous.as_bytes() >= key.as_bytes()) {
            return Err(ContractError::NonCanonicalEncoding);
        }
        previous_key = Some(key);
        let nested = cbor_to_json(&pair[1])?;
        if result.insert(key.clone(), nested).is_some() {
            return Err(ContractError::NonCanonicalEncoding);
        }
    }
    Ok(JsonValue::Object(result))
}

fn integer(value: u64) -> CborValue {
    CborValue::Integer(Integer::from(value))
}

fn value_u64(value: &CborValue, error: &'static str) -> Result<u64, ContractError> {
    match value {
        CborValue::Integer(value) => {
            u64::try_from(*value).map_err(|_| ContractError::InvalidShape(error))
        }
        _ => Err(ContractError::InvalidShape(error)),
    }
}

fn take_u64(value: Option<CborValue>, error: &'static str) -> Result<u64, ContractError> {
    value
        .as_ref()
        .ok_or(ContractError::InvalidShape(error))
        .and_then(|value| value_u64(value, error))
}

fn take_bytes(value: Option<CborValue>, error: &'static str) -> Result<Vec<u8>, ContractError> {
    match value {
        Some(CborValue::Bytes(value)) => Ok(value),
        _ => Err(ContractError::InvalidShape(error)),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use academic_domain::{
        Actor, BatchId, DeviceId, DomainId, EventId, EventPayload, TimestampMillis,
    };
    use academic_ledger::{EVENT_SCHEMA_VERSION, event};

    use super::*;

    fn minimal_batch() -> Result<UnsignedBatch, DomainError> {
        let domain_id = DomainId::from_str("01900000-0000-7000-8000-000000000001")?;
        Ok(UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: BatchId::from_str("01900000-0000-7000-8000-000000000002")?,
            device_id: DeviceId::from_str("01900000-0000-7000-8000-000000000003")?,
            origin_seq_start: 1,
            origin_seq_end: 1,
            previous_batch_hash: None,
            origin_created_at: TimestampMillis::new(10),
            events: vec![event(
                EventId::from_str("01900000-0000-7000-8000-000000000004")?,
                1,
                TimestampMillis::new(9),
                Actor::User,
                domain_id,
                EventPayload::DecisionRecorded(academic_domain::UserDecision {
                    id: academic_domain::DecisionId::from_str(
                        "01900000-0000-7000-8000-000000000005",
                    )?,
                    target_claim_id: academic_domain::ClaimId::from_str(
                        "01900000-0000-7000-8000-000000000006",
                    )?,
                    action: academic_domain::DecisionAction::Reject,
                    scope_id: None,
                    rationale_evidence_ids: Vec::new(),
                    decided_at: TimestampMillis::new(9),
                    reversible_until: None,
                }),
            )],
        })
    }

    #[test]
    fn encoding_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let first = encode_unsigned_batch(&batch)?;
        let second = encode_unsigned_batch(&batch)?;
        assert_eq!(first, second);
        assert_eq!(decode_unsigned_batch(&first)?, batch);
        Ok(())
    }

    #[test]
    fn signature_and_expected_key_are_both_required() -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let other_key = SigningKey::from_bytes(&[8_u8; 32]).verifying_key();
        let signed = sign_batch(&batch, &signing_key)?;
        assert!(matches!(
            verify_signed_batch(&signed, &other_key),
            Err(ContractError::UnexpectedSigningKey)
        ));
        let verified = verify_signed_batch(&signed, &signing_key.verifying_key())?;
        assert_eq!(verified.batch, batch);
        Ok(())
    }

    #[test]
    fn tampering_fails_signature_verification() -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let mut signed = sign_batch(&batch, &signing_key)?;
        let middle = signed.len() / 2;
        signed[middle] ^= 1;
        assert!(verify_signed_batch(&signed, &signing_key.verifying_key()).is_err());
        Ok(())
    }
}
