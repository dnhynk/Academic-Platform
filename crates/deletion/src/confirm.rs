//! The confirmation, and why no automatic actor can produce one.
//!
//! t068 section 5 lists `deletion_confirmation_is_non_delegable`. `P2-M4`, the
//! task that forces non-delegable actions generally, has not merged; this crate
//! closes its own case with a type rather than waiting for it, and the door it
//! uses is `P2-M2`'s so that a second actor check is not written here.
//!
//! # Two things the type says, not the caller
//!
//! 1. **A user decided.** [`academic_proposal::UserDecision::by`] takes an
//!    [`academic_domain::Actor`] and matches exhaustively over that closed
//!    enum, issuing a receipt only for `Actor::User`. A fifth actor variant
//!    stops `academic-proposal` compiling until it says which side it is on, so
//!    the refusal is not a negated list that a new variant slips past.
//! 2. **They decided about *this* deletion.** A `UserDecision` on its own is a
//!    blanket "a user said yes to something". [`DeletionConfirmation`] binds one
//!    to the digest of the exact preview that was shown, the same shape
//!    `academic_proposal::ExplicitApproval` and `academic_domain::ImpactPreview`
//!    use, so a confirmation of one deletion cannot authorise another.
//!
//! There is no `Default`, no public field, and no constructor that takes an
//! `Actor` directly — the only way in is through the receipt.
//! `tests/compile_fail` holds the struct-literal and the automatic-actor cases.

use academic_domain::{Actor, ContentDigest, TimestampMillis};
use academic_proposal::UserDecision;

use crate::{error::DeletionFlowError, preview::DeletionImpactPreview};

/// A deletion one user confirmed, after seeing one preview.
///
/// It owns the preview. A confirmation that referred to a preview by digest
/// alone would let a caller run a different plan under a matching digest by
/// building a second preview; owning it means the plan that runs is the object
/// the digest was taken over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionConfirmation {
    preview: DeletionImpactPreview,
    decision: UserDecision,
    shown_digest: ContentDigest,
    confirmed_at: TimestampMillis,
}

impl DeletionConfirmation {
    /// Records one user's confirmation of one preview.
    ///
    /// `shown` is the digest the surface displayed. It is compared rather than
    /// derived, so a surface that rendered one preview and submitted another
    /// fails here instead of deleting the second.
    ///
    /// # Errors
    ///
    /// [`DeletionFlowError::AutomaticActor`] for a deterministic engine, a
    /// model run, or an importer; [`DeletionFlowError::ConfirmedAnotherPreview`]
    /// when `shown` is not this preview's digest.
    pub fn given(
        preview: DeletionImpactPreview,
        actor: &Actor,
        shown: ContentDigest,
        confirmed_at: TimestampMillis,
    ) -> Result<Self, DeletionFlowError> {
        let decision = UserDecision::by(actor).map_err(|_| DeletionFlowError::AutomaticActor {
            actor: actor.kind_name(),
        })?;
        if shown != preview.digest() {
            return Err(DeletionFlowError::ConfirmedAnotherPreview);
        }
        Ok(Self {
            preview,
            decision,
            shown_digest: shown,
            confirmed_at,
        })
    }

    /// What the user was shown, and what will run.
    #[must_use]
    pub const fn preview(&self) -> &DeletionImpactPreview {
        &self.preview
    }

    /// The receipt proving a user and not an automatic actor decided.
    #[must_use]
    pub const fn decision(&self) -> &UserDecision {
        &self.decision
    }

    /// The digest the surface displayed.
    #[must_use]
    pub const fn shown_digest(&self) -> ContentDigest {
        self.shown_digest
    }

    /// When it was confirmed.
    #[must_use]
    pub const fn confirmed_at(&self) -> TimestampMillis {
        self.confirmed_at
    }
}
