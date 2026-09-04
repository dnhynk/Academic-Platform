//! Section 20.2's `AND/OR branch`, held by the absence of an argument.
//!
//! `BranchGroup::of` stamps the condition onto every member it is given, and no
//! public constructor of a `ConceptRequirement` takes a `RequirementCondition`.
//! So a member of an `OR` branch cannot be published unconditionally.

use academic_build_learn::{BuildLearnError, ConceptRequirement, PartId, RequirementCondition};
use academic_domain::{EntityId, entity_registry::EntityKind};

fn forge(
    concept: EntityId,
    kind: EntityKind,
    serves: PartId,
) -> Result<ConceptRequirement, BuildLearnError> {
    ConceptRequirement::always(concept, kind, serves, RequirementCondition::Unconditional)
}

fn main() {
    let _ = forge;
}
