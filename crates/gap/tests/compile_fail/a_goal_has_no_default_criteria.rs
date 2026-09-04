//! Section 15.2 step 1: `활성 목표를 concept/competency success criteria로
//! 명시한다`, and step 2 expands only after it.
//!
//! `GoalCriteria` has no `Default`, so there is no criteria value to declare a
//! goal with until somebody states one. That is `P2-N3`'s `PersonalizationSpeed`
//! has no `Default` applied to step 1.

use academic_domain::{EntityId, ScopeId, entity_registry::EntityKind};
use academic_gap::{ActiveGoal, GoalCriteria};

fn main() {
    let goal = EntityId::try_from_uuid(uuid::Uuid::now_v7()).unwrap();
    let scope = ScopeId::try_from_uuid(uuid::Uuid::now_v7()).unwrap();
    let concept = EntityId::try_from_uuid(uuid::Uuid::now_v7()).unwrap();
    let _ = ActiveGoal::declare(
        goal,
        scope,
        concept,
        EntityKind::Concept,
        GoalCriteria::default(),
    );
}
