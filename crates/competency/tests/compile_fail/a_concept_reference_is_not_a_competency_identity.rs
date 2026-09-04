//! Section 24.1's first sentence, in one direction.
//!
//! A concept and a competency are different objects, so the identity of one is
//! not the identity of the other. There is no `From`, no `Into` and no
//! constructor of `CompetencyId` that takes a `ConceptRef`.

use academic_competency::{CompetencyId, ConceptRef};

fn main() {
    let concept = ConceptRef::classification("redis").unwrap_or_else(|_| unreachable!());
    let _competency: CompetencyId = concept.into();
}
