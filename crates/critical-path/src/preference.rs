//! Section 16.2's `사용자의 slider는 이 벡터를 정렬하는 preference일 뿐 진리를
//! 바꾸지 않는다`, and the four names the design document shows the survivors
//! under.
//!
//! ## The slider cannot write, and that is the borrow checker's answer
//!
//! [`rank`] takes `&`[`crate::pareto::ParetoFront`] and returns a
//! [`Ranking`] that **borrows** it. A shared reference is the whole guarantee:
//! there is no `&mut` anywhere in this crate, no interior mutability of any
//! kind, and no constructor of a [`crate::vector::CostVector`] or
//! [`crate::vector::BenefitVector`] reachable from this module. A slider
//! physically cannot reach a vector component to change it, so
//! `slider_changes_order_not_facts` is a property of the types rather than an
//! assertion a test remembers to make.
//!
//! `crates/critical-path/tests/compile_fail/` holds the compiled half:
//! mutating a front through a ranking, and building a ranking out of an
//! un-eliminated candidate list, are each programs that do not compile.
//!
//! ## The ordering forms no scalar either
//!
//! A [`PreferenceSlider`] is a **permutation of every axis**, not a weight
//! vector. Comparison walks the axes in the user's priority order and takes the
//! first that separates the two candidates; on a cost axis lower wins and on a
//! benefit axis higher wins, each compared on **both** interval ends.
//!
//! Weights would multiply and add, and the product would be a single number
//! whose value depended on the preference -- exactly the collapse section 16.2
//! forbids. A permutation cannot: no arithmetic combines two axes anywhere in
//! this file.
//!
//! The permutation must be **complete**. [`PreferenceSlider::of`] refuses an
//! order that omits or repeats an axis, because an omitted axis is one the
//! preference silently decided does not matter, which is a change to the answer
//! and not to its order.
//!
//! ## The four names are section 16.2's, and the hedge is recorded
//!
//! Section 16.2 writes `“빠른 project unblock”, “학교 강의 활용”, “기초 견고성”,
//! “낮은 불확실성” **같은** 이름으로 보여준다`. `같은` is `such as`, so the
//! four are examples rather than a closed set in the prose. `REQ-16-006`'s
//! acceptance evidence is `four archetype fixtures`, which fixes four as the
//! measured number, and [`NAMED_STRATEGIES`] is those four with
//! [`STRATEGY_NAMES_ARE_EXAMPLES`] recording that the design document hedged.
//! `docs/contracts/critical-path.md` records it too.
//!
//! ## A name is a slider, and nothing more
//!
//! [`NamedStrategy::slider`] returns a [`PreferenceSlider`]. That is the entire
//! mechanism: `named_strategies_do_not_alter_vectors` holds because a strategy
//! has no other output and no other effect, and the four rankings it produces
//! are four orders over one unchanged front.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    CriticalPathError,
    pareto::ParetoFront,
    plan::Candidate,
    vector::{BenefitComponent, CostComponent, VectorAxis, all_axes},
};

/// A user preference: a complete priority order over every axis.
///
/// Private field, one constructor, no `Default`. There is deliberately no
/// neutral preference: an engine that shipped one would be answering the
/// ordering question on the user's behalf, and section 16.5's whole closing
/// paragraph is that the engine recommends and the user chooses. This is
/// `P2-N3`'s `PersonalizationSpeed` has no `Default` applied to section 16.2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<VectorAxis>", into = "Vec<VectorAxis>")]
pub struct PreferenceSlider {
    order: Vec<VectorAxis>,
}

impl PreferenceSlider {
    /// Declares a priority order.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::SliderIsNotAPermutation`] when `order` is not
    /// exactly every axis of both vectors, each once. See the module note.
    pub fn of(order: Vec<VectorAxis>) -> Result<Self, CriticalPathError> {
        let offered: BTreeSet<VectorAxis> = order.iter().copied().collect();
        let expected: BTreeSet<VectorAxis> = all_axes().into_iter().collect();
        if offered.len() != order.len() || offered != expected {
            return Err(CriticalPathError::SliderIsNotAPermutation);
        }
        Ok(Self { order })
    }

    /// The priority order, most important first.
    #[must_use]
    pub fn order(&self) -> &[VectorAxis] {
        &self.order
    }
}

impl TryFrom<Vec<VectorAxis>> for PreferenceSlider {
    type Error = CriticalPathError;

    fn try_from(order: Vec<VectorAxis>) -> Result<Self, Self::Error> {
        Self::of(order)
    }
}

impl From<PreferenceSlider> for Vec<VectorAxis> {
    fn from(value: PreferenceSlider) -> Self {
        value.order
    }
}

/// Section 16.2's four strategy names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NamedStrategy {
    /// `빠른 project unblock`.
    FastProjectUnblock,
    /// `학교 강의 활용`.
    UseSchoolCourses,
    /// `기초 견고성`.
    FoundationalSolidity,
    /// `낮은 불확실성`.
    LowUncertainty,
}

/// The four, in section 16.2's own order.
pub const NAMED_STRATEGIES: [NamedStrategy; 4] = [
    NamedStrategy::FastProjectUnblock,
    NamedStrategy::UseSchoolCourses,
    NamedStrategy::FoundationalSolidity,
    NamedStrategy::LowUncertainty,
];

/// Section 16.2 introduces the four names with `같은`, which is `such as`.
///
/// Kept as a named constant because the count is a measurement of an open list.
/// See the module note.
pub const STRATEGY_NAMES_ARE_EXAMPLES: &str =
    "section 16.2 writes 같은 이름으로, so the four are examples and not a closed set";

impl NamedStrategy {
    /// The name section 16.2 writes, verbatim and without its quotation marks.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::FastProjectUnblock => "빠른 project unblock",
            Self::UseSchoolCourses => "학교 강의 활용",
            Self::FoundationalSolidity => "기초 견고성",
            Self::LowUncertainty => "낮은 불확실성",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FastProjectUnblock => "FAST_PROJECT_UNBLOCK",
            Self::UseSchoolCourses => "USE_SCHOOL_COURSES",
            Self::FoundationalSolidity => "FOUNDATIONAL_SOLIDITY",
            Self::LowUncertainty => "LOW_UNCERTAINTY",
        }
    }

    /// The preference this name means.
    ///
    /// Each is a complete permutation built by moving the axes the name is
    /// about to the front and keeping every other axis in section 16.2's own
    /// order behind them. That is the only thing a strategy does.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::SliderIsNotAPermutation`] can only fire if the
    /// leading axes below stop being a subset of [`all_axes`], which is a
    /// compile-time impossibility today and a runtime refusal rather than a
    /// panic if a later edit breaks it.
    pub fn slider(self) -> Result<PreferenceSlider, CriticalPathError> {
        let leading: Vec<VectorAxis> = match self {
            Self::FastProjectUnblock => vec![
                VectorAxis::Benefit {
                    component: BenefitComponent::ImmediateProjectValue,
                },
                VectorAxis::Cost {
                    component: CostComponent::CalendarDelay,
                },
                VectorAxis::Cost {
                    component: CostComponent::LearningEffort,
                },
            ],
            Self::UseSchoolCourses => vec![
                VectorAxis::Benefit {
                    component: BenefitComponent::CurriculumValue,
                },
                VectorAxis::Benefit {
                    component: BenefitComponent::EvidenceOpportunity,
                },
            ],
            Self::FoundationalSolidity => vec![
                VectorAxis::Benefit {
                    component: BenefitComponent::ReuseAcrossGoals,
                },
                VectorAxis::Cost {
                    component: CostComponent::PrerequisiteRisk,
                },
                VectorAxis::Benefit {
                    component: BenefitComponent::GoalCoverage,
                },
            ],
            Self::LowUncertainty => vec![
                VectorAxis::Cost {
                    component: CostComponent::Uncertainty,
                },
                VectorAxis::Cost {
                    component: CostComponent::PrerequisiteRisk,
                },
            ],
        };
        let mut order = leading.clone();
        order.extend(
            all_axes()
                .into_iter()
                .filter(|axis| !leading.contains(axis)),
        );
        PreferenceSlider::of(order)
    }
}

/// An order over a Pareto front, and the front it is an order over.
///
/// Holds `&'a ParetoFront`. That shared borrow is what makes a ranking unable
/// to change a fact. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranking<'a> {
    front: &'a ParetoFront,
    order: Vec<usize>,
    slider: PreferenceSlider,
}

impl<'a> Ranking<'a> {
    /// The candidates in ranked order.
    #[must_use]
    pub fn candidates(&self) -> Vec<&'a Candidate> {
        self.order
            .iter()
            .filter_map(|index| self.front.candidates().get(*index))
            .collect()
    }

    /// The ranked positions, as indices into the front's own list.
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// The preference this order was produced under.
    #[must_use]
    pub const fn slider(&self) -> &PreferenceSlider {
        &self.slider
    }

    /// The front this is an order over. Shared, never mutable.
    #[must_use]
    pub const fn front(&self) -> &'a ParetoFront {
        self.front
    }
}

/// Orders a Pareto front under one preference.
///
/// The comparison is lexicographic over the slider's axis order and forms no
/// scalar. Ties fall through to the candidate's own satisfying-set concept
/// order, so the result is total and two runs over equal input agree.
#[must_use]
pub fn rank<'a>(front: &'a ParetoFront, slider: &PreferenceSlider) -> Ranking<'a> {
    let mut order: Vec<usize> = (0..front.candidates().len()).collect();
    order.sort_by(|left, right| {
        let (Some(a), Some(b)) = (
            front.candidates().get(*left),
            front.candidates().get(*right),
        ) else {
            return left.cmp(right);
        };
        compare_under(a, b, slider).then_with(|| {
            a.satisfying_set()
                .concepts()
                .iter()
                .map(|id| id.as_uuid())
                .collect::<Vec<_>>()
                .cmp(
                    &b.satisfying_set()
                        .concepts()
                        .iter()
                        .map(|id| id.as_uuid())
                        .collect::<Vec<_>>(),
                )
        })
    });
    Ranking {
        front,
        order,
        slider: slider.clone(),
    }
}

/// Lexicographic comparison over the slider's axis order.
///
/// Pinned by `the_critical_path_decisions_are_pinned`: the rule that a cost
/// axis prefers the lower interval and a benefit axis the higher one, on both
/// ends, is the whole of what a preference may do.
fn compare_under(
    left: &Candidate,
    right: &Candidate,
    slider: &PreferenceSlider,
) -> std::cmp::Ordering {
    for axis in slider.order() {
        let ordering = match axis {
            VectorAxis::Cost { component } => {
                let a = left.cost().component(*component);
                let b = right.cost().component(*component);
                (a.high(), a.low()).cmp(&(b.high(), b.low()))
            }
            VectorAxis::Benefit { component } => {
                let a = left.benefit().component(*component);
                let b = right.benefit().component(*component);
                (b.low(), b.high()).cmp(&(a.low(), a.high()))
            }
        };
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    std::cmp::Ordering::Equal
}
