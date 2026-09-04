//! `P2-M4` — the actions an AI may not take for the user.
//!
//! `INV-C-010` is *AI alone never resolves a question*, and this crate is the
//! half of it that lives above the types. The execution plan's outcome is
//! service-level refusal of `MODEL_RUN`, `IMPORTER` and `DETERMINISTIC_ENGINE`
//! for six actions, with the set as a compiled constant and every entry needing
//! an authenticated user actor and an explicit decision event.
//!
//! # The six, and where each is already refused
//!
//! Three of the six were already refused by a type before this task, and the
//! contract this crate fixes is that its constant **agrees with them** rather
//! than checking them a second time. Three were not refused anywhere, and those
//! are the ones this layer closes.
//!
//! | action | the door that already existed | what this crate adds |
//! |---|---|---|
//! | [`NonDelegableAction::ResolveQuestion`] | `academic_domain::question` verifies `Actor::User` before a resolution claim | agreement, measured |
//! | [`NonDelegableAction::ConfirmMastery`] | `P2-N2`'s `UserConfirmation`, and an `AutomaticLevel` with no `Fluent` variant | agreement, measured |
//! | [`NonDelegableAction::ConfirmDeletion`] | `P2-P2`'s `DeletionConfirmation`, through `P2-M2`'s `UserDecision` | agreement, measured |
//! | [`NonDelegableAction::DecideEnrollmentOrCareer`] | **none.** `academic_record::RegistrationConfirmation::new` takes no actor | the refusal |
//! | [`NonDelegableAction::AttestPermission`] | **none.** `academic_consent::AuthorityGrant::record` takes no actor | the refusal |
//! | [`NonDelegableAction::ApproveEgress`] | **none.** `academic_policy`'s request tuple carries `actor_id` as an untyped string | the refusal |
//!
//! The three "none" rows are measured, not assumed:
//! `ai_cannot_decide_enrollment_or_career` builds a real registration
//! confirmation, `ai_cannot_attest_permission` a real authority grant, and
//! `ai_cannot_approve_egress` installs two real egress rules differing only in
//! `actor_id` and observes the broker allow both. Each drives the crate that
//! owns the action and observes the absence rather than describing it.
//!
//! # `graduation_result_cannot_come_from_generation` is a different axis
//!
//! Section 27.2's ninth bullet is `graduation pass/fail을 자유 텍스트
//! generation으로 결정`. That is **not** a member of the set above, and putting
//! it there would have been wrong in a way worth stating: the six actions refuse
//! `DETERMINISTIC_ENGINE`, and a deterministic engine is exactly the correct
//! author of a graduation result. Section 28's `Graduation Audit` row is an
//! engine, and `P2-U3` is that engine.
//!
//! What the bullet forbids is a *generation* deciding it, which is a claim about
//! where the input came from and not about which actor pressed the button. So it
//! is held by three separate facts, each measured rather than declared:
//!
//! * no row of section 27.1 is a graduation row, so a model may not even produce
//!   a candidate for one;
//! * `academic_domain::InputValue::Reference` is `identifier-shaped … never a
//!   sentence`, so free text cannot enter the frozen inputs the engine is a
//!   function of; and
//! * `academic_audit::DeterminateVerdict::new` is `pub(crate)`, so no crate
//!   outside `academic-audit` can assemble a verdict at all —
//!   `tests/compile_fail` names it from here and observes the error.
//!
//! # What this is not evidence for
//!
//! **The daemon has no product command surface yet.**
//! `academic_rpc::ValidatedWriteCommand` has three synthetic Phase 1 arms and
//! none of them carries an actor, so there is no wire command for any of the six
//! and no arm for a refusal to sit inside. [`authorise`] is the door a command
//! layer calls; a caller that reaches
//! `academic_record::RegistrationConfirmation::new` or
//! `academic_consent::AuthorityGrant::record` directly still bypasses it, and
//! [`crate::command`] says so beside the function.
//!
//! **Nothing persists.** There is no `academic-store` edge, this task claims no
//! migration number, and a refusal is a value the caller records rather than a
//! table this task invents.
//!
//! **The `PREDICTION` gate is untouched.** Whether a deterministic engine may
//! assert its own forecast under `AuthorityClass::Prediction` is a separate open
//! user decision about *claims*; this crate is about *decisions*, names no
//! authority class, and has no `academic-ledger` edge.

#![doc(test(attr(deny(warnings))))]

pub mod action;
pub mod command;
pub mod decision;
pub mod error;

pub use action::{Action, CandidateGeneration, Delegability, NonDelegableAction};
pub use command::{ActionCommand, AuthorizedCommand, AuthorizedProposal, authorise};
pub use decision::DecisionEvent;
pub use error::NonDelegableError;
