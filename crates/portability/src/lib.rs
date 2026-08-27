//! Vendor-neutral format names for later synthetic export, backup, and restore.
//!
//! F0 performs no filesystem I/O and supplies no archive, backup, or restore
//! implementation.

/// Deterministic open-directory export contract name.
pub const PHASE1_EXPORT_FORMAT: &str = "learning-platform-phase1-export-v1";
/// Synthetic-only backup manifest contract name.
pub const PHASE1_BACKUP_FORMAT: &str = "learning-platform-phase1-backup-v1";
/// Restore is allowed only into a new empty profile.
pub const RESTORE_REQUIRES_EMPTY_PROFILE: bool = true;
/// Projections are disposable and excluded from the canonical export by default.
pub const EXPORT_INCLUDES_PROJECTIONS_BY_DEFAULT: bool = false;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portability_contract_preserves_projection_non_authority() {
        const {
            assert!(RESTORE_REQUIRES_EMPTY_PROFILE);
            assert!(!EXPORT_INCLUDES_PROJECTIONS_BY_DEFAULT);
        }
    }
}
