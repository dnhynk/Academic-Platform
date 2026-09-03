//! What the center refuses, and with what.
//!
//! Every variant carries identifiers and closed enums and nothing else. There
//! is no `String` field and no `&str` field anywhere in this enum, which is not
//! a style preference: an error message is the easiest place for a fragment of
//! a transcript, a document or a payload to reach a log, and
//! `the_center_cannot_name_a_payload_byte` compares the whole set of declared
//! field types in this crate rather than a list of suspicious field names.

use academic_domain::{ClaimId, TimestampMillis};
use academic_proposal::{ProposalId, WorkflowError};
use thiserror::Error;

use crate::{ConflictClass, PermissionRef, ProposalClass};

/// Every refusal this crate makes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CenterError {
    /// A proposal with this identity is already in the inbox.
    #[error("proposal {proposal} is already in the inbox")]
    ProposalAlreadyAdmitted {
        /// Which proposal.
        proposal: ProposalId,
    },
    /// A conflict decision was filed by an actor that is not the user.
    ///
    /// The judgement is `academic-proposal`'s: `UserDecision::by` is issued
    /// only for `Actor::User`, and this arm carries its refusal rather than
    /// repeating the actor match.
    #[error("a conflict is settled by the user alone")]
    NotTheUser {
        /// `P2-M2`'s own refusal.
        refusal: WorkflowError,
    },
    /// A correction record names a conflict that is not open.
    #[error("no open conflict of class {class:?} holds claim {claim}")]
    NoSuchConflict {
        /// Which class was addressed.
        class: ConflictClass,
        /// Which claim the record named.
        claim: ClaimId,
    },
    /// A dependent action was gated against a permission that has expired.
    #[error("permission {permission:?} expired at {expires_at:?}")]
    PermissionExpired {
        /// Which permission.
        permission: PermissionRef,
        /// When it lapsed.
        expires_at: TimestampMillis,
    },
    /// A dependent action requires a permission the queue does not hold.
    #[error("permission {permission:?} is not in the queue")]
    PermissionAbsent {
        /// Which permission was required.
        permission: PermissionRef,
    },
    /// An inbox lookup named a class no entry carries.
    #[error("the inbox holds no entry of class {class:?}")]
    NoEntryOfClass {
        /// Which class.
        class: ProposalClass,
    },
}
