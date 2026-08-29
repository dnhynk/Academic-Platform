//! The sealed proposal wrapper that projected values never escape.

use std::{cmp::Ordering, fmt};

use academic_domain::{ContentDigest, MasteryLevel, ModelRunId, TimestampMillis};
use serde::{Deserialize, Serialize};

/// A value proposed by a model, a simulator, or any other non-observational
/// lane, sealed away from the canonical types a writer accepts.
///
/// The seal is the absence of an exit, so the list of what is deliberately
/// missing is part of the contract:
///
/// - no `into_inner`, `value`, or `get` — nothing returns `T`;
/// - no [`Deref`](std::ops::Deref), [`AsRef`], or [`Borrow`](std::borrow::Borrow);
/// - no `From<Proposed<T>> for T`, and therefore no `Into`;
/// - no `map`/`and_then` taking a caller closure, which would hand `T` out
///   under another name;
/// - no blanket [`Serialize`], so the value cannot leave as bytes either;
/// - a [`Debug`] that redacts the value rather than printing it into a log.
///
/// What remains is enough to review, rank, and calibrate a proposal without
/// ever holding the value it proposes. [`Proposed::provenance`] says where it
/// came from and [`Proposed::calibrate`] compares it against a later
/// observation, returning only the direction of the error.
///
/// Promotion to canonical state is not a conversion and there is no method for
/// it here. It is a user decision recorded as its own event, and the accepted
/// value is built from that decision rather than lifted out of the proposal.
pub struct Proposed<T> {
    value: T,
    provenance: ProposalProvenance,
}

impl<T> Proposed<T> {
    /// Seals a proposed value together with the provenance that produced it.
    #[must_use]
    pub const fn new(value: T, provenance: ProposalProvenance) -> Self {
        Self { value, provenance }
    }

    /// Returns where the proposal came from.
    #[must_use]
    pub const fn provenance(&self) -> &ProposalProvenance {
        &self.provenance
    }

    /// Reads the sealed value from inside this crate.
    ///
    /// Crate-private on purpose, and it is the only read of the value that
    /// exists. This crate has no dependency of any kind on the canonical writer
    /// crate, so a value read here cannot be carried to a write from here; the
    /// visibility is what stops the accessor from becoming a public exit.
    pub(crate) const fn sealed_value(&self) -> &T {
        &self.value
    }
}

impl<T: Ord> Proposed<T> {
    /// Compares a proposal against a later actual observation.
    ///
    /// This is the §22.5 calibration path: it reports whether the projection
    /// ran ahead of, matched, or lagged what actually happened, and reports
    /// nothing else. The proposed value is neither returned nor derivable from
    /// the result, so calibrating a model never becomes a way to read a
    /// projection back out. The subject of the result is the model, never the
    /// user.
    #[must_use]
    pub fn calibrate(&self, actual: &T) -> ProjectionCalibration {
        match self.value.cmp(actual) {
            Ordering::Less => ProjectionCalibration::Underprojected,
            Ordering::Equal => ProjectionCalibration::Matched,
            Ordering::Greater => ProjectionCalibration::Overprojected,
        }
    }
}

impl<T: PartialEq> PartialEq for Proposed<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.provenance == other.provenance
    }
}

impl<T: Eq> Eq for Proposed<T> {}

impl<T: Clone> Clone for Proposed<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            provenance: self.provenance,
        }
    }
}

impl<T: Copy> Copy for Proposed<T> {}

/// `Debug` redacts the proposed value.
///
/// `missing_debug_implementations` is denied workspace-wide, so this type needs
/// a `Debug`. A derived one would print the sealed value into every log line
/// and error message that formats a projection, and a mastery level recovered
/// from a log is the leak the seal exists to prevent.
impl<T> fmt::Debug for Proposed<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Proposed")
            .field("value", &"<sealed>")
            .field("provenance", &self.provenance)
            .finish()
    }
}

/// Direction of a projection error, measured against an actual observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectionCalibration {
    /// The projection was lower than what was actually observed.
    Underprojected,
    /// The projection equalled what was actually observed.
    Matched,
    /// The projection was higher than what was actually observed.
    Overprojected,
}

/// Why a proposal exists and what it was computed from.
///
/// §3.10 requires a `ModelRun` behind every `AI_INFERRED` or `PREDICTION`
/// claim; the same identifier is carried here so a projection is never
/// anonymous, even before anyone decides what to do with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProposalProvenance {
    model_run_id: ModelRunId,
    inputs_digest: ContentDigest,
    engine_version: u32,
    proposed_at: TimestampMillis,
}

impl ProposalProvenance {
    /// Records the model run, frozen inputs, engine version, and proposal time.
    #[must_use]
    pub const fn new(
        model_run_id: ModelRunId,
        inputs_digest: ContentDigest,
        engine_version: u32,
        proposed_at: TimestampMillis,
    ) -> Self {
        Self {
            model_run_id,
            inputs_digest,
            engine_version,
            proposed_at,
        }
    }

    /// Returns the model run required behind every non-observed value.
    #[must_use]
    pub const fn model_run_id(&self) -> ModelRunId {
        self.model_run_id
    }

    /// Returns the digest of the frozen inputs the proposal was computed from.
    #[must_use]
    pub const fn inputs_digest(&self) -> ContentDigest {
        self.inputs_digest
    }

    /// Returns the engine version that produced the proposal.
    #[must_use]
    pub const fn engine_version(&self) -> u32 {
        self.engine_version
    }

    /// Returns when the proposal was produced.
    #[must_use]
    pub const fn proposed_at(&self) -> TimestampMillis {
        self.proposed_at
    }
}

/// A proposed mastery level.
///
/// The simulator never emits one. §22.3 forbids projecting course completion
/// into mastery, and [`ScenarioProjection`](crate::simulate::ScenarioProjection)
/// has no field that could carry it. The alias exists because other proposal
/// lanes — a model reading a submitted assignment, say — do propose a mastery
/// level, and such a proposal must be exactly as unreachable from an
/// actual-state write as every other projection here.
pub type ProjectedMastery = Proposed<MasteryLevel>;

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> Result<ProposalProvenance, academic_domain::DomainError> {
        Ok(ProposalProvenance::new(
            "01936f2a-0000-7000-8000-0000000000a1".parse()?,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".parse()?,
            1,
            TimestampMillis::new(7),
        ))
    }

    #[test]
    fn debug_redacts_the_proposed_value() -> Result<(), academic_domain::DomainError> {
        // A derived `Debug` would print `Fluent` into every log line and error
        // message that formats a projection, which is the same leak the sealed
        // accessors prevent, arriving by another route.
        let proposed = ProjectedMastery::new(MasteryLevel::Fluent, provenance()?);
        let rendered = format!("{proposed:?}");
        assert!(rendered.contains("<sealed>"), "{rendered}");
        assert!(!rendered.contains("Fluent"), "{rendered}");
        for level in [
            MasteryLevel::Unseen,
            MasteryLevel::Exposed,
            MasteryLevel::Understood,
            MasteryLevel::Practiced,
            MasteryLevel::Applied,
        ] {
            assert!(!rendered.contains(&format!("{level:?}")), "{rendered}");
        }
        Ok(())
    }

    #[test]
    fn calibration_reports_direction_only() -> Result<(), academic_domain::DomainError> {
        let proposed = ProjectedMastery::new(MasteryLevel::Applied, provenance()?);
        assert_eq!(
            proposed.calibrate(&MasteryLevel::Fluent),
            ProjectionCalibration::Underprojected
        );
        assert_eq!(
            proposed.calibrate(&MasteryLevel::Applied),
            ProjectionCalibration::Matched
        );
        assert_eq!(
            proposed.calibrate(&MasteryLevel::Exposed),
            ProjectionCalibration::Overprojected
        );
        Ok(())
    }

    #[test]
    fn provenance_is_carried_verbatim() -> Result<(), academic_domain::DomainError> {
        let expected = provenance()?;
        let proposed = ProjectedMastery::new(MasteryLevel::Unseen, expected);
        assert_eq!(*proposed.provenance(), expected);
        assert_eq!(proposed.provenance().engine_version(), 1);
        assert_eq!(proposed.provenance().proposed_at(), TimestampMillis::new(7));
        Ok(())
    }
}
