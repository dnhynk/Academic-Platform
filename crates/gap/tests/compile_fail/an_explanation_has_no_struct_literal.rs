//! Section 15.3's eight fields are private and `GapExplanation::of` runs the
//! specificity validator before it returns, so an explanation that skipped the
//! validator has no representation.

use academic_gap::GapExplanation;

fn main() {
    let _ = GapExplanation {
        evidence: Vec::new(),
    };
}
