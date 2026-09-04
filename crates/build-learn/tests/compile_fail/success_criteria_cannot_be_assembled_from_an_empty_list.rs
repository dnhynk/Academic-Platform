//! Section 20.1's first step, held by the absence of a field.
//!
//! `SuccessCriteria::of` returns `None` for an empty list, and a caller who
//! wanted the empty value anyway has no field to write it into.

use academic_build_learn::SuccessCriteria;

fn empty() -> SuccessCriteria {
    SuccessCriteria {
        criteria: Vec::new(),
    }
}

fn main() {
    let _ = empty;
}
