//! Synthetic-only artifact-vault format names and policy boundaries.
//!
//! No object writer, encryption, key hierarchy, garbage collection, or
//! filesystem mutation exists in F0.

/// Disposable plaintext object format used only by synthetic Phase 1 work.
pub const VAULT_WRITE_FORMAT: &str = "PLAINTEXT_SYNTHETIC_V1";
/// Oldest readable object format during Phase 1.
pub const VAULT_MIN_READ_FORMAT: &str = "PLAINTEXT_SYNTHETIC_V1";
/// Version component used in the future keyed-locator input.
pub const VAULT_FORMAT_VERSION: u16 = 1;

/// Describes the deliberately non-production vault contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultFormatContract {
    pub read_format: &'static str,
    pub write_format: &'static str,
    pub encrypted: bool,
    pub production_data_allowed: bool,
}

/// Exact F0 vault posture.
pub const PHASE1_VAULT_FORMAT: VaultFormatContract = VaultFormatContract {
    read_format: VAULT_MIN_READ_FORMAT,
    write_format: VAULT_WRITE_FORMAT,
    encrypted: false,
    production_data_allowed: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_vault_format_never_claims_acceptance() {
        const {
            assert!(!PHASE1_VAULT_FORMAT.encrypted);
            assert!(!PHASE1_VAULT_FORMAT.production_data_allowed);
        }
        assert!(VAULT_WRITE_FORMAT.contains("PLAINTEXT_SYNTHETIC"));
    }
}
