//! The frozen `AEAD_CHUNKED_V2` object format.
//!
//! # On-disk layout
//!
//! ```text
//! offset size field
//! 0      4    "ACOB"
//! 4      2    u16 LE format_version = 2
//! 6      2    u16 LE header_len = 200
//! 8      1    u8  aead_id = 1 (XChaCha20-Poly1305)
//! 9      4    u32 LE chunk_size
//! 13     16   artifact_id
//! 29     16   domain_id
//! 45     1    u8  retention_class
//! 46     16   permission_lineage_id
//! 62     24   base_nonce                     <- end of the streaming prefix P0 (86 bytes)
//! 86     32   locator
//! 118    8    u64 LE plaintext_len
//! 126    2    u16 LE wrapped_dek_len = 80    <- end of the wrap AAD P (128 bytes)
//! 128    80   wrapped_dek                    <- end of the header (208 bytes)
//! 208    ..   chunk 0, chunk 1, ...
//! ```
//!
//! ```text
//! wrapped_dek := XChaCha20Poly1305(KEK_d, nonce = base_nonce, aad = P,
//!                                  plaintext = DEK(32) || plaintext_sha256(32))
//!              = 64 bytes ciphertext || 16 bytes Poly1305 tag
//! chunk_i     := XChaCha20Poly1305(DEK, nonce = base_nonce XOR LE64(i + 1),
//!                                  aad = SHA-256(P0) | LE64(i) | LE32(len_i) | u8 is_final,
//!                                  plaintext = plaintext[i * chunk_size ..])
//! ```
//!
//! `header_tag` is **not a separate field**: it is the trailing 16 bytes of
//! `wrapped_dek`, which is the Poly1305 tag of the wrap above. A reader looking
//! for a distinct tag field will not find one.
//!
//! The chunk nonce XORs `LE64(i + 1)` into the **trailing eight bytes** of the
//! 24-byte base nonce, that is `base_nonce[16..24]`, little-endian. Bytes
//! `0..16` are never modified.
//!
//! Every multi-byte integer in the format is little-endian. A zero-length
//! object has exactly one chunk, with `len_0 = 0` and `is_final = 1`.
//!
//! # Why this is not §3.4 word for word
//!
//! t068 §3.4 writes `12B base_nonce` while fixing `aead_id = 1` to
//! XChaCha20-Poly1305, whose nonce is 24 bytes; the width is what changed.
//!
//! §3.4 also writes `header_tag := AEAD(KEK_d, nonce=base_nonce, aad="ACOB"|
//! version|header_len, plaintext = header)` with `base_nonce` and `wrapped_dek`
//! *inside* that header. That cannot be executed: a reader must read
//! `base_nonce` before it can run any KEK_d operation, an already-wrapped DEK
//! cannot also be the plaintext that wraps it, and a second KEK_d operation
//! under the same `base_nonce` would be nonce reuse. The wrap above is the one
//! reading that keeps every property §3.4 asserts.
//!
//! Finally, the chunk AAD hashes `P0` rather than the whole header. §7 requires
//! `OB01` ("kill after DEK generation, before header write") and `OB03` ("kill
//! after final chunk, before header tag") to be reachable, so the header is
//! written *after* the last chunk — and the header carries `locator` and
//! `plaintext_len`, neither of which is known until the stream ends. Binding
//! chunks to `P0` loses nothing: `P0` is a prefix of `P`, `P` is the wrap's
//! AAD, and the wrap is verified before any chunk is read, so the header tag
//! already authenticates `P0`. `base_nonce` is 24 random bytes per object, so
//! `SHA-256(P0)` still identifies one object uniquely.

use academic_domain::{ArtifactDescriptor, ContentDigest, RetentionClass};
use chacha20poly1305::{
    KeyInit as _, XChaCha20Poly1305, XNonce,
    aead::{Aead as _, AeadInPlace as _, Payload},
};
use sha2::{Digest as _, Sha256};

use crate::VaultError;

/// Object magic. Every `AEAD_CHUNKED_V2` object starts with these four bytes.
pub const OBJECT_MAGIC: [u8; 4] = *b"ACOB";
/// Object format version written into the header and into every descriptor.
pub const OBJECT_FORMAT_VERSION: u16 = 2;
/// The only admitted AEAD identifier: XChaCha20-Poly1305.
pub const AEAD_ID_XCHACHA20_POLY1305: u8 = 1;
/// Default plaintext chunk size, in bytes.
pub const DEFAULT_CHUNK_SIZE: u32 = 1_048_576;
/// Width of the per-object base nonce.
pub const BASE_NONCE_BYTES: usize = 24;
/// Width of the AEAD authentication tag.
pub const TAG_BYTES: usize = 16;
/// Width of a key in the hierarchy.
pub const KEY_BYTES: usize = 32;
/// Width of `wrapped_dek`: DEK, plaintext digest, and the wrap tag.
pub const WRAPPED_DEK_BYTES: usize = KEY_BYTES + 32 + TAG_BYTES;

/// Byte offset at which the streaming prefix `P0` ends.
pub const STREAMING_PREFIX_BYTES: usize = 86;
/// Byte offset at which the wrap AAD `P` ends.
pub const WRAP_AAD_BYTES: usize = 128;
/// Total on-disk header width.
pub const HEADER_BYTES: usize = WRAP_AAD_BYTES + WRAPPED_DEK_BYTES;
/// Value written into the `header_len` field: everything after the 8-byte magic
/// and version prefix.
pub const HEADER_LEN_FIELD: u16 = 200;

/// Byte offset at which the key slot — the wrapped DEK — starts.
///
/// `P2-K5` destroys exactly `[KEY_SLOT_OFFSET, HEADER_BYTES)` to crypto-shred
/// an object. Nothing else in the file is touched: the chunks stay, the file
/// stays, and its length stays.
pub const KEY_SLOT_OFFSET: usize = WRAP_AAD_BYTES;

/// Marker written over a destroyed key slot.
///
/// Exactly 24 bytes, followed by the 32-byte digest of the tombstone that
/// authorized the shred and 24 zero bytes, so a destroyed slot is the same
/// width as the wrapped DEK it replaced and names the record that explains it.
///
/// A shredded object is *not* a corrupt object and must not be reported as
/// one. An attacker who can write these bytes can equally overwrite the slot
/// with noise, so the marker is an operator-facing label rather than a
/// security boundary; what it buys is that a deliberate shred and a bit-rotted
/// object have different reports.
pub const KEY_SLOT_SHRED_MARKER: &[u8; 24] = b"ACOB-KEYSLOT-SHREDDED-V1";

/// Builds the exact 80 bytes a shredded key slot holds.
#[must_use]
pub fn shredded_key_slot(tombstone_digest: &[u8; 32]) -> [u8; WRAPPED_DEK_BYTES] {
    let mut slot = [0_u8; WRAPPED_DEK_BYTES];
    slot[..KEY_SLOT_SHRED_MARKER.len()].copy_from_slice(KEY_SLOT_SHRED_MARKER);
    slot[KEY_SLOT_SHRED_MARKER.len()..KEY_SLOT_SHRED_MARKER.len() + 32]
        .copy_from_slice(tombstone_digest);
    slot
}

/// Reports whether these header bytes carry a destroyed key slot.
#[must_use]
pub fn is_shredded_header(bytes: &[u8]) -> bool {
    bytes.len() >= HEADER_BYTES
        && bytes[KEY_SLOT_OFFSET..KEY_SLOT_OFFSET + KEY_SLOT_SHRED_MARKER.len()]
            == *KEY_SLOT_SHRED_MARKER
}

/// Reads the cleartext locator out of a header without any key.
///
/// The locator is a domain-keyed HMAC written in the clear at a fixed offset,
/// so a restore can match an object against a backup tombstone without holding
/// the profile key. That is what makes re-deletion on restore possible before
/// anything is unlocked.
pub fn read_locator(bytes: &[u8]) -> Result<[u8; 32], ObjectFormatError> {
    if bytes.len() < HEADER_BYTES {
        return Err(ObjectFormatError::Truncated);
    }
    if bytes[MAGIC_AT..MAGIC_AT + 4] != OBJECT_MAGIC {
        return Err(ObjectFormatError::BadMagic);
    }
    let format_version = read_u16(bytes, FORMAT_VERSION_AT);
    if format_version != OBJECT_FORMAT_VERSION {
        return Err(ObjectFormatError::UnsupportedFormatVersion(format_version));
    }
    let mut locator = [0_u8; 32];
    locator.copy_from_slice(&bytes[LOCATOR_AT..LOCATOR_AT + 32]);
    Ok(locator)
}

const MAGIC_AT: usize = 0;
const FORMAT_VERSION_AT: usize = 4;
const HEADER_LEN_AT: usize = 6;
const AEAD_ID_AT: usize = 8;
const CHUNK_SIZE_AT: usize = 9;
const ARTIFACT_ID_AT: usize = 13;
const DOMAIN_ID_AT: usize = 29;
const RETENTION_CLASS_AT: usize = 45;
const PERMISSION_LINEAGE_AT: usize = 46;
const BASE_NONCE_AT: usize = 62;
const LOCATOR_AT: usize = 86;
const PLAINTEXT_LEN_AT: usize = 118;
const WRAPPED_DEK_LEN_AT: usize = 126;
const WRAPPED_DEK_AT: usize = 128;

/// Frozen `retention_class` encoding. Changing a value is a format break.
const RETENTION_EPHEMERAL: u8 = 1;
const RETENTION_COURSE_TERM: u8 = 2;
const RETENTION_USER_MANAGED: u8 = 3;
const RETENTION_LEGAL_HOLD: u8 = 4;

const fn retention_code(value: RetentionClass) -> u8 {
    match value {
        RetentionClass::Ephemeral => RETENTION_EPHEMERAL,
        RetentionClass::CourseTerm => RETENTION_COURSE_TERM,
        RetentionClass::UserManaged => RETENTION_USER_MANAGED,
        RetentionClass::LegalHold => RETENTION_LEGAL_HOLD,
    }
}

/// Why an object did not decode, decrypt, or authenticate.
///
/// No variant carries plaintext, key material, or any part of a tag. `Aead`
/// deliberately does not say which check failed: an attacker learns nothing
/// from the difference between a wrong key, a wrong domain, and a flipped byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObjectFormatError {
    /// The file is shorter than a header, or shorter than its own header says.
    Truncated,
    /// The first four bytes are not `ACOB`.
    BadMagic,
    /// The header names a format version this reader does not write or read.
    UnsupportedFormatVersion(u16),
    /// The header names an AEAD this reader does not implement.
    UnsupportedAead(u8),
    /// A header field is outside its admitted range.
    MalformedHeader(&'static str),
    /// An AEAD tag did not verify.
    Aead,
    /// The key slot was deliberately destroyed by a crypto-shred.
    ///
    /// This is terminal and is not repairable: the wrapped DEK is the only
    /// copy of the key this object was sealed under. It is a distinct variant
    /// from [`Self::Aead`] so an operator report can tell "we destroyed this"
    /// from "this failed to authenticate".
    Shredded,
    /// The header does not describe the descriptor it was fetched for.
    IdentityMismatch(&'static str),
    /// The plaintext read back is not the plaintext the header commits to.
    PlaintextMismatch,
}

impl core::fmt::Display for ObjectFormatError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("encrypted object is truncated"),
            Self::BadMagic => formatter.write_str("encrypted object has no ACOB magic"),
            Self::UnsupportedFormatVersion(version) => write!(
                formatter,
                "encrypted object names unsupported format version {version}"
            ),
            Self::UnsupportedAead(id) => {
                write!(formatter, "encrypted object names unsupported AEAD {id}")
            }
            Self::MalformedHeader(field) => {
                write!(
                    formatter,
                    "encrypted object header field {field} is invalid"
                )
            }
            Self::Aead => formatter
                .write_str("encrypted object failed authentication; no plaintext was produced"),
            Self::Shredded => formatter.write_str(concat!(
                "encrypted object was crypto-shredded: its key slot was destroyed, ",
                "so no key opens it and the remaining ciphertext is unreadable. ",
                "The file itself was not deleted",
            )),
            Self::IdentityMismatch(field) => write!(
                formatter,
                "encrypted object header field {field} does not match its descriptor"
            ),
            Self::PlaintextMismatch => {
                formatter.write_str("encrypted object plaintext does not match its header digest")
            }
        }
    }
}

impl std::error::Error for ObjectFormatError {}

/// Everything the header commits to, after its tag verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectHeader {
    chunk_size: u32,
    artifact_id: [u8; 16],
    domain_id: [u8; 16],
    retention_class: u8,
    permission_lineage_id: [u8; 16],
    base_nonce: [u8; BASE_NONCE_BYTES],
    locator: [u8; 32],
    plaintext_len: u64,
    streaming_prefix_digest: [u8; 32],
}

impl ObjectHeader {
    /// Builds a header carrying only chunk geometry.
    ///
    /// Chunk count, chunk length, chunk offset, and sealed length are pure
    /// functions of `chunk_size` and `plaintext_len`. This constructor exists
    /// so that arithmetic can be checked at sizes no test may write to disk;
    /// the identity fields are zero and the value must not be used to
    /// authenticate anything.
    #[must_use]
    pub const fn geometry(chunk_size: u32, plaintext_len: u64) -> Self {
        Self {
            chunk_size,
            artifact_id: [0; 16],
            domain_id: [0; 16],
            retention_class: RETENTION_EPHEMERAL,
            permission_lineage_id: [0; 16],
            base_nonce: [0; BASE_NONCE_BYTES],
            locator: [0; 32],
            plaintext_len,
            streaming_prefix_digest: [0; 32],
        }
    }

    /// Returns the plaintext chunk size this object was sealed with.
    #[must_use]
    pub const fn chunk_size(&self) -> u32 {
        self.chunk_size
    }

    /// Returns the exact plaintext length the header commits to.
    #[must_use]
    pub const fn plaintext_len(&self) -> u64 {
        self.plaintext_len
    }

    /// Returns the number of chunks this object has.
    ///
    /// A zero-length object has exactly one chunk, so a reader always has a
    /// final chunk whose `is_final` byte it can authenticate.
    #[must_use]
    pub const fn chunk_count(&self) -> u64 {
        let chunk_size = self.chunk_size as u64;
        if self.plaintext_len == 0 {
            1
        } else {
            self.plaintext_len.div_ceil(chunk_size)
        }
    }

    /// Returns the plaintext length of chunk `index`.
    #[must_use]
    pub const fn chunk_plaintext_len(&self, index: u64) -> u32 {
        let chunk_size = self.chunk_size as u64;
        let start = index.saturating_mul(chunk_size);
        if start >= self.plaintext_len {
            return 0;
        }
        let remaining = self.plaintext_len - start;
        if remaining < chunk_size {
            // The remainder is below `chunk_size`, which is a `u32`.
            remaining as u32
        } else {
            self.chunk_size
        }
    }

    /// Returns the byte offset of chunk `index` inside the sealed file.
    #[must_use]
    pub const fn chunk_offset(&self, index: u64) -> u64 {
        let record = self.chunk_size as u64 + TAG_BYTES as u64;
        HEADER_BYTES as u64 + index.saturating_mul(record)
    }

    /// Returns the exact sealed file length this header implies.
    #[must_use]
    pub const fn sealed_len(&self) -> u64 {
        let count = self.chunk_count();
        HEADER_BYTES as u64 + self.plaintext_len + count.saturating_mul(TAG_BYTES as u64)
    }

    /// Returns the AAD for chunk `index`.
    #[must_use]
    pub fn chunk_aad(&self, index: u64) -> [u8; 45] {
        let mut aad = [0_u8; 45];
        aad[0..32].copy_from_slice(&self.streaming_prefix_digest);
        aad[32..40].copy_from_slice(&index.to_le_bytes());
        aad[40..44].copy_from_slice(&self.chunk_plaintext_len(index).to_le_bytes());
        aad[44] = u8::from(index + 1 == self.chunk_count());
        aad
    }

    /// Returns the nonce for chunk `index`.
    #[must_use]
    pub fn chunk_nonce(&self, index: u64) -> [u8; BASE_NONCE_BYTES] {
        let mut nonce = self.base_nonce;
        let counter = index.wrapping_add(1).to_le_bytes();
        for (slot, byte) in nonce[16..24].iter_mut().zip(counter) {
            *slot ^= byte;
        }
        nonce
    }

    /// Checks the header against the descriptor the caller asked for.
    pub fn require_matches(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<(), ObjectFormatError> {
        if &self.artifact_id != descriptor.id.as_bytes() {
            return Err(ObjectFormatError::IdentityMismatch("artifact_id"));
        }
        if &self.domain_id != descriptor.domain_id.as_bytes() {
            return Err(ObjectFormatError::IdentityMismatch("domain_id"));
        }
        if self.retention_class != retention_code(descriptor.retention_class) {
            return Err(ObjectFormatError::IdentityMismatch("retention_class"));
        }
        if &self.permission_lineage_id != descriptor.permission_lineage_id.as_bytes() {
            return Err(ObjectFormatError::IdentityMismatch("permission_lineage_id"));
        }
        if &self.locator != descriptor.vault_locator.as_bytes() {
            return Err(ObjectFormatError::IdentityMismatch("locator"));
        }
        if self.plaintext_len != descriptor.byte_length {
            return Err(ObjectFormatError::IdentityMismatch("plaintext_len"));
        }
        Ok(())
    }
}

/// The streaming prefix `P0`, fixed before the first chunk is encrypted.
#[derive(Debug)]
pub struct StreamingPrefix {
    bytes: [u8; STREAMING_PREFIX_BYTES],
    digest: [u8; 32],
}

impl StreamingPrefix {
    /// Builds `P0` from the identity a caller knows before it reads a byte.
    #[must_use]
    pub fn new(
        chunk_size: u32,
        artifact_id: [u8; 16],
        domain_id: [u8; 16],
        retention_class: RetentionClass,
        permission_lineage_id: [u8; 16],
        base_nonce: [u8; BASE_NONCE_BYTES],
    ) -> Self {
        let mut bytes = [0_u8; STREAMING_PREFIX_BYTES];
        bytes[MAGIC_AT..MAGIC_AT + 4].copy_from_slice(&OBJECT_MAGIC);
        bytes[FORMAT_VERSION_AT..FORMAT_VERSION_AT + 2]
            .copy_from_slice(&OBJECT_FORMAT_VERSION.to_le_bytes());
        bytes[HEADER_LEN_AT..HEADER_LEN_AT + 2].copy_from_slice(&HEADER_LEN_FIELD.to_le_bytes());
        bytes[AEAD_ID_AT] = AEAD_ID_XCHACHA20_POLY1305;
        bytes[CHUNK_SIZE_AT..CHUNK_SIZE_AT + 4].copy_from_slice(&chunk_size.to_le_bytes());
        bytes[ARTIFACT_ID_AT..ARTIFACT_ID_AT + 16].copy_from_slice(&artifact_id);
        bytes[DOMAIN_ID_AT..DOMAIN_ID_AT + 16].copy_from_slice(&domain_id);
        bytes[RETENTION_CLASS_AT] = retention_code(retention_class);
        bytes[PERMISSION_LINEAGE_AT..PERMISSION_LINEAGE_AT + 16]
            .copy_from_slice(&permission_lineage_id);
        bytes[BASE_NONCE_AT..BASE_NONCE_AT + BASE_NONCE_BYTES].copy_from_slice(&base_nonce);
        let digest = Sha256::digest(bytes).into();
        Self { bytes, digest }
    }

    /// Returns the AAD for chunk `index` under a not-yet-final header.
    #[must_use]
    pub fn chunk_aad(&self, index: u64, plaintext_len: u32, is_final: bool) -> [u8; 45] {
        let mut aad = [0_u8; 45];
        aad[0..32].copy_from_slice(&self.digest);
        aad[32..40].copy_from_slice(&index.to_le_bytes());
        aad[40..44].copy_from_slice(&plaintext_len.to_le_bytes());
        aad[44] = u8::from(is_final);
        aad
    }

    /// Returns the nonce for chunk `index`.
    #[must_use]
    pub fn chunk_nonce(&self, index: u64) -> [u8; BASE_NONCE_BYTES] {
        let mut nonce = [0_u8; BASE_NONCE_BYTES];
        nonce.copy_from_slice(&self.bytes[BASE_NONCE_AT..BASE_NONCE_AT + BASE_NONCE_BYTES]);
        let counter = index.wrapping_add(1).to_le_bytes();
        for (slot, byte) in nonce[16..24].iter_mut().zip(counter) {
            *slot ^= byte;
        }
        nonce
    }

    /// Returns the base nonce, which is also the header wrap's nonce.
    #[must_use]
    pub fn base_nonce(&self) -> [u8; BASE_NONCE_BYTES] {
        let mut nonce = [0_u8; BASE_NONCE_BYTES];
        nonce.copy_from_slice(&self.bytes[BASE_NONCE_AT..BASE_NONCE_AT + BASE_NONCE_BYTES]);
        nonce
    }

    /// Builds the cleartext header prefix `P` once the stream's locator and
    /// length are known.
    ///
    /// `P` is the first of the header's two write stages and is also the wrap's
    /// AAD. Writing it before `wrapped_dek` is what makes t068 §7's `OB01`
    /// ("before header write") and `OB03` ("before header tag") two distinct,
    /// reachable on-disk states rather than one.
    pub fn wrap_aad(
        &self,
        locator: [u8; 32],
        plaintext_len: u64,
    ) -> Result<[u8; WRAP_AAD_BYTES], ObjectFormatError> {
        let mut prefix = [0_u8; WRAP_AAD_BYTES];
        prefix[..STREAMING_PREFIX_BYTES].copy_from_slice(&self.bytes);
        prefix[LOCATOR_AT..LOCATOR_AT + 32].copy_from_slice(&locator);
        prefix[PLAINTEXT_LEN_AT..PLAINTEXT_LEN_AT + 8]
            .copy_from_slice(&plaintext_len.to_le_bytes());
        let wrapped_dek_len = u16::try_from(WRAPPED_DEK_BYTES)
            .map_err(|_| ObjectFormatError::MalformedHeader("wrapped_dek_len"))?;
        prefix[WRAPPED_DEK_LEN_AT..WRAPPED_DEK_LEN_AT + 2]
            .copy_from_slice(&wrapped_dek_len.to_le_bytes());
        Ok(prefix)
    }

    /// Seals `DEK || plaintext_sha256` under `KEK_d`, producing `wrapped_dek`.
    ///
    /// The trailing 16 bytes of the return value are the Poly1305 tag that
    /// t068 §3.4 calls `header_tag`; there is no separate tag field.
    pub fn seal_wrapped_dek(
        &self,
        domain_kek: &[u8; KEY_BYTES],
        dek: &[u8; KEY_BYTES],
        wrap_aad: &[u8; WRAP_AAD_BYTES],
        plaintext_digest: [u8; 32],
    ) -> Result<[u8; WRAPPED_DEK_BYTES], ObjectFormatError> {
        let mut secret = [0_u8; KEY_BYTES + 32];
        secret[..KEY_BYTES].copy_from_slice(dek);
        secret[KEY_BYTES..].copy_from_slice(&plaintext_digest);

        let cipher = XChaCha20Poly1305::new(domain_kek.into());
        let base_nonce = self.base_nonce();
        let sealed = cipher
            .encrypt(
                XNonce::from_slice(&base_nonce),
                Payload {
                    msg: &secret,
                    aad: wrap_aad.as_slice(),
                },
            )
            .map_err(|_| ObjectFormatError::Aead)?;
        secret.fill(0);
        <[u8; WRAPPED_DEK_BYTES]>::try_from(sealed.as_slice())
            .map_err(|_| ObjectFormatError::MalformedHeader("wrapped_dek"))
    }

    /// Seals the finished header in one step.
    ///
    /// The returned bytes are the complete 208-byte on-disk header. The writer
    /// uses the two stages above instead; this exists for the corpus emitter
    /// and for tests that need the whole header at once.
    pub fn seal_header(
        &self,
        domain_kek: &[u8; KEY_BYTES],
        dek: &[u8; KEY_BYTES],
        locator: [u8; 32],
        plaintext_len: u64,
        plaintext_digest: [u8; 32],
    ) -> Result<[u8; HEADER_BYTES], ObjectFormatError> {
        let prefix = self.wrap_aad(locator, plaintext_len)?;
        let wrapped = self.seal_wrapped_dek(domain_kek, dek, &prefix, plaintext_digest)?;
        let mut header = [0_u8; HEADER_BYTES];
        header[..WRAP_AAD_BYTES].copy_from_slice(&prefix);
        header[WRAPPED_DEK_AT..].copy_from_slice(&wrapped);
        Ok(header)
    }
}

/// The DEK and plaintext digest recovered from a verified header.
pub struct OpenedHeader {
    /// The authenticated header fields.
    pub header: ObjectHeader,
    dek: [u8; KEY_BYTES],
    plaintext_digest: [u8; 32],
}

impl OpenedHeader {
    /// Borrows the per-object data-encryption key for the length of the call.
    ///
    /// The name is deliberate and greppable, matching `academic-crypto`: every
    /// call site is a place a reviewer must confirm the bytes do not escape.
    #[must_use]
    pub const fn expose_dek(&self) -> &[u8; KEY_BYTES] {
        &self.dek
    }

    /// Returns the logical SHA-256 of the object's plaintext.
    ///
    /// t068 §3.4 keeps this digest inside the encrypted metadata, so it is a
    /// borrow rather than a field: it identifies the plaintext and must not
    /// reach a log line or an audit row by accident.
    #[must_use]
    pub const fn plaintext_digest(&self) -> &[u8; 32] {
        &self.plaintext_digest
    }
}

/// Prints no key byte and no plaintext digest.
///
/// The derived implementation would put a live DEK into any log line, panic
/// message, or audit row that formatted a reader, which is exactly what ADR-005
/// forbids.
impl core::fmt::Debug for OpenedHeader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OpenedHeader")
            .field("header", &self.header)
            .field("dek", &"<redacted>")
            .field("plaintext_digest", &"<redacted>")
            .finish()
    }
}

impl Drop for OpenedHeader {
    fn drop(&mut self) {
        self.dek.fill(0);
        self.plaintext_digest.fill(0);
    }
}

/// Parses and authenticates one on-disk header under a domain key.
///
/// This is the whole of the wrong-key, wrong-domain, and header-tamper gate:
/// it runs before any chunk byte is read, and it fails without producing a
/// single byte of plaintext.
pub fn open_header(
    bytes: &[u8],
    domain_kek: &[u8; KEY_BYTES],
) -> Result<OpenedHeader, ObjectFormatError> {
    if bytes.len() < HEADER_BYTES {
        return Err(ObjectFormatError::Truncated);
    }
    if bytes[MAGIC_AT..MAGIC_AT + 4] != OBJECT_MAGIC {
        return Err(ObjectFormatError::BadMagic);
    }
    let format_version = read_u16(bytes, FORMAT_VERSION_AT);
    if format_version != OBJECT_FORMAT_VERSION {
        return Err(ObjectFormatError::UnsupportedFormatVersion(format_version));
    }
    if read_u16(bytes, HEADER_LEN_AT) != HEADER_LEN_FIELD {
        return Err(ObjectFormatError::MalformedHeader("header_len"));
    }
    let aead_id = bytes[AEAD_ID_AT];
    if aead_id != AEAD_ID_XCHACHA20_POLY1305 {
        return Err(ObjectFormatError::UnsupportedAead(aead_id));
    }
    let chunk_size = read_u32(bytes, CHUNK_SIZE_AT);
    if chunk_size == 0 {
        return Err(ObjectFormatError::MalformedHeader("chunk_size"));
    }
    let retention_class = bytes[RETENTION_CLASS_AT];
    if !matches!(
        retention_class,
        RETENTION_EPHEMERAL | RETENTION_COURSE_TERM | RETENTION_USER_MANAGED | RETENTION_LEGAL_HOLD
    ) {
        return Err(ObjectFormatError::MalformedHeader("retention_class"));
    }
    if usize::from(read_u16(bytes, WRAPPED_DEK_LEN_AT)) != WRAPPED_DEK_BYTES {
        return Err(ObjectFormatError::MalformedHeader("wrapped_dek_len"));
    }

    let mut base_nonce = [0_u8; BASE_NONCE_BYTES];
    base_nonce.copy_from_slice(&bytes[BASE_NONCE_AT..BASE_NONCE_AT + BASE_NONCE_BYTES]);

    // A destroyed key slot is checked before the AEAD is attempted, so a
    // shredded object is reported as shredded rather than as a failed tag.
    if is_shredded_header(bytes) {
        return Err(ObjectFormatError::Shredded);
    }

    let cipher = XChaCha20Poly1305::new(domain_kek.into());
    let opened = cipher
        .decrypt(
            XNonce::from_slice(&base_nonce),
            Payload {
                msg: &bytes[WRAPPED_DEK_AT..HEADER_BYTES],
                aad: &bytes[..WRAP_AAD_BYTES],
            },
        )
        .map_err(|_| ObjectFormatError::Aead)?;
    if opened.len() != KEY_BYTES + 32 {
        return Err(ObjectFormatError::MalformedHeader("wrapped_dek"));
    }

    let mut dek = [0_u8; KEY_BYTES];
    dek.copy_from_slice(&opened[..KEY_BYTES]);
    let mut plaintext_digest = [0_u8; 32];
    plaintext_digest.copy_from_slice(&opened[KEY_BYTES..]);

    let mut artifact_id = [0_u8; 16];
    artifact_id.copy_from_slice(&bytes[ARTIFACT_ID_AT..ARTIFACT_ID_AT + 16]);
    let mut domain_id = [0_u8; 16];
    domain_id.copy_from_slice(&bytes[DOMAIN_ID_AT..DOMAIN_ID_AT + 16]);
    let mut permission_lineage_id = [0_u8; 16];
    permission_lineage_id
        .copy_from_slice(&bytes[PERMISSION_LINEAGE_AT..PERMISSION_LINEAGE_AT + 16]);
    let mut locator = [0_u8; 32];
    locator.copy_from_slice(&bytes[LOCATOR_AT..LOCATOR_AT + 32]);
    let streaming_prefix_digest = Sha256::digest(&bytes[..STREAMING_PREFIX_BYTES]).into();

    Ok(OpenedHeader {
        header: ObjectHeader {
            chunk_size,
            artifact_id,
            domain_id,
            retention_class,
            permission_lineage_id,
            base_nonce,
            locator,
            plaintext_len: read_u64(bytes, PLAINTEXT_LEN_AT),
            streaming_prefix_digest,
        },
        dek,
        plaintext_digest,
    })
}

/// Seals one plaintext chunk in place, appending its tag.
///
/// `buffer` holds the chunk plaintext on entry and its ciphertext plus tag on
/// return.
pub fn seal_chunk(
    dek: &[u8; KEY_BYTES],
    prefix: &StreamingPrefix,
    index: u64,
    is_final: bool,
    buffer: &mut Vec<u8>,
) -> Result<(), ObjectFormatError> {
    let plaintext_len =
        u32::try_from(buffer.len()).map_err(|_| ObjectFormatError::MalformedHeader("chunk_len"))?;
    let aad = prefix.chunk_aad(index, plaintext_len, is_final);
    let nonce = prefix.chunk_nonce(index);
    let cipher = XChaCha20Poly1305::new(dek.into());
    let tag = cipher
        .encrypt_in_place_detached(XNonce::from_slice(&nonce), &aad, buffer.as_mut_slice())
        .map_err(|_| ObjectFormatError::Aead)?;
    buffer.extend_from_slice(&tag);
    Ok(())
}

/// Opens one sealed chunk in place, removing its tag.
///
/// `buffer` holds ciphertext plus tag on entry and plaintext on return. The
/// chunk index, its exact plaintext length, and whether it is the final chunk
/// are all AAD, so a reordered, spliced, or truncated object fails here rather
/// than yielding bytes.
pub fn open_chunk(
    dek: &[u8; KEY_BYTES],
    header: &ObjectHeader,
    index: u64,
    buffer: &mut Vec<u8>,
) -> Result<(), ObjectFormatError> {
    if buffer.len() < TAG_BYTES {
        return Err(ObjectFormatError::Truncated);
    }
    let split = buffer.len() - TAG_BYTES;
    let mut tag = [0_u8; TAG_BYTES];
    tag.copy_from_slice(&buffer[split..]);
    buffer.truncate(split);
    let aad = header.chunk_aad(index);
    let nonce = header.chunk_nonce(index);
    let cipher = XChaCha20Poly1305::new(dek.into());
    cipher
        .decrypt_in_place_detached(
            XNonce::from_slice(&nonce),
            &aad,
            buffer.as_mut_slice(),
            (&tag).into(),
        )
        .map_err(|_| {
            buffer.fill(0);
            ObjectFormatError::Aead
        })?;
    Ok(())
}

/// Confirms recovered plaintext against the digest the header commits to.
pub fn require_plaintext_digest(
    expected: &[u8; 32],
    observed: ContentDigest,
) -> Result<(), ObjectFormatError> {
    if observed.as_bytes() == expected {
        Ok(())
    } else {
        Err(ObjectFormatError::PlaintextMismatch)
    }
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    let mut value = [0_u8; 2];
    value.copy_from_slice(&bytes[at..at + 2]);
    u16::from_le_bytes(value)
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    let mut value = [0_u8; 4];
    value.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(value)
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(value)
}

impl From<ObjectFormatError> for VaultError {
    fn from(value: ObjectFormatError) -> Self {
        Self::ObjectFormat(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEK: [u8; KEY_BYTES] = [0x11; KEY_BYTES];
    const DEK: [u8; KEY_BYTES] = [0x22; KEY_BYTES];
    const NONCE: [u8; BASE_NONCE_BYTES] = [0x33; BASE_NONCE_BYTES];

    fn prefix() -> StreamingPrefix {
        StreamingPrefix::new(
            DEFAULT_CHUNK_SIZE,
            [0x44; 16],
            [0x55; 16],
            RetentionClass::UserManaged,
            [0x66; 16],
            NONCE,
        )
    }

    #[test]
    fn an_opened_header_prints_no_key_byte_or_plaintext_digest() {
        let prefix = prefix();
        let Ok(header) = prefix.seal_header(&KEK, &DEK, [0x77; 32], 9, [0x88; 32]) else {
            unreachable!("sealing must succeed");
        };
        let Ok(opened) = open_header(&header, &KEK) else {
            unreachable!("the sealing key must open the header");
        };
        let rendered = format!("{opened:?}");
        assert!(rendered.contains("<redacted>"));
        // The DEK is 0x20..0x3f and the digest is 0x88 repeated; neither may
        // appear in any spelling a formatter could produce.
        assert!(!rendered.contains("32, 33, 34"));
        assert!(!rendered.contains("136, 136"));
        assert!(!rendered.contains("2021222324"));
        assert_eq!(opened.expose_dek(), &DEK);
        assert_eq!(opened.plaintext_digest(), &[0x88; 32]);
    }

    #[test]
    fn the_frozen_offsets_add_up() {
        const {
            assert!(STREAMING_PREFIX_BYTES == 86);
            assert!(WRAP_AAD_BYTES == 128);
            assert!(WRAPPED_DEK_BYTES == 80);
            assert!(HEADER_BYTES == 208);
            // `header_len` counts everything after the eight-byte
            // `"ACOB" | format_version | header_len` prefix, not the whole
            // header. 208 on disk, 200 in the field.
            assert!(HEADER_LEN_FIELD as usize == HEADER_BYTES - 8);
            assert!(BASE_NONCE_AT + BASE_NONCE_BYTES == STREAMING_PREFIX_BYTES);
            assert!(WRAPPED_DEK_AT == WRAP_AAD_BYTES);
        }
    }

    #[test]
    fn the_two_write_stages_compose_into_the_one_step_header() {
        let prefix = prefix();
        let Ok(one_step) = prefix.seal_header(&KEK, &DEK, [0x77; 32], 9, [0x88; 32]) else {
            unreachable!("sealing must succeed");
        };
        let Ok(aad) = prefix.wrap_aad([0x77; 32], 9) else {
            unreachable!("the wrap AAD must build");
        };
        let Ok(wrapped) = prefix.seal_wrapped_dek(&KEK, &DEK, &aad, [0x88; 32]) else {
            unreachable!("the DEK wrap must succeed");
        };
        assert_eq!(one_step[..WRAP_AAD_BYTES], aad);
        assert_eq!(one_step[WRAPPED_DEK_AT..], wrapped);
        // The stage boundary is a real one: a header with `P` written and
        // `wrapped_dek` still zero is not openable, which is the OB03 state.
        let mut half_written = one_step;
        half_written[WRAPPED_DEK_AT..].fill(0);
        assert_eq!(
            open_header(&half_written, &KEK).err(),
            Some(ObjectFormatError::Aead)
        );
        assert_eq!(&half_written[..4], b"ACOB");
    }

    #[test]
    fn the_chunk_nonce_xors_only_the_trailing_eight_bytes() {
        let prefix = prefix();
        let zero = prefix.chunk_nonce(0);
        // index 0 XORs LE64(1), which touches exactly one byte.
        assert_eq!(zero[0..16], NONCE[0..16]);
        assert_eq!(zero[16], NONCE[16] ^ 1);
        assert_eq!(zero[17..24], NONCE[17..24]);

        // index 255 XORs LE64(256), which touches exactly the second byte.
        let far = prefix.chunk_nonce(255);
        assert_eq!(far[0..16], NONCE[0..16]);
        assert_eq!(far[16], NONCE[16]);
        assert_eq!(far[17], NONCE[17] ^ 1);
    }

    #[test]
    fn a_sealed_header_opens_only_under_its_own_domain_key() {
        let prefix = prefix();
        let Ok(header) = prefix.seal_header(&KEK, &DEK, [0x77; 32], 9, [0x88; 32]) else {
            unreachable!("sealing must succeed");
        };
        assert_eq!(header.len(), HEADER_BYTES);

        let Ok(opened) = open_header(&header, &KEK) else {
            unreachable!("the sealing key must open the header");
        };
        assert_eq!(opened.dek, DEK);
        assert_eq!(opened.plaintext_digest, [0x88; 32]);
        assert_eq!(opened.header.plaintext_len(), 9);
        assert_eq!(opened.header.chunk_size(), DEFAULT_CHUNK_SIZE);

        let mut other = KEK;
        other[0] ^= 0xff;
        assert_eq!(
            open_header(&header, &other).err(),
            Some(ObjectFormatError::Aead)
        );
    }

    #[test]
    fn header_field_tampering_fails_the_tag() {
        let prefix = prefix();
        let Ok(header) = prefix.seal_header(&KEK, &DEK, [0x77; 32], 9, [0x88; 32]) else {
            unreachable!("sealing must succeed");
        };
        for at in [
            ARTIFACT_ID_AT,
            DOMAIN_ID_AT,
            RETENTION_CLASS_AT,
            PERMISSION_LINEAGE_AT,
            BASE_NONCE_AT,
            LOCATOR_AT,
            PLAINTEXT_LEN_AT,
        ] {
            let mut tampered = header;
            tampered[at] ^= 0x01;
            assert!(
                matches!(
                    open_header(&tampered, &KEK).err(),
                    Some(ObjectFormatError::Aead | ObjectFormatError::MalformedHeader(_))
                ),
                "tampering at offset {at} was not detected"
            );
        }
    }

    #[test]
    fn chunk_geometry_covers_the_zero_length_and_partial_cases() {
        let build = |plaintext_len: u64, chunk_size: u32| ObjectHeader {
            chunk_size,
            artifact_id: [0; 16],
            domain_id: [0; 16],
            retention_class: RETENTION_USER_MANAGED,
            permission_lineage_id: [0; 16],
            base_nonce: NONCE,
            locator: [0; 32],
            plaintext_len,
            streaming_prefix_digest: [0; 32],
        };

        let empty = build(0, 16);
        assert_eq!(empty.chunk_count(), 1);
        assert_eq!(empty.chunk_plaintext_len(0), 0);
        assert_eq!(empty.chunk_aad(0)[44], 1);
        assert_eq!(empty.sealed_len(), HEADER_BYTES as u64 + TAG_BYTES as u64);

        let exact = build(32, 16);
        assert_eq!(exact.chunk_count(), 2);
        assert_eq!(exact.chunk_plaintext_len(1), 16);
        assert_eq!(exact.chunk_aad(0)[44], 0);
        assert_eq!(exact.chunk_aad(1)[44], 1);

        let ragged = build(33, 16);
        assert_eq!(ragged.chunk_count(), 3);
        assert_eq!(ragged.chunk_plaintext_len(2), 1);
        assert_eq!(ragged.chunk_offset(1), HEADER_BYTES as u64 + 16 + 16);
        assert_eq!(
            ragged.sealed_len(),
            HEADER_BYTES as u64 + 33 + 3 * TAG_BYTES as u64
        );
    }

    #[test]
    fn a_chunk_round_trips_and_rejects_a_moved_index() {
        let prefix = prefix();
        let mut buffer = b"synthetic".to_vec();
        let Ok(()) = seal_chunk(&DEK, &prefix, 0, true, &mut buffer) else {
            unreachable!("sealing must succeed");
        };
        assert_eq!(buffer.len(), 9 + TAG_BYTES);

        let header = ObjectHeader {
            chunk_size: DEFAULT_CHUNK_SIZE,
            artifact_id: [0x44; 16],
            domain_id: [0x55; 16],
            retention_class: RETENTION_USER_MANAGED,
            permission_lineage_id: [0x66; 16],
            base_nonce: NONCE,
            locator: [0; 32],
            plaintext_len: 9,
            streaming_prefix_digest: Sha256::digest(prefix.bytes).into(),
        };
        let mut opened = buffer.clone();
        let Ok(()) = open_chunk(&DEK, &header, 0, &mut opened) else {
            unreachable!("the sealing key must open the chunk");
        };
        assert_eq!(opened, b"synthetic");

        // The same bytes at index 1 authenticate a different AAD and nonce.
        let mut moved = buffer.clone();
        assert_eq!(
            open_chunk(&DEK, &header, 1, &mut moved),
            Err(ObjectFormatError::Aead)
        );
    }
}
