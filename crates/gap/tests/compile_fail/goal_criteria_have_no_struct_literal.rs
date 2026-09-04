//! `GoalCriteria::of` returns `None` for an empty list, and its field is
//! private, so an empty criteria set has no representation at all.

use academic_gap::GoalCriteria;

fn main() {
    let _ = GoalCriteria {
        criteria: Vec::new(),
    };
}
