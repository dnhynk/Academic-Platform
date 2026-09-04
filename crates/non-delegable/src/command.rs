//! The command layer: one total function from a submitted command to either an
//! automatic actor's proposal or one user's decision.
//!
//! # Where this is enforced, and where it is not
//!
//! The execution plan says *enforced in the daemon command layer, not only in
//! the UI*. Two facts about this repository decide what that can mean today:
//!
//! * The daemon's own command surface is Phase 1 and synthetic.
//!   `academic_rpc::ValidatedWriteCommand` has three arms — synthetic ingest,
//!   backup and restore — and **no arm carries an actor at all**. There is no
//!   product command in the wire protocol for any of the six actions, so there
//!   is no arm for a refusal to live inside.
//! * The place the six actions are actually performed is the crates that own
//!   them, and they are libraries.
//!
//! So this crate is the layer, and [`authorise`] is the door a command layer
//! calls before it dispatches. What that is **not** evidence for is stated in
//! the same breath: a caller that reaches
//! `academic_record::RegistrationConfirmation::new` or
//! `academic_consent::AuthorityGrant::record` directly bypasses this door,
//! because neither takes an actor. `ai_cannot_decide_enrollment_or_career` and
//! `ai_cannot_attest_permission` drive both for real and observe exactly that,
//! which is why the refusal has to be here and why the contract page says a
//! surface that skips this door skips the refusal.
//!
//! # The two guards do not mask each other
//!
//! [`authorise`] dispatches on [`crate::Delegability`] and
//! [`crate::DecisionEvent::recorded`] refuses an automatic actor. Deleting
//! either leaves the other passing something it must not:
//!
//! * delete the dispatch and a **user's** non-delegable command comes back as
//!   an [`AuthorizedCommand::Proposal`], which is an AI candidate;
//! * delete the actor check and an **automatic** actor's non-delegable command
//!   comes back as an [`AuthorizedCommand::Decision`].
//!
//! `a_user_command_is_a_decision_not_a_proposal` holds the first and the six
//! acceptance tests hold the second.

use academic_domain::{Actor, ContentDigest, TimestampMillis};

use crate::{
    action::{Action, CandidateGeneration, Delegability},
    decision::DecisionEvent,
    error::NonDelegableError,
};

/// One command as it arrives at the layer.
///
/// The actor is a required field with no default. A command that did not say
/// who submitted it is not a value that exists, which is the difference between
/// this and a check a caller can forget to run.
#[derive(Debug, Clone)]
pub struct ActionCommand {
    action: Action,
    actor: Actor,
    subject: ContentDigest,
    submitted_at: TimestampMillis,
}

impl ActionCommand {
    /// Wraps a submitted command.
    #[must_use]
    pub const fn submitted(
        action: Action,
        actor: Actor,
        subject: ContentDigest,
        submitted_at: TimestampMillis,
    ) -> Self {
        Self {
            action,
            actor,
            subject,
            submitted_at,
        }
    }

    /// The action requested.
    #[must_use]
    pub const fn action(&self) -> Action {
        self.action
    }

    /// Who submitted it.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The digest of what it is about.
    #[must_use]
    pub const fn subject(&self) -> ContentDigest {
        self.subject
    }

    /// When it arrived.
    #[must_use]
    pub const fn submitted_at(&self) -> TimestampMillis {
        self.submitted_at
    }
}

/// An automatic actor's candidate for one of section 27.1's ten rows.
///
/// Private fields and no public constructor: the only way one exists is through
/// [`authorise`], so a caller cannot manufacture the conclusion that a
/// generation was authorised.
#[derive(Debug, Clone)]
pub struct AuthorizedProposal {
    generation: CandidateGeneration,
    actor: Actor,
    subject: ContentDigest,
    submitted_at: TimestampMillis,
}

impl AuthorizedProposal {
    /// Which of section 27.1's rows this proposes for.
    #[must_use]
    pub const fn generation(&self) -> CandidateGeneration {
        self.generation
    }

    /// Who proposed it.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The digest of what it is about.
    #[must_use]
    pub const fn subject(&self) -> ContentDigest {
        self.subject
    }

    /// When it arrived.
    #[must_use]
    pub const fn submitted_at(&self) -> TimestampMillis {
        self.submitted_at
    }
}

/// What the layer produces for a command it accepted.
///
/// Two arms, one per [`Delegability`] value. A non-delegable action cannot come
/// back as a proposal and a generation cannot come back as a decision, because
/// the two arms hold different types and [`DecisionEvent`] has no producer that
/// takes an automatic actor.
#[derive(Debug)]
pub enum AuthorizedCommand {
    /// Section 27.1. An automatic actor may propose this.
    Proposal(AuthorizedProposal),
    /// Section 27.2 and 27.4. One authenticated user's explicit decision.
    Decision(DecisionEvent),
}

/// The command layer's door.
///
/// Total over [`Action`]: every command gets one of the two arms or a refusal,
/// and which one is decided by [`Action::delegability`] rather than by a list of
/// names this function carries.
///
/// # Errors
///
/// [`NonDelegableError::AutomaticActor`] when a deterministic engine, a model
/// run or an importer submits one of the six.
pub fn authorise(command: ActionCommand) -> Result<AuthorizedCommand, NonDelegableError> {
    let ActionCommand {
        action,
        actor,
        subject,
        submitted_at,
    } = command;
    match action.delegability() {
        Delegability::AutomaticActorMayPropose(generation) => {
            Ok(AuthorizedCommand::Proposal(AuthorizedProposal {
                generation,
                actor,
                subject,
                submitted_at,
            }))
        }
        Delegability::AuthenticatedUserOnly(decide) => {
            let event = DecisionEvent::recorded(decide, &actor, subject, submitted_at)?;
            Ok(AuthorizedCommand::Decision(event))
        }
    }
}
