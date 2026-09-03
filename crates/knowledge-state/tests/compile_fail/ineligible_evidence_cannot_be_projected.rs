//! Section 13.4's four checks produce `EligibleEvidence` and nothing else does.
//!
//! `project` takes a slice of those, so evidence that failed a check is not
//! evidence a later layer has to remember to filter — it has no value of the
//! type the projection accepts.

use academic_knowledge_state::{BlockedEvidence, ConceptEvidence, project};

fn skip(raw: &[ConceptEvidence], blocked: &[BlockedEvidence]) {
    let _ = project(raw, blocked);
}

fn main() {}
