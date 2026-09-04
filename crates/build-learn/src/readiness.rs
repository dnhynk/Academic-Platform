//! Section 20.2's readiness categories, and the count the design document
//! states two different ways.
//!
//! ## The design document says five in one place and six in another
//!
//! The reverse-path drawing's fifth line is
//!
//! > `ready / refresh / direct need / conditional / later-scale`
//!
//! which names **five**. The table under `결과는 다음 범주로 제시한다` has
//! **six** rows: `이미 준비됨`, `refresh 필요`, `현재 약함`, `구현에 직접 필요`,
//! `선택에 따라 필요`, `규모/조건이 바뀌면`. `t001` derives six consecutive
//! requirements from the table, `REQ-20-008`–`REQ-20-013`, and derives the
//! drawing's five in `REQ-20-006`.
//!
//! `t068` names the acceptance test `five_readiness_categories_map_exactly`.
//! Both readings are kept and neither is invented away:
//!
//! * [`ReadinessCategory`] has **six** variants, one per table row, because the
//!   table is what a result is presented as and six requirements enumerate it.
//! * [`SHORT_NAMES`] is the drawing's five, in the drawing's order, each paired
//!   with the table row it names.
//! * `five_readiness_categories_map_exactly` parses **both** out of
//!   `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` at run time and
//!   requires the five to be an order-preserving injection into the six, with
//!   the residue — `현재 약함` — named. It fails if the document stops saying
//!   either thing.
//!
//! `docs/contracts/build-to-learn.md` records the discrepancy. Five is not a
//! number written here and neither is six.
//!
//! ## The rule is a total function of two things, and its order is pinned
//!
//! A category is decided by where the requirement came from and by what the
//! user's overlay says, in [`RESOLUTION_ORDER`]'s order. Two of the six rows
//! speak about evidence the user has, three about why the concept is on the list
//! at all, and one about a benefit whose trigger has not fired; the order is
//! what makes the six mutually exclusive rather than a table a reader has to
//! adjudicate. [`categorize`] is total over [`RequirementOrigin`] and
//! [`academic_gap::ConceptState`] with no wildcard arm.
//!
//! The evidence half is read and not recomputed. `충분하고 최근인 evidence` is
//! `P2-N2`'s [`academic_knowledge_state::SufficiencyGap`] list being empty,
//! reached through `P2-N5`'s overlay; `stale` is `P2-N3`'s band, read through
//! `P2-N6`'s [`academic_critical_path::is_stale`]. This crate has no ladder, no
//! decay, no threshold and no clock.

use academic_critical_path::is_stale;
use academic_domain::{FreshnessBand, MasteryLevel};
use academic_gap::ConceptState;
use academic_repository_classification::BenefitContract;
use serde::{Deserialize, Serialize};

use crate::{
    branch::{ConceptRequirement, RequirementCondition},
    text::PartId,
};

/// Section 20.2's six result rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadinessCategory {
    /// `이미 준비됨` — `충분하고 최근인 evidence`.
    AlreadyReady,
    /// `refresh 필요` — `mastery evidence는 있으나 stale`.
    RefreshNeeded,
    /// `현재 약함` — `직접 prerequisite이나 evidence 부족`.
    CurrentlyWeak,
    /// `구현에 직접 필요` — `성공 조건 자체를 정의`.
    DirectImplementationNeed,
    /// `선택에 따라 필요` — `architecture branch에 종속`.
    ConditionalOnChoice,
    /// `규모/조건이 바뀌면` — `trigger 기반 benefit`.
    LaterScale,
}

/// The six, in the table's own row order.
pub const READINESS_CATEGORIES: [ReadinessCategory; 6] = [
    ReadinessCategory::AlreadyReady,
    ReadinessCategory::RefreshNeeded,
    ReadinessCategory::CurrentlyWeak,
    ReadinessCategory::DirectImplementationNeed,
    ReadinessCategory::ConditionalOnChoice,
    ReadinessCategory::LaterScale,
];

/// The reverse-path drawing's five short names, paired with the row each names.
///
/// In the drawing's own order, which is the table's order with the third row
/// left out. See the module note.
pub const SHORT_NAMES: [(&str, ReadinessCategory); 5] = [
    ("ready", ReadinessCategory::AlreadyReady),
    ("refresh", ReadinessCategory::RefreshNeeded),
    ("direct need", ReadinessCategory::DirectImplementationNeed),
    ("conditional", ReadinessCategory::ConditionalOnChoice),
    ("later-scale", ReadinessCategory::LaterScale),
];

/// The one table row the drawing's five do not name.
pub const ROW_WITHOUT_A_SHORT_NAME: ReadinessCategory = ReadinessCategory::CurrentlyWeak;

impl ReadinessCategory {
    /// The table's `범주` cell, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::AlreadyReady => "이미 준비됨",
            Self::RefreshNeeded => "refresh 필요",
            Self::CurrentlyWeak => "현재 약함",
            Self::DirectImplementationNeed => "구현에 직접 필요",
            Self::ConditionalOnChoice => "선택에 따라 필요",
            Self::LaterScale => "규모/조건이 바뀌면",
        }
    }

    /// The table's `뜻` cell, verbatim.
    #[must_use]
    pub const fn meaning_token(self) -> &'static str {
        match self {
            Self::AlreadyReady => "충분하고 최근인 evidence",
            Self::RefreshNeeded => "mastery evidence는 있으나 stale",
            Self::CurrentlyWeak => "직접 prerequisite이나 evidence 부족",
            Self::DirectImplementationNeed => "성공 조건 자체를 정의",
            Self::ConditionalOnChoice => "architecture branch에 종속",
            Self::LaterScale => "trigger 기반 benefit",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyReady => "ALREADY_READY",
            Self::RefreshNeeded => "REFRESH_NEEDED",
            Self::CurrentlyWeak => "CURRENTLY_WEAK",
            Self::DirectImplementationNeed => "DIRECT_IMPLEMENTATION_NEED",
            Self::ConditionalOnChoice => "CONDITIONAL_ON_CHOICE",
            Self::LaterScale => "LATER_SCALE",
        }
    }

    /// The drawing's short name, when the drawing names this row.
    #[must_use]
    pub fn short_name(self) -> Option<&'static str> {
        SHORT_NAMES
            .iter()
            .find(|(_, category)| *category == self)
            .map(|(name, _)| *name)
    }
}

/// Why a concept is on the plan at all.
///
/// The three arms are the three provenances section 20.2's table distinguishes:
/// a concept the success criterion itself is about, a concept an unresolved
/// decision brings with it, and a `P2-R4` `WOULD_BENEFIT_FROM` whose trigger has
/// not fired. `P2-R4` owns the third — the contract carries the trigger and the
/// trade-off it was published with, and this crate re-derives neither.
///
/// **Not serialized**, and deliberately: `P2-R4`'s
/// [`academic_repository_classification::BenefitContract`] implements neither
/// `Serialize` nor `Deserialize`, and giving one a wire form here would be a
/// second serialization of a value that crate chose not to publish. So this
/// type and [`ReadinessFinding`] are in-memory values; the wire forms in this
/// crate are the goal, its four groups, the plan steps and the motivation
/// display, whose key sets are compared as whole sets in
/// `goal_schema_separates_four_groups` and `motivation_edges_are_shown_in_parallel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementOrigin {
    /// `성공 조건 자체를 정의`: the criterion the requirement's responsibility
    /// serves is about this concept.
    DefinesSuccessCriterion {
        /// The criterion it defines.
        criterion: PartId,
    },
    /// The concept is reached through the prerequisite neighbourhood of one that
    /// is required, rather than being required in its own right.
    PrerequisiteNeighbour {
        /// The requirement it is a prerequisite of.
        of_concept: PartId,
    },
    /// `trigger 기반 benefit`: `P2-R4`'s contract, carried whole.
    BenefitTrigger(Box<BenefitContract>),
}

impl RequirementOrigin {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::DefinesSuccessCriterion { .. } => "DEFINES_SUCCESS_CRITERION",
            Self::PrerequisiteNeighbour { .. } => "PREREQUISITE_NEIGHBOUR",
            Self::BenefitTrigger(_) => "BENEFIT_TRIGGER",
        }
    }
}

/// The order [`categorize`] answers in, as text a reader can check.
///
/// Pinned as data rather than described in a comment, because the ordering is
/// the whole content of the rule: two rows are about the user's evidence, three
/// about provenance, one about a benefit that is not required yet, and without a
/// stated order more than one row would be true of the same requirement.
pub const RESOLUTION_ORDER: [(&str, ReadinessCategory); 6] = [
    (
        "a P2-R4 benefit contract, whatever the overlay says",
        ReadinessCategory::LaterScale,
    ),
    (
        "no sufficiency gap and not stale",
        ReadinessCategory::AlreadyReady,
    ),
    (
        "no sufficiency gap and stale",
        ReadinessCategory::RefreshNeeded,
    ),
    (
        "a gap, and conditional on an open decision",
        ReadinessCategory::ConditionalOnChoice,
    ),
    (
        "a gap, and the criterion is about this concept",
        ReadinessCategory::DirectImplementationNeed,
    ),
    (
        "a gap, reached as a prerequisite neighbour",
        ReadinessCategory::CurrentlyWeak,
    ),
];

/// One requirement, compared against the user's state. Not serialized; see
/// [`RequirementOrigin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessFinding {
    requirement: ConceptRequirement,
    origin: RequirementOrigin,
    category: ReadinessCategory,
    mastery: MasteryLevel,
    freshness: FreshnessBand,
    sufficiency_gap_count: usize,
}

impl ReadinessFinding {
    /// The requirement this is about.
    #[must_use]
    pub const fn requirement(&self) -> &ConceptRequirement {
        &self.requirement
    }

    /// Why the concept is on the plan.
    #[must_use]
    pub const fn origin(&self) -> &RequirementOrigin {
        &self.origin
    }

    /// Which of the six rows it is presented under.
    #[must_use]
    pub const fn category(&self) -> ReadinessCategory {
        self.category
    }

    /// `P2-N2`'s rung, as `P2-N5`'s overlay reported it.
    #[must_use]
    pub const fn mastery(&self) -> MasteryLevel {
        self.mastery
    }

    /// `P2-N3`'s band, as `P2-N5`'s overlay reported it.
    #[must_use]
    pub const fn freshness(&self) -> FreshnessBand {
        self.freshness
    }

    /// How many `P2-N2` sufficiency gaps the overlay carried.
    #[must_use]
    pub const fn sufficiency_gap_count(&self) -> usize {
        self.sufficiency_gap_count
    }
}

/// Section 20.2's comparison, for one requirement and one overlay.
///
/// Total over [`RequirementOrigin`], [`RequirementCondition`] and the overlay's
/// two readings, with no wildcard arm, in [`RESOLUTION_ORDER`]'s order.
#[must_use]
pub fn categorize(
    requirement: &ConceptRequirement,
    origin: RequirementOrigin,
    state: &ConceptState,
) -> ReadinessFinding {
    let sufficient = state.sufficiency_gaps().is_empty();
    let stale = is_stale(state.freshness());
    let category = match &origin {
        RequirementOrigin::BenefitTrigger(_) => ReadinessCategory::LaterScale,
        RequirementOrigin::DefinesSuccessCriterion { .. }
        | RequirementOrigin::PrerequisiteNeighbour { .. }
            if sufficient && !stale =>
        {
            ReadinessCategory::AlreadyReady
        }
        RequirementOrigin::DefinesSuccessCriterion { .. }
        | RequirementOrigin::PrerequisiteNeighbour { .. }
            if sufficient =>
        {
            ReadinessCategory::RefreshNeeded
        }
        RequirementOrigin::DefinesSuccessCriterion { .. }
        | RequirementOrigin::PrerequisiteNeighbour { .. } => match requirement.condition() {
            RequirementCondition::Conditional { .. } => ReadinessCategory::ConditionalOnChoice,
            RequirementCondition::Unconditional => match &origin {
                RequirementOrigin::DefinesSuccessCriterion { .. } => {
                    ReadinessCategory::DirectImplementationNeed
                }
                RequirementOrigin::PrerequisiteNeighbour { .. }
                | RequirementOrigin::BenefitTrigger(_) => ReadinessCategory::CurrentlyWeak,
            },
        },
    };
    ReadinessFinding {
        requirement: requirement.clone(),
        origin,
        category,
        mastery: state.mastery(),
        freshness: state.freshness(),
        sufficiency_gap_count: state.sufficiency_gaps().len(),
    }
}
