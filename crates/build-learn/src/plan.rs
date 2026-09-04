//! Section 20.2's last stage: `learning + experiment + implementation
//! checkpoints`, and the plan they make up.
//!
//! Three step kinds and no fourth, which is REQ-20-007's own list. A learning
//! step is a [`crate::learning::LearningItem`], so it carries its evidence task
//! and its return checkpoint by construction; an experiment step is a small run
//! that answers one open decision; an implementation step is the one kind that
//! satisfies a success criterion, and it is what a return checkpoint has to
//! return to. See [`crate::validate`].

use academic_domain::EntityId;
use serde::{Deserialize, Serialize};

use crate::{
    branch::ArchitectureBranch,
    learning::LearningItem,
    motivation::MotivationDisplay,
    readiness::ReadinessFinding,
    text::{NonEmptyText, PartId},
};

/// One step of a build-to-learn plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanStep {
    /// A learning item, with its evidence task and return checkpoint.
    Learning(LearningItem),
    /// A small run that answers one of the goal's open decisions.
    Experiment {
        /// Its identity within the plan.
        id: PartId,
        /// The decision it answers.
        answers: PartId,
        /// What is run.
        runs: NonEmptyText,
    },
    /// A step that builds part of the project.
    Implementation {
        /// Its identity within the plan.
        id: PartId,
        /// The success criterion it moves toward.
        satisfies: PartId,
        /// What is built.
        builds: NonEmptyText,
    },
}

/// The three step kinds, in REQ-20-007's own order.
pub const STEP_KINDS: [&str; 3] = ["LEARNING", "EXPERIMENT", "IMPLEMENTATION"];

impl PlanStep {
    /// Its identity within the plan.
    #[must_use]
    pub const fn id(&self) -> &PartId {
        match self {
            Self::Learning(item) => item.id(),
            Self::Experiment { id, .. } | Self::Implementation { id, .. } => id,
        }
    }

    /// Stable spelling of the kind.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Learning(_) => "LEARNING",
            Self::Experiment { .. } => "EXPERIMENT",
            Self::Implementation { .. } => "IMPLEMENTATION",
        }
    }

    /// The learning item, when this step is one.
    #[must_use]
    pub const fn learning(&self) -> Option<&LearningItem> {
        match self {
            Self::Learning(item) => Some(item),
            Self::Experiment { .. } | Self::Implementation { .. } => None,
        }
    }

    /// The success criterion this step moves toward, when it builds.
    #[must_use]
    pub const fn satisfies(&self) -> Option<&PartId> {
        match self {
            Self::Implementation { satisfies, .. } => Some(satisfies),
            Self::Learning(_) | Self::Experiment { .. } => None,
        }
    }

    /// The decision this step answers, when it is an experiment.
    #[must_use]
    pub const fn answers(&self) -> Option<&PartId> {
        match self {
            Self::Experiment { answers, .. } => Some(answers),
            Self::Learning(_) | Self::Implementation { .. } => None,
        }
    }
}

/// One build-to-learn plan: the branch it was derived from, the readiness
/// findings over it, the steps, and the motivation rows shown beside them.
///
/// Public fields, the way `P2-R4`'s `ClassificationInput` has them: this is the
/// argument list of [`crate::validate::validate`], and a plan is not published
/// until that function has answered about it. The published value is
/// [`crate::validate::ValidatedPlan`], which has private fields and one
/// producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDraft<'a> {
    /// The AND/OR requirements the plan is about.
    pub branch: &'a ArchitectureBranch,
    /// One finding per requirement, in the branch's order.
    pub findings: &'a [ReadinessFinding],
    /// The steps, in the order they are presented.
    pub steps: &'a [PlanStep],
    /// The parallel motivation rows, one display per concept that has any.
    pub motivations: &'a [MotivationDisplay],
}

impl PlanDraft<'_> {
    /// Every implementation step of the draft, in order.
    #[must_use]
    pub fn implementation_steps(&self) -> Vec<&PlanStep> {
        self.steps
            .iter()
            .filter(|step| step.satisfies().is_some())
            .collect()
    }

    /// Every learning item of the draft, in order.
    #[must_use]
    pub fn learning_items(&self) -> Vec<&LearningItem> {
        self.steps.iter().filter_map(PlanStep::learning).collect()
    }

    /// The motivation display for `concept`, when the draft carries one.
    #[must_use]
    pub fn motivation(&self, concept: EntityId) -> Option<&MotivationDisplay> {
        self.motivations
            .iter()
            .find(|display| display.concept() == concept)
    }
}
