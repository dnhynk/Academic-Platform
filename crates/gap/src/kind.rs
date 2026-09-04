//! Section 15.2's five gap kinds and the four dimensions step 3 overlays.
//!
//! Neither count is a number this file chose. `five_gap_types_route_correctly`
//! reads section 15.2's own table back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares its rows
//! against [`GAP_KINDS`] in both directions, and
//! `four_state_dimensions_are_overlaid` does the same for step 3's sentence
//! against [`STATE_DIMENSIONS`].
//!
//! ## Step 6 names four and the table names five
//!
//! Section 15.2's sixth step reads `hard gap, refresh gap, evidence gap,
//! terminology mismatch를 구분한다` — four informal names, and the table
//! immediately below it has five rows. The four map onto `MASTERY_GAP`,
//! `FRESHNESS_GAP`, `EVIDENCE_GAP` and `ONTOLOGY_GAP`; `CONTEXT_GAP` appears in
//! the table and in no prose sentence.
//!
//! The table is the normative enumeration because it is the half that fixes the
//! identifiers, and `P2-N5`'s acceptance evidence is named
//! `five_gap_types_route_correctly`. So this file has five kinds, and
//! [`STEP_SIX_INFORMAL_NAMES`] holds step 6's four so the mismatch is a
//! measured value with a test on it rather than a discrepancy a later reader
//! rediscovers. `docs/contracts/gap-engine.md` records it.

use serde::{Deserialize, Serialize};

/// Section 15.2's five gap kinds, in the table's own row order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GapKind {
    /// `prerequisite 수행 evidence가 부족`.
    MasteryGap,
    /// `과거 mastery는 있으나 즉시 사용 불확실`.
    FreshnessGap,
    /// `실제로 알 수 있으나 시스템에 근거가 없음`.
    EvidenceGap,
    /// `synonym/granularity 오류로 잘못 분리됨`.
    OntologyGap,
    /// `목표나 구현 선택이 불명확해 prerequisite가 갈림`.
    ContextGap,
}

/// The five, in section 15.2's table order.
pub const GAP_KINDS: [GapKind; 5] = [
    GapKind::MasteryGap,
    GapKind::FreshnessGap,
    GapKind::EvidenceGap,
    GapKind::OntologyGap,
    GapKind::ContextGap,
];

/// Section 15.2 step 6's four informal names, in the order it writes them.
///
/// Kept because they are four and the table is five. See the module note.
pub const STEP_SIX_INFORMAL_NAMES: [&str; 4] = [
    "hard gap",
    "refresh gap",
    "evidence gap",
    "terminology mismatch",
];

impl GapKind {
    /// Stable spelling, identical to the table's first column.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MasteryGap => "MASTERY_GAP",
            Self::FreshnessGap => "FRESHNESS_GAP",
            Self::EvidenceGap => "EVIDENCE_GAP",
            Self::OntologyGap => "ONTOLOGY_GAP",
            Self::ContextGap => "CONTEXT_GAP",
        }
    }

    /// The table's `뜻` cell, verbatim.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::MasteryGap => "prerequisite 수행 evidence가 부족",
            Self::FreshnessGap => "과거 mastery는 있으나 즉시 사용 불확실",
            Self::EvidenceGap => "실제로 알 수 있으나 시스템에 근거가 없음",
            Self::OntologyGap => "synonym/granularity 오류로 잘못 분리됨",
            Self::ContextGap => "목표나 구현 선택이 불명확해 prerequisite가 갈림",
        }
    }

    /// The table's `예시 대응` cell, verbatim.
    ///
    /// It is the *shape* of the remediation the design document orders for this
    /// kind, and [`crate::explanation::MinimumRemediation`] is checked against
    /// it, so a `FRESHNESS_GAP` answered with `기초 설명·문제·실험` is a routing
    /// error a reader can see rather than a plausible-looking suggestion.
    #[must_use]
    pub const fn response(self) -> &'static str {
        match self {
            Self::MasteryGap => "기초 설명·문제·실험",
            Self::FreshnessGap => "짧은 retrieval/refresher",
            Self::EvidenceGap => "사용자 확인 또는 diagnostic",
            Self::OntologyGap => "merge/sense correction",
            Self::ContextGap => "선택지와 조건 명확화",
        }
    }
}

/// Section 15.2 step 3's four overlay dimensions, in the order it names them.
///
/// `사용자 mastery, freshness, confidence와 contradicting evidence를 overlay한다`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StateDimension {
    /// `P2-N2`'s ladder position.
    Mastery,
    /// `P2-N3`'s band.
    Freshness,
    /// `P2-N2`'s `estimateConfidence`, which is evidence sufficiency.
    Confidence,
    /// The eligible items that contradict the projection.
    ContradictingEvidence,
}

/// The four, in step 3's own order.
pub const STATE_DIMENSIONS: [StateDimension; 4] = [
    StateDimension::Mastery,
    StateDimension::Freshness,
    StateDimension::Confidence,
    StateDimension::ContradictingEvidence,
];

impl StateDimension {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mastery => "MASTERY",
            Self::Freshness => "FRESHNESS",
            Self::Confidence => "CONFIDENCE",
            Self::ContradictingEvidence => "CONTRADICTING_EVIDENCE",
        }
    }

    /// The words step 3 uses for this dimension, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::Mastery => "mastery",
            Self::Freshness => "freshness",
            Self::Confidence => "confidence",
            Self::ContradictingEvidence => "contradicting evidence",
        }
    }
}
