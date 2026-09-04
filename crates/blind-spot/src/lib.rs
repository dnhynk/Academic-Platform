//! `P2-N7`: section 23's blind spot detector — the five states, the coverage
//! that never becomes a mastery, the scope the user selects, the four
//! dispositions that outlive a rerun, and the copy that does not say anything
//! about the person.
//!
//! `P2-N2` answered *what does the evidence support saying about this concept*
//! and established that `UNSEEN` is not a failed test. `P2-N1` fixed what a
//! field, a concept and an operation are. This crate answers the question
//! section 23 opens with, and the discipline is the same one:
//!
//! > Blind Spot은 선택한 CS taxonomy와 시간 window에서 **판단할 exposure 자체가
//! > 거의 없는 영역**이다.
//!
//! Not *what the user cannot do*. What the record cannot be read for.
//!
//! ## What holds section 23, and where
//!
//! | Section 23 rule | What holds it |
//! |---|---|
//! | the five states mean five different things | [`state::StateBasis`] has one payload per state, each with its own refusals, and [`state::state_of`] is a bijection onto [`state::BLIND_SPOT_STATES`] |
//! | `UNOBSERVED` says ability cannot be inferred | [`presentation::headline`] answers section 23's own replacement phrase, and no string this crate emits is the claim it replaces |
//! | coverage never becomes a mastery score | the crate has no name for a mastery level, and [`coverage::FieldCoverage`] derives neither `PartialOrd` nor `Ord` |
//! | granularity and window are the user's | [`scope::BlindSpotScope`] has no `Default`, no shipped constant, and four arguments by value |
//! | a disposition is the user's and it lasts | [`disposition::UserDispositionChoice::verify`] runs ADR-003's actor matrix, and [`disposition::DispositionLedger`] has no removal method |
//! | `NOT_RELEVANT` survives an AI rerun | [`detector::detect`] is this crate's only producer of a finding and reads the ledger first |
//! | no goal to equalise coverage is generated | there is no `academic-gap` edge, so a goal this engine emitted would first have to be a goal it could name |
//! | low relevance uses neutral copy and demands nothing | every string a [`presentation::NeutralPresentation`] renders is the design document's own, and the value has no field an action could occupy |
//! | `EXPLORE` opens one bounded taste path | [`taste::TastePath`] holds one [`taste::TasteStep`] and not a list |
//!
//! `crates/blind-spot/tests/compile_fail/` holds the compiled half.
//!
//! ## None of the four counts is a number in this crate
//!
//! `five_states_are_semantically_distinct` reads section 23's own `text` block,
//! `coverage_never_becomes_mastery` reads its coverage sentence,
//! `four_dispositions_are_durable` reads its disposition bullet, and
//! `explore_creates_one_bounded_taste_path` reads its taste-path bullet — each
//! back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and each
//! compared against [`state::BLIND_SPOT_STATES`], [`coverage::EXPOSURE_SOURCES`],
//! [`disposition::DISPOSITIONS`] and [`taste::TASTE_STEPS`] in both directions.
//! The schema block's eight keys are measured against [`finding::FINDING_FIELDS`]
//! the same way.
//!
//! Section 23's schema example writes a `userDisposition` its own UX bullet does
//! not list. [`disposition::SCHEMA_EXAMPLE_DISPOSITION`] keeps that spelling so
//! the discrepancy is a measured value with a test on it;
//! `docs/contracts/blind-spot-detector.md` records it.
//!
//! ## What this task does not decide
//!
//! * **Gaps.** `P2-N5` owns whether an active goal is actually blocked. This
//!   crate carries its finding and computes nothing about goals.
//! * **Freshness.** `P2-N3` owns the bands. This crate carries one and computes
//!   none; it names [`state::LOW_RECENCY_BANDS`] and no threshold.
//! * **Mastery.** `P2-N2` owns the ladder, and this crate cannot name it.
//! * **Persistence.** Nothing here is written. There is no migration and no edge
//!   to `academic-store`. It opens no file, opens no socket and reads no clock.
//! * **`§38`.** `P2-N7` opens and closes no gate.

pub mod coverage;
pub mod detector;
pub mod disposition;
pub mod explanation;
pub mod finding;
pub mod presentation;
pub mod reading;
pub mod relevance;
pub mod resolution;
pub mod scope;
pub mod state;
pub mod taste;

pub use coverage::{
    EXPOSURE_SOURCES, EvidenceDiversity, ExposureItem, ExposureSource, FieldCoverage,
};
pub use detector::detect;
pub use disposition::{
    DISPOSITION_PREDICATE, DISPOSITIONS, DispositionLedger, SCHEMA_EXAMPLE_DISPOSITION,
    UserDisposition, UserDispositionChoice,
};
pub use explanation::{ExposureDriver, SkewExplanation};
pub use finding::{BlindSpotFinding, BlindSpotFindingWire, FINDING_FIELDS};
pub use presentation::{
    CANNOT_INFER_ABILITY, CLAIM_ABOUT_THE_PERSON, EMPHASIS, FindingPresentation,
    NOT_A_JUDGEMENT_OF_ABILITY, NeutralPresentation, headline, renderable_copy,
};
pub use reading::KeyReading;
pub use relevance::GoalRelevance;
pub use resolution::FieldResolver;
pub use scope::{BlindSpotScope, GRANULARITIES, ObservationWindow, TaxonomyGranularity};
pub use state::{
    BLIND_SPOT_STATES, BelowMinimum, BlindSpotState, EXPOSURE_CLASSES, ExposureClass, GoalBlock,
    LOW_RECENCY_BANDS, LowRecency, ObservedDifficulty, ScopeExclusion, StateBasis, state_of,
};
pub use taste::{TASTE_STEPS, TastePath, TasteStep};

/// Why a blind-spot operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BlindSpotError {
    /// A boundary below this one refused the value.
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
    /// A `BelowMinimum` was offered for a count that is not below the minimum.
    #[error("{observed} admitted items is not below the minimum of {minimum}")]
    CoverageIsNotBelowMinimum {
        /// The observed count.
        observed: u32,
        /// The user's minimum.
        minimum: u32,
    },
    /// An `ObservedDifficulty` was offered with no failing attempt.
    #[error("observed difficulty needs at least one failed attempt")]
    DifficultyHasNoAttempt,
    /// A band outside [`LOW_RECENCY_BANDS`] was offered as low recency.
    #[error("{0:?} is not a band section 23's 최근성 낮음 may be read off")]
    BandIsNotLowRecency(academic_domain::FreshnessBand),
    /// A scope exclusion was built from a disposition that is not
    /// `NOT_RELEVANT`.
    #[error("a scope exclusion needs the user's NOT_RELEVANT choice")]
    ExclusionNeedsNotRelevant,
    /// A goal was offered as its own blocking concept.
    #[error("a goal cannot be its own blocking concept")]
    GoalBlocksItself,
    /// The claim offered as a disposition choice is not one.
    #[error("the claim is not a user disposition choice")]
    NotADispositionChoice,
    /// A disposition claim named another field.
    #[error("the disposition claim names another field")]
    DispositionSubjectMismatch,
    /// A disposition claim did not cite the evidence offered.
    #[error("the disposition claim does not cite the offered evidence")]
    DispositionEvidenceMissing,
    /// A `HIDE_UNTIL` was offered with no deadline.
    #[error("HIDE_UNTIL needs a deadline")]
    DeadlineRequired,
    /// A deadline was offered for a disposition that takes none.
    #[error("{0:?} takes no deadline")]
    DeadlineNotAllowed(UserDisposition),
    /// A `HIDE_UNTIL` deadline does not outlast the instant it was chosen at.
    #[error("a HIDE_UNTIL deadline must be after the instant it was chosen at")]
    DeadlineIsNotInTheFuture,
    /// A replayed choice is not newer than the one it would replace.
    #[error("a disposition cannot be replaced by one that is not newer")]
    DispositionIsOlderThanTheOneItReplaces,
    /// The ledger's standing choice for a key names another field.
    #[error("the standing disposition names another field")]
    DispositionIsAboutAnotherField,
    /// The user selected a minimum exposure of zero.
    #[error("a minimum exposure of zero is a scope under which nothing is unobserved")]
    MinimumExposureIsZero,
    /// A bounded window with no instants in it.
    #[error("an observation window must end after it starts")]
    WindowIsEmpty,
    /// An offered item is about a different aggregation key.
    #[error("an item about {found:?} was offered for {expected:?}")]
    ItemIsAboutAnotherKey {
        /// The key being counted.
        expected: academic_domain::EntityId,
        /// The key the item resolved to.
        found: academic_domain::EntityId,
    },
    /// An offered item is about an entity the selected taxonomy does not hold.
    #[error("evidence {0:?} is about an entity this taxonomy version does not hold")]
    ItemIsOutsideTheTaxonomy(academic_domain::EvidenceId),
    /// A taste path was requested without the user's `EXPLORE` choice.
    #[error("a taste path needs the user's EXPLORE choice")]
    TastePathNeedsExplore,
    /// A taste path was requested for a key the choice does not name.
    #[error("the EXPLORE choice names another field")]
    TastePathIsAboutAnotherField,
    /// The user pressed `EXPLORE` and no taste step was offered.
    #[error("EXPLORE was chosen and no taste step was offered for that key")]
    ExploreWithoutAStep,
}
