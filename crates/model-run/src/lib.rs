//! `P2-M1`: model-run provenance and the calibration registry.
//!
//! Three contracts live here.
//!
//! **Every model execution records the twelve section 27.3 fields.**
//! [`record::ModelRun`] has one constructor and it takes all twelve, so a run
//! that omits one does not compile. Migration `0007` is the storage half:
//! `model_run_provenance` fills the place migration `0004` left for this
//! aggregate's typed columns, and the two list fields get the child tables a
//! list in a column could not have been constrained in.
//!
//! **Every displayed confidence is interpreted through a per-model calibration
//! dataset.** [`calibration::RawScore`] has no ordering trait, no accessor that
//! returns its number, and a hand-written `Debug` that prints no number;
//! [`calibration::DisplayedConfidence::of`] takes a
//! [`calibration::CalibratedConfidence`], which only
//! [`calibration::CalibrationRegistry::interpret`] issues. So an uninterpreted
//! score reaching a reader is a type error rather than a check that one layer
//! out has already been skipped.
//!
//! **Raw scores from different providers are never ordered against each
//! other.** There is no comparison to order them with: the type implements
//! neither `PartialOrd` nor `Ord`, and offers no number to compare by hand.
//!
//! What this crate does not do is persist anything. The typed rows are
//! `academic-store`'s, written inside the acceptance transaction that inserts
//! the `MODEL_RUN_RECORDED` event, and the audit rows the reconciliation reads
//! are `academic-policy`'s.

pub mod calibration;
pub mod reanalysis;
pub mod reconcile;
pub mod record;

pub use calibration::{
    CalibratedConfidence, CalibrationBin, CalibrationDataset, CalibrationDatasetId,
    CalibrationRegistry, DisplayedConfidence, RawScore,
};
pub use reanalysis::{Candidate, CandidateId, ReanalysisDiff, ReanalysisError};
pub use reconcile::{Reconciliation, ReconciliationError, reconcile_transmitted_ranges};
pub use record::{
    ArtifactId, Cost, Digest32, EgressGrantId, InputArtifactRef, InputArtifactRefs, ModelRun,
    ModelRunId, ModelVersion, ProviderId, Purpose, RetentionDeclaration, Transmission,
    TransmittedRange,
};

/// Why a model-run record or a calibration dataset was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelRunError {
    /// A required text field was empty.
    #[error("{0} must not be empty")]
    EmptyField(&'static str),
    /// A run recorded no input artifact, so it has no provenance.
    #[error("a model run reads at least one input artifact")]
    NoInputArtifacts,
    /// A half-open range was empty or unnamed.
    #[error("a transmitted range names an object and covers at least one byte")]
    InvalidRange,
    /// An egressed transmission carried no range to reconcile.
    #[error("an egressed transmission records at least one byte range")]
    EgressWithoutRanges,
    /// A cost currency was not a three-letter uppercase code.
    #[error("a cost currency is a three-letter uppercase code")]
    InvalidCurrency,
    /// A calibration dataset had no samples or no bins.
    #[error("a calibration dataset has at least one sample and one bin")]
    EmptyCalibrationDataset,
    /// A calibration dataset declared a zero refresh interval.
    #[error("a calibration dataset refreshes on a positive interval")]
    InvalidRefreshInterval,
    /// A calibration curve folded back on itself.
    #[error("a calibration curve is increasing in raw units and non-decreasing in permille")]
    NonMonotonicCalibration,
    /// A second dataset was registered for one provider, version and purpose.
    #[error("a calibration dataset is already registered for this model and purpose")]
    DuplicateCalibrationDataset,
    /// No dataset interprets this provider's numbers for this purpose.
    #[error("no calibration dataset is registered for provider {0}")]
    NoCalibrationDataset(String),
    /// The dataset that would interpret this number has aged out.
    #[error("calibration dataset {0} is past its refresh interval")]
    StaleCalibrationDataset(String),
    /// The raw number is above every bin the dataset measured.
    #[error("calibration dataset {0} does not cover this raw score")]
    RawScoreOutsideCalibration(String),
    /// A permille value was outside the ledger's inclusive range.
    #[error("confidence permille must be in 0..=1000, got {0}")]
    ConfidenceOutOfRange(u16),
}
