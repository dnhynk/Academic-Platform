//! Section 20.1's `ProjectGoal`, and the four groups it keeps apart.
//!
//! ```yaml
//! ProjectGoal:
//!   text: "실시간 협업 편집기를 만들고 싶다"
//!   successCriteria: [...]
//!   constraints: [...]
//!   unresolvedDecisions: [...]
//! ```
//!
//! ## Four groups, four types, four serialized keys
//!
//! [`ProjectGoal`] holds a [`crate::text::NonEmptyText`], a [`SuccessCriteria`],
//! a [`Constraints`] and an [`UnresolvedDecisions`], and those are four
//! different types rather than four lists of sentences. So
//! `goal_schema_separates_four_groups` is not a check that somebody spelled the
//! keys right: an unresolved decision **cannot** be serialized as a constraint,
//! because [`UnresolvedDecision`] has an alternative list and [`Constraint`] has
//! no field that could hold one, and neither has a conversion to the other.
//!
//! ## The criteria come first, and that is a type and not an order of statements
//!
//! [`SuccessCriteria::of`] returns `None` for an empty list, [`ProjectGoal::state`]
//! takes a `SuccessCriteria` **by value**, and there is no other constructor —
//! no `Default`, no public field, no `new` taking a `Vec`. That is
//! `P2-N5`'s [`academic_gap::GoalCriteria`] applied to section 20's own criteria,
//! and it is the first link of the chain [`crate::technology`] describes: a goal
//! with no success criteria is a value that cannot be built, so a technology
//! list derived from one is a value that cannot be built either.
//!
//! ## An observable criterion states how it would be observed
//!
//! Section 20.1's three example criteria are `concurrent edits converge
//! according to chosen semantics`, `reconnect does not silently lose
//! acknowledged edits` and `user-visible latency target is stated`. Each names
//! something that could be watched happening. `실시간 협업 편집기를 만들고
//! 싶다` — the goal's own `text` — names nothing that could. So
//! [`ObservableCriterion::state`] takes the statement **and** the observation
//! that decides it, both non-blank, which is `P2-Y1`'s
//! [`academic_competency::PerformanceCriterion`] discipline: a statement with no
//! occasion on which anybody could watch it happen is not a criterion.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    BuildLearnError,
    input::{InputKind, NormalizedIntent},
    text::{NonEmptyText, PartId},
};

/// One row of section 20.1's `successCriteria`.
///
/// Private fields, one constructor. Both parts are required: what has to be
/// true, and what would be watched to decide it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservableCriterion {
    id: PartId,
    statement: NonEmptyText,
    observed_by: NonEmptyText,
}

impl ObservableCriterion {
    /// Records one criterion.
    #[must_use]
    pub const fn state(id: PartId, statement: NonEmptyText, observed_by: NonEmptyText) -> Self {
        Self {
            id,
            statement,
            observed_by,
        }
    }

    /// Its identity within the goal.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// What has to be true.
    #[must_use]
    pub const fn statement(&self) -> &NonEmptyText {
        &self.statement
    }

    /// What would be watched to decide it.
    #[must_use]
    pub const fn observed_by(&self) -> &NonEmptyText {
        &self.observed_by
    }
}

/// A non-empty set of section 20.1 success criteria.
///
/// Private field, one constructor returning `None` for the empty list, no
/// `Default`. See the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "Vec<ObservableCriterion>",
    into = "Vec<ObservableCriterion>"
)]
pub struct SuccessCriteria {
    criteria: Vec<ObservableCriterion>,
}

impl SuccessCriteria {
    /// Declares the criteria. `None` when the list is empty.
    #[must_use]
    pub fn of(criteria: Vec<ObservableCriterion>) -> Option<Self> {
        if criteria.is_empty() {
            return None;
        }
        Some(Self { criteria })
    }

    /// The criteria, in declaration order.
    #[must_use]
    pub fn criteria(&self) -> &[ObservableCriterion] {
        &self.criteria
    }

    /// The criterion `id` names.
    #[must_use]
    pub fn criterion(&self, id: &PartId) -> Option<&ObservableCriterion> {
        self.criteria.iter().find(|item| item.id() == id)
    }
}

impl TryFrom<Vec<ObservableCriterion>> for SuccessCriteria {
    type Error = BuildLearnError;

    fn try_from(criteria: Vec<ObservableCriterion>) -> Result<Self, Self::Error> {
        Self::of(criteria).ok_or(BuildLearnError::GoalHasNoSuccessCriteria)
    }
}

impl From<SuccessCriteria> for Vec<ObservableCriterion> {
    fn from(value: SuccessCriteria) -> Self {
        value.criteria
    }
}

/// One row of section 20.1's `constraints`: something already fixed.
///
/// `web client`, `current single-region deployment`. A constraint is a fact the
/// plan has to work inside, so it has one sentence and no alternatives. That
/// absence is the separation: an unresolved decision has alternatives and this
/// has no field one could go in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    id: PartId,
    statement: NonEmptyText,
}

impl Constraint {
    /// Records one constraint.
    #[must_use]
    pub const fn fixed(id: PartId, statement: NonEmptyText) -> Self {
        Self { id, statement }
    }

    /// Its identity within the goal.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// What is already fixed.
    #[must_use]
    pub const fn statement(&self) -> &NonEmptyText {
        &self.statement
    }
}

/// Section 20.1's `constraints`, which may legitimately be empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Constraints {
    constraints: Vec<Constraint>,
}

impl Constraints {
    /// Declares the constraints.
    #[must_use]
    pub const fn of(constraints: Vec<Constraint>) -> Self {
        Self { constraints }
    }

    /// The constraints, in declaration order.
    #[must_use]
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
}

/// One alternative of an unresolved decision.
///
/// `central ordering`, `peer/offline merge`, `OT`, `CRDT`. This is the **only**
/// place in this crate where a technology may be named, and it is reachable
/// only through an [`UnresolvedDecision`], which is reachable only through a
/// [`ProjectGoal`]. See [`crate::technology`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alternative {
    id: PartId,
    name: NonEmptyText,
}

impl Alternative {
    /// Records one alternative.
    #[must_use]
    pub const fn named(id: PartId, name: NonEmptyText) -> Self {
        Self { id, name }
    }

    /// Its identity within the decision.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// What it is called.
    #[must_use]
    pub const fn name(&self) -> &NonEmptyText {
        &self.name
    }
}

/// One row of section 20.1's `unresolvedDecisions`.
///
/// `central ordering vs peer/offline merge`, `OT vs CRDT conditional branch`.
/// At least two alternatives: a decision with one alternative is a constraint
/// wearing the other group's name, and section 20's whole point is that the
/// user can see there is a choice. That is `P2-N6`'s
/// [`academic_critical_path::Hyperedge::requires_one_of`] rule applied one layer
/// up, and it is why [`crate::branch`] can turn a decision into a disjunction
/// without a second check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedDecision {
    id: PartId,
    question: NonEmptyText,
    alternatives: Vec<Alternative>,
}

impl UnresolvedDecision {
    /// Records one open decision.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::DecisionHasOneAlternative`] for fewer than two, and
    /// [`BuildLearnError::DuplicateAlternative`] when two share an identity.
    pub fn open(
        id: PartId,
        question: NonEmptyText,
        alternatives: Vec<Alternative>,
    ) -> Result<Self, BuildLearnError> {
        if alternatives.len() < 2 {
            return Err(BuildLearnError::DecisionHasOneAlternative(
                id.as_str().to_owned(),
            ));
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for alternative in &alternatives {
            if !seen.insert(alternative.id().as_str()) {
                return Err(BuildLearnError::DuplicateAlternative(
                    alternative.id().as_str().to_owned(),
                ));
            }
        }
        Ok(Self {
            id,
            question,
            alternatives,
        })
    }

    /// Its identity within the goal.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// What has not been decided.
    #[must_use]
    pub const fn question(&self) -> &NonEmptyText {
        &self.question
    }

    /// The alternatives, in declaration order. At least two.
    #[must_use]
    pub fn alternatives(&self) -> &[Alternative] {
        &self.alternatives
    }

    /// The alternative `id` names.
    #[must_use]
    pub fn alternative(&self, id: &PartId) -> Option<&Alternative> {
        self.alternatives.iter().find(|item| item.id() == id)
    }
}

/// Section 20.1's `unresolvedDecisions`, which may legitimately be empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnresolvedDecisions {
    decisions: Vec<UnresolvedDecision>,
}

impl UnresolvedDecisions {
    /// Declares the open decisions.
    #[must_use]
    pub const fn of(decisions: Vec<UnresolvedDecision>) -> Self {
        Self { decisions }
    }

    /// The decisions, in declaration order.
    #[must_use]
    pub fn decisions(&self) -> &[UnresolvedDecision] {
        &self.decisions
    }

    /// The decision `id` names.
    #[must_use]
    pub fn decision(&self, id: &PartId) -> Option<&UnresolvedDecision> {
        self.decisions.iter().find(|item| item.id() == id)
    }
}

/// Section 20.1's `ProjectGoal`.
///
/// Private fields, one constructor, no `Default`, no setter. The four groups are
/// four types and the source kind is retained beside them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectGoal {
    source: InputKind,
    text: NonEmptyText,
    success_criteria: SuccessCriteria,
    constraints: Constraints,
    unresolved_decisions: UnresolvedDecisions,
}

impl ProjectGoal {
    /// States the goal, taking its criteria by value.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::DuplicateCriterion`],
    /// [`BuildLearnError::DuplicateConstraint`] and
    /// [`BuildLearnError::DuplicateDecision`] when two rows of one group share
    /// an identity — a joined document cannot say which of the two a later
    /// requirement is about, so the collision is refused where it is made
    /// rather than resolved silently later.
    pub fn state(
        intent: &NormalizedIntent,
        success_criteria: SuccessCriteria,
        constraints: Constraints,
        unresolved_decisions: UnresolvedDecisions,
    ) -> Result<Self, BuildLearnError> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for criterion in success_criteria.criteria() {
            if !seen.insert(criterion.id().as_str()) {
                return Err(BuildLearnError::DuplicateCriterion(
                    criterion.id().as_str().to_owned(),
                ));
            }
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for constraint in constraints.constraints() {
            if !seen.insert(constraint.id().as_str()) {
                return Err(BuildLearnError::DuplicateConstraint(
                    constraint.id().as_str().to_owned(),
                ));
            }
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for decision in unresolved_decisions.decisions() {
            if !seen.insert(decision.id().as_str()) {
                return Err(BuildLearnError::DuplicateDecision(
                    decision.id().as_str().to_owned(),
                ));
            }
        }
        Ok(Self {
            source: intent.source(),
            text: intent.capability().clone(),
            success_criteria,
            constraints,
            unresolved_decisions,
        })
    }

    /// Which of section 20.1's six kinds this goal came from.
    #[must_use]
    pub const fn source(&self) -> InputKind {
        self.source
    }

    /// The goal's own words.
    #[must_use]
    pub const fn text(&self) -> &NonEmptyText {
        &self.text
    }

    /// The success criteria. Never empty.
    #[must_use]
    pub const fn success_criteria(&self) -> &SuccessCriteria {
        &self.success_criteria
    }

    /// The constraints.
    #[must_use]
    pub const fn constraints(&self) -> &Constraints {
        &self.constraints
    }

    /// The unresolved decisions.
    #[must_use]
    pub const fn unresolved_decisions(&self) -> &UnresolvedDecisions {
        &self.unresolved_decisions
    }
}
