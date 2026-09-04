//! The explicit decision event every non-delegable action requires.
//!
//! # Two things the type says, not the caller
//!
//! 1. **A user decided.** [`academic_proposal::UserDecision::by`] takes an
//!    [`Actor`] and matches exhaustively over that closed enum, issuing a
//!    receipt only for `Actor::User`. A fifth actor variant stops
//!    `academic-proposal` compiling until it says which side it is on, so the
//!    refusal is not a negated list a new variant slips past. `P2-P2` used the
//!    same door for its deletion confirmation, which is why this crate's
//!    constant and that crate's contract are one fact rather than two checks
//!    that could drift.
//! 2. **They decided about *this* action and *this* subject.** A
//!    [`academic_proposal::UserDecision`] on its own is a blanket "a user said
//!    yes to something". [`DecisionEvent`] binds one to a
//!    [`NonDelegableAction`] and to the digest of the exact subject that was
//!    shown, so a resolution of one question cannot close another and a
//!    confirmation of one deletion cannot authorise an egress.
//!
//! There is no `Default`, no public field, no setter, and no constructor that
//! takes an [`Actor`] and skips the receipt. `tests/compile_fail` holds the
//! struct-literal case and the automatic-actor case.

use academic_domain::{Actor, ContentDigest, TimestampMillis};
use academic_proposal::UserDecision;

use crate::{action::NonDelegableAction, error::NonDelegableError};

/// One user's explicit decision about one non-delegable action.
#[derive(Debug)]
pub struct DecisionEvent {
    action: NonDelegableAction,
    decision: UserDecision,
    subject: ContentDigest,
    decided_at: TimestampMillis,
}

impl DecisionEvent {
    /// Records one user's decision about one subject.
    ///
    /// # Errors
    ///
    /// [`NonDelegableError::AutomaticActor`] for a deterministic engine, a
    /// model run or an importer.
    pub fn recorded(
        action: NonDelegableAction,
        actor: &Actor,
        subject: ContentDigest,
        decided_at: TimestampMillis,
    ) -> Result<Self, NonDelegableError> {
        let decision = UserDecision::by(actor).map_err(|_| NonDelegableError::AutomaticActor {
            action,
            actor: actor.kind_name(),
        })?;
        Ok(Self {
            action,
            decision,
            subject,
            decided_at,
        })
    }

    /// The action this event settles.
    #[must_use]
    pub const fn action(&self) -> NonDelegableAction {
        self.action
    }

    /// The receipt proving a user and not an automatic actor decided.
    #[must_use]
    pub const fn decision(&self) -> &UserDecision {
        &self.decision
    }

    /// The digest of the subject the user was shown.
    #[must_use]
    pub const fn subject(&self) -> ContentDigest {
        self.subject
    }

    /// When it was decided.
    #[must_use]
    pub const fn decided_at(&self) -> TimestampMillis {
        self.decided_at
    }

    /// Whether this event authorises exactly this action over exactly this
    /// subject.
    ///
    /// One comparison in one place. A caller that wrote its own would be free
    /// to compare the action and forget the subject, which is the shape
    /// `P2-P1` measured passing while touching no byte it meant to check.
    ///
    /// # Errors
    ///
    /// [`NonDelegableError::DecisionNamesAnotherAction`] or
    /// [`NonDelegableError::DecisionNamesAnotherSubject`].
    pub fn authorises(
        &self,
        action: NonDelegableAction,
        subject: ContentDigest,
    ) -> Result<(), NonDelegableError> {
        if self.action != action {
            return Err(NonDelegableError::DecisionNamesAnotherAction {
                recorded: self.action,
                offered: action,
            });
        }
        if self.subject != subject {
            return Err(NonDelegableError::DecisionNamesAnotherSubject { action });
        }
        Ok(())
    }
}
