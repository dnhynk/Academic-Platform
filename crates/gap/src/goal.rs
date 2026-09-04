//! Section 15.2 step 1: `활성 목표를 concept/competency success criteria로
//! 명시한다` — and the fact that step 2 cannot run before it.
//!
//! ## There is no expansion without criteria, and no criteria without a member
//!
//! [`GoalCriteria`] has private fields and one constructor, which returns
//! `None` for an empty list. [`ActiveGoal::declare`] takes a `GoalCriteria` **by
//! value** and there is no other way to build one — no `Default`, no public
//! field, no `new` taking a `Vec`. [`crate::engine::expand`] and
//! [`crate::engine::search`] take an `&ActiveGoal` and take no other route in.
//!
//! So `goal_criteria_required_before_expansion` is not a check somebody
//! remembers to run before expanding: **a goal without success criteria is a
//! value that cannot be constructed**, which is `P2-N3`'s `PersonalizationSpeed`
//! has no `Default` and `P2-N2`'s `AutomaticLevel` has no `Fluent` applied to
//! step 1. `crates/gap/tests/compile_fail/` holds the compiled half.
//!
//! ## `low_mastery_without_goal_is_not_a_gap` is the same absence read the
//! other way
//!
//! Nothing in this crate turns a concept and a state into a gap. Every producer
//! of a [`crate::case::GapCase`] is reached through [`crate::engine::search`],
//! whose first argument is an `&ActiveGoal`, so a low mastery with no goal
//! has no function to be passed to. There is no `GapCase::for_concept`, no
//! `GapCase::new` and no `From<ConceptState> for GapCase`.

use academic_domain::{EntityId, MasteryLevel, ScopeId, entity_registry::EntityKind};
use serde::{Deserialize, Serialize};

use crate::{GapError, node::gap_bearing};

/// One success criterion of an active goal.
///
/// Section 15.2 step 1 admits two shapes and this enumeration is those two.
/// A criterion naming a `FIELD` is refused for the reason section 7.4 gives:
/// a field carries no independent prerequisite, so `Database Systems를 할 수
/// 있다` fixes nothing an expansion could aim at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuccessCriterion {
    /// A concept, and the ladder position the goal needs on it.
    Concept {
        /// The concept identity.
        concept: EntityId,
        /// The tier `P2-C3`'s registry holds for it.
        kind: EntityKind,
        /// The rung the goal counts as success.
        at_least: MasteryLevel,
    },
    /// A competency, stated as section 7.1 requires: `관찰 가능한 상황에서
    /// 수행할 수 있다`.
    Competency {
        /// The competency identity.
        competency: EntityId,
        /// The observable-performance sentence. Never empty.
        performance: String,
    },
}

impl SuccessCriterion {
    /// Declares a concept criterion.
    ///
    /// # Errors
    ///
    /// [`GapError::CriterionSubjectCarriesNoPrerequisite`] when `kind` is a tier
    /// that carries no independent prerequisite of its own.
    pub fn concept(
        concept: EntityId,
        kind: EntityKind,
        at_least: MasteryLevel,
    ) -> Result<Self, GapError> {
        if !gap_bearing(kind) {
            return Err(GapError::CriterionSubjectCarriesNoPrerequisite { kind });
        }
        Ok(Self::Concept {
            concept,
            kind,
            at_least,
        })
    }

    /// Declares a competency criterion.
    ///
    /// # Errors
    ///
    /// [`GapError::CompetencyPerformanceMissing`] when the observable-performance
    /// sentence is blank.
    pub fn competency(competency: EntityId, performance: &str) -> Result<Self, GapError> {
        if performance.trim().is_empty() {
            return Err(GapError::CompetencyPerformanceMissing);
        }
        Ok(Self::Competency {
            competency,
            performance: performance.to_owned(),
        })
    }

    /// The entity this criterion is about.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        match self {
            Self::Concept { concept, .. } => *concept,
            Self::Competency { competency, .. } => *competency,
        }
    }
}

/// A non-empty set of section 15.2 step 1 success criteria.
///
/// Private field, one constructor, no `Default`. See the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<SuccessCriterion>", into = "Vec<SuccessCriterion>")]
pub struct GoalCriteria {
    criteria: Vec<SuccessCriterion>,
}

impl GoalCriteria {
    /// Declares the criteria. Returns `None` when the list is empty.
    #[must_use]
    pub fn of(criteria: Vec<SuccessCriterion>) -> Option<Self> {
        if criteria.is_empty() {
            return None;
        }
        Some(Self { criteria })
    }

    /// The criteria, in declaration order.
    #[must_use]
    pub fn criteria(&self) -> &[SuccessCriterion] {
        &self.criteria
    }

    /// Whether any criterion names `entity`.
    #[must_use]
    pub fn names(&self, entity: EntityId) -> bool {
        self.criteria
            .iter()
            .any(|criterion| criterion.subject() == entity)
    }

    /// The rung the goal needs on `concept`, when a concept criterion names it.
    #[must_use]
    pub fn required_level(&self, concept: EntityId) -> Option<MasteryLevel> {
        self.criteria.iter().find_map(|criterion| match criterion {
            SuccessCriterion::Concept {
                concept: named,
                at_least,
                ..
            } if *named == concept => Some(*at_least),
            _ => None,
        })
    }
}

impl TryFrom<Vec<SuccessCriterion>> for GoalCriteria {
    type Error = GapError;

    fn try_from(criteria: Vec<SuccessCriterion>) -> Result<Self, Self::Error> {
        Self::of(criteria).ok_or(GapError::GoalHasNoSuccessCriteria)
    }
}

impl From<GoalCriteria> for Vec<SuccessCriterion> {
    fn from(value: GoalCriteria) -> Self {
        value.criteria
    }
}

/// An active goal, its surface concept and its success criteria.
///
/// Section 15.1's `goal` and `surfaceConcept`. `declare` is the only
/// constructor and it takes a [`GoalCriteria`] by value, so step 2 has nothing
/// to expand from until step 1 has happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveGoal {
    goal: EntityId,
    scope: ScopeId,
    surface_concept: EntityId,
    surface_kind: EntityKind,
    criteria: GoalCriteria,
}

impl ActiveGoal {
    /// Declares the goal.
    ///
    /// # Errors
    ///
    /// [`GapError::SurfaceConceptCarriesNoPrerequisite`] when the surface tier
    /// carries no independent prerequisite of its own — section 7.4's
    /// `지나치게 큰 "Database"는 Field/cluster`, which is section 15.3's
    /// `데이터베이스를 더 공부하세요` refused at the point the goal is stated
    /// rather than at the point advice is printed.
    pub fn declare(
        goal: EntityId,
        scope: ScopeId,
        surface_concept: EntityId,
        surface_kind: EntityKind,
        criteria: GoalCriteria,
    ) -> Result<Self, GapError> {
        if !gap_bearing(surface_kind) {
            return Err(GapError::SurfaceConceptCarriesNoPrerequisite { kind: surface_kind });
        }
        Ok(Self {
            goal,
            scope,
            surface_concept,
            surface_kind,
            criteria,
        })
    }

    /// The goal identity.
    #[must_use]
    pub const fn goal(&self) -> EntityId {
        self.goal
    }

    /// The resolution scope the goal was declared under.
    #[must_use]
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Section 15.1's `surfaceConcept`.
    #[must_use]
    pub const fn surface_concept(&self) -> EntityId {
        self.surface_concept
    }

    /// The surface concept's tier.
    #[must_use]
    pub const fn surface_kind(&self) -> EntityKind {
        self.surface_kind
    }

    /// The criteria step 1 requires.
    #[must_use]
    pub const fn criteria(&self) -> &GoalCriteria {
        &self.criteria
    }
}
