//! An approved record has one producer, and it is inside the queue.
//!
//! `Approved::new` is `pub(crate)`, so `USER_CONFIRMED` cannot be stamped on a
//! value the history carries no user decision for.

use academic_proposal::{Approved, ProposalId};

fn main() {
    let _approved = Approved::new(ProposalId::new(1), String::from("candidate"));
}
