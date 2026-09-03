//! There is no accessor that hands the payload back.
//!
//! `release` is `pub(crate)`, the field is private, and no other method returns
//! the wrapped value. A caller outside the crate that wants to write the
//! model's candidate has to go through a door that records a disposition first.

use academic_domain::ConfidencePermille;
use academic_proposal::{ImpactPermille, ProposalId, Proposed, RiskTier};

fn main() {
    let proposed = Proposed::new(
        ProposalId::new(1),
        RiskTier::MediumReview,
        ConfidencePermille::new(500).unwrap(),
        ImpactPermille::new(500).unwrap(),
        String::from("candidate"),
    );
    let _by_accessor: String = proposed.release();
    let _by_field: String = proposed.value;
}
