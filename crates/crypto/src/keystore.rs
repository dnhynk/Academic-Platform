//! The seam between the key hierarchy and the operating-system broker.
//!
//! The hierarchy depends on this trait, never on the native crate directly, so
//! the unlock policy -- fail closed, no weaker fallback, integrity incident on a
//! wrong key -- is testable without a host keystore and identical on every
//! platform. The reviewed FFI leaf is bound in only by the `os-keystore`
//! feature.

use core::fmt;

use zeroize::Zeroizing;

/// Why the broker could not serve a request.
///
/// Carries no key bytes and no native handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeystoreFailure {
    /// The broker is absent, not running, or refused to start.
    Unavailable,
    /// The broker holds nothing under this label.
    NotFound,
    /// The broker refused this caller, or would need a user prompt.
    AccessDenied,
    /// The stored blob is not one this broker wrote.
    InvalidBlob,
    /// This target has no reviewed broker.
    Unsupported,
}

impl fmt::Display for KeystoreFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let described = match self {
            Self::Unavailable => "the operating-system key broker is unavailable",
            Self::NotFound => "the operating-system key broker holds no key for this label",
            Self::AccessDenied => "the operating-system key broker refused this caller",
            Self::InvalidBlob => "the stored broker blob is not well-formed",
            Self::Unsupported => "this platform has no reviewed key broker",
        };
        formatter.write_str(described)
    }
}

impl std::error::Error for KeystoreFailure {}

/// A broker that can hold one secret per label on behalf of this user.
pub trait DeviceKeystore {
    /// Stable spelling of the broker, recorded in the recipient parameters.
    fn provider(&self) -> &str;

    /// Whether recipient creation requires durable staging before native seal.
    ///
    /// Existing implementations retain their historical one-step behavior.
    /// Add-only persistent brokers must opt in and implement the separate
    /// `RecoverableDeviceKeystore` seam; the one-step recipient helper then
    /// refuses before generating or storing any key.
    fn requires_publication_journal(&self) -> bool {
        false
    }

    /// Asks the broker to hold `secret`, returning the blob to persist.
    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure>;

    /// Recovers the secret the broker holds for `label`.
    fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure>;
}

/// A fresh exact seal identity that can be consumed only once.
///
/// Implementations must not create persistent state while preparing it, clone
/// the token, or reconstruct it from old blobs. The identity must bind the
/// requested provider and label, and remain usable for exact open/purge after
/// either a successful or an ambiguous failed seal attempt.
pub trait PreparedDeviceSeal {
    /// The blob to persist in the incomplete recipient before calling seal.
    fn blob(&self) -> &[u8];

    /// Attempts add once, without replacing any existing item.
    fn seal(self, secret: &[u8]) -> Result<(), KeystoreFailure>;
}

/// Optional extension for brokers with exact, add-only persistent identities.
pub trait RecoverableDeviceKeystore: DeviceKeystore {
    /// One fresh identity; persisted blobs cannot be converted back into it.
    type PreparedSeal: PreparedDeviceSeal;

    /// Prepares identity only, with no persistent side effect.
    fn prepare_seal(&self, label: &str) -> Result<Self::PreparedSeal, KeystoreFailure>;

    /// Purges only the exact generation identified by this label and blob.
    ///
    /// False means no matching generation remains. Failure is not absence;
    /// callers must retain the incomplete record and retry exact cleanup.
    fn purge_incomplete(&self, label: &str, blob: &[u8]) -> Result<bool, KeystoreFailure>;
}

#[cfg(feature = "os-keystore")]
mod platform {
    use academic_keystore_platform as native;
    use zeroize::Zeroizing;

    use super::{DeviceKeystore, KeystoreFailure, PreparedDeviceSeal, RecoverableDeviceKeystore};

    /// The reviewed native broker for this target.
    #[derive(Debug, Clone, Copy, Default)]
    #[non_exhaustive]
    pub struct PlatformKeystore;

    impl PlatformKeystore {
        /// Binds the compiled-in native broker.
        #[must_use]
        pub const fn new() -> Self {
            Self
        }
    }

    fn translate(error: &native::KeystoreError) -> KeystoreFailure {
        match error.code {
            native::KeystoreErrorCode::NotFound => KeystoreFailure::NotFound,
            native::KeystoreErrorCode::AccessDenied => KeystoreFailure::AccessDenied,
            native::KeystoreErrorCode::InvalidSealedBlob
            | native::KeystoreErrorCode::ProviderMismatch
            | native::KeystoreErrorCode::InvalidLabel
            | native::KeystoreErrorCode::SecretTooLarge => KeystoreFailure::InvalidBlob,
            native::KeystoreErrorCode::Unsupported => KeystoreFailure::Unsupported,
            // An operating-system failure is not evidence that the key is
            // absent, so it must read as unavailable and keep the profile
            // locked rather than inviting a re-seal that would orphan the key.
            native::KeystoreErrorCode::Unavailable | native::KeystoreErrorCode::OperatingSystem => {
                KeystoreFailure::Unavailable
            }
            _ => KeystoreFailure::Unavailable,
        }
    }

    fn label_of(label: &str) -> Result<native::KeystoreLabel, KeystoreFailure> {
        native::KeystoreLabel::new(label).map_err(|error| translate(&error))
    }

    impl DeviceKeystore for PlatformKeystore {
        fn provider(&self) -> &str {
            native::PROVIDER.as_str()
        }

        fn requires_publication_journal(&self) -> bool {
            matches!(
                native::PROVIDER,
                native::KeystoreProvider::MacosKeychainDataProtectionV1
            )
        }

        fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
            native::seal(&label_of(label)?, secret).map_err(|error| translate(&error))
        }

        fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure> {
            let recovered =
                native::open(&label_of(label)?, blob).map_err(|error| translate(&error))?;
            Ok(Zeroizing::new(recovered.expose().to_vec()))
        }
    }

    /// Redacted, single-use wrapper around the native prepared identity.
    #[derive(Debug)]
    pub struct PlatformPreparedSeal(native::PreparedSeal);

    impl PreparedDeviceSeal for PlatformPreparedSeal {
        fn blob(&self) -> &[u8] {
            self.0.blob()
        }

        fn seal(self, secret: &[u8]) -> Result<(), KeystoreFailure> {
            self.0.seal(secret).map_err(|error| translate(&error))
        }
    }

    impl RecoverableDeviceKeystore for PlatformKeystore {
        type PreparedSeal = PlatformPreparedSeal;

        fn prepare_seal(&self, label: &str) -> Result<Self::PreparedSeal, KeystoreFailure> {
            native::prepare_seal(&label_of(label)?)
                .map(PlatformPreparedSeal)
                .map_err(|error| translate(&error))
        }

        fn purge_incomplete(&self, label: &str, blob: &[u8]) -> Result<bool, KeystoreFailure> {
            purge(label, blob)
        }
    }

    /// Removes the stored key for `label`, when the broker stores one.
    ///
    /// Exposed for tests, explicit incomplete-publication cleanup and `P2-K5`'s
    /// revocation work. No automatic or label-only deletion is performed.
    pub fn purge(label: &str, blob: &[u8]) -> Result<bool, KeystoreFailure> {
        let outcome = native::purge(&label_of(label)?, blob).map_err(|error| translate(&error))?;
        Ok(matches!(outcome, native::PurgeOutcome::Removed))
    }
}

#[cfg(feature = "os-keystore")]
pub use platform::{PlatformKeystore, PlatformPreparedSeal, purge};
