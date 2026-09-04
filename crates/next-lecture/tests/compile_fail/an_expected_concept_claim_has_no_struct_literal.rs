//! Section 27.4's low-risk row says an extracted topic is a candidate marked
//! `AI_INFERRED`. `ExpectedConceptClaim::extract` is the only producer and it
//! takes no status, so a claim assembled field by field -- past the tier check,
//! past the citation check, and with any standing at all -- has no value.

use academic_next_lecture::ExpectedConceptClaim;

fn forge() -> ExpectedConceptClaim {
    ExpectedConceptClaim {
        concept: todo!(),
        concept_kind: todo!(),
        material: todo!(),
        citations: todo!(),
        confidence: todo!(),
    }
}

fn main() {}
