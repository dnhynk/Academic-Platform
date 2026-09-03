//! No trait takes the boundary off either.
//!
//! `Deref`, `AsRef`, `Borrow`, `Into` and `Display` would each hand out the
//! payload, or a view of it, without passing a door. None is implemented, and
//! the orphan rule stops another crate implementing them here.

use std::borrow::Borrow;
use std::ops::Deref;

use academic_domain::ConfidencePermille;
use academic_proposal::{ImpactPermille, ProposalId, Proposed, RiskTier};

fn wrapped() -> Proposed<String> {
    Proposed::new(
        ProposalId::new(1),
        RiskTier::MediumReview,
        ConfidencePermille::new(500).unwrap(),
        ImpactPermille::new(500).unwrap(),
        String::from("candidate"),
    )
}

fn main() {
    let _deref: &String = wrapped().deref();
    let _as_ref: &String = wrapped().as_ref();
    let _borrow: &String = wrapped().borrow();
    let _into: String = wrapped().into();
    let _display = format!("{}", wrapped());
    let _to_string = wrapped().to_string();
}
