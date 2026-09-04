//! Section 13.3's six bands and its seven computation inputs.
//!
//! ## The six are not declared here
//!
//! `academic_domain::FreshnessBand` already holds them and this crate declares
//! no second enumeration. What is here is [`BANDS`] — the same six in the order
//! section 13.3's own sentence names them — and [`band_token`], whose `match`
//! has no wildcard arm. A seventh band added to the domain enumeration is a
//! compile error in this file rather than a value some list quietly fails to
//! mention.
//!
//! **The count is not a number in this crate.**
//! `freshness_bands_are_exactly_six` reads
//! `Freshness는 … band로 표시한다` back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits the sentence
//! on its own back-quoted spellings and compares them against [`BANDS`] in both
//! directions, so six is a measurement of the design document. The same test
//! reads the seven `- ` bullets under `계산 입력은 다음과 같다` and compares them
//! against [`FreshnessSignal::ALL`], so seven is too.
//!
//! ## The order in the sentence is not the order in the enumeration
//!
//! Section 13.3 writes the bands **best first** — `VERY_HIGH, HIGH, MODERATE,
//! LOW, STALE, UNKNOWN`. `academic_domain::FreshnessBand` derives `Ord` with
//! `Unknown` lowest, which is the opposite direction and the one arithmetic
//! needs. Both are kept: [`BANDS`] is the sentence's order and [`rank`] is the
//! enumeration's, and `freshness_bands_are_exactly_six` requires [`BANDS`] to be
//! strictly *decreasing* under [`rank`] so the two cannot silently drift apart.
//!
//! `UNKNOWN` sits at the bottom of that order and it is not "very stale": it is
//! the band for a concept about which nothing datable was ever admitted.
//! `STALE` means the opposite — something *was* admitted, and only its immediate
//! retrieval is unverified. That is the distinction section 13.3's own example
//! block draws, and [`crate::disclosure`] is where the user-facing half of it
//! lives.

use academic_domain::FreshnessBand;
use serde::{Deserialize, Serialize};

/// Section 13.3's six bands, in the order its own sentence names them.
///
/// Best first. Compared against the design document in both directions by
/// `freshness_bands_are_exactly_six`.
pub const BANDS: [FreshnessBand; 6] = [
    FreshnessBand::VeryHigh,
    FreshnessBand::High,
    FreshnessBand::Moderate,
    FreshnessBand::Low,
    FreshnessBand::Stale,
    FreshnessBand::Unknown,
];

/// The band's position in `academic_domain::FreshnessBand`'s own order, lowest
/// first.
///
/// Total with no wildcard arm, so a band added to the domain enumeration fails
/// to compile here rather than defaulting to zero.
#[must_use]
pub const fn rank(band: FreshnessBand) -> u8 {
    match band {
        FreshnessBand::Unknown => 0,
        FreshnessBand::Stale => 1,
        FreshnessBand::Low => 2,
        FreshnessBand::Moderate => 3,
        FreshnessBand::High => 4,
        FreshnessBand::VeryHigh => 5,
    }
}

/// The band's wire spelling, which is section 13.3's own.
///
/// Total with no wildcard arm.
#[must_use]
pub const fn band_token(band: FreshnessBand) -> &'static str {
    match band {
        FreshnessBand::Unknown => "UNKNOWN",
        FreshnessBand::Stale => "STALE",
        FreshnessBand::Low => "LOW",
        FreshnessBand::Moderate => "MODERATE",
        FreshnessBand::High => "HIGH",
        FreshnessBand::VeryHigh => "VERY_HIGH",
    }
}

/// The next band down, or `None` at the bottom.
///
/// This is how a bounded contribution is expressed as *lower than* rather than
/// as a number: [`crate::spillover`] demotes a neighbour's own band by one step
/// and takes the lower of that and its ceiling, so `spillover_is_one_hop_and_cited`
/// can compare the two bands instead of comparing two weights.
#[must_use]
pub const fn step_down(band: FreshnessBand) -> Option<FreshnessBand> {
    match band {
        FreshnessBand::Unknown => None,
        FreshnessBand::Stale => Some(FreshnessBand::Unknown),
        FreshnessBand::Low => Some(FreshnessBand::Stale),
        FreshnessBand::Moderate => Some(FreshnessBand::Low),
        FreshnessBand::High => Some(FreshnessBand::Moderate),
        FreshnessBand::VeryHigh => Some(FreshnessBand::High),
    }
}

/// The lower of two bands.
#[must_use]
pub fn floor_of(left: FreshnessBand, right: FreshnessBand) -> FreshnessBand {
    if rank(left) <= rank(right) {
        left
    } else {
        right
    }
}

/// The higher of two bands.
#[must_use]
pub fn ceiling_of(left: FreshnessBand, right: FreshnessBand) -> FreshnessBand {
    if rank(left) >= rank(right) {
        left
    } else {
        right
    }
}

/// Section 13.3's seven computation inputs, in the order its bullets list them.
///
/// This is the *trace* vocabulary the `P2-N3` contract fixes: every entry of a
/// [`crate::projection::FreshnessTrace`] names the input it came from, so a band
/// can be explained by the sentence of the design document that licensed it.
/// The seven are compared against the design document's own bullet list by
/// `freshness_bands_are_exactly_six`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FreshnessSignal {
    /// `마지막 strong evidence의 시점과 종류`
    LastStrongEvidence,
    /// `최근 일정 window의 반복 횟수와 간격`
    RepetitionAndInterval,
    /// `노출·복습보다 실제 적용·debugging·설계에 더 긴 지속성`
    EvidenceTypePersistence,
    /// `관련 concept의 최근 사용에서 오는 약한 spillover`
    RelatedConceptSpillover,
    /// `사용자 직접 “지금도 바로 사용할 수 있음/복습 필요” 확인`
    UserRecallStatement,
    /// `concept별 retention profile과 사용자별 경험적 보정`
    RetentionPriorAndCalibration,
    /// `반대 evidence: 설명 실패, 기억 안 남음 표시, 재학습 필요 event`
    ContraryEvidence,
}

impl FreshnessSignal {
    /// Exhaustive, in section 13.3's own bullet order.
    pub const ALL: [Self; 7] = [
        Self::LastStrongEvidence,
        Self::RepetitionAndInterval,
        Self::EvidenceTypePersistence,
        Self::RelatedConceptSpillover,
        Self::UserRecallStatement,
        Self::RetentionPriorAndCalibration,
        Self::ContraryEvidence,
    ];

    /// The design document's own bullet, verbatim.
    ///
    /// Total with no wildcard arm. `freshness_bands_are_exactly_six` requires
    /// each of these to be a line of section 13.3, so a bullet reworded in the
    /// design document fails here rather than drifting.
    #[must_use]
    pub const fn bullet(self) -> &'static str {
        match self {
            Self::LastStrongEvidence => "마지막 strong evidence의 시점과 종류",
            Self::RepetitionAndInterval => "최근 일정 window의 반복 횟수와 간격",
            Self::EvidenceTypePersistence => {
                "노출·복습보다 실제 적용·debugging·설계에 더 긴 지속성"
            }
            Self::RelatedConceptSpillover => "관련 concept의 최근 사용에서 오는 약한 spillover",
            Self::UserRecallStatement => "사용자 직접 “지금도 바로 사용할 수 있음/복습 필요” 확인",
            Self::RetentionPriorAndCalibration => {
                "concept별 retention profile과 사용자별 경험적 보정"
            }
            Self::ContraryEvidence => {
                "반대 evidence: 설명 실패, 기억 안 남음 표시, 재학습 필요 event"
            }
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::LastStrongEvidence => "LAST_STRONG_EVIDENCE",
            Self::RepetitionAndInterval => "REPETITION_AND_INTERVAL",
            Self::EvidenceTypePersistence => "EVIDENCE_TYPE_PERSISTENCE",
            Self::RelatedConceptSpillover => "RELATED_CONCEPT_SPILLOVER",
            Self::UserRecallStatement => "USER_RECALL_STATEMENT",
            Self::RetentionPriorAndCalibration => "RETENTION_PRIOR_AND_CALIBRATION",
            Self::ContraryEvidence => "CONTRARY_EVIDENCE",
        }
    }
}
