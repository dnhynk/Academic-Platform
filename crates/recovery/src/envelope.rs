//! The signed, encrypted backup manifest envelope.
//!
//! ADR-012 requires the manifest to be *signed* and *encrypted* under the
//! independent backup key. Both keys come from the backup root:
//!
//! ```text
//! MKEY   = HKDF-SHA-512(BMK, salt = backup_set_id, info = "academic-os/backup-manifest/v1")
//! SIGSEED= HKDF-SHA-512(BMK, salt = backup_set_id, info = "academic-os/backup-signature/v1")
//! ```
//!
//! The signature is over the *sealed* bytes plus the plaintext header, so a
//! holder of the public key can verify who produced a backup without being able
//! to read it. The public key is recorded in the plaintext header, so the two
//! properties stay separable: verification needs the header, opening needs the
//! root.
//!
//! The body is opaque here. What goes into it — watermark, counts, object
//! closure, file inventory — is the encrypted portability lane's contract.

use academic_crypto::KEY_BYTES;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use ciborium::value::{Integer, Value};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use sha2::{Digest as _, Sha256};

use crate::backup_key::{BackupKeyError, BackupMasterKey, BackupSetId, decode_canonical, encode};

/// Version of the sealed-manifest envelope.
pub const ENVELOPE_VERSION: u8 = 1;
/// Info string for the manifest AEAD key.
pub const MANIFEST_AEAD_INFO: &[u8] = b"academic-os/backup-manifest/v1";
/// Info string for the manifest signing seed.
pub const MANIFEST_SIGNING_INFO: &[u8] = b"academic-os/backup-signature/v1";
/// Domain separator mixed into the signed preimage.
pub const SIGNATURE_DOMAIN: &[u8] = b"academic-os/backup-manifest-signature/v1";
/// Bytes of the AEAD nonce.
pub const NONCE_BYTES: usize = 24;

/// One sealed manifest: a plaintext header, ciphertext, and a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedManifest {
    set_id: BackupSetId,
    format: String,
    manifest_version: u32,
    nonce: [u8; NONCE_BYTES],
    ciphertext: Vec<u8>,
    verifying_key: [u8; 32],
    signature: [u8; 64],
}

impl SealedManifest {
    /// Seals and signs one manifest body under the backup root.
    pub fn seal(
        root: &BackupMasterKey,
        set_id: BackupSetId,
        format: &str,
        manifest_version: u32,
        body: &[u8],
    ) -> Result<Self, SealedManifestError> {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| SealedManifestError::Randomness)?;
        let signing = signing_key(root, set_id)?;
        let verifying_key = signing.verifying_key().to_bytes();

        let mut sealed = Self {
            set_id,
            format: format.to_owned(),
            manifest_version,
            nonce,
            ciphertext: Vec::new(),
            verifying_key,
            signature: [0_u8; 64],
        };
        let aad = sealed.header_bytes()?;
        let aead_key = root
            .derive(set_id, MANIFEST_AEAD_INFO)
            .map_err(SealedManifestError::Key)?;
        let cipher = XChaCha20Poly1305::new_from_slice(aead_key.as_ref())
            .map_err(|_| SealedManifestError::Seal)?;
        sealed.ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: body,
                    aad: &aad,
                },
            )
            .map_err(|_| SealedManifestError::Seal)?;
        sealed.signature = signing.sign(&sealed.signed_preimage()?).to_bytes();
        Ok(sealed)
    }

    /// Verifies the signature without opening the manifest.
    ///
    /// This is the half a holder of the public key alone can do; it proves the
    /// bytes came from a holder of the backup root and have not been edited,
    /// and it reads nothing out of the body.
    pub fn verify_signature(&self) -> Result<(), SealedManifestError> {
        let verifying = VerifyingKey::from_bytes(&self.verifying_key)
            .map_err(|_| SealedManifestError::Signature)?;
        let signature = Signature::from_bytes(&self.signature);
        verifying
            .verify(&self.signed_preimage()?, &signature)
            .map_err(|_| SealedManifestError::Signature)
    }

    /// Verifies the signature and returns the manifest body.
    ///
    /// The signature is checked first, so a tampered envelope is reported as a
    /// forged signature rather than as an AEAD failure, and the AEAD is never
    /// asked to open bytes that already failed a cheaper check.
    pub fn open(&self, root: &BackupMasterKey) -> Result<Vec<u8>, SealedManifestError> {
        self.verify_signature()?;
        let signing = signing_key(root, self.set_id)?;
        if signing.verifying_key().to_bytes() != self.verifying_key {
            return Err(SealedManifestError::WrongKey);
        }
        let aad = self.header_bytes()?;
        let aead_key = root
            .derive(self.set_id, MANIFEST_AEAD_INFO)
            .map_err(SealedManifestError::Key)?;
        let cipher = XChaCha20Poly1305::new_from_slice(aead_key.as_ref())
            .map_err(|_| SealedManifestError::Seal)?;
        cipher
            .decrypt(
                XNonce::from_slice(&self.nonce),
                Payload {
                    msg: self.ciphertext.as_slice(),
                    aad: &aad,
                },
            )
            .map_err(|_| SealedManifestError::WrongKey)
    }

    /// Returns the backup identity this envelope is bound to.
    #[must_use]
    pub const fn set_id(&self) -> BackupSetId {
        self.set_id
    }

    /// Returns the declared backup format name.
    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Returns the declared manifest version.
    #[must_use]
    pub const fn manifest_version(&self) -> u32 {
        self.manifest_version
    }

    /// Returns the public key the signature verifies against.
    #[must_use]
    pub const fn verifying_key(&self) -> &[u8; 32] {
        &self.verifying_key
    }

    /// Returns the SHA-256 digest of the sealed envelope bytes.
    ///
    /// Two backups of the same watermark do not have equal envelope digests —
    /// the nonce and the volatile block differ — so this identifies one
    /// physical envelope, never a semantic state.
    pub fn envelope_digest(&self) -> Result<[u8; 32], SealedManifestError> {
        let mut hasher = Sha256::new();
        hasher.update(self.to_canonical_cbor()?);
        Ok(hasher.finalize().into())
    }

    fn header_value(&self) -> Value {
        Value::Map(vec![
            (
                Value::Integer(Integer::from(0_u64)),
                Value::Integer(Integer::from(u32::from(ENVELOPE_VERSION))),
            ),
            (
                Value::Integer(Integer::from(1_u64)),
                Value::Bytes(self.set_id.as_bytes().to_vec()),
            ),
            (
                Value::Integer(Integer::from(2_u64)),
                Value::Text(self.format.clone()),
            ),
            (
                Value::Integer(Integer::from(3_u64)),
                Value::Integer(Integer::from(self.manifest_version)),
            ),
            (
                Value::Integer(Integer::from(4_u64)),
                Value::Bytes(self.nonce.to_vec()),
            ),
            (
                Value::Integer(Integer::from(5_u64)),
                Value::Bytes(self.verifying_key.to_vec()),
            ),
        ])
    }

    fn header_bytes(&self) -> Result<Vec<u8>, SealedManifestError> {
        encode(&self.header_value()).map_err(SealedManifestError::Key)
    }

    fn signed_preimage(&self) -> Result<Vec<u8>, SealedManifestError> {
        let mut preimage = Vec::with_capacity(SIGNATURE_DOMAIN.len() + self.ciphertext.len() + 64);
        preimage.extend_from_slice(SIGNATURE_DOMAIN);
        preimage.extend_from_slice(&self.header_bytes()?);
        preimage.extend_from_slice(&self.ciphertext);
        Ok(preimage)
    }

    /// Encodes the envelope as canonical deterministic CBOR.
    pub fn to_canonical_cbor(&self) -> Result<Vec<u8>, SealedManifestError> {
        let Value::Map(mut entries) = self.header_value() else {
            return Err(SealedManifestError::Seal);
        };
        entries.push((
            Value::Integer(Integer::from(6_u64)),
            Value::Bytes(self.ciphertext.clone()),
        ));
        entries.push((
            Value::Integer(Integer::from(7_u64)),
            Value::Bytes(self.signature.to_vec()),
        ));
        encode(&Value::Map(entries)).map_err(SealedManifestError::Key)
    }

    /// Decodes and canonicality-checks an envelope.
    pub fn from_canonical_cbor(input: &[u8]) -> Result<Self, SealedManifestError> {
        let value = decode_canonical(input).map_err(SealedManifestError::Key)?;
        let entries = value.as_map().ok_or(SealedManifestError::Shape)?;
        if entries.len() != 8 {
            return Err(SealedManifestError::Shape);
        }
        for (index, entry) in entries.iter().enumerate() {
            let key = entry
                .0
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
                .ok_or(SealedManifestError::Shape)?;
            if usize::try_from(key).unwrap_or(usize::MAX) != index {
                return Err(SealedManifestError::Shape);
            }
        }
        let version = entries[0]
            .1
            .as_integer()
            .and_then(|integer| u128::try_from(integer).ok())
            .ok_or(SealedManifestError::Shape)?;
        if version != u128::from(ENVELOPE_VERSION) {
            return Err(SealedManifestError::UnsupportedVersion);
        }
        let set_id = fixed_bytes::<16>(&entries[1].1)?;
        let format = entries[1 + 1]
            .1
            .as_text()
            .map(str::to_owned)
            .ok_or(SealedManifestError::Shape)?;
        let manifest_version = entries[3]
            .1
            .as_integer()
            .and_then(|integer| u128::try_from(integer).ok())
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(SealedManifestError::Shape)?;
        Ok(Self {
            set_id: BackupSetId::from_bytes(set_id),
            format,
            manifest_version,
            nonce: fixed_bytes::<NONCE_BYTES>(&entries[4].1)?,
            verifying_key: fixed_bytes::<32>(&entries[5].1)?,
            ciphertext: entries[6]
                .1
                .as_bytes()
                .cloned()
                .ok_or(SealedManifestError::Shape)?,
            signature: fixed_bytes::<64>(&entries[7].1)?,
        })
    }
}

fn fixed_bytes<const N: usize>(value: &Value) -> Result<[u8; N], SealedManifestError> {
    let bytes = value.as_bytes().ok_or(SealedManifestError::Shape)?;
    <[u8; N]>::try_from(bytes.as_slice()).map_err(|_| SealedManifestError::Shape)
}

fn signing_key(
    root: &BackupMasterKey,
    set_id: BackupSetId,
) -> Result<SigningKey, SealedManifestError> {
    let seed = root
        .derive(set_id, MANIFEST_SIGNING_INFO)
        .map_err(SealedManifestError::Key)?;
    let mut bytes = [0_u8; KEY_BYTES];
    bytes.copy_from_slice(seed.as_ref());
    Ok(SigningKey::from_bytes(&bytes))
}

/// Why a sealed manifest could not be produced, verified, or opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SealedManifestError {
    /// The envelope is not the shape this build writes.
    #[error("the sealed manifest is not the expected shape")]
    Shape,
    /// The envelope declares an unsupported version.
    #[error("the sealed manifest declares an unsupported envelope version")]
    UnsupportedVersion,
    /// The signature did not verify.
    #[error("the sealed manifest signature did not verify")]
    Signature,
    /// The presented root does not open this manifest.
    #[error("the presented backup root does not open this manifest")]
    WrongKey,
    /// The AEAD refused to seal.
    #[error("the manifest could not be sealed")]
    Seal,
    /// Operating-system randomness was unavailable.
    #[error("operating-system randomness was unavailable")]
    Randomness,
    /// A key or encoding operation failed.
    #[error(transparent)]
    Key(#[from] BackupKeyError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> BackupMasterKey {
        BackupMasterKey::generate().unwrap_or_else(|_| unreachable!("randomness must be available"))
    }

    #[test]
    fn a_sealed_manifest_round_trips_through_canonical_cbor() -> Result<(), SealedManifestError> {
        let key = root();
        let set_id = BackupSetId::from_bytes([0x21; 16]);
        let sealed = SealedManifest::seal(&key, set_id, "TEST_FORMAT", 2, b"body")?;
        let bytes = sealed.to_canonical_cbor()?;
        let parsed = SealedManifest::from_canonical_cbor(&bytes)?;
        assert_eq!(parsed, sealed);
        assert_eq!(parsed.open(&key)?, b"body");
        parsed.verify_signature()?;
        Ok(())
    }

    #[test]
    fn a_flipped_ciphertext_byte_fails_the_signature() -> Result<(), SealedManifestError> {
        let key = root();
        let set_id = BackupSetId::from_bytes([0x22; 16]);
        let mut sealed = SealedManifest::seal(&key, set_id, "TEST_FORMAT", 2, b"body")?;
        sealed.ciphertext[0] ^= 0x01;
        assert_eq!(
            sealed.verify_signature(),
            Err(SealedManifestError::Signature)
        );
        assert_eq!(sealed.open(&key), Err(SealedManifestError::Signature));
        Ok(())
    }

    #[test]
    fn another_root_neither_verifies_nor_opens() -> Result<(), SealedManifestError> {
        let key = root();
        let other = root();
        let set_id = BackupSetId::from_bytes([0x23; 16]);
        let sealed = SealedManifest::seal(&key, set_id, "TEST_FORMAT", 2, b"body")?;
        assert_eq!(sealed.open(&other), Err(SealedManifestError::WrongKey));
        Ok(())
    }
}
