//! The unavoidable Phase 1 policy banner and its machine-readable object.
//!
//! Every command path in this binary emits both. There is deliberately no quiet
//! flag, no environment override, no configuration key, and no debug path that
//! suppresses the banner or changes a single policy field: the values below are
//! compile-time constants copied from the frozen protocol contract, so no
//! runtime input can reach them.

use serde::Serialize;

/// Exact sentence printed before every human-readable result.
pub const PHASE1_POLICY_BANNER: &str = academic_rpc::PHASE1_POLICY_BANNER;

/// The frozen policy object repeated in every machine-readable document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DataPolicy {
    /// Data admission posture.
    pub data_policy: &'static str,
    /// Current temporary store mode.
    pub storage_mode: &'static str,
    /// Explicit lack of at-rest encryption acceptance.
    pub storage_encryption: &'static str,
    /// Real or production data admission. This is always `false`.
    pub production_data_allowed: bool,
    /// Product network posture.
    pub product_network: &'static str,
}

/// The single policy value this binary can report.
pub const PHASE1_DATA_POLICY: DataPolicy = DataPolicy {
    data_policy: academic_rpc::PHASE1_PROTOCOL_POLICY.data_policy,
    storage_mode: academic_rpc::PHASE1_PROTOCOL_POLICY.storage_mode,
    storage_encryption: academic_rpc::PHASE1_PROTOCOL_POLICY.storage_encryption,
    production_data_allowed: academic_rpc::PHASE1_PROTOCOL_POLICY.production_data_allowed,
    product_network: academic_rpc::PHASE1_PROTOCOL_POLICY.product_network,
};

/// Fails the build if the binary could ever claim real data is permitted.
const _: () = assert!(!PHASE1_DATA_POLICY.production_data_allowed);
const _: () = assert!(!academic_rpc::PHASE1_PROTOCOL_POLICY.production_data_allowed);

/// Returns the only policy object this binary can produce.
///
/// The function takes no argument on purpose: there is no caller-supplied input
/// that can select a different posture.
#[must_use]
pub const fn data_policy() -> DataPolicy {
    PHASE1_DATA_POLICY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_matches_the_frozen_protocol_contract() {
        let policy = data_policy();
        assert_eq!(
            policy.data_policy,
            "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED"
        );
        assert_eq!(policy.storage_mode, "PLAINTEXT_TEMPORARY_SQLITE");
        assert_eq!(policy.storage_encryption, "NONE");
        assert!(!policy.production_data_allowed);
        assert_eq!(policy.product_network, "NONE");
    }

    #[test]
    fn policy_agrees_with_the_store_and_protocol_constants() {
        assert_eq!(
            data_policy().data_policy,
            academic_rpc::PHASE1_PROTOCOL_POLICY.data_policy
        );
        assert_eq!(
            PHASE1_POLICY_BANNER,
            "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN"
        );
    }

    // A child-process battery proving no environment variable, flag, or
    // configuration key can move this posture lives in `tests/cli.rs`
    // (`cli_has_no_real_data_override`); the workspace forbids `unsafe_code`,
    // so the in-process variant cannot set variables here.
}
