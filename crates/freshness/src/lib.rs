//! `P2-N3`: section 13.3's freshness — the six bands, their seven inputs, the
//! bounded spillover, the versioned prior, and the decay that touches this axis
//! and no other.
//!
//! `P2-N2` answered *what does the evidence support saying about this concept*.
//! This crate answers the other question section 13.3 separates out: **could the
//! person retrieve it right now** — and, just as carefully, what a `no` to that
//! does *not* mean.
//!
//! ## Time does not take anything away
//!
//! Section 1's fifth invariant is `Mastery와 Freshness를 합치지 않는다. 오래
//! 되었다는 이유로 실력을 자동 강등하지 않는다`, and section 13.3 says it again
//! as `시간 decay는 freshness projection에만 적용한다. mastery를 자동 내리지
//! 않는다`.
//!
//! That is held here by a **missing vocabulary**, not by a rule:
//!
//! | Section 13.3 rule | What holds it |
//! |---|---|
//! | decay cannot reach a mastery | this crate has no name for a `MasteryLevel`; [`decay::decay`] takes a span and a window |
//! | freshness reaches `P2-N2` through one value | [`projection::FreshnessProjection::input`] returns a `FreshnessInput`, which has two fields and no third |
//! | ineligible evidence cannot freshen a concept | [`evidence::DatedEvidence`] wraps an `EligibleEvidence` and there is no other constructor |
//! | another concept's evidence cannot freshen this one | [`projection::project`] refuses it, and so does [`spillover::NeighborUse::direct`] one hop out |
//! | spillover is one hop | [`spillover::NeighborUse`] is built from dated evidence and from no band, projection or contribution |
//! | spillover weighs less than direct use | [`spillover::Spillover::band`] is a band strictly below the neighbour's own |
//! | an AI cannot say the user still remembers | [`recall::RecallStatement`]'s one constructor runs ADR-003's actor matrix |
//! | the shipped prior cannot pass for a measured one | [`persistence::PriorBasis::NoEvidenceBasisEstablished`] is a value, and [`persistence::PersonalizationSpeed`] has no `Default` |
//!
//! `crates/freshness/tests/compile_fail/` holds the compiled half.
//!
//! ## Neither count is a number in this crate
//!
//! `freshness_bands_are_exactly_six` reads section 13.3's own sentence and its
//! own bullet list back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares them
//! against [`band::BANDS`] and [`band::FreshnessSignal::ALL`] in both
//! directions. Six and seven are measurements of the design document.
//!
//! ## What this task does not decide
//!
//! * **`GATE-38-024`** stays open. The evidence basis for the priors and the
//!   speed of personalization are configuration decisions and this task makes
//!   neither: [`persistence::UNCALIBRATED_PRIOR_V1`] carries
//!   `NO_EVIDENCE_BASIS_ESTABLISHED` and says `UNCALIBRATED` in its own name,
//!   and [`persistence::PersonalizationSpeed`] has no default value anywhere, so
//!   nothing personalizes until somebody chooses a minimum sample count and a
//!   step.
//! * **Mastery.** `P2-N2` owns the ladder, the facets, the ceilings and the
//!   eligibility gate. This crate reads its `EligibleEvidence` and hands back a
//!   `FreshnessInput`.
//! * **Persistence.** Nothing here is written. There is no migration and no edge
//!   to `academic-store`. It opens no file, opens no socket and reads no clock:
//!   `as_of` is an argument.
//! * **Gaps and paths.** `P2-N5` and `P2-N6`.

pub mod band;
pub mod decay;
pub mod disclosure;
pub mod evidence;
pub mod persistence;
pub mod projection;
pub mod recall;
pub mod spillover;

pub use band::{BANDS, FreshnessSignal, band_token, ceiling_of, floor_of, rank, step_down};
pub use decay::decay;
pub use disclosure::{
    JUDGEMENT_TOKENS, NO_RECENT_USE, STALE_ACTION, STALE_DISCLOSURE, STALE_MEANING, StaleDisclosure,
};
pub use evidence::{DatedEvidence, Repetition};
pub use persistence::{
    CALIBRATED_PRIOR_NAME, Calibration, DAY_MILLIS, PersistenceClass, PersistenceWindow,
    PersonalizationSpeed, PriorBasis, PriorIdentity, PriorName, RetentionPrior,
    UNCALIBRATED_PRIOR_NAME, UNCALIBRATED_PRIOR_V1, elapsed_millis, persistence_class,
};
pub use projection::{
    ConfidenceGap, FRESHNESS_GAP_PERMILLE, FreshnessInputs, FreshnessProjection, FreshnessTrace,
    TraceEntry, project,
};
pub use recall::{
    ContraryEvent, ContraryKind, RECALL_STATEMENT_PREDICATE, RecallCheck, RecallDirection,
    RecallStatement, UserRecall,
};
pub use spillover::{
    CitedEdge, NeighborUse, SPILLOVER_CEILING, SPILLOVER_EDGES, SPILLOVER_SOURCE_FLOOR, Spillover,
};

/// Why a freshness operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FreshnessError {
    /// An input was about some other concept.
    #[error("input is about another concept")]
    EvidenceNamesAnotherConcept,
    /// A spillover contribution was computed toward another concept.
    #[error("spillover was computed toward another concept")]
    SpilloverNamesAnotherConcept,
    /// An input is dated after the instant being projected for.
    #[error("input is dated after the instant being projected for")]
    InputAfterAsOf,
    /// A claim did not carry the exact predicate, object or status a recall
    /// statement needs.
    #[error("claim is not a recall statement")]
    NotARecallStatement,
    /// A recall statement claim named a different concept.
    #[error("recall statement names the wrong concept")]
    RecallSubjectMismatch,
    /// The cited evidence item was not attached to the recall claim.
    #[error("recall statement does not cite its evidence")]
    RecallEvidenceMissing,
    /// A recall claim named a band the statement does not mean.
    #[error("recall statement names a different band")]
    RecallBandMismatch,
    /// ADR-003's actor/authority/status matrix rejected a claim.
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
}
