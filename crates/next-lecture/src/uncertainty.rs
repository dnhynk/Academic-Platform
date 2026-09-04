//! Section 12.7's last sentence, as three types that have no common shape.
//!
//! > 예상 concept, prerequisite edge, 사용자 state가 모두 불확실할 수 있으므로
//! > 각각의 근거와 confidence를 분리한다.
//!
//! ## The fold is not refused, it is unrepresentable
//!
//! A three-element array of one reading type would be separated on paper and
//! folded in one line: `readings.iter().map(confidence).min()`. So the three
//! axes are three **different types** — [`ExpectedConceptReading`],
//! [`PrerequisiteEdgeReading`], [`UserStateReading`] — with no trait between
//! them, no shared supertype and no collection that holds more than one of them.
//! There is no array to fold, no iterator to reduce and no
//! [`PrepUncertainty`] method that answers with a confidence.
//!
//! `prep_uncertainty_factorization` observes that as a whole-set classification
//! rather than a list of forbidden method names: every public signature in this
//! crate whose return type mentions `ConfidencePermille` is enumerated, and each
//! is required to be an accessor on exactly one of the three reading types. A
//! `PrepUncertainty::confidence`, a `PreparationCandidate::confidence` or a
//! `PreparationBrief::confidence` added later is a new entry in that set with an
//! owner the answer does not hold, whatever it is called.
//!
//! ## Each axis's evidence comes from its own owner
//!
//! [`PrepUncertainty::factor`] takes the claim, the edge and the state, and
//! reads each axis out of the argument that owns it. There is no parameter a
//! caller could pass the same list to three times:
//!
//! | Axis | `근거` | `confidence` |
//! |---|---|---|
//! | 예상 concept | the claim's own `P2-G5` spans | the claim's own |
//! | prerequisite edge | the edge's own cited items | **supplied** |
//! | 사용자 state | the overlay's own supporting and contradicting items | the overlay's own |
//!
//! Two of the three confidences are read off the value that owns them and
//! cannot be folded because they are not parameters at all. The third is
//! supplied because `P2-N5`'s `PrerequisiteEdge` carries no confidence of its
//! own — section 7.3 puts an edge's confidence on the claim that asserts it, not
//! on the traversal — and that asymmetry is recorded rather than hidden behind a
//! default.
//!
//! The evidence types differ too. The expected-concept axis cites
//! `ResolvedSpan`s into untrusted documents; the other two cite `EvidenceId`s.
//! `P2-N5`'s own `RootCandidate::evidence` merges the edge's items with the
//! state's into one list, which is the fold this crate exists not to inherit:
//! nothing here reads that accessor.

use academic_domain::{
    ConfidencePermille, EntityId, EvidenceId, FreshnessBand, MasteryLevel,
    predicates::{PredicateName, PrerequisiteStrength},
};
use academic_gap::{ConceptState, PrerequisiteEdge};
use academic_untrusted_content::ResolvedSpan;
use serde::{Deserialize, Serialize};

use crate::{NextLectureError, claim::ExpectedConceptClaim, source::MaterialReference};

/// One of the three things section 12.7's last sentence says may be uncertain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrepAxis {
    /// `예상 concept`.
    ExpectedConcept,
    /// `prerequisite edge`.
    PrerequisiteEdge,
    /// `사용자 state`.
    UserState,
}

/// The three, in section 12.7's own order.
///
/// The same array under a module-level name; the one literal list is
/// [`PrepAxis::ALL`].
pub const PREP_AXES: [PrepAxis; 3] = PrepAxis::ALL;

impl PrepAxis {
    /// Exhaustive order, in section 12.7's own sentence order.
    pub const ALL: [Self; 3] = [
        Self::ExpectedConcept,
        Self::PrerequisiteEdge,
        Self::UserState,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpectedConcept => "EXPECTED_CONCEPT",
            Self::PrerequisiteEdge => "PREREQUISITE_EDGE",
            Self::UserState => "USER_STATE",
        }
    }

    /// The cell section 12.7's last sentence writes for this axis, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::ExpectedConcept => "예상 concept",
            Self::PrerequisiteEdge => "prerequisite edge",
            Self::UserState => "사용자 state",
        }
    }
}

/// Axis one: how sure the extraction is that tomorrow uses this concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedConceptReading {
    concept: EntityId,
    material: MaterialReference,
    citations: Vec<ResolvedSpan>,
    confidence: ConfidencePermille,
}

impl ExpectedConceptReading {
    /// Which axis this is.
    pub const AXIS: PrepAxis = PrepAxis::ExpectedConcept;

    /// The concept the material named.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// `자료 날짜` and which of the seven places, both from section 27.1's
    /// confirmation condition.
    #[must_use]
    pub const fn material(&self) -> &MaterialReference {
        &self.material
    }

    /// `근거`: the spans of that material the model cited.
    #[must_use]
    pub fn citations(&self) -> &[ResolvedSpan] {
        &self.citations
    }

    /// `confidence`, for this axis and no other.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }
}

/// Axis two: how sure the graph is that this really is a prerequisite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrerequisiteEdgeReading {
    advanced: EntityId,
    prerequisite: EntityId,
    predicate: PredicateName,
    strength: PrerequisiteStrength,
    evidence: Vec<EvidenceId>,
    confidence: ConfidencePermille,
}

impl PrerequisiteEdgeReading {
    /// Which axis this is.
    pub const AXIS: PrepAxis = PrepAxis::PrerequisiteEdge;

    /// The concept the edge runs out of.
    #[must_use]
    pub const fn advanced(&self) -> EntityId {
        self.advanced
    }

    /// The concept the edge runs into.
    #[must_use]
    pub const fn prerequisite(&self) -> EntityId {
        self.prerequisite
    }

    /// Which of `P2-C4`'s traversable predicates.
    #[must_use]
    pub const fn predicate(&self) -> PredicateName {
        self.predicate
    }

    /// Which strength that predicate was admitted at.
    #[must_use]
    pub const fn strength(&self) -> PrerequisiteStrength {
        self.strength
    }

    /// `근거`: the items the edge itself cites.
    #[must_use]
    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }

    /// `confidence`, for this axis and no other.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }
}

/// Axis three: how sure the record is about where the person stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStateReading {
    concept: EntityId,
    mastery: MasteryLevel,
    freshness: FreshnessBand,
    supporting: Vec<EvidenceId>,
    contradicting: Vec<EvidenceId>,
    confidence: ConfidencePermille,
}

impl UserStateReading {
    /// Which axis this is.
    pub const AXIS: PrepAxis = PrepAxis::UserState;

    /// Which concept the reading is about.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// `P2-N2`'s rung.
    #[must_use]
    pub const fn mastery(&self) -> MasteryLevel {
        self.mastery
    }

    /// `P2-N3`'s band.
    #[must_use]
    pub const fn freshness(&self) -> FreshnessBand {
        self.freshness
    }

    /// `근거`, the half that supports.
    #[must_use]
    pub fn supporting(&self) -> &[EvidenceId] {
        &self.supporting
    }

    /// `근거`, the half that contradicts. Kept apart from the half above
    /// because `P2-N2` keeps them apart, and because a recorded failure is a
    /// deficit at any rung.
    #[must_use]
    pub fn contradicting(&self) -> &[EvidenceId] {
        &self.contradicting
    }

    /// `confidence`, for this axis and no other.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }
}

/// The three axes, side by side and never summarised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepUncertainty {
    expected_concept: ExpectedConceptReading,
    prerequisite_edge: PrerequisiteEdgeReading,
    user_state: UserStateReading,
}

impl PrepUncertainty {
    /// Factors one preparation's uncertainty into section 12.7's three.
    ///
    /// # Errors
    ///
    /// [`NextLectureError::AxesDescribeDifferentConcepts`] when the edge does
    /// not run into the concept the state is about. Two axes that disagree about
    /// which concept they are reading are not a factorization of one
    /// uncertainty; they are two uncertainties about two things, and `P2-N5`
    /// refuses the same shape at `ConceptState::overlay`.
    pub fn factor(
        claim: &ExpectedConceptClaim,
        edge: &PrerequisiteEdge,
        edge_confidence: ConfidencePermille,
        state: &ConceptState,
    ) -> Result<Self, NextLectureError> {
        if edge.prerequisite() != state.concept() {
            return Err(NextLectureError::AxesDescribeDifferentConcepts {
                edge: edge.prerequisite(),
                state: state.concept(),
            });
        }
        Ok(Self {
            expected_concept: ExpectedConceptReading {
                concept: claim.concept(),
                material: claim.material().clone(),
                citations: claim.citations().to_vec(),
                confidence: claim.confidence(),
            },
            prerequisite_edge: PrerequisiteEdgeReading {
                advanced: edge.advanced(),
                prerequisite: edge.prerequisite(),
                predicate: edge.predicate(),
                strength: edge.strength(),
                evidence: edge.evidence().to_vec(),
                confidence: edge_confidence,
            },
            user_state: UserStateReading {
                concept: state.concept(),
                mastery: state.mastery(),
                freshness: state.freshness(),
                supporting: state.supporting().to_vec(),
                contradicting: state.contradicting().to_vec(),
                confidence: state.confidence(),
            },
        })
    }

    /// `예상 concept`.
    #[must_use]
    pub const fn expected_concept(&self) -> &ExpectedConceptReading {
        &self.expected_concept
    }

    /// `prerequisite edge`.
    #[must_use]
    pub const fn prerequisite_edge(&self) -> &PrerequisiteEdgeReading {
        &self.prerequisite_edge
    }

    /// `사용자 state`.
    #[must_use]
    pub const fn user_state(&self) -> &UserStateReading {
        &self.user_state
    }
}
