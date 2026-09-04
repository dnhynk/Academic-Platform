//! `P2-N8`: section 22's what-if semester simulator — the plan a user asks
//! *what happens if I take this*, and the line between what it may state and
//! what it may only guess.
//!
//! The whole crate is one sentence: **an assumption must not leak into the
//! record.** A product that let it would be editing the user's own history to
//! match a plan they were only considering.
//!
//! ## What holds section 22, and where
//!
//! | Section 22 rule | What holds it |
//! |---|---|
//! | facts and assumptions are frozen apart | [`basis::ScenarioBasis`] holds section 22.1's four `basedOn` references and nothing readable; [`assumption::PlanAssumptions`] holds its three assumptions |
//! | the two lanes are separate types | [`deterministic::DeterministicResults`] and [`projected::ProjectedResults`] share no field, no constructor and no conversion |
//! | the two lanes are separate UI sections | [`lane::SectionView`]'s two arms borrow **different types**, so a projection has no position under the deterministic heading |
//! | a plan never writes actual state | no edge of any kind to `academic-store` or `academic-store-platform`; every output has private fields and one crate-private producer; `tests/compile_fail/` holds the expressions that must not compile |
//! | `ProjectedEvidenceOpportunity` is the only future-knowledge output | section 22.3's first three bullets are `P2-C7`'s [`academic_scenario::ScenarioProjection`], produced by that crate's own engine; this crate declares no second future-knowledge type |
//! | no mastery delta in the output | this crate never names `MasteryLevel`, `KnowledgeState` or any band of the ladder, measured over its whole identifier inventory with a control |
//! | workload is a range with bias | [`projected::ProjectedWorkload`] takes `P2-U8`'s `BiasDisclosure` **by value**, and there is no point accessor anywhere |
//! | `STALE_INPUT` freezes and asks | [`stale::FrozenPlan`] returns the plan unchanged and [`stale::FrozenPlan::recompute`] takes a [`stale::RecomputeConsent`] built from `P2-M2`'s `UserDecision` |
//! | the graduation modes are distinct | [`graduation::HypotheticalGraduation`] is the only one here, and `academic-audit` is not reachable from this package at all |
//! | no default recommendation score | [`comparison::DimensionPriority`] is a **permutation**, not a weight vector; no arithmetic combines two dimensions anywhere |
//! | a reordering explains itself | [`comparison::ReorderingExplanation::between`] names every moved weight and the dimension that decided the new leader, and refuses an unchanged priority |
//! | calibration evaluates the model | [`calibration::ModelCalibrationReport`] carries the engine version, the input digest and the model run, and no field about a person |
//!
//! ## The counts are measurements of the design document
//!
//! `deterministic_and_projected_are_separate_types_and_sections` reads sections
//! 22.2 and 22.3 back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares them
//! against [`lane::DETERMINISTIC_LANE`] and [`lane::PROJECTED_LANE`] in both
//! directions. `scenario_basis_round_trip` does the same for section 22.1's
//! `basedOn` block, and the comparison and staleness suites for section 22.4's
//! table and section 22.5's fourth bullet. Seven, seven, four, three, six and
//! six are measurements, and no test in this crate asserts a count on its own.
//!
//! Two readings the design document leaves open are recorded rather than
//! reconciled:
//!
//! * **Section 22.4's `후속 경로` row is `Mixed` because section 22 puts its two
//!   halves in different lanes.** The official prerequisite unlock is section
//!   22.2's sixth bullet and the informal readiness is section 22.3's seventh.
//!   [`comparison::DimensionLane::Mixed`] is that cell, and folding the row
//!   onto one side would have made one half claim the other's certainty.
//! * **[`scenario::PlanScenario`] carries a seventh field section 22.1 does not
//!   name.** `inputs_digest` is the binding that makes a plan reproducible,
//!   stale-markable and calibratable, all three of which section 22.5 requires
//!   and none of which section 22.1's block provides a key for.
//!
//! `docs/contracts/what-if-simulator.md` records both.
//!
//! ## What this task does not decide
//!
//! * **Whether the user can graduate.** `P2-U3` owns section 11's audit and its
//!   three-gate `DETERMINATE` rule. This crate cannot reach it: see
//!   [`graduation`].
//! * **Whether an offering runs.** `P2-U5` owns section 8.3's four standings.
//!   This crate reads a [`academic_offering::ConfirmedSeat`], which only a
//!   confirmed standing produces.
//! * **What is on a critical path.** `P2-N6` owns section 16. This crate reads
//!   [`academic_critical_path::CriticalPathResult::roles`] through
//!   [`inputs::PathCoverageTargets::from_critical_path`] and computes no path.
//! * **What a review says.** `P2-U8` owns section 29.5's aggregate and its six
//!   bias dimensions; this crate carries one and produces none.
//! * **What the user knows, now or later.** `P2-N2` and `P2-N3` own the ladder,
//!   and this crate has no word for it.
//! * **Persistence.** Nothing here is written. There is no migration and no
//!   edge to `academic-store`. It opens no file, opens no socket and reads no
//!   clock: every instant arrives inside a caller-supplied value.
//! * **`§38`.** `P2-N8` opens and closes no gate.

pub mod assumption;
pub mod basis;
pub mod calibration;
pub mod comparison;
pub mod deterministic;
pub mod error;
pub mod graduation;
pub mod inputs;
pub mod lane;
pub mod projected;
pub mod scenario;
pub mod stale;

pub use assumption::{
    ASSUMPTION_KINDS, AssumedWorkload, AssumptionKind, HypotheticalCompletion, PlanAssumptions,
    ProbabilisticCoverage, StatedGradeAssumption, StatedGradeAssumptions,
};
pub use basis::{BASIS_FIELDS, BasisField, ScenarioBasis};
pub use calibration::{CalibrationEntry, ModelCalibrationReport, ObservedOccasion, calibrate};
pub use comparison::{
    COMPARISON_DIMENSIONS, ComparisonDimension, ComparisonView, DimensionLane, DimensionMove,
    DimensionPriority, ReorderingExplanation, compare,
};
pub use deterministic::{
    Allocation, AllocationLine, AverageExclusion, CategoryContribution, CreditLoad,
    DeterministicResults, DownstreamUnlock, ENROLMENT_LIMIT_STANDINGS, EnrolmentLimitStanding,
    GpaScenario, HypotheticalTermAverage, PrerequisiteStanding, RequirementVerdict,
    RuleContribution, ScheduleConflict, ScheduleConflicts, UnlockStanding,
};
pub use error::WhatIfError;
pub use graduation::{GRADUATION_MODES, GraduationMode, HypotheticalGraduation};
pub use inputs::{
    CatalogueRow, DownstreamCourse, InformalRecommendation, OfficialConditions, PLAN_CHOICE_FIELDS,
    PLAN_INPUT_FIELDS, PathCoverageTargets, PlanChoice, PlanChoiceField, PlanInputField,
    PlanInputs, RELEVANCE_SUBJECTS, RelevanceSignal, RelevanceSubject, plan_inputs_digest,
};
pub use lane::{
    DETERMINISTIC_LANE, DeterministicItem, LaneItem, PROJECTED_LANE, ProjectedItem, SectionView,
    UI_SECTIONS, UiSection,
};
pub use projected::{
    InformalReadiness, InformalReadinessEntry, PathCoverage, PathCoverageEntry, ProjectedResults,
    ProjectedWorkload, RelevanceEntry, RelevanceProjection,
};
pub use scenario::{PlanScenario, SCENARIO_KEYS, ScenarioKey, simulate};
pub use stale::{FrozenPlan, RecomputeConsent, STALE_CAUSES, STALE_INPUT, StaleCause, StaleInput};

/// Engine version stamped into every proposal this crate seals.
///
/// A different number from `academic_scenario::SCENARIO_ENGINE_VERSION` on
/// purpose: the workload proposal this crate seals is produced by this engine
/// over this engine's frozen inputs, and a shared version would let a payload
/// from one be replayed as the output of the other.
pub const WHAT_IF_ENGINE_VERSION: u32 = 1;
