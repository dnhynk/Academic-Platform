//! The ADR-005 key hierarchy and its unlock policy.
//!
//! ```text
//! Recovery recipient ──┐
//! Device recipient ────┼──> wrap(VMK)   VMK: 32 random bytes, never persisted unwrapped
//!                      │
//!                      ├──> KEK_d   = HKDF-SHA-512(VMK, salt=profile_id, info="academic-os/kek/v1"||domain_id)
//!                      ├──> SKEY_p  = HKDF-SHA-512(VMK, salt=profile_id, info="academic-os/store/v1")
//!                      └──> AUDKEY  = HKDF-SHA-512(VMK, salt=profile_id, info="academic-os/audit/v1")
//! ```
//!
//! Both recipient kinds have the same structure: something produces a 32-byte
//! wrapping key, and the VMK is sealed under it with XChaCha20-Poly1305. For a
//! device recipient the operating-system broker holds that key; for a recovery
//! recipient a pinned Argon2id profile derives it from a 256-bit secret. Only
//! ciphertext reaches `keys/recipients.cbor`.
//!
//! # What this crate does not do
//!
//! It writes no file, opens no database, and selects no recovery profile. It
//! hands a caller a `RecipientSet` to persist and a `VaultMasterKey` to derive
//! from. Choosing which recipients a profile has is `P2-K4`'s decision and
//! remains an open user choice.

pub mod derive;
mod fault;
pub mod keys;
pub mod keystore;
pub mod recipient;
pub mod recovery;

use zeroize::Zeroizing;

pub use derive::{
    AUDIT_INFO, KEK_INFO_PREFIX, KEY_GENERATION_INFO, KeyScheduleError, RECIPIENT_MAC_INFO,
    REHEARSAL_INFO, STORE_INFO, VAULT_LOCATOR_INFO,
};
pub use fault::{FAULT_ACTION_VARIABLE, FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE};
pub use keys::{
    ArtifactDek, AuditKey, DeviceWrappingKey, DomainId, DomainKek, DomainLocatorKey,
    IDENTIFIER_BYTES, KEY_BYTES, ProfileId, RandomnessUnavailable, RecipientMacKey,
    RecipientWrapKey, RecoverySecret, RehearsalKey, StoreKey, VaultMasterKey,
};
pub use keystore::{DeviceKeystore, KeystoreFailure};
#[cfg(feature = "os-keystore")]
pub use keystore::{PlatformKeystore, purge as purge_device_key};
pub use recipient::{
    DEVICE_KDF_ALGORITHM_ID, RECORD_MAC_BYTES, RECORD_VERSION, RECOVERY_KDF_ALGORITHM_ID,
    RecipientKind, RecipientParameters, RecipientRecord, RecipientSet, RecordError, SET_VERSION,
    WRAP_ALGORITHM_ID, WRAP_NONCE_BYTES, WRAPPED_VMK_BYTES,
};
pub use recovery::{
    Argon2idProfile, PINNED_PROFILES, RECOVERY_ARGON2ID_V1, RecoveryError, UnlockThrottle,
};

use fault::FaultPoint;
use recipient::RecipientError;

/// Test-only feature name that may activate the `KY` failpoints.
pub const FAULT_INJECTION_FEATURE: &str = "phase2-fault-injection";

/// The `KY` rows of the Phase 2 fault matrix owned by `P2-K1`.
///
/// `KY03`-`KY05` are rotation and revocation faults and land with `P2-K5`.
pub const PHASE2_KEY_FAULT_IDS: &[&str] = &["KY01", "KY02", "KY06", "KY07", "KY08"];

/// Why an unlock did not produce a Vault Master Key.
///
/// No variant carries key bytes. The variants distinguish the three outcomes
/// the fault matrix requires: an unavailable broker (`KY01`), a wrong secret
/// (`KY06`), and an integrity incident (`KY07`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UnlockError {
    /// The operating-system broker could not be reached. The profile stays
    /// locked; there is no weaker key to fall back to.
    #[error(
        "the {provider} key broker is unavailable, so the profile stays locked; \
         no weaker key is used. Restore access to the broker for label {label}, \
         or unlock with a recovery recipient"
    )]
    KeystoreUnavailable {
        /// Stable broker spelling, so the message names what to fix.
        provider: String,
        /// Label the device key is stored under.
        label: String,
    },
    /// The broker holds no key for this recipient.
    #[error(
        "the {provider} key broker holds no key for label {label}; the profile \
         stays locked. Unlock with a recovery recipient"
    )]
    KeystoreKeyMissing {
        /// Stable broker spelling.
        provider: String,
        /// Label the device key was expected under.
        label: String,
    },
    /// The broker refused this caller.
    #[error("the {provider} key broker refused this caller; the profile stays locked")]
    KeystoreAccessDenied {
        /// Stable broker spelling.
        provider: String,
    },
    /// The broker in hand is not the one this record was sealed under.
    ///
    /// A record moved between platforms must say so, rather than asking the
    /// wrong broker and reporting a misleading outage.
    #[error(
        "this recipient record was sealed by the {expected} key broker but this          build carries {actual}; the profile stays locked"
    )]
    KeystoreProviderMismatch {
        /// Broker named by the record.
        expected: String,
        /// Broker compiled into this build.
        actual: String,
    },
    /// The broker returned a key that does not open this recipient.
    ///
    /// This is an integrity incident: a correctly functioning broker paired
    /// with an unmodified record cannot produce it.
    #[error(
        "the {provider} key broker returned a key that does not open this \
         recipient record; this is an integrity incident and no plaintext was produced"
    )]
    KeystoreKeyRejected {
        /// Stable broker spelling.
        provider: String,
    },
    /// The presented recovery secret did not open this recipient.
    ///
    /// One variant for every wrong-secret cause, so nothing can be learned
    /// about the secret from the outcome.
    #[error("the presented recovery secret did not open this recipient")]
    WrongRecoverySecret,
    /// The record MAC did not verify under the recovered VMK.
    #[error(
        "the recipient record failed its integrity check under the recovered \
         key; this is an integrity incident and no plaintext was produced"
    )]
    RecordIntegrity,
    /// Another attempt is not yet allowed.
    #[error("another recovery attempt is not allowed for {retry_after_ms} ms")]
    RateLimited {
        /// Milliseconds the caller must wait.
        retry_after_ms: u64,
    },
    /// The record was asked to open with the wrong kind of key.
    #[error("this recipient is not opened by the presented kind of key")]
    RecipientKindMismatch,
    /// The record is unusable.
    #[error("the recipient record is unusable: {0}")]
    Record(#[from] RecordError),
    /// The record belongs to another profile.
    #[error("the recipient record belongs to a different profile")]
    ProfileMismatch,
    /// The key schedule failed.
    #[error("the key schedule failed")]
    KeySchedule,
    /// Operating-system randomness was unavailable.
    #[error("operating-system randomness was unavailable")]
    Randomness,
    /// The recovery key derivation failed.
    #[error("the recovery key derivation failed: {0}")]
    Recovery(#[from] RecoveryError),
}

impl UnlockError {
    /// Whether this outcome must be raised as an integrity incident.
    ///
    /// A wrong recovery secret is an ordinary refusal; a broker returning a key
    /// that does not open the record, or a record whose MAC does not verify, is
    /// not, and `KY07` requires the difference to be visible.
    #[must_use]
    pub const fn is_integrity_incident(&self) -> bool {
        matches!(
            self,
            Self::KeystoreKeyRejected { .. } | Self::RecordIntegrity
        )
    }

    /// Whether the profile is left locked. Every variant leaves it locked;
    /// the method exists so a caller can assert that rather than assume it.
    #[must_use]
    pub const fn leaves_profile_locked(&self) -> bool {
        true
    }
}

impl From<RandomnessUnavailable> for UnlockError {
    fn from(_: RandomnessUnavailable) -> Self {
        Self::Randomness
    }
}

impl From<KeyScheduleError> for UnlockError {
    fn from(_: KeyScheduleError) -> Self {
        Self::KeySchedule
    }
}

fn from_recipient_error(error: RecipientError, wrong_key: UnlockError) -> UnlockError {
    match error {
        RecipientError::WrongKey => wrong_key,
        RecipientError::RecordIntegrity => UnlockError::RecordIntegrity,
        RecipientError::ProfileMismatch => UnlockError::ProfileMismatch,
        RecipientError::KeySchedule => UnlockError::KeySchedule,
        RecipientError::Randomness => UnlockError::Randomness,
        RecipientError::Record(inner) => UnlockError::Record(inner),
    }
}

fn keystore_error(failure: KeystoreFailure, provider: &str, label: &str) -> UnlockError {
    match failure {
        KeystoreFailure::NotFound => UnlockError::KeystoreKeyMissing {
            provider: provider.to_owned(),
            label: label.to_owned(),
        },
        KeystoreFailure::AccessDenied => UnlockError::KeystoreAccessDenied {
            provider: provider.to_owned(),
        },
        KeystoreFailure::Unavailable
        | KeystoreFailure::Unsupported
        | KeystoreFailure::InvalidBlob => UnlockError::KeystoreUnavailable {
            provider: provider.to_owned(),
            label: label.to_owned(),
        },
    }
}

fn wrap_key_from(bytes: &[u8]) -> Result<RecipientWrapKey, UnlockError> {
    let sized =
        <[u8; KEY_BYTES]>::try_from(bytes).map_err(|_| UnlockError::Record(RecordError::Shape))?;
    Ok(RecipientWrapKey::from_zeroizing(Zeroizing::new(sized)))
}

/// Generates a Vault Master Key and its device recipient in one step.
///
/// Nothing is written: the caller receives the record to persist. The `KY08`
/// failpoint stands between generating key material and returning it, so a
/// harness can prove a termination there leaves no key material behind.
pub fn create_device_recipient<K: DeviceKeystore + ?Sized>(
    master: &VaultMasterKey,
    profile: ProfileId,
    recipient_id: [u8; IDENTIFIER_BYTES],
    label: &str,
    keystore: &K,
) -> Result<RecipientRecord, UnlockError> {
    let device_key = DeviceWrappingKey::generate()?;
    let blob = keystore
        .seal(label, device_key.expose_secret())
        .map_err(|failure| keystore_error(failure, keystore.provider(), label))?;

    fault::trip(FaultPoint::Ky08);

    let wrap_key = wrap_key_from(device_key.expose_secret())?;
    recipient::wrap(
        master,
        profile,
        recipient_id,
        RecipientParameters::DeviceKeystore {
            provider: keystore.provider().to_owned(),
            label: label.to_owned(),
        },
        blob,
        &wrap_key,
    )
    .map_err(|error| {
        from_recipient_error(
            error,
            UnlockError::KeystoreKeyRejected {
                provider: keystore.provider().to_owned(),
            },
        )
    })
}

/// Wraps an existing Vault Master Key for a recovery recipient.
pub fn create_recovery_recipient(
    master: &VaultMasterKey,
    profile: ProfileId,
    recipient_id: [u8; IDENTIFIER_BYTES],
    secret: &RecoverySecret,
    argon2_profile: Argon2idProfile,
) -> Result<RecipientRecord, UnlockError> {
    let mut salt = [0_u8; IDENTIFIER_BYTES];
    getrandom::fill(&mut salt).map_err(|_| UnlockError::Randomness)?;
    let wrap_key = argon2_profile.derive_wrap_key(secret, &salt)?;
    recipient::wrap(
        master,
        profile,
        recipient_id,
        RecipientParameters::RecoverySecret {
            profile: argon2_profile,
            salt,
        },
        Vec::new(),
        &wrap_key,
    )
    .map_err(|error| from_recipient_error(error, UnlockError::WrongRecoverySecret))
}

/// Unlocks the Vault Master Key through the operating-system broker.
///
/// Fails closed on every broker failure: there is no second key source, no
/// cached copy, and no downgrade. The `KY02` failpoint stands between the
/// broker returning the wrapping key and the VMK being produced.
pub fn unlock_with_device<K: DeviceKeystore + ?Sized>(
    record: &RecipientRecord,
    profile: ProfileId,
    keystore: &K,
) -> Result<VaultMasterKey, UnlockError> {
    let RecipientParameters::DeviceKeystore { provider, label } = record.parameters() else {
        return Err(UnlockError::RecipientKindMismatch);
    };
    if provider != keystore.provider() {
        return Err(UnlockError::KeystoreProviderMismatch {
            expected: provider.clone(),
            actual: keystore.provider().to_owned(),
        });
    }
    let recovered = keystore
        .open(label, record.keystore_blob())
        .map_err(|failure| keystore_error(failure, provider, label))?;

    fault::trip(FaultPoint::Ky02);

    let wrap_key = wrap_key_from(&recovered)?;
    recipient::unwrap(record, profile, &wrap_key).map_err(|error| {
        from_recipient_error(
            error,
            UnlockError::KeystoreKeyRejected {
                provider: provider.clone(),
            },
        )
    })
}

/// Unlocks the Vault Master Key from a 256-bit recovery secret.
///
/// The throttle is advanced by this call: a refused attempt extends the wait
/// and a successful one clears it. Every wrong secret produces the same error,
/// and no API here accepts or reports an individual word of a phrase.
pub fn unlock_with_recovery(
    record: &RecipientRecord,
    profile: ProfileId,
    secret: &RecoverySecret,
    throttle: &mut UnlockThrottle,
    now_ms: u64,
) -> Result<VaultMasterKey, UnlockError> {
    if let Some(retry_after_ms) = throttle.wait_remaining_ms(now_ms) {
        return Err(UnlockError::RateLimited { retry_after_ms });
    }
    let RecipientParameters::RecoverySecret {
        profile: argon2_profile,
        salt,
    } = record.parameters()
    else {
        return Err(UnlockError::RecipientKindMismatch);
    };
    let wrap_key = argon2_profile.derive_wrap_key(secret, salt)?;
    match recipient::unwrap(record, profile, &wrap_key) {
        Ok(master) => {
            throttle.record_success();
            Ok(master)
        }
        Err(error) => {
            let mapped = from_recipient_error(error, UnlockError::WrongRecoverySecret);
            // A record-integrity failure is not an attempt against the secret,
            // so it does not feed the throttle.
            if matches!(mapped, UnlockError::WrongRecoverySecret) {
                throttle.record_failure(now_ms);
            }
            Err(mapped)
        }
    }
}
