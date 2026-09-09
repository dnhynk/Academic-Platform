//! Pure framing for the new macOS provider; historical provider payloads stay intact.

use crate::{KeystoreError, KeystoreErrorCode, KeystoreLabel};

pub(super) const GENERATION_BYTES: usize = 32;
const VERSION: u8 = 1;
const PREFIX_BYTES: usize = 3;

/// A random, non-secret item generation, matched as `kSecAttrGeneric`.
/// Keeping it separate from the account's primary key makes duplicate labels
/// fail atomically, while stale blobs cannot delete a later replacement item.
pub(super) fn encode(label: &KeystoreLabel, generation: &[u8; GENERATION_BYTES]) -> Vec<u8> {
    let bytes = label.as_str().as_bytes();
    let mut payload = Vec::with_capacity(PREFIX_BYTES + bytes.len() + GENERATION_BYTES);
    payload.push(VERSION);
    payload.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    payload.extend_from_slice(bytes);
    payload.extend_from_slice(generation);
    payload
}

pub(super) fn decode<'a>(
    label: &KeystoreLabel,
    payload: &'a [u8],
    operation: &'static str,
) -> Result<&'a [u8; GENERATION_BYTES], KeystoreError> {
    let invalid = || KeystoreError::new(KeystoreErrorCode::InvalidSealedBlob, operation, None);
    let bytes = label.as_str().as_bytes();
    if payload.len() != PREFIX_BYTES + bytes.len() + GENERATION_BYTES
        || payload[0] != VERSION
        || usize::from(u16::from_le_bytes([payload[1], payload[2]])) != bytes.len()
        || payload[PREFIX_BYTES..PREFIX_BYTES + bytes.len()] != *bytes
    {
        return Err(invalid());
    }
    payload[PREFIX_BYTES + bytes.len()..]
        .try_into()
        .map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_payload_binds_the_label_version_and_exact_generation_length()
    -> Result<(), KeystoreError> {
        let label = KeystoreLabel::new("macos:test:one")?;
        let other = KeystoreLabel::new("macos:test:two")?;
        let generation = [0xA3; GENERATION_BYTES];
        let payload = encode(&label, &generation);
        assert_eq!(decode(&label, &payload, "test")?, &generation);
        assert_eq!(
            decode(&other, &payload, "test").err().map(|e| e.code),
            Some(KeystoreErrorCode::InvalidSealedBlob)
        );
        for length in 0..payload.len() {
            assert!(decode(&label, &payload[..length], "test").is_err());
        }
        let mut trailing = payload.clone();
        trailing.push(0);
        assert!(decode(&label, &trailing, "test").is_err());
        for offset in 0..PREFIX_BYTES + label.as_str().len() {
            let mut changed = payload.clone();
            changed[offset] ^= 0x80;
            assert!(decode(&label, &changed, "test").is_err());
        }
        let max_label = KeystoreLabel::new(&"a".repeat(crate::MAX_LABEL_BYTES))?;
        assert_eq!(
            decode(&max_label, &encode(&max_label, &generation), "test")?,
            &generation
        );
        Ok(())
    }
}
