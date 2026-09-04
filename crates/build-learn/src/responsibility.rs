//! Section 20.2 step 1: `decompose into observable responsibilities`.
//!
//! ```text
//! desired capability
//!   ↓ decompose into observable responsibilities
//! architecture choices + constraints
//!   ↓
//! concept requirements with AND/OR branches
//! ```
//!
//! ## `responsibilities_precede_architecture_branch` is one by-value argument
//!
//! [`ResponsibilityDecomposition::decompose`] takes a [`crate::goal::ProjectGoal`]
//! **by value** and is the only producer of the type. [`crate::branch::ArchitectureBranch::of`]
//! takes a `ResponsibilityDecomposition` **by value** and is the only producer of
//! *that* type. So the arrow in the diagram is the ownership of a value, and a
//! branch stated before the responsibilities is a program that does not compile
//! rather than a rule somebody has to remember.
//!
//! ## A responsibility is about one criterion, and says what fails without it
//!
//! `observable` is the load-bearing word. [`ObservableResponsibility::of`]
//! requires the criterion it serves — one the goal actually holds, checked in
//! [`ResponsibilityDecomposition::decompose`] — and the failure that is visible
//! when the responsibility is absent. That is `P2-R4`'s
//! [`academic_repository_classification::ConcreteNeed`] shape: `구체적 책임 또는
//! 실패 시나리오`, not a topic.
//!
//! And the decomposition is total over the goal: every success criterion must be
//! served by at least one responsibility, or
//! [`crate::BuildLearnError::CriterionHasNoResponsibility`] names the one that is
//! not. A decomposition that quietly drops a criterion is what would let the plan
//! below it be complete about the wrong thing.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    BuildLearnError,
    goal::ProjectGoal,
    text::{NonEmptyText, PartId},
};

/// One observable responsibility the capability decomposes into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservableResponsibility {
    id: PartId,
    serves: PartId,
    statement: NonEmptyText,
    failure_if_absent: NonEmptyText,
}

impl ObservableResponsibility {
    /// Records one responsibility.
    #[must_use]
    pub const fn of(
        id: PartId,
        serves: PartId,
        statement: NonEmptyText,
        failure_if_absent: NonEmptyText,
    ) -> Self {
        Self {
            id,
            serves,
            statement,
            failure_if_absent,
        }
    }

    /// Its identity within the decomposition.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        &self.id
    }

    /// The success criterion it serves.
    #[must_use]
    pub const fn serves(&self) -> &PartId {
        &self.serves
    }

    /// What the system has to do.
    #[must_use]
    pub const fn statement(&self) -> &NonEmptyText {
        &self.statement
    }

    /// What is observably wrong when it is absent.
    #[must_use]
    pub const fn failure_if_absent(&self) -> &NonEmptyText {
        &self.failure_if_absent
    }
}

/// Section 20.2's first stage, over one goal.
///
/// Private fields, one producer, no `Default`. Holds the goal by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponsibilityDecomposition {
    goal: ProjectGoal,
    responsibilities: Vec<ObservableResponsibility>,
}

impl ResponsibilityDecomposition {
    /// Decomposes `goal` into observable responsibilities.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::DuplicateResponsibility`] when two share an identity;
    /// [`BuildLearnError::ResponsibilityServesNoCriterion`] when one names a
    /// criterion the goal does not hold; and
    /// [`BuildLearnError::CriterionHasNoResponsibility`] when a criterion of the
    /// goal is served by none.
    pub fn decompose(
        goal: ProjectGoal,
        responsibilities: Vec<ObservableResponsibility>,
    ) -> Result<Self, BuildLearnError> {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for responsibility in &responsibilities {
            if !seen.insert(responsibility.id().as_str()) {
                return Err(BuildLearnError::DuplicateResponsibility(
                    responsibility.id().as_str().to_owned(),
                ));
            }
            if goal
                .success_criteria()
                .criterion(responsibility.serves())
                .is_none()
            {
                return Err(BuildLearnError::ResponsibilityServesNoCriterion {
                    responsibility: responsibility.id().as_str().to_owned(),
                    criterion: responsibility.serves().as_str().to_owned(),
                });
            }
        }
        let served: BTreeSet<&str> = responsibilities
            .iter()
            .map(|item| item.serves().as_str())
            .collect();
        for criterion in goal.success_criteria().criteria() {
            if !served.contains(criterion.id().as_str()) {
                return Err(BuildLearnError::CriterionHasNoResponsibility(
                    criterion.id().as_str().to_owned(),
                ));
            }
        }
        Ok(Self {
            goal,
            responsibilities,
        })
    }

    /// The goal this decomposes.
    #[must_use]
    pub const fn goal(&self) -> &ProjectGoal {
        &self.goal
    }

    /// The responsibilities, in declaration order.
    #[must_use]
    pub fn responsibilities(&self) -> &[ObservableResponsibility] {
        &self.responsibilities
    }

    /// The responsibility `id` names.
    #[must_use]
    pub fn responsibility(&self, id: &PartId) -> Option<&ObservableResponsibility> {
        self.responsibilities.iter().find(|item| item.id() == id)
    }

    /// Every responsibility serving `criterion`, in declaration order.
    #[must_use]
    pub fn serving(&self, criterion: &PartId) -> Vec<&ObservableResponsibility> {
        self.responsibilities
            .iter()
            .filter(|item| item.serves() == criterion)
            .collect()
    }
}
