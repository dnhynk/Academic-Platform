//! Section 13.2's ceiling, as a value with one producer.
//!
//! `PromotingEvidence::of` is the one place a section 13.2 row is asked whether
//! it licenses a promotion. The field behind it is private, so a caller cannot
//! wrap a dependency-presence item and skip the question.

use academic_competency::PromotingEvidence;
use academic_knowledge_state::EligibleEvidence;

fn wrap(admitted: EligibleEvidence) {
    let _ = PromotingEvidence { inner: admitted };
}

fn main() {}
