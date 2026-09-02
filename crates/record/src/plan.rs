//! Planned coursework, kept apart from the attempt ledger.
//!
//! "`PlannedCourse`는 CourseAttempt와도 분리한다. … 따라서 계획 삭제가 학사
//! 이력을 지우거나, 계획만으로 졸업 actual progress가 올라가지 않는다."
//!
//! Both halves of that sentence are structural here rather than checked:
//!
//! - A [`PlanScenario`] owns [`PlanScenarioChoice`] values, and this module
//!   declares no method on either that returns a `CourseAttempt`, no
//!   `From`/`Into` between them, and no method returning a
//!   `RegistrationConfirmation`. `CourseAttempt`'s two constructors take a
//!   `RegistrationConfirmation` or a confirmed transcript row, and neither
//!   accepts a `PlanScenarioChoice`, so a plan choice has nothing to hand
//!   either of them. `registered_attempt_gate` enumerates the constructors and
//!   the statuses each can produce.
//!
//! - [`delete_scenario`] takes the scenario store and an immutable borrow of
//!   the [`AttemptHistory`]. It cannot reach a mutator of the history because
//!   it does not hold one, and `AttemptHistory` has no removal mutator to
//!   reach. `delete_plan_preserves_attempts` deletes a scenario whose every
//!   choice names a course the history has attempts for, and observes the
//!   ledger byte-identical afterwards.
//!
//! ## Against `P2-K5`
//!
//! This deletion is not the retention deletion. `academic-retention` plans over
//! a `RetentionSubject` — a vault object or a span inside one — and settles
//! with a shredded key slot and a tombstone that a restore re-applies. A plan
//! scenario is neither: it is canonical record state, it has no vault object,
//! and deleting one writes no tombstone. The two paths share no type and no
//! function, so there is nothing here for a rotation or a restore to reach.

use std::collections::BTreeMap;

use academic_domain::EntityId;

use crate::{RecordError, attempt::AttemptHistory, term::TermKey};

/// One candidate course inside a scenario.
///
/// It references a course code and an intended term, and it carries no grade,
/// no credits earned, and no attempt identity — a plan is a proposal, and the
/// fields an attempt has are exactly the ones a proposal has no answer for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanScenarioChoice {
    course_code: String,
    intended_term: TermKey,
}

impl PlanScenarioChoice {
    /// Builds a plan choice.
    pub fn new(
        course_code: impl Into<String>,
        intended_term: TermKey,
    ) -> Result<Self, RecordError> {
        let course_code = course_code.into();
        if course_code.trim().is_empty() {
            return Err(RecordError::EmptyField("course code"));
        }
        Ok(Self {
            course_code,
            intended_term,
        })
    }

    /// Returns the course the choice proposes.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// Returns the term the choice proposes it for.
    #[must_use]
    pub const fn intended_term(&self) -> TermKey {
        self.intended_term
    }
}

/// A named set of plan choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanScenario {
    id: EntityId,
    label: String,
    choices: Vec<PlanScenarioChoice>,
}

impl PlanScenario {
    /// Builds a scenario.
    pub fn new(
        id: EntityId,
        label: impl Into<String>,
        choices: Vec<PlanScenarioChoice>,
    ) -> Result<Self, RecordError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(RecordError::EmptyField("scenario label"));
        }
        Ok(Self { id, label, choices })
    }

    /// Returns the scenario identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Returns the scenario label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the choices.
    #[must_use]
    pub fn choices(&self) -> &[PlanScenarioChoice] {
        &self.choices
    }
}

/// The scenarios a profile holds.
///
/// Unlike [`AttemptHistory`], this **is** removable: a plan is a proposal, and
/// discarding a proposal is an ordinary thing to do. Keeping the two in
/// separate types is what makes that difference safe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanStore {
    scenarios: BTreeMap<EntityId, PlanScenario>,
}

impl PlanStore {
    /// Builds an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scenarios: BTreeMap::new(),
        }
    }

    /// Adds a scenario, refusing a duplicate identity.
    pub fn insert(&mut self, scenario: PlanScenario) -> Result<(), RecordError> {
        if self.scenarios.contains_key(&scenario.id()) {
            return Err(RecordError::DuplicateScenarioId(scenario.id()));
        }
        self.scenarios.insert(scenario.id(), scenario);
        Ok(())
    }

    /// Returns one scenario.
    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&PlanScenario> {
        self.scenarios.get(&id)
    }

    /// Returns how many scenarios are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}

/// What a scenario deletion did, and what it deliberately left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDeletion {
    /// The scenario removed.
    pub scenario_id: EntityId,
    /// How many choices went with it.
    pub choices_removed: usize,
    /// How many attempts the ledger held before, and still holds after.
    pub attempts_preserved: usize,
}

/// Deletes one plan scenario.
///
/// The `history` argument is an immutable borrow and is read only to report
/// how many attempts survived. That is the whole point of the signature: a
/// deletion that *could* reach the ledger would need `&mut AttemptHistory`, and
/// the caller can see from the type that it does not have one.
pub fn delete_scenario(
    store: &mut PlanStore,
    history: &AttemptHistory,
    scenario_id: EntityId,
) -> Result<PlanDeletion, RecordError> {
    let removed = store
        .scenarios
        .remove(&scenario_id)
        .ok_or(RecordError::UnknownScenarioId(scenario_id))?;
    Ok(PlanDeletion {
        scenario_id,
        choices_removed: removed.choices().len(),
        attempts_preserved: history.all().len(),
    })
}
