//! Section 17.4's last sentence: carry a locator into a new snapshot, and keep
//! the original.
//!
//! > file path만 저장하면 이후 line 이동 시 evidence가 깨진다. blob hash, symbol
//! > fingerprint, syntax span과 commit을 함께 저장하고, 새 snapshot에서는
//! > locator migration을 시도하되 원래 evidence를 보존한다.
//!
//! `P2-R2` stored the four. This module does the two verbs: *시도하되* — attempt
//! the migration, and report an attempt that did not land rather than dropping
//! it — and *보존한다* — the original evidence survives the attempt.
//!
//! A [`MigratedFinding`] therefore holds the whole original [`Finding`] by
//! value and never edits it. What migration produces is a list **beside** it,
//! which is the same shape `P2-R3`'s `ImplementationDrift` has and the same
//! reason: `CONTRIBUTING.md` rule 2.
//!
//! ## Why the result is a list and not a map
//!
//! This Run's `P2-A1` audit found a P1 defect of exactly this shape one step
//! away: an artifact's **content** was used as its identity, so deleting two
//! byte-identical artifacts wrote two tombstones under one key and the second
//! silently replaced the first. Two things here would reproduce it.
//!
//! * Keying on the **original** locator. Two locators of one finding can be
//!   equal in every field. `P2-R2`'s own extractor produces such a pair: a
//!   scalar in an infrastructure document is pushed to both the configuration
//!   index and the IaC index at one span, and the ladder reads
//!   `config_tokens().chain(iac_tokens())`, so a subject naming that scalar
//!   gets two sites whose locators are byte-identical.
//! * Keying on the **migrated** symbol. Two sites inside one declaration
//!   migrate to one symbol fingerprint.
//!
//! So [`MigratedFinding::migrations`] is a [`Vec`] with one entry per original
//! locator, in the original's order, each carrying its
//! [`LocatorMigration::ordinal`]. `migrations().len()` equals
//! `finding.locators().len()` for every input, and
//! `finding_locator_migration_preserves_original_evidence` injects both
//! collapsing shapes and observes two records each time.

use academic_repository_analysis::{
    Finding, Locator, RepositoryAnalysis, SourceSpan, SymbolFingerprint, SymbolKind, SymbolRecord,
};

/// Why a locator did not reach the new snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnmatchedReason {
    /// The original locator names no symbol, so there is nothing to follow.
    ///
    /// A manifest row and a configuration key sit outside every declaration;
    /// `P2-R2` records them with no symbol, and a path plus a span is exactly
    /// the pair section 17.4 says breaks when a line moves.
    NoSymbolAnchor,
    /// The path is not in the new snapshot's coverage at all.
    PathRemoved,
    /// The path is there and the symbol is not.
    SymbolGone,
}

impl UnmatchedReason {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::NoSymbolAnchor, Self::PathRemoved, Self::SymbolGone];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoSymbolAnchor => "NO_SYMBOL_ANCHOR",
            Self::PathRemoved => "PATH_REMOVED",
            Self::SymbolGone => "SYMBOL_GONE",
        }
    }
}

/// Where one locator landed in the new snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigratedSite {
    path: String,
    symbol: SymbolFingerprint,
    symbol_kind: SymbolKind,
    span: SourceSpan,
}

impl MigratedSite {
    /// The path in the new snapshot.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The symbol, which is the same fingerprint the original carried.
    #[must_use]
    pub const fn symbol(&self) -> &SymbolFingerprint {
        &self.symbol
    }

    /// What kind of symbol it is.
    #[must_use]
    pub const fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
    }

    /// The span in the new snapshot, which is where the line movement shows.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }
}

/// What the attempt produced for one locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationOutcome {
    /// The symbol was found; here is where it is now.
    Migrated(MigratedSite),
    /// It was not, and this is why.
    Unmatched(UnmatchedReason),
}

impl MigrationOutcome {
    /// Stable spelling of which arm this is.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Migrated(_) => "MIGRATED",
            Self::Unmatched(_) => "UNMATCHED",
        }
    }

    /// The new site, when there is one.
    #[must_use]
    pub const fn site(&self) -> Option<&MigratedSite> {
        match self {
            Self::Migrated(site) => Some(site),
            Self::Unmatched(_) => None,
        }
    }
}

/// One original locator, its position, and what the attempt produced.
///
/// The ordinal is what makes two byte-identical originals two records. See the
/// module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatorMigration {
    ordinal: usize,
    original: Locator,
    outcome: MigrationOutcome,
}

impl LocatorMigration {
    /// Which of the original finding's locators this is, counting from zero.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// The original locator, unchanged.
    #[must_use]
    pub const fn original(&self) -> &Locator {
        &self.original
    }

    /// What the attempt produced.
    #[must_use]
    pub const fn outcome(&self) -> &MigrationOutcome {
        &self.outcome
    }
}

/// One finding carried at a new snapshot, with the original beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigratedFinding {
    original: Finding,
    to_snapshot: String,
    migrations: Vec<LocatorMigration>,
}

impl MigratedFinding {
    /// The finding as it was, at the snapshot it was found in.
    #[must_use]
    pub const fn original(&self) -> &Finding {
        &self.original
    }

    /// The snapshot the original was found in.
    #[must_use]
    pub fn from_snapshot(&self) -> &str {
        self.original.snapshot_id()
    }

    /// The snapshot the locators were carried into.
    #[must_use]
    pub fn to_snapshot(&self) -> &str {
        &self.to_snapshot
    }

    /// One entry per original locator, in the original's order.
    #[must_use]
    pub fn migrations(&self) -> &[LocatorMigration] {
        &self.migrations
    }

    /// How many locators reached the new snapshot.
    #[must_use]
    pub fn migrated_count(&self) -> usize {
        self.migrations
            .iter()
            .filter(|migration| migration.outcome.site().is_some())
            .count()
    }
}

/// Attempts to carry every locator of `finding` into `into`.
///
/// The match is on `P2-R2`'s [`SymbolFingerprint`], which is a digest of path,
/// symbol kind and name and holds no span — so inserting lines before a
/// declaration moves its span and leaves its fingerprint alone, which is what
/// makes the migration possible at all.
///
/// Nothing here is fallible. A locator that does not land is a
/// [`MigrationOutcome::Unmatched`] record rather than an error, because section
/// 17.4's verb is *시도하되*: the attempt is reported, and the original is what
/// the reader still has either way.
#[must_use]
pub fn migrate_locators(finding: &Finding, into: &RepositoryAnalysis) -> MigratedFinding {
    let symbols = into.symbols();
    let migrations = finding
        .locators()
        .iter()
        .enumerate()
        .map(|(ordinal, original)| LocatorMigration {
            ordinal,
            original: original.clone(),
            outcome: follow(original, &symbols, into),
        })
        .collect();
    MigratedFinding {
        original: finding.clone(),
        to_snapshot: into.snapshot_id().to_owned(),
        migrations,
    }
}

/// Follows one locator's symbol into the new analysis.
fn follow(
    original: &Locator,
    symbols: &[SymbolRecord],
    into: &RepositoryAnalysis,
) -> MigrationOutcome {
    let Some(fingerprint) = original.symbol() else {
        return MigrationOutcome::Unmatched(UnmatchedReason::NoSymbolAnchor);
    };
    let found = symbols
        .iter()
        .find(|record| record.fingerprint() == fingerprint);
    match found {
        Some(record) => MigrationOutcome::Migrated(MigratedSite {
            path: record.path().to_owned(),
            symbol: record.fingerprint().clone(),
            symbol_kind: record.kind(),
            span: record.span(),
        }),
        None => {
            let path_present = into
                .coverage()
                .iter()
                .any(|row| row.path() == original.path());
            if path_present {
                MigrationOutcome::Unmatched(UnmatchedReason::SymbolGone)
            } else {
                MigrationOutcome::Unmatched(UnmatchedReason::PathRemoved)
            }
        }
    }
}
