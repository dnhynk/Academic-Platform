//! Synthetic-only Phase 1 profile and SQLite boundary.
//!
//! The default Cargo feature fixes bundled plaintext SQLite as the disposable,
//! synthetic-only semantics lane. The non-default SQLCipher feature is only an
//! admission point for a later evidence spike; enabling it is not acceptance of
//! encrypted storage or permission to process production data.

pub mod accept;
mod authorizer;
pub mod connection;
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

use std::{error::Error, fmt};

use academic_domain::{ArtifactDescriptor, ArtifactId, ContentDigest};

/// SQLite application identifier (`ACAD`) reserved for the local core store.
pub const SQLITE_APPLICATION_ID: u32 = 0x4143_4144;
/// First physical store schema version.
pub const STORE_SCHEMA_VERSION: u32 = 1;
/// Semantic version written by migration 0001.
pub const STORE_SCHEMA_SEMVER: &str = "1.0.0";
/// Stable format UUID stored in the schema singleton.
pub const STORE_FORMAT_UUID: [u8; 16] = [
    0x9e, 0x4e, 0xb5, 0x3c, 0xcc, 0xb1, 0x4b, 0x2a, 0x8b, 0xe1, 0x3d, 0x32, 0xdb, 0x16, 0x6e, 0xe4,
];
/// Reversible Phase 1 busy-timeout default.
pub const SQLITE_BUSY_TIMEOUT_MILLIS: u64 = 250;
/// Default plaintext feature name fixed for product builds.
pub const BUNDLED_SQLITE_FEATURE: &str = "bundled-sqlite";
/// Explicit non-default SQLCipher evidence-spike feature name.
pub const SQLCIPHER_SPIKE_FEATURE: &str = "sqlcipher-spike";

/// Exact machine-readable posture attached to every future data-bearing surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase1StoragePolicy {
    pub data_policy: &'static str,
    pub storage_mode: &'static str,
    pub storage_encryption: &'static str,
    pub production_data_allowed: bool,
    pub product_network: &'static str,
}

/// Plaintext synthetic-only policy frozen for Phase 1.
pub const PHASE1_STORAGE_POLICY: Phase1StoragePolicy = Phase1StoragePolicy {
    data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
    storage_mode: "PLAINTEXT_TEMPORARY_SQLITE",
    storage_encryption: "NONE",
    production_data_allowed: false,
    product_network: "NONE",
};

/// Unavoidable warning printed before future human-readable data commands.
pub const PHASE1_POLICY_BANNER: &str =
    "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN";
/// Mandatory warning marker created in every synthetic profile.
pub const SYNTHETIC_PROFILE_MARKER: &str = "SYNTHETIC_ONLY_PLAINTEXT_DO_NOT_USE_REAL_DATA.txt";
/// SQLite filename inside a Phase 1 profile.
pub const STORE_DATABASE_FILE: &str = "academic-platform.sqlite3";
/// Bootstrap marker whose presence makes startup fail closed.
pub const INCOMPLETE_PROFILE_MARKER: &str = ".academic-profile-incomplete";

/// Opaque evidence that an object was sealed and read back before SQL begins.
pub trait SealedObjectReceipt: fmt::Debug + Send + Sync {
    /// Identifies the descriptor whose exact bytes were sealed.
    fn artifact_id(&self) -> ArtifactId;
    /// Returns the exact plaintext digest verified during sealing.
    fn content_digest(&self) -> ContentDigest;
}

/// Capability boundary consumed by the later store acceptance transaction.
///
/// The store depends on this interface and never on the daemon. A vault may
/// implement it for its own private receipt type, while the core composes the
/// two crates without giving SQL a byte/hash bypass.
pub trait SealedObjectVerifier: fmt::Debug + Send + Sync {
    type Receipt: SealedObjectReceipt;
    type Error: Error + Send + Sync + 'static;

    /// Verifies an already sealed object for the exact immutable descriptor.
    fn verify_sealed_object(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<Self::Receipt, Self::Error>;
}

#[cfg(test)]
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
