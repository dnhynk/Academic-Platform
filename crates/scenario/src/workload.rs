//! Projected workload, expressed as a range rather than a point estimate.

use serde::{Deserialize, Serialize, Serializer, de::Deserializer};

use crate::{error::ScenarioError, proposed::Proposed};

/// An inclusive weekly-hours range.
///
/// Workload is never a point estimate. §22.4 shows it as `34–46 h/week` beside
/// the sample count, the observation date, and the selection bias of the
/// reviews it came from, because a single number reads as a measurement of a
/// quantity nobody measured.
///
/// A bare range is an *input*: it is what the user or a review model assumes.
/// The simulator's *output* is a [`ProjectedWorkloadRange`], which seals the
/// same range inside [`Proposed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkloadHoursRange {
    low_hours: u16,
    high_hours: u16,
}

impl WorkloadHoursRange {
    /// Largest weekly hour count a range may name.
    ///
    /// A week holds 168 hours, so anything above it is a malformed input rather
    /// than an extreme opinion.
    pub const MAXIMUM_WEEKLY_HOURS: u16 = 168;

    /// Constructs an ordered, in-week range.
    pub fn new(low_hours: u16, high_hours: u16) -> Result<Self, ScenarioError> {
        if low_hours > high_hours || high_hours > Self::MAXIMUM_WEEKLY_HOURS {
            return Err(ScenarioError::InvalidWorkloadRange {
                low: low_hours,
                high: high_hours,
                maximum: Self::MAXIMUM_WEEKLY_HOURS,
            });
        }
        Ok(Self {
            low_hours,
            high_hours,
        })
    }

    /// Returns the low end of the range.
    #[must_use]
    pub const fn low_hours(self) -> u16 {
        self.low_hours
    }

    /// Returns the high end of the range.
    #[must_use]
    pub const fn high_hours(self) -> u16 {
        self.high_hours
    }

    /// Adds another range, saturating at one full week.
    ///
    /// A plan's workload is the sum of its choices, and the saturation keeps a
    /// nonsensical total representable rather than wrapping it into a small
    /// number that would read as a light semester.
    #[must_use]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self {
            low_hours: saturate(self.low_hours.saturating_add(other.low_hours)),
            high_hours: saturate(self.high_hours.saturating_add(other.high_hours)),
        }
    }
}

const fn saturate(hours: u16) -> u16 {
    if hours > WorkloadHoursRange::MAXIMUM_WEEKLY_HOURS {
        WorkloadHoursRange::MAXIMUM_WEEKLY_HOURS
    } else {
        hours
    }
}

/// A projected weekly workload range for one plan.
///
/// Sealed like every other proposal: there is no accessor that returns the
/// [`WorkloadHoursRange`] it proposes, so no expression turns a projected
/// workload into the integer a canonical writer would accept.
pub type ProjectedWorkloadRange = Proposed<WorkloadHoursRange>;

/// The wire form of a projected workload.
///
/// [`Proposed<T>`] is deliberately not serialisable for an arbitrary `T` — that
/// is what stops a projected mastery level from leaving as bytes. A projected
/// *workload* still has to cross the envelope so a reader can render the band,
/// so serialisation is implemented for exactly this one instantiation and for
/// no other.
///
/// This is a stated limit of the type isolation, not a hole in it: bytes are
/// not types, and a reader who re-encodes a disclosed range into a canonical
/// integer has forged a payload. That is the case
/// [`admit_projection_payload`](crate::envelope::admit_projection_payload)
/// exists to refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct WireWorkload {
    range: WorkloadHoursRange,
    provenance: crate::proposed::ProposalProvenance,
}

impl Serialize for ProjectedWorkloadRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        WireWorkload {
            range: *self.sealed_value(),
            provenance: *self.provenance(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProjectedWorkloadRange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = WireWorkload::deserialize(deserializer)?;
        Ok(Self::new(wire.range, wire.provenance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_rejects_reversed_and_overlong_spans() {
        assert!(WorkloadHoursRange::new(46, 34).is_err());
        assert!(WorkloadHoursRange::new(0, 169).is_err());
        assert!(WorkloadHoursRange::new(0, 168).is_ok());
        assert!(WorkloadHoursRange::new(34, 34).is_ok());
    }

    #[test]
    fn totals_saturate_at_one_week() -> Result<(), ScenarioError> {
        let heavy = WorkloadHoursRange::new(100, 160)?;
        let total = heavy.saturating_add(heavy);
        // Saturating rather than wrapping: a wrapped total would render as a
        // light semester, which is the one reading that must never happen.
        assert_eq!(total.low_hours(), 168);
        assert_eq!(total.high_hours(), 168);
        Ok(())
    }
}
