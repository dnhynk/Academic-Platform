//! Section 15.2 step 3: `사용자 mastery, freshness, confidence와 contradicting
//! evidence를 overlay한다`.
//!
//! ## Four dimensions and none of them is computed here
//!
//! `P2-N2` owns mastery and `estimateConfidence`, `P2-N3` owns the band. This
//! module reads all four onto one concept and its whole job is the sentence
//! that step 3 leaves implicit: **onto *one* concept.**
//!
//! ## The misattribution this task inherits
//!
//! `P2-N2` closed it at the history: `KnowledgeStateHistory` refuses admitted
//! evidence linked to another concept, because without that a history for one
//! concept could be projected out of another's. `P2-N3` closed the one-hop form:
//! `NeighborUse::direct` refuses a dated item linked to any concept but the
//! neighbour, and reported that the route surviving every other limit is one
//! concept's evidence crossing a real edge into a neighbour's reading.
//!
//! **`P2-N5`'s traversal crosses exactly those edges**, so both forms arrive
//! here and a third one with them.
//!
//! *One.* `academic_knowledge_state::project` takes a slice of
//! `EligibleEvidence` and **does not require the slice to be about one
//! concept**; the `MasteryProjection` it returns carries no concept at all, so
//! nothing downstream can recover which one it was about. A gap engine that
//! accepted a ready-made projection would have no way to check. So
//! [`ConceptState::overlay`] does not accept one: it takes the *inputs* to
//! section 13.4's four checks, runs `EligibilityOutcome::admit` itself, and
//! refuses any item whose resolved link names another concept — including a
//! blocked item, whose dossier still holds the link that
//! `academic_knowledge_state::BlockedEvidence` does not retain.
//!
//! *Two.* A `FreshnessProjection` does carry its concept, so it is checked; and
//! every `Spillover` carries the concept it was computed toward, so that is
//! checked too.
//!
//! *Three, and it is the one this task had to find.* Section 13.3's spillover is
//! licensed on `REQUIRES`, `BUILDS_ON`, `RELATED_TO` and `SPECIAL_CASE_OF`, and
//! **two of those four are the edges this engine descends**. Section 36.4's own
//! worked example is the case: `Buffer Pool` is the surface concept of an active
//! goal, so it is the concept the user is using *now*; `Disk Page` is one
//! `REQUIRES` hop below it; and a spillover from `Buffer Pool` across that very
//! edge puts `Disk Page` at `MODERATE` with no evidence of its own. The design
//! document's answer in section 36.4 is that `Disk Page` **is** the root gap. So
//! reading that band as `Disk Page`'s retrieval readiness is the surface
//! concept's evidence deciding its own prerequisite's deficit, one hop out from
//! where `P2-N2` closed it and one hop out from where `P2-N3` closed it.
//!
//! It cannot be refused here, because whether the neighbour lies on the blocking
//! path is not known until the descent knows the path. [`SpilloverSource`]
//! carries the neighbour and the edge so that [`crate::engine::search`] can
//! refuse it at the point the path exists, and
//! [`crate::GapError::FreshnessRestsOnPathSpillover`] names the neighbour and
//! the predicate rather than quietly lowering the band: the caller re-projects
//! that concept with `P2-N3`'s own function and without that contribution, which
//! keeps the concept's own evidence rather than discarding it.
//!
//! ## A projection cannot hide a contribution it used
//!
//! `overlay` also compares the declared contributions against the projection's
//! own trace — the count of `FreshnessSignal::RelatedConceptSpillover` entries
//! and the multiset of their bands — so a caller cannot hand over a projection
//! built from three spillovers while declaring one. Both halves are typed
//! accessors; nothing here parses a trace's detail text.

use std::collections::BTreeMap;

use academic_domain::{
    ConfidencePermille, EntityId, EvidenceId, FreshnessBand, MasteryLevel,
    entity_registry::EntityKind, predicates::PredicateName,
};
use academic_freshness::{FreshnessProjection, FreshnessSignal, Spillover};
use academic_knowledge_state::{
    ConceptEvidence, ConceptLink, EligibilityOutcome, EligibleEvidence, EvidenceDossier,
    EvidenceSufficiency, SufficiencyGap, UnseenBasis, project,
};
use serde::{Deserialize, Serialize};

use crate::{GapError, node::IdentityStanding};

/// One evidence item offered for a concept, before section 13.4's four checks.
///
/// The overlay takes these rather than `EligibleEvidence`, so the admission
/// decision and the concept check happen in the same place. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedEvidence {
    evidence: ConceptEvidence,
    evidence_id: EvidenceId,
    dossier: EvidenceDossier,
}

impl OfferedEvidence {
    /// Offers one item with the four answers `P2-N2` requires.
    #[must_use]
    pub const fn of(
        evidence: ConceptEvidence,
        evidence_id: EvidenceId,
        dossier: EvidenceDossier,
    ) -> Self {
        Self {
            evidence,
            evidence_id,
            dossier,
        }
    }

    /// Which item.
    #[must_use]
    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    /// The concept check one resolved, when it resolved one.
    #[must_use]
    pub const fn linked_concept(&self) -> Option<EntityId> {
        match self.dossier.concept_link() {
            ConceptLink::Exact(concept, _) => Some(concept),
            ConceptLink::Ambiguous | ConceptLink::Absent => None,
        }
    }
}

/// One neighbour whose recent use raised this concept's band.
///
/// Carried so the descent can refuse a band that rests on a concept it is
/// itself blaming. See the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpilloverSource {
    /// The neighbouring concept whose own use produced the contribution.
    pub neighbor: EntityId,
    /// The section 7.2 edge it was cited on.
    pub predicate: PredicateName,
    /// The band the contribution offered.
    pub band: FreshnessBand,
}

/// Section 15.2 step 3's overlay, on one concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptState {
    concept: EntityId,
    kind: EntityKind,
    identity: IdentityStanding,
    mastery: MasteryLevel,
    unseen_basis: Option<UnseenBasis>,
    freshness: FreshnessBand,
    confidence: ConfidencePermille,
    sufficiency_gaps: Vec<SufficiencyGap>,
    supporting: Vec<EvidenceId>,
    contradicting: Vec<EvidenceId>,
    offered_count: usize,
    spillover_sources: Vec<SpilloverSource>,
}

impl ConceptState {
    /// Overlays all four dimensions onto `concept`.
    ///
    /// # Errors
    ///
    /// [`GapError::EvidenceNamesAnotherConcept`] when an offered item's
    /// resolved concept link names a different concept;
    /// [`GapError::FreshnessNamesAnotherConcept`] when the projection is about a
    /// different concept; [`GapError::SpilloverNamesAnotherConcept`] when a
    /// contribution was computed toward a different concept;
    /// [`GapError::SpilloverNotDeclared`] when the declared contributions do not
    /// match the projection's own trace; and
    /// [`GapError::KnowledgeState`] when `P2-N2` refuses the projection.
    pub fn overlay(
        concept: EntityId,
        kind: EntityKind,
        identity: IdentityStanding,
        offered: &[OfferedEvidence],
        freshness: &FreshnessProjection,
        spillover: &[Spillover],
    ) -> Result<Self, GapError> {
        if freshness.concept() != concept {
            return Err(GapError::FreshnessNamesAnotherConcept);
        }
        for contribution in spillover {
            if contribution.subject() != concept {
                return Err(GapError::SpilloverNamesAnotherConcept);
            }
        }
        require_trace_declares(freshness, spillover)?;

        let mut admitted: Vec<EligibleEvidence> = Vec::new();
        let mut blocked = Vec::new();
        for item in offered {
            match EligibilityOutcome::admit(item.evidence.clone(), item.evidence_id, &item.dossier)
            {
                // An admitted item carries the concept check one resolved.
                EligibilityOutcome::Admitted(value) => {
                    if value.concept() != concept {
                        return Err(GapError::EvidenceNamesAnotherConcept);
                    }
                    admitted.push(value);
                }
                // A blocked one does not: `BlockedEvidence` keeps the failing
                // codes and drops the link. The dossier still holds it, and this
                // is the only place that answer survives — so the two arms are
                // two guards over disjoint halves, and neither can stand in for
                // the other.
                EligibilityOutcome::Blocked(value) => {
                    if item
                        .linked_concept()
                        .is_some_and(|linked| linked != concept)
                    {
                        return Err(GapError::EvidenceNamesAnotherConcept);
                    }
                    blocked.push(value);
                }
            }
        }

        let projection = project(&admitted, &blocked)?;
        let sufficiency: &EvidenceSufficiency = projection.sufficiency();

        Ok(Self {
            concept,
            kind,
            identity,
            mastery: projection.level(),
            unseen_basis: projection.unseen_basis(),
            freshness: freshness.band(),
            confidence: sufficiency.permille(),
            sufficiency_gaps: sufficiency.gaps().to_vec(),
            supporting: projection.supporting().to_vec(),
            contradicting: projection.contradicting().to_vec(),
            offered_count: offered.len(),
            spillover_sources: spillover
                .iter()
                .map(|contribution| SpilloverSource {
                    neighbor: contribution.neighbor(),
                    predicate: contribution.edge().predicate(),
                    band: contribution.band(),
                })
                .collect(),
        })
    }

    /// Which concept this overlay is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Its ontology tier.
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        self.kind
    }

    /// What `P2-C3`'s registry says about its identity.
    #[must_use]
    pub const fn identity(&self) -> &IdentityStanding {
        &self.identity
    }

    /// Dimension one.
    #[must_use]
    pub const fn mastery(&self) -> MasteryLevel {
        self.mastery
    }

    /// Why the level is `UNSEEN`, when it is. `P2-N2`'s two bases.
    #[must_use]
    pub const fn unseen_basis(&self) -> Option<UnseenBasis> {
        self.unseen_basis
    }

    /// Dimension two.
    #[must_use]
    pub const fn freshness(&self) -> FreshnessBand {
        self.freshness
    }

    /// Dimension three. `P2-N2`'s `estimateConfidence`, which is evidence
    /// sufficiency and not a skill score.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// What dimension three found missing.
    #[must_use]
    pub fn sufficiency_gaps(&self) -> &[SufficiencyGap] {
        &self.sufficiency_gaps
    }

    /// The admitted items supporting the level.
    #[must_use]
    pub fn supporting(&self) -> &[EvidenceId] {
        &self.supporting
    }

    /// Dimension four.
    #[must_use]
    pub fn contradicting(&self) -> &[EvidenceId] {
        &self.contradicting
    }

    /// How many items were offered at all, admitted or not.
    ///
    /// `P2-N2` separates `nothing was recorded` from `something was recorded and
    /// none of it licensed a promotion`, and section 15.2 routes those two to
    /// different gap kinds.
    #[must_use]
    pub const fn offered_count(&self) -> usize {
        self.offered_count
    }

    /// The neighbours whose use raised the band, in the order they were given.
    #[must_use]
    pub fn spillover_sources(&self) -> &[SpilloverSource] {
        &self.spillover_sources
    }
}

/// Refuses a projection whose trace names contributions the caller did not
/// declare, or declares contributions the trace does not name.
fn require_trace_declares(
    freshness: &FreshnessProjection,
    spillover: &[Spillover],
) -> Result<(), GapError> {
    let mut traced: BTreeMap<FreshnessBand, usize> = BTreeMap::new();
    for entry in freshness
        .trace()
        .of(FreshnessSignal::RelatedConceptSpillover)
    {
        if let Some(band) = entry.band() {
            *traced.entry(band).or_default() += 1;
        }
    }
    let mut declared: BTreeMap<FreshnessBand, usize> = BTreeMap::new();
    for contribution in spillover {
        *declared.entry(contribution.band()).or_default() += 1;
    }
    if traced == declared {
        Ok(())
    } else {
        Err(GapError::SpilloverNotDeclared)
    }
}

/// Section 15.3's `현재 상태`: the four dimensions as one serialisable value.
///
/// One field per [`crate::kind::StateDimension`], named after it, plus the concept the
/// reading is about and `P2-C3`'s standing for its identity.
/// `four_state_dimensions_are_overlaid` reads all four off this value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Which concept the reading is about. Never absent: a state reading with
    /// no concept on it is what let `academic_knowledge_state::project`'s
    /// output be attributed to the wrong one.
    pub concept: EntityId,
    /// Dimension one.
    pub mastery: MasteryLevel,
    /// Why the level is `UNSEEN`, when it is.
    pub unseen_basis: Option<UnseenBasis>,
    /// Dimension two.
    pub freshness: FreshnessBand,
    /// Dimension three.
    pub confidence: ConfidencePermille,
    /// What dimension three found missing.
    pub sufficiency_gaps: Vec<SufficiencyGap>,
    /// Dimension four.
    pub contradicting: Vec<EvidenceId>,
    /// `P2-C3`'s standing for the identity the other four are read onto.
    pub identity: IdentityStanding,
}

impl From<&ConceptState> for StateSnapshot {
    fn from(state: &ConceptState) -> Self {
        Self {
            concept: state.concept,
            mastery: state.mastery,
            unseen_basis: state.unseen_basis,
            freshness: state.freshness,
            confidence: state.confidence,
            sufficiency_gaps: state.sufficiency_gaps.clone(),
            contradicting: state.contradicting.clone(),
            identity: state.identity.clone(),
        }
    }
}
