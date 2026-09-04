//! Section 23's five states and the five preconditions that keep them apart.
//!
//! Neither the count nor the order is a number this file chose.
//! `five_states_are_semantically_distinct` reads section 23's own `text` block
//! back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compares its lines against [`BLIND_SPOT_STATES`] in both directions, name and
//! meaning cell alike.
//!
//! ## Distinct means a different precondition, not a different spelling
//!
//! Five names in one enumeration are five names. What makes them *semantically*
//! distinct here is [`StateBasis`]: one variant per state, each carrying a value
//! whose own constructor refuses the facts that state is not, and [`state_of`]
//! is a total match with no wildcard arm. The map is a bijection, which is what
//! `five_states_are_semantically_distinct` measures in both directions — no two
//! bases land on one state and no state is reachable from two bases.
//!
//! Every payload has private fields and a fallible constructor, so a basis is
//! not a record a caller fills in:
//!
//! | Basis | What its constructor refuses |
//! |---|---|
//! | [`BelowMinimum`] | an observed count at or above the minimum |
//! | [`ObservedDifficulty`] | an empty list of failed attempts |
//! | [`LowRecency`] | any band but [`LOW_RECENCY_BANDS`] |
//! | [`ScopeExclusion`] | a disposition that is not `NOT_RELEVANT` |
//! | [`GoalBlock`] | a goal that is its own blocking concept |
//!
//! ## Coverage cannot reach two of the five
//!
//! [`ExposureClass`] is the three states an evidence reading may yield. It has
//! no `OutOfScope` and no `Gap`, so:
//!
//! * `OUT_OF_SCOPE` is `사용자가 현재 탐색하지 않기로 함` and is reachable only
//!   through [`ScopeExclusion::of`], which takes a
//!   [`crate::disposition::UserDispositionChoice`] — a value ADR-003's actor
//!   matrix refuses every automatic actor; and
//! * `GAP` is `활성 목표를 실제로 막음`, which is `P2-N5`'s question. This crate
//!   has no `academic-gap` edge, so a `Gap` this engine minted out of a coverage
//!   reading is not an omission somebody has to remember to check for — it is a
//!   value [`ExposureClass`] cannot express.
//!
//! [`EXPOSURE_CLASSES`] and [`BLIND_SPOT_STATES`] are compared as sets, so the
//! three are measured to be a subset and `OUT_OF_SCOPE` and `GAP` are measured
//! to be exactly the complement.

use serde::{Deserialize, Serialize};

use academic_domain::{EntityId, EvidenceId, FreshnessBand, ScopeId};

use crate::{
    BlindSpotError,
    disposition::{UserDisposition, UserDispositionChoice},
};

/// Section 23's five states, in the order its `text` block writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlindSpotState {
    /// `evidence가 거의 없어 실력을 말할 수 없음`.
    Unobserved,
    /// `시도·평가 evidence에서 어려움이 관찰됨`.
    Weak,
    /// `과거 evidence는 있으나 최근성 낮음`.
    Stale,
    /// `사용자가 현재 탐색하지 않기로 함`.
    OutOfScope,
    /// `활성 목표를 실제로 막음`.
    Gap,
}

/// The five, in section 23's own order.
pub const BLIND_SPOT_STATES: [BlindSpotState; 5] = [
    BlindSpotState::Unobserved,
    BlindSpotState::Weak,
    BlindSpotState::Stale,
    BlindSpotState::OutOfScope,
    BlindSpotState::Gap,
];

impl BlindSpotState {
    /// Stable spelling, identical to the block's own left-hand column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unobserved => "UNOBSERVED",
            Self::Weak => "WEAK",
            Self::Stale => "STALE",
            Self::OutOfScope => "OUT_OF_SCOPE",
            Self::Gap => "GAP",
        }
    }

    /// The block's right-hand cell, verbatim.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::Unobserved => "evidence가 거의 없어 실력을 말할 수 없음",
            Self::Weak => "시도·평가 evidence에서 어려움이 관찰됨",
            Self::Stale => "과거 evidence는 있으나 최근성 낮음",
            Self::OutOfScope => "사용자가 현재 탐색하지 않기로 함",
            Self::Gap => "활성 목표를 실제로 막음",
        }
    }
}

/// The three states an evidence reading may yield.
///
/// There is no `OutOfScope` and no `Gap`: see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExposureClass {
    /// Coverage below the minimum the user selected.
    Unobserved,
    /// An admitted attempt whose recorded outcome was a failure.
    Weak,
    /// Coverage present, no observed difficulty, recency low.
    Stale,
}

/// The three, in section 23's own order.
pub const EXPOSURE_CLASSES: [ExposureClass; 3] = [
    ExposureClass::Unobserved,
    ExposureClass::Weak,
    ExposureClass::Stale,
];

impl From<ExposureClass> for BlindSpotState {
    fn from(class: ExposureClass) -> Self {
        match class {
            ExposureClass::Unobserved => Self::Unobserved,
            ExposureClass::Weak => Self::Weak,
            ExposureClass::Stale => Self::Stale,
        }
    }
}

/// The bands section 23's `최근성 낮음` may be read off.
///
/// `P2-N3` owns the six and this crate computes none of them. `UNKNOWN` is not
/// here: it says nothing about recency, so it cannot support the claim that past
/// evidence has gone stale.
pub const LOW_RECENCY_BANDS: [FreshnessBand; 2] = [FreshnessBand::Stale, FreshnessBand::Low];

/// Section 23's `evidence가 거의 없어`, with the two numbers that made it true.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BelowMinimum {
    observed: u32,
    minimum: u32,
}

impl BelowMinimum {
    /// Records that `observed` admitted items fell below `minimum`.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::CoverageIsNotBelowMinimum`] when it did not.
    pub const fn of(observed: u32, minimum: u32) -> Result<Self, BlindSpotError> {
        if observed >= minimum {
            return Err(BlindSpotError::CoverageIsNotBelowMinimum { observed, minimum });
        }
        Ok(Self { observed, minimum })
    }

    /// How many admitted items the window held.
    #[must_use]
    pub const fn observed(self) -> u32 {
        self.observed
    }

    /// The minimum the user selected.
    #[must_use]
    pub const fn minimum(self) -> u32 {
        self.minimum
    }
}

/// Section 23's `시도·평가 evidence에서 어려움이 관찰됨`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedDifficulty {
    attempts: Vec<EvidenceId>,
}

impl ObservedDifficulty {
    /// Records the admitted attempts whose outcome `P2-N2` has as a failure.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::DifficultyHasNoAttempt`] for an empty list.
    pub fn of(attempts: Vec<EvidenceId>) -> Result<Self, BlindSpotError> {
        if attempts.is_empty() {
            return Err(BlindSpotError::DifficultyHasNoAttempt);
        }
        Ok(Self { attempts })
    }

    /// The failing attempts, in the order they were offered.
    #[must_use]
    pub fn attempts(&self) -> &[EvidenceId] {
        &self.attempts
    }
}

/// Section 23's `과거 evidence는 있으나 최근성 낮음`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LowRecency {
    band: FreshnessBand,
}

impl LowRecency {
    /// Carries `P2-N3`'s band for this field's newest admitted evidence.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::BandIsNotLowRecency`] for any band outside
    /// [`LOW_RECENCY_BANDS`].
    pub fn of(band: FreshnessBand) -> Result<Self, BlindSpotError> {
        if !LOW_RECENCY_BANDS.contains(&band) {
            return Err(BlindSpotError::BandIsNotLowRecency(band));
        }
        Ok(Self { band })
    }

    /// The band, unchanged.
    #[must_use]
    pub const fn band(self) -> FreshnessBand {
        self.band
    }
}

/// Section 23's `사용자가 현재 탐색하지 않기로 함`.
///
/// Reachable only from a verified user choice, so the one state that says the
/// user decided something is the one state a model run cannot produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeExclusion {
    user_id: EntityId,
    scope_id: ScopeId,
}

impl ScopeExclusion {
    /// Reads the exclusion off the user's own `NOT_RELEVANT` choice.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::ExclusionNeedsNotRelevant`] for any other disposition.
    pub fn of(choice: &UserDispositionChoice) -> Result<Self, BlindSpotError> {
        if choice.disposition() != UserDisposition::NotRelevant {
            return Err(BlindSpotError::ExclusionNeedsNotRelevant);
        }
        Ok(Self {
            user_id: choice.user_id(),
            scope_id: choice.scope_id(),
        })
    }

    /// Whose scope.
    #[must_use]
    pub const fn user_id(self) -> EntityId {
        self.user_id
    }

    /// Which resolution scope.
    #[must_use]
    pub const fn scope_id(self) -> ScopeId {
        self.scope_id
    }
}

/// Section 23's `활성 목표를 실제로 막음`, as `P2-N5` decided it.
///
/// This crate has no edge to `academic-gap` and computes nothing about goals:
/// the two identities arrive from the engine section 15 gave the question to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoalBlock {
    goal: EntityId,
    blocking_concept: EntityId,
}

impl GoalBlock {
    /// Carries `P2-N5`'s finding.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::GoalBlocksItself`] when the goal is its own blocking
    /// concept, which is not a prerequisite deficit but a naming mistake.
    pub fn of(goal: EntityId, blocking_concept: EntityId) -> Result<Self, BlindSpotError> {
        if goal == blocking_concept {
            return Err(BlindSpotError::GoalBlocksItself);
        }
        Ok(Self {
            goal,
            blocking_concept,
        })
    }

    /// The goal.
    #[must_use]
    pub const fn goal(self) -> EntityId {
        self.goal
    }

    /// The concept the blocking path ends at.
    #[must_use]
    pub const fn blocking_concept(self) -> EntityId {
        self.blocking_concept
    }
}

/// Why one field holds the state it holds.
///
/// One variant per state, each carrying a value whose own constructor refuses
/// the facts that state is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateBasis {
    /// Section 23's `evidence가 거의 없어`.
    CoverageBelowMinimum(BelowMinimum),
    /// Section 23's `시도·평가 evidence에서 어려움이 관찰됨`.
    DifficultyObserved(ObservedDifficulty),
    /// Section 23's `과거 evidence는 있으나 최근성 낮음`.
    RecencyLow(LowRecency),
    /// Section 23's `사용자가 현재 탐색하지 않기로 함`.
    UserExcluded(ScopeExclusion),
    /// Section 23's `활성 목표를 실제로 막음`.
    ActiveGoalBlocked(GoalBlock),
}

/// Which of section 23's five states a basis is.
///
/// Total, with no wildcard arm, and injective:
/// `five_states_are_semantically_distinct` compares the image of the five bases
/// against [`BLIND_SPOT_STATES`] in both directions.
#[must_use]
pub const fn state_of(basis: &StateBasis) -> BlindSpotState {
    match basis {
        StateBasis::CoverageBelowMinimum(_) => BlindSpotState::Unobserved,
        StateBasis::DifficultyObserved(_) => BlindSpotState::Weak,
        StateBasis::RecencyLow(_) => BlindSpotState::Stale,
        StateBasis::UserExcluded(_) => BlindSpotState::OutOfScope,
        StateBasis::ActiveGoalBlocked(_) => BlindSpotState::Gap,
    }
}
