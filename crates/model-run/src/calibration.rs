//! Confidence is interpreted through a per-model calibration dataset, and a
//! provider's raw number never reaches a reader or another provider's number.
//!
//! The two prohibitions are types rather than checks:
//!
//!   * [`RawScore`] implements no ordering trait, has no accessor that returns
//!     its number, and hand-writes `Debug` so no formatting trait prints one.
//!     `<`, `max`, `sort`, `cmp` and `BTreeSet` are therefore all compile
//!     errors on it, and there is no number to compare across providers even by
//!     hand.
//!   * [`DisplayedConfidence::of`] takes a [`CalibratedConfidence`], which only
//!     [`CalibrationRegistry::interpret`] issues. A raw score handed to the
//!     display path is a type error, not a missing run-time check.
//!
//! `CalibratedConfidence` *is* ordered, and that is the point of calibrating:
//! two datasets map their own provider's numbers onto the one permille scale,
//! so values on that scale are comparable when the raw ones were not.

use core::{cmp::Ordering, fmt};

use academic_domain::ConfidencePermille;

use crate::{
    ModelRunError,
    record::{Digest32, ModelVersion, ProviderId, Purpose},
};

/// A provider's own confidence number, before any interpretation.
///
/// Deliberately not `PartialOrd`, not `Ord`, and not `Display`. Two providers'
/// numbers mean different things, so the type offers no way to rank them --
/// including no way to read the number back out and rank it by hand.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RawScore {
    provider: ProviderId,
    model_version: ModelVersion,
    units: u32,
}

impl RawScore {
    /// Records a provider's raw number together with whose number it is.
    #[must_use]
    pub const fn new(provider: ProviderId, model_version: ModelVersion, units: u32) -> Self {
        Self {
            provider,
            model_version,
            units,
        }
    }

    /// Which provider produced the number.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which model version produced the number.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }
}

impl fmt::Debug for RawScore {
    /// Prints whose score this is and never the number.
    ///
    /// A `Debug` that printed the units would be a second display path around
    /// [`DisplayedConfidence`], reachable with `format!("{:?}")` from anywhere.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawScore")
            .field("provider", &self.provider.as_str())
            .field("model_version", &self.model_version.as_str())
            .field("units", &"<uncalibrated>")
            .finish()
    }
}

/// One point of a calibration curve: raw units at or below `upper_raw_units`
/// are read as `confidence_permille`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CalibrationBin {
    upper_raw_units: u32,
    confidence_permille: u16,
}

impl CalibrationBin {
    /// Constructs one bin, refusing a permille outside the ledger's range.
    pub fn new(upper_raw_units: u32, confidence_permille: u16) -> Result<Self, ModelRunError> {
        if confidence_permille > 1000 {
            return Err(ModelRunError::ConfidenceOutOfRange(confidence_permille));
        }
        Ok(Self {
            upper_raw_units,
            confidence_permille,
        })
    }

    /// The inclusive upper bound of raw units this bin covers.
    #[must_use]
    pub const fn upper_raw_units(&self) -> u32 {
        self.upper_raw_units
    }

    /// The calibrated permille this bin reads its raw units as.
    #[must_use]
    pub const fn confidence_permille(&self) -> u16 {
        self.confidence_permille
    }
}

/// A calibration dataset identifier.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CalibrationDatasetId(String);

impl CalibrationDatasetId {
    /// Constructs a non-empty dataset identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ModelRunError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ModelRunError::EmptyField("CalibrationDatasetId"));
        }
        Ok(Self(value))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A per-model calibration dataset, with the refresh metadata that decides
/// whether it may still be used.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CalibrationDataset {
    id: CalibrationDatasetId,
    provider: ProviderId,
    model_version: ModelVersion,
    purpose: Purpose,
    digest: Digest32,
    sample_count: u32,
    refreshed_at: u64,
    refresh_interval_millis: u64,
    bins: Vec<CalibrationBin>,
}

impl CalibrationDataset {
    /// Registers one dataset.
    ///
    /// The bins must be non-empty, strictly increasing in raw units, and
    /// non-decreasing in permille: a curve that folded back would read a higher
    /// raw number as less confident, which is not a calibration.
    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments are the dataset identity plus the refresh metadata that decides staleness"
    )]
    pub fn new(
        id: CalibrationDatasetId,
        provider: ProviderId,
        model_version: ModelVersion,
        purpose: Purpose,
        digest: Digest32,
        sample_count: u32,
        refreshed_at: u64,
        refresh_interval_millis: u64,
        bins: Vec<CalibrationBin>,
    ) -> Result<Self, ModelRunError> {
        if sample_count == 0 {
            return Err(ModelRunError::EmptyCalibrationDataset);
        }
        if refresh_interval_millis == 0 {
            return Err(ModelRunError::InvalidRefreshInterval);
        }
        if bins.is_empty() {
            return Err(ModelRunError::EmptyCalibrationDataset);
        }
        for pair in bins.windows(2) {
            let [lower, upper] = pair else {
                continue;
            };
            if upper.upper_raw_units() <= lower.upper_raw_units()
                || upper.confidence_permille() < lower.confidence_permille()
            {
                return Err(ModelRunError::NonMonotonicCalibration);
            }
        }
        Ok(Self {
            id,
            provider,
            model_version,
            purpose,
            digest,
            sample_count,
            refreshed_at,
            refresh_interval_millis,
            bins,
        })
    }

    /// The dataset identifier.
    #[must_use]
    pub const fn id(&self) -> &CalibrationDatasetId {
        &self.id
    }

    /// The provider whose numbers this dataset interprets.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The exact model version this dataset was measured on.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// The purpose the dataset was measured for.
    #[must_use]
    pub const fn purpose(&self) -> &Purpose {
        &self.purpose
    }

    /// Digest of the dataset contents.
    #[must_use]
    pub const fn digest(&self) -> &Digest32 {
        &self.digest
    }

    /// How many observations the curve rests on.
    #[must_use]
    pub const fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// When the dataset was last refreshed.
    #[must_use]
    pub const fn refreshed_at(&self) -> u64 {
        self.refreshed_at
    }

    /// How long a refresh stays valid.
    #[must_use]
    pub const fn refresh_interval_millis(&self) -> u64 {
        self.refresh_interval_millis
    }

    /// Whether the dataset has aged out at `now`.
    ///
    /// A clock before the refresh is stale as well: a dataset that claims to
    /// have been measured in the future is not one to interpret through.
    #[must_use]
    pub const fn is_stale(&self, now: u64) -> bool {
        match self.refreshed_at.checked_add(self.refresh_interval_millis) {
            None => now < self.refreshed_at,
            Some(expiry) => now < self.refreshed_at || now >= expiry,
        }
    }
}

/// Registered calibration datasets, keyed by provider, model version, purpose.
#[derive(Clone, Default, Debug)]
pub struct CalibrationRegistry {
    datasets: Vec<CalibrationDataset>,
}

impl CalibrationRegistry {
    /// An empty registry. Nothing is displayable until a dataset is registered.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            datasets: Vec::new(),
        }
    }

    /// Registers one dataset, refusing a second for the same key.
    pub fn register(&mut self, dataset: CalibrationDataset) -> Result<(), ModelRunError> {
        if self
            .datasets
            .iter()
            .any(|existing| Self::matches(existing, &dataset))
        {
            return Err(ModelRunError::DuplicateCalibrationDataset);
        }
        self.datasets.push(dataset);
        Ok(())
    }

    fn matches(existing: &CalibrationDataset, candidate: &CalibrationDataset) -> bool {
        existing.provider() == candidate.provider()
            && existing.model_version() == candidate.model_version()
            && existing.purpose() == candidate.purpose()
    }

    /// The registered datasets, in registration order.
    #[must_use]
    pub fn datasets(&self) -> &[CalibrationDataset] {
        &self.datasets
    }

    /// Interprets one provider's raw number through that model's dataset.
    ///
    /// This is the only function in the workspace that produces a
    /// [`CalibratedConfidence`], and therefore the only route to a displayable
    /// confidence. Without a fresh dataset for the exact provider, model
    /// version and purpose, it returns an error and there is nothing to show.
    pub fn interpret(
        &self,
        score: &RawScore,
        purpose: &Purpose,
        now: u64,
    ) -> Result<CalibratedConfidence, ModelRunError> {
        let dataset = self
            .datasets
            .iter()
            .find(|dataset| {
                dataset.provider() == score.provider()
                    && dataset.model_version() == score.model_version()
                    && dataset.purpose() == purpose
            })
            .ok_or_else(|| {
                ModelRunError::NoCalibrationDataset(score.provider().as_str().to_owned())
            })?;
        if dataset.is_stale(now) {
            return Err(ModelRunError::StaleCalibrationDataset(
                dataset.id().as_str().to_owned(),
            ));
        }
        let permille = dataset
            .bins
            .iter()
            .find(|bin| score.units <= bin.upper_raw_units())
            .ok_or_else(|| {
                ModelRunError::RawScoreOutsideCalibration(dataset.id().as_str().to_owned())
            })?
            .confidence_permille();
        Ok(CalibratedConfidence {
            confidence: ConfidencePermille::new(permille)
                .map_err(|_| ModelRunError::ConfidenceOutOfRange(permille))?,
            dataset: dataset.id().clone(),
            dataset_digest: *dataset.digest(),
            provider: score.provider().clone(),
            model_version: score.model_version().clone(),
        })
    }
}

/// A confidence that has been read through a named calibration dataset.
///
/// Ordered, and only this type is: interpreting is what puts two providers'
/// numbers on one scale, so a comparison after it means something and a
/// comparison before it does not.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CalibratedConfidence {
    confidence: ConfidencePermille,
    dataset: CalibrationDatasetId,
    dataset_digest: Digest32,
    provider: ProviderId,
    model_version: ModelVersion,
}

impl CalibratedConfidence {
    /// The calibrated confidence on the ledger's shared permille scale.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// Which dataset interpreted the raw number.
    #[must_use]
    pub const fn dataset(&self) -> &CalibrationDatasetId {
        &self.dataset
    }

    /// Digest of that dataset's contents.
    #[must_use]
    pub const fn dataset_digest(&self) -> &Digest32 {
        &self.dataset_digest
    }

    /// The provider whose number was interpreted.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The model version whose number was interpreted.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }
}

impl PartialOrd for CalibratedConfidence {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CalibratedConfidence {
    /// Orders by the calibrated permille alone.
    ///
    /// Not derived: a derived ordering would compare the fields in declaration
    /// order, which would rank by permille and then by dataset identifier and
    /// provider name -- an ordering with a provider's name in it, which is the
    /// thing this module exists to refuse.
    fn cmp(&self, other: &Self) -> Ordering {
        self.confidence.value().cmp(&other.confidence.value())
    }
}

/// A confidence that has reached a reader.
///
/// [`DisplayedConfidence::of`] is the only constructor and it takes a
/// [`CalibratedConfidence`], so the display surface cannot be reached from an
/// uninterpreted number.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DisplayedConfidence {
    permille: u16,
    dataset: CalibrationDatasetId,
}

impl DisplayedConfidence {
    /// Prepares a calibrated confidence for display.
    #[must_use]
    pub fn of(calibrated: &CalibratedConfidence) -> Self {
        Self {
            permille: calibrated.confidence().value(),
            dataset: calibrated.dataset().clone(),
        }
    }

    /// The dataset a reader is shown beside the number.
    #[must_use]
    pub const fn dataset(&self) -> &CalibrationDatasetId {
        &self.dataset
    }
}

impl fmt::Display for DisplayedConfidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}% (calibrated by {})",
            self.permille / 10,
            self.permille % 10,
            self.dataset.as_str()
        )
    }
}
