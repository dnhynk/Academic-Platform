//! Section 20.2's `OS가 긴 강의 목록만 제시해 build 동기를 끊지 않는다`, as a
//! structural refusal rather than a phrase list.
//!
//! ## `lecture_list_only_plan_fails_validation`
//!
//! The plan this sentence refuses is one where every step is something to study
//! and nothing is built. What makes that refusable without reading any word is
//! that section 20.2 already says what a plan is *for*: the goal's success
//! criteria, reached by implementation, with each learning item returning to
//! one. So the validator reads four structural facts and no text at all:
//!
//! | [`PlanDefect`] | What is structurally absent |
//! |---|---|
//! | [`PlanDefect::NoImplementationStep`] | no step of the plan builds anything |
//! | [`PlanDefect::CriterionReachedByNoImplementation`] | a success criterion no implementation step moves toward |
//! | [`PlanDefect::CheckpointReturnsToNoStep`] | a return checkpoint naming a step the plan does not have |
//! | [`PlanDefect::CheckpointReturnsToNonImplementation`] | a return checkpoint that returns to more studying |
//!
//! Four more hold the joins that would otherwise let a plan be complete about
//! the wrong thing: [`PlanDefect::LearningItemIsAboutNoRequirement`],
//! [`PlanDefect::RequirementHasNoFinding`],
//! [`PlanDefect::AcquisitionNeededButNoStep`] and
//! [`PlanDefect::ExperimentAnswersNoDecision`].
//!
//! ## No phrase list, and that is measured
//!
//! This is `P2-N5`'s
//! [`academic_gap::SpecificityDefect`] discipline: `the_build_learn_crate_holds_no_phrase_list`
//! observes that the product sources contain no string literal long enough to be
//! a phrase to match against, outside the design document's own quoted cells,
//! and `a_fluent_lecture_list_plan_fails_validation` drives a plan whose every
//! step is well-formed, whose evidence tasks and four-stage checkpoints are all
//! real, whose wording uses none of the design document's words — and observes
//! the same three defects. A validator that keyed on a word would pass it.
//!
//! `P2-R4`'s `generic_nice_to_have_list_produces_zero_findings` and `P2-N5`'s
//! specificity validator draw the same line one and two layers down.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use academic_domain::EntityId;

use crate::{
    plan::{PlanDraft, PlanStep},
    readiness::ReadinessCategory,
    text::PartId,
};

/// What a plan is structurally missing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanDefect {
    /// No step of the plan builds anything.
    NoImplementationStep,
    /// A success criterion of the goal that no implementation step moves toward.
    CriterionReachedByNoImplementation {
        /// The criterion.
        criterion: PartId,
    },
    /// A return checkpoint naming a step the plan does not have.
    CheckpointReturnsToNoStep {
        /// The learning item.
        item: PartId,
        /// The step it names.
        step: PartId,
    },
    /// A return checkpoint that returns to something other than building.
    CheckpointReturnsToNonImplementation {
        /// The learning item.
        item: PartId,
        /// The step it returns to.
        step: PartId,
    },
    /// A learning item about a concept no requirement of the branch names.
    LearningItemIsAboutNoRequirement {
        /// The learning item.
        item: PartId,
    },
    /// A requirement of the branch with no readiness finding.
    RequirementHasNoFinding {
        /// The concept.
        concept: String,
    },
    /// A requirement the user is not ready for, with no step that acquires it.
    AcquisitionNeededButNoStep {
        /// The concept.
        concept: String,
        /// The category it landed in.
        category: ReadinessCategory,
    },
    /// An experiment step naming a decision the goal did not leave open.
    ExperimentAnswersNoDecision {
        /// The step.
        step: PartId,
        /// The decision it names.
        decision: PartId,
    },
}

impl PlanDefect {
    /// Stable spelling of the kind, without its payload.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoImplementationStep => "NO_IMPLEMENTATION_STEP",
            Self::CriterionReachedByNoImplementation { .. } => {
                "CRITERION_REACHED_BY_NO_IMPLEMENTATION"
            }
            Self::CheckpointReturnsToNoStep { .. } => "CHECKPOINT_RETURNS_TO_NO_STEP",
            Self::CheckpointReturnsToNonImplementation { .. } => {
                "CHECKPOINT_RETURNS_TO_NON_IMPLEMENTATION"
            }
            Self::LearningItemIsAboutNoRequirement { .. } => {
                "LEARNING_ITEM_IS_ABOUT_NO_REQUIREMENT"
            }
            Self::RequirementHasNoFinding { .. } => "REQUIREMENT_HAS_NO_FINDING",
            Self::AcquisitionNeededButNoStep { .. } => "ACQUISITION_NEEDED_BUT_NO_STEP",
            Self::ExperimentAnswersNoDecision { .. } => "EXPERIMENT_ANSWERS_NO_DECISION",
        }
    }
}

/// Every defect kind, in the order [`validate`] can emit them.
pub const PLAN_DEFECT_KINDS: [&str; 8] = [
    "NO_IMPLEMENTATION_STEP",
    "CRITERION_REACHED_BY_NO_IMPLEMENTATION",
    "CHECKPOINT_RETURNS_TO_NO_STEP",
    "CHECKPOINT_RETURNS_TO_NON_IMPLEMENTATION",
    "LEARNING_ITEM_IS_ABOUT_NO_REQUIREMENT",
    "REQUIREMENT_HAS_NO_FINDING",
    "ACQUISITION_NEEDED_BUT_NO_STEP",
    "EXPERIMENT_ANSWERS_NO_DECISION",
];

/// A plan that passed [`validate`].
///
/// Private fields, one producer, no `Default`. The only way to hold one is to
/// have had a draft answered about, which is why nothing downstream needs to
/// remember to run the validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedPlan {
    steps: Vec<PlanStep>,
}

impl ValidatedPlan {
    /// The steps, in the order the draft presented them.
    #[must_use]
    pub fn steps(&self) -> &[PlanStep] {
        &self.steps
    }
}

/// What one validation run found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanVerdict {
    /// The plan is publishable.
    Accepted(ValidatedPlan),
    /// The plan is not, and these are the reasons, in [`PLAN_DEFECT_KINDS`]'
    /// order.
    Refused(Vec<PlanDefect>),
}

impl PlanVerdict {
    /// The defects, empty when the plan was accepted.
    #[must_use]
    pub fn defects(&self) -> &[PlanDefect] {
        match self {
            Self::Accepted(_) => &[],
            Self::Refused(defects) => defects,
        }
    }

    /// The validated plan, when there was one.
    #[must_use]
    pub const fn plan(&self) -> Option<&ValidatedPlan> {
        match self {
            Self::Accepted(plan) => Some(plan),
            Self::Refused(_) => None,
        }
    }

    /// Whether the plan was accepted.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }
}

/// Section 20.2's plan validation, over one draft.
///
/// Every defect the draft has is reported, not the first: a plan with three
/// things structurally missing is more usefully described by all three than by
/// whichever the loop reached first.
#[must_use]
pub fn validate(draft: &PlanDraft<'_>) -> PlanVerdict {
    let mut defects: Vec<PlanDefect> = Vec::new();

    let implementation: BTreeMap<&str, &PlanStep> = draft
        .steps
        .iter()
        .filter(|step| step.satisfies().is_some())
        .map(|step| (step.id().as_str(), step))
        .collect();
    if implementation.is_empty() {
        defects.push(PlanDefect::NoImplementationStep);
    }

    let reached: BTreeSet<&str> = implementation
        .values()
        .filter_map(|step| step.satisfies())
        .map(PartId::as_str)
        .collect();
    for criterion in draft.branch.goal().success_criteria().criteria() {
        if !reached.contains(criterion.id().as_str()) {
            defects.push(PlanDefect::CriterionReachedByNoImplementation {
                criterion: criterion.id().clone(),
            });
        }
    }

    let step_ids: BTreeSet<&str> = draft.steps.iter().map(|step| step.id().as_str()).collect();
    for item in draft.learning_items() {
        let target = item.checkpoint().returns_to();
        if !step_ids.contains(target.as_str()) {
            defects.push(PlanDefect::CheckpointReturnsToNoStep {
                item: item.id().clone(),
                step: target.clone(),
            });
        } else if !implementation.contains_key(target.as_str()) {
            defects.push(PlanDefect::CheckpointReturnsToNonImplementation {
                item: item.id().clone(),
                step: target.clone(),
            });
        }
    }

    let required: BTreeSet<EntityId> = draft
        .branch
        .requirements()
        .iter()
        .map(|requirement| requirement.concept())
        .collect();
    for item in draft.learning_items() {
        if !required.contains(&item.concept()) {
            defects.push(PlanDefect::LearningItemIsAboutNoRequirement {
                item: item.id().clone(),
            });
        }
    }

    let found: BTreeSet<EntityId> = draft
        .findings
        .iter()
        .map(|finding| finding.requirement().concept())
        .collect();
    for requirement in draft.branch.requirements() {
        if !found.contains(&requirement.concept()) {
            defects.push(PlanDefect::RequirementHasNoFinding {
                concept: requirement.concept().to_string(),
            });
        }
    }

    let acquired: BTreeSet<EntityId> = draft
        .learning_items()
        .iter()
        .map(|item| item.concept())
        .collect();
    for finding in draft.findings {
        if finding.category() == ReadinessCategory::AlreadyReady {
            continue;
        }
        if !acquired.contains(&finding.requirement().concept()) {
            defects.push(PlanDefect::AcquisitionNeededButNoStep {
                concept: finding.requirement().concept().to_string(),
                category: finding.category(),
            });
        }
    }

    for step in draft.steps {
        let Some(decision) = step.answers() else {
            continue;
        };
        if draft
            .branch
            .goal()
            .unresolved_decisions()
            .decision(decision)
            .is_none()
        {
            defects.push(PlanDefect::ExperimentAnswersNoDecision {
                step: step.id().clone(),
                decision: decision.clone(),
            });
        }
    }

    if defects.is_empty() {
        PlanVerdict::Accepted(ValidatedPlan {
            steps: draft.steps.to_vec(),
        })
    } else {
        defects.sort_by_key(|defect| {
            PLAN_DEFECT_KINDS
                .iter()
                .position(|kind| *kind == defect.as_str())
                .unwrap_or(PLAN_DEFECT_KINDS.len())
        });
        PlanVerdict::Refused(defects)
    }
}
