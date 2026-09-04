//! Section 13.3: `명시적 근거로 제한`.
//!
//! `CitedEdge`'s fields are private and `of` is its one constructor, which
//! refuses a predicate outside the allowlist, a self-edge and an edge citing no
//! evidence. An edge assembled past it has no representation.

use academic_domain::{EntityId, EvidenceId, predicates::PredicateName};
use academic_freshness::CitedEdge;

fn forge(subject: EntityId, object: EntityId, evidence: Vec<EvidenceId>) -> CitedEdge {
    CitedEdge {
        predicate: PredicateName::TaughtIn,
        subject,
        object,
        evidence,
    }
}

fn main() {}
