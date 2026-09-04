//! The two recorded criteria this crate has, and why neither has a default.
//!
//! Section 8.3 puts a bound in two places and states neither number.
//! `CONFIRMED` requires 최근 확인 — a *recent* verification — and does not say
//! how recent. `HISTORICALLY_LIKELY` requires 여러 과거 학기의 재현 가능한 패턴
//! — a reproducible pattern over *several* past terms — and does not say how
//! many, or how likely a calibrated probability has to read before the pattern
//! counts as reproducible. `t001`'s `REQ-08-024`, `REQ-08-025` and `REQ-08-026`
//! rows each record the missing number as an open gate candidate.
//!
//! So neither is a constant here. [`VerificationRecency`] and
//! [`ForecastPolicy`] have private fields, one constructor each that takes
//! every bound, **no `Default`**, and no associated constant. `P2-U3` took the
//! same shape for `SourceFreshnessPolicy` for the same reason: a threshold
//! nobody has recorded, given a plausible-looking default, is a verdict
//! manufactured out of nothing.
//!
//! What stands while they are empty is stated where it bites.
//! `academic_offering::source::ConfirmationEvidence::from_registration_system`
//! cannot be called without a [`VerificationRecency`], and
//! [`crate::standing::resolve`] takes `Option<&ForecastPolicy>` and abstains
//! with [`crate::standing::AbstentionReason::ForecastPolicyAbsent`] when it is
//! `None`. The fixtures in this crate record a **synthetic, user-confirmed**
//! pair, labelled as such, so a case that reaches each side exists to check.

use crate::error::OfferingError;

/// How recent a registration-system reading has to be to confirm an offering.
///
/// Section 8.3's `CONFIRMED` row: *해당 학기 공식 수강편람/수강신청 시스템에
/// 존재하고 최근 확인*. The bound is in milliseconds because the reading
/// carries a `TimestampMillis`; the *term* axis orders history, and this is
/// the one place a wall-clock interval is the right unit, because it measures
/// how stale a retrieval is rather than how far apart two academic terms are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerificationRecency {
    within_millis: u64,
}

impl VerificationRecency {
    /// Records the bound. There is no other constructor and no default.
    pub fn new(within_millis: u64) -> Result<Self, OfferingError> {
        if within_millis == 0 {
            return Err(OfferingError::EmptyField("verification recency"));
        }
        Ok(Self { within_millis })
    }

    /// The recorded bound.
    #[must_use]
    pub const fn within_millis(self) -> u64 {
        self.within_millis
    }
}

/// The two numbers a forecast needs and no official source states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ForecastPolicy {
    likely_floor_permille: u16,
    minimum_window_terms: u32,
}

impl ForecastPolicy {
    /// Records both bounds. There is no other constructor and no default.
    ///
    /// `likely_floor_permille` is how high a *calibrated* probability has to
    /// read before section 8.3's 재현 가능한 패턴 holds; `minimum_window_terms`
    /// is how many same-semester terms have to have been read before there is
    /// a pattern to speak of at all.
    pub fn new(
        likely_floor_permille: u16,
        minimum_window_terms: u32,
    ) -> Result<Self, OfferingError> {
        if likely_floor_permille > 1000 {
            return Err(OfferingError::EmptyField("likely floor permille"));
        }
        if minimum_window_terms == 0 {
            return Err(OfferingError::EmptyField("minimum window terms"));
        }
        Ok(Self {
            likely_floor_permille,
            minimum_window_terms,
        })
    }

    /// The calibrated permille a forecast has to reach to be likely.
    #[must_use]
    pub const fn likely_floor_permille(self) -> u16 {
        self.likely_floor_permille
    }

    /// The same-semester terms a window has to hold before it is a pattern.
    #[must_use]
    pub const fn minimum_window_terms(self) -> u32 {
        self.minimum_window_terms
    }
}
