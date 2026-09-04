//! Planned coursework, and why it can only annotate an audit.
//!
//! Section 11.3 renders a planned-only course as `NOT_SATISFIED`, so a plan has
//! to be *visible* in an explanation. Section 6 says a `DegreeAuditAggregate`
//! is a reproducible proof tree over a `StudentProfile`, a `RequirementSet` and
//! a transcript snapshot -- three inputs, and a plan is not one of them. Both
//! hold here, and the way they both hold is that the plan never reaches the
//! audit at all:
//!
//! - [`crate::engine::DegreeAudit::evaluate`] **has no plan parameter**. There
//!   is no argument to pass one as, so no plan can move a measure, a status, or
//!   the audit's input binding.
//! - [`PlanAnnotatedView`] borrows a finished audit and a plan and produces
//!   labels. It has no method returning a `DegreeAudit`, no `&mut` borrow of
//!   one, and no method returning a `ProofStatus`, so an annotation cannot
//!   become a verdict.
//!
//! `plan_excluded_from_actual_audit` runs the same audit with and without a
//! plan and requires the whole `EngineOutcome` -- result, tree, explanation --
//! to be byte-identical, and then requires the annotation to have found the
//! planned course anyway. Without the second half the first would pass on an
//! annotation that found nothing.
//!
//! # Three layers, and this is only the outermost
//!
//! `P2-C7` sealed a projected value: `academic_scenario::Proposed<T>` has no
//! exit and this crate has no `academic-scenario` edge, so a projection is not
//! nameable from a product file here. `P2-U4` separated a plan from an attempt:
//! `PlanScenarioChoice` has no route to a `CourseAttempt` and
//! `AttemptStatus::Planned` has no producer. And `P2-U4`'s credit engine
//! reports a not-settled attempt as `NotEarned`, so even a planned row that
//! reached the ledger would earn no credit. This module adds the fourth thing:
//! the audit function has no plan argument.

use std::collections::BTreeMap;

use academic_record::{
    plan::{PlanScenario, PlanScenarioChoice},
    term::TermKey,
};

/// The coursework a plan scenario proposes.
///
/// Built from `P2-U4`'s `PlanScenario` and carrying only what a label needs:
/// which course, and for which term. It has no attempt identity, no grade and
/// no credits, because a proposal has no answer for any of them -- which is the
/// same absence `PlanScenarioChoice` already is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlannedCoursework {
    entries: BTreeMap<String, TermKey>,
}

impl PlannedCoursework {
    /// No planned coursework.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Reads one scenario's choices.
    #[must_use]
    pub fn from_scenario(scenario: &PlanScenario) -> Self {
        let mut entries = BTreeMap::new();
        for choice in scenario.choices() {
            entries.insert(
                PlanScenarioChoice::course_code(choice).to_owned(),
                choice.intended_term(),
            );
        }
        Self { entries }
    }

    /// The term a course is planned for, when it is planned.
    #[must_use]
    pub fn intended_term(&self, course_code: &str) -> Option<TermKey> {
        self.entries.get(course_code).copied()
    }

    /// Every planned course code, in order.
    pub fn course_codes(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Whether anything is planned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What a plan says about one unsatisfied leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanNote {
    /// Section 11.3's *planned only*: the course is in the plan and in no
    /// settled attempt that earned credit.
    PlannedOnly {
        /// The term the plan proposes.
        intended_term: TermKey,
    },
    /// The plan says nothing about this leaf.
    NotPlanned,
}

impl PlanNote {
    /// The stable spelling a view renders.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlannedOnly { .. } => "PLANNED_ONLY",
            Self::NotPlanned => "NOT_PLANNED",
        }
    }
}

/// A read-only labelling of a finished audit against a plan.
///
/// It borrows both. There is no method here that returns a
/// [`crate::engine::DegreeAudit`], no `&mut` borrow of one, and no method that
/// returns a `ProofStatus` or a `Measure`: the only thing this view produces is
/// a [`PlanNote`], and a note is not a verdict.
#[derive(Debug, Clone, Copy)]
pub struct PlanAnnotatedView<'audit> {
    audit: &'audit crate::engine::DegreeAudit,
    plan: &'audit PlannedCoursework,
}

impl<'audit> PlanAnnotatedView<'audit> {
    /// Labels one audit against one plan.
    #[must_use]
    pub const fn new(
        audit: &'audit crate::engine::DegreeAudit,
        plan: &'audit PlannedCoursework,
    ) -> Self {
        Self { audit, plan }
    }

    /// What the plan says about one course.
    ///
    /// `PlannedOnly` only when the transcript holds no entry for the course
    /// that earned credit: a course that is both planned and already passed is
    /// not planned-only, and labelling it so would tell a user their completed
    /// work is a proposal.
    #[must_use]
    pub fn note_for(&self, course_code: &str) -> PlanNote {
        let earned = self.audit.transcript().entries().iter().any(|entry| {
            entry.course_code() == course_code
                && matches!(
                    entry.admission(),
                    crate::transcript::EntryAdmission::Counted { .. }
                )
        });
        match (earned, self.plan.intended_term(course_code)) {
            (false, Some(intended_term)) => PlanNote::PlannedOnly { intended_term },
            _ => PlanNote::NotPlanned,
        }
    }

    /// Every planned-only course, in plan order.
    #[must_use]
    pub fn planned_only(&self) -> Vec<&'audit str> {
        self.plan
            .course_codes()
            .filter(|code| matches!(self.note_for(code), PlanNote::PlannedOnly { .. }))
            .collect()
    }
}
