//! Reviewed operating-system key-broker boundary.
//!
//! This private workspace crate is the only place where the product may ask the
//! operating system to hold or seal key material. Its public API is safe: no
//! raw handle, descriptor, pointer, or key serial crosses this module boundary,
//! and every returned secret is zeroized on drop with a redacted `Debug`.
//!
//! The broker seals one secret -- the 32-byte device wrapping key of
//! `academic-crypto` -- and returns an opaque blob. It never sees the vault
//! master key and never writes a file.

use std::fmt;

#[cfg(any(windows, test))]
use zeroize::Zeroize as _;
use zeroize::Zeroizing;

#[cfg(all(target_os = "linux", feature = "secret-service"))]
mod linux;
#[cfg(windows)]
mod windows;

/// Native broker compiled into this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeystoreProvider {
    /// Windows CNG DPAPI (`NCryptProtectSecret`) under a `LOCAL=user` descriptor.
    WindowsDpapiCng,
    /// Linux Secret Service (`org.freedesktop.secrets`) default collection.
    LinuxSecretService,
    /// No reviewed broker exists for this target.
    Unsupported,
}

impl KeystoreProvider {
    /// Returns the stable external spelling used by records and reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDpapiCng => "WINDOWS_DPAPI_CNG",
            Self::LinuxSecretService => "LINUX_SECRET_SERVICE",
            Self::Unsupported => "UNSUPPORTED",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::WindowsDpapiCng => 1,
            Self::LinuxSecretService => 2,
            Self::Unsupported => 0,
        }
    }
}

/// The broker compiled into this build, decided at compile time only.
pub const PROVIDER: KeystoreProvider = compiled_provider();

const fn compiled_provider() -> KeystoreProvider {
    #[cfg(windows)]
    {
        KeystoreProvider::WindowsDpapiCng
    }
    #[cfg(all(target_os = "linux", feature = "secret-service"))]
    {
        KeystoreProvider::LinuxSecretService
    }
    #[cfg(not(any(windows, all(target_os = "linux", feature = "secret-service"))))]
    {
        KeystoreProvider::Unsupported
    }
}

/// Largest secret this boundary will seal.
///
/// The product seals one 32-byte device wrapping key. The bound exists so a
/// caller mistake becomes a typed refusal instead of an unbounded native call.
pub const MAX_SECRET_BYTES: usize = 4096;

/// Longest accepted label.
pub const MAX_LABEL_BYTES: usize = 128;

/// Stable category for key-broker failures.
///
/// No variant carries key bytes, a label secret, or a native handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeystoreErrorCode {
    /// The platform broker is not present or refused to start.
    Unavailable,
    /// The label has no stored secret.
    NotFound,
    /// The broker refused this caller.
    AccessDenied,
    /// The label is empty, over-long, or not in the accepted alphabet.
    InvalidLabel,
    /// The blob is not a well-formed sealed envelope.
    InvalidSealedBlob,
    /// The blob was sealed by a different provider than this build carries.
    ProviderMismatch,
    /// The secret is empty or exceeds [`MAX_SECRET_BYTES`].
    SecretTooLarge,
    /// This target has no reviewed broker.
    Unsupported,
    /// The native call failed for an operating-system reason.
    OperatingSystem,
}

/// Privacy-bounded error returned by the native facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystoreError {
    /// Stable failure category.
    pub code: KeystoreErrorCode,
    /// Constant description of the step that failed.
    pub operation: &'static str,
    /// Operating-system status, when the native call produced one.
    pub os_code: Option<i64>,
}

impl KeystoreError {
    pub(crate) const fn new(
        code: KeystoreErrorCode,
        operation: &'static str,
        os_code: Option<i64>,
    ) -> Self {
        Self {
            code,
            operation,
            os_code,
        }
    }
}

impl fmt::Display for KeystoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} while {}", self.code, self.operation)?;
        if let Some(code) = self.os_code {
            write!(formatter, " (os code {code})")?;
        }
        Ok(())
    }
}

impl std::error::Error for KeystoreError {}

/// What a purge actually did.
///
/// The two brokers differ and the difference is carried in the type rather than
/// hidden: a stored-key broker removes an object, a stateless sealing broker has
/// nothing to remove and cannot revoke an already-issued blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PurgeOutcome {
    /// A stored key object was removed from the operating system.
    Removed,
    /// This provider stores nothing; the blob remains openable by its owner.
    NothingStored,
}

/// A validated broker label.
///
/// The label is a non-secret, stable name for one sealed secret. It is bound
/// into the sealed plaintext, so presenting a blob under a different label
/// fails instead of returning another recipient's key.
#[derive(Clone, PartialEq, Eq)]
pub struct KeystoreLabel(String);

impl KeystoreLabel {
    /// Accepts a bounded ASCII label of `a-z`, `0-9`, `-`, `_`, `:` and `.`.
    pub fn new(value: &str) -> Result<Self, KeystoreError> {
        const OPERATION: &str = "validate keystore label";
        if value.is_empty() || value.len() > MAX_LABEL_BYTES {
            return Err(KeystoreError::new(
                KeystoreErrorCode::InvalidLabel,
                OPERATION,
                None,
            ));
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b':' | b'.')
        }) {
            return Err(KeystoreError::new(
                KeystoreErrorCode::InvalidLabel,
                OPERATION,
                None,
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// Returns the validated label text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for KeystoreLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("KeystoreLabel")
            .field(&self.0)
            .finish()
    }
}

/// A secret recovered from the broker.
///
/// The buffer is zeroized on drop and `Debug` prints only its length.
pub struct RecoveredSecret(Zeroizing<Vec<u8>>);

impl RecoveredSecret {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Borrows the recovered bytes for the length of the call.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Returns the recovered length without exposing the bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether nothing was recovered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RecoveredSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveredSecret")
            .field("bytes", &"<redacted>")
            .field("len", &self.0.len())
            .finish()
    }
}

const ENVELOPE_MAGIC: [u8; 4] = *b"AKSB";
const ENVELOPE_VERSION: u8 = 1;
const ENVELOPE_HEADER_LEN: usize = 10;

/// Frames a provider payload so a foreign or corrupt blob fails before any
/// native call, and a blob from the other platform is refused by provider tag.
fn encode_envelope(provider: KeystoreProvider, payload: &[u8]) -> Vec<u8> {
    let declared = u32::try_from(payload.len()).unwrap_or(u32::MAX);
    let mut blob = Vec::with_capacity(ENVELOPE_HEADER_LEN + payload.len());
    blob.extend_from_slice(&ENVELOPE_MAGIC);
    blob.push(ENVELOPE_VERSION);
    blob.push(provider.tag());
    blob.extend_from_slice(&declared.to_le_bytes());
    blob.extend_from_slice(payload);
    blob
}

fn decode_envelope<'blob>(
    blob: &'blob [u8],
    operation: &'static str,
) -> Result<&'blob [u8], KeystoreError> {
    let invalid = || KeystoreError::new(KeystoreErrorCode::InvalidSealedBlob, operation, None);
    let header = blob.get(..ENVELOPE_HEADER_LEN).ok_or_else(invalid)?;
    if header[..4] != ENVELOPE_MAGIC || header[4] != ENVELOPE_VERSION {
        return Err(invalid());
    }
    if header[5] != PROVIDER.tag() {
        return Err(KeystoreError::new(
            KeystoreErrorCode::ProviderMismatch,
            operation,
            None,
        ));
    }
    let declared = u32::from_le_bytes([header[6], header[7], header[8], header[9]]);
    let payload = blob.get(ENVELOPE_HEADER_LEN..).ok_or_else(invalid)?;
    if u64::from(declared) != payload.len() as u64 {
        return Err(invalid());
    }
    Ok(payload)
}

/// Binds the label into the sealed plaintext so a blob cannot be replayed under
/// a different label by a caller that can read the recipient record.
///
/// Only a stateless sealing provider needs this. A stored-key provider finds
/// its item *by* the label, so the binding is inherent there.
#[cfg(any(windows, test))]
fn bind_label(label: &KeystoreLabel, secret: &[u8]) -> Zeroizing<Vec<u8>> {
    let label_bytes = label.as_str().as_bytes();
    let declared = u16::try_from(label_bytes.len()).unwrap_or(u16::MAX);
    let mut bound = Vec::with_capacity(2 + label_bytes.len() + secret.len());
    bound.extend_from_slice(&declared.to_le_bytes());
    bound.extend_from_slice(label_bytes);
    bound.extend_from_slice(secret);
    Zeroizing::new(bound)
}

#[cfg(any(windows, test))]
fn unbind_label(
    label: &KeystoreLabel,
    mut bound: Vec<u8>,
    operation: &'static str,
) -> Result<RecoveredSecret, KeystoreError> {
    let label_bytes = label.as_str().as_bytes();
    let prefix = 2 + label_bytes.len();
    let matches = bound.len() > prefix
        && usize::from(u16::from_le_bytes([bound[0], bound[1]])) == label_bytes.len()
        && bound[2..prefix] == *label_bytes;
    if !matches {
        bound.zeroize();
        return Err(KeystoreError::new(
            KeystoreErrorCode::InvalidSealedBlob,
            operation,
            None,
        ));
    }
    let secret = bound.split_off(prefix);
    bound.zeroize();
    Ok(RecoveredSecret::new(secret))
}

/// Asks the operating system to hold `secret` under `label`.
///
/// Returns an opaque blob for the caller to persist beside its recipient
/// record. On a stored-key provider the blob carries no secret byte at all; on
/// a sealing provider it is ciphertext the operating system alone can open.
pub fn seal(label: &KeystoreLabel, secret: &[u8]) -> Result<Vec<u8>, KeystoreError> {
    const OPERATION: &str = "seal device secret";
    if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
        return Err(KeystoreError::new(
            KeystoreErrorCode::SecretTooLarge,
            OPERATION,
            None,
        ));
    }
    #[cfg(windows)]
    {
        windows::seal(label, secret, OPERATION)
    }
    #[cfg(all(target_os = "linux", feature = "secret-service"))]
    {
        linux::seal(label, secret, OPERATION)
    }
    #[cfg(not(any(windows, all(target_os = "linux", feature = "secret-service"))))]
    {
        let _ = (label, secret);
        Err(KeystoreError::new(
            KeystoreErrorCode::Unsupported,
            OPERATION,
            None,
        ))
    }
}

/// Recovers the secret previously sealed under `label`.
pub fn open(label: &KeystoreLabel, blob: &[u8]) -> Result<RecoveredSecret, KeystoreError> {
    const OPERATION: &str = "open device secret";
    let payload = decode_envelope(blob, OPERATION)?;
    #[cfg(windows)]
    {
        windows::open(label, payload, OPERATION)
    }
    #[cfg(all(target_os = "linux", feature = "secret-service"))]
    {
        linux::open(label, payload, OPERATION)
    }
    #[cfg(not(any(windows, all(target_os = "linux", feature = "secret-service"))))]
    {
        let _ = (label, payload);
        Err(KeystoreError::new(
            KeystoreErrorCode::Unsupported,
            OPERATION,
            None,
        ))
    }
}

/// Removes a stored key object, when this provider stores one.
pub fn purge(label: &KeystoreLabel, blob: &[u8]) -> Result<PurgeOutcome, KeystoreError> {
    const OPERATION: &str = "purge device secret";
    let payload = decode_envelope(blob, OPERATION)?;
    #[cfg(windows)]
    {
        let _ = (label, payload);
        Ok(PurgeOutcome::NothingStored)
    }
    #[cfg(all(target_os = "linux", feature = "secret-service"))]
    {
        linux::purge(label, payload, OPERATION)
    }
    #[cfg(not(any(windows, all(target_os = "linux", feature = "secret-service"))))]
    {
        let _ = (label, payload);
        Err(KeystoreError::new(
            KeystoreErrorCode::Unsupported,
            OPERATION,
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(value: &str) -> KeystoreLabel {
        match KeystoreLabel::new(value) {
            Ok(label) => label,
            Err(error) => unreachable!("test label rejected: {error}"),
        }
    }

    #[test]
    fn labels_accept_only_the_bounded_alphabet() {
        assert!(KeystoreLabel::new("academic-os:device:v1.0").is_ok());
        for value in ["", "Upper", "space bar", "slash/", "\u{fe0f}"] {
            let Err(error) = KeystoreLabel::new(value) else {
                unreachable!("{value} must be refused");
            };
            assert_eq!(error.code, KeystoreErrorCode::InvalidLabel, "{value}");
        }
        let over_long = "a".repeat(MAX_LABEL_BYTES + 1);
        let Err(error) = KeystoreLabel::new(&over_long) else {
            unreachable!("an over-long label must be refused");
        };
        assert_eq!(error.code, KeystoreErrorCode::InvalidLabel);
    }

    #[test]
    fn envelope_round_trips_and_rejects_foreign_blobs() {
        let framed = encode_envelope(PROVIDER, b"payload");
        assert_eq!(decode_envelope(&framed, "test").ok(), Some(&b"payload"[..]));

        let mut truncated = framed.clone();
        truncated.pop();
        let Err(error) = decode_envelope(&truncated, "test") else {
            unreachable!("a truncated blob must be refused");
        };
        assert_eq!(error.code, KeystoreErrorCode::InvalidSealedBlob);

        let mut wrong_magic = framed.clone();
        wrong_magic[0] = b'X';
        let Err(error) = decode_envelope(&wrong_magic, "test") else {
            unreachable!("a foreign magic must be refused");
        };
        assert_eq!(error.code, KeystoreErrorCode::InvalidSealedBlob);

        let mut wrong_version = framed.clone();
        wrong_version[4] = ENVELOPE_VERSION.wrapping_add(1);
        let Err(error) = decode_envelope(&wrong_version, "test") else {
            unreachable!("an unknown version must be refused");
        };
        assert_eq!(error.code, KeystoreErrorCode::InvalidSealedBlob);

        let mut other_provider = framed;
        other_provider[5] = PROVIDER.tag().wrapping_add(1);
        let Err(error) = decode_envelope(&other_provider, "test") else {
            unreachable!("another provider's blob must be refused");
        };
        assert_eq!(error.code, KeystoreErrorCode::ProviderMismatch);
    }

    #[test]
    fn label_binding_detects_a_replayed_blob() {
        let first = label("first");
        let second = label("second");
        let bound = bind_label(&first, &[7_u8; 32]);
        let Ok(recovered) = unbind_label(&first, bound.to_vec(), "test") else {
            unreachable!("the sealing label must open its own blob");
        };
        assert_eq!(recovered.expose(), &[7_u8; 32]);
        let Err(error) = unbind_label(&second, bound.to_vec(), "test") else {
            unreachable!("another label must not open this blob");
        };
        assert_eq!(error.code, KeystoreErrorCode::InvalidSealedBlob);
    }

    #[test]
    fn recovered_secret_debug_is_redacted() {
        let secret = RecoveredSecret::new(vec![0xAB; 32]);
        let rendered = format!("{secret:?}");
        assert!(rendered.contains("<redacted>"), "{rendered}");
        assert!(!rendered.contains("171"), "{rendered}");
    }

    #[test]
    fn provider_spelling_and_tag_are_stable() {
        assert_eq!(
            KeystoreProvider::WindowsDpapiCng.as_str(),
            "WINDOWS_DPAPI_CNG"
        );
        assert_eq!(
            KeystoreProvider::LinuxSecretService.as_str(),
            "LINUX_SECRET_SERVICE"
        );
        assert_eq!(KeystoreProvider::WindowsDpapiCng.tag(), 1);
        assert_eq!(KeystoreProvider::LinuxSecretService.tag(), 2);
    }
}
