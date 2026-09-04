//! `P2-R6`: section 20's Build → Learn mode and section 21's course ↔ project
//! mapping — the order a goal is normalised in, and the four things that order
//! makes impossible.
//!
//! `P2-R4` answered *which concepts this project observes, requires and would
//! benefit from*. `P2-N6` answered *which whole set of concepts satisfies a
//! goal, and at what cost*. This crate answers the question section 20 opens
//! with, and it is mostly a question about refusing to answer too early:
//!
//! > 시스템은 이를 바로 기술 목록으로 바꾸지 않고 성공 조건과 선택 지점을
//! > 추출한다.
//!
//! ## What holds section 20, and where
//!
//! | Section 20 rule | What holds it |
//! |---|---|
//! | six input kinds normalise | [`input::INPUT_KINDS`], measured against the design document in both directions |
//! | criteria and choices precede technology | [`technology::TechnologySlate::under`] is the only producer and takes a [`goal::ProjectGoal`], which takes a [`goal::SuccessCriteria`] by value that cannot be built from `[]` |
//! | the goal schema separates four groups | [`goal::ProjectGoal`] holds four **types**; a decision has alternatives and a constraint has no field one could go in |
//! | responsibilities precede the architecture branch | [`branch::ArchitectureBranch::of`] takes a [`responsibility::ResponsibilityDecomposition`] by value, and that takes a `ProjectGoal` by value |
//! | AND/OR branches are conditional | [`branch::BranchGroup::of`] stamps the condition; no public constructor of a [`branch::ConceptRequirement`] takes one as an argument |
//! | five readiness categories map exactly | [`readiness::SHORT_NAMES`] and [`readiness::READINESS_CATEGORIES`], both parsed back out of the design document |
//! | a learning item carries both | [`learning::LearningItem::plan`] takes an [`learning::EvidenceTask`] and a [`learning::ReturnCheckpoint`] by value |
//! | the four checkpoint stages are ordered | each of [`learning::ReadingDone`] → [`learning::ExplainedByHand`] → [`learning::SimulationPassed`] → [`learning::SelectionApproved`] takes the previous by value |
//! | a lecture-list-only plan is refused | [`validate::validate`] reads four structural absences and no word |
//! | the three motivation edges are never summed | [`motivation::MotivationDisplay`] hands out rows; no conversion trait and no folding signature exists in this crate |
//!
//! `crates/build-learn/tests/compile_fail/` holds the compiled half.
//!
//! ## The counts are measurements of the design document
//!
//! `six_input_kinds_normalize` and `five_readiness_categories_map_exactly` each
//! read section 20 back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compare it against
//! [`input::INPUT_KINDS`], [`readiness::READINESS_CATEGORIES`] and
//! [`readiness::SHORT_NAMES`] in both directions. Six and five are measurements.
//!
//! One reading the design document leaves open is recorded rather than
//! reconciled:
//!
//! * **Section 20.2 names five readiness categories in its drawing and six in
//!   its table.** The drawing's line is `ready / refresh / direct need /
//!   conditional / later-scale`; the table has six rows and `t001` derives six
//!   consecutive requirements, `REQ-20-008`–`REQ-20-013`, from them. The row the
//!   drawing does not name is `현재 약함`.
//!   [`readiness::ROW_WITHOUT_A_SHORT_NAME`] records which, and the acceptance
//!   test requires the five to be an order-preserving injection into the six —
//!   so it fails if the document stops saying either thing.
//!
//! `docs/contracts/build-to-learn.md` records it.
//!
//! ## It is not a thirteenth `P2-C5` engine, and it does not solve
//!
//! Section 28's table has twelve rows and none of them is a build-to-learn
//! planner, so no registry row is added, no `engine_id` is claimed and nothing
//! appears under `testdata/engines/`. And the AND/OR structure section 20.2
//! derives **is** section 16.1's hypergraph:
//! [`branch::ArchitectureBranch::satisfying_sets`] delegates to
//! [`academic_critical_path::satisfying_sets`]. There is no solver here and no
//! path length anywhere in this crate.
//!
//! ## It opens nothing
//!
//! Every goal, snapshot identity, overlay, revision and offering arrives as an
//! argument, no path is read, no clock is read, and this crate holds no
//! repository byte at all.
//! `the_build_learn_crate_touches_no_file_and_no_socket` compares the whole set
//! of its `use` items, the whole set of the paths it reaches through a crate
//! root, and the whole set of the macros it invokes against pinned inventories,
//! in both directions.
//!
//! ## What this task does not decide
//!
//! * **Whether a concept is understood.** `P2-N2` owns the ladder and `P2-N3`
//!   the band; [`readiness::categorize`] reads `P2-N5`'s overlay and computes
//!   neither.
//! * **What a concept costs.** `P2-N6` owns the vectors and the Pareto
//!   elimination. [`mapping::ChannelComparison`] holds two of that crate's
//!   estimates side by side and folds neither.
//! * **Whether an offering runs.** `P2-U1` owns section 8.3's four standings and
//!   this crate reads one.
//! * **Whether the repository observes a concept.** `P2-R4` owns section 18's
//!   three classifications; [`readiness::RequirementOrigin::BenefitTrigger`]
//!   carries that crate's contract whole.
//! * **Persistence.** Nothing here is written. There is no migration and no edge
//!   to `academic-store`.
//! * **`§38`.** `P2-R6` opens and closes no gate.

pub mod branch;
pub mod goal;
pub mod input;
pub mod learning;
pub mod mapping;
pub mod motivation;
pub mod plan;
pub mod readiness;
pub mod responsibility;
pub mod technology;
pub mod text;
pub mod validate;

pub use branch::{ArchitectureBranch, BranchGroup, ConceptRequirement, RequirementCondition};
pub use goal::{
    Alternative, Constraint, Constraints, ObservableCriterion, ProjectGoal, SuccessCriteria,
    UnresolvedDecision, UnresolvedDecisions,
};
pub use input::{GoalInput, INPUT_KINDS, InputKind, NormalizedIntent, normalize};
pub use learning::{
    CHECKPOINT_STAGES, CheckpointStage, EvidenceTask, ExplainedByHand, LearningItem, ReadingDone,
    ReturnCheckpoint, SelectionApproved, SimulationPassed,
};
pub use mapping::{
    ActualCoverage, COVERAGE_EVIDENCE_KINDS, ChannelComparison, CourseProjectMapping,
    CoverageEvidenceKind, DesignedCoverage, EnrolmentStanding, MAPPING_STATUSES, MappingEvidence,
    MappingStatus, PersonalEvidenceStanding,
};
pub use motivation::{MOTIVATIONS, Motivation, MotivationDisplay, MotivationEdge, MotivationRow};
pub use plan::{PlanDraft, PlanStep, STEP_KINDS};
pub use readiness::{
    READINESS_CATEGORIES, RESOLUTION_ORDER, ROW_WITHOUT_A_SHORT_NAME, ReadinessCategory,
    ReadinessFinding, RequirementOrigin, SHORT_NAMES, categorize,
};
pub use responsibility::{ObservableResponsibility, ResponsibilityDecomposition};
pub use technology::{TechnologyEntry, TechnologySlate};
pub use text::{NonEmptyText, PartId};
pub use validate::{PLAN_DEFECT_KINDS, PlanDefect, PlanVerdict, ValidatedPlan, validate};

use academic_domain::{EntityId, entity_registry::EntityKind};

/// Why a build-to-learn step was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BuildLearnError {
    /// A required sentence carried nothing but whitespace.
    #[error("the text is empty")]
    EmptyText,
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the identifier {0:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(String),
    /// A specification input offered no statement.
    #[error("the specification carries no statement")]
    SpecificationHasNoStatement,
    /// Section 20.1's `successCriteria` was empty.
    #[error(
        "the goal states no success criteria; section 20.1 extracts those before anything else"
    )]
    GoalHasNoSuccessCriteria,
    /// Two success criteria shared an identity.
    #[error("two success criteria are both named {0}")]
    DuplicateCriterion(String),
    /// Two constraints shared an identity.
    #[error("two constraints are both named {0}")]
    DuplicateConstraint(String),
    /// Two unresolved decisions shared an identity.
    #[error("two unresolved decisions are both named {0}")]
    DuplicateDecision(String),
    /// Two alternatives of one decision shared an identity.
    #[error("two alternatives are both named {0}")]
    DuplicateAlternative(String),
    /// A decision offered fewer than two alternatives.
    #[error("decision {0} offers fewer than two alternatives; that is a constraint, not a choice")]
    DecisionHasOneAlternative(String),
    /// Two responsibilities shared an identity.
    #[error("two responsibilities are both named {0}")]
    DuplicateResponsibility(String),
    /// A responsibility named a criterion the goal does not hold.
    #[error("responsibility {responsibility} serves {criterion}, which this goal does not state")]
    ResponsibilityServesNoCriterion {
        /// The responsibility.
        responsibility: String,
        /// The criterion it named.
        criterion: String,
    },
    /// A success criterion was served by no responsibility.
    #[error("success criterion {0} is served by no responsibility")]
    CriterionHasNoResponsibility(String),
    /// Section 7.4's rule: a tier that carries no independent prerequisite.
    #[error("a concept at tier {} carries no independent prerequisite of its own", .kind.as_str())]
    ConceptCarriesNoPrerequisite {
        /// The tier that was offered.
        kind: EntityKind,
    },
    /// A branch group offered no member.
    #[error("branch {alternative} of decision {decision} brings no concept with it")]
    EmptyBranchGroup {
        /// The decision.
        decision: String,
        /// The alternative.
        alternative: String,
    },
    /// A requirement named a responsibility the decomposition does not hold.
    #[error(
        "the requirement on {concept} serves {responsibility}, which is not in this decomposition"
    )]
    RequirementServesNoResponsibility {
        /// The concept.
        concept: String,
        /// The responsibility it named.
        responsibility: String,
    },
    /// A branch group named a decision the goal did not leave open.
    #[error("a branch names decision {0}, which this goal does not leave open")]
    BranchNamesNoDecision(String),
    /// A branch group named an alternative the decision does not offer.
    #[error(
        "a branch names alternative {alternative} of decision {decision}, which is not one of its alternatives"
    )]
    BranchNamesNoAlternative {
        /// The decision.
        decision: String,
        /// The alternative it named.
        alternative: String,
    },
    /// An open decision was offered fewer than two branch groups.
    #[error(
        "decision {0} is offered fewer than two distinct branches; a `ONE OF` with one branch is a conjunction wearing the other shape's name"
    )]
    DecisionHasOneBranch(String),
    /// An open decision was offered no branch group at all.
    #[error("decision {0} is left open by the goal and no branch answers it")]
    DecisionHasNoBranch(String),
    /// A requirement had no admitted prerequisite edge to build a member from.
    #[error("the requirement on {0} has no admitted P2-N5 edge")]
    RequirementHasNoAdmittedEdge(EntityId),
    /// A motivation edge named a different concept.
    #[error("a motivation edge is about {found}, not {expected}")]
    MotivationEdgeIsAboutAnotherConcept {
        /// The concept the display is about.
        expected: String,
        /// The concept the edge named.
        found: String,
    },
    /// One motivation label arrived twice for one concept.
    #[error("the {0} motivation arrives twice for one concept")]
    DuplicateMotivationEdge(&'static str),
    /// An actual coverage was offered with nothing that observed it.
    #[error("the coverage claim on {0} cites no syllabus, lecture, assignment or assessment")]
    CoverageHasNoEvidence(String),
    /// A coverage named a different subject from the mapping it was offered to.
    #[error("the coverage is about {found}, not {expected}")]
    CoverageIsAboutAnotherSubject {
        /// The subject the mapping is about.
        expected: String,
        /// The subject the coverage named.
        found: String,
    },
    /// Section 21.2's two offering-bound statuses, without an offering.
    #[error("{0} asserts that a particular offering covers the subject and none was observed")]
    StatusRequiresActualCoverage(&'static str),
    /// `P2-N5` refused an edge.
    #[error("P2-N5 refused the prerequisite edge: {0}")]
    Gap(String),
    /// `P2-N6` refused a hyperedge or a graph.
    #[error("P2-N6 refused the hypergraph: {0}")]
    CriticalPath(String),
}

impl From<academic_gap::GapError> for BuildLearnError {
    fn from(error: academic_gap::GapError) -> Self {
        Self::Gap(error.to_string())
    }
}

impl From<academic_critical_path::CriticalPathError> for BuildLearnError {
    fn from(error: academic_critical_path::CriticalPathError) -> Self {
        Self::CriticalPath(error.to_string())
    }
}
