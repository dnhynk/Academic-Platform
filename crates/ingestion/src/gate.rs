//! The two section 38 cells this task leaves open, stated where they bite.
//!
//! Neither has a default and neither is given one here, exactly as
//! `academic-consent`'s `OpenGate` does it for `GATE-38-009` and `GATE-38-019`.
//! What this module supplies is the shape of each cell and the refusal that
//! stands while it is empty.
//!
//! There is deliberately no constant holding a "reasonable" crawl rate, no
//! `Default` for [`crate::terms::TermsStatus`], and no browser-automation
//! module whose existence would answer the second cell by shipping.

use crate::terms::{Fallback, TermsStatus};

/// A section 38 cell this task leaves for the user to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OpenGate {
    /// `GATE-38-020`: the LMS and registration-system terms, the robots
    /// directives, and the rate limits each source permits.
    SourceTermsAndRateLimits,
    /// `GATE-38-027`: where manual export ends and browser-assisted capture
    /// begins.
    ManualExportVersusAssistedCapture,
}

impl OpenGate {
    /// Both cells.
    pub const ALL: [Self; 2] = [
        Self::SourceTermsAndRateLimits,
        Self::ManualExportVersusAssistedCapture,
    ];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::SourceTermsAndRateLimits => "GATE-38-020",
            Self::ManualExportVersusAssistedCapture => "GATE-38-027",
        }
    }

    /// What the cell leaves open, and what stands while it is empty.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::SourceTermsAndRateLimits => {
                "which access methods and which frequency each source permits \
                 is a user and legal decision recorded per source \
                 (GATE-38-020); a connector with no recorded review reads as \
                 UNREVIEWED, which denies, and the fixture-driven tests of this \
                 crate are not blocked by it"
            }
            Self::ManualExportVersusAssistedCapture => {
                "where a user-performed export ends and a browser-assisted \
                 capture begins is undecided (GATE-38-027); Phase 2 ships \
                 manual import and user-provided export and contains no \
                 browser automation module, and the four fallbacks are all \
                 actions a person takes"
            }
        }
    }
}

/// What Phase 2 ships against `GATE-38-027`, as a value rather than a promise.
///
/// The four fallbacks are the whole of it. Each is something a person does; not
/// one of them is a module that drives a browser.
/// `manual_and_export_fallbacks_are_offered_when_denied` compares this against
/// [`Fallback::ALL`], and `no_captcha_or_access_control_bypass_module_exists`
/// is the workspace-level statement that no automation stands behind them.
#[must_use]
pub const fn phase2_shipped_fallbacks() -> [Fallback; 4] {
    Fallback::ALL
}

/// What a connector with no recorded terms review reads as.
///
/// `GATE-38-020` open means no record exists, and this is what an absent record
/// resolves to. It is not a default in the sense of a chosen value: it is the
/// one status that permits nothing.
#[must_use]
pub const fn unreviewed_status() -> TermsStatus {
    TermsStatus::Unreviewed
}
