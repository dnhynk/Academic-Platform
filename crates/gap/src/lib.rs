//! `P2-N5`: section 15's gap engine — the five kinds, the four-dimension
//! overlay, the descent to the first strong deficit, the candidates it refuses
//! to choose between, and the eight-field explanation contract.
//!
//! `P2-N2` answered *what does the evidence support saying about this concept*.
//! `P2-N3` answered *could the person retrieve it right now*. `P2-N4` fixed what
//! a question is. This crate answers the question section 15.1 opens with, and
//! it is mostly a question about restraint:
//!
//! > Gap은 낮은 Knowledge State 자체가 아니라 **활성 목표의 성공을 가로막는,
//! > 근거가 있는 prerequisite 부족**이다.
//!
//! ## What holds section 15, and where
//!
//! | Section 15 rule | What holds it |
//! |---|---|
//! | a low state with no goal is not a gap | no function here turns a state into a [`case::GapCase`]; [`engine::search`]'s first argument is an [`goal::ActiveGoal`] |
//! | success criteria come before expansion | [`goal::GoalCriteria::of`] returns `None` for an empty list and [`goal::ActiveGoal::declare`] takes one by value |
//! | traversal is `REQUIRES` and strong `BUILDS_ON` | [`graph::PrerequisiteEdge::admit`] calls `P2-C4`'s `prerequisite_descriptor`; this crate holds no allowlist |
//! | a weak `BUILDS_ON` never blocks | [`graph::blocking_floor`] answers `None` for `HELPFUL`, so [`graph::PrerequisiteEdge::blocks`] is false and the descent has nothing to cross |
//! | four dimensions are overlaid onto **one** concept | [`state::ConceptState::overlay`] refuses evidence, a projection or a contribution naming another |
//! | equal candidates are both retained | [`case::roots_of`] returns every tied root and [`case::GapCase::of`] refuses a tie with no diagnostic |
//! | broad advice is not an explanation | [`explanation::GapExplanation::of`] refuses one, on structure and never on words |
//!
//! `crates/gap/tests/compile_fail/` holds the compiled half.
//!
//! ## None of the three counts is a number in this crate
//!
//! `five_gap_types_route_correctly` reads section 15.2's table,
//! `four_state_dimensions_are_overlaid` reads step 3's sentence, and
//! `eight_field_explanation_is_complete` reads section 15.3's sentence — each
//! back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and each
//! compared against [`kind::GAP_KINDS`], [`kind::STATE_DIMENSIONS`] and
//! [`explanation::EXPLANATION_FIELDS`] in both directions. Five, four and eight
//! are measurements of the design document.
//!
//! Section 15.2's **step 6 names four** informal kinds where its table has five
//! rows. The table is normative and [`kind::STEP_SIX_INFORMAL_NAMES`] keeps the
//! four so the discrepancy is measured rather than rediscovered.
//! `docs/contracts/gap-engine.md` records it.
//!
//! ## One concept's evidence never becomes another's
//!
//! `P2-N2` closed that at the history and `P2-N3` closed the one-hop form,
//! reporting that the surviving route is one concept's evidence crossing a real
//! edge. This engine descends exactly those edges, so [`state`]'s module
//! documentation names three routes it found, and
//! [`GapError::FreshnessRestsOnPathSpillover`] is the third: a band raised by a
//! neighbour that lies on the blocking path the engine is building.
//!
//! ## What this task does not decide
//!
//! * **Paths and costs.** `P2-N6` owns the AND/OR hypergraph, the cost and
//!   benefit vectors and the choice between routes. This crate computes no
//!   alternative route and ranks nothing by preference.
//! * **Blind spots.** `P2-N7`. Nothing here reports a concept the goal does not
//!   reach.
//! * **Persistence.** Nothing here is written. There is no migration and no edge
//!   to `academic-store`. It opens no file, opens no socket and reads no clock.
//! * **`§38`.** `P2-N5` opens and closes no gate.

pub mod case;
pub mod engine;
pub mod explanation;
pub mod goal;
pub mod graph;
pub mod kind;
pub mod node;
pub mod path;
pub mod routing;
pub mod state;

pub use case::{GapCase, GapCaseWire, RootCandidate, TieDiagnostic, roots_of};
pub use engine::{ConceptReading, DiagnosticOffer, expand, search};
pub use explanation::{
    AlternativePath, EXPLANATION_FIELDS, ExplanationParts, GapExplanation, LinkedContext,
    MinimumRemediation, NoAlternativeReason, RemediationActivity, SpecificityDefect,
};
pub use goal::{ActiveGoal, GoalCriteria, SuccessCriterion};
pub use graph::{
    PrerequisiteEdge, PrerequisiteGraph, blocking_floor, predicate_of_token, strength_of_token,
    strength_token,
};
pub use kind::{GAP_KINDS, GapKind, STATE_DIMENSIONS, STEP_SIX_INFORMAL_NAMES, StateDimension};
pub use node::{IdentityStanding, gap_bearing};
pub use path::{AncestorImpact, BlockingPath, PathStep};
pub use routing::{BranchStanding, RETRIEVAL_FLOOR, route};
pub use state::{ConceptState, OfferedEvidence, SpilloverSource, StateSnapshot};

/// Why a gap operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GapError {
    /// A goal was offered with no success criterion. Section 15.2 step 1.
    #[error("an active goal needs at least one success criterion")]
    GoalHasNoSuccessCriteria,
    /// A success criterion named a tier that carries no prerequisite of its own.
    #[error("a {kind:?} carries no independent prerequisite of its own")]
    CriterionSubjectCarriesNoPrerequisite {
        /// The offered tier.
        kind: academic_domain::entity_registry::EntityKind,
    },
    /// A competency criterion carried no observable-performance sentence.
    #[error("a competency criterion needs an observable-performance statement")]
    CompetencyPerformanceMissing,
    /// A goal's surface concept was a tier that carries no prerequisite.
    #[error("a {kind:?} cannot be a goal's surface concept")]
    SurfaceConceptCarriesNoPrerequisite {
        /// The offered tier.
        kind: academic_domain::entity_registry::EntityKind,
    },
    /// A predicate `P2-C4`'s registry does not mark traversable.
    #[error("{0} is not a predicate a path engine may traverse")]
    NotATraversablePredicate(&'static str),
    /// A strength the registry does not let that predicate carry.
    #[error("{predicate} does not admit strength {strength:?}")]
    StrengthNotAdmitted {
        /// The predicate.
        predicate: &'static str,
        /// The offered strength.
        strength: academic_domain::predicates::PrerequisiteStrength,
    },
    /// An edge joining a concept to itself.
    #[error("{0} cannot join a concept to itself")]
    SelfEdge(&'static str),
    /// An edge carrying no evidence. Section 7.3.
    #[error("{0} must cite at least one evidence item")]
    UncitedEdge(&'static str),
    /// A strength token outside the registry's three.
    #[error("unknown prerequisite strength token: {0}")]
    UnknownStrengthToken(String),
    /// A predicate name outside the registry's twenty.
    #[error("unknown predicate name: {0}")]
    UnknownPredicateToken(String),
    /// Offered evidence resolved to a different concept.
    #[error("evidence is linked to another concept")]
    EvidenceNamesAnotherConcept,
    /// A freshness projection was about a different concept.
    #[error("freshness projection is about another concept")]
    FreshnessNamesAnotherConcept,
    /// A spillover contribution was computed toward a different concept.
    #[error("spillover was computed toward another concept")]
    SpilloverNamesAnotherConcept,
    /// The declared contributions do not match the projection's own trace.
    #[error("the projection's trace does not match the declared spillover")]
    SpilloverNotDeclared,
    /// A node's band was raised by a neighbour on its own blocking path.
    #[error(
        "concept {concept}'s freshness rests on {neighbor} across {predicate}, \
         which is on its own blocking path"
    )]
    FreshnessRestsOnPathSpillover {
        /// The node whose band is contaminated.
        concept: academic_domain::EntityId,
        /// The neighbour the band came from.
        neighbor: academic_domain::EntityId,
        /// The section 7.2 edge the contribution was cited on.
        predicate: &'static str,
    },
    /// The goal's own surface concept has an unsettled identity.
    #[error("surface concept {0} has an unsettled identity")]
    SurfaceIdentityUnsettled(academic_domain::EntityId),
    /// The descent reached a concept with no reading.
    #[error("no reading was supplied for concept {0}")]
    NoReadingForConcept(academic_domain::EntityId),
    /// A path step did not correspond to an admitted blocking edge.
    #[error("a path step does not correspond to an admitted blocking edge")]
    NonBlockingEdgeOnPath,
    /// An explanation failed the specificity contract.
    #[error("the explanation is too broad: {0:?}")]
    NotSpecific(Vec<explanation::SpecificityDefect>),
    /// A candidate's explanation was about another concept or kind.
    #[error("the explanation is about another concept or another kind")]
    CandidateExplainsAnotherConcept,
    /// A candidate carried no `reason` cell.
    #[error("a root candidate needs a reason")]
    CandidateReasonMissing,
    /// A case was assembled with no candidate.
    #[error("a gap case needs at least one candidate")]
    CaseHasNoCandidate,
    /// A candidate's path started somewhere other than the surface concept.
    #[error("a candidate's blocking path does not start at the surface concept")]
    CandidateLeavesSurface,
    /// Several roots tied and no diagnostic was offered. Section 15.2 step 5.
    #[error("several root candidates tie and no diagnostic activity was proposed")]
    TiedRootsNeedADiagnostic,
    /// A diagnostic named a set other than the roots.
    #[error("the diagnostic does not name exactly the tied roots")]
    DiagnosticDoesNotNameTheRoots,
    /// A diagnostic was offered over fewer than two candidates.
    #[error("a diagnostic separates at least two candidates")]
    DiagnosticNeedsTwoCandidates,
    /// A diagnostic's activity was not section 15.2's diagnostic response.
    #[error("a tie diagnostic must be a user confirmation or diagnostic activity")]
    DiagnosticIsNotADiagnostic,
    /// A diagnostic referenced a question that is not open.
    #[error("a tie diagnostic may only reference an open question")]
    DiagnosticQuestionIsNotOpen,
    /// `P2-N2` refused the projection.
    #[error(transparent)]
    KnowledgeState(#[from] academic_knowledge_state::KnowledgeStateError),
    /// `P2-C1` refused a value.
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
}
