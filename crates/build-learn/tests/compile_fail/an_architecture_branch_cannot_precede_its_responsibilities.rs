//! Section 20.2's first arrow, held by an argument type.
//!
//! `ArchitectureBranch::of` takes a `ResponsibilityDecomposition` by value. A
//! goal on its own is not one, so the branch cannot be derived before the
//! capability has been decomposed.

use academic_build_learn::{ArchitectureBranch, BuildLearnError, ProjectGoal};
use academic_domain::EntityId;

fn early(goal: ProjectGoal, target: EntityId) -> Result<ArchitectureBranch, BuildLearnError> {
    ArchitectureBranch::of(goal, target, Vec::new(), Vec::new())
}

fn main() {
    let _ = early;
}
