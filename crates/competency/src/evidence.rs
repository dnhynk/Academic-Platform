//! What may found a filled rubric cell, and what may not.
//!
//! Two doors, and there is no third. Both of them are values another crate
//! produced under its own checks, so nothing in this crate turns a caller's
//! assertion into evidence.
//!
//! ## Door one: `P2-N2`'s admitted evidence, if its row promotes
//!
//! Section 24.3 opens with `dependency를 사용했다는 이유만으로 competency를
//! 채우지 않는다`, and section 13.2 already answered it: the
//! `dependency/install/import만 존재` row carries `EvidenceCeiling::NoPromotion`.
//! [`PromotingEvidence`] has one producer and it refuses that ceiling, so a
//! dependency declaration is a value that cannot become stage evidence at all.
//!
//! The refusal is read out of `P2-N2`'s own table rather than restated here.
//! This crate names no evidence row, no ceiling level and no mastery level; it
//! asks [`EvidenceKind::ceiling`] and believes the answer.
//!
//! ## Door two: `P2-R5`'s `User APPLIED Concept`
//!
//! `P2-R5` split section 17.6's two claims into two identities with two
//! provenances, and this crate takes the second one. **There is no arm for the
//! first.** [`EvidenceSource`] enumerates two origins and neither of them is a
//! `ProjectObservationClaim`, so `ProjectSnapshot OBSERVES Concept` has no
//! spelling here — which is what keeps section 17.6's separation from being
//! quietly rejoined one layer up.
//!
//! A claim that has been taken back founds nothing either: `P2-R5`'s
//! `ClaimStanding::Rejected` is refused by [`StageEvidence::of_personal_claim`].
//!
//! ## The concept comes from the foundation, never from the caller
//!
//! [`StageEvidence`] has no concept argument. It reads the concept out of the
//! value that founded it, so there is no way to record evidence about one
//! concept and file it under another. `P2-R5` found the same defect in the
//! opposite shape — a join that succeeded on a weaker key than the one it meant
//! to compare — and the repair in both places is that the weaker key does not
//! exist.
//!
//! ## It deserializes into nothing
//!
//! [`StageEvidence`] is `Serialize` and not `Deserialize`. A filled cell is a
//! derivation over evidence two crates below already froze; a constructor that
//! read one back out of JSON would be a third door that ran neither of the two
//! checks above.

use academic_domain::{EntityId, EvidenceId};
use academic_knowledge_state::{EligibleEvidence, EvidenceCeiling, EvidenceKind};
use academic_repository_competency::{ClaimStanding, PersonalApplicationClaim};
use serde::Serialize;

use crate::{
    CompetencyError,
    identity::{ConceptRef, RecordId},
    stage::EvidenceStage,
};

/// `P2-N2` evidence whose section 13.2 row licenses a promotion.
///
/// One producer, [`PromotingEvidence::of`], and it refuses
/// [`EvidenceCeiling::NoPromotion`]. Section 24.3's first sentence is therefore
/// a value that does not exist rather than a check somebody has to remember to
/// run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotingEvidence {
    inner: EligibleEvidence,
}

impl PromotingEvidence {
    /// Admits one piece of `P2-N2` evidence, if its row promotes anything.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::EvidenceLicensesNoPromotion`] when section 13.2 gives
    /// this row [`EvidenceCeiling::NoPromotion`], which is the case for
    /// `dependency/install/import만 존재` and for `과목 grade`.
    pub fn of(inner: EligibleEvidence) -> Result<Self, CompetencyError> {
        match inner.kind().ceiling() {
            EvidenceCeiling::NoPromotion => Err(CompetencyError::EvidenceLicensesNoPromotion(
                inner.kind().as_str(),
            )),
            EvidenceCeiling::UpTo(_) => Ok(Self { inner }),
        }
    }

    /// Which concept `P2-N2` linked it to, exactly.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.inner.concept()
    }

    /// Which of section 13.2's rows it is.
    #[must_use]
    pub const fn kind(&self) -> EvidenceKind {
        self.inner.kind()
    }

    /// Its own evidence identifier.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.inner.evidence_id()
    }

    /// The admitted evidence, unchanged.
    #[must_use]
    pub const fn admitted(&self) -> &EligibleEvidence {
        &self.inner
    }
}

/// Which boundary founded one piece of stage evidence.
///
/// Two origins, and no `ProjectObservationClaim` among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceOrigin {
    /// `P2-N2`'s admitted concept evidence.
    KnowledgeState,
    /// `P2-R5`'s `User APPLIED Concept`.
    PersonalApplication,
}

impl EvidenceOrigin {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::KnowledgeState, Self::PersonalApplication];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KnowledgeState => "KNOWLEDGE_STATE",
            Self::PersonalApplication => "PERSONAL_APPLICATION",
        }
    }
}

/// The founding value, by the identifier its own boundary gave it.
///
/// The identifier and not the value, for `P2-R5`'s own reason: a cell that
/// embedded a claim could be read as speaking for it, and a claim taken back
/// later would leave a copy of itself standing inside a sheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "origin", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceSource {
    /// `P2-N2`'s evidence identifier.
    KnowledgeState(EvidenceId),
    /// `P2-R5`'s claim identifier.
    PersonalApplication(String),
}

impl EvidenceSource {
    /// Which boundary it came from.
    #[must_use]
    pub const fn origin(&self) -> EvidenceOrigin {
        match self {
            Self::KnowledgeState(_) => EvidenceOrigin::KnowledgeState,
            Self::PersonalApplication(_) => EvidenceOrigin::PersonalApplication,
        }
    }
}

/// One performance, at one of section 24.3's stages, about one concept.
///
/// No public field and no constructor that takes a concept: both producers read
/// the concept out of the value that founded the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StageEvidence {
    id: RecordId,
    stage: EvidenceStage,
    concept: ConceptRef,
    source: EvidenceSource,
}

impl StageEvidence {
    /// Records one performance founded on `P2-N2`'s admitted evidence.
    ///
    /// The concept is [`PromotingEvidence::concept`], in the ontology
    /// namespace, because that is the namespace `P2-N2` resolved it in.
    #[must_use]
    pub fn of_knowledge_state(
        id: RecordId,
        stage: EvidenceStage,
        evidence: &PromotingEvidence,
    ) -> Self {
        Self {
            id,
            stage,
            concept: ConceptRef::ontology(evidence.concept()),
            source: EvidenceSource::KnowledgeState(evidence.evidence_id()),
        }
    }

    /// Records one performance founded on `P2-R5`'s personal claim.
    ///
    /// The concept is the claim's own, in the classification namespace, because
    /// that is the namespace `P2-R4` keyed it in. This crate does not resolve
    /// one namespace into the other; see the module documentation of
    /// [`crate::identity`].
    ///
    /// # Errors
    ///
    /// [`CompetencyError::ClaimIsRejected`] when the claim has been taken back,
    /// and [`CompetencyError::InvalidIdentifier`] when its concept token is not
    /// the shape `P2-R4` issues.
    pub fn of_personal_claim(
        id: RecordId,
        stage: EvidenceStage,
        claim: &PersonalApplicationClaim,
    ) -> Result<Self, CompetencyError> {
        if let ClaimStanding::Rejected { .. } = claim.standing() {
            return Err(CompetencyError::ClaimIsRejected(
                claim.id().as_str().to_owned(),
            ));
        }
        Ok(Self {
            id,
            stage,
            concept: ConceptRef::classification(claim.concept())?,
            source: EvidenceSource::PersonalApplication(claim.id().as_str().to_owned()),
        })
    }

    /// This record's identity.
    #[must_use]
    pub const fn id(&self) -> &RecordId {
        &self.id
    }

    /// Which of section 24.3's stages it is.
    #[must_use]
    pub const fn stage(&self) -> EvidenceStage {
        self.stage
    }

    /// Which concept it is about, in the namespace its foundation named.
    #[must_use]
    pub const fn concept(&self) -> &ConceptRef {
        &self.concept
    }

    /// Where a reader opens it.
    #[must_use]
    pub const fn source(&self) -> &EvidenceSource {
        &self.source
    }
}
