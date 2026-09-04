//! Section 22.1's `assumptions` block, and the grade assumption section 22.2
//! makes a precondition.
//!
//! ```yaml
//! assumptions:
//!   - workloadHoursRange: [34, 46]
//!     source: review_model_...
//!   - completionStatus: HYPOTHETICAL
//!   - expectedCoverage: probabilistic
//! ```
//!
//! # `HYPOTHETICAL` is the only completion status there is
//!
//! [`HypotheticalCompletion`] is a unit struct. It has no second value, no
//! `Default` that could be swapped for one, and no constructor that takes a
//! status. Section 22.2's fourth bullet is *이수한다고 **가정했을 때의** 졸업
//! rule contribution*, and
//! [`crate::deterministic::RuleContribution`] takes this value **by
//! parameter** — so a rule contribution computed without the assumption is not
//! a call that can be written, rather than a call that forgets to set a flag.
//!
//! The same shape carries [`ProbabilisticCoverage`]: the expected coverage of a
//! plan is probabilistic and there is no value meaning *certain*.
//!
//! # A GPA needs the grades stated, and stated for every choice
//!
//! Section 22.2: *GPA scenario는 사용자가 명시한 grade 가정에 한해서만 계산*.
//! [`StatedGradeAssumptions`] is a set the user supplies, and
//! [`crate::deterministic::GpaScenario::under`] refuses a set that leaves any
//! of the plan's choices unstated with
//! [`crate::WhatIfError::GradeAssumptionMissing`] rather than assuming one. A
//! plan with no stated grades has no GPA value at all: the field is `None`,
//! which is a different thing from a GPA of zero and from a GPA over the subset
//! the user happened to type.

use academic_domain::{ModelRunId, OfferingId};
use academic_record::grade::GradeSymbol;
use academic_scenario::WorkloadHoursRange;

use crate::error::WhatIfError;

/// One entry of section 22.1's `assumptions` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssumptionKind {
    /// `workloadHoursRange`, with its `source`.
    WorkloadHoursRange,
    /// `completionStatus`.
    CompletionStatus,
    /// `expectedCoverage`.
    ExpectedCoverage,
}

/// The three, in section 22.1's own order.
pub const ASSUMPTION_KINDS: [AssumptionKind; 3] = [
    AssumptionKind::WorkloadHoursRange,
    AssumptionKind::CompletionStatus,
    AssumptionKind::ExpectedCoverage,
];

impl AssumptionKind {
    /// The key section 22.1's YAML block writes, verbatim.
    #[must_use]
    pub const fn spec_key(self) -> &'static str {
        match self {
            Self::WorkloadHoursRange => "workloadHoursRange",
            Self::CompletionStatus => "completionStatus",
            Self::ExpectedCoverage => "expectedCoverage",
        }
    }
}

/// Section 22.1's `completionStatus: HYPOTHETICAL`.
///
/// One value, no `Default`, no constructor that takes a status. See the module
/// note: this is the argument that makes an assumed completion an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HypotheticalCompletion;

impl HypotheticalCompletion {
    /// The value section 22.1 writes, verbatim.
    pub const SPEC_VALUE: &'static str = "HYPOTHETICAL";
}

/// Section 22.1's `expectedCoverage: probabilistic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProbabilisticCoverage;

impl ProbabilisticCoverage {
    /// The value section 22.1 writes, verbatim.
    pub const SPEC_VALUE: &'static str = "probabilistic";
}

/// Section 22.1's `workloadHoursRange` together with its `source`.
///
/// The range is an *input*: it is what the user or a review model assumes. The
/// simulator's output is [`crate::projected::ProjectedWorkload`], which seals
/// the same range inside `P2-C7`'s [`academic_scenario::Proposed`] and carries
/// the bias section 22.4 requires beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedWorkload {
    range: WorkloadHoursRange,
    source: ModelRunId,
}

impl AssumedWorkload {
    /// Records one assumed weekly range and the model run behind it.
    #[must_use]
    pub const fn of(range: WorkloadHoursRange, source: ModelRunId) -> Self {
        Self { range, source }
    }

    /// The assumed range.
    #[must_use]
    pub const fn range(&self) -> WorkloadHoursRange {
        self.range
    }

    /// The model run that produced it.
    #[must_use]
    pub const fn source(&self) -> ModelRunId {
        self.source
    }
}

/// The three assumptions one plan is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanAssumptions {
    workload: AssumedWorkload,
    completion: HypotheticalCompletion,
    coverage: ProbabilisticCoverage,
}

impl PlanAssumptions {
    /// Records the three.
    #[must_use]
    pub const fn of(
        workload: AssumedWorkload,
        completion: HypotheticalCompletion,
        coverage: ProbabilisticCoverage,
    ) -> Self {
        Self {
            workload,
            completion,
            coverage,
        }
    }

    /// The assumed weekly range and its source.
    #[must_use]
    pub const fn workload(&self) -> AssumedWorkload {
        self.workload
    }

    /// The completion status, which is always hypothetical.
    #[must_use]
    pub const fn completion(&self) -> HypotheticalCompletion {
        self.completion
    }

    /// The expected coverage, which is always probabilistic.
    #[must_use]
    pub const fn coverage(&self) -> ProbabilisticCoverage {
        self.coverage
    }
}

/// One grade the user stated for one of the plan's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatedGradeAssumption {
    offering_id: OfferingId,
    grade: GradeSymbol,
}

impl StatedGradeAssumption {
    /// Records one stated grade.
    #[must_use]
    pub const fn of(offering_id: OfferingId, grade: GradeSymbol) -> Self {
        Self { offering_id, grade }
    }

    /// Which offering the grade was stated for.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The stated grade.
    #[must_use]
    pub const fn grade(&self) -> GradeSymbol {
        self.grade
    }
}

/// Every grade the user stated, each offering at most once.
///
/// Private field and one constructor. There is no `Default` and no empty
/// constructor: an empty set of stated grades is spelled by having no
/// [`StatedGradeAssumptions`] at all, which is what makes the absent GPA in
/// [`crate::deterministic::DeterministicResults`] an absent value rather than a
/// value computed over nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatedGradeAssumptions {
    stated: Vec<StatedGradeAssumption>,
}

impl StatedGradeAssumptions {
    /// Records the stated grades.
    ///
    /// # Errors
    ///
    /// [`WhatIfError::DuplicateGradeAssumption`] when one offering was given
    /// two grades. Nothing here picks the later one: two stated grades for one
    /// course is a question for the user, not a precedence rule.
    pub fn stating(stated: Vec<StatedGradeAssumption>) -> Result<Self, WhatIfError> {
        for (index, assumption) in stated.iter().enumerate() {
            if stated[..index]
                .iter()
                .any(|earlier| earlier.offering_id() == assumption.offering_id())
            {
                return Err(WhatIfError::DuplicateGradeAssumption(
                    assumption.offering_id(),
                ));
            }
        }
        Ok(Self { stated })
    }

    /// The stated grades, in the caller's order.
    #[must_use]
    pub fn stated(&self) -> &[StatedGradeAssumption] {
        &self.stated
    }

    /// The grade stated for one offering, if the user stated one.
    #[must_use]
    pub fn grade_for(&self, offering_id: OfferingId) -> Option<GradeSymbol> {
        self.stated
            .iter()
            .find(|assumption| assumption.offering_id() == offering_id)
            .map(StatedGradeAssumption::grade)
    }
}

impl TryFrom<Vec<StatedGradeAssumption>> for StatedGradeAssumptions {
    type Error = WhatIfError;

    fn try_from(stated: Vec<StatedGradeAssumption>) -> Result<Self, Self::Error> {
        Self::stating(stated)
    }
}

impl From<StatedGradeAssumptions> for Vec<StatedGradeAssumption> {
    fn from(value: StatedGradeAssumptions) -> Self {
        value.stated
    }
}
