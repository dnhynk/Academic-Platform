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

    /// Asks the broker to hold `secret`, returning the blob to persist.
    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure>;

    /// Recovers the secret the broker holds for `label`.
    fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure>;
}

#[cfg(feature = "os-keystore")]
mod platform {
    use academic_keystore_platform as native;
    use zeroize::Zeroizing;

    use super::{DeviceKeystore, KeystoreFailure};

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

        fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
            native::seal(&label_of(label)?, secret).map_err(|error| translate(&error))
        }

        fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure> {
            let recovered =
                native::open(&label_of(label)?, blob).map_err(|error| translate(&error))?;
            Ok(Zeroizing::new(recovered.expose().to_vec()))
        }
    }

    /// Removes the stored key for `label`, when the broker stores one.
    ///
    /// Exposed for tests and for `P2-K5`'s revocation work; the hierarchy itself
    /// never deletes a key.
    pub fn purge(label: &str, blob: &[u8]) -> Result<bool, KeystoreFailure> {
        let outcome = native::purge(&label_of(label)?, blob).map_err(|error| translate(&error))?;
        Ok(matches!(outcome, native::PurgeOutcome::Removed))
    }
}

#[cfg(feature = "os-keystore")]
pub use platform::{PlatformKeystore, purge};
