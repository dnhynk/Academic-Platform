//! The section 38 cell this task leaves open, and where it bites.
//!
//! `GATE-38-021` — per-source access, storage, analysis and retention rights —
//! is a user and legal decision. There is no default here and nothing guesses
//! one: [`crate::access::SourceTermsLedger`] starts empty, a source and mode
//! nobody recorded reads as `academic_ingestion::TermsStatus::Unreviewed`, and
//! [`crate::access::permit`] refuses it with the whole of
//! `academic_ingestion::Fallback::ALL`.
//!
//! An unconfigured source therefore keeps its connector disabled by having no
//! record rather than by holding a switch somebody could flip. The
//! fixture-driven tests in this crate are not blocked by the gate — they record
//! their own synthetic decisions — and no live connector runs behind it,
//! because this crate has no transport to run one with.

use core::fmt;

/// The one section 38 cell this task leaves open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OpenGate {
    /// `GATE-38-021`: per-source access, storage, analysis and retention
    /// rights.
    PerSourceRights,
}

impl OpenGate {
    /// Exhaustive listing.
    pub const ALL: [Self; 1] = [Self::PerSourceRights];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::PerSourceRights => "GATE-38-021",
        }
    }

    /// What stays undecided.
    #[must_use]
    pub const fn question(self) -> &'static str {
        match self {
            Self::PerSourceRights => {
                "which access, storage, analysis and retention rights each review source grants"
            }
        }
    }

    /// What this crate does while it is open.
    #[must_use]
    pub const fn while_open(self) -> &'static str {
        match self {
            Self::PerSourceRights => {
                "an unrecorded source and mode reads as UNREVIEWED, which permits no collection"
            }
        }
    }
}

impl fmt::Display for OpenGate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.identifier(), self.question())
    }
}
