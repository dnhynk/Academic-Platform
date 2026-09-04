//! Section 22.2's lane: the seven things a plan can state as fact, and the one
//! assumption each of them still rests on.
//!
//! Every value here is a function of the frozen inputs and of nothing else.
//! There is no clock, no RNG and no ambient state in this module, so two runs
//! over one [`crate::inputs::PlanInputs`] agree byte for byte.
//!
//! # Deterministic is not the same as unconditional
//!
//! Section 22.2's fourth bullet says *이수한다고 **가정했을 때의**", and its
//! last says *사용자가 명시한 grade 가정에 **한해서만**". Both conditions are
//! parameters rather than flags: [`RuleContribution::under`] takes
//! [`crate::assumption::HypotheticalCompletion`] by value, and
//! [`GpaScenario::under`] takes [`crate::assumption::StatedGradeAssumptions`]
//! by reference and refuses a set that leaves any choice unstated. A plan with
//! no stated grades therefore has `None` in [`DeterministicResults::gpa`],
//! which is not zero and is not an average over the part the user typed.
//!
//! # An official timetable conflict needs an official timetable
//!
//! [`ScheduleConflicts::of`] takes `P2-U5`'s
//! [`academic_offering::ConfirmedSeat`] values. That type's one producer is
//! `ConfirmedStanding::seat`, and section 8.3's `HISTORICALLY_LIKELY` row —
//! *placeholder만, 졸업계획 확정에 사용 금지* — has no seat to hand it. So the
//! deterministic lane cannot be computed over a predicted offering: not because
//! a check refuses it, but because the argument does not exist.
//!
//! # `Unknown` is never a pass
//!
//! [`EnrolmentLimitStanding::Unknown`] is what an unrecorded enrolment
//! restriction reads as, and [`PrerequisiteStanding::verdict`] maps it to
//! [`RequirementVerdict::Unknown`] rather than to `Met`. That is `P2-N6`'s
//! `OfficialPrerequisiteStanding::Unknown` rule in this lane: section 28's
//! `OFFICIAL_PREREQUISITE` engine is `PLANNED`, so nothing here may conclude
//! that a registrar would admit the registration.

use std::collections::{BTreeMap, BTreeSet};

use academic_curriculum::{Credits, CurriculumCategory, Meeting, Weekday};
use academic_domain::{CourseId, Decimal, OfferingId};
use academic_offering::ConfirmedSeat;
use academic_record::{decimal, grade::GradingScheme};

use crate::{
    assumption::{HypotheticalCompletion, StatedGradeAssumptions},
    error::WhatIfError,
    inputs::{DownstreamCourse, PlanChoice},
    lane::DeterministicItem,
};

/// Section 22.2's first bullet: the requested load and its per-course lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditLoad {
    lines: Vec<(OfferingId, Credits)>,
    requested: u16,
}

impl CreditLoad {
    /// Adds the plan's per-course credits up.
    ///
    /// The total is a `u16` over a catalogue value bounded at thirty credits,
    /// so a plan would need more than two thousand choices to overflow it; the
    /// addition is saturating anyway, because a wrapped total would render as a
    /// light semester and that is the one reading that must never happen.
    pub(crate) fn of(choices: &[PlanChoice]) -> Self {
        let mut lines = Vec::with_capacity(choices.len());
        let mut requested: u16 = 0;
        for choice in choices {
            lines.push((choice.offering_id(), choice.credits()));
            requested = requested.saturating_add(u16::from(choice.credits().value()));
        }
        Self { lines, requested }
    }

    /// The per-course credits, in the plan's own order.
    #[must_use]
    pub fn lines(&self) -> &[(OfferingId, Credits)] {
        &self.lines
    }

    /// The total requested credits.
    #[must_use]
    pub const fn requested(&self) -> u16 {
        self.requested
    }
}

/// One official timetable collision between two choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScheduleConflict {
    earlier_choice: OfferingId,
    later_choice: OfferingId,
    weekday: Weekday,
    from_minute: u16,
    to_minute: u16,
}

impl ScheduleConflict {
    /// The choice that appears first in the plan.
    #[must_use]
    pub const fn earlier_choice(&self) -> OfferingId {
        self.earlier_choice
    }

    /// The choice that appears second in the plan.
    #[must_use]
    pub const fn later_choice(&self) -> OfferingId {
        self.later_choice
    }

    /// The weekday the two meet on.
    #[must_use]
    pub const fn weekday(&self) -> Weekday {
        self.weekday
    }

    /// The first overlapping minute, inclusive.
    #[must_use]
    pub const fn from_minute(&self) -> u16 {
        self.from_minute
    }

    /// The last overlapping minute, exclusive.
    #[must_use]
    pub const fn to_minute(&self) -> u16 {
        self.to_minute
    }
}

/// Section 22.2's second bullet: every official collision the plan contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleConflicts {
    conflicts: Vec<ScheduleConflict>,
}

impl ScheduleConflicts {
    /// Finds every overlap between the plan's confirmed timetables.
    ///
    /// A meeting is half-open, so a class that ends when another begins does
    /// not conflict with it. That is `P2-U1`'s own reading of section 8.2's
    /// `meetings`, reused rather than restated.
    pub(crate) fn of(choices: &[PlanChoice]) -> Self {
        let mut conflicts = Vec::new();
        for (index, left) in choices.iter().enumerate() {
            for right in &choices[index + 1..] {
                for left_meeting in seat_meetings(left.seat()) {
                    for right_meeting in seat_meetings(right.seat()) {
                        if let Some(overlap) = overlap(*left_meeting, *right_meeting) {
                            conflicts.push(ScheduleConflict {
                                earlier_choice: left.offering_id(),
                                later_choice: right.offering_id(),
                                weekday: overlap.0,
                                from_minute: overlap.1,
                                to_minute: overlap.2,
                            });
                        }
                    }
                }
            }
        }
        conflicts.sort_unstable();
        Self { conflicts }
    }

    /// Every conflict, in a stable order.
    #[must_use]
    pub fn conflicts(&self) -> &[ScheduleConflict] {
        &self.conflicts
    }

    /// Whether the plan's official timetables collide anywhere.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.conflicts.is_empty()
    }
}

fn seat_meetings(seat: &ConfirmedSeat) -> &[Meeting] {
    seat.meetings()
}

/// The overlapping window of two meetings, when they overlap.
fn overlap(left: Meeting, right: Meeting) -> Option<(Weekday, u16, u16)> {
    if left.weekday() != right.weekday() {
        return None;
    }
    let from = left.from_minute().max(right.from_minute());
    let to = left.to_minute().min(right.to_minute());
    if from < to {
        Some((left.weekday(), from, to))
    } else {
        None
    }
}

/// What an official reading says about one choice's enrolment restriction.
///
/// `Unknown` is what an unrecorded restriction reads as. It is not a default
/// and it is never a pass: see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EnrolmentLimitStanding {
    /// The official reading records that the restriction is satisfied.
    Satisfied,
    /// The official reading records that it is not.
    NotSatisfied,
    /// No official reading of the restriction was recorded.
    Unknown,
}

/// Exhaustive listing, in the order a reader should least prefer last.
pub const ENROLMENT_LIMIT_STANDINGS: [EnrolmentLimitStanding; 3] = [
    EnrolmentLimitStanding::Satisfied,
    EnrolmentLimitStanding::NotSatisfied,
    EnrolmentLimitStanding::Unknown,
];

impl EnrolmentLimitStanding {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "SATISFIED",
            Self::NotSatisfied => "NOT_SATISFIED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Whether one choice's official conditions are met.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequirementVerdict {
    /// Every official prerequisite is completed and the restriction is
    /// satisfied.
    Met,
    /// A prerequisite is missing, or the restriction is not satisfied.
    NotMet,
    /// Nothing contradicts the registration and something is unrecorded.
    Unknown,
}

/// Section 22.2's third bullet, for one choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrerequisiteStanding {
    offering_id: OfferingId,
    unmet: Vec<CourseId>,
    enrolment_limit: EnrolmentLimitStanding,
}

impl PrerequisiteStanding {
    /// Reads one choice's official conditions against the completed set.
    ///
    /// The completed set is a frozen input drawn from the record snapshot the
    /// plan's basis names. This crate does not read the record: it reads what
    /// the caller froze, and the basis digest is what says which record that
    /// was.
    pub(crate) fn of(choice: &PlanChoice, completed: &BTreeSet<CourseId>) -> Self {
        let mut unmet: Vec<CourseId> = choice
            .official_prerequisites()
            .iter()
            .map(|prerequisite| prerequisite.course())
            .filter(|course| !completed.contains(course))
            .collect();
        unmet.sort_unstable();
        unmet.dedup();
        Self {
            offering_id: choice.offering_id(),
            unmet,
            enrolment_limit: choice.enrolment_limit(),
        }
    }

    /// Which choice.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The official prerequisites the completed set does not hold.
    #[must_use]
    pub fn unmet(&self) -> &[CourseId] {
        &self.unmet
    }

    /// The enrolment restriction reading.
    #[must_use]
    pub const fn enrolment_limit(&self) -> EnrolmentLimitStanding {
        self.enrolment_limit
    }

    /// The verdict.
    ///
    /// `NotMet` outranks `Unknown`: a known failure is a conclusion, and an
    /// unrecorded restriction beside it does not soften it.
    #[must_use]
    pub fn verdict(&self) -> RequirementVerdict {
        if !self.unmet.is_empty() || self.enrolment_limit == EnrolmentLimitStanding::NotSatisfied {
            return RequirementVerdict::NotMet;
        }
        match self.enrolment_limit {
            EnrolmentLimitStanding::Satisfied => RequirementVerdict::Met,
            EnrolmentLimitStanding::NotSatisfied | EnrolmentLimitStanding::Unknown => {
                RequirementVerdict::Unknown
            }
        }
    }
}

/// One category's credit contribution under the completion assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CategoryContribution {
    category: CurriculumCategory,
    credits: u16,
}

impl CategoryContribution {
    /// Which category.
    #[must_use]
    pub const fn category(&self) -> CurriculumCategory {
        self.category
    }

    /// How many credits the plan would put in it.
    #[must_use]
    pub const fn credits(&self) -> u16 {
        self.credits
    }
}

/// Section 22.2's fourth bullet: what the plan contributes *if completed*.
///
/// Section 22.4's first row is `전선 +9`, which is a credit delta against a
/// category, so that is what this carries. Private fields, and the only
/// constructor takes [`HypotheticalCompletion`] by value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleContribution {
    contributions: Vec<CategoryContribution>,
    completion: HypotheticalCompletion,
}

impl RuleContribution {
    /// Totals the plan's credits per category, under the stated assumption.
    pub(crate) fn under(choices: &[PlanChoice], completion: HypotheticalCompletion) -> Self {
        let mut totals: BTreeMap<CurriculumCategory, u16> = BTreeMap::new();
        for choice in choices {
            let entry = totals.entry(choice.category()).or_default();
            *entry = entry.saturating_add(u16::from(choice.credits().value()));
        }
        Self {
            contributions: totals
                .into_iter()
                .map(|(category, credits)| CategoryContribution { category, credits })
                .collect(),
            completion,
        }
    }

    /// Every category the plan touches, in the category enumeration's order.
    #[must_use]
    pub fn contributions(&self) -> &[CategoryContribution] {
        &self.contributions
    }

    /// The assumption this contribution rests on.
    ///
    /// It is carried on the value rather than stated in the documentation, so a
    /// renderer that shows a contribution has the assumption in its hand.
    #[must_use]
    pub const fn completion(&self) -> HypotheticalCompletion {
        self.completion
    }
}

/// One line of the allocation proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AllocationLine {
    offering_id: OfferingId,
    course: CourseId,
    category: CurriculumCategory,
    credits: Credits,
}

impl AllocationLine {
    /// Which choice.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The course behind the choice.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// Which bucket the credits were put in.
    #[must_use]
    pub const fn category(&self) -> CurriculumCategory {
        self.category
    }

    /// How many credits.
    #[must_use]
    pub const fn credits(&self) -> Credits {
        self.credits
    }
}

/// Section 22.2's fifth bullet: the allocation, and the proof of it.
///
/// The proof is not a rendering of the totals: it is the per-choice lines the
/// totals were added from. Section 22.4's first row shows `rule proof 열기`
/// beside the number, and a proof that could not be opened onto individual
/// choices would be the number a second time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    lines: Vec<AllocationLine>,
}

impl Allocation {
    /// Records one line per choice.
    pub(crate) fn of(choices: &[PlanChoice]) -> Self {
        Self {
            lines: choices
                .iter()
                .map(|choice| AllocationLine {
                    offering_id: choice.offering_id(),
                    course: choice.course(),
                    category: choice.category(),
                    credits: choice.credits(),
                })
                .collect(),
        }
    }

    /// Every line, in the plan's own order.
    #[must_use]
    pub fn lines(&self) -> &[AllocationLine] {
        &self.lines
    }

    /// The lines behind one category's total.
    #[must_use]
    pub fn proof_for(&self, category: CurriculumCategory) -> Vec<AllocationLine> {
        self.lines
            .iter()
            .filter(|line| line.category() == category)
            .copied()
            .collect()
    }
}

/// What this plan would do to one downstream course's official prerequisites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockStanding {
    /// The completed set already satisfies it; the plan is not what unlocks it.
    AlreadyUnlocked,
    /// The plan's choices, if completed, would satisfy the last of them.
    UnlockedByThisPlan,
    /// Prerequisites remain that neither the record nor the plan supplies.
    StillBlocked {
        /// The courses still outstanding, in identifier order.
        remaining: Vec<CourseId>,
    },
}

/// Section 22.2's sixth bullet, for one downstream course.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownstreamUnlock {
    course: CourseId,
    standing: UnlockStanding,
    completion: HypotheticalCompletion,
}

impl DownstreamUnlock {
    /// Reads one downstream course against the record and the plan.
    pub(crate) fn of(
        downstream: &DownstreamCourse,
        completed: &BTreeSet<CourseId>,
        planned: &BTreeSet<CourseId>,
        completion: HypotheticalCompletion,
    ) -> Self {
        let required: BTreeSet<CourseId> = downstream
            .official_prerequisites()
            .iter()
            .map(|prerequisite| prerequisite.course())
            .collect();
        let standing = if required.iter().all(|course| completed.contains(course)) {
            UnlockStanding::AlreadyUnlocked
        } else if required
            .iter()
            .all(|course| completed.contains(course) || planned.contains(course))
        {
            UnlockStanding::UnlockedByThisPlan
        } else {
            UnlockStanding::StillBlocked {
                remaining: required
                    .into_iter()
                    .filter(|course| !completed.contains(course) && !planned.contains(course))
                    .collect(),
            }
        };
        Self {
            course: downstream.course(),
            standing,
            completion,
        }
    }

    /// Which downstream course.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// What the plan would do to it.
    #[must_use]
    pub const fn standing(&self) -> &UnlockStanding {
        &self.standing
    }

    /// The assumption the standing rests on.
    #[must_use]
    pub const fn completion(&self) -> HypotheticalCompletion {
        self.completion
    }
}

/// Why a stated grade did not reach the average.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AverageExclusion {
    /// The grading scheme excludes this symbol from the average — `S`/`U` and
    /// the withdrawal and incomplete symbols.
    NotGradedInScheme,
}

/// A hypothetical term average, under grades the user stated.
///
/// A deliberately different type from `academic_record::views::GpaValue`, and
/// deliberately not reachable from one. That crate's value is the average of
/// attempts that happened; this one is the average of grades nobody has yet
/// received. A shared type would have made the two interchangeable at exactly
/// the place section 22.5 separates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HypotheticalTermAverage {
    /// The average, at the scheme's own published scale.
    Known(Decimal),
    /// No stated grade participates in an average under this scheme.
    NoGradedChoice,
}

/// Section 22.2's seventh bullet.
///
/// Private fields and one producer, [`GpaScenario::under`], which takes the
/// stated grades as a parameter. There is no constructor that omits them, so
/// a GPA scenario without stated grades is not a value that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpaScenario {
    average: HypotheticalTermAverage,
    scale: u8,
    scheme_id: String,
    included: Vec<OfferingId>,
    excluded: Vec<(OfferingId, AverageExclusion)>,
}

impl GpaScenario {
    /// Computes the term average under the stated grades.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::GradeAssumptionOutsidePlan`] when a grade was stated for
    /// something the plan does not choose, and
    /// [`WhatIfError::GradeAssumptionMissing`] when one of the plan's choices
    /// has no stated grade. Neither is filled in: section 22.2 admits the
    /// average only under the grades the user stated, and an average over the
    /// stated subset is an average of a plan nobody made.
    pub(crate) fn under(
        choices: &[PlanChoice],
        stated: &StatedGradeAssumptions,
        scheme: &GradingScheme,
    ) -> Result<Self, WhatIfError> {
        let chosen: BTreeSet<OfferingId> = choices.iter().map(PlanChoice::offering_id).collect();
        for assumption in stated.stated() {
            if !chosen.contains(&assumption.offering_id()) {
                return Err(WhatIfError::GradeAssumptionOutsidePlan(
                    assumption.offering_id(),
                ));
            }
        }
        let mut quality = decimal::zero()?;
        let mut denominator = decimal::zero()?;
        let mut included = Vec::new();
        let mut excluded = Vec::new();
        for choice in choices {
            let Some(symbol) = stated.grade_for(choice.offering_id()) else {
                return Err(WhatIfError::GradeAssumptionMissing(choice.offering_id()));
            };
            let treatment = scheme.treatment(symbol);
            let Some(points) = treatment
                .grade_points()
                .filter(|_| treatment.participates_in_average())
            else {
                excluded.push((choice.offering_id(), AverageExclusion::NotGradedInScheme));
                continue;
            };
            let credits = decimal::integer(i128::from(choice.credits().value()))?;
            quality = decimal::add(quality, decimal::mul(credits, points)?)?;
            denominator = decimal::add(denominator, credits)?;
            included.push(choice.offering_id());
        }
        let scale = scheme.published_scale();
        let average = if decimal::is_zero(denominator) {
            HypotheticalTermAverage::NoGradedChoice
        } else {
            HypotheticalTermAverage::Known(decimal::div_round_half_up(quality, denominator, scale)?)
        };
        Ok(Self {
            average,
            scale,
            scheme_id: scheme.id().to_owned(),
            included,
            excluded,
        })
    }

    /// The average.
    #[must_use]
    pub const fn average(&self) -> &HypotheticalTermAverage {
        &self.average
    }

    /// The scale the average is published to.
    #[must_use]
    pub const fn scale(&self) -> u8 {
        self.scale
    }

    /// The grading scheme version the grades were read through.
    #[must_use]
    pub fn scheme_id(&self) -> &str {
        &self.scheme_id
    }

    /// The choices that reached the denominator.
    #[must_use]
    pub fn included(&self) -> &[OfferingId] {
        &self.included
    }

    /// The choices that did not, and why.
    #[must_use]
    pub fn excluded(&self) -> &[(OfferingId, AverageExclusion)] {
        &self.excluded
    }
}

/// Section 22.1's `deterministicResults`.
///
/// Seven fields, one per section 22.2 bullet, in the document's own order.
/// Private fields and one crate-private constructor: the engine is the only
/// thing that produces one, so a caller cannot assemble a result whose credits
/// and whose allocation disagree.
///
/// Note what the type does not hold: no projected value of any kind, no
/// [`academic_scenario::ProjectedEvidenceOpportunity`], no
/// [`academic_scenario::Proposed`], no likelihood and no confidence. That is
/// the data-type half of section 22.1's split, and it is an absence rather than
/// a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicResults {
    credits: CreditLoad,
    schedule: ScheduleConflicts,
    prerequisites: Vec<PrerequisiteStanding>,
    rule_contribution: RuleContribution,
    allocation: Allocation,
    downstream: Vec<DownstreamUnlock>,
    gpa: Option<GpaScenario>,
}

impl DeterministicResults {
    /// Assembles the lane.
    pub(crate) const fn of(
        credits: CreditLoad,
        schedule: ScheduleConflicts,
        prerequisites: Vec<PrerequisiteStanding>,
        rule_contribution: RuleContribution,
        allocation: Allocation,
        downstream: Vec<DownstreamUnlock>,
        gpa: Option<GpaScenario>,
    ) -> Self {
        Self {
            credits,
            schedule,
            prerequisites,
            rule_contribution,
            allocation,
            downstream,
            gpa,
        }
    }

    /// Section 22.2's first bullet.
    #[must_use]
    pub const fn credits(&self) -> &CreditLoad {
        &self.credits
    }

    /// Section 22.2's second bullet.
    #[must_use]
    pub const fn schedule(&self) -> &ScheduleConflicts {
        &self.schedule
    }

    /// Section 22.2's third bullet, one entry per choice.
    #[must_use]
    pub fn prerequisites(&self) -> &[PrerequisiteStanding] {
        &self.prerequisites
    }

    /// Section 22.2's fourth bullet.
    #[must_use]
    pub const fn rule_contribution(&self) -> &RuleContribution {
        &self.rule_contribution
    }

    /// Section 22.2's fifth bullet.
    #[must_use]
    pub const fn allocation(&self) -> &Allocation {
        &self.allocation
    }

    /// Section 22.2's sixth bullet, one entry per downstream course.
    #[must_use]
    pub fn downstream(&self) -> &[DownstreamUnlock] {
        &self.downstream
    }

    /// Section 22.2's seventh bullet, absent when no grade was stated.
    #[must_use]
    pub const fn gpa(&self) -> Option<&GpaScenario> {
        self.gpa.as_ref()
    }

    /// Which of section 22.2's bullets this result carries a value for.
    ///
    /// Six always, and the seventh only under stated grades. The list is built
    /// from the fields rather than returned as a constant, so a result that
    /// somehow held fewer would report fewer rather than claim seven.
    #[must_use]
    pub fn produced(&self) -> Vec<DeterministicItem> {
        let mut produced = vec![
            DeterministicItem::RequestedAndPerCourseCredits,
            DeterministicItem::OfficialScheduleConflict,
            DeterministicItem::OfficialPrerequisiteAndEnrolmentLimit,
            DeterministicItem::RuleContributionUnderCompletionAssumption,
            DeterministicItem::AllocationAndProof,
            DeterministicItem::DownstreamOfficialUnlock,
        ];
        if self.gpa.is_some() {
            produced.push(DeterministicItem::GpaUnderStatedGradeAssumptions);
        }
        produced
    }
}
