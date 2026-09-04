//! Section 15.2 step 2 traverses `REQUIRES` and strong `BUILDS_ON`, and
//! `P2-C4`'s registry is what says which those are. `PrerequisiteEdge::admit`
//! asks it; the fields are private, so an edge that skipped the registry has no
//! representation.

use academic_domain::{EntityId, predicates::{PredicateName, PrerequisiteStrength}};
use academic_gap::PrerequisiteEdge;

fn main() {
    let advanced = EntityId::try_from_uuid(uuid::Uuid::now_v7()).unwrap();
    let prerequisite = EntityId::try_from_uuid(uuid::Uuid::now_v7()).unwrap();
    let _ = PrerequisiteEdge {
        predicate: PredicateName::RelatedTo,
        strength: PrerequisiteStrength::Hard,
        advanced,
        prerequisite,
        evidence: Vec::new(),
    };
}
