//! `an_authorised_proposal_has_no_struct_literal`.
//!
//! The command layer's two answers are a proposal and a decision. If a caller
//! could write the proposal itself, "this action was classified as one an
//! automatic actor may propose" would be the caller's claim rather than the
//! layer's, and the classification would stop being load-bearing.

use academic_domain::TimestampMillis;
use academic_non_delegable::{AuthorizedProposal, CandidateGeneration};

fn main() {
    let _forged = AuthorizedProposal {
        generation: CandidateGeneration::CareerMapping,
        actor: unimplemented!(),
        subject: unimplemented!(),
        submitted_at: TimestampMillis::new(0),
    };
}
