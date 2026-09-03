//! A user decision has one producer, and it takes an actor.
//!
//! `UserDecision::by` refuses every automatic actor. A struct literal would let
//! a model run manufacture the conclusion that a user decided, which is what
//! section 27.4's fourth row exists to prevent.

use academic_proposal::{ExplicitApproval, ProposalId, UserDecision};

fn main() {
    let forged = UserDecision { user_id: 1 };
    let _approval = ExplicitApproval {
        proposal_id: ProposalId::new(1),
        decision: forged,
    };
}
