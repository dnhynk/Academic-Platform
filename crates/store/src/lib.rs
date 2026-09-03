//! The two mutually exclusive store lanes and their SQLite boundary.
//!
//! The default Cargo feature fixes bundled plaintext SQLite as the disposable,
//! synthetic-only semantics lane. The non-default SQLCipher spike feature is
//! only an admission point for the E1 evidence harness. The non-default
//! `sqlcipher-store` feature is the encrypted lane: it creates and opens the
//! schema-2 encrypted profile. Enabling any SQLCipher feature is not
//! acceptance of ADR-002 and not permission to process production data.
//!
//! The plaintext and encrypted lanes are mutually exclusive at compile time,
//! so exactly one schema identity, one profile shape, and one SQLite build
//! exist in any binary.

// t068 section 2.3-13 requires a proof that the plaintext synthetic lane and
// the encrypted lane are never linked into the same binary. This is that
// proof's compile-time half: the two features cannot both be enabled, so no
// binary can contain both `profile` and `cipher`, both schema identities, or
// both a plaintext and a SQLCipher `libsqlite3`. The scanned half lives in
// `tests/encrypted_profile.rs` and `tests/sqlcipher_spike.rs`.
#[cfg(all(feature = "bundled-sqlite", feature = "sqlcipher-store"))]
compile_error!(
    "`bundled-sqlite` and `sqlcipher-store` are mutually exclusive store lanes: \
     the plaintext synthetic lane and the encrypted lane must never link into \
     one binary. Build the encrypted lane with \
     `--no-default-features --features sqlcipher-store`."
);

// t068 section 2.3-8: the store<->vault seam is a trait pair, not a concrete
// vault. `academic-store` names only these two traits, so an encrypted vault
// can supply read-back evidence without the store gaining a byte or hash
// bypass, and no second acceptance path exists for one of the two lanes.
pub use academic_vault::{SealedObjectReceipt, SealedObjectVerifier};

pub mod accept;
mod authorizer;
#[cfg(feature = "sqlcipher-store")]
pub mod cipher;
pub mod connection;
pub mod descriptor_migration;
pub mod error;
pub mod fault;
pub mod idempotency;
pub mod migration;
pub mod outbox;
pub mod path_policy;
mod platform;
pub mod profile;
pub mod queries;
pub mod repository;
mod schema_fingerprint;
pub mod timeline;

/// SQLite application identifier (`ACAD`) reserved for the local core store.
///
/// Both lanes are the same application's canonical store, so this identifier
/// does not change with the schema version; the schema version, the format
/// UUID, and the singleton CHECKs are what separate them.
pub const SQLITE_APPLICATION_ID: u32 = 0x4143_4144;

// The schema identity below is selected by lane. The two lanes cannot be
// enabled together, so exactly one of each constant exists in any binary, and
// every shared admission path compares against the right one without needing
// to know which lane it was compiled into.

/// Physical store schema version this binary creates and admits.
#[cfg(not(feature = "sqlcipher-store"))]
pub const STORE_SCHEMA_VERSION: u32 = 1;
/// Physical store schema version this binary creates and admits.
#[cfg(feature = "sqlcipher-store")]
pub const STORE_SCHEMA_VERSION: u32 = 2;

/// Semantic version written by migration 0001.
#[cfg(not(feature = "sqlcipher-store"))]
pub const STORE_SCHEMA_SEMVER: &str = "1.0.0";
/// Semantic version written by migration 0003.
#[cfg(feature = "sqlcipher-store")]
pub const STORE_SCHEMA_SEMVER: &str = "2.0.0";

/// Minimum reader protocol `(major, minor)` recorded in the schema singleton.
#[cfg(not(feature = "sqlcipher-store"))]
pub const STORE_MINIMUM_READER_PROTOCOL: (u32, u32) = (1, 0);
/// Minimum reader protocol `(major, minor)` recorded in the schema singleton.
#[cfg(feature = "sqlcipher-store")]
pub const STORE_MINIMUM_READER_PROTOCOL: (u32, u32) = (2, 0);

/// Minimum writer protocol `(major, minor)` recorded in the schema singleton.
#[cfg(not(feature = "sqlcipher-store"))]
pub const STORE_MINIMUM_WRITER_PROTOCOL: (u32, u32) = (1, 0);
/// Minimum writer protocol `(major, minor)` recorded in the schema singleton.
#[cfg(feature = "sqlcipher-store")]
pub const STORE_MINIMUM_WRITER_PROTOCOL: (u32, u32) = (2, 0);

/// Stable format UUID stored in the schema singleton.
#[cfg(not(feature = "sqlcipher-store"))]
pub const STORE_FORMAT_UUID: [u8; 16] = [
    0x9e, 0x4e, 0xb5, 0x3c, 0xcc, 0xb1, 0x4b, 0x2a, 0x8b, 0xe1, 0x3d, 0x32, 0xdb, 0x16, 0x6e, 0xe4,
];
/// Stable format UUID stored in the schema singleton.
///
/// Frozen by `P2-K2`. Migration 0003 pins the same bytes in a column CHECK, so
/// a singleton carrying the Phase 1 UUID cannot exist in an encrypted profile
/// and a singleton carrying this one cannot exist in a Phase 1 profile.
#[cfg(feature = "sqlcipher-store")]
pub const STORE_FORMAT_UUID: [u8; 16] = [
    0x67, 0xcb, 0x6d, 0x3e, 0xa2, 0x7e, 0x4b, 0x53, 0xb1, 0xe7, 0x27, 0xd4, 0x69, 0x20, 0xe4, 0xf9,
];
/// Reversible Phase 1 busy-timeout default.
pub const SQLITE_BUSY_TIMEOUT_MILLIS: u64 = 250;
/// Default plaintext feature name fixed for product builds.
pub const BUNDLED_SQLITE_FEATURE: &str = "bundled-sqlite";
/// Explicit non-default SQLCipher evidence-spike feature name.
pub const SQLCIPHER_SPIKE_FEATURE: &str = "sqlcipher-spike";
/// Explicit non-default encrypted-store lane feature name.
pub const SQLCIPHER_STORE_FEATURE: &str = "sqlcipher-store";

/// Exact machine-readable posture attached to every future data-bearing surface.
#[cfg(not(feature = "sqlcipher-store"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase1StoragePolicy {
    pub data_policy: &'static str,
    pub storage_mode: &'static str,
    pub storage_encryption: &'static str,
    pub production_data_allowed: bool,
    pub product_network: &'static str,
}

/// Plaintext synthetic-only policy frozen for Phase 1.
#[cfg(not(feature = "sqlcipher-store"))]
pub const PHASE1_STORAGE_POLICY: Phase1StoragePolicy = Phase1StoragePolicy {
    data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
    storage_mode: "PLAINTEXT_TEMPORARY_SQLITE",
    storage_encryption: "NONE",
    production_data_allowed: false,
    product_network: "NONE",
};

/// Unavoidable warning printed before future human-readable data commands.
#[cfg(not(feature = "sqlcipher-store"))]
pub const PHASE1_POLICY_BANNER: &str =
    "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN";
/// Mandatory warning marker created in every synthetic profile.
///
/// Deliberately present in both lanes: t068 section 3.2 makes this marker and
/// [`PROFILE_FORMAT_V2_MARKER`] mutually exclusive and refuses a profile
/// carrying both, so each lane has to recognise the other's marker in order to
/// reject it. Only the *name* crosses the lane boundary; the plaintext posture
/// strings and the code that writes them do not.
pub const SYNTHETIC_PROFILE_MARKER: &str = "SYNTHETIC_ONLY_PLAINTEXT_DO_NOT_USE_REAL_DATA.txt";
/// Marker naming the encrypted profile format and its schema version.
pub const PROFILE_FORMAT_V2_MARKER: &str = "PROFILE_FORMAT_V2";
/// SQLite filename inside a Phase 1 profile.
pub const STORE_DATABASE_FILE: &str = "academic-platform.sqlite3";
/// Bootstrap marker whose presence makes startup fail closed.
pub const INCOMPLETE_PROFILE_MARKER: &str = ".academic-profile-incomplete";

#[cfg(all(test, not(feature = "sqlcipher-store")))]
mod tests {
    use super::*;

    #[test]
    fn default_storage_contract_is_plaintext_synthetic_only() {
        assert_eq!(
            PHASE1_STORAGE_POLICY.storage_mode,
            "PLAINTEXT_TEMPORARY_SQLITE"
        );
        assert_eq!(PHASE1_STORAGE_POLICY.storage_encryption, "NONE");
        const {
            assert!(!PHASE1_STORAGE_POLICY.production_data_allowed);
        }
        assert_eq!(PHASE1_STORAGE_POLICY.product_network, "NONE");
        assert_eq!(SQLITE_APPLICATION_ID.to_be_bytes(), *b"ACAD");
    }
}

#[cfg(test)]
mod aggregate_closure_tests;
#[cfg(test)]
mod aggregate_timeline_tests;
#[cfg(test)]
mod entity_registry_tests;
#[cfg(test)]
mod model_run_closure_tests;
#[cfg(test)]
mod proposal_closure_tests;
