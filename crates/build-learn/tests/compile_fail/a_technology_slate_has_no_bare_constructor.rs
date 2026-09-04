//! Section 20.1's `기술 목록으로 바꾸지 않고`, held by the absence of a
//! constructor.
//!
//! The only producer of a `TechnologySlate` takes a `ProjectGoal`, which cannot
//! be stated without its success criteria. A list of names has no way in.

use academic_build_learn::TechnologySlate;

fn shopping_list() -> TechnologySlate {
    TechnologySlate::of(vec!["OT".to_owned(), "CRDT".to_owned()])
}

fn main() {
    let _ = shopping_list;
}
