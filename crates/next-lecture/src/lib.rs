//! `P2-L6`: section 12.7's next-lecture preparation — seven places a concept is
//! expected from, the candidate standing every one of them keeps, the minimum
//! that is proposed and the breadth that is not, and three uncertainties that
//! are never one.
//!
//! `P2-L4` made a lecture a document nothing was deleted from. `P2-N5` made a
//! gap an evidence-backed prerequisite deficit blocking an active goal. This
//! crate answers the question section 12.7 opens with, and it is mostly a
//! question about restraint:
//!
//! > syllabus, 다음 title/slide, 교재 chapter, LMS 자료, 과제, 공지, 직전 강의
//! > 말미에서 `ExpectedConceptClaim`을 추출한다. 이것을 Knowledge State와
//! > prerequisite graph에 비교해 **이해를 막을 가능성이 큰 최소 기초**만
//! > 제안한다.
//!
//! ## What holds section 12.7, and where
//!
//! | Section 12.7 rule | What holds it |
//! |---|---|
//! | seven places, and no eighth | [`EXPECTED_CONCEPT_SOURCES`], compared with the sentence in both directions and with the sentence's leftovers required to be separators |
//! | an extraction is a candidate | [`ExpectedConceptClaim::STANDING`] is `AI_INFERRED` and `extract` takes no status; nothing here returns a `academic_knowledge_state` type |
//! | the material is outside text | `extract` takes a `P2-G5` [`Proposal`](academic_untrusted_content::Proposal), the only value `adjudicate` produces, and no product file here can read one ingested byte |
//! | the seventh place is a lecture this system kept | [`MaterialReference::of`] requires a `P2-L4` `NodeId` for it and refuses one for the other six |
//! | `최소 기초만` | [`minimality_defects`] is three graph facts and this crate holds no phrase list |
//! | `1–3개` | [`PreparationBrief::assemble`] takes the whole list and there is no `push` |
//! | three uncertainties, separated | three axis types with nothing in common, so there is no array to fold |
//!
//! `crates/next-lecture/tests/compile_fail/` holds the compiled half.
//!
//! ## Neither count is a number in this crate
//!
//! `expected_concept_source_matrix` reads section 12.7's first sentence and
//! `prep_uncertainty_factorization` reads its last, each back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and each compared
//! against [`EXPECTED_CONCEPT_SOURCES`] and [`PREP_AXES`] in both directions.
//! Seven and three are measurements of the design document.
//!
//! **`다음 title/slide` is one place and not two**, and the reading is executed:
//! after the seven cells are removed from the sentence, what is left has to be
//! separators, so a document punctuated for eight fails here.
//! [the next-lecture contract](../../docs/contracts/next-lecture-preparation.md)
//! records it.
//!
//! ## What this task does not decide
//!
//! * **Which of several foundations comes first.** `P2-N6` owns the AND/OR
//!   hypergraph and the choice between routes. [`propose`] refuses a case with
//!   more strong deficits than the morning has room for rather than ranking
//!   them, which is section 25.2's own `자동 중요도 순으로 숨기지 않고`.
//! * **Whether the person actually knows the foundation.** Section 27.2 says AI
//!   does not `개념 이해·질문 해결을 사용자 대신 확정`. Nothing here writes a
//!   knowledge state, and nothing here returns a type `P2-N2` reads.
//! * **Persistence.** Nothing here is written. There is no migration and no edge
//!   to `academic-store`. It opens no file, opens no socket and reads no clock.
//! * **`§38`.** `P2-L6` opens and closes no gate.
#![deny(missing_docs)]

pub mod brief;
pub mod claim;
pub mod engine;
pub mod minimality;
pub mod source;
pub mod uncertainty;

pub use brief::{
    CandidateParts, HIGHEST_PREPARATION, LOWEST_PREPARATION, PreparationBrief, PreparationCandidate,
};
pub use claim::ExpectedConceptClaim;
pub use engine::propose;
pub use minimality::{MinimalityDefect, minimality_defects};
pub use source::{EXPECTED_CONCEPT_SOURCES, ExpectedConceptSource, MaterialReference};
pub use uncertainty::{
    ExpectedConceptReading, PREP_AXES, PrepAxis, PrepUncertainty, PrerequisiteEdgeReading,
    UserStateReading,
};

/// Why a next-lecture preparation operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NextLectureError {
    /// A claim named a tier that carries no prerequisite of its own.
    #[error("a {kind:?} carries no independent prerequisite of its own")]
    ExpectedConceptCarriesNoPrerequisite {
        /// The offered tier.
        kind: academic_domain::entity_registry::EntityKind,
    },
    /// No citation pointed into the material the claim names.
    #[error("no cited span points into the {place:?} the claim names")]
    ClaimDoesNotQuoteItsMaterial {
        /// Which of section 12.7's seven places was claimed.
        place: ExpectedConceptSource,
    },
    /// The prior lecture's ending cited no `P2-L4` node.
    #[error("the prior lecture's ending must cite the document node it ended at")]
    PriorLectureEndingNeedsItsDocumentNode,
    /// A place that is not the prior lecture's ending cited a `P2-L4` node.
    #[error("{place:?} did not come from a lecture this system recorded")]
    OnlyThePriorLectureEndingCitesADocumentNode {
        /// Which of section 12.7's seven places cited one.
        place: ExpectedConceptSource,
    },
    /// The edge and the overlay are about two different concepts.
    #[error("the edge runs into {edge} and the state is about {state}")]
    AxesDescribeDifferentConcepts {
        /// The concept the edge runs into.
        edge: academic_domain::EntityId,
        /// The concept the overlay is about.
        state: academic_domain::EntityId,
    },
    /// The supplied overlay was not about the candidate's own concept.
    #[error("the state for candidate {candidate} is about {state}")]
    CandidateStateIsAboutAnotherConcept {
        /// The candidate concept.
        candidate: academic_domain::EntityId,
        /// The concept the overlay was about.
        state: academic_domain::EntityId,
    },
    /// The graph held no blocking edge for the path's last hop.
    #[error("the graph holds no blocking edge for the candidate's deepest step")]
    NoEdgeForTheDeepestStep,
    /// The proposal was broader than section 12.7's minimum.
    #[error("the preparation is not minimal: {0:?}")]
    NotMinimal(Vec<MinimalityDefect>),
    /// The descent did not start at the claim's concept.
    #[error("the gap case is about {case} and the claim is about {claim}")]
    CaseIsAboutAnotherConcept {
        /// The concept the descent started from.
        case: academic_domain::EntityId,
        /// The concept the claim named.
        claim: academic_domain::EntityId,
    },
    /// No overlay was supplied for a root the case holds.
    #[error("no state was supplied for concept {0}")]
    NoStateForConcept(academic_domain::EntityId),
    /// More strong deficits than the morning has room for. Section 25.2 says
    /// not to hide them by automatic importance order, so none is chosen.
    #[error("{count} blocking foundations were found and the morning holds at most 3")]
    TooManyBlockingFoundations {
        /// How many were found.
        count: usize,
    },
    /// A brief was assembled outside the documents' `1–3`.
    #[error("a preparation brief holds {count} candidates, and 1 to 3 are allowed")]
    PreparationCountOutOfBounds {
        /// How many were offered.
        count: usize,
    },
    /// Two candidates named one concept.
    #[error("concept {concept} is offered twice in one brief")]
    CandidateRepeatsAnother {
        /// The repeated concept.
        concept: academic_domain::EntityId,
    },
    /// `P2-N5` refused a value.
    #[error(transparent)]
    Gap(#[from] academic_gap::GapError),
    /// `P2-C1` refused a value.
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
}
