//! `P2-Y3`: section 24.3's career readiness view, and the four things it
//! refuses to become.
//!
//! `P2-Y1` fixed what a competency is and what evidence may found a rubric
//! cell. `P2-Y2` put competencies into a bundle somebody can name, version and
//! fork without the bundle's name becoming a claim about the labour market.
//! This crate is the step that shows a person what they can and cannot do —
//! **without summarising them as one number.**
//!
//! Section 24.3's first sentence and section 36.9's last one are the whole task:
//!
//! > `percentage 대신 competency × evidence matrix를 기본으로 한다.`
//!
//! > `한 학기의 결과는 "Database 83%"가 아니라 서로 다른 증거와 아직 없는 수행을
//! > 보여주는 구조다.`
//!
//! # The six columns are a measurement, not a number written here
//!
//! Section 24.3's example table and section 36.9's per-competency block are two
//! independent places in the design document, and they name the same six
//! columns in the same order. Both are parsed back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` at run time and
//! compared against [`ReadinessAxis::ALL`] position by position and in both
//! directions, so *six* is a fact about the document rather than a constant this
//! crate asserts.
//!
//! **Section 24.3 states a second six, and it is not this one.** Its prose asks
//! for `사용해봄`, `구조 이해`, `문제 해결`, `장애 debugging`, `설계 선택`,
//! `새 상황 전이` to be distinguished, and those are
//! `academic_competency::EvidenceStage`, which `P2-Y1` owns. The two sets share
//! one spelling — `설계 선택` — and rhyme on two more, which is exactly why they
//! are two types with no conversion in either direction. A stage is how deep a
//! performance went; an axis is which column it is displayed in. See
//! [`crate::cell`] for why no function maps one to the other, and
//! `docs/contracts/career-readiness-matrix.md` for the open reading this
//! records.
//!
//! # What the type system carries, and what it does not
//!
//! | The thing that must not happen | What stops it |
//! |---|---|
//! | one number summarising a person | [`ReadinessView::render`] emits [`view::ViewBlock::Matrix`] first in every view this crate can produce, the block vocabulary is closed, and no `f32` or `f64` is declared anywhere in the crate |
//! | a score without its four disclosures | [`score::disclose`] is the one producer of an [`AuxiliaryScore`], it takes all four by value, and it has **no score parameter** — the number is computed, never supplied |
//! | a disclosure that describes some other matrix | `disclose` re-derives the rubric, the sources and the missing data from the matrix it is publishing over and refuses any that disagrees |
//! | missing, unknown and freshness folded into one axis | three types — [`AxisCell::Missing`], [`AxisCell::Unknown`] and [`FreshnessCell`] — with no conversion between the cell and the band in either direction |
//! | a drill-down that ends in prose | [`navigate::Terminus`] has two arms and no field of either is free text; [`navigate::Termination`] takes its first terminus by value, so an empty walk is not a value |
//! | a hidden score losing what it said | [`ReadinessView::hide_score`] and [`ReadinessView::reset_weights`] take `&self` and return a new view; nothing here takes `&mut self` |
//! | a matrix travelling without its notice | [`NonGuaranteeNotice`] has one producer and no argument, every serialized view carries it, and [`view::published_notice`] refuses a document that does not |
//!
//! # It opens nothing and persists nothing
//!
//! No file, no socket, no clock, no `academic-store` edge and no migration.
//! Every input arrives as an argument, and every function here is pure. A
//! readiness view is a derivation over values three crates below already froze.
//!
//! # What this task does not decide
//!
//! * **The Career Explorer surface.** Section 25.11's graph, comparison view
//!   and acquisition options are a `P2-X`-stage screen. This crate renders an
//!   ordered list of blocks and draws nothing.
//! * **Freshness.** `P2-N3` owns the bands and the decay. There is no time
//!   input to any function here: a band arrives as an argument.
//! * **What evidence is admissible.** `P2-N2` owns eligibility and `P2-Y1` owns
//!   which of section 13.2's rows may found a stage record. This crate takes
//!   `StageEvidence` values that already passed both.
//! * **§38.** This task leaves no gate open and closes none.

pub mod axis;
pub mod cell;
pub mod history;
pub mod identity;
pub mod matrix;
pub mod navigate;
pub mod notice;
pub mod score;
pub mod view;

use serde::{Deserialize, Serialize};

pub use axis::ReadinessAxis;
pub use cell::{
    AxisCell, AxisEvidence, FreshnessCell, MISSING_CELL_MARK, RefusedPlacement, UnknownBasis,
};
pub use history::ReadinessEvent;
pub use identity::{EvidenceLocatorId, MAX_IDENTIFIER, StartingPointId};
pub use matrix::{ColumnReading, CompetencyInput, ReadinessMatrix, ReadinessRow, take};
pub use navigate::{
    AbsenceState, NavigationDirection, StartingPoint, Termination, Terminus, traverse,
};
pub use notice::{
    ALLOWED_INSTEAD, NonGuaranteeNotice, REFUSAL_REASON, REFUSED_PRODUCT, SPECIFICATION_PHRASE,
};
pub use score::{
    AuxiliaryScore, AxisWeight, MissingDataDisclosure, MissingDatum, RubricDisclosure, RubricLines,
    ScoreValue, SourceDisclosure, WeightDisclosure, disclose,
};
pub use view::{NOTICE_KEY, ReadinessView, ViewBlock, published_notice};

/// Why a readiness operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ReadinessError {
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the {0} identifier {1:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(&'static str, String),
    /// A required piece of prose carried nothing.
    #[error("the {0} carries no text")]
    EmptyText(&'static str),
    /// The freshness column carries a band and never a locator or a weight.
    #[error("the freshness column is not one of the evidence columns")]
    FreshnessIsNotAnEvidenceColumn,
    /// A disclosure was not the one this matrix and these competencies produce.
    #[error("the {0} disclosure is not the one {1} produces")]
    DisclosureDoesNotCoverTheMatrix(&'static str, String),
    /// The weighting did not name every evidence column exactly once.
    #[error("a weighting names every evidence column exactly once, and this one does not")]
    WeightingIsNotTotal,
    /// The weighted total does not fit, and a wrapped total would be a smaller
    /// number claiming to be the whole.
    #[error("the weighted total would overflow")]
    ScoreWouldOverflow,
    /// There was no displayed score to hide or to reweight.
    #[error("no auxiliary score is displayed")]
    NoScoreIsDisplayed,
    /// A published document carried no non-guarantee notice.
    #[error("the document carries no non-guarantee notice")]
    NoticeIsMissing,
    /// A published document carried a notice that is not the rendered one.
    #[error("the document's non-guarantee notice is not the one this build renders")]
    NoticeDoesNotMatch,
}

/// Section 7.1's node type a readiness row is about, from the shared
/// vocabulary.
///
/// `academic_domain` already places `Competency` in the node hierarchy, so this
/// reads that enumeration rather than declaring a second one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum RowSubject {
    /// A row is about one competency of the bundle.
    Competency,
}

impl RowSubject {
    /// The domain node type this subject is.
    #[must_use]
    pub const fn node_type() -> academic_domain::predicates::NodeType {
        academic_domain::predicates::NodeType::Competency
    }
}
