//! Receipt-derived posture shared by every CLI output path.

use std::path::Path;

pub use academic_admission::Posture as DataPolicy;

/// Returns the unchanged synthetic posture for commands with no profile.
#[cfg(test)]
#[must_use]
pub const fn data_policy() -> DataPolicy {
    DataPolicy::synthetic()
}

/// Resolves a profile's receipt once for the command output envelope.
#[must_use]
pub fn posture_for_profile(profile_root: Option<&Path>) -> DataPolicy {
    profile_root.map_or_else(
        DataPolicy::synthetic,
        academic_admission::AdmissionVerifier::posture,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_matches_the_frozen_protocol_contract() {
        let policy = data_policy();
        assert_eq!(
            policy.data_policy(),
            "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED"
        );
        assert_eq!(policy.storage_mode(), "PLAINTEXT_TEMPORARY_SQLITE");
        assert_eq!(policy.storage_encryption(), "NONE");
        assert!(!policy.production_data_allowed());
        assert_eq!(policy.product_network(), "NONE");
        assert_eq!(
            policy.banner(),
            "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN"
        );
    }
}
