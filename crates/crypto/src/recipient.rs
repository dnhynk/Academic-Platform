//! The recipient record: the only thing written to `keys/recipients.cbor`.
//!
//! ADR-005 requires each record to carry `recipient_id`, `kind`, the KDF and
//! wrap algorithm identifiers, their parameters, the wrapped VMK, and a MAC over
//! the record taken under the VMK. The layout below is the frozen CBOR shape of
//! that requirement.
//!
//! Two independent checks stand between a wrong key and a plaintext VMK:
//!
//! 1. the AEAD tag, over an AAD that is the canonical encoding of the record's
//!    identity, algorithm, and parameter fields, so a tampered parameter or a
//!    wrong wrapping key fails before any plaintext is produced; and
//! 2. the record MAC under the VMK, verified after unwrapping, which is what
//!    catches a record whose MAC field was replaced or lifted from elsewhere.
//!
//! The VMK itself is never encoded. `wrapped_vmk` is always ciphertext.

use chacha20poly1305::{
    KeyInit as _, XChaCha20Poly1305, XNonce,
    aead::{Aead as _, Payload},
};
use ciborium::value::{Integer, Value};
use hmac::{Hmac, Mac};
use sha2::Sha512;
use subtle::ConstantTimeEq as _;
use zeroize::Zeroizing;

use crate::{
    keys::{
        IDENTIFIER_BYTES, KEY_BYTES, ProfileId, RandomnessUnavailable, RecipientWrapKey,
        VaultMasterKey,
    },
    recovery::Argon2idProfile,
};

/// Frozen record version.
pub const RECORD_VERSION: u8 = 1;
/// Frozen recipient-set container version.
pub const SET_VERSION: u8 = 1;

/// Nonce width of the wrap AEAD.
pub const WRAP_NONCE_BYTES: usize = 24;
/// Ciphertext-plus-tag width of a wrapped 32-byte VMK.
pub const WRAPPED_VMK_BYTES: usize = KEY_BYTES + 16;
/// Width of the record MAC.
pub const RECORD_MAC_BYTES: usize = 64;

/// Frozen wrap algorithm identifier.
pub const WRAP_ALGORITHM_ID: &str = "XCHACHA20-POLY1305";
/// Frozen key-source identifier for a device recipient.
pub const DEVICE_KDF_ALGORITHM_ID: &str = "OS-KEYSTORE-V1";
/// Frozen key-derivation identifier for a recovery recipient.
pub const RECOVERY_KDF_ALGORITHM_ID: &str = "ARGON2ID";

/// Which key source opens one recipient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecipientKind {
    /// The operating-system broker holds the wrapping key.
    DeviceKeystore,
    /// A 256-bit recovery secret derives the wrapping key through Argon2id.
    RecoverySecret,
}

impl RecipientKind {
    const fn tag(self) -> u8 {
        match self {
            Self::DeviceKeystore => 1,
            Self::RecoverySecret => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::DeviceKeystore),
            2 => Some(Self::RecoverySecret),
            _ => None,
        }
    }

    const fn kdf_algorithm_id(self) -> &'static str {
        match self {
            Self::DeviceKeystore => DEVICE_KDF_ALGORITHM_ID,
            Self::RecoverySecret => RECOVERY_KDF_ALGORITHM_ID,
        }
    }
}

/// The parameters that identify how one recipient's wrapping key is obtained.
///
/// They are stored verbatim in the record and read back on every unlock, so a
/// downgraded Argon2id cost or a swapped broker is visible rather than implied.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecipientParameters {
    /// Broker identity and the label the secret is stored under.
    DeviceKeystore {
        /// Stable spelling of the native broker, e.g. `WINDOWS_DPAPI_CNG`.
        provider: String,
        /// Label the broker stored the device wrapping key under.
        label: String,
    },
    /// The versioned, pinned Argon2id profile and its per-recipient salt.
    RecoverySecret {
        /// The pinned profile.
        profile: Argon2idProfile,
        /// Per-recipient salt.
        salt: [u8; IDENTIFIER_BYTES],
    },
}

impl RecipientParameters {
    const fn kind(&self) -> RecipientKind {
        match self {
            Self::DeviceKeystore { .. } => RecipientKind::DeviceKeystore,
            Self::RecoverySecret { .. } => RecipientKind::RecoverySecret,
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Self::DeviceKeystore { provider, label } => map([
                (0, Value::Text(provider.clone())),
                (1, Value::Text(label.clone())),
            ]),
            Self::RecoverySecret { profile, salt } => map([
                (0, Value::Text(profile.identifier.to_owned())),
                (1, unsigned(profile.memory_kib)),
                (2, unsigned(profile.iterations)),
                (3, unsigned(profile.parallelism)),
                (4, unsigned(u32::try_from(KEY_BYTES).unwrap_or(u32::MAX))),
                (5, Value::Bytes(salt.to_vec())),
            ]),
        }
    }

    fn from_value(kind: RecipientKind, value: &Value) -> Result<Self, RecordError> {
        let entries = value.as_map().ok_or(RecordError::Shape)?;
        match kind {
            RecipientKind::DeviceKeystore => {
                expect_keys(entries, &[0, 1])?;
                Ok(Self::DeviceKeystore {
                    provider: text(entries, 0)?,
                    label: text(entries, 1)?,
                })
            }
            RecipientKind::RecoverySecret => {
                expect_keys(entries, &[0, 1, 2, 3, 4, 5])?;
                let output_len = integer(entries, 4)?;
                if usize::try_from(output_len).unwrap_or(usize::MAX) != KEY_BYTES {
                    return Err(RecordError::Shape);
                }
                let salt_bytes = bytes(entries, 5)?;
                let salt = <[u8; IDENTIFIER_BYTES]>::try_from(salt_bytes.as_slice())
                    .map_err(|_| RecordError::Shape)?;
                Ok(Self::RecoverySecret {
                    profile: Argon2idProfile::from_record(
                        &text(entries, 0)?,
                        integer(entries, 1)?,
                        integer(entries, 2)?,
                        integer(entries, 3)?,
                    )?,
                    salt,
                })
            }
        }
    }
}

/// One wrapped copy of the Vault Master Key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipientRecord {
    profile_id: ProfileId,
    recipient_id: [u8; IDENTIFIER_BYTES],
    parameters: RecipientParameters,
    wrap_nonce: [u8; WRAP_NONCE_BYTES],
    wrapped_vmk: Vec<u8>,
    keystore_blob: Vec<u8>,
    record_mac: Vec<u8>,
}

/// Failure while encoding, decoding, or verifying a recipient record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecordError {
    /// The bytes are not well-formed CBOR.
    #[error("the recipient record is not well-formed CBOR")]
    Malformed,
    /// The bytes are well-formed but not the one canonical encoding.
    #[error("the recipient record is not in canonical deterministic CBOR")]
    NonCanonical,
    /// A field is missing, duplicated, of the wrong type, or of the wrong width.
    #[error("the recipient record has an unexpected shape")]
    Shape,
    /// The record declares a version this build does not implement.
    #[error("the recipient record declares an unsupported version")]
    UnsupportedVersion,
    /// The record declares an algorithm this build does not implement.
    #[error("the recipient record declares an unsupported algorithm")]
    UnsupportedAlgorithm,
    /// The record declares an Argon2id profile that is not the pinned one.
    #[error("the recipient record declares an unpinned Argon2id profile")]
    UnpinnedKdfProfile,
    /// Encoding failed.
    #[error("the recipient record could not be encoded")]
    Encode,
}

fn unsigned(value: u32) -> Value {
    Value::Integer(Integer::from(value))
}

fn map<const N: usize>(entries: [(u64, Value); N]) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Value::Integer(Integer::from(key)), value))
            .collect(),
    )
}

fn find(entries: &[(Value, Value)], key: u64) -> Result<&Value, RecordError> {
    entries
        .iter()
        .find_map(|(candidate, value)| {
            let matches = candidate
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
                .is_some_and(|integer| integer == key);
            matches.then_some(value)
        })
        .ok_or(RecordError::Shape)
}

/// Requires exactly these keys, in exactly this ascending order, with no extras.
///
/// Deterministic CBOR fixes the ordering; refusing an unknown key keeps a
/// forward-compatible reader from silently ignoring a field that changes meaning.
fn expect_keys(entries: &[(Value, Value)], keys: &[u64]) -> Result<(), RecordError> {
    if entries.len() != keys.len() {
        return Err(RecordError::Shape);
    }
    for (entry, expected) in entries.iter().zip(keys) {
        let actual = entry
            .0
            .as_integer()
            .and_then(|integer| u64::try_from(integer).ok())
            .ok_or(RecordError::Shape)?;
        if actual != *expected {
            return Err(RecordError::Shape);
        }
    }
    Ok(())
}

fn text(entries: &[(Value, Value)], key: u64) -> Result<String, RecordError> {
    find(entries, key)?
        .as_text()
        .map(str::to_owned)
        .ok_or(RecordError::Shape)
}

fn bytes(entries: &[(Value, Value)], key: u64) -> Result<Vec<u8>, RecordError> {
    find(entries, key)?
        .as_bytes()
        .cloned()
        .ok_or(RecordError::Shape)
}

fn integer(entries: &[(Value, Value)], key: u64) -> Result<u32, RecordError> {
    let raw = find(entries, key)?.as_integer().ok_or(RecordError::Shape)?;
    u128::try_from(raw)
        .ok()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(RecordError::Shape)
}

fn fixed<const N: usize>(entries: &[(Value, Value)], key: u64) -> Result<[u8; N], RecordError> {
    <[u8; N]>::try_from(bytes(entries, key)?.as_slice()).map_err(|_| RecordError::Shape)
}

fn encode(value: &Value) -> Result<Vec<u8>, RecordError> {
    let mut encoded = Vec::new();
    ciborium::into_writer(value, &mut encoded).map_err(|_| RecordError::Encode)?;
    Ok(encoded)
}

/// Decodes CBOR and proves the bytes are the one canonical encoding.
fn decode_canonical(input: &[u8]) -> Result<Value, RecordError> {
    let value: Value = ciborium::from_reader(input).map_err(|_| RecordError::Malformed)?;
    if encode(&value)? != input {
        return Err(RecordError::NonCanonical);
    }
    Ok(value)
}

impl RecipientRecord {
    /// Fields `0..=6`: everything that identifies the recipient and fixes how
    /// its key is obtained. This is the AEAD associated data.
    fn identity_value(&self) -> Value {
        map([
            (0, unsigned(u32::from(RECORD_VERSION))),
            (1, Value::Bytes(self.profile_id.as_bytes().to_vec())),
            (2, Value::Bytes(self.recipient_id.to_vec())),
            (3, unsigned(u32::from(self.parameters.kind().tag()))),
            (
                4,
                Value::Text(self.parameters.kind().kdf_algorithm_id().to_owned()),
            ),
            (5, self.parameters.to_value()),
            (6, Value::Text(WRAP_ALGORITHM_ID.to_owned())),
        ])
    }

    /// Fields `0..=9`: the identity fields plus the wrapped key material and the
    /// broker blob. This is what the record MAC covers.
    fn mac_covered_value(&self) -> Value {
        let Value::Map(mut entries) = self.identity_value() else {
            // `identity_value` always builds a map.
            return Value::Map(Vec::new());
        };
        entries.push((
            Value::Integer(Integer::from(7_u64)),
            Value::Bytes(self.wrap_nonce.to_vec()),
        ));
        entries.push((
            Value::Integer(Integer::from(8_u64)),
            Value::Bytes(self.wrapped_vmk.clone()),
        ));
        entries.push((
            Value::Integer(Integer::from(9_u64)),
            Value::Bytes(self.keystore_blob.clone()),
        ));
        Value::Map(entries)
    }

    fn to_value(&self) -> Value {
        let Value::Map(mut entries) = self.mac_covered_value() else {
            return Value::Map(Vec::new());
        };
        entries.push((
            Value::Integer(Integer::from(10_u64)),
            Value::Bytes(self.record_mac.clone()),
        ));
        Value::Map(entries)
    }

    /// Encodes the record as canonical deterministic CBOR.
    pub fn to_canonical_cbor(&self) -> Result<Vec<u8>, RecordError> {
        encode(&self.to_value())
    }

    /// Decodes and canonicality-checks a record.
    pub fn from_canonical_cbor(input: &[u8]) -> Result<Self, RecordError> {
        let value = decode_canonical(input)?;
        let entries = value.as_map().ok_or(RecordError::Shape)?;
        expect_keys(entries, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10])?;
        if integer(entries, 0)? != u32::from(RECORD_VERSION) {
            return Err(RecordError::UnsupportedVersion);
        }
        let kind_tag = u8::try_from(integer(entries, 3)?).map_err(|_| RecordError::Shape)?;
        let kind = RecipientKind::from_tag(kind_tag).ok_or(RecordError::Shape)?;
        if text(entries, 4)? != kind.kdf_algorithm_id() {
            return Err(RecordError::UnsupportedAlgorithm);
        }
        if text(entries, 6)? != WRAP_ALGORITHM_ID {
            return Err(RecordError::UnsupportedAlgorithm);
        }
        let parameters = RecipientParameters::from_value(kind, find(entries, 5)?)?;
        let wrapped_vmk = bytes(entries, 8)?;
        if wrapped_vmk.len() != WRAPPED_VMK_BYTES {
            return Err(RecordError::Shape);
        }
        let record_mac = bytes(entries, 10)?;
        if record_mac.len() != RECORD_MAC_BYTES {
            return Err(RecordError::Shape);
        }
        Ok(Self {
            profile_id: ProfileId::from_bytes(fixed::<IDENTIFIER_BYTES>(entries, 1)?),
            recipient_id: fixed::<IDENTIFIER_BYTES>(entries, 2)?,
            parameters,
            wrap_nonce: fixed::<WRAP_NONCE_BYTES>(entries, 7)?,
            wrapped_vmk,
            keystore_blob: bytes(entries, 9)?,
            record_mac,
        })
    }

    /// Returns the recipient identity.
    #[must_use]
    pub const fn recipient_id(&self) -> &[u8; IDENTIFIER_BYTES] {
        &self.recipient_id
    }

    /// Returns which key source opens this recipient.
    #[must_use]
    pub const fn kind(&self) -> RecipientKind {
        self.parameters.kind()
    }

    /// Returns the parameters exactly as they are stored, for read-back.
    #[must_use]
    pub const fn parameters(&self) -> &RecipientParameters {
        &self.parameters
    }

    /// Returns the profile this record belongs to.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    /// Returns the opaque broker blob, which carries no key byte on a
    /// stored-key provider and is ciphertext on a sealing provider.
    #[must_use]
    pub fn keystore_blob(&self) -> &[u8] {
        &self.keystore_blob
    }
}

fn mac_key_bytes(
    master: &VaultMasterKey,
    profile: ProfileId,
) -> Result<Zeroizing<[u8; KEY_BYTES]>, RecipientError> {
    let key = master
        .derive_recipient_mac_key(profile)
        .map_err(|_| RecipientError::KeySchedule)?;
    Ok(Zeroizing::new(*key.expose_secret()))
}

fn compute_mac(
    master: &VaultMasterKey,
    profile: ProfileId,
    covered: &[u8],
) -> Result<Vec<u8>, RecipientError> {
    let key = mac_key_bytes(master, profile)?;
    let Ok(mut mac) = <Hmac<Sha512> as Mac>::new_from_slice(key.as_ref()) else {
        return Err(RecipientError::KeySchedule);
    };
    mac.update(covered);
    Ok(mac.finalize().into_bytes().to_vec())
}

/// Failure while wrapping or unwrapping a Vault Master Key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecipientError {
    /// The key schedule refused.
    #[error("the key schedule failed")]
    KeySchedule,
    /// Operating-system randomness was unavailable.
    #[error("operating-system randomness was unavailable")]
    Randomness,
    /// The record could not be encoded, decoded, or is not canonical.
    #[error("the recipient record is unusable: {0}")]
    Record(#[from] RecordError),
    /// The wrapping key did not open this record.
    ///
    /// Deliberately one variant for every wrong-key cause: a wrong device key
    /// and a wrong recovery secret are indistinguishable to a caller.
    #[error("the presented key did not open this recipient")]
    WrongKey,
    /// The record MAC did not verify under the recovered VMK.
    ///
    /// This is an integrity incident, not a wrong-key result.
    #[error("the recipient record failed its integrity check")]
    RecordIntegrity,
    /// The record belongs to another profile.
    #[error("the recipient record belongs to a different profile")]
    ProfileMismatch,
}

impl From<RandomnessUnavailable> for RecipientError {
    fn from(_: RandomnessUnavailable) -> Self {
        Self::Randomness
    }
}

/// Wraps `master` for one recipient under `wrap_key`.
pub(crate) fn wrap(
    master: &VaultMasterKey,
    profile: ProfileId,
    recipient_id: [u8; IDENTIFIER_BYTES],
    parameters: RecipientParameters,
    keystore_blob: Vec<u8>,
    wrap_key: &RecipientWrapKey,
) -> Result<RecipientRecord, RecipientError> {
    let mut nonce_bytes = [0_u8; WRAP_NONCE_BYTES];
    getrandom::fill(&mut nonce_bytes).map_err(|_| RecipientError::Randomness)?;

    let mut record = RecipientRecord {
        profile_id: profile,
        recipient_id,
        parameters,
        wrap_nonce: nonce_bytes,
        wrapped_vmk: Vec::new(),
        keystore_blob,
        record_mac: Vec::new(),
    };

    let associated = encode(&record.identity_value())?;
    let cipher = XChaCha20Poly1305::new(wrap_key.expose_secret().into());
    let sealed = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: master.expose_secret(),
                aad: &associated,
            },
        )
        .map_err(|_| RecipientError::WrongKey)?;
    if sealed.len() != WRAPPED_VMK_BYTES {
        return Err(RecipientError::Record(RecordError::Shape));
    }
    record.wrapped_vmk = sealed;

    let covered = encode(&record.mac_covered_value())?;
    record.record_mac = compute_mac(master, profile, &covered)?;
    Ok(record)
}

/// Unwraps the Vault Master Key from `record` using `wrap_key`.
///
/// The AEAD tag is checked first, so a wrong key never produces plaintext. The
/// record MAC is then verified under the recovered VMK in constant time; a MAC
/// failure is reported as an integrity incident and the recovered bytes are
/// dropped without being returned.
pub(crate) fn unwrap(
    record: &RecipientRecord,
    profile: ProfileId,
    wrap_key: &RecipientWrapKey,
) -> Result<VaultMasterKey, RecipientError> {
    if record.profile_id.as_bytes() != profile.as_bytes() {
        return Err(RecipientError::ProfileMismatch);
    }
    let associated = encode(&record.identity_value())?;
    let cipher = XChaCha20Poly1305::new(wrap_key.expose_secret().into());
    let opened = cipher
        .decrypt(
            XNonce::from_slice(&record.wrap_nonce),
            Payload {
                msg: &record.wrapped_vmk,
                aad: &associated,
            },
        )
        .map_err(|_| RecipientError::WrongKey)?;
    let opened = Zeroizing::new(opened);
    let master_bytes =
        <[u8; KEY_BYTES]>::try_from(opened.as_slice()).map_err(|_| RecipientError::WrongKey)?;
    let master = VaultMasterKey::from_bytes(Zeroizing::new(master_bytes));

    let covered = encode(&record.mac_covered_value())?;
    let expected = compute_mac(&master, profile, &covered)?;
    if expected.ct_eq(&record.record_mac).unwrap_u8() != 1 {
        // `master` is dropped here and zeroized; nothing is returned.
        return Err(RecipientError::RecordIntegrity);
    }
    Ok(master)
}

/// The complete `keys/recipients.cbor` document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipientSet {
    profile_id: ProfileId,
    recipients: Vec<RecipientRecord>,
}

impl RecipientSet {
    /// Builds a set for one profile.
    #[must_use]
    pub const fn new(profile_id: ProfileId) -> Self {
        Self {
            profile_id,
            recipients: Vec::new(),
        }
    }

    /// Appends a record.
    pub fn push(&mut self, record: RecipientRecord) {
        self.recipients.push(record);
    }

    /// Returns the records in stored order.
    #[must_use]
    pub fn records(&self) -> &[RecipientRecord] {
        &self.recipients
    }

    /// Encodes the document as canonical deterministic CBOR.
    pub fn to_canonical_cbor(&self) -> Result<Vec<u8>, RecordError> {
        let records = self
            .recipients
            .iter()
            .map(RecipientRecord::to_value)
            .collect();
        encode(&map([
            (0, unsigned(u32::from(SET_VERSION))),
            (1, Value::Bytes(self.profile_id.as_bytes().to_vec())),
            (2, Value::Array(records)),
        ]))
    }

    /// Decodes and canonicality-checks a document.
    pub fn from_canonical_cbor(input: &[u8]) -> Result<Self, RecordError> {
        let value = decode_canonical(input)?;
        let entries = value.as_map().ok_or(RecordError::Shape)?;
        expect_keys(entries, &[0, 1, 2])?;
        if integer(entries, 0)? != u32::from(SET_VERSION) {
            return Err(RecordError::UnsupportedVersion);
        }
        let profile_id = ProfileId::from_bytes(fixed::<IDENTIFIER_BYTES>(entries, 1)?);
        let array = find(entries, 2)?.as_array().ok_or(RecordError::Shape)?;
        let mut recipients = Vec::with_capacity(array.len());
        for element in array {
            let encoded = encode(element)?;
            let record = RecipientRecord::from_canonical_cbor(&encoded)?;
            if record.profile_id.as_bytes() != profile_id.as_bytes() {
                return Err(RecordError::Shape);
            }
            recipients.push(record);
        }
        Ok(Self {
            profile_id,
            recipients,
        })
    }
}
