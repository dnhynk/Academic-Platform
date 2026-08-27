//! Prost-generated local-core schema plus deterministic drift fingerprints.

include!(concat!(env!("OUT_DIR"), "/academic.v1.rs"));

const EXPECTED_SCHEMA_FNV1A64: u64 = 0xaf81_b89f_50b1_09be;
const EXPECTED_CODEGEN_FNV1A64: u64 = 0xeb5c_7f8a_8d13_6489;

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    value
}

/// Checks both the exact Proto source bytes and pinned Prost output bytes.
#[must_use]
pub fn codegen_fingerprints_match() -> bool {
    fnv1a64(include_bytes!(
        "../../../schemas/proto/academic/v1/local_core.proto"
    )) == EXPECTED_SCHEMA_FNV1A64
        && fnv1a64(include_bytes!(concat!(env!("OUT_DIR"), "/academic.v1.rs")))
            == EXPECTED_CODEGEN_FNV1A64
}
