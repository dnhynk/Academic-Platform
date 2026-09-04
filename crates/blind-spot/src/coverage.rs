//! Section 23's `존재와 다양성을 집계하되 mastery 점수로 바꾸지 않는다`.
//!
//! ## The count is not a number this file chose
//!
//! `coverage_never_becomes_mastery` reads section 23's own sentence —
//! `Field별 coverage는 … evidence의 존재와 다양성을 집계하되` — back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, splits the run of
//! names before `evidence의` on its own `·` separators and compares them against
//! [`EXPOSURE_SOURCES`] in both directions.
//!
//! ## Why coverage cannot become a mastery
//!
//! Not because a function declines to convert it. Because **this crate has no
//! name for a mastery level**: `academic-knowledge-state` is a product edge and
//! hands out `LADDER`, `rung`, `level_token`, `AutomaticLevel` and
//! `MasteryProjection`, `academic_domain::MasteryLevel` is one `use` away, and
//! nothing here reaches for any of them. `P2-N3` holds section 1's fifth
//! invariant the same way on the freshness axis.
//!
//! [`FieldCoverage`] derives neither `PartialOrd` nor `Ord`, so two fields'
//! coverage cannot be ranked by the type, and neither does [`EvidenceDiversity`].
//! There is no conversion in either direction with any level, and no method here
//! returns a score, a percentage or a weight.
//!
//! ## `존재` is admitted evidence, not anything a caller says is evidence
//!
//! An [`ExposureItem`] wraps an `EligibleEvidence` and has no other constructor,
//! so the items counted here are the items that passed section 13.4's four
//! checks. `P2-N3` dates the same value for the same reason: a second
//! admissibility rule here would be a second ladder.
//!
//! The *source* is the other axis and it does not come from section 13.2's row.
//! Section 13.2 says what a piece of evidence licenses; section 23's five say
//! where it came from, and one exercise can arrive from a lecture or from an
//! assignment. It arrives as an argument, the way section 7.4's tier arrives as
//! an argument in `P2-N2`.
//!
//! ## The diversity scale
//!
//! Section 23 exhibits exactly one token, `LOW`, beside
//! `exposureEvidenceCount: 1`. One item carries one source, so `LOW` holds at
//! one distinct source; [`EvidenceDiversity::Mixed`] is this crate's name for
//! its complement and the split point is measured against the example's own
//! count. `docs/contracts/blind-spot-detector.md` records that the second name
//! is not the design document's.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use academic_domain::{EntityId, EvidenceId, TimestampMillis};
use academic_knowledge_state::{EligibleEvidence, Outcome};

use crate::{BlindSpotError, resolution::FieldResolver, scope::BlindSpotScope};

/// Section 23's five exposure sources, in the order its sentence writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExposureSource {
    /// `강의`.
    Lecture,
    /// `과제`.
    Assignment,
    /// `project`.
    Project,
    /// `질문`.
    Question,
    /// `사용자 확인`.
    UserConfirmation,
}

/// The five, in section 23's own order.
pub const EXPOSURE_SOURCES: [ExposureSource; 5] = [
    ExposureSource::Lecture,
    ExposureSource::Assignment,
    ExposureSource::Project,
    ExposureSource::Question,
    ExposureSource::UserConfirmation,
];

impl ExposureSource {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lecture => "LECTURE",
            Self::Assignment => "ASSIGNMENT",
            Self::Project => "PROJECT",
            Self::Question => "QUESTION",
            Self::UserConfirmation => "USER_CONFIRMATION",
        }
    }

    /// The design document's own spelling for this source.
    #[must_use]
    pub const fn design_token(self) -> &'static str {
        match self {
            Self::Lecture => "강의",
            Self::Assignment => "과제",
            Self::Project => "project",
            Self::Question => "질문",
            Self::UserConfirmation => "사용자 확인",
        }
    }
}

/// One admitted item, counted as exposure.
///
/// Private fields and one constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposureItem {
    evidence: EligibleEvidence,
    source: ExposureSource,
    observed_at: TimestampMillis,
}

impl ExposureItem {
    /// Counts `evidence` as exposure from `source`, observed at `observed_at`.
    #[must_use]
    pub const fn of(
        evidence: EligibleEvidence,
        source: ExposureSource,
        observed_at: TimestampMillis,
    ) -> Self {
        Self {
            evidence,
            source,
            observed_at,
        }
    }

    /// The admitted evidence, unchanged.
    #[must_use]
    pub const fn evidence(&self) -> &EligibleEvidence {
        &self.evidence
    }

    /// Which entity it is about.
    #[must_use]
    pub fn concept(&self) -> EntityId {
        self.evidence.concept()
    }

    /// Which evidence item.
    #[must_use]
    pub fn evidence_id(&self) -> EvidenceId {
        self.evidence.evidence_id()
    }

    /// Which of section 23's five sources.
    #[must_use]
    pub const fn source(&self) -> ExposureSource {
        self.source
    }

    /// When it was observed.
    #[must_use]
    pub const fn observed_at(&self) -> TimestampMillis {
        self.observed_at
    }

    /// Whether `P2-N2` recorded the attempt as a failure.
    ///
    /// Section 23's `시도·평가 evidence에서 어려움이 관찰됨` read off `P2-N2`'s
    /// own outcome rather than off a second reading of difficulty. A `Failed`
    /// outcome is a *known* outcome there — it passes section 13.4's third check
    /// and promotes nothing — and that is exactly the observation this state
    /// wants.
    #[must_use]
    pub fn records_difficulty(&self) -> bool {
        self.evidence.outcome() == Outcome::Failed
    }
}

/// How diverse a field's exposure is.
///
/// Neither `PartialOrd` nor `Ord`: see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceDiversity {
    /// At most one of section 23's five sources. The example's own token.
    Low,
    /// More than one.
    Mixed,
}

impl EvidenceDiversity {
    /// Reads the diversity off the distinct sources present.
    #[must_use]
    pub const fn of_distinct_sources(distinct: usize) -> Self {
        if distinct > 1 { Self::Mixed } else { Self::Low }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Mixed => "MIXED",
        }
    }
}

/// One aggregation key's exposure, counted over the user's window.
///
/// Neither `PartialOrd` nor `Ord`, and no method here returns a level, a score,
/// a percentage or a weight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCoverage {
    key: EntityId,
    evidence_count: u32,
    by_source: BTreeMap<ExposureSource, u32>,
    failed_attempts: Vec<EvidenceId>,
    newest: Option<TimestampMillis>,
}

impl FieldCoverage {
    /// Counts the items that resolve to `key` under `scope`'s granularity and
    /// fall inside `scope`'s window.
    ///
    /// # Errors
    ///
    /// [`BlindSpotError::ItemIsAboutAnotherKey`] when an offered item's entity
    /// resolves to a different aggregation key, and
    /// [`BlindSpotError::ItemIsOutsideTheTaxonomy`] when it resolves to none —
    /// the release the user selected does not hold that entity, so counting it
    /// would attribute evidence to a field this taxonomy never named.
    pub fn of(
        key: EntityId,
        scope: &BlindSpotScope,
        resolver: &FieldResolver,
        items: &[ExposureItem],
    ) -> Result<Self, BlindSpotError> {
        let mut evidence_count = 0_u32;
        let mut by_source: BTreeMap<ExposureSource, u32> = BTreeMap::new();
        let mut failed_attempts = Vec::new();
        let mut newest: Option<TimestampMillis> = None;
        for item in items {
            let Some(resolved) = resolver.resolve(item.concept()) else {
                return Err(BlindSpotError::ItemIsOutsideTheTaxonomy(item.evidence_id()));
            };
            if resolved != key {
                return Err(BlindSpotError::ItemIsAboutAnotherKey {
                    expected: key,
                    found: resolved,
                });
            }
            if !scope.window().holds(item.observed_at()) {
                continue;
            }
            evidence_count = evidence_count.saturating_add(1);
            by_source
                .entry(item.source())
                .and_modify(|held| *held = held.saturating_add(1))
                .or_insert(1);
            if item.records_difficulty() {
                failed_attempts.push(item.evidence_id());
            }
            if newest.is_none_or(|held| held.value() < item.observed_at().value()) {
                newest = Some(item.observed_at());
            }
        }
        Ok(Self {
            key,
            evidence_count,
            by_source,
            failed_attempts,
            newest,
        })
    }

    /// Which aggregation key.
    #[must_use]
    pub const fn key(&self) -> EntityId {
        self.key
    }

    /// Section 23's `exposureEvidenceCount`.
    #[must_use]
    pub const fn evidence_count(&self) -> u32 {
        self.evidence_count
    }

    /// Which of section 23's five sources are present.
    #[must_use]
    pub fn sources(&self) -> BTreeSet<ExposureSource> {
        self.by_source.keys().copied().collect()
    }

    /// How many items each present source contributed.
    ///
    /// Section 23's `backend repo 세 개` is one entry of this map.
    #[must_use]
    pub const fn by_source(&self) -> &BTreeMap<ExposureSource, u32> {
        &self.by_source
    }

    /// Section 23's `evidenceDiversity`.
    #[must_use]
    pub fn diversity(&self) -> EvidenceDiversity {
        EvidenceDiversity::of_distinct_sources(self.by_source.len())
    }

    /// The admitted attempts `P2-N2` recorded as failures, in offer order.
    #[must_use]
    pub fn failed_attempts(&self) -> &[EvidenceId] {
        &self.failed_attempts
    }

    /// When the newest counted item was observed, if any was.
    #[must_use]
    pub const fn newest(&self) -> Option<TimestampMillis> {
        self.newest
    }
}
