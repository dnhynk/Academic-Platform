//! The only future-knowledge output type.

use academic_domain::{ConfidencePermille, EntityId, OfferingId};
use serde::{Deserialize, Serialize};

/// A projected chance to *generate evidence*, which is the only thing the
/// simulator is allowed to say about the future.
///
/// The forbidden shape is "take Networks and TCP becomes `Understood` at 68%".
/// The permitted shape is "TCP is likely to be covered in lecture, moderately
/// likely to be implemented in an assignment, and the mastery that results will
/// be decided during the term by the evidence actually produced". This type
/// spells the second shape and cannot spell the first: it carries no mastery
/// level, no freshness band, and no claim object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProjectedEvidenceOpportunity {
    /// The offering whose published material implies the opportunity.
    pub offering_id: OfferingId,
    /// The concept the opportunity is about.
    pub concept_entity_id: EntityId,
    /// Which of the three §22.3 opportunity kinds this is.
    pub kind: OpportunityKind,
    /// How likely the opportunity is to materialise.
    pub likelihood: LikelihoodBand,
    /// What the projection was inferred from, so a reader can discount it.
    pub basis: OpportunityBasis,
    /// Confidence in the likelihood band itself.
    pub confidence: ConfidencePermille,
}

/// The three opportunity kinds §22.3 admits.
///
/// The enum is closed on purpose. Adding a fourth kind that named an outcome
/// rather than an opportunity — "attained", "mastered", "retained" — would
/// reintroduce the overprediction this type exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpportunityKind {
    /// The concept is expected to be presented.
    Exposure,
    /// The concept is expected to be applied in graded or ungraded work.
    Practice,
    /// The concept is expected to be assessed.
    Assessment,
}

/// Coarse likelihood, deliberately banded rather than numeric.
///
/// A percentage invites arithmetic across projections and reads as a
/// measurement. A band does neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LikelihoodBand {
    /// No basis in the frozen inputs supports the opportunity.
    Unknown,
    /// Weakly supported.
    Low,
    /// Supported, with material uncertainty.
    Moderate,
    /// Strongly supported by the frozen inputs.
    High,
}

/// What the projection was inferred from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpportunityBasis {
    /// Read from the offering's published syllabus.
    Syllabus,
    /// Read from a published assignment or project brief.
    AssignmentBrief,
    /// Read from a published assessment plan.
    AssessmentPlan,
    /// Inferred from the historical pattern of earlier offerings.
    HistoricalOffering,
}

impl ProjectedEvidenceOpportunity {
    /// Returns the likelihood band.
    ///
    /// The accessor returns the band and nothing else. There is deliberately no
    /// method that turns an opportunity into a mastery level, a freshness band,
    /// a confidence-weighted score, or a claim object.
    #[must_use]
    pub const fn likelihood(&self) -> LikelihoodBand {
        self.likelihood
    }
}
