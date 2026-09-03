//! Correction markers, and why a time-travel view cannot hide one.
//!
//! Section 34.6's fourth recovery principle: *`과거 화면에는 당시 잘못된 결과가
//! 사용되었음을 correction marker로 남긴다`*. Section 31.2 fixes what a past
//! screen is: a projection at a coordinate, read either as-known-at or
//! valid-at, over events nothing overwrites.
//!
//! # The one rule that makes this work
//!
//! A correction is recorded *after* the reading it corrects. So if a marker
//! were filtered by the same as-known-at coordinate as the claims it annotates,
//! it would be invisible in exactly the view that needs it: the view of the
//! moment the wrong value was used. [`CorrectionLedger::view_at`] therefore
//! reads two things at two different coordinates on purpose:
//!
//! * the **claims shown** are filtered by the coordinate, because that is what
//!   the past screen showed and rewriting it would destroy the record;
//! * the **markers** are not, because a marker is a present-tense statement
//!   *about* that past screen.
//!
//! `correction_marker_appears_in_historical_views` drives both halves and a
//! control: a view whose coordinate never reached the corrected claim carries
//! no marker, so the marker is attached to the claim rather than shown
//! unconditionally.
//!
//! # What a marker does not do
//!
//! It does not change what the view shows. The wrong claim is still in
//! [`HistoricalView::shown`] after the correction lands, which is section
//! 34.6's first and second principles — the original artefact and the existing
//! claim are preserved, and the wrong claim is superseded by a corrected claim
//! added beside it.

use academic_domain::{ClaimId, TimestampMillis, temporal::TimeCoordinates};

/// Why a recorded value moved.
///
/// `P2-C6`'s four origins, named here rather than redefined, because a marker
/// that could not say whether the user changed or the observation system
/// changed would be section 34.4's `analyzer/model 변화가 code 변화처럼 보임` row
/// happening on the trust screen itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CorrectionOrigin {
    /// Evidence about the subject moved.
    EvidenceChange,
    /// An identity merge or split moved what the value attaches to.
    OntologyChange,
    /// The projector that computed the value changed version.
    AnalyzerUpgrade,
    /// An official source superseded an earlier official statement.
    OfficialSourceCorrection,
}

impl CorrectionOrigin {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::EvidenceChange,
        Self::OntologyChange,
        Self::AnalyzerUpgrade,
        Self::OfficialSourceCorrection,
    ];

    /// Whether this is a change in the observation system rather than in the
    /// user.
    #[must_use]
    pub const fn is_observation_system_change(self) -> bool {
        match self {
            Self::EvidenceChange => false,
            Self::OntologyChange | Self::AnalyzerUpgrade | Self::OfficialSourceCorrection => true,
        }
    }
}

/// One claim as one past screen used it.
///
/// `accepted_at_seq` is the acceptance sequence the claim entered the ledger
/// at, and `applies` is the valid instant from which it applied. Both are
/// needed: section 31.1's whole point is that a `recordedAt` and a `validFrom`
/// collapsed into one `updatedAt` mixes a past audit with a future plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsedClaim {
    claim: ClaimId,
    accepted_at_seq: u64,
    applies_from: TimestampMillis,
}

impl UsedClaim {
    /// A claim a past screen used.
    #[must_use]
    pub const fn new(claim: ClaimId, accepted_at_seq: u64, applies_from: TimestampMillis) -> Self {
        Self {
            claim,
            accepted_at_seq,
            applies_from,
        }
    }

    /// Which claim.
    #[must_use]
    pub const fn claim(&self) -> ClaimId {
        self.claim
    }

    /// The acceptance sequence it entered the ledger at.
    #[must_use]
    pub const fn accepted_at_seq(&self) -> u64 {
        self.accepted_at_seq
    }

    /// The valid instant from which it applied.
    #[must_use]
    pub const fn applies_from(&self) -> TimestampMillis {
        self.applies_from
    }

    /// Whether this claim was visible at `coordinates`.
    ///
    /// Both axes, and neither stands in for the other: known-at compares the
    /// acceptance sequence, valid-at compares the applicability instant.
    #[must_use]
    pub fn visible_at(&self, coordinates: TimeCoordinates) -> bool {
        self.accepted_at_seq <= coordinates.known_at_accept_seq
            && self.applies_from <= coordinates.valid_at
    }
}

/// One correction, as a marker over the claim it corrects.
///
/// It names the corrected claim and the claim that supersedes it. It does not
/// carry a value, because a marker is not a second answer: the corrected claim
/// is where the corrected answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrectionMarker {
    corrected: ClaimId,
    superseding: ClaimId,
    origin: CorrectionOrigin,
    recorded_at_seq: u64,
    recorded_at: TimestampMillis,
}

impl CorrectionMarker {
    /// A correction.
    #[must_use]
    pub const fn new(
        corrected: ClaimId,
        superseding: ClaimId,
        origin: CorrectionOrigin,
        recorded_at_seq: u64,
        recorded_at: TimestampMillis,
    ) -> Self {
        Self {
            corrected,
            superseding,
            origin,
            recorded_at_seq,
            recorded_at,
        }
    }

    /// The claim that turned out to be wrong.
    #[must_use]
    pub const fn corrected(&self) -> ClaimId {
        self.corrected
    }

    /// The claim that replaced it.
    #[must_use]
    pub const fn superseding(&self) -> ClaimId {
        self.superseding
    }

    /// Why the value moved.
    #[must_use]
    pub const fn origin(&self) -> CorrectionOrigin {
        self.origin
    }

    /// The acceptance sequence the correction was recorded at.
    ///
    /// It is *not* compared against a view's known-at coordinate. It is here so
    /// that a reader can see that the correction came after the reading, which
    /// is the fact the marker exists to state.
    #[must_use]
    pub const fn recorded_at_seq(&self) -> u64 {
        self.recorded_at_seq
    }

    /// When the correction was recorded.
    #[must_use]
    pub const fn recorded_at(&self) -> TimestampMillis {
        self.recorded_at
    }
}

/// One past screen: what it showed, and what is now known about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalView {
    coordinates: TimeCoordinates,
    shown: Vec<UsedClaim>,
    markers: Vec<CorrectionMarker>,
}

impl HistoricalView {
    /// Which coordinates this view was read at.
    #[must_use]
    pub const fn coordinates(&self) -> TimeCoordinates {
        self.coordinates
    }

    /// Exactly the claims the screen showed at those coordinates.
    ///
    /// A correction does not remove one. A screen that quietly dropped the
    /// claim it later corrected would be unable to answer the question the
    /// marker exists for: what was this decision made on?
    #[must_use]
    pub fn shown(&self) -> &[UsedClaim] {
        &self.shown
    }

    /// Every correction that applies to a claim this screen showed.
    ///
    /// Not filtered by the view's coordinates. A correction is always recorded
    /// after the reading it corrects, so filtering markers by known-at would
    /// hide every one of them from the view that needs it.
    #[must_use]
    pub fn markers(&self) -> &[CorrectionMarker] {
        &self.markers
    }

    /// Whether this screen carries a marker for `claim`.
    #[must_use]
    pub fn is_marked(&self, claim: ClaimId) -> bool {
        self.markers
            .iter()
            .any(|marker| marker.corrected() == claim)
    }
}

/// The claims a screen can show, and every correction recorded against them.
#[derive(Debug, Clone, Default)]
pub struct CorrectionLedger {
    used: Vec<UsedClaim>,
    markers: Vec<CorrectionMarker>,
}

impl CorrectionLedger {
    /// An empty ledger.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            used: Vec::new(),
            markers: Vec::new(),
        }
    }

    /// Records that a claim entered the ledger and became showable.
    pub fn record_claim(&mut self, claim: UsedClaim) {
        self.used.push(claim);
    }

    /// Records a correction. Nothing already recorded is removed or edited.
    pub fn record_correction(&mut self, marker: CorrectionMarker) {
        self.markers.push(marker);
    }

    /// Every claim the ledger holds.
    #[must_use]
    pub fn claims(&self) -> &[UsedClaim] {
        &self.used
    }

    /// Every correction the ledger holds.
    #[must_use]
    pub fn corrections(&self) -> &[CorrectionMarker] {
        &self.markers
    }

    /// The past screen at `coordinates`.
    ///
    /// The claims are filtered by both bitemporal axes. The markers are the
    /// ones whose corrected claim is among those claims, filtered by nothing
    /// else — see this module's header for why.
    #[must_use]
    pub fn view_at(&self, coordinates: TimeCoordinates) -> HistoricalView {
        let shown: Vec<UsedClaim> = self
            .used
            .iter()
            .filter(|claim| claim.visible_at(coordinates))
            .copied()
            .collect();
        let markers: Vec<CorrectionMarker> = self
            .markers
            .iter()
            .filter(|marker| {
                shown
                    .iter()
                    .any(|claim| claim.claim() == marker.corrected())
            })
            .copied()
            .collect();
        HistoricalView {
            coordinates,
            shown,
            markers,
        }
    }
}
