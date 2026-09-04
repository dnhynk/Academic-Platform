//! Why a deletion flow refused.
//!
//! Every arm names something the caller can act on. There is no arm meaning
//! "refused", because a refusal without a reason is the defect
//! `protected_artifact_returns_a_policy_reason` exists to prevent, and an error
//! enum is one of the places it would reappear.

use crate::{protection::ProtectionReason, target::DeletionTarget};

/// Why a deletion flow refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DeletionFlowError {
    /// A policy protects this artifact, and this is which one.
    #[error("{} is protected: {}", .target.to_row(), .reason.to_row())]
    Protected {
        /// The artifact that is protected.
        target: Box<DeletionTarget>,
        /// The policy that protects it, in its own words.
        reason: Box<ProtectionReason>,
    },
    /// The preview does not cover an artifact the dry run reaches.
    ///
    /// The totality guard. A preview whose citation map was short would show a
    /// user fewer affected projections than the deletion actually reaches, and
    /// would look complete while doing it.
    #[error("{} is reached by this deletion and has no evidence citation", .0.to_row())]
    EvidenceCitationMissing(Box<DeletionTarget>),
    /// The digest of the preview the user confirmed is not this preview's.
    #[error("the confirmed preview is not the preview this deletion would run")]
    ConfirmedAnotherPreview,
    /// An automatic actor tried to confirm a deletion.
    #[error("a deletion confirmation was attempted by {actor}, which is not a user")]
    AutomaticActor {
        /// The actor variant that tried.
        actor: &'static str,
    },
    /// A receipt was recorded for a provider erasure nobody requested.
    #[error("a provider erasure receipt names a request this deletion did not make")]
    ReceiptWithoutRequest,
    /// The plan's actions and the dry run's targets stopped being one list.
    ///
    /// `P2-K5`'s `PlannedAction` names a locator, and a locator is shared by
    /// every artifact in one domain that holds the same bytes. The executor
    /// adapter walks the plan and the dry run positionally and compares each
    /// pair; this is what it returns when they disagree, because a failure
    /// attributed to the wrong artifact is the `P1-G1` defect one layer out.
    #[error("the deletion plan is no longer the dry run it was built from")]
    PlanDrifted,
    /// The journal refused a record.
    #[error("the retention journal refused a record: {0}")]
    Journal(String),
}
