//! Order-stable checksums for canonical projection records.

use academic_domain::ContentDigest;

/// Hashes a set of canonical records independently of input iteration order.
///
/// Records are sorted bytewise, length-delimited with unsigned big-endian
/// lengths, and then hashed. The length prefix prevents concatenation
/// ambiguity while preserving byte-for-byte portability across platforms.
#[must_use]
pub fn order_stable_checksum<I>(records: I) -> ContentDigest
where
    I: IntoIterator<Item = Vec<u8>>,
{
    let mut records: Vec<Vec<u8>> = records.into_iter().collect();
    records.sort();
    let total_bytes = records.iter().fold(0_usize, |total, record| {
        total.saturating_add(record.len()).saturating_add(8)
    });
    let mut canonical = Vec::with_capacity(total_bytes);
    for record in records {
        let length = u64::try_from(record.len()).unwrap_or(u64::MAX);
        canonical.extend_from_slice(&length.to_be_bytes());
        canonical.extend_from_slice(&record);
    }
    ContentDigest::sha256(&canonical)
}

/// Appends one byte field with an unambiguous unsigned big-endian length.
pub(crate) fn append_field(target: &mut Vec<u8>, field: &[u8]) {
    let length = u64::try_from(field.len()).unwrap_or(u64::MAX);
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(field);
}

/// Appends an optional byte field without conflating absent and empty values.
pub(crate) fn append_optional_field(target: &mut Vec<u8>, field: Option<&[u8]>) {
    match field {
        Some(field) => {
            target.push(1);
            append_field(target, field);
        }
        None => target.push(0),
    }
}
