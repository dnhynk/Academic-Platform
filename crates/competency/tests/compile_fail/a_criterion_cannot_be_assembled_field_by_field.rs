//! The checks `PerformanceCriterion::of` runs, as fields that cannot be set.
//!
//! A criterion that names no concept is refused by the one constructor, and
//! writing the struct out by hand is how a caller would get past it. Every
//! field is private, so there is no such value.

use academic_competency::PerformanceCriterion;

fn main() {
    let _ = PerformanceCriterion {
        id: academic_competency::CriterionId::new("c-1").unwrap_or_else(|_| unreachable!()),
        requirement: "knows B+ Tree".to_owned(),
        about: Vec::new(),
    };
}
