//! Drift estimation, the confidence it produces, and the append-only mapping
//! ledger a manual realignment adds to.
//!
//! # Beyond tolerance is low confidence, not a failure
//!
//! Section 34.1's misalignment row asks for "±초 오차 범위와
//! `ALIGNMENT_LOW_CONFIDENCE`" as the *uncertainty display*, beside detection
//! and prevention. So a drift past the tolerance is neither refused nor ignored:
//! [`estimate_drift`] still returns an estimate, still carries the ± range, and
//! marks the confidence [`AlignmentConfidence::Low`].
//! `drift_beyond_tolerance_is_alignment_low_confidence` tests both sides of the
//! boundary — exactly at the tolerance is [`AlignmentConfidence::Normal`], one
//! nanosecond past it is low — and moves the effective policy row's date to
//! show the boundary is the row's and not a constant's.
//!
//! # A realignment appends a version and edits none
//!
//! Section 34.1's recovery cell is "수동 anchor 2개로 재정렬, mapping version
//! 추가". [`MappingLedger`] therefore has one mutating operation, and it pushes.
//! There is no `&mut` accessor into an existing [`MappingVersion`], no removal,
//! and no replacement, so the mapping a transcript was aligned under last week
//! is still readable beside the one it is aligned under now. That is ADR-003's
//! "corrections append a new assertion" applied to alignment rather than a
//! second correction mechanism invented for it.
//!
//! # An anchor is where a second clock would enter
//!
//! Both anchors carry [`crate::clock::SessionTick`]s, and a tick is unforgeable,
//! so the only way to hold one from another clock is to have started a second
//! clock. [`MappingLedger::append_realignment`] admits an anchor through the
//! session's own clock and refuses a foreign domain, which is the reachable
//! second-clock injection `shared_session_clock_for_audio_and_capture` runs.

use crate::{
    clock::{ClockFault, SessionClock, SessionTick},
    policy::CapturePolicyRow,
};

/// The section 34.1 badge an alignment carries when it is past tolerance.
pub const ALIGNMENT_LOW_CONFIDENCE: &str = "ALIGNMENT_LOW_CONFIDENCE";

/// One manual anchor: a session instant the user says lines up with a known
/// instant on the reference timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    at: SessionTick,
    reference_nanos: u64,
}

impl Anchor {
    /// Declares an anchor.
    #[must_use]
    pub const fn at(at: SessionTick, reference_nanos: u64) -> Self {
        Self {
            at,
            reference_nanos,
        }
    }

    /// Where it sits on the session clock.
    #[must_use]
    pub const fn session_tick(self) -> SessionTick {
        self.at
    }

    /// Where the user says it sits on the reference timeline.
    #[must_use]
    pub const fn reference_nanos(self) -> u64 {
        self.reference_nanos
    }

    /// The signed distance between the two, in nanoseconds.
    #[must_use]
    pub fn offset_nanos(self) -> Option<i64> {
        let session = i64::try_from(self.at.elapsed_nanos()).ok()?;
        let reference = i64::try_from(self.reference_nanos).ok()?;
        session.checked_sub(reference)
    }
}

/// How much an alignment can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentConfidence {
    /// The drift between the anchors is inside the effective tolerance.
    Normal,
    /// It is not. The badge is [`ALIGNMENT_LOW_CONFIDENCE`] and the range is
    /// the drift itself, because that is the width the alignment could be
    /// wrong by.
    Low {
        /// The ± range, in nanoseconds.
        plus_minus_nanos: u64,
    },
}

impl AlignmentConfidence {
    /// The badge a surface displays, or `None` while the alignment is normal.
    #[must_use]
    pub const fn badge(self) -> Option<&'static str> {
        match self {
            Self::Normal => None,
            Self::Low { .. } => Some(ALIGNMENT_LOW_CONFIDENCE),
        }
    }

    /// The ± range in whole seconds, rounded up, which is the unit section 34.1
    /// displays. Zero while the alignment is normal.
    #[must_use]
    pub const fn plus_minus_seconds(self) -> u64 {
        match self {
            Self::Normal => 0,
            Self::Low { plus_minus_nanos } => plus_minus_nanos.div_ceil(1_000_000_000),
        }
    }

    /// Whether this is the low-confidence arm.
    #[must_use]
    pub const fn is_low(self) -> bool {
        matches!(self, Self::Low { .. })
    }

    /// The frame byte.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Normal => 1,
            Self::Low { .. } => 2,
        }
    }
}

/// What two anchors say about the session clock against the reference timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriftEstimate {
    offset_nanos: i64,
    drift_nanos: i64,
    confidence: AlignmentConfidence,
}

impl DriftEstimate {
    /// The offset the first anchor fixes: session minus reference.
    #[must_use]
    pub const fn offset_nanos(self) -> i64 {
        self.offset_nanos
    }

    /// How much that offset has moved by the second anchor.
    #[must_use]
    pub const fn drift_nanos(self) -> i64 {
        self.drift_nanos
    }

    /// The confidence the drift produces against the effective tolerance.
    #[must_use]
    pub const fn confidence(self) -> AlignmentConfidence {
        self.confidence
    }
}

/// Why an estimate or a realignment was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AlignmentFault {
    /// An anchor came from another session clock.
    #[error("an anchor came from another session clock")]
    ForeignClock(#[from] ClockFault),
    /// The two anchors sit at the same session instant, so there is no interval
    /// to measure drift over.
    #[error("the two anchors sit at the same session instant")]
    AnchorsCoincide,
    /// The arithmetic left the range a signed nanosecond count can hold.
    #[error("the anchor interval does not fit a signed nanosecond count")]
    IntervalOutOfRange,
}

/// Estimates drift between two anchors against one policy row.
///
/// The drift is the change in offset between the two anchors. A drift whose
/// magnitude is *greater than* the tolerance is low confidence; a drift exactly
/// at it is not, which is the boundary
/// `drift_beyond_tolerance_is_alignment_low_confidence` tests from both sides.
pub fn estimate_drift(
    first: Anchor,
    second: Anchor,
    policy: CapturePolicyRow,
) -> Result<DriftEstimate, AlignmentFault> {
    if first.session_tick().elapsed_nanos() == second.session_tick().elapsed_nanos() {
        return Err(AlignmentFault::AnchorsCoincide);
    }
    let first_offset = first
        .offset_nanos()
        .ok_or(AlignmentFault::IntervalOutOfRange)?;
    let second_offset = second
        .offset_nanos()
        .ok_or(AlignmentFault::IntervalOutOfRange)?;
    let drift_nanos = second_offset
        .checked_sub(first_offset)
        .ok_or(AlignmentFault::IntervalOutOfRange)?;
    let magnitude = drift_nanos.unsigned_abs();
    let confidence = if magnitude > policy.drift_tolerance_nanos() {
        AlignmentConfidence::Low {
            plus_minus_nanos: magnitude,
        }
    } else {
        AlignmentConfidence::Normal
    };
    Ok(DriftEstimate {
        offset_nanos: first_offset,
        drift_nanos,
        confidence,
    })
}

/// One version of the mapping from session instants to the reference timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingVersion {
    version: u32,
    first: Anchor,
    second: Anchor,
    estimate: DriftEstimate,
    policy_id: &'static str,
}

impl MappingVersion {
    /// Its number, from one.
    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }

    /// The earlier anchor.
    #[must_use]
    pub const fn first(self) -> Anchor {
        self.first
    }

    /// The later anchor.
    #[must_use]
    pub const fn second(self) -> Anchor {
        self.second
    }

    /// What the two anchors say.
    #[must_use]
    pub const fn estimate(self) -> DriftEstimate {
        self.estimate
    }

    /// Which policy row's tolerance decided the confidence.
    #[must_use]
    pub const fn policy_id(self) -> &'static str {
        self.policy_id
    }
}

/// Every mapping version a session has, oldest first.
///
/// One mutating operation and it appends. `manual_two_anchor_realignment_appends_a_mapping_version`
/// compares version one before and after appending version two.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MappingLedger {
    versions: Vec<MappingVersion>,
}

impl MappingLedger {
    /// An empty ledger.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }

    /// Every version, oldest first.
    #[must_use]
    pub fn versions(&self) -> &[MappingVersion] {
        &self.versions
    }

    /// The mapping in force, which is the last one appended.
    #[must_use]
    pub fn current(&self) -> Option<MappingVersion> {
        self.versions.last().copied()
    }

    /// Appends a version derived from two manual anchors.
    ///
    /// Both anchors are admitted through the session's own clock first, so an
    /// anchor from a second clock is refused here rather than silently
    /// realigning the session against a timeline it never ran on.
    pub(crate) fn append_realignment(
        &mut self,
        clock: &SessionClock,
        first: Anchor,
        second: Anchor,
        policy: CapturePolicyRow,
    ) -> Result<MappingVersion, AlignmentFault> {
        clock.admit(first.session_tick())?;
        clock.admit(second.session_tick())?;
        let estimate = estimate_drift(first, second, policy)?;
        let version = MappingVersion {
            version: u32::try_from(self.versions.len())
                .unwrap_or(u32::MAX)
                .saturating_add(1),
            first,
            second,
            estimate,
            policy_id: policy.id(),
        };
        self.versions.push(version);
        Ok(version)
    }
}
