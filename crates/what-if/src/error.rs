//! Typed what-if failures.
//!
//! Every fallible path in this crate returns one of these. `clippy::panic`,
//! `unwrap_used` and `expect_used` are denied workspace-wide, and a simulator
//! that panicked on a malformed assumption would take the process down over a
//! value the user typed into a plan.

use academic_critical_path::CriticalPathError;
use academic_curriculum::CurriculumError;
use academic_domain::{DomainError, OfferingId};
use academic_record::RecordError;
use academic_review::ReviewError;
use academic_scenario::ScenarioError;
use thiserror::Error;

use academic_domain::EntityId;

use crate::{comparison::ComparisonDimension, lane::DeterministicItem};

/// A rejected plan input, or a refused simulator operation.
///
/// `RecordError` is neither `Clone` nor `Eq`, so neither is this, for the
/// reason `P2-U3` gives: a test that wants to say which refusal it got matches
/// on the variant, and a derive that forced `P2-U4` to grow two traits for the
/// convenience would be this crate reaching across a boundary.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WhatIfError {
    /// A plan named no offering to evaluate.
    #[error("a plan chooses at least one offering")]
    EmptyPlan,
    /// A plan named the same offering twice.
    #[error("a plan chose offering {0} more than once")]
    DuplicateChoice(OfferingId),
    /// A relevance reading named an offering the plan does not choose.
    #[error("a relevance reading names offering {0}, which this plan does not choose")]
    RelevanceOutsidePlan(OfferingId),
    /// A path answer named one concept twice.
    #[error("the critical path names concept {0} more than once")]
    DuplicatePathTarget(EntityId),
    /// A grade assumption named an offering the plan does not choose.
    ///
    /// Section 22.2 admits a GPA scenario only under grades the user stated,
    /// and a grade stated about something the plan does not contain is not a
    /// stated grade for anything the plan does.
    #[error("a grade assumption names offering {0}, which this plan does not choose")]
    GradeAssumptionOutsidePlan(OfferingId),
    /// A stated grade set left one of the plan's choices unstated.
    ///
    /// Refused rather than filled in. A GPA over the subset the user happened
    /// to state is a number nobody asked for, and section 22.2's `한해서만` is
    /// the whole of the permission.
    #[error("no grade was stated for offering {0}, so this plan has no GPA scenario")]
    GradeAssumptionMissing(OfferingId),
    /// The same offering was given two stated grades.
    #[error("offering {0} was given more than one stated grade")]
    DuplicateGradeAssumption(OfferingId),
    /// A recomputation consent named a different plan.
    #[error("this consent names another plan")]
    ConsentNamesAnotherPlan,
    /// A recomputation consent did not cover every stale input.
    #[error("this consent does not cover every stale input the plan is frozen on")]
    ConsentIsIncomplete,
    /// A comparison priority that is not a complete permutation.
    #[error("a comparison priority orders every section 22.4 dimension exactly once")]
    PriorityIsNotAPermutation,
    /// A reordering explanation was asked for between two identical priorities.
    ///
    /// Section 22.4 requires the product to say *why the order changed*. There
    /// is no changed weight to name when nothing changed, so the caller gets a
    /// refusal rather than an explanation with an empty reason.
    #[error("a reordering explanation needs a priority that actually changed")]
    PriorityDidNotChange,
    /// A comparison was asked for over fewer than two plans.
    #[error("a comparison ranks at least two plans")]
    ComparisonNeedsTwoPlans,
    /// Two plans in one comparison carried the same identity.
    #[error("a comparison holds each plan once")]
    DuplicatePlanInComparison,
    /// A calibration was asked to compare a term against another term's plan.
    #[error("an end-of-term calibration reads the plan it is the term of")]
    CalibrationNamesAnotherPlan,
    /// A dimension was asked for a lane it does not sit in.
    #[error("section 22.4 dimension {dimension:?} does not sit in one lane alone")]
    DimensionIsMixed {
        /// The dimension.
        dimension: ComparisonDimension,
    },
    /// A deterministic item was read out of a plan that refused to produce it.
    #[error("this plan produced no value for deterministic item {item:?}")]
    DeterministicItemAbsent {
        /// The item.
        item: DeterministicItem,
    },
    /// `P2-N6` refused a value.
    #[error(transparent)]
    CriticalPath(#[from] CriticalPathError),
    /// `P2-U1` refused a value.
    #[error(transparent)]
    Curriculum(#[from] CurriculumError),
    /// `P2-C1` refused a value.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// `P2-U4` refused a value.
    #[error(transparent)]
    Record(#[from] RecordError),
    /// `P2-U8` refused a value.
    #[error(transparent)]
    Review(#[from] ReviewError),
    /// `P2-C7` refused a value.
    #[error(transparent)]
    Scenario(#[from] ScenarioError),
}
