//! Section 23: `mastery 점수로 바꾸지 않는다`.
//!
//! A score is a value two of which can be ranked. `FieldCoverage` derives
//! neither `PartialOrd` nor `Ord`, so a comparison between two readings has no
//! representation.

use academic_blind_spot::FieldCoverage;

fn ranked(a: &FieldCoverage, b: &FieldCoverage) -> bool {
    a < b
}

fn main() {}
