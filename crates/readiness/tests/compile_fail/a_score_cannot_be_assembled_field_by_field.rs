//! Section 24.3's `보조 score` does not exist without its four disclosures.
//!
//! `disclose` is the one producer and it computes the number from the disclosed
//! weights over the disclosed matrix. A struct literal would be a second door
//! that ran none of the three re-derivations and took the number as an input.

use academic_readiness::{AuxiliaryScore, ScoreValue};

fn shape(value: ScoreValue) -> AuxiliaryScore {
    AuxiliaryScore { value }
}

fn main() {
    let _ = shape;
}
