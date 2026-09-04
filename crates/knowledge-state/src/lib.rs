//! `P2-N2`: section 13's knowledge state — the ladder, the five facets, the
//! eight evidence ceilings, and the promotions an AI may not make.
//!
//! `P2-N1` fixed what a concept is. `P2-M3` fixed which authority wins when two
//! claims about one subject disagree. `P2-L4` fixed what a lecture document is,
//! and `P2-R4` fixed what a project snapshot observes. This crate is the step
//! that reads all four and answers one question about a person: **what does the
//! evidence support saying about this concept, and what does it not.**
//!
//! ## What holds section 13, and where
//!
//! Every rule below is a value that does not exist rather than a check somebody
//! has to remember to run.
//!
//! | Section 13 rule | What holds it |
//! |---|---|
//! | an automatic projection never reaches `FLUENT` | [`ladder::AutomaticLevel`] has no `Fluent` variant |
//! | `FLUENT` needs repetition **and** user confirmation | [`confirmation::FluentAuthorization`] takes both by value, and is [`projection::MasteryProjection::with_fluency`]'s only argument |
//! | an AI cannot mint a user confirmation | [`confirmation::UserConfirmation`]'s one constructor runs ADR-003's actor matrix |
//! | a grade cannot promote a concept | [`evidence::CourseGradeSignal`] has no concept field and no [`evidence::ConceptEvidence`] variant |
//! | ineligible evidence cannot be projected | [`eligibility::EligibleEvidence`] has one producer, and [`projection::project`] takes only those |
//! | an assertion is never mutated | no `&mut self` method, no setter, no public field; [`assertion::KnowledgeStateAssertion::revise`] returns a new value |
//! | a deserialized `FLUENT` cannot skip the gate | `Deserialize` is `try_from` and refuses `FLUENT` without its record |
//!
//! `crates/knowledge-state/tests/compile_fail/` holds the compiled half.
//!
//! ## `estimateConfidence` is not a skill score
//!
//! Section 13.1 says so in as many words, and the schema field keeps the
//! design's name while the type keeps the meaning:
//! [`projection::EvidenceSufficiency`] is not orderable, converts to and from no
//! mastery level, and always carries what is missing. See its documentation.
//!
//! ## Mastery and freshness are two fields, and this task owns one
//!
//! Section 1's fifth invariant is `Mastery와 Freshness를 합치지 않는다`. The
//! assertion carries `P2-N3`'s band and its confidence in their own fields and
//! this crate computes neither, reads no clock, and applies no decay. There is
//! no time input to any function here, which is what makes *time never demotes
//! mastery* a graph fact rather than a promise.
//!
//! ## It opens nothing and persists nothing
//!
//! No file, no socket, no `academic-store` edge and no migration. Every input
//! arrives as an argument.
//!
//! ## What this task does not decide
//!
//! * **`GATE-38-023`** — whether facets are always visible or progressively
//!   disclosed below the level. That is a user-tested interface decision, and
//!   both modes must stay reachable and accessible. This crate exposes all five
//!   facets through one accessor and selects no disclosure behaviour.
//! * **`GATE-38-025`** — the conditions under which reconfirmation of a
//!   user-confirmed state should be recommended. `P2-M3` left it open and this
//!   task leaves it open: nothing here expires a confirmation, downgrades one,
//!   or schedules a prompt.
//! * **Freshness.** `P2-N3` owns the bands, their inputs, the priors and the
//!   spillover.
//! * **The question graph, gaps and paths.** `P2-N4` through `P2-N6`.

pub mod assertion;
pub mod confirmation;
pub mod conflict;
pub mod eligibility;
pub mod evidence;
pub mod history;
pub mod ladder;
pub mod projection;

pub use assertion::{
    AssertionId, AssertionWire, ConfirmationRecord, FluencyRecord, KnowledgeStateAssertion,
};
pub use confirmation::{
    AdjustmentDirection, AiProposal, FluentAuthorization, STATE_CONFIRMATION_OBJECT,
    STATE_CONFIRMATION_PREDICATE, TransferContext, TransferRepetition, UserConfirmation,
};
pub use conflict::KnowledgeStateConflict;
pub use eligibility::{
    BlockedEvidence, ConceptLink, EligibilityCheck, EligibilityOutcome, EligibilityReasonCode,
    EligibleEvidence, EvidenceDossier, Outcome, Participation, SourceIntegrity,
};
pub use evidence::{
    BroadSignal, CEILINGS, CeilingRow, ConceptEvidence, CourseGradeSignal, DependencyOnly,
    EvidenceCeiling, EvidenceKind, ExerciseOutcome, IncidentRepair, ProjectUse, SelfExplanation,
    TeachingSite,
};
pub use history::{
    EvidenceRetraction, FreshnessInput, HistoryEntry, KnowledgeStateHistory, ProposalApplication,
    ProposalOutcome,
};
pub use ladder::{
    AutomaticLevel, FacetProfile, FacetStrength, LADDER, MasteryFacet, level_token, rung,
};
pub use projection::{
    CeilingDisclosure, EvidenceSufficiency, MasteryProjection, SufficiencyGap, UNSEEN_MEANING,
    UnseenBasis, automatic_contribution, project,
};

/// Why a knowledge-state operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum KnowledgeStateError {
    /// The named node is not in the named `P2-L4` document.
    #[error("document {document} holds no node {node}")]
    TeachingSiteNotInDocument {
        /// The document that was offered.
        document: String,
        /// The node identifier that was not in it.
        node: String,
    },
    /// A confirmation claim did not carry the exact predicate, object or
    /// status.
    #[error("confirmation claim has the wrong predicate, object or status")]
    InvalidConfirmationAction,
    /// A confirmation claim named a different concept.
    #[error("confirmation claim names the wrong concept")]
    ConfirmationSubjectMismatch,
    /// A confirmation claim belonged to another resolution scope.
    #[error("confirmation claim names the wrong scope")]
    ConfirmationScopeMismatch,
    /// The cited evidence item was not attached to the confirmation claim.
    #[error("confirmation does not cite its evidence")]
    ConfirmationEvidenceMissing,
    /// A confirmation named a level the projection does not hold.
    #[error("confirmation names a different mastery level")]
    ConfirmationLevelMismatch,
    /// A `FLUENT` value arrived without the record that authorizes it.
    #[error("FLUENT requires a recorded repetition and user confirmation")]
    FluencyRecordMissing,
    /// A fluency record arrived on a level that is not `FLUENT`.
    #[error("a fluency record belongs only to FLUENT")]
    FluencyRecordNotFluent,
    /// A deserialized assertion's identity did not match its own bytes.
    #[error("assertion identity does not match its contents")]
    AssertionIdentityMismatch,
    /// A retraction named evidence this history never admitted.
    #[error("retraction names evidence this history does not hold")]
    RetractionNamesUnknownEvidence,
    /// An operation needed a standing assertion and there was none.
    #[error("history holds no assertion")]
    HistoryHasNoAssertion,
    /// A proposal was about another concept.
    #[error("proposal names another concept")]
    ProposalNamesAnotherConcept,
    /// ADR-003's actor/authority/status matrix rejected a claim.
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
}
