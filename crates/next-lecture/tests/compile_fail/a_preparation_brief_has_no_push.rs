//! Section 4 and section 25.2 both bound the morning at `1-3`.
//! `PreparationBrief::assemble` takes the whole list and checks the bound where
//! the value comes into existence, so a fourth foundation added afterwards has
//! no method to be added by.

use academic_next_lecture::{PreparationBrief, PreparationCandidate};

fn widen(brief: &mut PreparationBrief, extra: PreparationCandidate) {
    brief.push(extra);
}

fn main() {}
