//! The backup root key and the recipients that may hold it.
//!
//! # The contract this module exists to keep
//!
//! ADR-012 requires a backup manifest sealed "with a recovery recipient
//! **independent of the live OS wrapper**". Independence here is a property of
//! the key graph, not of an access-control decision, and it fails in two ways
//! that both have to be closed:
//!
//! - **Directly**, if a device recipient could wrap the backup root. There is
//!   no such code path: [`BackupRecipientKind`] has no device variant, so the
//!   type system offers nowhere to put one.
//! - **Transitively**, if the backup root were derived from the Vault Master
//!   Key. The device wrapper unwraps the VMK, so any key derived from the VMK
//!   is a key the device wrapper produces. The backup root is therefore a
//!   *root*: 32 fresh random bytes with no derivation edge from the VMK, and
//!   nothing in this crate accepts a [`VaultMasterKey`] at all.
//!
//! The consequence is deliberate and is the same fact section 3.3 states in
//! words: under `DEVICE_ONLY` there is no recipient that survives the loss of
//! the device, so there is no backup key, so there is no backup.
//!
//! [`VaultMasterKey`]: academic_crypto::VaultMasterKey

use core::fmt;

use academic_crypto::{
    Argon2idProfile, IDENTIFIER_BYTES, KEY_BYTES, RECOVERY_ARGON2ID_V1, RandomnessUnavailable,
    RecoverySecret,
};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use ciborium::value::{Integer, Value};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha512;
use subtle::ConstantTimeEq as _;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::profile::{RecipientRequirement, RecoveryProfile, RecoveryProfileError};

/// Version of the backup recipient record and set.
pub const BACKUP_RECIPIENT_SET_VERSION: u8 = 1;
/// Frozen wrap algorithm for a backup recipient.
pub const BACKUP_WRAP_ALGORITHM_ID: &str = "XCHACHA20-POLY1305";
/// Info string for the key a backup recipient record's MAC is taken under.
pub const BACKUP_ROOT_INFO: &[u8] = b"academic-os/backup-recipient-mac/v1";
/// Bytes of the wrapped backup root: the key plus the AEAD tag.
pub const WRAPPED_ROOT_BYTES: usize = KEY_BYTES + 16;
/// Bytes of the XChaCha20-Poly1305 nonce.
pub const WRAP_NONCE_BYTES: usize = 24;
/// Bytes of the record MAC.
pub const RECORD_MAC_BYTES: usize = 64;

/// Identity of one backup key set.
///
/// It is the HKDF salt for every key derived from the root, so two backup sets
/// made from the same phrase still derive different manifest keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackupSetId([u8; IDENTIFIER_BYTES]);

impl BackupSetId {
    /// Wraps 16 exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; IDENTIFIER_BYTES]) -> Self {
        Self(bytes)
    }

    /// Generates a fresh identity.
    pub fn generate() -> Result<Self, BackupKeyError> {
        let mut bytes = [0_u8; IDENTIFIER_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| BackupKeyError::Randomness)?;
        Ok(Self(bytes))
    }

    /// Borrows the raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; IDENTIFIER_BYTES] {
        &self.0
    }
}

/// The backup root: 32 random bytes, never persisted unwrapped.
///
/// It is intentionally not constructible from a [`VaultMasterKey`]. The only
/// ways to obtain one are [`BackupMasterKey::generate`] and
/// [`BackupRecipientRecord::open`], and the second needs a recovery secret.
///
/// [`VaultMasterKey`]: academic_crypto::VaultMasterKey
pub struct BackupMasterKey(Zeroizing<[u8; KEY_BYTES]>);

impl BackupMasterKey {
    /// Generates a fresh backup root from operating-system randomness.
    pub fn generate() -> Result<Self, BackupKeyError> {
        let mut bytes = Zeroizing::new([0_u8; KEY_BYTES]);
        getrandom::fill(bytes.as_mut()).map_err(|_| BackupKeyError::Randomness)?;
        Ok(Self(bytes))
    }

    const fn from_zeroizing(bytes: Zeroizing<[u8; KEY_BYTES]>) -> Self {
        Self(bytes)
    }

    /// Borrows the raw key bytes for the length of the call.
    ///
    /// The name is deliberate and greppable: every call site is a place a
    /// reviewer must confirm the bytes do not escape.
    #[must_use]
    pub fn expose_secret(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    /// Derives a purpose key from the root, salted by the backup set identity.
    pub(crate) fn derive(
        &self,
        set_id: BackupSetId,
        info: &[u8],
    ) -> Result<Zeroizing<[u8; KEY_BYTES]>, BackupKeyError> {
        let extracted = Hkdf::<Sha512>::new(Some(set_id.as_bytes()), self.expose_secret());
        let mut output = Zeroizing::new([0_u8; KEY_BYTES]);
        extracted
            .expand(info, output.as_mut())
            .map_err(|_| BackupKeyError::Derivation)?;
        Ok(output)
    }
}

impl std::fmt::Debug for BackupMasterKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BackupMasterKey")
            .field("bytes", &"<redacted>")
            .field("len", &KEY_BYTES)
            .finish()
    }
}

impl Zeroize for BackupMasterKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for BackupMasterKey {}

/// Which secret opens one backup recipient.
///
/// **There is no device-keystore variant, and adding one would break the
/// contract this module exists to keep.** Both variants are held by the user
/// away from the device, so both survive an OS reimage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackupRecipientKind {
    /// The printed 24-word recovery phrase.
    RecoveryPhrase,
    /// A key file the user stores separately from the device.
    OfflineKeyFile,
}

impl BackupRecipientKind {
    const fn tag(self) -> u8 {
        match self {
            Self::RecoveryPhrase => 1,
            Self::OfflineKeyFile => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::RecoveryPhrase),
            2 => Some(Self::OfflineKeyFile),
            _ => None,
        }
    }

    /// Returns the stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RecoveryPhrase => "RECOVERY_PHRASE",
            Self::OfflineKeyFile => "OFFLINE_KEY_FILE",
        }
    }

    /// The section 3.3 requirement this recipient satisfies.
    #[must_use]
    pub const fn requirement(self) -> RecipientRequirement {
        match self {
            Self::RecoveryPhrase => RecipientRequirement::RecoveryPhrase,
            Self::OfflineKeyFile => RecipientRequirement::OfflineKeyFile,
        }
    }

    /// Maps a section 3.3 requirement onto a backup recipient kind.
    ///
    /// Returns `None` for [`RecipientRequirement::DeviceKeystore`]: that is the
    /// total function stating there is no device backup recipient.
    #[must_use]
    pub const fn from_requirement(requirement: RecipientRequirement) -> Option<Self> {
        match requirement {
            RecipientRequirement::DeviceKeystore => None,
            RecipientRequirement::RecoveryPhrase => Some(Self::RecoveryPhrase),
            RecipientRequirement::OfflineKeyFile => Some(Self::OfflineKeyFile),
        }
    }
}

/// One wrapped copy of the backup root.
#[derive(Clone, PartialEq, Eq)]
pub struct BackupRecipientRecord {
    set_id: BackupSetId,
    recipient_id: [u8; IDENTIFIER_BYTES],
    kind: BackupRecipientKind,
    kdf: Argon2idProfile,
    salt: [u8; IDENTIFIER_BYTES],
    wrap_nonce: [u8; WRAP_NONCE_BYTES],
    wrapped_root: Vec<u8>,
    record_mac: Vec<u8>,
}

impl fmt::Debug for BackupRecipientRecord {
    /// Redacting: the wrapped backup root reaches the formatter only as a
    /// length. It is the backup's half of `BackupMasterKey`, which this crate
    /// already registers and hand-writes a redacting `Debug` for.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupRecipientRecord")
            .field("recipient_id", &self.recipient_id)
            .field("kind", &self.kind)
            .field("wrapped_root_len", &self.wrapped_root.len())
            .finish_non_exhaustive()
    }
}

impl BackupRecipientRecord {
    /// Returns the backup set this record belongs to.
    #[must_use]
    pub const fn set_id(&self) -> BackupSetId {
        self.set_id
    }

    /// Returns the recipient identity.
    #[must_use]
    pub const fn recipient_id(&self) -> &[u8; IDENTIFIER_BYTES] {
        &self.recipient_id
    }

    /// Returns which secret opens this record.
    #[must_use]
    pub const fn kind(&self) -> BackupRecipientKind {
        self.kind
    }

    /// Returns the pinned Argon2id profile the wrapping key is derived with.
    #[must_use]
    pub const fn kdf(&self) -> Argon2idProfile {
        self.kdf
    }

    /// Wraps a backup root for one recovery-class secret.
    pub fn wrap(
        root: &BackupMasterKey,
        set_id: BackupSetId,
        recipient_id: [u8; IDENTIFIER_BYTES],
        kind: BackupRecipientKind,
        secret: &RecoverySecret,
    ) -> Result<Self, BackupKeyError> {
        let mut salt = [0_u8; IDENTIFIER_BYTES];
        getrandom::fill(&mut salt).map_err(|_| BackupKeyError::Randomness)?;
        let mut wrap_nonce = [0_u8; WRAP_NONCE_BYTES];
        getrandom::fill(&mut wrap_nonce).map_err(|_| BackupKeyError::Randomness)?;
        let kdf = RECOVERY_ARGON2ID_V1;
        let wrap_key = kdf
            .derive_wrap_key(secret, &salt)
            .map_err(|_| BackupKeyError::Derivation)?;

        let mut record = Self {
            set_id,
            recipient_id,
            kind,
            kdf,
            salt,
            wrap_nonce,
            wrapped_root: Vec::new(),
            record_mac: Vec::new(),
        };
        let aad = record.identity_bytes()?;
        let cipher = XChaCha20Poly1305::new_from_slice(wrap_key.expose_secret())
            .map_err(|_| BackupKeyError::Wrap)?;
        record.wrapped_root = cipher
            .encrypt(
                XNonce::from_slice(&wrap_nonce),
                Payload {
                    msg: root.expose_secret(),
                    aad: &aad,
                },
            )
            .map_err(|_| BackupKeyError::Wrap)?;
        record.record_mac = record.compute_mac(root)?;
        Ok(record)
    }

    /// Recovers the backup root from a recovery-class secret.
    ///
    /// A wrong secret and a tampered record are told apart: the first is an
    /// ordinary refusal and the second is an integrity incident, exactly as
    /// `KY06`/`KY07` require of the profile's own recipients.
    pub fn open(&self, secret: &RecoverySecret) -> Result<BackupMasterKey, BackupKeyError> {
        let wrap_key = self
            .kdf
            .derive_wrap_key(secret, &self.salt)
            .map_err(|_| BackupKeyError::Derivation)?;
        self.open_with_wrapping_key_bytes(wrap_key.expose_secret())
    }

    /// Recovers the backup root from an already-derived wrapping key.
    ///
    /// This exists so a caller can present a wrapping key that came from
    /// somewhere other than the pinned KDF — a device wrapping key, for
    /// instance — and observe that it is refused. It is not a second key
    /// source: it derives nothing, generates nothing, and admits nothing the
    /// [`open`](Self::open) path would not admit.
    pub fn open_with_wrapping_key_bytes(
        &self,
        wrapping_key: &[u8; KEY_BYTES],
    ) -> Result<BackupMasterKey, BackupKeyError> {
        let aad = self.identity_bytes()?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(wrapping_key).map_err(|_| BackupKeyError::Wrap)?;
        let opened = cipher
            .decrypt(
                XNonce::from_slice(&self.wrap_nonce),
                Payload {
                    msg: self.wrapped_root.as_slice(),
                    aad: &aad,
                },
            )
            .map_err(|_| BackupKeyError::WrongSecret)?;
        let mut bytes = Zeroizing::new(
            <[u8; KEY_BYTES]>::try_from(opened.as_slice()).map_err(|_| BackupKeyError::Shape)?,
        );
        let root = BackupMasterKey::from_zeroizing(std::mem::replace(
            &mut bytes,
            Zeroizing::new([0_u8; KEY_BYTES]),
        ));
        let expected = self.compute_mac(&root)?;
        if expected.ct_eq(self.record_mac.as_slice()).into() {
            Ok(root)
        } else {
            Err(BackupKeyError::RecordIntegrity)
        }
    }

    fn compute_mac(&self, root: &BackupMasterKey) -> Result<Vec<u8>, BackupKeyError> {
        let mac_key = root.derive(self.set_id, BACKUP_ROOT_INFO)?;
        let mut mac = <Hmac<Sha512> as Mac>::new_from_slice(mac_key.as_ref())
            .map_err(|_| BackupKeyError::Derivation)?;
        mac.update(&self.mac_covered_bytes()?);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    fn identity_value(&self) -> Value {
        map([
            (0, unsigned(u32::from(BACKUP_RECIPIENT_SET_VERSION))),
            (1, Value::Bytes(self.set_id.as_bytes().to_vec())),
            (2, Value::Bytes(self.recipient_id.to_vec())),
            (3, unsigned(u32::from(self.kind.tag()))),
            (4, Value::Text(self.kdf.identifier.to_owned())),
            (5, unsigned(self.kdf.memory_kib)),
            (6, unsigned(self.kdf.iterations)),
            (7, unsigned(self.kdf.parallelism)),
            (8, Value::Bytes(self.salt.to_vec())),
            (9, Value::Text(BACKUP_WRAP_ALGORITHM_ID.to_owned())),
        ])
    }

    fn identity_bytes(&self) -> Result<Vec<u8>, BackupKeyError> {
        encode(&self.identity_value())
    }

    fn mac_covered_value(&self) -> Value {
        let Value::Map(mut entries) = self.identity_value() else {
            return Value::Map(Vec::new());
        };
        entries.push((
            Value::Integer(Integer::from(10_u64)),
            Value::Bytes(self.wrap_nonce.to_vec()),
        ));
        entries.push((
            Value::Integer(Integer::from(11_u64)),
            Value::Bytes(self.wrapped_root.clone()),
        ));
        Value::Map(entries)
    }

    fn mac_covered_bytes(&self) -> Result<Vec<u8>, BackupKeyError> {
        encode(&self.mac_covered_value())
    }

    fn to_value(&self) -> Value {
        let Value::Map(mut entries) = self.mac_covered_value() else {
            return Value::Map(Vec::new());
        };
        entries.push((
            Value::Integer(Integer::from(12_u64)),
            Value::Bytes(self.record_mac.clone()),
        ));
        Value::Map(entries)
    }

    /// Encodes the record as canonical deterministic CBOR.
    pub fn to_canonical_cbor(&self) -> Result<Vec<u8>, BackupKeyError> {
        encode(&self.to_value())
    }

    /// Decodes and canonicality-checks a record.
    pub fn from_canonical_cbor(input: &[u8]) -> Result<Self, BackupKeyError> {
        let value = decode_canonical(input)?;
        let entries = value.as_map().ok_or(BackupKeyError::Shape)?;
        expect_keys(entries, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])?;
        if integer(entries, 0)? != u32::from(BACKUP_RECIPIENT_SET_VERSION) {
            return Err(BackupKeyError::UnsupportedVersion);
        }
        if text(entries, 9)? != BACKUP_WRAP_ALGORITHM_ID {
            return Err(BackupKeyError::UnsupportedAlgorithm);
        }
        let kind = u8::try_from(integer(entries, 3)?)
            .ok()
            .and_then(BackupRecipientKind::from_tag)
            .ok_or(BackupKeyError::UnsupportedAlgorithm)?;
        let kdf = pinned_profile(
            &text(entries, 4)?,
            integer(entries, 5)?,
            integer(entries, 6)?,
            integer(entries, 7)?,
        )?;
        let wrapped_root = bytes(entries, 11)?;
        if wrapped_root.len() != WRAPPED_ROOT_BYTES {
            return Err(BackupKeyError::Shape);
        }
        let record_mac = bytes(entries, 12)?;
        if record_mac.len() != RECORD_MAC_BYTES {
            return Err(BackupKeyError::Shape);
        }
        Ok(Self {
            set_id: BackupSetId::from_bytes(fixed(entries, 1)?),
            recipient_id: fixed(entries, 2)?,
            kind,
            kdf,
            salt: fixed(entries, 8)?,
            wrap_nonce: fixed(entries, 10)?,
            wrapped_root,
            record_mac,
        })
    }
}

/// Every recipient that may open one backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRecipientSet {
    set_id: BackupSetId,
    records: Vec<BackupRecipientRecord>,
}

impl BackupRecipientSet {
    /// Creates an empty set bound to one backup identity.
    #[must_use]
    pub const fn new(set_id: BackupSetId) -> Self {
        Self {
            set_id,
            records: Vec::new(),
        }
    }

    /// Returns the backup identity every record must carry.
    #[must_use]
    pub const fn set_id(&self) -> BackupSetId {
        self.set_id
    }

    /// Appends one record.
    pub fn push(&mut self, record: BackupRecipientRecord) -> Result<(), BackupKeyError> {
        if record.set_id != self.set_id {
            return Err(BackupKeyError::SetMismatch);
        }
        self.records.push(record);
        Ok(())
    }

    /// Borrows the records in insertion order.
    #[must_use]
    pub fn records(&self) -> &[BackupRecipientRecord] {
        &self.records
    }

    /// Opens the root with a secret, trying each record of the given kind.
    ///
    /// A fresh machine holds a phrase and nothing else; it does not know which
    /// record that phrase belongs to. Trying every record of the right kind is
    /// what makes `fresh_machine_restore_with_phrase_only` possible without a
    /// recipient index the user would have to have kept.
    pub fn open(
        &self,
        kind: BackupRecipientKind,
        secret: &RecoverySecret,
    ) -> Result<BackupMasterKey, BackupKeyError> {
        let mut integrity_incident = false;
        for record in self.records.iter().filter(|record| record.kind == kind) {
            match record.open(secret) {
                Ok(root) => return Ok(root),
                Err(BackupKeyError::RecordIntegrity) => integrity_incident = true,
                Err(BackupKeyError::WrongSecret) => {}
                Err(other) => return Err(other),
            }
        }
        if integrity_incident {
            Err(BackupKeyError::RecordIntegrity)
        } else {
            Err(BackupKeyError::WrongSecret)
        }
    }

    /// Encodes the set as canonical deterministic CBOR.
    pub fn to_canonical_cbor(&self) -> Result<Vec<u8>, BackupKeyError> {
        let records = self
            .records
            .iter()
            .map(BackupRecipientRecord::to_value)
            .collect::<Vec<_>>();
        encode(&map([
            (0, unsigned(u32::from(BACKUP_RECIPIENT_SET_VERSION))),
            (1, Value::Bytes(self.set_id.as_bytes().to_vec())),
            (2, Value::Array(records)),
        ]))
    }

    /// Decodes and canonicality-checks a set.
    pub fn from_canonical_cbor(input: &[u8]) -> Result<Self, BackupKeyError> {
        let value = decode_canonical(input)?;
        let entries = value.as_map().ok_or(BackupKeyError::Shape)?;
        expect_keys(entries, &[0, 1, 2])?;
        if integer(entries, 0)? != u32::from(BACKUP_RECIPIENT_SET_VERSION) {
            return Err(BackupKeyError::UnsupportedVersion);
        }
        let set_id = BackupSetId::from_bytes(fixed(entries, 1)?);
        let array = find(entries, 2)?
            .as_array()
            .ok_or(BackupKeyError::Shape)?
            .clone();
        let mut set = Self::new(set_id);
        for element in &array {
            let record = BackupRecipientRecord::from_canonical_cbor(&encode(element)?)?;
            set.push(record)?;
        }
        Ok(set)
    }
}

/// Creates a backup key set for one selected recovery profile.
///
/// Every secret is presented by the caller in the order
/// [`RecoveryProfile::backup_capable_recipients`] returns, and a profile with
/// no such recipient is refused with its own loss statement quoted back.
pub fn create_backup_key_set(
    profile: RecoveryProfile,
    set_id: BackupSetId,
    secrets: &[(BackupRecipientKind, &RecoverySecret)],
) -> Result<(BackupMasterKey, BackupRecipientSet), BackupKeyError> {
    let expected = profile.backup_capable_recipients();
    if expected.is_empty() {
        return Err(BackupKeyError::Profile(
            RecoveryProfileError::NoIndependentBackupRecipient {
                profile: profile.as_str(),
                statement: profile.loss_statement(),
            },
        ));
    }
    if secrets.len() != expected.len() {
        return Err(BackupKeyError::Profile(
            RecoveryProfileError::MissingRecipient {
                profile: profile.as_str(),
                requirement: expected
                    .get(secrets.len())
                    .map_or("RECOVERY_PHRASE", |requirement| requirement.as_str()),
            },
        ));
    }
    for (index, requirement) in expected.iter().enumerate() {
        let declared = secrets.get(index).map(|(kind, _)| *kind);
        if declared != BackupRecipientKind::from_requirement(*requirement) {
            return Err(BackupKeyError::Profile(
                RecoveryProfileError::MissingRecipient {
                    profile: profile.as_str(),
                    requirement: requirement.as_str(),
                },
            ));
        }
    }

    let root = BackupMasterKey::generate()?;
    let mut set = BackupRecipientSet::new(set_id);
    for (index, (kind, secret)) in secrets.iter().enumerate() {
        let mut recipient_id = [0_u8; IDENTIFIER_BYTES];
        getrandom::fill(&mut recipient_id).map_err(|_| BackupKeyError::Randomness)?;
        // Keep the index visible in the identity so a report can name which
        // recipient failed without decoding the record.
        recipient_id[0] = u8::try_from(index).unwrap_or(u8::MAX);
        set.push(BackupRecipientRecord::wrap(
            &root,
            set_id,
            recipient_id,
            *kind,
            secret,
        )?)?;
    }
    Ok((root, set))
}

/// Why a backup key operation did not produce a root.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BackupKeyError {
    /// The presented secret did not open this recipient.
    #[error("the presented secret did not open this backup recipient")]
    WrongSecret,
    /// The record MAC did not verify under the recovered root.
    #[error(
        "the backup recipient record failed its integrity check under the \
         recovered key; this is an integrity incident and no plaintext was produced"
    )]
    RecordIntegrity,
    /// The record belongs to another backup set.
    #[error("the backup recipient record belongs to a different backup set")]
    SetMismatch,
    /// The selected recovery profile cannot hold a backup key.
    #[error(transparent)]
    Profile(#[from] RecoveryProfileError),
    /// The record is not the shape this build writes.
    #[error("the backup recipient record is not the expected shape")]
    Shape,
    /// The bytes are not the one canonical encoding.
    #[error("the backup recipient record is not canonically encoded")]
    NonCanonical,
    /// The bytes are not CBOR at all.
    #[error("the backup recipient record is malformed")]
    Malformed,
    /// The record declares an unsupported version.
    #[error("the backup recipient record declares an unsupported version")]
    UnsupportedVersion,
    /// The record declares an algorithm or profile this build does not accept.
    #[error("the backup recipient record declares an unsupported or unpinned algorithm")]
    UnsupportedAlgorithm,
    /// Encoding failed.
    #[error("the backup recipient record could not be encoded")]
    Encode,
    /// The AEAD refused to wrap.
    #[error("the backup root could not be wrapped")]
    Wrap,
    /// A key derivation failed.
    #[error("the backup key derivation failed")]
    Derivation,
    /// Operating-system randomness was unavailable.
    #[error("operating-system randomness was unavailable")]
    Randomness,
}

impl From<RandomnessUnavailable> for BackupKeyError {
    fn from(_: RandomnessUnavailable) -> Self {
        Self::Randomness
    }
}

impl BackupKeyError {
    /// Whether this outcome must be raised as an integrity incident.
    #[must_use]
    pub const fn is_integrity_incident(&self) -> bool {
        matches!(self, Self::RecordIntegrity)
    }
}

fn pinned_profile(
    identifier: &str,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<Argon2idProfile, BackupKeyError> {
    academic_crypto::PINNED_PROFILES
        .iter()
        .copied()
        .find(|pinned| {
            pinned.identifier == identifier
                && pinned.memory_kib == memory_kib
                && pinned.iterations == iterations
                && pinned.parallelism == parallelism
        })
        .ok_or(BackupKeyError::UnsupportedAlgorithm)
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

fn find(entries: &[(Value, Value)], key: u64) -> Result<&Value, BackupKeyError> {
    entries
        .iter()
        .find_map(|(candidate, value)| {
            let matches = candidate
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
                .is_some_and(|integer| integer == key);
            matches.then_some(value)
        })
        .ok_or(BackupKeyError::Shape)
}

fn expect_keys(entries: &[(Value, Value)], keys: &[u64]) -> Result<(), BackupKeyError> {
    if entries.len() != keys.len() {
        return Err(BackupKeyError::Shape);
    }
    for (entry, expected) in entries.iter().zip(keys) {
        let actual = entry
            .0
            .as_integer()
            .and_then(|integer| u64::try_from(integer).ok())
            .ok_or(BackupKeyError::Shape)?;
        if actual != *expected {
            return Err(BackupKeyError::Shape);
        }
    }
    Ok(())
}

fn text(entries: &[(Value, Value)], key: u64) -> Result<String, BackupKeyError> {
    find(entries, key)?
        .as_text()
        .map(str::to_owned)
        .ok_or(BackupKeyError::Shape)
}

fn bytes(entries: &[(Value, Value)], key: u64) -> Result<Vec<u8>, BackupKeyError> {
    find(entries, key)?
        .as_bytes()
        .cloned()
        .ok_or(BackupKeyError::Shape)
}

fn integer(entries: &[(Value, Value)], key: u64) -> Result<u32, BackupKeyError> {
    let raw = find(entries, key)?
        .as_integer()
        .ok_or(BackupKeyError::Shape)?;
    u128::try_from(raw)
        .ok()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(BackupKeyError::Shape)
}

fn fixed<const N: usize>(entries: &[(Value, Value)], key: u64) -> Result<[u8; N], BackupKeyError> {
    <[u8; N]>::try_from(bytes(entries, key)?.as_slice()).map_err(|_| BackupKeyError::Shape)
}

pub(crate) fn encode(value: &Value) -> Result<Vec<u8>, BackupKeyError> {
    let mut encoded = Vec::new();
    ciborium::into_writer(value, &mut encoded).map_err(|_| BackupKeyError::Encode)?;
    Ok(encoded)
}

pub(crate) fn decode_canonical(input: &[u8]) -> Result<Value, BackupKeyError> {
    let value: Value = ciborium::from_reader(input).map_err(|_| BackupKeyError::Malformed)?;
    if encode(&value)? != input {
        return Err(BackupKeyError::NonCanonical);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_recipient_kinds_are_never_the_device_keystore() {
        assert_eq!(
            BackupRecipientKind::from_requirement(RecipientRequirement::DeviceKeystore),
            None
        );
        for kind in [
            BackupRecipientKind::RecoveryPhrase,
            BackupRecipientKind::OfflineKeyFile,
        ] {
            assert!(kind.requirement().survives_device_loss());
        }
    }

    #[test]
    fn device_only_cannot_create_a_backup_key_set() -> Result<(), BackupKeyError> {
        let set_id = BackupSetId::from_bytes([0x11; IDENTIFIER_BYTES]);
        let error = create_backup_key_set(RecoveryProfile::DeviceOnly, set_id, &[])
            .err()
            .ok_or(BackupKeyError::Shape)?;
        assert_eq!(
            error,
            BackupKeyError::Profile(RecoveryProfileError::NoIndependentBackupRecipient {
                profile: "DEVICE_ONLY",
                statement: crate::profile::DEVICE_ONLY_IRRECOVERABILITY_STATEMENT,
            })
        );
        Ok(())
    }
}
