//! Why an offering standing, a forecast, a seat or an evaluation was refused.
//!
//! Every variant names the exact thing that is missing or wrong. `§2.3-11`
//! makes `panic`, `unwrap` and `expect` workspace-denied, so a malformed
//! history, an empty evaluation set or an absent calibration dataset arrives
//! here as a value rather than as an abort.

use academic_curriculum::CurriculumError;
use academic_domain::DomainError;
use academic_model_run::ModelRunError;
use academic_record::RecordError;

/// Why this crate refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OfferingError {
    /// A required text field was empty.
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    /// Two observations claim the same course and term.
    #[error("the history already records {course} in {term}")]
    DuplicateObservation {
        /// The course the second observation named.
        course: String,
        /// The term both observations named.
        term: String,
    },
    /// A window's first term is not before its last.
    #[error("an observation window runs from an earlier term to a later one")]
    EmptyWindow,
    /// The forecast horizon is inside the window it would be forecast from.
    #[error("the forecast term {0} is inside its own observation window")]
    HorizonInsideWindow(String),
    /// A confirmation was founded on a reading that is not §8.4's level four.
    #[error("only the registration system confirms an offering; {0} is a cross source")]
    NotTheRegistrationSystem(&'static str),
    /// The primary reading is older than the recorded verification bound.
    #[error("the registration reading is older than the recorded verification bound")]
    VerificationStale,
    /// The registration system was read for this term and listed no section.
    #[error("the registration reading lists no section, so it confirms nothing")]
    BasisListsNoSection,
    /// An evaluation set had no entries at all.
    #[error("a term evaluation covers at least one course")]
    EmptyEvaluation,
    /// A calibrated probability could not be produced.
    #[error("the forecast probability has no fresh calibration dataset: {0}")]
    Uncalibrated(ModelRunError),
    /// The domain layer refused a claim, a window or a confidence.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// `P2-U1` refused a course code, a term code or an instructor name.
    #[error(transparent)]
    Curriculum(#[from] CurriculumError),
    /// `P2-U4` refused a term or a plan choice.
    ///
    /// Carried as text rather than by value: `academic_record::RecordError` is
    /// `Debug + Error` and derives neither `Clone` nor `Eq`, and this type is
    /// both so a test can compare two refusals for equality. Widening
    /// `RecordError` to satisfy this crate would change `P2-U4`'s public
    /// surface for a reason that is not `P2-U4`'s.
    #[error("{0}")]
    Record(String),
    /// `P2-M1` refused a raw score, a dataset or an interpretation.
    #[error(transparent)]
    ModelRun(#[from] ModelRunError),
    /// The engine harness refused the frozen inputs or the proof tree.
    #[error("{0}")]
    Engine(String),
}

impl From<RecordError> for OfferingError {
    fn from(error: RecordError) -> Self {
        Self::Record(error.to_string())
    }
}
