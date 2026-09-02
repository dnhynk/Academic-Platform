//! Receipt-bound data-admission posture.
//!
//! The product verifier has one acceptance-key source: [`ACCEPTANCE_PUBLIC_KEY`].
//! It is currently typed as unprovisioned, so every product verification is
//! denied and every surface emits the synthetic posture. Test receipts use a
//! separate verifier compiled only under `cfg(test)`; no key parameter or
//! setter is present in the product API.

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use ciborium::value::{Integer, Value};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Serialize, Serializer, ser::SerializeStruct as _};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Where the signed receipt lives relative to a profile root.
pub const ADMISSION_RECEIPT_RELATIVE_PATH: &str = "admission/receipt.cbor";
/// Deterministic signed-envelope version shared with `academic-contracts`.
pub const SIGNED_ENVELOPE_VERSION: u16 = 1;
/// Admission payload version accepted by this build.
pub const ADMISSION_RECEIPT_VERSION: u16 = 1;
/// Store schema version admitted by this verifier.
pub const COMPILED_STORE_SCHEMA_VERSION: u32 = 2;
/// Canonical end-state design-document SHA-256.
pub const CANONICAL_SPEC_DIGEST: [u8; 32] = [
    0x48, 0x30, 0xde, 0xbd, 0x1a, 0x9e, 0xe8, 0xbe, 0x13, 0xb1, 0x0d, 0x1e, 0x72, 0xba, 0x3d, 0x2a,
    0x39, 0x43, 0xf9, 0xd6, 0x34, 0x17, 0x05, 0x1c, 0xc1, 0x23, 0xef, 0x51, 0x74, 0x3b, 0x2e, 0x45,
];
/// Five platform rows required before admission can succeed.
pub const REQUIRED_ADMISSION_PLATFORMS: [&str; 5] = [
    "windows-x86_64",
    "windows-aarch64",
    "linux-x86_64",
    "linux-aarch64",
    "macos-aarch64",
];

const MAX_RECEIPT_BYTES: usize = 1_048_576;
const ENCRYPTED_PROFILE_MARKER: &str = "PROFILE_FORMAT_V2";
const SYNTHETIC_PROFILE_MARKER: &str = "SYNTHETIC_ONLY_PLAINTEXT_DO_NOT_USE_REAL_DATA.txt";
const ENCRYPTED_PROFILE_MARKER_BYTES: &[u8] = concat!(
    "ACADEMIC_PLATFORM_ENCRYPTED_PROFILE_FORMAT_V2\n",
    "format_uuid=67cb6d3ea27e4b53b1e727d46920e4f9\n",
    "schema_version=2\n",
    "schema_semver=2.0.0\n",
    "storage_mode=SQLCIPHER_ENCRYPTED_PROFILE_V2\n",
    "storage_encryption=SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000\n",
)
.as_bytes();

const SYNTHETIC_DATA_POLICY: &str = "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED";
const SYNTHETIC_STORAGE_MODE: &str = "PLAINTEXT_TEMPORARY_SQLITE";
const SYNTHETIC_STORAGE_ENCRYPTION: &str = "NONE";
const SYNTHETIC_PRODUCT_NETWORK: &str = "NONE";
const ADMITTED_DATA_POLICY: &str = "REAL_PERSONAL_DATA_PERMITTED";
const ADMITTED_STORAGE_MODE: &str = "SQLCIPHER_ENCRYPTED_PROFILE_V2";
const ADMITTED_STORAGE_ENCRYPTION: &str = "SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000";
const ADMITTED_OBJECT_FORMAT: &str = "AEAD_CHUNKED_V2";
const ADMITTED_PRODUCT_NETWORK: &str = "BROKERED_EGRESS_ONLY";

/// Compile-time acceptance-key provisioning state.
///
/// Provisioning replaces the value of [`ACCEPTANCE_PUBLIC_KEY`] with
/// `Self::Provisioned(<the user's 32-byte Ed25519 public key>)`. The user's
/// private half is not an input to this crate or to any product command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptancePublicKey {
    /// No user acceptance key has been supplied.
    Unprovisioned,
    /// One exact Ed25519 public key compiled into the binary.
    Provisioned([u8; 32]),
}

/// The sole product acceptance-key source; currently fail-closed.
pub const ACCEPTANCE_PUBLIC_KEY: AcceptancePublicKey = AcceptancePublicKey::Unprovisioned;

/// Exact data posture emitted on every surface.
///
/// Its fields are private so an admitted value cannot be assembled with a
/// struct literal. The only admitted constructor consumes the opaque
/// [`VerifiedAdmission`] issued by [`AdmissionVerifier::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posture {
    kind: PostureKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PostureKind {
    Synthetic,
    Admitted {
        receipt_digest: String,
        platforms: Vec<String>,
    },
}

impl Posture {
    /// Returns the unchanged Phase 1 synthetic posture.
    #[must_use]
    pub const fn synthetic() -> Self {
        Self {
            kind: PostureKind::Synthetic,
        }
    }

    /// Returns the admitted posture bound to one verified receipt.
    #[must_use]
    pub fn from_verified(verified: &VerifiedAdmission) -> Self {
        Self {
            kind: PostureKind::Admitted {
                receipt_digest: verified.receipt_digest.clone(),
                platforms: verified.platforms.clone(),
            },
        }
    }

    /// Data policy spelling.
    #[must_use]
    pub const fn data_policy(&self) -> &'static str {
        match self.kind {
            PostureKind::Synthetic => SYNTHETIC_DATA_POLICY,
            PostureKind::Admitted { .. } => ADMITTED_DATA_POLICY,
        }
    }

    /// Physical store mode spelling.
    #[must_use]
    pub const fn storage_mode(&self) -> &'static str {
        match self.kind {
            PostureKind::Synthetic => SYNTHETIC_STORAGE_MODE,
            PostureKind::Admitted { .. } => ADMITTED_STORAGE_MODE,
        }
    }

    /// Storage encryption spelling.
    #[must_use]
    pub const fn storage_encryption(&self) -> &'static str {
        match self.kind {
            PostureKind::Synthetic => SYNTHETIC_STORAGE_ENCRYPTION,
            PostureKind::Admitted { .. } => ADMITTED_STORAGE_ENCRYPTION,
        }
    }

    /// Optional object format, present only after admission.
    #[must_use]
    pub const fn object_format(&self) -> Option<&'static str> {
        match self.kind {
            PostureKind::Synthetic => None,
            PostureKind::Admitted { .. } => Some(ADMITTED_OBJECT_FORMAT),
        }
    }

    /// Whether real or production data is admitted.
    #[must_use]
    pub const fn production_data_allowed(&self) -> bool {
        matches!(self.kind, PostureKind::Admitted { .. })
    }

    /// Product network spelling.
    #[must_use]
    pub const fn product_network(&self) -> &'static str {
        match self.kind {
            PostureKind::Synthetic => SYNTHETIC_PRODUCT_NETWORK,
            PostureKind::Admitted { .. } => ADMITTED_PRODUCT_NETWORK,
        }
    }

    /// Receipt digest, present only after admission.
    #[must_use]
    pub fn admission_receipt_digest(&self) -> Option<&str> {
        match &self.kind {
            PostureKind::Synthetic => None,
            PostureKind::Admitted { receipt_digest, .. } => Some(receipt_digest),
        }
    }

    /// Required platforms carried by an admitted receipt.
    #[must_use]
    pub fn admission_platforms(&self) -> &[String] {
        match &self.kind {
            PostureKind::Synthetic => &[],
            PostureKind::Admitted { platforms, .. } => platforms,
        }
    }

    /// Stable banner paired with this posture.
    #[must_use]
    pub const fn banner(&self) -> &'static str {
        match self.kind {
            PostureKind::Synthetic => {
                "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN"
            }
            PostureKind::Admitted { .. } => {
                "ENCRYPTED ADMITTED PROFILE — REAL PERSONAL DATA IS PERMITTED"
            }
        }
    }

    /// Compact canonical JSON bytes used by CLI, IPC, and export surfaces.
    #[must_use]
    pub fn canonical_json_bytes(&self) -> Vec<u8> {
        match &self.kind {
            PostureKind::Synthetic => concat!(
                "{\"data_policy\":\"SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED\",",
                "\"storage_mode\":\"PLAINTEXT_TEMPORARY_SQLITE\",",
                "\"storage_encryption\":\"NONE\",",
                "\"production_data_allowed\":false,",
                "\"product_network\":\"NONE\"}"
            )
            .as_bytes()
            .to_vec(),
            PostureKind::Admitted {
                receipt_digest,
                platforms: _,
            } => format!(
                concat!(
                    "{{\"data_policy\":\"REAL_PERSONAL_DATA_PERMITTED\",",
                    "\"storage_mode\":\"SQLCIPHER_ENCRYPTED_PROFILE_V2\",",
                    "\"storage_encryption\":",
                    "\"SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000\",",
                    "\"object_format\":\"AEAD_CHUNKED_V2\",",
                    "\"production_data_allowed\":true,",
                    "\"product_network\":\"BROKERED_EGRESS_ONLY\",",
                    "\"admission_receipt_digest\":\"{}\",",
                    "\"admission_platforms\":[",
                    "\"windows-x86_64\",\"windows-aarch64\",",
                    "\"linux-x86_64\",\"linux-aarch64\",\"macos-aarch64\"]}}"
                ),
                receipt_digest
            )
            .into_bytes(),
        }
    }
}

impl Default for Posture {
    fn default() -> Self {
        Self::synthetic()
    }
}

impl Serialize for Posture {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.kind {
            PostureKind::Synthetic => {
                let mut state = serializer.serialize_struct("Posture", 5)?;
                state.serialize_field("data_policy", self.data_policy())?;
                state.serialize_field("storage_mode", self.storage_mode())?;
                state.serialize_field("storage_encryption", self.storage_encryption())?;
                state
                    .serialize_field("production_data_allowed", &self.production_data_allowed())?;
                state.serialize_field("product_network", self.product_network())?;
                state.end()
            }
            PostureKind::Admitted {
                receipt_digest,
                platforms,
            } => {
                let mut state = serializer.serialize_struct("Posture", 8)?;
                state.serialize_field("data_policy", self.data_policy())?;
                state.serialize_field("storage_mode", self.storage_mode())?;
                state.serialize_field("storage_encryption", self.storage_encryption())?;
                state.serialize_field("object_format", &ADMITTED_OBJECT_FORMAT)?;
                state
                    .serialize_field("production_data_allowed", &self.production_data_allowed())?;
                state.serialize_field("product_network", self.product_network())?;
                state.serialize_field("admission_receipt_digest", receipt_digest)?;
                state.serialize_field("admission_platforms", platforms)?;
                state.end()
            }
        }
    }
}

/// Opaque evidence that one receipt passed the five verification stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAdmission {
    receipt_digest: String,
    platforms: Vec<String>,
}

impl VerifiedAdmission {
    /// SHA-256 of the canonical signed receipt envelope.
    #[must_use]
    pub fn receipt_digest(&self) -> &str {
        &self.receipt_digest
    }

    /// Exact required platform order authenticated by the receipt.
    #[must_use]
    pub fn platforms(&self) -> &[String] {
        &self.platforms
    }

    /// Converts this capability into the admitted posture.
    #[must_use]
    pub fn posture(&self) -> Posture {
        Posture::from_verified(self)
    }
}

/// Product admission verifier. It holds no runtime-configurable state.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdmissionVerifier;

impl AdmissionVerifier {
    /// Runs the five-stage receipt verification contract.
    pub fn verify(profile_root: &Path) -> Result<VerifiedAdmission, AdmissionError> {
        let path = profile_root.join(ADMISSION_RECEIPT_RELATIVE_PATH);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(AdmissionError::ReceiptAbsent { path });
            }
            Err(source) => {
                return Err(AdmissionError::ReadReceipt { path, source });
            }
        };
        if bytes.len() > MAX_RECEIPT_BYTES {
            return Err(AdmissionError::ReceiptTooLarge {
                actual: bytes.len(),
                maximum: MAX_RECEIPT_BYTES,
            });
        }
        let decoded = decode_envelope(&bytes)?;
        verify_with_compiled_acceptance_key(&decoded)?;
        let payload = decode_payload(&decoded.payload)?;
        let verified = validate_payload(payload, &bytes)?;
        require_encrypted_profile_marker(profile_root)?;
        Ok(verified)
    }

    /// Returns the emitted posture; every verification failure maps to synthetic.
    #[must_use]
    pub fn posture(profile_root: &Path) -> Posture {
        match Self::verify(profile_root) {
            Ok(verified) => verified.posture(),
            Err(_) => Posture::synthetic(),
        }
    }
}

struct DecodedEnvelope {
    payload: Vec<u8>,
    public_key: [u8; 32],
    signature: [u8; 64],
}

#[derive(Debug)]
struct DecodedPayload {
    spec_digest: [u8; 32],
    schema_version: u32,
    rows: Vec<PlatformRow>,
}

#[derive(Debug)]
struct PlatformRow {
    platform: String,
    spec_digest: [u8; 32],
    schema_version: u32,
    build_digest: [u8; 32],
    sqlcipher_version: String,
    sqlite_version: String,
    crypto_provider_version: String,
    canary_file_count: u64,
    canary_byte_count: u64,
    canary_hit_count: u64,
    fault_matrix_digest: [u8; 32],
    independent_restore_digest: [u8; 32],
}

fn verify_with_compiled_acceptance_key(decoded: &DecodedEnvelope) -> Result<(), AdmissionError> {
    let AcceptancePublicKey::Provisioned(expected) = ACCEPTANCE_PUBLIC_KEY else {
        return Err(AdmissionError::AcceptanceKeyUnprovisioned);
    };
    if expected == [0_u8; 32] {
        return Err(AdmissionError::InvalidAcceptanceKey);
    }
    if decoded.public_key != expected {
        return Err(AdmissionError::SignerKeyMismatch);
    }
    let verifying =
        VerifyingKey::from_bytes(&expected).map_err(|_| AdmissionError::InvalidAcceptanceKey)?;
    verifying
        .verify_strict(&decoded.payload, &Signature::from_bytes(&decoded.signature))
        .map_err(|_| AdmissionError::InvalidSignature)
}

fn validate_payload(
    payload: DecodedPayload,
    envelope_bytes: &[u8],
) -> Result<VerifiedAdmission, AdmissionError> {
    if payload.spec_digest != CANONICAL_SPEC_DIGEST {
        return Err(AdmissionError::StaleSpecDigest);
    }
    if payload.schema_version != COMPILED_STORE_SCHEMA_VERSION {
        return Err(AdmissionError::SchemaVersionMismatch {
            expected: COMPILED_STORE_SCHEMA_VERSION,
            actual: payload.schema_version,
        });
    }
    let mut platforms = Vec::with_capacity(REQUIRED_ADMISSION_PLATFORMS.len());
    for required in REQUIRED_ADMISSION_PLATFORMS {
        let mut matches = payload.rows.iter().filter(|row| row.platform == required);
        let Some(row) = matches.next() else {
            return Err(AdmissionError::MissingPlatformRow { platform: required });
        };
        if matches.next().is_some() {
            return Err(AdmissionError::DuplicatePlatformRow { platform: required });
        }
        validate_platform_row(row)?;
        platforms.push(required.to_owned());
    }
    for row in &payload.rows {
        if !REQUIRED_ADMISSION_PLATFORMS.contains(&row.platform.as_str()) {
            return Err(AdmissionError::UnexpectedPlatformRow {
                platform: row.platform.clone(),
            });
        }
    }
    if payload.rows.len() != REQUIRED_ADMISSION_PLATFORMS.len() {
        return Err(AdmissionError::InvalidPlatformEvidence {
            platform: "receipt",
            field: "rows",
        });
    }
    Ok(VerifiedAdmission {
        receipt_digest: lowercase_hex(&Sha256::digest(envelope_bytes)),
        platforms,
    })
}

fn validate_platform_row(row: &PlatformRow) -> Result<(), AdmissionError> {
    if row.spec_digest != CANONICAL_SPEC_DIGEST
        || row.schema_version != COMPILED_STORE_SCHEMA_VERSION
    {
        return Err(AdmissionError::StalePlatformRow {
            platform: row.platform.clone(),
        });
    }
    for (field, value) in [
        ("build_digest", row.build_digest),
        ("fault_matrix_digest", row.fault_matrix_digest),
        ("independent_restore_digest", row.independent_restore_digest),
    ] {
        if value == [0_u8; 32] {
            return Err(AdmissionError::InvalidPlatformEvidence {
                platform: required_platform_name(&row.platform),
                field,
            });
        }
    }
    for (field, value) in [
        ("sqlcipher_version", row.sqlcipher_version.as_str()),
        ("sqlite_version", row.sqlite_version.as_str()),
        (
            "crypto_provider_version",
            row.crypto_provider_version.as_str(),
        ),
    ] {
        if value.is_empty() || value.len() > 128 || !value.is_ascii() {
            return Err(AdmissionError::InvalidPlatformEvidence {
                platform: required_platform_name(&row.platform),
                field,
            });
        }
    }
    if row.canary_file_count == 0 {
        return Err(AdmissionError::InvalidPlatformEvidence {
            platform: required_platform_name(&row.platform),
            field: "canary_file_count",
        });
    }
    if row.canary_byte_count == 0 {
        return Err(AdmissionError::InvalidPlatformEvidence {
            platform: required_platform_name(&row.platform),
            field: "canary_byte_count",
        });
    }
    if row.canary_hit_count != 0 {
        return Err(AdmissionError::InvalidPlatformEvidence {
            platform: required_platform_name(&row.platform),
            field: "canary_hit_count",
        });
    }
    Ok(())
}

fn required_platform_name(platform: &str) -> &'static str {
    REQUIRED_ADMISSION_PLATFORMS
        .iter()
        .copied()
        .find(|candidate| *candidate == platform)
        .unwrap_or("unexpected-platform")
}

fn require_encrypted_profile_marker(profile_root: &Path) -> Result<(), AdmissionError> {
    if profile_root.join(SYNTHETIC_PROFILE_MARKER).exists() {
        return Err(AdmissionError::InvalidProfileFormat);
    }
    let path = profile_root.join(ENCRYPTED_PROFILE_MARKER);
    let marker = fs::read(&path).map_err(|_| AdmissionError::InvalidProfileFormat)?;
    if marker != ENCRYPTED_PROFILE_MARKER_BYTES {
        return Err(AdmissionError::InvalidProfileFormat);
    }
    Ok(())
}

fn decode_envelope(bytes: &[u8]) -> Result<DecodedEnvelope, AdmissionError> {
    let value = decode_single(bytes)?;
    let Value::Array(mut fields) = value else {
        return Err(AdmissionError::MalformedReceipt(
            "envelope must be an array",
        ));
    };
    if fields.len() != 4 {
        return Err(AdmissionError::MalformedReceipt(
            "envelope must contain four fields",
        ));
    }
    let signature = fixed_bytes::<64>(fields.pop(), "signature")?;
    let public_key = fixed_bytes::<32>(fields.pop(), "public key")?;
    let payload = bytes_field(fields.pop(), "payload")?;
    let version = unsigned(fields.pop(), "envelope version")?;
    if version != u64::from(SIGNED_ENVELOPE_VERSION) {
        return Err(AdmissionError::UnsupportedEnvelopeVersion { actual: version });
    }
    let canonical = encode_value(&Value::Array(vec![
        Value::Integer(Integer::from(version)),
        Value::Bytes(payload.clone()),
        Value::Bytes(public_key.to_vec()),
        Value::Bytes(signature.to_vec()),
    ]))?;
    if canonical != bytes {
        return Err(AdmissionError::NonCanonicalReceipt);
    }
    Ok(DecodedEnvelope {
        payload,
        public_key,
        signature,
    })
}

fn decode_payload(bytes: &[u8]) -> Result<DecodedPayload, AdmissionError> {
    let value = decode_single(bytes)?;
    let Value::Array(mut fields) = value else {
        return Err(AdmissionError::MalformedReceipt("payload must be an array"));
    };
    if fields.len() != 4 {
        return Err(AdmissionError::MalformedReceipt(
            "payload must contain four fields",
        ));
    }
    let rows_value = fields
        .pop()
        .ok_or(AdmissionError::MalformedReceipt("platform rows are absent"))?;
    let schema_version = u32::try_from(unsigned(fields.pop(), "schema version")?)
        .map_err(|_| AdmissionError::MalformedReceipt("schema version exceeds u32"))?;
    let spec_digest = fixed_bytes::<32>(fields.pop(), "spec digest")?;
    let version = unsigned(fields.pop(), "receipt version")?;
    if version != u64::from(ADMISSION_RECEIPT_VERSION) {
        return Err(AdmissionError::UnsupportedReceiptVersion { actual: version });
    }
    let Value::Array(row_values) = rows_value else {
        return Err(AdmissionError::MalformedReceipt(
            "platform rows must be an array",
        ));
    };
    let rows = row_values
        .into_iter()
        .map(decode_platform_row)
        .collect::<Result<Vec<_>, _>>()?;
    if encode_payload(&spec_digest, schema_version, &rows)? != bytes {
        return Err(AdmissionError::NonCanonicalReceipt);
    }
    Ok(DecodedPayload {
        spec_digest,
        schema_version,
        rows,
    })
}

fn decode_platform_row(value: Value) -> Result<PlatformRow, AdmissionError> {
    let Value::Array(mut fields) = value else {
        return Err(AdmissionError::MalformedReceipt(
            "platform row must be an array",
        ));
    };
    if fields.len() != 12 {
        return Err(AdmissionError::MalformedReceipt(
            "platform row must contain twelve fields",
        ));
    }
    let independent_restore_digest = fixed_bytes::<32>(fields.pop(), "restore digest")?;
    let fault_matrix_digest = fixed_bytes::<32>(fields.pop(), "fault matrix digest")?;
    let canary_hit_count = unsigned(fields.pop(), "canary hit count")?;
    let canary_byte_count = unsigned(fields.pop(), "canary byte count")?;
    let canary_file_count = unsigned(fields.pop(), "canary file count")?;
    let crypto_provider_version = text_field(fields.pop(), "crypto provider version")?;
    let sqlite_version = text_field(fields.pop(), "SQLite version")?;
    let sqlcipher_version = text_field(fields.pop(), "SQLCipher version")?;
    let build_digest = fixed_bytes::<32>(fields.pop(), "build digest")?;
    let schema_version = u32::try_from(unsigned(fields.pop(), "row schema version")?)
        .map_err(|_| AdmissionError::MalformedReceipt("row schema version exceeds u32"))?;
    let spec_digest = fixed_bytes::<32>(fields.pop(), "row spec digest")?;
    let platform = text_field(fields.pop(), "platform")?;
    Ok(PlatformRow {
        platform,
        spec_digest,
        schema_version,
        build_digest,
        sqlcipher_version,
        sqlite_version,
        crypto_provider_version,
        canary_file_count,
        canary_byte_count,
        canary_hit_count,
        fault_matrix_digest,
        independent_restore_digest,
    })
}

fn encode_payload(
    spec_digest: &[u8; 32],
    schema_version: u32,
    rows: &[PlatformRow],
) -> Result<Vec<u8>, AdmissionError> {
    let row_values = rows.iter().map(platform_row_value).collect();
    encode_value(&Value::Array(vec![
        Value::Integer(Integer::from(u64::from(ADMISSION_RECEIPT_VERSION))),
        Value::Bytes(spec_digest.to_vec()),
        Value::Integer(Integer::from(u64::from(schema_version))),
        Value::Array(row_values),
    ]))
}

fn platform_row_value(row: &PlatformRow) -> Value {
    Value::Array(vec![
        Value::Text(row.platform.clone()),
        Value::Bytes(row.spec_digest.to_vec()),
        Value::Integer(Integer::from(u64::from(row.schema_version))),
        Value::Bytes(row.build_digest.to_vec()),
        Value::Text(row.sqlcipher_version.clone()),
        Value::Text(row.sqlite_version.clone()),
        Value::Text(row.crypto_provider_version.clone()),
        Value::Integer(Integer::from(row.canary_file_count)),
        Value::Integer(Integer::from(row.canary_byte_count)),
        Value::Integer(Integer::from(row.canary_hit_count)),
        Value::Bytes(row.fault_matrix_digest.to_vec()),
        Value::Bytes(row.independent_restore_digest.to_vec()),
    ])
}

fn decode_single(bytes: &[u8]) -> Result<Value, AdmissionError> {
    let mut cursor = Cursor::new(bytes);
    let value = ciborium::de::from_reader(&mut cursor)
        .map_err(|_| AdmissionError::MalformedReceipt("CBOR decoding failed"))?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| AdmissionError::MalformedReceipt("receipt length exceeds u64"))?;
    if cursor.position() != length {
        return Err(AdmissionError::MalformedReceipt("trailing bytes"));
    }
    Ok(value)
}

fn encode_value(value: &Value) -> Result<Vec<u8>, AdmissionError> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes)
        .map_err(|_| AdmissionError::MalformedReceipt("CBOR encoding failed"))?;
    Ok(bytes)
}

fn unsigned(value: Option<Value>, field: &'static str) -> Result<u64, AdmissionError> {
    value
        .and_then(|value| {
            value
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
        })
        .ok_or(AdmissionError::MalformedReceipt(field))
}

fn bytes_field(value: Option<Value>, field: &'static str) -> Result<Vec<u8>, AdmissionError> {
    value
        .and_then(|value| value.as_bytes().cloned())
        .ok_or(AdmissionError::MalformedReceipt(field))
}

fn fixed_bytes<const N: usize>(
    value: Option<Value>,
    field: &'static str,
) -> Result<[u8; N], AdmissionError> {
    let bytes = bytes_field(value, field)?;
    bytes
        .try_into()
        .map_err(|_| AdmissionError::MalformedReceipt(field))
}

fn text_field(value: Option<Value>, field: &'static str) -> Result<String, AdmissionError> {
    value
        .and_then(|value| value.as_text().map(str::to_owned))
        .ok_or(AdmissionError::MalformedReceipt(field))
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

/// Closed fail-closed verifier result vocabulary.
#[derive(Debug, Error)]
pub enum AdmissionError {
    /// The receipt file is absent.
    #[error("admission receipt is absent at {path}")]
    ReceiptAbsent { path: PathBuf },
    /// The receipt could not be read.
    #[error("failed to read admission receipt at {path}: {source}")]
    ReadReceipt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The bounded receipt limit was exceeded.
    #[error("admission receipt is {actual} bytes, above the {maximum}-byte limit")]
    ReceiptTooLarge { actual: usize, maximum: usize },
    /// The receipt shape is invalid.
    #[error("malformed admission receipt: {0}")]
    MalformedReceipt(&'static str),
    /// Decode/re-encode did not preserve exact bytes.
    #[error("admission receipt is not canonical deterministic CBOR")]
    NonCanonicalReceipt,
    /// The signed-envelope version is not accepted.
    #[error("unsupported signed-envelope version {actual}")]
    UnsupportedEnvelopeVersion { actual: u64 },
    /// The receipt payload version is not accepted.
    #[error("unsupported admission receipt version {actual}")]
    UnsupportedReceiptVersion { actual: u64 },
    /// No user's offline acceptance public key is compiled in.
    #[error("the offline acceptance public key is not provisioned in this build")]
    AcceptanceKeyUnprovisioned,
    /// The compiled acceptance key is invalid.
    #[error("the compiled acceptance public key is invalid")]
    InvalidAcceptanceKey,
    /// The envelope names a different signer.
    #[error("receipt signer does not match the compiled acceptance public key")]
    SignerKeyMismatch,
    /// The receipt signature failed.
    #[error("receipt Ed25519 signature did not verify")]
    InvalidSignature,
    /// The receipt targets a stale design document.
    #[error("receipt spec digest does not match the canonical design document")]
    StaleSpecDigest,
    /// The receipt targets a different physical store schema.
    #[error("receipt store schema version {actual} does not match compiled version {expected}")]
    SchemaVersionMismatch { expected: u32, actual: u32 },
    /// One required platform row is absent.
    #[error("required admission platform row is absent: {platform}")]
    MissingPlatformRow { platform: &'static str },
    /// One required platform appears more than once.
    #[error("required admission platform row is duplicated: {platform}")]
    DuplicatePlatformRow { platform: &'static str },
    /// A row names a platform outside the compiled set.
    #[error("receipt carries an unexpected platform row: {platform}")]
    UnexpectedPlatformRow { platform: String },
    /// A platform row targets stale spec/schema facts.
    #[error("platform evidence row is stale: {platform}")]
    StalePlatformRow { platform: String },
    /// A platform evidence field is empty, zeroed, malformed, or reports hits.
    #[error("platform evidence is invalid for {platform}: {field}")]
    InvalidPlatformEvidence {
        platform: &'static str,
        field: &'static str,
    },
    /// The profile marker is absent, malformed, or synthetic.
    #[error("profile is not the compiled encrypted schema-2 format")]
    InvalidProfileFormat,
}

impl AdmissionError {
    /// Stable machine-readable denial reason.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ReceiptAbsent { .. } => "ADMISSION_RECEIPT_ABSENT",
            Self::ReadReceipt { .. } => "ADMISSION_RECEIPT_READ_FAILED",
            Self::ReceiptTooLarge { .. } => "ADMISSION_RECEIPT_TOO_LARGE",
            Self::MalformedReceipt(_) => "ADMISSION_RECEIPT_MALFORMED",
            Self::NonCanonicalReceipt => "ADMISSION_RECEIPT_NON_CANONICAL",
            Self::UnsupportedEnvelopeVersion { .. } => "ADMISSION_ENVELOPE_VERSION_UNSUPPORTED",
            Self::UnsupportedReceiptVersion { .. } => "ADMISSION_RECEIPT_VERSION_UNSUPPORTED",
            Self::AcceptanceKeyUnprovisioned => "ADMISSION_ACCEPTANCE_KEY_UNPROVISIONED",
            Self::InvalidAcceptanceKey => "ADMISSION_ACCEPTANCE_KEY_INVALID",
            Self::SignerKeyMismatch => "ADMISSION_SIGNER_KEY_MISMATCH",
            Self::InvalidSignature => "ADMISSION_SIGNATURE_INVALID",
            Self::StaleSpecDigest => "ADMISSION_SPEC_DIGEST_STALE",
            Self::SchemaVersionMismatch { .. } => "ADMISSION_SCHEMA_VERSION_MISMATCH",
            Self::MissingPlatformRow { .. } => "ADMISSION_PLATFORM_ROW_MISSING",
            Self::DuplicatePlatformRow { .. } => "ADMISSION_PLATFORM_ROW_DUPLICATED",
            Self::UnexpectedPlatformRow { .. } => "ADMISSION_PLATFORM_ROW_UNEXPECTED",
            Self::StalePlatformRow { .. } => "ADMISSION_PLATFORM_ROW_STALE",
            Self::InvalidPlatformEvidence { .. } => "ADMISSION_PLATFORM_EVIDENCE_INVALID",
            Self::InvalidProfileFormat => "ADMISSION_PROFILE_FORMAT_INVALID",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs};

    use ed25519_dalek::{Signer as _, SigningKey};
    use tempfile::TempDir;

    use super::*;

    const TEST_SIGNING_SEED: [u8; 32] = [0x6b; 32];

    fn row(platform: &str, discriminator: u8) -> PlatformRow {
        PlatformRow {
            platform: platform.to_owned(),
            spec_digest: CANONICAL_SPEC_DIGEST,
            schema_version: COMPILED_STORE_SCHEMA_VERSION,
            build_digest: [discriminator; 32],
            sqlcipher_version: "4.14.0 community".to_owned(),
            sqlite_version: "3.51.3".to_owned(),
            crypto_provider_version: "OpenSSL 3.5.4".to_owned(),
            canary_file_count: 114,
            canary_byte_count: 3_231_203,
            canary_hit_count: 0,
            fault_matrix_digest: [discriminator.wrapping_add(1); 32],
            independent_restore_digest: [discriminator.wrapping_add(2); 32],
        }
    }

    fn complete_rows() -> Vec<PlatformRow> {
        REQUIRED_ADMISSION_PLATFORMS
            .iter()
            .enumerate()
            .map(|(index, platform)| row(platform, u8::try_from(index + 1).unwrap_or(1)))
            .collect()
    }

    fn signed_receipt(
        spec_digest: [u8; 32],
        schema_version: u32,
        rows: &[PlatformRow],
        signing: &SigningKey,
    ) -> Result<Vec<u8>, AdmissionError> {
        let payload = encode_payload(&spec_digest, schema_version, rows)?;
        let signature = signing.sign(&payload).to_bytes();
        encode_value(&Value::Array(vec![
            Value::Integer(Integer::from(u64::from(SIGNED_ENVELOPE_VERSION))),
            Value::Bytes(payload),
            Value::Bytes(signing.verifying_key().to_bytes().to_vec()),
            Value::Bytes(signature.to_vec()),
        ]))
    }

    fn profile_with_receipt(receipt: &[u8]) -> Result<TempDir, Box<dyn Error>> {
        let profile = tempfile::tempdir()?;
        fs::write(
            profile.path().join(ENCRYPTED_PROFILE_MARKER),
            ENCRYPTED_PROFILE_MARKER_BYTES,
        )?;
        fs::create_dir(profile.path().join("admission"))?;
        fs::write(
            profile.path().join(ADMISSION_RECEIPT_RELATIVE_PATH),
            receipt,
        )?;
        Ok(profile)
    }

    fn verify_with_test_key(
        profile_root: &Path,
        key_bytes: &[u8],
    ) -> Result<VerifiedAdmission, AdmissionError> {
        let bytes =
            fs::read(profile_root.join(ADMISSION_RECEIPT_RELATIVE_PATH)).map_err(|source| {
                AdmissionError::ReadReceipt {
                    path: profile_root.join(ADMISSION_RECEIPT_RELATIVE_PATH),
                    source,
                }
            })?;
        let decoded = decode_envelope(&bytes)?;
        let key: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| AdmissionError::InvalidAcceptanceKey)?;
        if key == [0_u8; 32] || decoded.public_key != key {
            return Err(AdmissionError::InvalidAcceptanceKey);
        }
        let verifying =
            VerifyingKey::from_bytes(&key).map_err(|_| AdmissionError::InvalidAcceptanceKey)?;
        verifying
            .verify_strict(&decoded.payload, &Signature::from_bytes(&decoded.signature))
            .map_err(|_| AdmissionError::InvalidSignature)?;
        let payload = decode_payload(&decoded.payload)?;
        let verified = validate_payload(payload, &bytes)?;
        require_encrypted_profile_marker(profile_root)?;
        Ok(verified)
    }

    #[test]
    fn admitted_posture_requires_verified_receipt() -> Result<(), Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&TEST_SIGNING_SEED);
        let receipt = signed_receipt(
            CANONICAL_SPEC_DIGEST,
            COMPILED_STORE_SCHEMA_VERSION,
            &complete_rows(),
            &signing,
        )?;
        let profile = profile_with_receipt(&receipt)?;
        assert!(matches!(
            AdmissionVerifier::verify(profile.path()),
            Err(AdmissionError::AcceptanceKeyUnprovisioned)
        ));
        let posture = AdmissionVerifier::posture(profile.path());
        assert!(!posture.production_data_allowed());
        assert_eq!(posture, Posture::synthetic());
        Ok(())
    }

    #[test]
    fn missing_platform_row_denies_admission() -> Result<(), Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&TEST_SIGNING_SEED);
        let mut rows = complete_rows();
        rows.remove(1);
        let receipt = signed_receipt(
            CANONICAL_SPEC_DIGEST,
            COMPILED_STORE_SCHEMA_VERSION,
            &rows,
            &signing,
        )?;
        let profile = profile_with_receipt(&receipt)?;
        assert!(matches!(
            verify_with_test_key(profile.path(), &signing.verifying_key().to_bytes()),
            Err(AdmissionError::MissingPlatformRow {
                platform: "windows-aarch64"
            })
        ));
        Ok(())
    }

    #[test]
    fn stale_spec_digest_denies_admission() -> Result<(), Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&TEST_SIGNING_SEED);
        let receipt = signed_receipt(
            [0x55; 32],
            COMPILED_STORE_SCHEMA_VERSION,
            &complete_rows(),
            &signing,
        )?;
        let profile = profile_with_receipt(&receipt)?;
        assert!(matches!(
            verify_with_test_key(profile.path(), &signing.verifying_key().to_bytes()),
            Err(AdmissionError::StaleSpecDigest)
        ));
        Ok(())
    }

    #[test]
    fn forged_signature_denies_admission() -> Result<(), Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&TEST_SIGNING_SEED);
        let receipt = signed_receipt(
            CANONICAL_SPEC_DIGEST,
            COMPILED_STORE_SCHEMA_VERSION,
            &complete_rows(),
            &signing,
        )?;
        let mut decoded = decode_envelope(&receipt)?;
        decoded.signature[0] ^= 1;
        let forged = encode_value(&Value::Array(vec![
            Value::Integer(Integer::from(u64::from(SIGNED_ENVELOPE_VERSION))),
            Value::Bytes(decoded.payload),
            Value::Bytes(decoded.public_key.to_vec()),
            Value::Bytes(decoded.signature.to_vec()),
        ]))?;
        let profile = profile_with_receipt(&forged)?;
        assert!(matches!(
            verify_with_test_key(profile.path(), &signing.verifying_key().to_bytes()),
            Err(AdmissionError::InvalidSignature)
        ));
        Ok(())
    }

    #[test]
    fn unprovisioned_empty_zero_byte_and_all_zero_keys_fail_closed() -> Result<(), Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&TEST_SIGNING_SEED);
        let receipt = signed_receipt(
            CANONICAL_SPEC_DIGEST,
            COMPILED_STORE_SCHEMA_VERSION,
            &complete_rows(),
            &signing,
        )?;
        let profile = profile_with_receipt(&receipt)?;
        assert!(matches!(
            AdmissionVerifier::verify(profile.path()),
            Err(AdmissionError::AcceptanceKeyUnprovisioned)
        ));
        for invalid in [&[][..], &[0_u8][..], &[0_u8; 32][..]] {
            assert!(matches!(
                verify_with_test_key(profile.path(), invalid),
                Err(AdmissionError::InvalidAcceptanceKey)
            ));
        }
        Ok(())
    }

    #[test]
    fn synthetic_posture_is_emitted_when_receipt_absent() -> Result<(), Box<dyn Error>> {
        let profile = tempfile::tempdir()?;
        let posture = AdmissionVerifier::posture(profile.path());
        assert_eq!(posture, Posture::synthetic());
        assert_eq!(
            posture.canonical_json_bytes(),
            serde_json::to_vec(&posture)?
        );
        assert!(!posture.production_data_allowed());
        Ok(())
    }

    #[test]
    fn candidate_receipt_has_first_windows_and_second_linux_rows() -> Result<(), Box<dyn Error>> {
        let signing = SigningKey::from_bytes(&TEST_SIGNING_SEED);
        let rows = vec![row("windows-x86_64", 1), row("linux-x86_64", 2)];
        let receipt = signed_receipt(
            CANONICAL_SPEC_DIGEST,
            COMPILED_STORE_SCHEMA_VERSION,
            &rows,
            &signing,
        )?;
        assert_eq!(
            lowercase_hex(&receipt),
            include_str!("../../../testdata/admission/incomplete-receipt.cbor.hex").trim()
        );
        let profile = profile_with_receipt(&receipt)?;
        assert!(matches!(
            verify_with_test_key(profile.path(), &signing.verifying_key().to_bytes()),
            Err(AdmissionError::MissingPlatformRow {
                platform: "windows-aarch64"
            })
        ));
        Ok(())
    }
}
