//! `P2-N6`: section 16's critical path engine — the AND/OR hypergraph it
//! satisfies, the two vectors it never folds, the eight constraints it applies,
//! the elimination that runs before any preference, and the five things it
//! always discloses.
//!
//! `P2-N5` answered *which prerequisite deficit blocks this goal, and why*.
//! This crate answers the question section 16.1 opens with, and it is mostly a
//! question about refusing to answer too confidently:
//!
//! > Critical Path는 concept 수가 가장 적은 shortest path가 아니다.
//!
//! ## What holds section 16, and where
//!
//! | Section 16 rule | What holds it |
//! |---|---|
//! | the answer is satisfaction, not a shortest path | [`hypergraph::satisfying_sets`] returns **sets**; [`hypergraph::shortest_by_node_count`] is the wrong answer, implemented once so the suite can show the two differ, and unreachable from [`engine::plan`] |
//! | seven cost axes stay apart | [`vector::CostVector`] has seven private fields, one accessor per named axis, and no `total`, `sum`, `score`, `Ord` or numeric conversion |
//! | five benefit axes stay apart | [`vector::BenefitVector`], the same way |
//! | a slider reorders and never rewrites | [`preference::rank`] takes `&`[`pareto::ParetoFront`] and returns a [`preference::Ranking`] that **borrows** it; there is no `&mut` and no interior mutability in this crate |
//! | elimination happens first | [`pareto::ParetoFront`]'s one constructor is [`pareto::ParetoFront::eliminate`]; the ranker takes nothing else |
//! | a named strategy is only a slider | [`preference::NamedStrategy::slider`] is its whole output |
//! | an unknown cost is a range | [`vector::CostEstimate`] has `low` and `high` and **no** `point`, `midpoint` or `value`; an unmeasured one with `low == high` is refused |
//! | a course is an acquisition option | [`option::AcquisitionOption`] hands out [`option::Opportunity`] values and has no function returning a mastery, a state or a satisfied concept |
//! | all eight constraints are answered | [`constraint::evaluate`] returns `[ConstraintFinding; 8]`, one per [`constraint::CONSTRAINTS`] member, with no filter and no `Option` |
//! | an uncertain ratio inserts a checkpoint | [`checkpoint::CheckpointDecision::for_ratio`], and it is section 16.3's **eighth constraint** rather than a ninth rule |
//! | an edge's counterfactual is computed | [`counterfactual::sensitivity`] re-runs the same solver without the edge |
//! | an edit keeps the base | [`edit::EditedPlan::apply`] carries the **original** base forward, never the previous recomputation |
//! | five groups are always disclosed | [`disclosure::Disclosure`] has five private fields, one constructor taking all five, no `Default`, and [`plan::CriticalPathResult`] holds one by value |
//!
//! `crates/critical-path/tests/compile_fail/` holds the compiled half.
//!
//! ## The counts are measurements of the design document
//!
//! `cost_vector_has_seven_separate_components`,
//! `benefit_vector_has_five_separate_components`,
//! `eight_constraints_are_enforced` and
//! `five_disclosure_groups_are_always_present` each read section 16 back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compare it against
//! [`vector::COST_COMPONENTS`], [`vector::BENEFIT_COMPONENTS`],
//! [`constraint::CONSTRAINTS`] and [`disclosure::DISCLOSURE_GROUPS`] in both
//! directions. Seven, five, eight and five are measurements.
//!
//! Two readings the design document leaves open are recorded rather than
//! reconciled, because `t068`'s own instruction is that every count in it is
//! derived and unverified:
//!
//! * **Section 16.2's four strategy names are examples.** The sentence is
//!   `“빠른 project unblock”, ... 같은 이름으로`, and `같은` is *such as*.
//!   `REQ-16-006`'s acceptance evidence fixes four, so
//!   [`preference::NAMED_STRATEGIES`] has four and
//!   [`preference::STRATEGY_NAMES_ARE_EXAMPLES`] records the hedge.
//! * **Section 16.3's eighth constraint and the checkpoint rule are one rule.**
//!   `t068` names `eight_constraints_are_enforced` and
//!   `uncertain_edge_ratio_inserts_diagnostic_checkpoint` separately; the
//!   design document has eight bullets and the last one *is* the checkpoint. A
//!   reader who counted the two acceptance rows would look for a ninth bullet
//!   that does not exist.
//!
//! `docs/contracts/critical-path.md` records both.
//!
//! ## This crate is not a thirteenth `P2-C5` engine
//!
//! Section 28's table has twelve rows and none of them is a critical path
//! engine, and `P2-C5`'s registry is pinned to that table as an enumeration. So
//! no registry row is added, no `engine_id` is claimed, and nothing appears
//! under `testdata/engines/`. Determinism is proved in `P2-C5`'s own
//! vocabulary over this crate's own corpus. See [`proof`].
//!
//! ## One concept's evidence never becomes another's, here either
//!
//! `P2-N2` closed that at the history, `P2-N3` closed the one-hop form and
//! `P2-N5` closed the form where a band raised by a neighbour on the blocking
//! path decides that neighbour's own deficit. This engine takes a
//! [`academic_gap::GapCase`] as its input rather than a concept and a state, so
//! every band it plans around was already refused or admitted there. What it
//! adds is one boundary of its own: [`engine::plan`] takes an estimate **per
//! concept**, keyed by that concept's identity, and
//! [`CriticalPathError::NoEstimateForConcept`] refuses a satisfying set that
//! reaches a concept with no estimate rather than borrowing a neighbour's.
//!
//! ## What this task does not decide
//!
//! * **What a concept costs.** Section 16.2's four estimation input families
//!   arrive as [`vector::CostBasis`] on an estimate the caller supplies. This
//!   crate compares intervals; it measures none.
//! * **Whether an offering runs.** `P2-U1` owns section 8.3's four standings and
//!   this crate reads one.
//! * **Whether the registrar's prerequisites are met.** §28's
//!   `OFFICIAL_PREREQUISITE` engine is `PLANNED`, so
//!   [`constraint::OfficialPrerequisiteStanding::Unknown`] is a value and never
//!   a pass.
//! * **A semester.** `P2-N8` owns `PlanScenario` and the deterministic/projected
//!   split.
//! * **Persistence.** Nothing here is written. There is no migration and no edge
//!   to `academic-store`. It opens no file, opens no socket and reads no clock.
//! * **`§38`.** `P2-N6` opens and closes no gate.

pub mod checkpoint;
pub mod constraint;
pub mod counterfactual;
pub mod disclosure;
pub mod edit;
pub mod engine;
pub mod hypergraph;
pub mod option;
pub mod pareto;
pub mod plan;
pub mod preference;
pub mod proof;
pub mod vector;

pub use checkpoint::{
    CheckpointDecision, UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE, uncertain_edge_ratio_permille,
};
pub use constraint::{
    CONSTRAINTS, Constraint, ConstraintFinding, ConstraintInputs, ConstraintVerdict,
    OfficialPrerequisiteStanding, RequiredInsertion, is_stale,
};
pub use counterfactual::{EdgeOutcome, EdgeSensitivity, sensitivity, sensitivity_of, without};
pub use disclosure::{
    AlternativeRoute, Alternatives, ComputationSnapshot, CostAssumption, CostAssumptions,
    DISCLOSURE_GROUPS, Disclosure, DisclosureGroup, ExcludedRoute, ExclusionReason, Exclusions,
    UncertainEdge, UncertainEdges, is_disclosed_standing,
};
pub use edit::{EditedPlan, RelationEdit, edited};
pub use engine::{ConceptEstimate, PlanRequest, plan};
pub use hypergraph::{
    EdgeMember, EdgeStanding, Hyperedge, MAX_SATISFYING_SETS, PrerequisiteHypergraph,
    SatisfyingSet, satisfying_sets, shortest_by_node_count,
};
pub use option::{AcquisitionOption, Opportunity, OpportunityKind};
pub use pareto::{Dominance, Dominated, ParetoFront, dominance};
pub use plan::{Candidate, CriticalPathResult, PATH_ROLES, PathRole, PlanStep, RankedPath};
pub use preference::{
    NAMED_STRATEGIES, NamedStrategy, PreferenceSlider, Ranking, STRATEGY_NAMES_ARE_EXAMPLES, rank,
};
pub use proof::{CRITICAL_PATH_CORPUS_ROOT, STAGE_RULES, frozen_inputs, outcome};
pub use vector::{
    BASIS_FAMILIES, BENEFIT_COMPONENTS, BasisFamily, BenefitComponent, BenefitVector,
    COST_COMPONENTS, CostBasis, CostComponent, CostEstimate, CostVector, Unit, VectorAxis,
    all_axes,
};

/// Why a critical path operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CriticalPathError {
    /// A basis called measured named none of section 16.2's four families.
    #[error("a measured cost basis names at least one input family")]
    MeasuredBasisNamesNoFamily,
    /// An interval whose upper end is below its lower end.
    #[error("a cost interval's high end is below its low end")]
    InvertedEstimate,
    /// Section 16.2's `근거가 없으면 범위로 표시한다`, refused at the
    /// constructor.
    #[error("an unmeasured cost estimate is a range, not a point")]
    UnmeasuredEstimateIsAPoint,
    /// Two intervals in different units were added.
    #[error("two cost intervals in different units cannot be added")]
    UnitMismatch,
    /// An estimate's unit is not the unit its axis is measured in.
    #[error("the estimate offered for {axis} is not in that axis's unit")]
    AxisUnitMismatch {
        /// The axis's own section 16.2 token.
        axis: &'static str,
    },
    /// A vector was assembled from the wrong number of axes.
    #[error("a vector was assembled from the wrong number of axes")]
    AxisCountChanged,
    /// A hyperedge with no members.
    #[error("a hyperedge requires at least one member")]
    EmptyHyperedge,
    /// A `REQUIRES ONE OF` with fewer than two branches.
    #[error("a disjunction offers at least two branches")]
    DisjunctionHasOneBranch,
    /// A member whose admitted edge is stated about another concept.
    #[error("a hyperedge member is stated about another concept")]
    HyperedgeMemberLeavesTarget,
    /// The branch product exceeded [`MAX_SATISFYING_SETS`].
    #[error("the hypergraph offers more satisfying sets than this engine will assemble")]
    HypergraphIsTooWide,
    /// An option that supplies no occasion at all.
    #[error("an acquisition option supplies at least one opportunity")]
    OptionSuppliesNoOpportunity,
    /// Section 16.2's `여러 exposure/practice 기회를 묶은`, refused.
    #[error("a course bundles at least one exposure and one practice opportunity")]
    CourseIsNotABundle,
    /// The eight-answer array stopped being eight.
    #[error("the constraint answers are not one per section 16.3 bullet")]
    ConstraintCountChanged,
    /// A slider that is not a complete permutation of every axis.
    #[error("a preference slider orders every axis exactly once")]
    SliderIsNotAPermutation,
    /// A plan step naming a concept its satisfying set does not hold.
    #[error("a plan step names a concept outside its satisfying set")]
    CandidateStepLeavesTheSet,
    /// A satisfying set concept with no plan step.
    #[error("a satisfying set concept has no plan step")]
    CandidateStepMissing,
    /// A ranked path that is not one of the front's survivors.
    #[error("a ranked path is not on the Pareto front")]
    RankedPathIsNotOnTheFront,
    /// A satisfying set that reached a concept with no estimate.
    #[error("no cost estimate was supplied for concept {0}")]
    NoEstimateForConcept(academic_domain::EntityId),
    /// An estimate offering no way of acquiring its concept.
    #[error("concept {0} has no acquisition option")]
    ConceptHasNoAcquisitionOption(academic_domain::EntityId),
    /// A satisfying set with no concepts at all.
    #[error("a satisfying set holds at least one concept")]
    EmptySatisfyingSet,
    /// A disclosure group that must carry entries was empty.
    #[error("disclosure group {group} cannot be empty")]
    DisclosureGroupIsEmpty {
        /// Which group, in section 16.5's own words.
        group: &'static str,
    },
    /// A recomputation for a different goal from the base.
    #[error("an edit recomputes the same goal")]
    EditChangesTheGoal,
    /// `P2-N5` refused a value.
    #[error(transparent)]
    Gap(#[from] academic_gap::GapError),
    /// `P2-C5` refused a value.
    #[error(transparent)]
    Engine(academic_domain::engines::EngineError),
    /// `P2-C1` refused a value.
    #[error(transparent)]
    Domain(academic_domain::DomainError),
}
