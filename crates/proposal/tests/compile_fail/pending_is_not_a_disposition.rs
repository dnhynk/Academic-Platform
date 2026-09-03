//! Pending is a queue state and carries no user authority.
//!
//! `DispositionState::Undisposed` is not a `DecisionAction` and has no
//! conversion into one, so "the user has not decided yet" cannot be handed to
//! anything that ranks a user decision. Making it a disposition would give it a
//! place in ADR-003's authority computation, where it would read as a judgment.

use academic_domain::DecisionAction;
use academic_proposal::DispositionState;

fn main() {
    let pending = DispositionState::Undisposed;
    let _as_action: DecisionAction = pending.into();
    let _by_from = DecisionAction::from(DispositionState::Undisposed);
}
