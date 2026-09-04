//! `P2-C7`'s seal, from the audit side.
//!
//! `academic_scenario::Proposed<T>` has no exit: no `into_inner`, no `Deref`,
//! no `From<Proposed<T>> for T`. A projected course fact is therefore not a
//! course fact, and there is no expression that turns one into the value
//! `CourseFactsIndex::with` takes.
//!
//! This crate has no `academic-scenario` product edge at all, so the type is
//! only nameable from the test tree -- which is why this case lives here and
//! why the scan sweeps the product tree for the same name.

use academic_audit::{CourseFactsIndex, CourseRequirementFacts};
use academic_scenario::{ProposalProvenance, Proposed};

fn main() {
    let facts: CourseRequirementFacts = unimplemented!();
    let provenance: ProposalProvenance = unimplemented!();
    let projected = Proposed::new(facts, provenance);

    // No accessor returns the sealed value.
    let _unsealed = projected.into_inner();

    // And the sealed wrapper is not the value the index takes.
    let _index = CourseFactsIndex::new().with("4190.101", projected);
}
