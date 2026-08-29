//! The restore rehearsal receipt and the ingest gate it feeds.
//!
//! t068 section 6 makes `GATE-P2-RECOVERY` block **the first real ingest of any
//! kind**: a completed independent restore into a fresh empty profile, using
//! the user's chosen recovery profile, must have happened, and its receipt must
//! be newer than the last change to key material.
//!
//! "Newer" is not a clock comparison. A wall clock can move backwards, and two
//! key changes inside one millisecond would be indistinguishable. The receipt
//! therefore records both a monotonic **generation** and a **digest** of the
//! key material it was taken against, and the gate requires both to equal what
//! the profile holds now. A rotation, a recipient added, or a recipient revoked
//! changes the digest, so the rehearsal stops matching and ingest is refused
//! until a new drill is run.
//!
//! The receipt is authenticated under `HKDF(VMK, info="academic-os/rehearsal/v1")`,
//! not under the backup key: the gate runs at ingest time on an unlocked
//! profile, where the VMK is in hand and the recovery phrase is not.

use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use academic_crypto::{IDENTIFIER_BYTES, ProfileId, RecipientSet, VaultMasterKey};
use ciborium::value::{Integer, Value};
use hmac::{Hmac, Mac as _};
use sha2::{Digest as _, Sha256, Sha512};
use subtle::ConstantTimeEq as _;

use crate::{
    backup_key::{BackupKeyError, BackupSetId, decode_canonical, encode},
    profile::RecoveryProfile,
};

/// Where the receipt lives inside a profile, relative to its root.
pub const REHEARSAL_RECEIPT_RELATIVE_PATH: &str = "admission/rehearsal.cbor";
/// The `admission` directory a profile keeps its receipts in.
pub const ADMISSION_DIRECTORY: &str = "admission";
/// Version of the receipt this build writes and accepts.
pub const RECEIPT_VERSION: u8 = 1;
/// Domain separator for the key-material digest.
pub const KEY_MATERIAL_DIGEST_DOMAIN: &[u8] = b"academic-os/key-material-state/v1";
/// Bytes of the receipt MAC.
pub const RECEIPT_MAC_BYTES: usize = 64;

/// Exactly which key material a profile holds right now.
///
/// `generation` is advanced by every key-material change — recipient added,
/// recipient revoked, rotation completed — and `digest` covers the recipient
/// set's canonical bytes. Two states are the same only if both agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyMaterialState {
    generation: u64,
    digest: [u8; 32],
    changed_at_unix_ms: i64,
}

impl KeyMaterialState {
    /// Builds a state from a recipient set at a stated generation.
    pub fn from_recipient_set(
        set: &RecipientSet,
        generation: u64,
        changed_at_unix_ms: i64,
    ) -> Result<Self, RehearsalError> {
        let bytes = set
            .to_canonical_cbor()
            .map_err(|_| RehearsalError::KeyMaterialUnreadable)?;
        let mut hasher = Sha256::new();
        hasher.update(KEY_MATERIAL_DIGEST_DOMAIN);
        hasher.update(generation.to_be_bytes());
        hasher.update(&bytes);
        Ok(Self {
            generation,
            digest: hasher.finalize().into(),
            changed_at_unix_ms,
        })
    }

    /// Builds a state from an exact digest, for a caller that already has one.
    #[must_use]
    pub const fn from_parts(generation: u64, digest: [u8; 32], changed_at_unix_ms: i64) -> Self {
        Self {
            generation,
            digest,
            changed_at_unix_ms,
        }
    }

    /// Returns the monotonic key-material generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the digest over the key material.
    #[must_use]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Returns when the key material last changed.
    #[must_use]
    pub const fn changed_at_unix_ms(&self) -> i64 {
        self.changed_at_unix_ms
    }
}

/// What a caller actually observed while carrying out one drill.
///
/// These are observations, not intentions: a caller that did not restore the
/// backup cannot fill in a canonical semantic digest that the gate will accept,
/// because the gate compares it against the profile the restore produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RehearsalObservations {
    /// The profile the drill was run for.
    pub profile_id: ProfileId,
    /// The recovery profile the drill exercised.
    pub recovery_profile: RecoveryProfile,
    /// The backup the drill restored from.
    pub backup_set_id: BackupSetId,
    /// The canonical semantic digest the restored profile reproduced.
    pub restored_canonical_semantic_digest: [u8; 32],
    /// How many sealed objects the restore closed over.
    pub restored_object_count: u64,
    /// When the drill completed.
    pub completed_at_unix_ms: i64,
}

/// A completed restore rehearsal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehearsalReceipt {
    profile_id: ProfileId,
    recovery_profile: RecoveryProfile,
    backup_set_id: BackupSetId,
    restored_canonical_semantic_digest: [u8; 32],
    restored_object_count: u64,
    key_material_generation: u64,
    key_material_digest: [u8; 32],
    completed_at_unix_ms: i64,
    receipt_mac: Vec<u8>,
}

impl RehearsalReceipt {
    /// Records a rehearsal that actually completed.
    ///
    /// Every field is an observation the caller made during the drill; nothing
    /// here can invent one. The digest is the canonical semantic digest of the
    /// *restored* profile, so a receipt cannot be written from a backup that
    /// was never restored.
    pub fn record(
        key: &VaultMasterKey,
        observed: &RehearsalObservations,
        key_material: &KeyMaterialState,
    ) -> Result<Self, RehearsalError> {
        let mut receipt = Self {
            profile_id: observed.profile_id,
            recovery_profile: observed.recovery_profile,
            backup_set_id: observed.backup_set_id,
            restored_canonical_semantic_digest: observed.restored_canonical_semantic_digest,
            restored_object_count: observed.restored_object_count,
            key_material_generation: key_material.generation,
            key_material_digest: key_material.digest,
            completed_at_unix_ms: observed.completed_at_unix_ms,
            receipt_mac: Vec::new(),
        };
        receipt.receipt_mac = receipt.compute_mac(key)?;
        Ok(receipt)
    }

    /// Returns the profile this rehearsal was run for.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    /// Returns the recovery profile the drill exercised.
    #[must_use]
    pub const fn recovery_profile(&self) -> RecoveryProfile {
        self.recovery_profile
    }

    /// Returns the backup the drill restored from.
    #[must_use]
    pub const fn backup_set_id(&self) -> BackupSetId {
        self.backup_set_id
    }

    /// Returns the canonical semantic digest the restore reproduced.
    #[must_use]
    pub const fn restored_canonical_semantic_digest(&self) -> &[u8; 32] {
        &self.restored_canonical_semantic_digest
    }

    /// Returns how many sealed objects the restore closed over.
    #[must_use]
    pub const fn restored_object_count(&self) -> u64 {
        self.restored_object_count
    }

    /// Returns the key-material generation the drill was run against.
    #[must_use]
    pub const fn key_material_generation(&self) -> u64 {
        self.key_material_generation
    }

    /// Returns when the drill completed.
    #[must_use]
    pub const fn completed_at_unix_ms(&self) -> i64 {
        self.completed_at_unix_ms
    }

    /// Verifies the receipt MAC under the profile's own key.
    pub fn verify(&self, key: &VaultMasterKey) -> Result<(), RehearsalError> {
        let expected = self.compute_mac(key)?;
        if expected.ct_eq(self.receipt_mac.as_slice()).into() {
            Ok(())
        } else {
            Err(RehearsalError::ReceiptIntegrity)
        }
    }

    fn compute_mac(&self, key: &VaultMasterKey) -> Result<Vec<u8>, RehearsalError> {
        let mac_key = key
            .derive_rehearsal_key(self.profile_id)
            .map_err(|_| RehearsalError::KeySchedule)?;
        let mut mac = Hmac::<Sha512>::new_from_slice(mac_key.expose_secret())
            .map_err(|_| RehearsalError::KeySchedule)?;
        mac.update(&self.mac_covered_bytes()?);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    fn mac_covered_value(&self) -> Value {
        Value::Map(vec![
            (key(0), unsigned(u64::from(RECEIPT_VERSION))),
            (key(1), Value::Bytes(self.profile_id.as_bytes().to_vec())),
            (
                key(2),
                Value::Text(self.recovery_profile.as_str().to_owned()),
            ),
            (key(3), Value::Bytes(self.backup_set_id.as_bytes().to_vec())),
            (
                key(4),
                Value::Bytes(self.restored_canonical_semantic_digest.to_vec()),
            ),
            (key(5), unsigned(self.restored_object_count)),
            (key(6), unsigned(self.key_material_generation)),
            (key(7), Value::Bytes(self.key_material_digest.to_vec())),
            (
                key(8),
                Value::Integer(Integer::from(self.completed_at_unix_ms)),
            ),
        ])
    }

    fn mac_covered_bytes(&self) -> Result<Vec<u8>, RehearsalError> {
        encode(&self.mac_covered_value()).map_err(RehearsalError::Encoding)
    }

    /// Encodes the receipt as canonical deterministic CBOR.
    pub fn to_canonical_cbor(&self) -> Result<Vec<u8>, RehearsalError> {
        let Value::Map(mut entries) = self.mac_covered_value() else {
            return Err(RehearsalError::Shape);
        };
        entries.push((key(9), Value::Bytes(self.receipt_mac.clone())));
        encode(&Value::Map(entries)).map_err(RehearsalError::Encoding)
    }

    /// Decodes and canonicality-checks a receipt.
    pub fn from_canonical_cbor(input: &[u8]) -> Result<Self, RehearsalError> {
        let value = decode_canonical(input).map_err(RehearsalError::Encoding)?;
        let entries = value.as_map().ok_or(RehearsalError::Shape)?;
        if entries.len() != 10 {
            return Err(RehearsalError::Shape);
        }
        for (index, entry) in entries.iter().enumerate() {
            let declared = entry
                .0
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
                .ok_or(RehearsalError::Shape)?;
            if usize::try_from(declared).unwrap_or(usize::MAX) != index {
                return Err(RehearsalError::Shape);
            }
        }
        if read_unsigned(&entries[0].1)? != u64::from(RECEIPT_VERSION) {
            return Err(RehearsalError::UnsupportedVersion);
        }
        let recovery_profile = entries[2]
            .1
            .as_text()
            .and_then(RecoveryProfile::parse)
            .ok_or(RehearsalError::Shape)?;
        let receipt_mac = entries[9]
            .1
            .as_bytes()
            .cloned()
            .ok_or(RehearsalError::Shape)?;
        if receipt_mac.len() != RECEIPT_MAC_BYTES {
            return Err(RehearsalError::Shape);
        }
        Ok(Self {
            profile_id: ProfileId::from_bytes(read_fixed::<IDENTIFIER_BYTES>(&entries[1].1)?),
            recovery_profile,
            backup_set_id: BackupSetId::from_bytes(read_fixed::<IDENTIFIER_BYTES>(&entries[3].1)?),
            restored_canonical_semantic_digest: read_fixed::<32>(&entries[4].1)?,
            restored_object_count: read_unsigned(&entries[5].1)?,
            key_material_generation: read_unsigned(&entries[6].1)?,
            key_material_digest: read_fixed::<32>(&entries[7].1)?,
            completed_at_unix_ms: entries[8]
                .1
                .as_integer()
                .and_then(|integer| i64::try_from(integer).ok())
                .ok_or(RehearsalError::Shape)?,
            receipt_mac,
        })
    }

    /// Returns the receipt path inside a profile root.
    #[must_use]
    pub fn path_in(profile_root: &Path) -> PathBuf {
        profile_root
            .join(ADMISSION_DIRECTORY)
            .join("rehearsal.cbor")
    }

    /// Writes the receipt into a profile, creating `admission/` if needed.
    ///
    /// The write is atomic in the only sense that matters here: the bytes land
    /// in a temporary file beside the target, are synchronized, and are then
    /// renamed over it, so a termination never leaves a truncated receipt that
    /// would fail its MAC and read as tampering.
    pub fn write_into_profile(&self, profile_root: &Path) -> Result<PathBuf, RehearsalError> {
        let directory = profile_root.join(ADMISSION_DIRECTORY);
        fs::create_dir_all(&directory).map_err(|source| {
            RehearsalError::io("create admission directory", &directory, source)
        })?;
        let target = Self::path_in(profile_root);
        let temporary = directory.join("rehearsal.cbor.partial");
        let bytes = self.to_canonical_cbor()?;
        // The handle that wrote the bytes is the handle that flushes them:
        // Windows refuses `FlushFileBuffers` on a read-only handle, so
        // reopening the file to synchronize it would fail with access denied.
        let mut file = fs::File::create(&temporary)
            .map_err(|source| RehearsalError::io("create rehearsal receipt", &temporary, source))?;
        file.write_all(&bytes)
            .map_err(|source| RehearsalError::io("write rehearsal receipt", &temporary, source))?;
        file.sync_all()
            .map_err(|source| RehearsalError::io("sync rehearsal receipt", &temporary, source))?;
        drop(file);
        fs::rename(&temporary, &target)
            .map_err(|source| RehearsalError::io("publish rehearsal receipt", &target, source))?;
        Ok(target)
    }

    /// Reads the receipt from a profile, or reports that there is none.
    pub fn read_from_profile(profile_root: &Path) -> Result<Option<Self>, RehearsalError> {
        let path = Self::path_in(profile_root);
        match fs::read(&path) {
            Ok(bytes) => Self::from_canonical_cbor(&bytes).map(Some),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(RehearsalError::io("read rehearsal receipt", &path, source)),
        }
    }
}

/// Why the first ingest of real data is refused.
///
/// The vocabulary is closed so a surface can render an exact reason rather than
/// a message, and so a caller cannot invent a fourth outcome that admits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IngestRefusal {
    /// No rehearsal receipt exists in this profile.
    #[error(
        "no restore rehearsal receipt exists at {path}; run a restore rehearsal \
         into a fresh empty profile before the first ingest"
    )]
    RehearsalAbsent {
        /// Where the receipt was looked for.
        path: String,
    },
    /// The receipt belongs to a different profile.
    #[error("the restore rehearsal receipt belongs to a different profile")]
    ProfileMismatch,
    /// The receipt did not authenticate under this profile's key.
    #[error(
        "the restore rehearsal receipt failed its integrity check; this is an \
         integrity incident and the receipt admits nothing"
    )]
    ReceiptUnverified,
    /// The key material changed after the rehearsal was run.
    #[error(
        "the restore rehearsal was run against key-material generation \
         {receipt_generation} and this profile now holds generation \
         {current_generation}; run the rehearsal again before the first ingest"
    )]
    StaleKeyMaterial {
        /// Generation named by the receipt.
        receipt_generation: u64,
        /// Generation the profile holds now.
        current_generation: u64,
    },
    /// The generation matches but the key material itself does not.
    #[error(
        "the restore rehearsal receipt names key material that is not the \
         material this profile holds; run the rehearsal again before the first ingest"
    )]
    KeyMaterialMismatch,
    /// The rehearsal exercised a recovery profile the caller did not select.
    #[error(
        "the restore rehearsal exercised recovery profile {rehearsed} but this \
         profile has selected {selected}; drill the profile that is in force"
    )]
    RecoveryProfileMismatch {
        /// What the drill exercised.
        rehearsed: &'static str,
        /// What the profile has selected.
        selected: &'static str,
    },
}

/// Decides whether the first real ingest may proceed.
///
/// Every path through this function either returns `Ok(())` or names one closed
/// refusal reason. There is no flag, environment variable, or debug shortcut
/// that reaches the admitted branch without a verified receipt whose key
/// material equals the profile's own.
pub fn admit_first_ingest(
    profile_root: &Path,
    key: &VaultMasterKey,
    profile_id: ProfileId,
    selected: RecoveryProfile,
    current: &KeyMaterialState,
) -> Result<RehearsalReceipt, IngestRefusal> {
    let receipt = RehearsalReceipt::read_from_profile(profile_root)
        .map_err(|_| IngestRefusal::ReceiptUnverified)?
        .ok_or_else(|| IngestRefusal::RehearsalAbsent {
            path: RehearsalReceipt::path_in(profile_root)
                .display()
                .to_string(),
        })?;
    if receipt.profile_id.as_bytes() != profile_id.as_bytes() {
        return Err(IngestRefusal::ProfileMismatch);
    }
    receipt
        .verify(key)
        .map_err(|_| IngestRefusal::ReceiptUnverified)?;
    if receipt.recovery_profile != selected {
        return Err(IngestRefusal::RecoveryProfileMismatch {
            rehearsed: receipt.recovery_profile.as_str(),
            selected: selected.as_str(),
        });
    }
    if receipt.key_material_generation != current.generation {
        return Err(IngestRefusal::StaleKeyMaterial {
            receipt_generation: receipt.key_material_generation,
            current_generation: current.generation,
        });
    }
    if receipt.key_material_digest != current.digest {
        return Err(IngestRefusal::KeyMaterialMismatch);
    }
    Ok(receipt)
}

/// Why a receipt could not be produced, read, or verified.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RehearsalError {
    /// A filesystem operation failed.
    #[error("{operation} failed for {path}: {source}")]
    Io {
        /// What was attempted.
        operation: &'static str,
        /// The path involved.
        path: String,
        /// The underlying error.
        source: io::Error,
    },
    /// The receipt is not the shape this build writes.
    #[error("the rehearsal receipt is not the expected shape")]
    Shape,
    /// The receipt declares an unsupported version.
    #[error("the rehearsal receipt declares an unsupported version")]
    UnsupportedVersion,
    /// The receipt MAC did not verify.
    #[error("the rehearsal receipt failed its integrity check")]
    ReceiptIntegrity,
    /// The recipient set could not be read.
    #[error("the profile's key material could not be read")]
    KeyMaterialUnreadable,
    /// The key schedule failed.
    #[error("the rehearsal key schedule failed")]
    KeySchedule,
    /// Encoding failed.
    #[error(transparent)]
    Encoding(BackupKeyError),
}

impl RehearsalError {
    fn io(operation: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.display().to_string(),
            source,
        }
    }
}

fn key(index: u64) -> Value {
    Value::Integer(Integer::from(index))
}

fn unsigned(value: u64) -> Value {
    Value::Integer(Integer::from(value))
}

fn read_unsigned(value: &Value) -> Result<u64, RehearsalError> {
    value
        .as_integer()
        .and_then(|integer| u128::try_from(integer).ok())
        .and_then(|raw| u64::try_from(raw).ok())
        .ok_or(RehearsalError::Shape)
}

fn read_fixed<const N: usize>(value: &Value) -> Result<[u8; N], RehearsalError> {
    let bytes = value.as_bytes().ok_or(RehearsalError::Shape)?;
    <[u8; N]>::try_from(bytes.as_slice()).map_err(|_| RehearsalError::Shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_receipt_path_is_the_one_the_plan_fixes() {
        let path = RehearsalReceipt::path_in(Path::new("profile"));
        let rendered = path.to_string_lossy().replace('\\', "/");
        assert!(
            rendered.ends_with(REHEARSAL_RECEIPT_RELATIVE_PATH),
            "receipt landed at {rendered}"
        );
    }
}
