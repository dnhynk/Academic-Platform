//! Section 22.4's comparison, and the one thing it refuses to produce.
//!
//! > 하나의 "추천 점수"를 기본으로 표시하지 않는다. 사용자가 중요도를 조정하면
//! > 왜 정렬이 바뀌었는지 보여준다.
//!
//! # There is no aggregate score, and the absence is exhaustive
//!
//! Nothing in this module adds, multiplies, weights or averages two dimensions.
//! [`DimensionPriority`] is a **complete permutation** of section 22.4's
//! dimensions, not a weight vector: comparison walks the dimensions in the
//! user's order and takes the first that separates two plans. A weight vector
//! would multiply and add, and the product would be a single number whose value
//! depended on the preference — which is exactly the collapse the sentence
//! above refuses. `P2-N6` holds section 16.2 the same way and for the same
//! reason.
//!
//! The absence is proved by exhaustion rather than by a list of forbidden
//! names: `no_default_recommendation_score` compares the whole declared field
//! and method inventory of this crate against a reviewed one in both
//! directions, so a headline number added anywhere — in a module nobody
//! predicted, spelling nothing anybody thought to forbid — fails as an extra
//! entry. `P2-X2` established that shape for section 25.2's hero metric.
//!
//! And there is no default: [`DimensionPriority`] has no `Default` and no
//! constant. A shipped neutral ordering would be this product answering the
//! importance question on the user's behalf, which is the first half of the
//! same sentence.
//!
//! # A reordering says which weight changed
//!
//! [`ReorderingExplanation::between`] takes the previous priority and the next
//! one and names every dimension that moved, with the rank it moved from and
//! the rank it moved to. It refuses two identical priorities: there is no
//! changed weight to name, and an explanation with an empty reason is the
//! shape `P2-U3` refuses for an indeterminate verdict.
//!
//! It also names the dimension that actually decided the new order — the first
//! one in the new priority that separates the plans it did not separate before.
//! Naming the moved weight without naming the decisive one would answer *what
//! did I change* rather than *why did the order change*.
//!
//! # A comparison cannot rewrite a fact
//!
//! [`compare`] takes `&[&PlanScenario]` and [`ComparisonView`] holds those
//! shared borrows. There is no `&mut` and no interior mutability anywhere in
//! this crate, so a priority physically cannot reach a credit total, a
//! conflict, a band or a bias to change it. That is `P2-N6`'s *slider는 이
//! 벡터를 정렬하는 preference일 뿐 진리를 바꾸지 않는다*, applied to section
//! 22.4's own dimensions.

use std::{cmp::Ordering, collections::BTreeSet};

use academic_domain::EntityId;

use crate::{
    deterministic::UnlockStanding, error::WhatIfError, inputs::RelevanceSubject,
    scenario::PlanScenario,
};

/// One row of section 22.4's comparison table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComparisonDimension {
    /// `졸업 rule contribution`.
    GraduationRuleContribution,
    /// `시간표`.
    Timetable,
    /// `project gap`.
    ProjectGap,
    /// `critical path`.
    CriticalPath,
    /// `workload`.
    Workload,
    /// `후속 경로`.
    DownstreamRoute,
}

/// Section 22.4's rows, in the table's own order.
pub const COMPARISON_DIMENSIONS: [ComparisonDimension; 6] = [
    ComparisonDimension::GraduationRuleContribution,
    ComparisonDimension::Timetable,
    ComparisonDimension::ProjectGap,
    ComparisonDimension::CriticalPath,
    ComparisonDimension::Workload,
    ComparisonDimension::DownstreamRoute,
];

/// Which of section 22's two lanes a comparison dimension is served from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionLane {
    /// Served entirely from section 22.2.
    Deterministic,
    /// Served entirely from section 22.3.
    Projected,
    /// Served from both, which is what section 22.4's `Mixed` cell says.
    Mixed,
}

impl ComparisonDimension {
    /// The label section 22.4's first column writes, verbatim.
    #[must_use]
    pub const fn spec_label(self) -> &'static str {
        match self {
            Self::GraduationRuleContribution => "졸업 rule contribution",
            Self::Timetable => "시간표",
            Self::ProjectGap => "project gap",
            Self::CriticalPath => "critical path",
            Self::Workload => "workload",
            Self::DownstreamRoute => "후속 경로",
        }
    }

    /// The label section 22.4's `확실성` column writes, verbatim.
    #[must_use]
    pub const fn spec_certainty(self) -> &'static str {
        match self {
            Self::GraduationRuleContribution => "Deterministic if completed",
            Self::Timetable => "Official schedule",
            Self::ProjectGap => "Syllabus-inferred",
            Self::CriticalPath => "Projection",
            Self::Workload => "Biased estimate",
            Self::DownstreamRoute => "Mixed",
        }
    }

    /// Which lane the dimension is served from.
    ///
    /// `DownstreamRoute` is [`DimensionLane::Mixed`] because section 22 puts
    /// its two halves on opposite sides: *후속 Course의 공식 prerequisite
    /// unlock* is section 22.2's sixth bullet and *후속 비공식 권장 지식의
    /// readiness* is section 22.3's seventh. The table's own `Mixed` cell says
    /// so, and folding the row onto one side would have made one of the two
    /// halves claim the other's certainty.
    #[must_use]
    pub const fn lane(self) -> DimensionLane {
        match self {
            Self::GraduationRuleContribution | Self::Timetable => DimensionLane::Deterministic,
            Self::ProjectGap | Self::CriticalPath | Self::Workload => DimensionLane::Projected,
            Self::DownstreamRoute => DimensionLane::Mixed,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GraduationRuleContribution => "GRADUATION_RULE_CONTRIBUTION",
            Self::Timetable => "TIMETABLE",
            Self::ProjectGap => "PROJECT_GAP",
            Self::CriticalPath => "CRITICAL_PATH",
            Self::Workload => "WORKLOAD",
            Self::DownstreamRoute => "DOWNSTREAM_ROUTE",
        }
    }
}

/// A user's importance ordering: a complete permutation of section 22.4's rows.
///
/// Private field, one constructor, no `Default` and no constant. See the module
/// note for both halves of why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimensionPriority {
    order: Vec<ComparisonDimension>,
}

impl DimensionPriority {
    /// Declares an importance ordering, most important first.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::PriorityIsNotAPermutation`] when the order omits or
    /// repeats a dimension. An omitted dimension is one the preference silently
    /// decided does not matter, which is a change to the answer and not to its
    /// order.
    pub fn of(order: Vec<ComparisonDimension>) -> Result<Self, WhatIfError> {
        let offered: BTreeSet<ComparisonDimension> = order.iter().copied().collect();
        let expected: BTreeSet<ComparisonDimension> = COMPARISON_DIMENSIONS.into_iter().collect();
        if offered.len() != order.len() || offered != expected {
            return Err(WhatIfError::PriorityIsNotAPermutation);
        }
        Ok(Self { order })
    }

    /// The ordering, most important first.
    #[must_use]
    pub fn order(&self) -> &[ComparisonDimension] {
        &self.order
    }

    /// Where one dimension sits, counting from zero.
    #[must_use]
    pub fn rank_of(&self, dimension: ComparisonDimension) -> Option<usize> {
        self.order
            .iter()
            .position(|candidate| *candidate == dimension)
    }
}

/// One plan's reading on one dimension, as a comparable shape.
///
/// Every variant is a count or a range, never a score, and two variants are
/// never compared with each other: [`compare_on`] matches the dimension and
/// compares like with like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DimensionQuantity {
    /// Higher is better for this dimension.
    prefers_more: bool,
    /// The primary quantity — a count, or the top of a range.
    primary: u32,
    /// The tie-break quantity — the bottom of a range, or zero.
    secondary: u32,
}

/// Reads one plan on one dimension.
fn read(plan: &PlanScenario, dimension: ComparisonDimension) -> DimensionQuantity {
    match dimension {
        ComparisonDimension::GraduationRuleContribution => DimensionQuantity {
            prefers_more: true,
            primary: plan
                .deterministic()
                .rule_contribution()
                .contributions()
                .iter()
                .map(|contribution| u32::from(contribution.credits()))
                .sum(),
            secondary: 0,
        },
        ComparisonDimension::Timetable => DimensionQuantity {
            prefers_more: false,
            primary: count(plan.deterministic().schedule().conflicts().len()),
            secondary: 0,
        },
        ComparisonDimension::ProjectGap => DimensionQuantity {
            prefers_more: true,
            primary: count(
                plan.projections()
                    .relevance()
                    .entries()
                    .iter()
                    .filter(|entry| entry.subject() == RelevanceSubject::Project)
                    .count(),
            ),
            secondary: 0,
        },
        ComparisonDimension::CriticalPath => DimensionQuantity {
            prefers_more: true,
            primary: count(plan.projections().path_coverage().entries().len()),
            secondary: 0,
        },
        ComparisonDimension::Workload => DimensionQuantity {
            prefers_more: false,
            primary: u32::from(plan.projections().workload().band().high_hours()),
            secondary: u32::from(plan.projections().workload().band().low_hours()),
        },
        ComparisonDimension::DownstreamRoute => DimensionQuantity {
            prefers_more: true,
            primary: count(
                plan.deterministic()
                    .downstream()
                    .iter()
                    .filter(|unlock| {
                        matches!(
                            unlock.standing(),
                            UnlockStanding::UnlockedByThisPlan | UnlockStanding::AlreadyUnlocked
                        )
                    })
                    .count(),
            ),
            secondary: 0,
        },
    }
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Compares two plans on one dimension.
///
/// `Less` means *ranks earlier*. No arithmetic combines two dimensions
/// anywhere: this function is called once per dimension and its result is used
/// or discarded, never accumulated.
fn compare_on(
    left: &PlanScenario,
    right: &PlanScenario,
    dimension: ComparisonDimension,
) -> Ordering {
    let a = read(left, dimension);
    let b = read(right, dimension);
    if a.prefers_more {
        (b.primary, b.secondary).cmp(&(a.primary, a.secondary))
    } else {
        (a.primary, a.secondary).cmp(&(b.primary, b.secondary))
    }
}

/// An order over a set of plans, and the plans it is an order over.
///
/// Holds shared borrows. That borrow is the whole guarantee that a priority
/// cannot change a fact; see the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonView<'a> {
    plans: Vec<&'a PlanScenario>,
    priority: DimensionPriority,
    order: Vec<usize>,
}

impl<'a> ComparisonView<'a> {
    /// The plans in ranked order.
    #[must_use]
    pub fn ranked(&self) -> Vec<&'a PlanScenario> {
        self.order
            .iter()
            .filter_map(|index| self.plans.get(*index).copied())
            .collect()
    }

    /// The ranked positions, as indices into the caller's own list.
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// The plan identities in ranked order.
    #[must_use]
    pub fn ranked_ids(&self) -> Vec<EntityId> {
        self.ranked().iter().map(|plan| plan.id()).collect()
    }

    /// The priority this order was produced under.
    #[must_use]
    pub const fn priority(&self) -> &DimensionPriority {
        &self.priority
    }
}

/// Orders a set of plans under one importance ordering.
///
/// # Errors
///
/// [`WhatIfError::ComparisonNeedsTwoPlans`] for fewer than two plans — section
/// 22.4 is a comparison and there is nothing to order otherwise — and
/// [`WhatIfError::DuplicatePlanInComparison`] when one plan appears twice.
pub fn compare<'a>(
    plans: &[&'a PlanScenario],
    priority: &DimensionPriority,
) -> Result<ComparisonView<'a>, WhatIfError> {
    if plans.len() < 2 {
        return Err(WhatIfError::ComparisonNeedsTwoPlans);
    }
    let mut seen = BTreeSet::new();
    for plan in plans {
        if !seen.insert(plan.id()) {
            return Err(WhatIfError::DuplicatePlanInComparison);
        }
    }
    let plans: Vec<&'a PlanScenario> = plans.to_vec();
    let mut order: Vec<usize> = (0..plans.len()).collect();
    order.sort_by(|left, right| {
        let (Some(a), Some(b)) = (plans.get(*left), plans.get(*right)) else {
            return left.cmp(right);
        };
        for dimension in priority.order() {
            let ordering = compare_on(a, b, *dimension);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        a.id().as_bytes().cmp(b.id().as_bytes())
    });
    Ok(ComparisonView {
        plans,
        priority: priority.clone(),
        order,
    })
}

/// One dimension that changed rank between two priorities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DimensionMove {
    dimension: ComparisonDimension,
    from_rank: usize,
    to_rank: usize,
}

impl DimensionMove {
    /// Which dimension.
    #[must_use]
    pub const fn dimension(&self) -> ComparisonDimension {
        self.dimension
    }

    /// Where it was, counting from zero.
    #[must_use]
    pub const fn from_rank(&self) -> usize {
        self.from_rank
    }

    /// Where it is now.
    #[must_use]
    pub const fn to_rank(&self) -> usize {
        self.to_rank
    }
}

/// Why the order changed.
///
/// Every field is about the *ordering*. There is no field here that carries a
/// plan value, so an explanation cannot restate a fact differently from the
/// plan it explains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReorderingExplanation {
    moved: Vec<DimensionMove>,
    decisive: Option<ComparisonDimension>,
    order_before: Vec<EntityId>,
    order_after: Vec<EntityId>,
}

impl ReorderingExplanation {
    /// Explains what one change of importance did.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::PriorityDidNotChange`] when the two priorities are equal.
    /// There is no changed weight to name, and an explanation whose reason list
    /// is empty is not a value this type has.
    ///
    /// Every error [`compare`] raises is raised here too, because this runs it
    /// twice over the same plans.
    pub fn between(
        plans: &[&PlanScenario],
        previous: &DimensionPriority,
        next: &DimensionPriority,
    ) -> Result<Self, WhatIfError> {
        if previous == next {
            return Err(WhatIfError::PriorityDidNotChange);
        }
        let before = compare(plans, previous)?;
        let after = compare(plans, next)?;
        let mut moved = Vec::new();
        for dimension in COMPARISON_DIMENSIONS {
            let (Some(from_rank), Some(to_rank)) =
                (previous.rank_of(dimension), next.rank_of(dimension))
            else {
                continue;
            };
            if from_rank != to_rank {
                moved.push(DimensionMove {
                    dimension,
                    from_rank,
                    to_rank,
                });
            }
        }
        Ok(Self {
            decisive: decisive_dimension(&before, &after, next),
            moved,
            order_before: before.ranked_ids(),
            order_after: after.ranked_ids(),
        })
    }

    /// Every dimension whose rank changed, in the table's own order.
    ///
    /// Never empty: two priorities that moved nothing are refused above.
    #[must_use]
    pub fn moved(&self) -> &[DimensionMove] {
        &self.moved
    }

    /// The dimension that decided the new leader, when the leader changed.
    #[must_use]
    pub const fn decisive(&self) -> Option<ComparisonDimension> {
        self.decisive
    }

    /// The order the plans were in before.
    #[must_use]
    pub fn order_before(&self) -> &[EntityId] {
        &self.order_before
    }

    /// The order they are in now.
    #[must_use]
    pub fn order_after(&self) -> &[EntityId] {
        &self.order_after
    }

    /// Whether the ranking itself moved.
    #[must_use]
    pub fn order_changed(&self) -> bool {
        self.order_before != self.order_after
    }
}

/// The first dimension of the new priority that separates the new leader from
/// the old one.
///
/// `None` when the leader did not change: a reordering that moved a weight and
/// left the ranking alone has a changed weight to name and no decisive
/// dimension, and saying so is more honest than naming one that decided
/// nothing.
fn decisive_dimension(
    before: &ComparisonView<'_>,
    after: &ComparisonView<'_>,
    next: &DimensionPriority,
) -> Option<ComparisonDimension> {
    let old_leader = *before.ranked().first()?;
    let new_leader = *after.ranked().first()?;
    if old_leader.id() == new_leader.id() {
        return None;
    }
    next.order()
        .iter()
        .copied()
        .find(|dimension| compare_on(new_leader, old_leader, *dimension) != Ordering::Equal)
}
