//! The same separation, in the other direction.
//!
//! A conversion either way would let one channel be computed from the other's
//! input, and section 26.2's eight channels are eight because they vary
//! independently.

use academic_cs_map::{LensRelevance, MasteryFill};

fn fill(relevance: LensRelevance) -> MasteryFill {
    MasteryFill::of(relevance)
}

fn main() {
    let _ = fill;
}
