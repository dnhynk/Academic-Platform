//! `ExpectedConceptClaim`: what an extraction produces, and what it never is.
//!
//! ## Candidate is the type, not a flag on it
//!
//! Section 27.4's low-risk row is `public syllabus의 topic 후보는 자동
//! 저장하되 AI_INFERRED 표시`, and section 27.2 says outright that AI does not
//! `개념 이해·질문 해결을 사용자 대신 확정`. So the claim's standing is
//! `P2-C1`'s own [`EpistemicStatus::AiInferred`] and there is no parameter,
//! setter, builder or second constructor by which any other value could be
//! reached: [`ExpectedConceptClaim::extract`] does not take a status.
//!
//! That the constant is right is a check somebody could delete. What is not is
//! the shape around it, and `an_extracted_claim_is_never_confirmed` reads three
//! whole sets rather than a list of names:
//!
//! * every public signature of this crate that mentions `EpistemicStatus` —
//!   there is one, and it is the accessor;
//! * every `EpistemicStatus` variant any product file here spells — there is
//!   one, and it is `AiInferred`;
//! * every public signature's return type, required to name no
//!   `academic_knowledge_state` type at all, so no function here produces the
//!   evidence a mastery promotion is read from.
//!
//! ## The material is outside text, so it arrives through `P2-G5`
//!
//! `extract` takes a [`Proposal`], which only `adjudicate` produces, and copies
//! its [`ResolvedSpan`]s. It never sees one byte of the material:
//! `Untrusted::expose` is `pub(crate)` in that crate, so a claim carries a
//! source identity, a byte range and a digest and cannot carry the words. What
//! it does check is that the claim's declared material is one the model actually
//! quoted — `ClaimDoesNotQuoteItsMaterial` — so a claim labelled `SYLLABUS`
//! whose spans all point into a README is a value that cannot be built.

use academic_domain::{ConfidencePermille, EntityId, EpistemicStatus, entity_registry::EntityKind};
use academic_gap::gap_bearing;
use academic_untrusted_content::{Proposal, ResolvedSpan};

use crate::{NextLectureError, source::MaterialReference};

/// One expected concept, as extracted from one material.
///
/// Private fields, one constructor, and the constructor runs every refusal
/// before it returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedConceptClaim {
    concept: EntityId,
    concept_kind: EntityKind,
    material: MaterialReference,
    citations: Vec<ResolvedSpan>,
    confidence: ConfidencePermille,
}

impl ExpectedConceptClaim {
    /// The one standing an extracted claim has.
    ///
    /// Section 27.4's low-risk row. It is an associated constant rather than a
    /// constructor parameter, which is the whole of the candidate contract:
    /// there is no argument a caller could pass to get another value.
    pub const STANDING: EpistemicStatus = EpistemicStatus::AiInferred;

    /// Extracts one claim from one adjudicated proposal.
    ///
    /// # Errors
    ///
    /// [`NextLectureError::ExpectedConceptCarriesNoPrerequisite`] when the
    /// named tier carries no independent prerequisite of its own — `P2-C3`'s
    /// own sentence, so a claim naming a whole `FIELD` is refused where it is
    /// made rather than where it is proposed;
    /// and [`NextLectureError::ClaimDoesNotQuoteItsMaterial`] when none of the
    /// proposal's spans points into the document the material names — which is
    /// also the answer for a proposal that cites nothing at all, because a
    /// citation set that is empty for either reason leaves the claim resting on
    /// no material. There is no second refusal for the empty case: `P2-G5`'s
    /// schema requires a `support` line, so `adjudicate` never produces a
    /// proposal with none, and a branch no input reaches is what `P2-R5`
    /// measured as a suite that cannot see a real defect.
    pub fn extract(
        concept: EntityId,
        concept_kind: EntityKind,
        material: MaterialReference,
        proposal: &Proposal,
        confidence: ConfidencePermille,
    ) -> Result<Self, NextLectureError> {
        if !gap_bearing(concept_kind) {
            return Err(NextLectureError::ExpectedConceptCarriesNoPrerequisite {
                kind: concept_kind,
            });
        }
        let citations: Vec<ResolvedSpan> = proposal
            .support()
            .iter()
            .filter(|span| span.source_id() == material.document())
            .cloned()
            .collect();
        if citations.is_empty() {
            return Err(NextLectureError::ClaimDoesNotQuoteItsMaterial {
                place: material.source(),
            });
        }
        Ok(Self {
            concept,
            concept_kind,
            material,
            citations,
            confidence,
        })
    }

    /// Which concept tomorrow's lecture is expected to use.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Its `P2-C3` tier.
    #[must_use]
    pub const fn concept_kind(&self) -> EntityKind {
        self.concept_kind
    }

    /// Which of section 12.7's seven places it came from, and when from.
    #[must_use]
    pub const fn material(&self) -> &MaterialReference {
        &self.material
    }

    /// Every span of the declared material the model cited, in the proposal's
    /// own order.
    #[must_use]
    pub fn citations(&self) -> &[ResolvedSpan] {
        &self.citations
    }

    /// How sure the extraction is. One axis of three; see
    /// [`crate::uncertainty`].
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// Section 27.4's `AI_INFERRED`, and never anything else.
    #[must_use]
    pub const fn standing(&self) -> EpistemicStatus {
        Self::STANDING
    }
}
