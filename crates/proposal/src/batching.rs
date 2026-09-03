//! Section 29.7's confidence/impact batching, and the versioned configuration
//! that decides the bands.

use academic_domain::ConfidencePermille;
use sha2::{Digest, Sha256};

use crate::{error::ThresholdError, proposed::ProposalId, tier::RiskTier};

/// Where the band edges are, and which version of them this is.
///
/// Section 29.7 says the queue batches on confidence and impact so a tired user
/// is not pushed into approval spam. It does not say where the edges go, and
/// they are a product decision that will move -- so the configuration carries
/// its own version and a digest over its contents, and a batch key carries the
/// version it was computed under. Two batches computed under different
/// configurations are therefore distinguishable rather than silently merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchingThresholds {
    version: u32,
    confidence_cuts: Vec<u16>,
    impact_cuts: Vec<u16>,
}

impl BatchingThresholds {
    /// The version of the shipped default.
    pub const SHIPPED_VERSION: u32 = 1;

    /// The shipped default: three confidence bands and three impact bands.
    ///
    /// The edges are round numbers on the permille scale and nothing here
    /// claims they are calibrated. What they are is a configuration with a
    /// version, so a later one is a different value rather than an edit.
    #[must_use]
    pub fn shipped() -> Self {
        Self {
            version: Self::SHIPPED_VERSION,
            confidence_cuts: vec![500, 800],
            impact_cuts: vec![300, 700],
        }
    }

    /// Builds a configuration, refusing one that does not describe a partition.
    ///
    /// # Errors
    ///
    /// [`ThresholdError`] when an axis has no cut, a cut is outside 1..=1000,
    /// or the cuts on an axis do not increase strictly.
    pub fn new(
        version: u32,
        confidence_cuts: Vec<u16>,
        impact_cuts: Vec<u16>,
    ) -> Result<Self, ThresholdError> {
        check_cuts("confidence", &confidence_cuts)?;
        check_cuts("impact", &impact_cuts)?;
        Ok(Self {
            version,
            confidence_cuts,
            impact_cuts,
        })
    }

    /// Which configuration this is.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// The confidence cut points, in order.
    #[must_use]
    pub fn confidence_cuts(&self) -> &[u16] {
        &self.confidence_cuts
    }

    /// The impact cut points, in order.
    #[must_use]
    pub fn impact_cuts(&self) -> &[u16] {
        &self.impact_cuts
    }

    /// SHA-256 over the version and both cut lists.
    ///
    /// A configuration that changed without its version changing produces a
    /// different digest, which is what makes the version claim checkable
    /// instead of decorative.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"academic-proposal/batching/1\n");
        hasher.update(self.version.to_be_bytes());
        for (axis, cuts) in [
            (b"confidence" as &[u8], &self.confidence_cuts),
            (b"impact", &self.impact_cuts),
        ] {
            hasher.update(axis);
            hasher.update(b"=");
            hasher.update(u32::try_from(cuts.len()).unwrap_or(u32::MAX).to_be_bytes());
            for cut in cuts {
                hasher.update(cut.to_be_bytes());
            }
            hasher.update(b"\n");
        }
        hasher.finalize().into()
    }

    /// The band a confidence value falls in.
    #[must_use]
    pub fn confidence_band(&self, confidence: ConfidencePermille) -> u8 {
        band_of(&self.confidence_cuts, confidence.value())
    }

    /// The band an impact value falls in.
    #[must_use]
    pub fn impact_band(&self, impact: crate::proposed::ImpactPermille) -> u8 {
        band_of(&self.impact_cuts, impact.value())
    }
}

/// The band `value` falls in: the count of cuts it is at or above.
///
/// Bands are half-open upward, so a value exactly on a cut belongs to the band
/// above it. Every `u16` lands in exactly one band, which is what makes the
/// batching a partition rather than a filter.
fn band_of(cuts: &[u16], value: u16) -> u8 {
    let count = cuts.iter().filter(|cut| value >= **cut).count();
    u8::try_from(count).unwrap_or(u8::MAX)
}

/// Rejects a cut list that does not divide the scale.
fn check_cuts(axis: &'static str, cuts: &[u16]) -> Result<(), ThresholdError> {
    let Some((first, rest)) = cuts.split_first() else {
        return Err(ThresholdError::NoCut { axis });
    };
    let mut previous = *first;
    check_range(axis, previous)?;
    for cut in rest {
        check_range(axis, *cut)?;
        if *cut <= previous {
            return Err(ThresholdError::CutsNotIncreasing {
                axis,
                previous,
                value: *cut,
            });
        }
        previous = *cut;
    }
    Ok(())
}

fn check_range(axis: &'static str, value: u16) -> Result<(), ThresholdError> {
    if value == 0 || value > 1000 {
        return Err(ThresholdError::CutOutOfRange { axis, value });
    }
    Ok(())
}

/// What one batch is about.
///
/// The tier is part of the key, not only the two bands, because grouping a
/// `NON_DELEGABLE` proposal with anything else is the bulk-approval shortcut
/// section 27.2 refuses. [`crate::ReviewQueue::batches`] additionally gives
/// every `NON_DELEGABLE` entry a batch of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchKey {
    /// The configuration version the bands were computed under.
    pub thresholds_version: u32,
    /// The tier every member of the batch shares.
    pub tier: RiskTier,
    /// The confidence band every member shares.
    pub confidence_band: u8,
    /// The impact band every member shares.
    pub impact_band: u8,
    /// The single member, for a tier that is never grouped.
    pub singleton: Option<ProposalId>,
}

/// One group of pending proposals a reviewer sees together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    key: BatchKey,
    members: Vec<ProposalId>,
}

impl Batch {
    pub(crate) const fn new(key: BatchKey, members: Vec<ProposalId>) -> Self {
        Self { key, members }
    }

    /// What the batch is about.
    #[must_use]
    pub const fn key(&self) -> BatchKey {
        self.key
    }

    /// The members, in ascending identifier order.
    #[must_use]
    pub fn members(&self) -> &[ProposalId] {
        &self.members
    }
}
