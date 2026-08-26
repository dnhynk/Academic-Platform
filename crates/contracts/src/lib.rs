//! Deterministic signed-batch contract.
//!
//! The signing surface is a strict CBOR profile made only of definite arrays,
//! integers, booleans, text, and bytes. JSON-shaped domain values are encoded
//! as tagged arrays, so map iteration order cannot affect signed bytes.

use std::{collections::BTreeMap, io::Cursor};

use academic_domain::{
    Actor, ContentDigest, DeviceId, DomainError, EVENT_SCHEMA_VERSION_V1, EVENT_SCHEMA_VERSION_V2,
    EntityId, UnsignedBatch,
};
use ciborium::value::{Integer, Value as CborValue};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use thiserror::Error;

mod proto_contract;

pub use proto_contract::{
    ProtoContractError, decode_claim_relation_event_proto, encode_claim_relation_event_proto,
};

/// Signed-envelope format version.
pub const SIGNED_ENVELOPE_VERSION: u16 = 1;

const JSON_NULL: u64 = 0;
const JSON_BOOL: u64 = 1;
const JSON_NUMBER: u64 = 2;
const JSON_STRING: u64 = 3;
const JSON_ARRAY: u64 = 4;
const JSON_OBJECT: u64 = 5;

/// Verified and decoded batch plus both semantic and envelope digests.
///
/// Callers cannot construct this acceptance capability:
///
/// ```compile_fail
/// use academic_contracts::VerifiedBatch;
/// use academic_domain::{ContentDigest, UnsignedBatch};
/// use ed25519_dalek::VerifyingKey;
///
/// fn forge(batch: UnsignedBatch, public_key: VerifyingKey, hash: ContentDigest) -> VerifiedBatch {
///     VerifiedBatch {
///         batch,
///         public_key,
///         source_schema_version: 2,
///         payload_hash: hash,
///         envelope_hash: hash,
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBatch {
    batch: UnsignedBatch,
    public_key: VerifyingKey,
    source_schema_version: u16,
    payload_hash: ContentDigest,
    envelope_hash: ContentDigest,
}

impl VerifiedBatch {
    /// Returns the fully revalidated semantic batch.
    #[must_use]
    pub const fn batch(&self) -> &UnsignedBatch {
        &self.batch
    }

    /// Returns the independently expected key that authenticated the batch.
    #[must_use]
    pub const fn public_key(&self) -> &VerifyingKey {
        &self.public_key
    }

    /// Returns the schema version authenticated in the original signed bytes.
    #[must_use]
    pub const fn source_schema_version(&self) -> u16 {
        self.source_schema_version
    }

    /// Returns the digest of the canonical signing payload.
    #[must_use]
    pub const fn payload_hash(&self) -> ContentDigest {
        self.payload_hash
    }

    /// Returns the digest of the complete canonical signed envelope.
    #[must_use]
    pub const fn envelope_hash(&self) -> ContentDigest {
        self.envelope_hash
    }
}

/// Independent trust anchor binding a signing key to a device and user identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAuthorization {
    device_id: DeviceId,
    user_id: EntityId,
    verifying_key: VerifyingKey,
}

impl DeviceAuthorization {
    /// Constructs an authorization from configuration outside the signed envelope.
    #[must_use]
    pub const fn new(device_id: DeviceId, user_id: EntityId, verifying_key: VerifyingKey) -> Self {
        Self {
            device_id,
            user_id,
            verifying_key,
        }
    }

    /// Returns the authorized device identity.
    #[must_use]
    pub const fn device_id(&self) -> DeviceId {
        self.device_id
    }

    /// Returns the user identity whose explicit actions this device may attest.
    #[must_use]
    pub const fn user_id(&self) -> EntityId {
        self.user_id
    }

    /// Returns the independently configured verification key.
    #[must_use]
    pub const fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
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
    /// The signed batch named a different device than the key's authorization.
    #[error("signed batch device does not match the authorized device")]
    UnexpectedDeviceId,
    /// A user actor did not match the identity authorized for this device.
    #[error("signed user actor does not match the authorized user")]
    UnexpectedUserActor,
    /// Ed25519 verification failed.
    #[error("Ed25519 signature verification failed")]
    InvalidSignature,
    /// A legacy v1 payload could not be deterministically mapped into v2 semantics.
    #[error("legacy event-schema compatibility failure: {0}")]
    LegacyCompatibility(&'static str),
    /// Typed decoding would discard or normalize bytes covered by the signature.
    #[error("typed signed-batch decoding would discard or normalize authenticated fields")]
    AuthenticatedFieldDiscarded,
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
    authorization: &DeviceAuthorization,
) -> Result<VerifiedBatch, ContractError> {
    let decoded = decode_envelope(envelope_bytes)?;
    let payload = decoded.payload;
    let json = decode_canonical_payload_json(&payload)?;
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
    if public_key != *authorization.verifying_key() {
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

    let source_schema_version = read_schema_version(&json)?;
    let batch = decode_source_payload(json, source_schema_version)?;
    require_source_typed_equality(&batch, source_schema_version, &payload)?;
    if batch.device_id != authorization.device_id() {
        return Err(ContractError::UnexpectedDeviceId);
    }
    if batch.events.iter().any(|event| {
        matches!(
            &event.actor,
            Actor::User { user_id } if *user_id != authorization.user_id()
        )
    }) {
        return Err(ContractError::UnexpectedUserActor);
    }
    Ok(VerifiedBatch {
        batch,
        public_key,
        source_schema_version,
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

fn encode_unsigned_batch_v1_projection(batch: &UnsignedBatch) -> Result<Vec<u8>, ContractError> {
    batch.validate()?;
    let mut json = serde_json::to_value(batch)?;
    transform_decisions_for_v1(&mut json)?;
    set_schema_version(&mut json, EVENT_SCHEMA_VERSION_V1)?;
    encode_cbor_value(&json_to_cbor(&json)?)
}

/// Decodes and canonicality-checks an unsigned signing payload.
pub fn decode_unsigned_batch(bytes: &[u8]) -> Result<UnsignedBatch, ContractError> {
    decode_unsigned_batch_with_source(bytes).map(|(batch, _)| batch)
}

fn decode_unsigned_batch_with_source(bytes: &[u8]) -> Result<(UnsignedBatch, u16), ContractError> {
    let json = decode_canonical_payload_json(bytes)?;
    let source_schema_version = read_schema_version(&json)?;
    let batch = decode_source_payload(json, source_schema_version)?;
    require_source_typed_equality(&batch, source_schema_version, bytes)?;
    Ok((batch, source_schema_version))
}

fn decode_canonical_payload_json(bytes: &[u8]) -> Result<JsonValue, ContractError> {
    let cbor = decode_single_cbor(bytes)?;
    let json = cbor_to_json(&cbor)?;
    if encode_cbor_value(&json_to_cbor(&json)?)? != bytes {
        return Err(ContractError::NonCanonicalEncoding);
    }
    Ok(json)
}

fn decode_source_payload(
    mut json: JsonValue,
    source_schema_version: u16,
) -> Result<UnsignedBatch, ContractError> {
    match source_schema_version {
        EVENT_SCHEMA_VERSION_V1 => {
            upcast_v1_to_v2(&mut json)?;
            set_schema_version(&mut json, EVENT_SCHEMA_VERSION_V2)?;
        }
        EVENT_SCHEMA_VERSION_V2 => {}
        other => return Err(DomainError::UnsupportedSchemaVersion(other).into()),
    }
    let batch: UnsignedBatch = serde_json::from_value(json)?;
    batch.validate()?;
    Ok(batch)
}

fn require_source_typed_equality(
    batch: &UnsignedBatch,
    source_schema_version: u16,
    original_bytes: &[u8],
) -> Result<(), ContractError> {
    let typed_bytes = match source_schema_version {
        EVENT_SCHEMA_VERSION_V1 => encode_unsigned_batch_v1_projection(batch)?,
        EVENT_SCHEMA_VERSION_V2 => encode_unsigned_batch(batch)?,
        other => return Err(DomainError::UnsupportedSchemaVersion(other).into()),
    };
    if typed_bytes != original_bytes {
        return Err(ContractError::AuthenticatedFieldDiscarded);
    }
    Ok(())
}

fn read_schema_version(json: &JsonValue) -> Result<u16, ContractError> {
    json.get("schema_version")
        .and_then(JsonValue::as_u64)
        .and_then(|version| u16::try_from(version).ok())
        .ok_or(ContractError::InvalidShape(
            "batch schema_version must be a u16 integer",
        ))
}

fn set_schema_version(json: &mut JsonValue, version: u16) -> Result<(), ContractError> {
    let object = json.as_object_mut().ok_or(ContractError::InvalidShape(
        "batch must be a JSON-shaped object",
    ))?;
    object.insert(
        "schema_version".to_owned(),
        JsonValue::Number(JsonNumber::from(version)),
    );
    Ok(())
}

fn claim_index(json: &JsonValue) -> Result<BTreeMap<String, JsonValue>, ContractError> {
    let events = json
        .get("events")
        .and_then(JsonValue::as_array)
        .ok_or(ContractError::InvalidShape("batch events must be an array"))?;
    let mut claims = BTreeMap::new();
    for event in events {
        let Some(payload) = event.get("payload").and_then(JsonValue::as_object) else {
            return Err(ContractError::InvalidShape(
                "event payload must be an object",
            ));
        };
        if payload.get("kind").and_then(JsonValue::as_str) != Some("CLAIM_ASSERTED") {
            continue;
        }
        let claim = payload.get("value").and_then(JsonValue::as_object).ok_or(
            ContractError::InvalidShape("claim assertion value must be an object"),
        )?;
        let id = claim
            .get("id")
            .and_then(JsonValue::as_str)
            .ok_or(ContractError::InvalidShape("claim id must be a string"))?;
        if claims
            .insert(id.to_owned(), JsonValue::Object(claim.clone()))
            .is_some()
        {
            return Err(ContractError::LegacyCompatibility(
                "duplicate claim id in compatibility batch",
            ));
        }
    }
    Ok(claims)
}

fn transform_decisions_for_v1(json: &mut JsonValue) -> Result<(), ContractError> {
    let claims = claim_index(json)?;
    let events = json
        .get_mut("events")
        .and_then(JsonValue::as_array_mut)
        .ok_or(ContractError::InvalidShape("batch events must be an array"))?;
    for event in events {
        let payload = event
            .get_mut("payload")
            .and_then(JsonValue::as_object_mut)
            .ok_or(ContractError::InvalidShape(
                "event payload must be an object",
            ))?;
        if payload.get("kind").and_then(JsonValue::as_str) != Some("DECISION_RECORDED") {
            continue;
        }
        let decision = payload
            .get_mut("value")
            .and_then(JsonValue::as_object_mut)
            .ok_or(ContractError::InvalidShape(
                "decision value must be an object",
            ))?;
        let target_id = decision
            .get("target_claim_id")
            .and_then(JsonValue::as_str)
            .ok_or(ContractError::InvalidShape(
                "decision target_claim_id must be a string",
            ))?;
        let target = claims.get(target_id).and_then(JsonValue::as_object).ok_or(
            ContractError::LegacyCompatibility(
                "v1-compatible decision target must be asserted in the same batch",
            ),
        )?;
        let slot = decision
            .get("resolution_slot")
            .and_then(JsonValue::as_object)
            .ok_or(ContractError::LegacyCompatibility(
                "v2 decision is missing its resolution slot",
            ))?;
        for (slot_field, claim_field) in [
            ("subject_entity_id", "subject_entity_id"),
            ("predicate_id", "predicate_id"),
            ("scope_id", "scope_id"),
        ] {
            if slot.get(slot_field) != target.get(claim_field) {
                return Err(ContractError::LegacyCompatibility(
                    "v2 decision slot cannot be represented by its v1 target",
                ));
            }
        }
        if decision.get("target_object") != target.get("object")
            || decision.get("valid_time") != target.get("valid_time")
        {
            return Err(ContractError::LegacyCompatibility(
                "v2 decision object or validity cannot be represented losslessly by v1",
            ));
        }
        let scope_id = slot
            .get("scope_id")
            .cloned()
            .ok_or(ContractError::LegacyCompatibility(
                "v2 decision slot is missing scope_id",
            ))?;
        decision.remove("target_object");
        decision.remove("resolution_slot");
        decision.remove("valid_time");
        decision.insert("scope_id".to_owned(), scope_id);
    }
    Ok(())
}

fn upcast_v1_to_v2(json: &mut JsonValue) -> Result<(), ContractError> {
    let claims = claim_index(json)?;
    let events = json
        .get_mut("events")
        .and_then(JsonValue::as_array_mut)
        .ok_or(ContractError::InvalidShape("batch events must be an array"))?;
    for event in events {
        let payload = event
            .get_mut("payload")
            .and_then(JsonValue::as_object_mut)
            .ok_or(ContractError::InvalidShape(
                "event payload must be an object",
            ))?;
        if payload.get("kind").and_then(JsonValue::as_str) != Some("DECISION_RECORDED") {
            continue;
        }
        let decision = payload
            .get_mut("value")
            .and_then(JsonValue::as_object_mut)
            .ok_or(ContractError::InvalidShape(
                "decision value must be an object",
            ))?;
        if decision.contains_key("target_object")
            || decision.contains_key("resolution_slot")
            || decision.contains_key("valid_time")
        {
            return Err(ContractError::LegacyCompatibility(
                "v1 decision contains v2-only semantic fields",
            ));
        }
        let target_id = decision
            .get("target_claim_id")
            .and_then(JsonValue::as_str)
            .ok_or(ContractError::InvalidShape(
                "decision target_claim_id must be a string",
            ))?;
        let target = claims.get(target_id).and_then(JsonValue::as_object).ok_or(
            ContractError::LegacyCompatibility(
                "v1 decision target must be asserted in the same batch",
            ),
        )?;
        let legacy_scope =
            decision
                .remove("scope_id")
                .ok_or(ContractError::LegacyCompatibility(
                    "v1 decision is missing scope_id",
                ))?;
        if Some(&legacy_scope) != target.get("scope_id") {
            return Err(ContractError::LegacyCompatibility(
                "v1 decision scope does not match its target claim",
            ));
        }
        let target_object = target
            .get("object")
            .cloned()
            .ok_or(ContractError::InvalidShape("claim object is missing"))?;
        let valid_time = target
            .get("valid_time")
            .cloned()
            .ok_or(ContractError::InvalidShape("claim valid_time is missing"))?;
        let subject_entity_id =
            target
                .get("subject_entity_id")
                .cloned()
                .ok_or(ContractError::InvalidShape(
                    "claim subject_entity_id is missing",
                ))?;
        let predicate_id = target
            .get("predicate_id")
            .cloned()
            .ok_or(ContractError::InvalidShape("claim predicate_id is missing"))?;
        decision.insert("target_object".to_owned(), target_object);
        decision.insert(
            "resolution_slot".to_owned(),
            JsonValue::Object(JsonMap::from_iter([
                ("subject_entity_id".to_owned(), subject_entity_id),
                ("predicate_id".to_owned(), predicate_id),
                ("scope_id".to_owned(), legacy_scope),
            ])),
        );
        decision.insert("valid_time".to_owned(), valid_time);
    }
    Ok(())
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
        Actor, AuthorityClass, BatchId, Claim, ClaimId, ClaimObject, ClaimRelation,
        ClaimRelationKind, Decimal, DeviceId, DomainId, EVENT_SCHEMA_VERSION, EntityId,
        EpistemicStatus, Event, EventId, EventPayload, EvidenceId, PredicateId, ResolutionSlot,
        ScopeId, TimestampMillis, ValidInterval,
    };

    use super::*;

    fn minimal_batch() -> Result<UnsignedBatch, DomainError> {
        let domain_id = DomainId::from_str("01900000-0000-7000-8000-000000000001")?;
        let user_id = EntityId::from_str("01900000-0000-7000-8000-000000000007")?;
        Ok(UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: BatchId::from_str("01900000-0000-7000-8000-000000000002")?,
            device_id: DeviceId::from_str("01900000-0000-7000-8000-000000000003")?,
            origin_seq_start: 1,
            origin_seq_end: 1,
            previous_batch_hash: None,
            origin_created_at: TimestampMillis::new(10),
            events: vec![Event {
                id: EventId::from_str("01900000-0000-7000-8000-000000000004")?,
                origin_seq: 1,
                origin_observed_at: TimestampMillis::new(9),
                actor: Actor::User { user_id },
                domain_id,
                payload: EventPayload::DecisionRecorded(academic_domain::UserDecision {
                    id: academic_domain::DecisionId::from_str(
                        "01900000-0000-7000-8000-000000000005",
                    )?,
                    target_claim_id: academic_domain::ClaimId::from_str(
                        "01900000-0000-7000-8000-000000000006",
                    )?,
                    target_object: ClaimObject::Text("synthetic".to_owned()),
                    resolution_slot: ResolutionSlot {
                        subject_entity_id: EntityId::from_str(
                            "01900000-0000-7000-8000-000000000009",
                        )?,
                        predicate_id: PredicateId::parse("test.value")?,
                        scope_id: ScopeId::from_str("01900000-0000-7000-8000-000000000008")?,
                    },
                    action: academic_domain::DecisionAction::Reject,
                    valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
                    rationale_evidence_ids: Vec::new(),
                    decided_at: TimestampMillis::new(9),
                    reversible_until: None,
                }),
            }],
        })
    }

    fn authorization(
        batch: &UnsignedBatch,
        signing_key: &SigningKey,
    ) -> Result<DeviceAuthorization, DomainError> {
        Ok(DeviceAuthorization::new(
            batch.device_id,
            EntityId::from_str("01900000-0000-7000-8000-000000000007")?,
            signing_key.verifying_key(),
        ))
    }

    fn sign_test_payload(
        payload: &[u8],
        signing_key: &SigningKey,
    ) -> Result<Vec<u8>, ContractError> {
        let signature = signing_key.sign(payload);
        encode_envelope(
            payload,
            signing_key.verifying_key().as_bytes(),
            &signature.to_bytes(),
        )
    }

    fn v1_compatible_batch() -> Result<UnsignedBatch, DomainError> {
        let mut batch = minimal_batch()?;
        let user_id = EntityId::from_str("01900000-0000-7000-8000-000000000007")?;
        let domain_id = batch.events[0].domain_id;
        let mut decision_event = batch.events.remove(0);
        decision_event.origin_seq = 2;
        let claim = Claim {
            id: ClaimId::from_str("01900000-0000-7000-8000-000000000006")?,
            subject_entity_id: EntityId::from_str("01900000-0000-7000-8000-000000000009")?,
            predicate_id: PredicateId::parse("test.value")?,
            object: ClaimObject::Text("synthetic".to_owned()),
            scope_id: ScopeId::from_str("01900000-0000-7000-8000-000000000008")?,
            authority_class: AuthorityClass::UserExplicit,
            epistemic_status: EpistemicStatus::UserConfirmed,
            confidence: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
            evidence_ids: vec![EvidenceId::from_str(
                "01900000-0000-7000-8000-00000000000b",
            )?],
        };
        batch.origin_seq_end = 2;
        batch.events = vec![
            Event {
                id: EventId::from_str("01900000-0000-7000-8000-00000000000a")?,
                origin_seq: 1,
                origin_observed_at: TimestampMillis::new(8),
                actor: Actor::User { user_id },
                domain_id,
                payload: EventPayload::ClaimAsserted(claim),
            },
            decision_event,
        ];
        batch.validate()?;
        Ok(batch)
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
    fn t008_v1_user_decision_wire_upcasts_deterministically_to_v2()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = v1_compatible_batch()?;

        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let first = encode_unsigned_batch_v1_projection(&batch)?;
        let second = encode_unsigned_batch_v1_projection(&batch)?;
        assert_eq!(first, second);
        assert_eq!(decode_unsigned_batch(&first)?, batch);

        let legacy_json = cbor_to_json(&decode_single_cbor(&first)?)?;
        let mut mismatched_scope = legacy_json.clone();
        mismatched_scope["events"][1]["payload"]["value"]["scope_id"] =
            JsonValue::String("01900000-0000-7000-8000-000000000099".to_owned());
        let mismatched_scope_bytes = encode_cbor_value(&json_to_cbor(&mismatched_scope)?)?;
        assert!(matches!(
            decode_unsigned_batch(&mismatched_scope_bytes),
            Err(ContractError::LegacyCompatibility(_))
        ));

        let mut missing_target = legacy_json.clone();
        missing_target["events"]
            .as_array_mut()
            .ok_or(ContractError::InvalidShape(
                "test batch events must be an array",
            ))?
            .remove(0);
        let missing_target_bytes = encode_cbor_value(&json_to_cbor(&missing_target)?)?;
        assert!(matches!(
            decode_unsigned_batch(&missing_target_bytes),
            Err(ContractError::LegacyCompatibility(_))
        ));

        let mut smuggled_v2_field = legacy_json;
        smuggled_v2_field["events"][1]["payload"]["value"]["target_object"] =
            serde_json::to_value(ClaimObject::Text("synthetic".to_owned()))?;
        let smuggled_v2_bytes = encode_cbor_value(&json_to_cbor(&smuggled_v2_field)?)?;
        assert!(matches!(
            decode_unsigned_batch(&smuggled_v2_bytes),
            Err(ContractError::LegacyCompatibility(_))
        ));

        let signed = sign_test_payload(&first, &signing_key)?;
        let verified = verify_signed_batch(&signed, &authorization(&batch, &signing_key)?)?;
        assert_eq!(
            verified.source_schema_version(),
            academic_domain::EVENT_SCHEMA_VERSION_V1
        );
        assert_eq!(
            verified.batch().schema_version,
            academic_domain::EVENT_SCHEMA_VERSION_V2
        );
        assert_eq!(verified.batch(), &batch);

        let EventPayload::DecisionRecorded(decision) = &mut batch.events[1].payload else {
            unreachable!("test fixture has a decision event")
        };
        decision.valid_time = ValidInterval::open_ended(TimestampMillis::new(1));
        assert!(matches!(
            encode_unsigned_batch_v1_projection(&batch),
            Err(ContractError::LegacyCompatibility(_))
        ));
        Ok(())
    }

    #[test]
    fn t010_signed_v1_and_v2_reject_unknown_authenticated_nested_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let batch = v1_compatible_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let authorization = authorization(&batch, &signing_key)?;
        for (source_version, payload) in [
            (
                EVENT_SCHEMA_VERSION_V1,
                encode_unsigned_batch_v1_projection(&batch)?,
            ),
            (EVENT_SCHEMA_VERSION_V2, encode_unsigned_batch(&batch)?),
        ] {
            let source_json = cbor_to_json(&decode_single_cbor(&payload)?)?;
            for pointer in [
                "",
                "/events/0",
                "/events/0/actor",
                "/events/0/payload",
                "/events/0/payload/value",
                "/events/0/payload/value/object",
                "/events/0/payload/value/valid_time",
                "/events/1/actor",
                "/events/1/payload/value",
                "/events/1/payload/value/action",
            ] {
                let mut smuggled = source_json.clone();
                let object = smuggled
                    .pointer_mut(pointer)
                    .and_then(JsonValue::as_object_mut)
                    .ok_or_else(|| format!("test pointer must select an object: {pointer}"))?;
                object.insert(
                    "authenticated_unknown".to_owned(),
                    JsonValue::String("must-not-disappear".to_owned()),
                );
                let smuggled_payload = encode_cbor_value(&json_to_cbor(&smuggled)?)?;
                let signed = sign_test_payload(&smuggled_payload, &signing_key)?;
                assert!(
                    verify_signed_batch(&signed, &authorization).is_err(),
                    "v{source_version} accepted unknown field at {pointer}",
                );
            }

            if source_version == EVENT_SCHEMA_VERSION_V1 {
                let current_json = serde_json::to_value(&batch)?;
                for field in ["resolution_slot", "target_object", "valid_time"] {
                    let mut smuggled = source_json.clone();
                    smuggled["events"][1]["payload"]["value"][field] =
                        current_json["events"][1]["payload"]["value"][field].clone();
                    let smuggled_payload = encode_cbor_value(&json_to_cbor(&smuggled)?)?;
                    let signed = sign_test_payload(&smuggled_payload, &signing_key)?;
                    assert!(matches!(
                        verify_signed_batch(&signed, &authorization),
                        Err(ContractError::LegacyCompatibility(_))
                    ));
                }
            } else {
                for pointer in [
                    "/events/1/payload/value/resolution_slot",
                    "/events/1/payload/value/target_object",
                    "/events/1/payload/value/valid_time",
                ] {
                    let mut smuggled = source_json.clone();
                    smuggled
                        .pointer_mut(pointer)
                        .and_then(JsonValue::as_object_mut)
                        .ok_or_else(|| format!("test pointer must select an object: {pointer}"))?
                        .insert(
                            "authenticated_unknown".to_owned(),
                            JsonValue::String("must-not-disappear".to_owned()),
                        );
                    let smuggled_payload = encode_cbor_value(&json_to_cbor(&smuggled)?)?;
                    let signed = sign_test_payload(&smuggled_payload, &signing_key)?;
                    assert!(verify_signed_batch(&signed, &authorization).is_err());
                }
            }

            let mut discarded_by_struct = source_json.clone();
            discarded_by_struct["events"][0]["payload"]["value"]["authenticated_claim_field"] =
                JsonValue::Bool(true);
            let discarded_payload = encode_cbor_value(&json_to_cbor(&discarded_by_struct)?)?;
            let signed = sign_test_payload(&discarded_payload, &signing_key)?;
            assert!(matches!(
                verify_signed_batch(&signed, &authorization),
                Err(ContractError::AuthenticatedFieldDiscarded)
            ));
        }

        let mut v2_smuggling = serde_json::to_value(&batch)?;
        v2_smuggling["events"][1]["payload"]["value"]["scope_id"] =
            JsonValue::String("01900000-0000-7000-8000-000000000008".to_owned());
        let v2_payload = encode_cbor_value(&json_to_cbor(&v2_smuggling)?)?;
        let signed = sign_test_payload(&v2_payload, &signing_key)?;
        assert!(matches!(
            verify_signed_batch(&signed, &authorization),
            Err(ContractError::AuthenticatedFieldDiscarded)
        ));
        Ok(())
    }

    #[test]
    fn t010_verification_orders_canonicality_and_signature_before_typed_equality()
    -> Result<(), Box<dyn std::error::Error>> {
        let batch = v1_compatible_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let authorization = authorization(&batch, &signing_key)?;

        let mut trailing_payload = encode_unsigned_batch(&batch)?;
        trailing_payload.push(0);
        let invalid_signature = [0_u8; 64];
        let trailing_envelope = encode_envelope(
            &trailing_payload,
            signing_key.verifying_key().as_bytes(),
            &invalid_signature,
        )?;
        assert!(matches!(
            verify_signed_batch(&trailing_envelope, &authorization),
            Err(ContractError::TrailingBytes)
        ));

        let mut lossy_json = serde_json::to_value(&batch)?;
        lossy_json["events"][0]["authenticated_event_field"] = JsonValue::Bool(true);
        let lossy_payload = encode_cbor_value(&json_to_cbor(&lossy_json)?)?;
        let invalid_signature_envelope = encode_envelope(
            &lossy_payload,
            signing_key.verifying_key().as_bytes(),
            &invalid_signature,
        )?;
        assert!(matches!(
            verify_signed_batch(&invalid_signature_envelope, &authorization),
            Err(ContractError::InvalidSignature)
        ));

        let signed_lossy = sign_test_payload(&lossy_payload, &signing_key)?;
        assert!(matches!(
            verify_signed_batch(&signed_lossy, &authorization),
            Err(ContractError::AuthenticatedFieldDiscarded)
        ));
        Ok(())
    }

    #[test]
    fn signature_and_expected_key_are_both_required() -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let other_key = SigningKey::from_bytes(&[8_u8; 32]).verifying_key();
        let signed = sign_batch(&batch, &signing_key)?;
        let wrong_key_authorization = DeviceAuthorization::new(
            batch.device_id,
            EntityId::from_str("01900000-0000-7000-8000-000000000007")?,
            other_key,
        );
        assert!(matches!(
            verify_signed_batch(&signed, &wrong_key_authorization),
            Err(ContractError::UnexpectedSigningKey)
        ));
        let verified = verify_signed_batch(&signed, &authorization(&batch, &signing_key)?)?;
        assert_eq!(verified.batch(), &batch);
        Ok(())
    }

    #[test]
    fn tampering_fails_signature_verification() -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let mut signed = sign_batch(&batch, &signing_key)?;
        let middle = signed.len() / 2;
        signed[middle] ^= 1;
        assert!(verify_signed_batch(&signed, &authorization(&batch, &signing_key)?).is_err());
        Ok(())
    }

    #[test]
    fn signed_batch_rejects_device_id_key_binding_mismatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let signed = sign_batch(&batch, &signing_key)?;
        let authorization = DeviceAuthorization::new(
            DeviceId::from_str("01900000-0000-7000-8000-000000000099")?,
            EntityId::from_str("01900000-0000-7000-8000-000000000007")?,
            signing_key.verifying_key(),
        );
        assert!(matches!(
            verify_signed_batch(&signed, &authorization),
            Err(ContractError::UnexpectedDeviceId)
        ));
        Ok(())
    }

    #[test]
    fn signed_batch_rejects_user_actor_identity_mismatch() -> Result<(), Box<dyn std::error::Error>>
    {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let signed = sign_batch(&batch, &signing_key)?;
        let authorization = DeviceAuthorization::new(
            batch.device_id,
            EntityId::from_str("01900000-0000-7000-8000-000000000099")?,
            signing_key.verifying_key(),
        );
        assert!(matches!(
            verify_signed_batch(&signed, &authorization),
            Err(ContractError::UnexpectedUserActor)
        ));
        Ok(())
    }

    #[test]
    fn protobuf_relation_and_structured_actor_round_trip_is_lossless()
    -> Result<(), Box<dyn std::error::Error>> {
        let actors = [
            Actor::User {
                user_id: EntityId::from_str("01900000-0000-7000-8000-000000000020")?,
            },
            Actor::DeterministicEngine {
                name: "resolver".to_owned(),
                version: "1.2.3".to_owned(),
            },
            Actor::ModelRun {
                run_id: EntityId::from_str("01900000-0000-7000-8000-000000000021")?,
            },
            Actor::Importer {
                name: "registrar".to_owned(),
                version: "2026.08".to_owned(),
            },
        ];
        for (index, actor) in actors.into_iter().enumerate() {
            let event = Event {
                id: EventId::from_str(&format!("01900000-0000-7000-8000-{:012x}", 30 + index))?,
                origin_seq: u64::try_from(index + 1)?,
                origin_observed_at: TimestampMillis::new(i64::try_from(index)?),
                actor,
                domain_id: DomainId::from_str("01900000-0000-7000-8000-000000000022")?,
                payload: EventPayload::ClaimRelated(ClaimRelation {
                    source_claim_id: ClaimId::from_str("01900000-0000-7000-8000-000000000023")?,
                    target_claim_id: ClaimId::from_str("01900000-0000-7000-8000-000000000024")?,
                    kind: ClaimRelationKind::Supersedes,
                    scope_id: ScopeId::from_str("01900000-0000-7000-8000-000000000025")?,
                }),
            };
            let encoded = encode_claim_relation_event_proto(&event)?;
            assert_eq!(decode_claim_relation_event_proto(&encoded)?, event);
        }
        Ok(())
    }

    #[test]
    fn t013_every_relation_kind_round_trips_exactly() -> Result<(), Box<dyn std::error::Error>> {
        for (index, kind) in [
            ClaimRelationKind::Supports,
            ClaimRelationKind::Contradicts,
            ClaimRelationKind::Supersedes,
            ClaimRelationKind::Retracts,
            ClaimRelationKind::Duplicates,
        ]
        .into_iter()
        .enumerate()
        {
            let event = Event {
                id: EventId::from_str(&format!("01900000-0000-7000-8000-{:012x}", 200 + index))?,
                origin_seq: u64::try_from(index + 1)?,
                origin_observed_at: TimestampMillis::new(i64::try_from(index)?),
                actor: Actor::Importer {
                    name: "relation-profile".to_owned(),
                    version: "1".to_owned(),
                },
                domain_id: DomainId::from_str("01900000-0000-7000-8000-000000000022")?,
                payload: EventPayload::ClaimRelated(ClaimRelation {
                    source_claim_id: ClaimId::from_str("01900000-0000-7000-8000-000000000023")?,
                    target_claim_id: ClaimId::from_str("01900000-0000-7000-8000-000000000024")?,
                    kind,
                    scope_id: ScopeId::from_str("01900000-0000-7000-8000-000000000025")?,
                }),
            };
            let encoded = encode_claim_relation_event_proto(&event)?;
            assert_eq!(
                decode_claim_relation_event_proto(&encoded)?,
                event,
                "relation kind {kind:?} changed during Rust/Proto conversion"
            );
        }
        Ok(())
    }

    #[test]
    fn t007_known_proto_oneof_arms_obey_last_value_wins_in_every_field_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let event = Event {
            id: EventId::from_str("01900000-0000-7000-8000-000000000030")?,
            origin_seq: 1,
            origin_observed_at: TimestampMillis::new(1),
            actor: Actor::Importer {
                name: "fixture".to_owned(),
                version: "1".to_owned(),
            },
            domain_id: DomainId::from_str("01900000-0000-7000-8000-000000000022")?,
            payload: EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: ClaimId::from_str("01900000-0000-7000-8000-000000000023")?,
                target_claim_id: ClaimId::from_str("01900000-0000-7000-8000-000000000024")?,
                kind: ClaimRelationKind::Supersedes,
                scope_id: ScopeId::from_str("01900000-0000-7000-8000-000000000025")?,
            }),
        };
        let relation_last = encode_claim_relation_event_proto(&event)?;
        for (name, encoded_key) in [
            ("artifact_registered", 0x52_u8),
            ("evidence_registered", 0x5a),
            ("claim_asserted", 0x62),
            ("decision_recorded", 0x6a),
            ("scope_registered", 0x72),
        ] {
            let mut earlier_known_arm = vec![encoded_key, 0];
            earlier_known_arm.extend_from_slice(&relation_last);
            assert_eq!(
                decode_claim_relation_event_proto(&earlier_known_arm)?,
                event,
                "relation must win when it is the final oneof arm after {name}",
            );

            let mut later_known_arm = relation_last.clone();
            later_known_arm.extend_from_slice(&[encoded_key, 0]);
            assert!(
                matches!(
                    decode_claim_relation_event_proto(&later_known_arm),
                    Err(ProtoContractError::UnsupportedPayload)
                ),
                "{name} must replace an earlier relation",
            );
            assert!(matches!(
                decode_claim_relation_event_proto(&[encoded_key, 0]),
                Err(ProtoContractError::UnsupportedPayload)
            ));
        }
        Ok(())
    }

    #[test]
    fn signed_decode_revalidates_uuidv7_before_issuing_capability()
    -> Result<(), Box<dyn std::error::Error>> {
        let batch = minimal_batch()?;
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let authorization = authorization(&batch, &signing_key)?;
        let mut json = serde_json::to_value(&batch)?;
        json["batch_id"] = JsonValue::String("00000000-0000-4000-8000-000000000000".to_owned());
        let payload = encode_cbor_value(&json_to_cbor(&json)?)?;
        let signature = signing_key.sign(&payload);
        let envelope = encode_envelope(
            &payload,
            signing_key.verifying_key().as_bytes(),
            &signature.to_bytes(),
        )?;
        assert!(matches!(
            verify_signed_batch(&envelope, &authorization),
            Err(ContractError::Json(_))
        ));
        Ok(())
    }

    #[test]
    fn deterministic_cbor_round_trips_decimal_i128_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        for (offset, coefficient) in [i128::MIN, i128::MAX].into_iter().enumerate() {
            let mut batch = minimal_batch()?;
            batch.events[0].payload = EventPayload::ClaimAsserted(Claim {
                id: ClaimId::from_str(&format!("01900000-0000-7000-8000-{:012x}", 100 + offset))?,
                subject_entity_id: EntityId::from_str("01900000-0000-7000-8000-000000000101")?,
                predicate_id: PredicateId::parse("test.decimal")?,
                object: ClaimObject::Decimal(Decimal::new(coefficient, 18)?),
                scope_id: ScopeId::from_str("01900000-0000-7000-8000-000000000008")?,
                authority_class: AuthorityClass::UserExplicit,
                epistemic_status: EpistemicStatus::UserConfirmed,
                confidence: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
                evidence_ids: vec![EvidenceId::from_str(
                    "01900000-0000-7000-8000-000000000102",
                )?],
            });
            let encoded = encode_unsigned_batch(&batch)?;
            assert_eq!(decode_unsigned_batch(&encoded)?, batch);
        }
        Ok(())
    }
}
