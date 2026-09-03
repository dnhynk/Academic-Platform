//! The four risk tiers of section 27.4 and the workflow each one requires.
//!
//! Section 27.4 of the authoritative spec is `Human-in-the-loop 강도`, and its
//! four rows are the whole of this module:
//!
//! * low risk -- a public syllabus topic candidate is saved automatically and
//!   marked `AI_INFERRED`;
//! * medium risk -- graph prerequisites, review themes and repository
//!   classification go to a review queue and stay undoable;
//! * high risk -- knowledge-state promotion, private-data egress and official
//!   rule publication need explicit approval;
//! * non-delegable -- question resolution, career and course decisions and
//!   permission attestation are the user's alone.
//!
//! The token spellings below are the execution plan's. The spec states the four
//! rows in Korean prose and names no identifier, so the plan's `LOW_AUTOSAVE`,
//! `MEDIUM_REVIEW`, `HIGH_APPROVAL` and `NON_DELEGABLE` are the spellings and
//! section 27.4 is the authority for what each one means.

use core::fmt;

/// How much of a human a change needs before it is real.
///
/// Closed and ordered from least to most human involvement. The order is the
/// order of section 27.4's four rows and is what [`RiskTier::ALL`] iterates; it
/// is not a comparison anything ranks proposals by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskTier {
    /// Saved without a human, and only ever as `AI_INFERRED`.
    LowAutosave,
    /// Queued for review, with the disposition reversible afterwards.
    MediumReview,
    /// Held until a user approves this exact proposal.
    HighApproval,
    /// The user's own decision. No automatic actor has a path to it.
    NonDelegable,
}

/// What a tier requires before a proposal becomes a record.
///
/// One variant per tier, and the mapping between them is total in both
/// directions: [`RiskTier::workflow`] is an exhaustive `match`, so a fifth tier
/// stops this crate compiling until it names its workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Workflow {
    /// Persist immediately, labelled `AI_INFERRED` and nothing else.
    AutosaveAsAiInferred,
    /// Enter the review queue; the disposition that settles it can be undone.
    QueueAndUndo,
    /// Wait for an explicit approval naming this proposal.
    ExplicitApproval,
    /// Wait for a user decision. Automatic actors are refused.
    UserOnly,
}

impl RiskTier {
    /// Exhaustive order, section 27.4's own.
    pub const ALL: [Self; 4] = [
        Self::LowAutosave,
        Self::MediumReview,
        Self::HighApproval,
        Self::NonDelegable,
    ];

    /// The workflow section 27.4 gives this tier.
    #[must_use]
    pub const fn workflow(self) -> Workflow {
        match self {
            Self::LowAutosave => Workflow::AutosaveAsAiInferred,
            Self::MediumReview => Workflow::QueueAndUndo,
            Self::HighApproval => Workflow::ExplicitApproval,
            Self::NonDelegable => Workflow::UserOnly,
        }
    }

    /// Whether settling a proposal in this tier needs a human at all.
    ///
    /// True for every tier but [`RiskTier::LowAutosave`], which is the one row
    /// section 27.4 lets run without one.
    #[must_use]
    pub const fn needs_a_human(self) -> bool {
        !matches!(self.workflow(), Workflow::AutosaveAsAiInferred)
    }

    /// Stable spelling, the execution plan's.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LowAutosave => "LOW_AUTOSAVE",
            Self::MediumReview => "MEDIUM_REVIEW",
            Self::HighApproval => "HIGH_APPROVAL",
            Self::NonDelegable => "NON_DELEGABLE",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tier| tier.as_str() == value)
    }
}

impl Workflow {
    /// Exhaustive order, matching [`RiskTier::ALL`] row for row.
    pub const ALL: [Self; 4] = [
        Self::AutosaveAsAiInferred,
        Self::QueueAndUndo,
        Self::ExplicitApproval,
        Self::UserOnly,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AutosaveAsAiInferred => "AUTOSAVE_AS_AI_INFERRED",
            Self::QueueAndUndo => "QUEUE_AND_UNDO",
            Self::ExplicitApproval => "EXPLICIT_APPROVAL",
            Self::UserOnly => "USER_ONLY",
        }
    }
}

impl fmt::Display for RiskTier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for Workflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
