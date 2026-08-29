//! The key types of the ADR-005 hierarchy and their zeroization boundary.
//!
//! Every type here owns exactly 32 bytes of key material, prints a redacted
//! `Debug`, implements [`Zeroize`] and [`ZeroizeOnDrop`], and hands its bytes
//! out only through an explicitly named `expose_secret`. There is no `Deref`,
//! no `AsRef<[u8]>`, no `Clone`, and no `Serialize`: a key cannot reach a
//! writer, a log line, an audit row, or an export by accident.

use core::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Width of every key in the hierarchy.
pub const KEY_BYTES: usize = 32;

/// Width of a profile or domain identifier used as HKDF salt or info suffix.
pub const IDENTIFIER_BYTES: usize = 16;

/// Failure of a randomness draw.
///
/// The operating system randomness source is the only external input to key
/// generation, so its failure is a distinct, non-swallowable error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomnessUnavailable;

impl fmt::Display for RandomnessUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("operating-system randomness was unavailable")
    }
}

impl std::error::Error for RandomnessUnavailable {}

/// Draws `KEY_BYTES` of operating-system randomness into a zeroizing buffer.
pub(crate) fn random_key_bytes() -> Result<Zeroizing<[u8; KEY_BYTES]>, RandomnessUnavailable> {
    let mut bytes = Zeroizing::new([0_u8; KEY_BYTES]);
    getrandom::fill(bytes.as_mut()).map_err(|_| RandomnessUnavailable)?;
    Ok(bytes)
}

/// A non-secret 16-byte profile identity, used as the HKDF salt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileId([u8; IDENTIFIER_BYTES]);

impl ProfileId {
    /// Wraps the caller's canonical 16-byte profile identity.
    ///
    /// This crate does not parse UUIDs; the caller supplies the exact bytes it
    /// uses everywhere else so the key schedule cannot drift from the profile.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; IDENTIFIER_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; IDENTIFIER_BYTES] {
        &self.0
    }
}

/// A non-secret 16-byte domain identity, appended to the KEK info string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainId([u8; IDENTIFIER_BYTES]);

impl DomainId {
    /// Wraps the caller's canonical 16-byte domain identity.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; IDENTIFIER_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; IDENTIFIER_BYTES] {
        &self.0
    }
}

macro_rules! secret_key {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// Zeroized on drop; `Debug` prints no key byte.
        pub struct $name(Zeroizing<[u8; KEY_BYTES]>);

        impl $name {
            pub(crate) const fn from_zeroizing(bytes: Zeroizing<[u8; KEY_BYTES]>) -> Self {
                Self(bytes)
            }

            /// Borrows the raw key bytes for the length of the call.
            ///
            /// The name is deliberate and greppable: every call site is a place
            /// a reviewer must confirm the bytes do not escape.
            #[must_use]
            pub fn expose_secret(&self) -> &[u8; KEY_BYTES] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("bytes", &"<redacted>")
                    .field("len", &KEY_BYTES)
                    .finish()
            }
        }

        impl Zeroize for $name {
            fn zeroize(&mut self) {
                self.0.zeroize();
            }
        }

        // `Zeroizing` already zeroizes its own buffer on drop; the marker states
        // that guarantee in the type system so a bound can assert it.
        impl ZeroizeOnDrop for $name {}
    };
}

secret_key!(
    VaultMasterKey,
    "The Vault Master Key: 32 random bytes, never persisted unwrapped."
);
secret_key!(
    DomainKek,
    "A per-domain key-encryption key, `KEK_d` of ADR-005."
);
secret_key!(
    StoreKey,
    "The SQLCipher raw store key, `SKEY_p` of ADR-005."
);
secret_key!(AuditKey, "The egress-audit key, `AUDKEY` of ADR-005.");
secret_key!(
    ArtifactDek,
    "A per-artifact data-encryption key, wrapped by a `DomainKek`."
);
secret_key!(
    DeviceWrappingKey,
    "The 32-byte secret the operating-system broker holds for a device recipient."
);
secret_key!(
    RecoverySecret,
    "The 256-bit recovery secret a recovery recipient is derived from."
);
secret_key!(
    RecipientMacKey,
    "The key the recipient-record MAC is taken under, derived from the VMK."
);
secret_key!(
    RecipientWrapKey,
    "The key a single recipient's wrapped VMK is sealed under."
);
secret_key!(
    DomainLocatorKey,
    "The HMAC key one domain's vault locators are derived under."
);
secret_key!(
    RehearsalKey,
    "The key a restore-rehearsal receipt is authenticated under, derived from the VMK."
);

impl VaultMasterKey {
    /// Generates a fresh Vault Master Key from operating-system randomness.
    pub fn generate() -> Result<Self, RandomnessUnavailable> {
        Ok(Self::from_zeroizing(random_key_bytes()?))
    }

    /// Reconstructs a key from bytes recovered by an unwrap.
    pub(crate) const fn from_bytes(bytes: Zeroizing<[u8; KEY_BYTES]>) -> Self {
        Self::from_zeroizing(bytes)
    }
}

impl ArtifactDek {
    /// Generates a fresh per-artifact data-encryption key.
    pub fn generate() -> Result<Self, RandomnessUnavailable> {
        Ok(Self::from_zeroizing(random_key_bytes()?))
    }
}

impl DeviceWrappingKey {
    /// Generates the secret handed to the operating-system broker.
    pub fn generate() -> Result<Self, RandomnessUnavailable> {
        Ok(Self::from_zeroizing(random_key_bytes()?))
    }
}

impl RecoverySecret {
    /// Generates a fresh 256-bit recovery secret.
    ///
    /// The 24-word encoding of this secret is owned by `P2-K4` with the
    /// recovery profiles; this crate deliberately exposes no word-level entry
    /// point, so no API here can report *which* word of a phrase was wrong.
    pub fn generate() -> Result<Self, RandomnessUnavailable> {
        Ok(Self::from_zeroizing(random_key_bytes()?))
    }

    /// Accepts an externally decoded 256-bit recovery secret.
    #[must_use]
    pub fn from_entropy(bytes: [u8; KEY_BYTES]) -> Self {
        Self::from_zeroizing(Zeroizing::new(bytes))
    }
}

impl StoreKey {
    /// Renders the key as the lowercase hex SQLCipher expects in
    /// `PRAGMA key = "x'<64 hex>'"`.
    ///
    /// Returned inside a `Zeroizing` so the rendered text is cleared too; the
    /// caller must not copy it into an owned `String`.
    #[must_use]
    pub fn expose_raw_hex(&self) -> Zeroizing<String> {
        let mut rendered = String::with_capacity(KEY_BYTES * 2);
        for byte in self.expose_secret() {
            // Written by hand rather than through a formatter so no temporary
            // allocation holding key text escapes the zeroizing buffer.
            rendered.push(nibble_to_hex(byte >> 4));
            rendered.push(nibble_to_hex(byte & 0x0F));
        }
        Zeroizing::new(rendered)
    }
}

const fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}

    #[test]
    fn every_key_type_is_zeroize_on_drop() {
        assert_zeroize_on_drop::<VaultMasterKey>();
        assert_zeroize_on_drop::<DomainKek>();
        assert_zeroize_on_drop::<StoreKey>();
        assert_zeroize_on_drop::<AuditKey>();
        assert_zeroize_on_drop::<ArtifactDek>();
        assert_zeroize_on_drop::<DeviceWrappingKey>();
        assert_zeroize_on_drop::<RecoverySecret>();
        assert_zeroize_on_drop::<RecipientMacKey>();
        assert_zeroize_on_drop::<RecipientWrapKey>();
        assert_zeroize_on_drop::<DomainLocatorKey>();
    }

    #[test]
    fn generated_keys_are_full_width_and_not_constant() {
        let Ok(first) = VaultMasterKey::generate() else {
            unreachable!("randomness must be available in a test host");
        };
        let Ok(second) = VaultMasterKey::generate() else {
            unreachable!("randomness must be available in a test host");
        };
        assert_eq!(first.expose_secret().len(), KEY_BYTES);
        assert_ne!(first.expose_secret(), second.expose_secret());
        assert_ne!(first.expose_secret(), &[0_u8; KEY_BYTES]);
    }

    #[test]
    fn raw_hex_is_lowercase_and_exactly_sixty_four_characters() {
        let key = StoreKey::from_zeroizing(Zeroizing::new([0xAB_u8; KEY_BYTES]));
        let rendered = key.expose_raw_hex();
        assert_eq!(rendered.len(), 64);
        assert_eq!(&*rendered, &"ab".repeat(32));
    }
}
