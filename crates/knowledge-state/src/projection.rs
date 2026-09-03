//! The automatic projection, its ceiling, and the two ways a state can be
//! `UNSEEN`.
//!
//! ## The ceiling is an output, not an internal
//!
//! Section 13.2's third column is a promise to the user, so [`MasteryProjection`]
//! carries a [`CeilingDisclosure`] naming the ceiling, the row that set it and
//! that row's own cell text. `course_attendance_only_ceiling_is_exposed` reads
//! both halves of its name: attendance-only evidence gives the ceiling
//! `EXPOSED`, and the ceiling is *exposed* — a reader is told what it is and
//! which row fixed it, rather than being told a level with no ceiling beside it.
//!
//! ## `UNSEEN` is not a failed test
//!
//! Section 13.1's first row: `evidence 없음이지 "모른다"는 시험 결과가 아님`.
//! Two states project as `UNSEEN` and they are **not the same value**:
//!
//! * [`UnseenBasis::NoEvidenceRecorded`] — nothing was recorded at all; and
//! * [`UnseenBasis::EvidenceRecordedWithoutPromotion`] — something was
//!   recorded and none of it licensed a promotion. A dependency that is merely
//!   installed is here. So is an exercise attempt that did not succeed, which
//!   is retained as contradicting evidence rather than as a verdict.
//!
//! A projection carries its basis, its supporting evidence and its
//! contradicting evidence, so *nothing observed* and *tried and did not
//! succeed* are distinguishable by a reader and by a test. If that distinction
//! collapses the product tells the user something about themselves that no
//! evidence supports, which is the failure `REQ-13-003` is written against.
//!
//! ## The automatic path cannot reach `FLUENT`
//!
//! [`project`] returns an [`AutomaticLevel`], which has five variants. Section
//! 13.2's sixth row has ceiling `Fluent candidate`, and its automatic
//! contribution here is `Applied` — the highest level the automatic type can
//! express — which is section 13.2's own `자동 상한은 안전한 기본값이다`.
//! `FLUENT` is added afterwards, and only by
//! [`MasteryProjection::with_fluency`], which takes a
//! [`crate::confirmation::FluentAuthorization`] by value.

use academic_domain::{ConfidencePermille, EvidenceId, MasteryLevel};
use serde::{Deserialize, Serialize};

use crate::{
    confirmation::FluentAuthorization,
    eligibility::{BlockedEvidence, EligibilityCheck, EligibleEvidence},
    evidence::{CEILINGS, EvidenceCeiling, EvidenceKind},
    ladder::AutomaticLevel,
};

/// Section 13.1's own gloss on `UNSEEN`, verbatim.
///
/// Carried as the projection's copy so the sentence a user is shown is the
/// design document's own and not a paraphrase that could drift into a verdict.
pub const UNSEEN_MEANING: &str = "evidence 없음이지 \"모른다\"는 시험 결과가 아님";

/// Why a projection is `UNSEEN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnseenBasis {
    /// No eligible evidence at all.
    NoEvidenceRecorded,
    /// Evidence was recorded and none of it licensed a promotion.
    EvidenceRecordedWithoutPromotion,
}

impl UnseenBasis {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [
        Self::NoEvidenceRecorded,
        Self::EvidenceRecordedWithoutPromotion,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoEvidenceRecorded => "NO_EVIDENCE_RECORDED",
            Self::EvidenceRecordedWithoutPromotion => "EVIDENCE_RECORDED_WITHOUT_PROMOTION",
        }
    }
}

/// The ceiling in force, with the row that fixed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CeilingDisclosure {
    ceiling: EvidenceCeiling,
    from: Option<EvidenceKind>,
    cell: &'static str,
}

impl CeilingDisclosure {
    /// The ceiling.
    #[must_use]
    pub const fn ceiling(self) -> EvidenceCeiling {
        self.ceiling
    }

    /// Which of section 13.2's rows fixed it, when any evidence did.
    #[must_use]
    pub const fn from(self) -> Option<EvidenceKind> {
        self.from
    }

    /// That row's `자동 상한` cell, verbatim.
    #[must_use]
    pub const fn cell(self) -> &'static str {
        self.cell
    }
}

/// Why the evidence supporting a projection is less than sufficient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SufficiencyGap {
    /// A candidate was blocked because its concept link was not exact.
    ConceptLinkUnresolved,
    /// A candidate was blocked because authorship or participation was unclear.
    AuthorshipUnresolved,
    /// A candidate was blocked because its outcome was unclear.
    OutcomeUnresolved,
    /// A candidate was blocked because its source could not be verified.
    SourceIntegrityUnresolved,
    /// Fewer than two eligible items support the projection.
    SingleSupportingItem,
    /// At least one eligible item contradicts it.
    Contradicted,
}

impl SufficiencyGap {
    /// Exhaustive order.
    pub const ALL: [Self; 6] = [
        Self::ConceptLinkUnresolved,
        Self::AuthorshipUnresolved,
        Self::OutcomeUnresolved,
        Self::SourceIntegrityUnresolved,
        Self::SingleSupportingItem,
        Self::Contradicted,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConceptLinkUnresolved => "CONCEPT_LINK_UNRESOLVED",
            Self::AuthorshipUnresolved => "AUTHORSHIP_UNRESOLVED",
            Self::OutcomeUnresolved => "OUTCOME_UNRESOLVED",
            Self::SourceIntegrityUnresolved => "SOURCE_INTEGRITY_UNRESOLVED",
            Self::SingleSupportingItem => "SINGLE_SUPPORTING_ITEM",
            Self::Contradicted => "CONTRADICTED",
        }
    }

    /// The permille this gap subtracts.
    #[must_use]
    pub const fn deduction(self) -> u16 {
        match self {
            // Section 13.1: `"mastery 4, confidence 0.45"는 applied evidence
            // 후보가 있지만 authorship이나 수행 결과가 불명확함을 뜻한다`. Two
            // unresolved eligibility checks and otherwise sufficient evidence
            // therefore lands on 450, which is that sentence's own number.
            Self::ConceptLinkUnresolved
            | Self::AuthorshipUnresolved
            | Self::OutcomeUnresolved
            | Self::SourceIntegrityUnresolved => 275,
            Self::SingleSupportingItem => 150,
            Self::Contradicted => 200,
        }
    }

    const fn of_check(check: EligibilityCheck) -> Self {
        match check {
            EligibilityCheck::ExactConceptLink => Self::ConceptLinkUnresolved,
            EligibilityCheck::AuthorshipOrParticipation => Self::AuthorshipUnresolved,
            EligibilityCheck::Outcome => Self::OutcomeUnresolved,
            EligibilityCheck::SourceIntegrity => Self::SourceIntegrityUnresolved,
        }
    }
}

/// Section 13.1's `estimateConfidence`, under the name of what it measures.
///
/// **It is not a skill score.** Section 13.1: `사용자의 실력 점수가 아니다.
/// 현재 mastery projection을 뒷받침하는 evidence의 충분성·일관성에 대한 시스템
/// 확신이다.` The name in the schema is fixed by the design document; the type
/// is what keeps the meaning, and it does three things a score would not:
///
/// * it is not `PartialOrd` and not `Ord`, so two users' or two concepts'
///   values cannot be ranked against each other by the type;
/// * there is no conversion in either direction between it and
///   `academic_domain::MasteryLevel`; and
/// * it carries [`SufficiencyGap`]s, so a low value always says *what is
///   missing* rather than *how good the user is*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSufficiency {
    permille: ConfidencePermille,
    gaps: Vec<SufficiencyGap>,
}

impl EvidenceSufficiency {
    /// Assesses the evidence behind one projection.
    ///
    /// Deterministic: 1000 permille less each gap's [`SufficiencyGap::deduction`],
    /// clamped at zero. A blocked candidate contributes one gap per distinct
    /// failing check, not one per blocked item, so the same missing answer on
    /// ten candidates is one gap.
    ///
    /// # Errors
    ///
    /// [`crate::KnowledgeStateError::Domain`] if the computed permille leaves
    /// `0..=1000`. The arithmetic above saturates, so it does not.
    pub fn assess(
        supporting: &[&EligibleEvidence],
        blocked: &[&BlockedEvidence],
        contradicting: &[&EligibleEvidence],
    ) -> Result<Self, crate::KnowledgeStateError> {
        let mut gaps: Vec<SufficiencyGap> = Vec::new();
        for check in EligibilityCheck::ALL {
            let present = blocked
                .iter()
                .any(|item| item.failed_checks().contains(&check));
            if present {
                gaps.push(SufficiencyGap::of_check(check));
            }
        }
        if supporting.len() < 2 {
            gaps.push(SufficiencyGap::SingleSupportingItem);
        }
        if !contradicting.is_empty() {
            gaps.push(SufficiencyGap::Contradicted);
        }
        let deducted: u32 = gaps.iter().map(|gap| u32::from(gap.deduction())).sum();
        let permille = u16::try_from(1000_u32.saturating_sub(deducted)).unwrap_or(0);
        Ok(Self {
            permille: ConfidencePermille::new(permille)?,
            gaps,
        })
    }

    /// The permille value.
    ///
    /// Deliberately not named `score`, and deliberately the only numeric
    /// accessor.
    #[must_use]
    pub const fn permille(&self) -> ConfidencePermille {
        self.permille
    }

    /// What is missing, in [`SufficiencyGap::ALL`] order.
    #[must_use]
    pub fn gaps(&self) -> &[SufficiencyGap] {
        &self.gaps
    }
}

/// The automatic contribution of one of section 13.2's rows.
///
/// Total over [`EvidenceKind`] with no wildcard arm. Row six's ceiling is
/// `Fluent candidate` and [`AutomaticLevel`] has no `Fluent`; its contribution
/// is `Applied`, which is section 13.2's own conservative default.
#[must_use]
pub const fn automatic_contribution(kind: EvidenceKind) -> AutomaticLevel {
    match kind {
        EvidenceKind::MeaningfulTeaching => AutomaticLevel::Exposed,
        EvidenceKind::SelfExplanationConfirmed => AutomaticLevel::Understood,
        EvidenceKind::ConceptSpecificExercise => AutomaticLevel::Practiced,
        EvidenceKind::AuthoredProjectCode
        | EvidenceKind::IncidentDebugging
        | EvidenceKind::RepeatedIndependentTransfer => AutomaticLevel::Applied,
        EvidenceKind::DependencyPresenceOnly | EvidenceKind::CourseGrade => AutomaticLevel::Unseen,
    }
}

fn ceiling_cell(kind: EvidenceKind) -> &'static str {
    CEILINGS
        .iter()
        .find(|row| row.kind == kind)
        .map_or("", |row| row.ceiling_cell)
}

/// One concept's projected mastery, its ceiling and its evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasteryProjection {
    level: MasteryLevel,
    automatic: AutomaticLevel,
    disclosure: CeilingDisclosure,
    unseen_basis: Option<UnseenBasis>,
    supporting: Vec<EvidenceId>,
    contradicting: Vec<EvidenceId>,
    sufficiency: EvidenceSufficiency,
    fluency_contexts: Option<usize>,
}

impl MasteryProjection {
    /// The projected level.
    #[must_use]
    pub const fn level(&self) -> MasteryLevel {
        self.level
    }

    /// The automatic half of it, which can never be `FLUENT`.
    #[must_use]
    pub const fn automatic(&self) -> AutomaticLevel {
        self.automatic
    }

    /// The ceiling in force and the row that fixed it.
    #[must_use]
    pub const fn ceiling(&self) -> CeilingDisclosure {
        self.disclosure
    }

    /// Why the projection is `UNSEEN`, when it is.
    #[must_use]
    pub const fn unseen_basis(&self) -> Option<UnseenBasis> {
        self.unseen_basis
    }

    /// Section 13.1's own gloss, when the projection is `UNSEEN`.
    #[must_use]
    pub const fn unseen_meaning(&self) -> Option<&'static str> {
        match self.unseen_basis {
            Some(_) => Some(UNSEEN_MEANING),
            None => None,
        }
    }

    /// The eligible evidence supporting it.
    #[must_use]
    pub fn supporting(&self) -> &[EvidenceId] {
        &self.supporting
    }

    /// The eligible evidence contradicting it.
    #[must_use]
    pub fn contradicting(&self) -> &[EvidenceId] {
        &self.contradicting
    }

    /// How sufficient the supporting evidence is. Not a skill score; see
    /// [`EvidenceSufficiency`].
    #[must_use]
    pub const fn sufficiency(&self) -> &EvidenceSufficiency {
        &self.sufficiency
    }

    /// How many distinct independent contexts authorized `FLUENT`, when it was
    /// authorized.
    #[must_use]
    pub const fn fluency_contexts(&self) -> Option<usize> {
        self.fluency_contexts
    }

    /// Raises the projection to `FLUENT`, as a new value.
    ///
    /// The one route to `MasteryLevel::Fluent` in this crate. It takes the
    /// authorization **by value**, so an authorization cannot be built once and
    /// applied twice, and it refuses an authorization built for another
    /// concept's confirmation by requiring the caller to pass the concept the
    /// projection is about.
    ///
    /// # Errors
    ///
    /// [`crate::KnowledgeStateError::ConfirmationSubjectMismatch`] when the
    /// authorization names another concept.
    pub fn with_fluency(
        &self,
        authorization: FluentAuthorization,
        concept: academic_domain::EntityId,
    ) -> Result<Self, crate::KnowledgeStateError> {
        if authorization.concept() != concept {
            return Err(crate::KnowledgeStateError::ConfirmationSubjectMismatch);
        }
        Ok(Self {
            level: MasteryLevel::Fluent,
            automatic: self.automatic,
            disclosure: self.disclosure,
            unseen_basis: None,
            supporting: self.supporting.clone(),
            contradicting: self.contradicting.clone(),
            sufficiency: self.sufficiency.clone(),
            fluency_contexts: Some(authorization.distinct_contexts()),
        })
    }
}

/// Projects one concept's mastery from eligible evidence.
///
/// `blocked` is not evidence for anything and is never promoted; it is read
/// only to say what is missing, which is what [`EvidenceSufficiency`] is.
///
/// # Errors
///
/// [`crate::KnowledgeStateError::Domain`] if the sufficiency permille leaves
/// `0..=1000`; the arithmetic saturates, so it does not.
pub fn project(
    evidence: &[EligibleEvidence],
    blocked: &[BlockedEvidence],
) -> Result<MasteryProjection, crate::KnowledgeStateError> {
    let (contradicting, supporting): (Vec<&EligibleEvidence>, Vec<&EligibleEvidence>) = evidence
        .iter()
        .partition(|item| item.evidence().contradicts());

    let mut automatic = AutomaticLevel::Unseen;
    let mut ceiling = EvidenceCeiling::NoPromotion;
    let mut ceiling_from: Option<EvidenceKind> = None;
    for item in &supporting {
        let kind = item.kind();
        let contribution = automatic_contribution(kind);
        if contribution > automatic {
            automatic = contribution;
        }
        let row = kind.ceiling();
        let raises = match (ceiling, row) {
            (EvidenceCeiling::NoPromotion, EvidenceCeiling::UpTo(_)) => true,
            (EvidenceCeiling::UpTo(held), EvidenceCeiling::UpTo(offered)) => offered > held,
            (_, EvidenceCeiling::NoPromotion) => false,
        };
        if raises || ceiling_from.is_none() {
            if raises {
                ceiling = row;
            }
            if ceiling_from.is_none() || raises {
                ceiling_from = Some(kind);
            }
        }
    }

    let level = automatic.level();
    let unseen_basis = if level == MasteryLevel::Unseen {
        if evidence.is_empty() {
            Some(UnseenBasis::NoEvidenceRecorded)
        } else {
            Some(UnseenBasis::EvidenceRecordedWithoutPromotion)
        }
    } else {
        None
    };

    let blocked_refs: Vec<&BlockedEvidence> = blocked.iter().collect();
    Ok(MasteryProjection {
        level,
        automatic,
        disclosure: CeilingDisclosure {
            ceiling,
            from: ceiling_from,
            cell: ceiling_from.map_or("", ceiling_cell),
        },
        unseen_basis,
        supporting: supporting.iter().map(|item| item.evidence_id()).collect(),
        contradicting: contradicting.iter().map(|item| item.evidence_id()).collect(),
        sufficiency: EvidenceSufficiency::assess(&supporting, &blocked_refs, &contradicting)?,
        fluency_contexts: None,
    })
}
