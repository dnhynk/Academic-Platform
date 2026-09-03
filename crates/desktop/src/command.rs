//! The typed local-core command allowlist.
//!
//! The desktop asks the core for things by naming a [`DesktopCommand`], and
//! there is no other constructor: no `TryFrom<&str>`, no `FromStr`, no variant
//! carrying a free-form capability identifier. What the surface can ask for is
//! therefore the set of variants below, and `tests/command_allowlist.rs`
//! compares that set against `academic_rpc`'s negotiated capability tables in
//! both directions.
//!
//! The comparison is against the wire contract rather than against a second
//! copy of it. A capability list restated here would drift from the daemon's
//! silently; a list compared against the daemon's cannot.

use academic_rpc::generated::{
    SyntheticBackupCommand, SyntheticIngestCommand, SyntheticRestoreCommand, mutable_request,
};

/// The synthetic fixtures the desktop can name.
///
/// A closed enum rather than a string, so the surface cannot ask the core to
/// ingest a path, a URL, or anything a user typed. The string this resolves to
/// is compared against `academic-core`'s own `PHASE1_SYNTHETIC_FIXTURE_ID` by
/// `desktop_names_only_the_core_fixture_allowlist` in
/// `tools/phase1-scaffold-policy.test.mjs`; the comparison is a source scan
/// because `academic-core` reaches the canonical writer and this crate must not
/// have an edge to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyntheticFixtureId {
    /// The Phase 1 bitemporal ledger fixture.
    Phase1BitemporalLedgerV2,
}

impl SyntheticFixtureId {
    /// The identifier the local-core command carries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Phase1BitemporalLedgerV2 => "phase0-synthetic-bitemporal-ledger-v2",
        }
    }

    /// Every fixture the desktop can name.
    pub const ALL: &'static [Self] = &[Self::Phase1BitemporalLedgerV2];
}

/// Everything the desktop may ask the local core to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopCommand {
    /// Read the daemon's own health and posture.
    Diagnostics,
    /// Export the profile through the core.
    SyntheticExport,
    /// Ingest one allowlisted synthetic fixture.
    SyntheticIngest(SyntheticFixtureId),
    /// Take a backup through the core.
    SyntheticBackup,
    /// Restore the backup a receipt names.
    SyntheticRestore {
        /// The sixteen opaque bytes of the backup receipt.
        backup_receipt_id: [u8; 16],
    },
}

impl DesktopCommand {
    /// One representative of every variant, for enumeration.
    pub const ALL: &'static [Self] = &[
        Self::Diagnostics,
        Self::SyntheticExport,
        Self::SyntheticIngest(SyntheticFixtureId::Phase1BitemporalLedgerV2),
        Self::SyntheticBackup,
        Self::SyntheticRestore {
            backup_receipt_id: [0_u8; 16],
        },
    ];

    /// The one capability identifier this command is issued under.
    #[must_use]
    pub const fn capability_id(self) -> &'static str {
        match self {
            Self::Diagnostics => "learning-platform.local.diagnostics.v1",
            Self::SyntheticExport => "learning-platform.local.synthetic-export.v1",
            Self::SyntheticIngest(_) => "learning-platform.local.synthetic-ingest.v1",
            Self::SyntheticBackup => "learning-platform.local.synthetic-backup.v1",
            Self::SyntheticRestore { .. } => "learning-platform.local.synthetic-restore.v1",
        }
    }

    /// The wire command this becomes, or `None` when it mutates nothing.
    ///
    /// The `None` arms are the read-only capabilities. They carry no arm of the
    /// closed `MutableRequest` oneof, which is what keeps a read from being
    /// submitted as a write.
    #[must_use]
    pub fn mutable_command(self) -> Option<mutable_request::Command> {
        match self {
            Self::Diagnostics | Self::SyntheticExport => None,
            Self::SyntheticIngest(fixture) => Some(mutable_request::Command::SyntheticIngest(
                SyntheticIngestCommand {
                    synthetic_fixture_id: fixture.as_str().to_owned(),
                },
            )),
            Self::SyntheticBackup => Some(mutable_request::Command::SyntheticBackup(
                SyntheticBackupCommand {},
            )),
            Self::SyntheticRestore { backup_receipt_id } => Some(
                mutable_request::Command::SyntheticRestore(SyntheticRestoreCommand {
                    backup_receipt_id: backup_receipt_id.to_vec(),
                }),
            ),
        }
    }

    /// Whether this command can change accepted state.
    #[must_use]
    pub const fn is_write(self) -> bool {
        match self {
            Self::Diagnostics | Self::SyntheticExport => false,
            Self::SyntheticIngest(_) | Self::SyntheticBackup | Self::SyntheticRestore { .. } => {
                true
            }
        }
    }
}

/// The capability identifiers the allowlist yields, sorted and deduplicated.
#[must_use]
pub fn capability_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = DesktopCommand::ALL
        .iter()
        .map(|command| command.capability_id())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}
