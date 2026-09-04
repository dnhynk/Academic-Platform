//! The append-only history, and what a retraction does to it.
//!
//! Section 13.2's closing paragraph:
//!
//! > 반대로 제출한 과제가 타인의 풀이를 복사한 것이라면 evidence를 철회할 수
//! > 있다. 철회 event도 역사에 남고 projection만 다시 계산한다.
//!
//! Two clauses and they are two different mechanisms here. `철회 event도 역사에
//! 남고` is [`HistoryEntry::Retracted`], appended and never removed.
//! `projection만 다시 계산한다` is the assertion that follows it: a **new
//! version** computed over the surviving evidence, with the retracted item
//! absent from its supporting list and the earlier version still readable at
//! its own identity.
//!
//! ## A retraction reason is not a new vocabulary
//!
//! A retraction says which of section 13.4's four eligibility checks the
//! evidence turned out to fail — `타인의 풀이를 복사한 것` is
//! [`crate::eligibility::EligibilityCheck::AuthorshipOrParticipation`],
//! answered again and answered differently. There is no second reason
//! enumeration, because a retraction is exactly the admission decision being
//! revisited.
//!
//! ## Nothing here takes `&mut self`
//!
//! Every method that changes what the history holds consumes it and returns a
//! new one, so `CONTRIBUTING.md` rule 2's *append-only, a correction is a new
//! event* is the shape of the API and not a convention a caller must follow.
//! `no_public_function_mutates_in_place` holds that over the whole package.

use academic_domain::{ConfidencePermille, EntityId, EvidenceId, FreshnessBand, TimestampMillis};

use crate::{
    KnowledgeStateError,
    assertion::{AssertionId, KnowledgeStateAssertion},
    confirmation::{AdjustmentDirection, AiProposal, UserConfirmation},
    conflict::KnowledgeStateConflict,
    eligibility::{BlockedEvidence, EligibilityCheck, EligibleEvidence},
    evidence::BroadSignal,
    ladder::FacetProfile,
    projection::{MasteryProjection, project},
};

/// Refuses evidence that is about some other concept.
///
/// Section 13.4's first check answers *which* concept an item is linked to, and
/// `EligibleEvidence` carries that answer. Nothing below this line re-reads it,
/// so without this a history for one concept could be projected out of another
/// concept's admitted evidence — the exact misattribution the first check
/// exists to prevent, one layer up from where it is asked.
fn require_about(
    concept: EntityId,
    evidence: &[EligibleEvidence],
) -> Result<(), KnowledgeStateError> {
    if evidence.iter().any(|item| item.concept() != concept) {
        return Err(KnowledgeStateError::EvidenceNamesAnotherConcept);
    }
    Ok(())
}

/// One evidence item withdrawn, and which check it failed on review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceRetraction {
    evidence_id: EvidenceId,
    failed_check: EligibilityCheck,
    retracted_at: TimestampMillis,
}

impl EvidenceRetraction {
    /// Records a withdrawal.
    #[must_use]
    pub const fn of(
        evidence_id: EvidenceId,
        failed_check: EligibilityCheck,
        retracted_at: TimestampMillis,
    ) -> Self {
        Self {
            evidence_id,
            failed_check,
            retracted_at,
        }
    }

    /// Which evidence item.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    /// Which of section 13.4's checks it failed on review.
    #[must_use]
    pub const fn failed_check(&self) -> EligibilityCheck {
        self.failed_check
    }

    /// When.
    #[must_use]
    pub const fn retracted_at(&self) -> TimestampMillis {
        self.retracted_at
    }
}

/// One row of the append-only history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryEntry {
    /// A new assertion version.
    Asserted(KnowledgeStateAssertion),
    /// An evidence item withdrawn.
    Retracted(EvidenceRetraction),
    /// A model proposal that a user-confirmed state refused.
    Conflicted(KnowledgeStateConflict),
}

/// What happened to a model proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalOutcome {
    /// The state was not user-confirmed, so a new version was appended.
    Superseded(AssertionId),
    /// The state was user-confirmed and the proposal moved it, so a card was
    /// opened and nothing was rewritten.
    Conflict(Box<KnowledgeStateConflict>),
    /// The state was user-confirmed and the proposal named the level it already
    /// holds, so there was nothing to adjust and nothing to show.
    NoAdjustment,
}

/// A history and the outcome of the step that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalApplication {
    history: KnowledgeStateHistory,
    outcome: ProposalOutcome,
}

impl ProposalApplication {
    /// The history after the step.
    #[must_use]
    pub const fn history(&self) -> &KnowledgeStateHistory {
        &self.history
    }

    /// The history after the step, by value.
    #[must_use]
    pub fn into_history(self) -> KnowledgeStateHistory {
        self.history
    }

    /// What happened.
    #[must_use]
    pub const fn outcome(&self) -> &ProposalOutcome {
        &self.outcome
    }
}

/// The freshness half of an assertion, which `P2-N3` computes and this task
/// only carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessInput {
    band: FreshnessBand,
    confidence: ConfidencePermille,
}

impl FreshnessInput {
    /// Carries `P2-N3`'s band and its confidence.
    #[must_use]
    pub const fn of(band: FreshnessBand, confidence: ConfidencePermille) -> Self {
        Self { band, confidence }
    }

    /// The band.
    #[must_use]
    pub const fn band(&self) -> FreshnessBand {
        self.band
    }

    /// The confidence in it.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }
}

/// One concept's assertions, evidence and retractions, in acceptance order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeStateHistory {
    concept: EntityId,
    admitted: Vec<EligibleEvidence>,
    blocked: Vec<BlockedEvidence>,
    retracted: Vec<EvidenceRetraction>,
    broad_signals: Vec<BroadSignal>,
    facets: FacetProfile,
    entries: Vec<HistoryEntry>,
}

impl KnowledgeStateHistory {
    /// Opens a history with version one.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::EvidenceNamesAnotherConcept`] when an admitted
    /// item is linked to some other concept, and whatever [`project`] and
    /// [`KnowledgeStateAssertion::open`] refuse.
    pub fn open(
        concept: EntityId,
        admitted: Vec<EligibleEvidence>,
        blocked: Vec<BlockedEvidence>,
        broad_signals: Vec<BroadSignal>,
        facets: FacetProfile,
        freshness: FreshnessInput,
        as_of: TimestampMillis,
    ) -> Result<Self, KnowledgeStateError> {
        require_about(concept, &admitted)?;
        let projection = project(&admitted, &blocked)?;
        let assertion = KnowledgeStateAssertion::open(
            concept,
            as_of,
            &projection,
            facets,
            freshness.band(),
            freshness.confidence(),
            broad_signals.clone(),
        )?;
        Ok(Self {
            concept,
            admitted,
            blocked,
            retracted: Vec::new(),
            broad_signals,
            facets,
            entries: vec![HistoryEntry::Asserted(assertion)],
        })
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Every row, in the order it was appended.
    #[must_use]
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Every retraction, in the order it was appended.
    #[must_use]
    pub fn retractions(&self) -> &[EvidenceRetraction] {
        &self.retracted
    }

    /// Every assertion version, oldest first.
    #[must_use]
    pub fn versions(&self) -> Vec<&KnowledgeStateAssertion> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                HistoryEntry::Asserted(assertion) => Some(assertion),
                HistoryEntry::Retracted(_) | HistoryEntry::Conflicted(_) => None,
            })
            .collect()
    }

    /// The version in force now.
    #[must_use]
    pub fn current(&self) -> Option<&KnowledgeStateAssertion> {
        self.versions().into_iter().next_back()
    }

    /// One version by its identity, which is how an earlier projection is read
    /// back after a later one exists.
    #[must_use]
    pub fn version_at(&self, id: AssertionId) -> Option<&KnowledgeStateAssertion> {
        self.versions()
            .into_iter()
            .find(|assertion| assertion.id() == id)
    }

    /// Every conflict card opened, in the order it was appended.
    #[must_use]
    pub fn conflicts(&self) -> Vec<&KnowledgeStateConflict> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                HistoryEntry::Conflicted(card) => Some(card),
                HistoryEntry::Asserted(_) | HistoryEntry::Retracted(_) => None,
            })
            .collect()
    }

    /// The evidence still standing, which is what the current projection reads.
    #[must_use]
    pub fn surviving_evidence(&self) -> Vec<&EligibleEvidence> {
        self.admitted
            .iter()
            .filter(|item| {
                !self
                    .retracted
                    .iter()
                    .any(|entry| entry.evidence_id() == item.evidence_id())
            })
            .collect()
    }

    fn surviving_owned(&self) -> Vec<EligibleEvidence> {
        self.surviving_evidence().into_iter().cloned().collect()
    }

    /// Projects the current state over the surviving evidence.
    ///
    /// # Errors
    ///
    /// Whatever [`project`] refuses.
    pub fn projection(&self) -> Result<MasteryProjection, KnowledgeStateError> {
        project(&self.surviving_owned(), &self.blocked)
    }

    /// Withdraws one evidence item and appends the recomputed version.
    ///
    /// The retraction row and every earlier assertion stay in the history; only
    /// the projection is recomputed, which is section 13.2's own `철회 event도
    /// 역사에 남고 projection만 다시 계산한다`.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::RetractionNamesUnknownEvidence`] when no admitted
    /// item carries that identity, and whatever [`project`] and
    /// [`KnowledgeStateAssertion::revise`] refuse.
    pub fn retract(
        mut self,
        retraction: EvidenceRetraction,
        freshness: FreshnessInput,
        as_of: TimestampMillis,
    ) -> Result<Self, KnowledgeStateError> {
        let known = self
            .admitted
            .iter()
            .any(|item| item.evidence_id() == retraction.evidence_id());
        if !known {
            return Err(KnowledgeStateError::RetractionNamesUnknownEvidence);
        }
        let Some(previous) = self.current().cloned() else {
            return Err(KnowledgeStateError::HistoryHasNoAssertion);
        };
        self.entries.push(HistoryEntry::Retracted(retraction));
        self.retracted.push(retraction);
        let projection = project(&self.surviving_owned(), &self.blocked)?;
        let next = previous.revise(
            as_of,
            &projection,
            self.facets,
            freshness.band(),
            freshness.confidence(),
            self.broad_signals.clone(),
        )?;
        self.entries.push(HistoryEntry::Asserted(next));
        Ok(self)
    }

    /// Records the user's own confirmation of the current level.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::HistoryHasNoAssertion`] when there is no version
    /// to confirm, and whatever
    /// [`KnowledgeStateAssertion::confirmed`] refuses.
    pub fn confirm(
        mut self,
        confirmation: &UserConfirmation,
        freshness: FreshnessInput,
        as_of: TimestampMillis,
    ) -> Result<Self, KnowledgeStateError> {
        let Some(previous) = self.current().cloned() else {
            return Err(KnowledgeStateError::HistoryHasNoAssertion);
        };
        let projection = project(&self.surviving_owned(), &self.blocked)?;
        let next = previous.confirmed(
            as_of,
            &projection,
            self.facets,
            freshness.band(),
            freshness.confidence(),
            confirmation,
        )?;
        self.entries.push(HistoryEntry::Asserted(next));
        Ok(self)
    }

    /// Records a `FLUENT` promotion the user authorized.
    ///
    /// The authorization is taken **by value** and is consumed here, so one
    /// authorization produces one promotion.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::HistoryHasNoAssertion`] when there is no version,
    /// and whatever [`MasteryProjection::with_fluency`] and
    /// [`KnowledgeStateAssertion::confirmed`] refuse.
    pub fn promote_to_fluent(
        mut self,
        authorization: crate::confirmation::FluentAuthorization,
        confirmation: &UserConfirmation,
        freshness: FreshnessInput,
        as_of: TimestampMillis,
    ) -> Result<Self, KnowledgeStateError> {
        let Some(previous) = self.current().cloned() else {
            return Err(KnowledgeStateError::HistoryHasNoAssertion);
        };
        let projection = project(&self.surviving_owned(), &self.blocked)?
            .with_fluency(authorization, self.concept)?;
        let next = previous.confirmed(
            as_of,
            &projection,
            self.facets,
            freshness.band(),
            freshness.confidence(),
            confirmation,
        )?;
        self.entries.push(HistoryEntry::Asserted(next));
        Ok(self)
    }

    /// Applies a model proposal.
    ///
    /// A user-confirmed state is immune in both directions: a proposal that
    /// would raise or lower it opens a [`KnowledgeStateConflict`] and rewrites
    /// nothing. An unconfirmed state accepts the proposal's admitted evidence
    /// as a **new version**, which the user still has to accept, edit, leave
    /// unconfirmed or reject.
    ///
    /// # Errors
    ///
    /// [`KnowledgeStateError::HistoryHasNoAssertion`] when there is no version,
    /// [`KnowledgeStateError::ProposalNamesAnotherConcept`] when the proposal is
    /// about a different concept,
    /// [`KnowledgeStateError::EvidenceNamesAnotherConcept`] when one of its
    /// admitted items is, and whatever [`project`] refuses.
    pub fn propose(
        mut self,
        proposal: AiProposal,
        admitted: Vec<EligibleEvidence>,
        blocked: Vec<BlockedEvidence>,
        freshness: FreshnessInput,
        as_of: TimestampMillis,
    ) -> Result<ProposalApplication, KnowledgeStateError> {
        if proposal.concept() != self.concept {
            return Err(KnowledgeStateError::ProposalNamesAnotherConcept);
        }
        require_about(self.concept, &admitted)?;
        let Some(previous) = self.current().cloned() else {
            return Err(KnowledgeStateError::HistoryHasNoAssertion);
        };
        if previous.user_confirmed() {
            let direction =
                AdjustmentDirection::between(previous.mastery_level(), proposal.proposed());
            let outcome = match direction {
                AdjustmentDirection::Raise | AdjustmentDirection::Lower => {
                    let card =
                        KnowledgeStateConflict::seal(self.concept, direction, previous, proposal);
                    self.entries.push(HistoryEntry::Conflicted(card.clone()));
                    ProposalOutcome::Conflict(Box::new(card))
                }
                AdjustmentDirection::Unchanged => ProposalOutcome::NoAdjustment,
            };
            return Ok(ProposalApplication {
                history: self,
                outcome,
            });
        }
        self.admitted.extend(admitted);
        self.blocked.extend(blocked);
        let projection = project(&self.surviving_owned(), &self.blocked)?;
        let next = previous.revise(
            as_of,
            &projection,
            self.facets,
            freshness.band(),
            freshness.confidence(),
            self.broad_signals.clone(),
        )?;
        let id = next.id();
        self.entries.push(HistoryEntry::Asserted(next));
        Ok(ProposalApplication {
            history: self,
            outcome: ProposalOutcome::Superseded(id),
        })
    }
}
