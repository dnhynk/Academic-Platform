//! Section 16.2's two vectors, and the fact that neither of them is ever a
//! number.
//!
//! ## Seven and five are measurements
//!
//! `cost_vector_has_seven_separate_components` and
//! `benefit_vector_has_five_separate_components` read section 16.2's two code
//! blocks back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compare their lines against [`COST_COMPONENTS`] and [`BENEFIT_COMPONENTS`]
//! in both directions. Seven and five are what the design document says, not
//! what this file decided.
//!
//! ## Nothing here folds a vector into a scalar
//!
//! [`CostVector`] and [`BenefitVector`] have private fields, one constructor
//! each that takes every component, and one accessor each that answers for one
//! named component. There is no `total`, no `sum`, no `score`, no `weighted`,
//! no `Ord`, no `PartialOrd` and no conversion to a number. A caller that wants
//! an order gets it from [`crate::preference`], which orders **by axis
//! priority** and forms no scalar either.
//!
//! That is section 16.2's own sentence -- `사용자의 slider는 이 벡터를 정렬하는
//! preference일 뿐 진리를 바꾸지 않는다` -- read as a type rule. A weighted sum
//! would make the slider's weights part of the answer's value rather than part
//! of its order, and the answer would then change when the preference changed.
//!
//! ## Units are declared, because section 16.2 declares none
//!
//! Section 16.2 fixes the seven axes and the five axes and no unit for any of
//! them. An interval with no unit compares to nothing, so this crate declares
//! one [`Unit`] per axis, and comparison is only ever between the *same* axis
//! of two paths. No operation in this crate compares two different axes, which
//! is why no common unit is needed and none is invented.

use serde::{Deserialize, Serialize};

use crate::CriticalPathError;

/// The unit one axis is measured in.
///
/// Declared here because section 16.2 declares none. See the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Unit {
    /// Wall-clock minutes of the user's own time.
    Minutes,
    /// Whole calendar days between now and availability.
    Days,
    /// Thousandths, for a proportion that is not a duration.
    Permille,
    /// A count of distinct occurrences.
    Occurrences,
}

impl Unit {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Minutes => "MINUTES",
            Self::Days => "DAYS",
            Self::Permille => "PERMILLE",
            Self::Occurrences => "OCCURRENCES",
        }
    }
}

/// Section 16.2's seven cost axes, in the order `Cost(P)` writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CostComponent {
    /// `learning_effort`.
    LearningEffort,
    /// `refresh_effort`.
    RefreshEffort,
    /// `prerequisite_risk`.
    PrerequisiteRisk,
    /// `uncertainty`.
    Uncertainty,
    /// `calendar_delay`.
    CalendarDelay,
    /// `context_switching`.
    ContextSwitching,
    /// `opportunity_cost`.
    OpportunityCost,
}

/// The seven, in section 16.2's own order.
pub const COST_COMPONENTS: [CostComponent; 7] = [
    CostComponent::LearningEffort,
    CostComponent::RefreshEffort,
    CostComponent::PrerequisiteRisk,
    CostComponent::Uncertainty,
    CostComponent::CalendarDelay,
    CostComponent::ContextSwitching,
    CostComponent::OpportunityCost,
];

impl CostComponent {
    /// The identifier section 16.2's `Cost(P)` block writes, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::LearningEffort => "learning_effort",
            Self::RefreshEffort => "refresh_effort",
            Self::PrerequisiteRisk => "prerequisite_risk",
            Self::Uncertainty => "uncertainty",
            Self::CalendarDelay => "calendar_delay",
            Self::ContextSwitching => "context_switching",
            Self::OpportunityCost => "opportunity_cost",
        }
    }

    /// The unit this axis is measured in.
    #[must_use]
    pub const fn unit(self) -> Unit {
        match self {
            Self::LearningEffort | Self::RefreshEffort | Self::OpportunityCost => Unit::Minutes,
            Self::PrerequisiteRisk | Self::Uncertainty => Unit::Permille,
            Self::CalendarDelay => Unit::Days,
            Self::ContextSwitching => Unit::Occurrences,
        }
    }
}

/// Section 16.2's five benefit axes, in the order `Benefit(P)` writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BenefitComponent {
    /// `goal_coverage`.
    GoalCoverage,
    /// `immediate_project_value`.
    ImmediateProjectValue,
    /// `curriculum_value`.
    CurriculumValue,
    /// `reuse_across_goals`.
    ReuseAcrossGoals,
    /// `evidence_opportunity`.
    EvidenceOpportunity,
}

/// The five, in section 16.2's own order.
pub const BENEFIT_COMPONENTS: [BenefitComponent; 5] = [
    BenefitComponent::GoalCoverage,
    BenefitComponent::ImmediateProjectValue,
    BenefitComponent::CurriculumValue,
    BenefitComponent::ReuseAcrossGoals,
    BenefitComponent::EvidenceOpportunity,
];

impl BenefitComponent {
    /// The identifier section 16.2's `Benefit(P)` block writes, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::GoalCoverage => "goal_coverage",
            Self::ImmediateProjectValue => "immediate_project_value",
            Self::CurriculumValue => "curriculum_value",
            Self::ReuseAcrossGoals => "reuse_across_goals",
            Self::EvidenceOpportunity => "evidence_opportunity",
        }
    }

    /// The unit this axis is measured in.
    #[must_use]
    pub const fn unit(self) -> Unit {
        match self {
            Self::GoalCoverage | Self::ImmediateProjectValue | Self::CurriculumValue => {
                Unit::Permille
            }
            Self::ReuseAcrossGoals | Self::EvidenceOpportunity => Unit::Occurrences,
        }
    }
}

/// Section 16.2's four input families for a concept's expected cost.
///
/// `개별 concept의 예상 비용은 사용자 state/freshness, concept granularity, 이용
/// 가능한 resource, 과거 실제 학습 속도를 사용한다`. A [`CostBasis::Measured`]
/// names which of the four it actually read; a basis that read none of them is
/// not `Measured`, and [`CostBasis::Unmeasured`] is the value section 16.2's
/// next sentence is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BasisFamily {
    /// `사용자 state/freshness`.
    StateAndFreshness,
    /// `concept granularity`.
    ConceptGranularity,
    /// `이용 가능한 resource`.
    AvailableResource,
    /// `과거 실제 학습 속도`.
    PastLearningSpeed,
}

/// The four, in the order section 16.2 names them.
pub const BASIS_FAMILIES: [BasisFamily; 4] = [
    BasisFamily::StateAndFreshness,
    BasisFamily::ConceptGranularity,
    BasisFamily::AvailableResource,
    BasisFamily::PastLearningSpeed,
];

impl BasisFamily {
    /// The words section 16.2 uses for this family, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::StateAndFreshness => "사용자 state/freshness",
            Self::ConceptGranularity => "concept granularity",
            Self::AvailableResource => "이용 가능한 resource",
            Self::PastLearningSpeed => "과거 실제 학습 속도",
        }
    }
}

/// What an estimate rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CostBasis {
    /// At least one of section 16.2's four families was read.
    Measured {
        /// Which families, deduplicated and in [`BASIS_FAMILIES`] order.
        families: Vec<BasisFamily>,
    },
    /// `근거가 없으면 범위로 표시한다`. Nothing was read.
    Unmeasured,
}

impl CostBasis {
    /// Records which of the four families an estimate read.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::MeasuredBasisNamesNoFamily`] for an empty list: a
    /// basis that read nothing is [`CostBasis::Unmeasured`], and calling it
    /// measured is exactly the false precision section 16.2 refuses.
    pub fn measured(families: &[BasisFamily]) -> Result<Self, CriticalPathError> {
        let kept: Vec<BasisFamily> = BASIS_FAMILIES
            .into_iter()
            .filter(|family| families.contains(family))
            .collect();
        if kept.is_empty() {
            return Err(CriticalPathError::MeasuredBasisNamesNoFamily);
        }
        Ok(Self::Measured { families: kept })
    }

    /// Whether anything was read.
    #[must_use]
    pub const fn is_measured(&self) -> bool {
        matches!(self, Self::Measured { .. })
    }

    /// The families read, empty for [`CostBasis::Unmeasured`].
    #[must_use]
    pub fn families(&self) -> &[BasisFamily] {
        match self {
            Self::Measured { families } => families,
            Self::Unmeasured => &[],
        }
    }
}

/// One axis's magnitude: a closed interval and what it rests on.
///
/// ## There is no point
///
/// The two accessors are [`CostEstimate::low`] and [`CostEstimate::high`].
/// There is no `point`, no `midpoint`, no `expected` and no `value`, so
/// `unknown_cost_is_a_range` is not a rule a caller has to respect -- the
/// narrowing operation does not exist. Aggregation along a path is interval
/// addition ([`CostEstimate::plus`]), which widens and never collapses.
///
/// A [`CostBasis::Unmeasured`] estimate is additionally required to be **wide**:
/// `low < high` strictly. That is section 16.2's `근거가 없으면 범위로
/// 표시한다` as a constructor refusal rather than a rendering convention, and it
/// is what stops a cold start from being reported as an exact number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostEstimate {
    low: u32,
    high: u32,
    unit: Unit,
    basis: CostBasis,
}

impl CostEstimate {
    /// Records one axis's interval.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::InvertedEstimate`] when `high < low`, and
    /// [`CriticalPathError::UnmeasuredEstimateIsAPoint`] when a
    /// [`CostBasis::Unmeasured`] estimate has `low == high`.
    pub fn of(
        low: u32,
        high: u32,
        unit: Unit,
        basis: CostBasis,
    ) -> Result<Self, CriticalPathError> {
        if high < low {
            return Err(CriticalPathError::InvertedEstimate);
        }
        if !basis.is_measured() && low == high {
            return Err(CriticalPathError::UnmeasuredEstimateIsAPoint);
        }
        Ok(Self {
            low,
            high,
            unit,
            basis,
        })
    }

    /// The interval's lower end.
    #[must_use]
    pub const fn low(&self) -> u32 {
        self.low
    }

    /// The interval's upper end.
    #[must_use]
    pub const fn high(&self) -> u32 {
        self.high
    }

    /// The unit both ends are in.
    #[must_use]
    pub const fn unit(&self) -> Unit {
        self.unit
    }

    /// What the interval rests on.
    #[must_use]
    pub const fn basis(&self) -> &CostBasis {
        &self.basis
    }

    /// Whether this interval has width, which every unmeasured one has.
    #[must_use]
    pub const fn is_range(&self) -> bool {
        self.low < self.high
    }

    /// Interval addition along a path.
    ///
    /// Both ends are saturating, and the result is measured only when **both**
    /// operands were: adding a measured estimate to an unmeasured one does not
    /// launder the unmeasured half into a basis it does not have.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::UnitMismatch`] when the two ends are in different
    /// units, which is the only comparison this crate refuses outright, and
    /// whatever [`CostEstimate::of`] raises for the sum.
    pub fn plus(&self, other: &Self) -> Result<Self, CriticalPathError> {
        if self.unit != other.unit {
            return Err(CriticalPathError::UnitMismatch);
        }
        let basis = match (&self.basis, &other.basis) {
            (CostBasis::Measured { families: left }, CostBasis::Measured { families: right }) => {
                CostBasis::Measured {
                    families: BASIS_FAMILIES
                        .into_iter()
                        .filter(|family| left.contains(family) || right.contains(family))
                        .collect(),
                }
            }
            _ => CostBasis::Unmeasured,
        };
        // An unmeasured sum of two points would be a point, which the
        // constructor refuses. Nothing in this crate builds one, and the
        // refusal is what says so rather than a silent widening here.
        Self::of(
            self.low.saturating_add(other.low),
            self.high.saturating_add(other.high),
            self.unit,
            basis,
        )
    }
}

/// Section 16.2's `Cost(P)`, with all seven axes kept apart.
///
/// Private fields, one constructor, one accessor. See the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostVector {
    learning_effort: CostEstimate,
    refresh_effort: CostEstimate,
    prerequisite_risk: CostEstimate,
    uncertainty: CostEstimate,
    calendar_delay: CostEstimate,
    context_switching: CostEstimate,
    opportunity_cost: CostEstimate,
}

impl CostVector {
    /// Builds the vector from one estimate per axis, in [`COST_COMPONENTS`]
    /// order.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::AxisUnitMismatch`] when an estimate's unit is not
    /// the unit [`CostComponent::unit`] fixes for its axis.
    pub fn of(estimates: [CostEstimate; 7]) -> Result<Self, CriticalPathError> {
        for (component, estimate) in COST_COMPONENTS.into_iter().zip(estimates.iter()) {
            if estimate.unit() != component.unit() {
                return Err(CriticalPathError::AxisUnitMismatch {
                    axis: component.spec_token(),
                });
            }
        }
        let [
            learning_effort,
            refresh_effort,
            prerequisite_risk,
            uncertainty,
            calendar_delay,
            context_switching,
            opportunity_cost,
        ] = estimates;
        Ok(Self {
            learning_effort,
            refresh_effort,
            prerequisite_risk,
            uncertainty,
            calendar_delay,
            context_switching,
            opportunity_cost,
        })
    }

    /// One named axis. Total over [`CostComponent`] with no wildcard arm.
    #[must_use]
    pub const fn component(&self, component: CostComponent) -> &CostEstimate {
        match component {
            CostComponent::LearningEffort => &self.learning_effort,
            CostComponent::RefreshEffort => &self.refresh_effort,
            CostComponent::PrerequisiteRisk => &self.prerequisite_risk,
            CostComponent::Uncertainty => &self.uncertainty,
            CostComponent::CalendarDelay => &self.calendar_delay,
            CostComponent::ContextSwitching => &self.context_switching,
            CostComponent::OpportunityCost => &self.opportunity_cost,
        }
    }

    /// Axis-wise interval addition, for a path made of several steps.
    ///
    /// # Errors
    ///
    /// Whatever [`CostEstimate::plus`] and [`CostVector::of`] raise.
    pub fn plus(&self, other: &Self) -> Result<Self, CriticalPathError> {
        let mut summed = Vec::with_capacity(COST_COMPONENTS.len());
        for component in COST_COMPONENTS {
            summed.push(self.component(component).plus(other.component(component))?);
        }
        let estimates: [CostEstimate; 7] = summed
            .try_into()
            .map_err(|_| CriticalPathError::AxisCountChanged)?;
        Self::of(estimates)
    }
}

/// Section 16.2's `Benefit(P)`, with all five axes kept apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenefitVector {
    goal_coverage: CostEstimate,
    immediate_project_value: CostEstimate,
    curriculum_value: CostEstimate,
    reuse_across_goals: CostEstimate,
    evidence_opportunity: CostEstimate,
}

impl BenefitVector {
    /// Builds the vector from one estimate per axis, in [`BENEFIT_COMPONENTS`]
    /// order.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::AxisUnitMismatch`] when an estimate's unit is not
    /// the unit [`BenefitComponent::unit`] fixes for its axis.
    pub fn of(estimates: [CostEstimate; 5]) -> Result<Self, CriticalPathError> {
        for (component, estimate) in BENEFIT_COMPONENTS.into_iter().zip(estimates.iter()) {
            if estimate.unit() != component.unit() {
                return Err(CriticalPathError::AxisUnitMismatch {
                    axis: component.spec_token(),
                });
            }
        }
        let [
            goal_coverage,
            immediate_project_value,
            curriculum_value,
            reuse_across_goals,
            evidence_opportunity,
        ] = estimates;
        Ok(Self {
            goal_coverage,
            immediate_project_value,
            curriculum_value,
            reuse_across_goals,
            evidence_opportunity,
        })
    }

    /// One named axis. Total over [`BenefitComponent`] with no wildcard arm.
    #[must_use]
    pub const fn component(&self, component: BenefitComponent) -> &CostEstimate {
        match component {
            BenefitComponent::GoalCoverage => &self.goal_coverage,
            BenefitComponent::ImmediateProjectValue => &self.immediate_project_value,
            BenefitComponent::CurriculumValue => &self.curriculum_value,
            BenefitComponent::ReuseAcrossGoals => &self.reuse_across_goals,
            BenefitComponent::EvidenceOpportunity => &self.evidence_opportunity,
        }
    }
}

/// One axis of either vector.
///
/// [`crate::preference::PreferenceSlider`] is a permutation of every value of
/// this type, so an ordering cannot silently drop an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "side", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VectorAxis {
    /// One of section 16.2's seven cost axes. Lower is preferred.
    Cost {
        /// Which axis.
        component: CostComponent,
    },
    /// One of section 16.2's five benefit axes. Higher is preferred.
    Benefit {
        /// Which axis.
        component: BenefitComponent,
    },
}

/// Every axis of both vectors, cost axes first, each in section 16.2's order.
///
/// The length is seven plus five because those two arrays are, and both are
/// measured against the design document. Nothing here restates a count.
#[must_use]
pub fn all_axes() -> Vec<VectorAxis> {
    let mut axes: Vec<VectorAxis> = COST_COMPONENTS
        .into_iter()
        .map(|component| VectorAxis::Cost { component })
        .collect();
    axes.extend(
        BENEFIT_COMPONENTS
            .into_iter()
            .map(|component| VectorAxis::Benefit { component }),
    );
    axes
}
