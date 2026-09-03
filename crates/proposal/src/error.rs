//! Every refusal this boundary can produce.

use academic_domain::DecisionAction;

use crate::{
    disposition::disposition_token,
    proposed::ProposalId,
    tier::{RiskTier, Workflow},
};

/// A workflow rule the caller did not satisfy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowError {
    /// The queue holds no entry under that identifier.
    #[error("{0} is not in the queue")]
    NoSuchProposal(ProposalId),
    /// Two entries were admitted under one identifier.
    #[error("{0} is already in the queue")]
    DuplicateProposal(ProposalId),
    /// The tier's workflow is not the one this entry point serves.
    ///
    /// This is the whole of the tier-to-workflow mapping, expressed as a
    /// refusal: each of the four entry points names the workflow it serves, and
    /// a proposal whose tier maps to a different one gets this.
    #[error("{tier} requires {required} and this is {attempted}")]
    WrongWorkflow {
        /// The entry's tier.
        tier: RiskTier,
        /// The workflow section 27.4 gives that tier.
        required: Workflow,
        /// The workflow the caller used.
        attempted: Workflow,
    },
    /// A deterministic engine, model run, or importer tried to act as a user.
    #[error("{actor} is an automatic actor and cannot make a user decision")]
    AutomaticActor {
        /// The actor variant name, from `academic-domain`.
        actor: &'static str,
    },
    /// An explicit approval named a proposal other than the one being approved.
    #[error("the approval names {named} and not {target}")]
    ApprovalNamesAnotherProposal {
        /// The proposal the approval carries.
        named: ProposalId,
        /// The proposal the caller tried to settle.
        target: ProposalId,
    },
    /// The payload has already left the queue.
    #[error("{0} has already been committed")]
    AlreadyCommitted(ProposalId),
    /// A commit ran against an entry no user has confirmed.
    ///
    /// `Replace` reaches this too, and deliberately: ADR-003 has a replacement
    /// reject the target and select a different object, so the proposal's own
    /// payload is not what becomes the record and this queue does not hand it
    /// out as though it were.
    #[error("{proposal} carries {current} and a commit needs CONFIRM")]
    NotConfirmed {
        /// The entry.
        proposal: ProposalId,
        /// What the entry's current disposition is.
        current: DispositionState,
    },
    /// An undo ran against an entry with nothing to undo.
    #[error("{0} has no disposition to undo")]
    NothingToUndo(ProposalId),
}

/// What an entry's history currently says.
///
/// [`DispositionState::Undisposed`] is the queue's word for pending. It is a
/// presentation state and not a decision: it is not a [`DecisionAction`], it
/// has no conversion into one, and nothing that ranks user authority can be
/// handed it. Making "not decided yet" into a decision would give it a place in
/// ADR-003's authority computation, where it would read as though the user had
/// judged. `pending_is_not_a_disposition` in `tests/compile_fail` is that as a
/// compile error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispositionState {
    /// No disposition has been recorded, or the last one was undone.
    Undisposed,
    /// The most recent record that has not been undone says this.
    Recorded(DecisionAction),
}

impl core::fmt::Display for DispositionState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Undisposed => formatter.write_str("no disposition"),
            Self::Recorded(action) => formatter.write_str(disposition_token(action)),
        }
    }
}

/// A batching configuration that does not describe a partition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThresholdError {
    /// A cut point was outside 1..=1000.
    #[error("{axis} cut {value} is outside 1..=1000")]
    CutOutOfRange {
        /// Which axis the cut belongs to.
        axis: &'static str,
        /// The offending value.
        value: u16,
    },
    /// Two cuts were equal or out of order.
    ///
    /// Cuts have to increase strictly, because a repeated cut makes an empty
    /// band and an out-of-order pair makes the band index depend on scan order
    /// rather than on the value.
    #[error("{axis} cuts must increase strictly; {previous} is followed by {value}")]
    CutsNotIncreasing {
        /// Which axis the cuts belong to.
        axis: &'static str,
        /// The earlier cut.
        previous: u16,
        /// The cut that did not exceed it.
        value: u16,
    },
    /// An axis had no cut at all.
    ///
    /// A single band is a batching configuration that batches nothing, which
    /// is a configuration mistake rather than a policy: section 29.7 batches on
    /// confidence *and* impact, so both axes have to divide.
    #[error("{axis} has no cut, so it has one band and divides nothing")]
    NoCut {
        /// Which axis is empty.
        axis: &'static str,
    },
}
