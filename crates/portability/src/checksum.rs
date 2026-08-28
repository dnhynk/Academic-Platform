//! Deterministic, domain-separated digests for portable synthetic artifacts.
//!
//! Every digest input is length-delimited with an unsigned big-endian 64-bit
//! prefix so no concatenation of two different field sequences can collide.
//! Nothing in this module observes locale, filesystem metadata, or wall-clock
//! time, so the same canonical input produces the same digest on Windows and
//! Linux.

use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

use academic_domain::ContentDigest;
use sha2::{Digest, Sha256};

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const FILE_HASH_CHUNK_BYTES: usize = 64 * 1024;

/// Encodes bytes as lowercase hexadecimal.
#[must_use]
pub fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

/// Decodes strict lowercase hexadecimal of an even length.
///
/// Uppercase, whitespace, prefixes, and odd lengths fail closed so a manifest
/// cannot carry two spellings of the same digest.
#[must_use]
pub fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in bytes.as_chunks::<2>().0 {
        let high = decode_hex_digit(pair[0])?;
        let low = decode_hex_digit(pair[1])?;
        output.push((high << 4) | low);
    }
    Some(output)
}

const fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Length-delimited canonical digest accumulator.
#[derive(Debug)]
pub struct CanonicalDigest {
    hasher: Sha256,
}

impl CanonicalDigest {
    /// Starts a digest bound to one explicit domain separator.
    #[must_use]
    pub fn new(domain_separator: &str) -> Self {
        let mut digest = Self {
            hasher: Sha256::new(),
        };
        digest.field(domain_separator.as_bytes());
        digest
    }

    /// Appends one length-delimited byte field.
    pub fn field(&mut self, bytes: &[u8]) -> &mut Self {
        let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        self.hasher.update(length.to_be_bytes());
        self.hasher.update(bytes);
        self
    }

    /// Appends one length-delimited UTF-8 text field.
    pub fn text(&mut self, value: &str) -> &mut Self {
        self.field(value.as_bytes())
    }

    /// Appends an unsigned integer in fixed big-endian form.
    pub fn unsigned(&mut self, value: u64) -> &mut Self {
        self.field(&value.to_be_bytes())
    }

    /// Appends a signed integer in fixed big-endian form.
    pub fn signed(&mut self, value: i64) -> &mut Self {
        self.field(&value.to_be_bytes())
    }

    /// Appends a boolean without conflating it with an integer field.
    pub fn boolean(&mut self, value: bool) -> &mut Self {
        self.field(if value { b"true" } else { b"false" })
    }

    /// Appends an optional field without conflating absent with empty.
    pub fn optional(&mut self, value: Option<&[u8]>) -> &mut Self {
        match value {
            Some(bytes) => {
                self.hasher.update([1_u8]);
                self.field(bytes)
            }
            None => {
                self.hasher.update([0_u8]);
                self
            }
        }
    }

    /// Finishes the digest.
    #[must_use]
    pub fn finish(self) -> ContentDigest {
        ContentDigest::from_sha256_bytes(self.hasher.finalize().into())
    }
}

/// Streams a file and returns its exact SHA-256 digest and byte length.
///
/// Filesystem metadata is deliberately not part of the result.
pub fn hash_file(path: &Path) -> io::Result<(ContentDigest, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; FILE_HASH_CHUNK_BYTES];
    let mut length = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        length = length.saturating_add(read as u64);
    }
    Ok((
        ContentDigest::from_sha256_bytes(hasher.finalize().into()),
        length,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_is_exact_and_strict() {
        let bytes = [0x00, 0x1f, 0xa5, 0xff];
        assert_eq!(encode_hex(&bytes), "001fa5ff");
        assert_eq!(decode_hex("001fa5ff"), Some(bytes.to_vec()));
        assert_eq!(decode_hex("001FA5FF"), None);
        assert_eq!(decode_hex("001fa5f"), None);
        assert_eq!(decode_hex("0x1f"), None);
    }

    #[test]
    fn length_delimiting_prevents_field_boundary_collisions() {
        let mut left = CanonicalDigest::new("test");
        left.text("ab").text("c");
        let mut right = CanonicalDigest::new("test");
        right.text("a").text("bc");
        assert_ne!(left.finish(), right.finish());
    }

    #[test]
    fn absent_and_empty_optional_fields_differ() {
        let mut absent = CanonicalDigest::new("test");
        absent.optional(None);
        let mut empty = CanonicalDigest::new("test");
        empty.optional(Some(b""));
        assert_ne!(absent.finish(), empty.finish());
    }
}
