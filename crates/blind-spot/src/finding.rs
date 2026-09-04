//! Section 23's `BlindSpotFinding` schema.
//!
//! The field count is not a number this file chose.
//! `five_states_are_semantically_distinct` reads the schema block's own `key:`
//! lines back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compares them against [`FINDING_FIELDS`] in both directions, so the eight are
//! a measurement of the design document.
//!
//! ## The evidence class and the user's disposition are two fields
//!
//! Section 39's twenty-first answer: `evidence 부족 분류와 user disposition을
//! 별도 저장한다`. The schema example is the case that makes it matter — it
//! carries `classification: UNOBSERVED` *and* a disposition at the same time —
//! so a disposition never overwrites the reading. What it does is decide whether
//! the finding warns, which is [`BlindSpotFinding::warns`].
//!
//! ## `likelyCause` is a distribution
//!
//! The example writes a sentence. This carries
//! [`crate::explanation::SkewExplanation`] instead, for the reason that module
//! gives: a free-text cause is the slot an action demand arrives through. The
//! wire keeps section 23's own field name.

use serde::{Deserialize, Serialize};

use academic_domain::EntityId;

use crate::{
    coverage::EvidenceDiversity,
    disposition::UserDisposition,
    explanation::SkewExplanation,
    presentation::FindingPresentation,
    relevance::GoalRelevance,
    state::{BlindSpotState, StateBasis, state_of},
};

/// Section 23's schema keys, in the order the block writes them.
pub const FINDING_FIELDS: [&str; 8] = [
    "field",
    "scope",
    "exposureEvidenceCount",
    "evidenceDiversity",
    "classification",
    "relevanceToActiveGoals",
    "likelyCause",
    "userDisposition",
];

/// One aggregation key's blind-spot reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlindSpotFinding {
    field: EntityId,
    scope: String,
    exposure_evidence_count: u32,
    evidence_diversity: EvidenceDiversity,
    basis: StateBasis,
    relevance_to_active_goals: GoalRelevance,
    likely_cause: SkewExplanation,
    user_disposition: Option<UserDisposition>,
    presentation: FindingPresentation,
    warns: bool,
}

impl BlindSpotFinding {
    /// Assembles one finding. Called only by [`crate::detector::detect`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn assemble(
        field: EntityId,
        scope: String,
        exposure_evidence_count: u32,
        evidence_diversity: EvidenceDiversity,
        basis: StateBasis,
        relevance_to_active_goals: GoalRelevance,
        likely_cause: SkewExplanation,
        user_disposition: Option<UserDisposition>,
        presentation: FindingPresentation,
        warns: bool,
    ) -> Self {
        Self {
            field,
            scope,
            exposure_evidence_count,
            evidence_diversity,
            basis,
            relevance_to_active_goals,
            likely_cause,
            user_disposition,
            presentation,
            warns,
        }
    }

    /// Section 23's `field`.
    #[must_use]
    pub const fn field(&self) -> EntityId {
        self.field
    }

    /// Section 23's `scope`.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Section 23's `exposureEvidenceCount`.
    #[must_use]
    pub const fn exposure_evidence_count(&self) -> u32 {
        self.exposure_evidence_count
    }

    /// Section 23's `evidenceDiversity`.
    #[must_use]
    pub const fn evidence_diversity(&self) -> EvidenceDiversity {
        self.evidence_diversity
    }

    /// Section 23's `classification`.
    #[must_use]
    pub const fn classification(&self) -> BlindSpotState {
        state_of(&self.basis)
    }

    /// Which fact put it in that state.
    #[must_use]
    pub const fn basis(&self) -> &StateBasis {
        &self.basis
    }

    /// Section 23's `relevanceToActiveGoals`.
    #[must_use]
    pub const fn relevance_to_active_goals(&self) -> &GoalRelevance {
        &self.relevance_to_active_goals
    }

    /// Section 23's `likelyCause`, as the distribution it summarises.
    #[must_use]
    pub const fn likely_cause(&self) -> &SkewExplanation {
        &self.likely_cause
    }

    /// Section 23's `userDisposition`, absent until the user records one.
    #[must_use]
    pub const fn user_disposition(&self) -> Option<UserDisposition> {
        self.user_disposition
    }

    /// What this finding shows.
    #[must_use]
    pub const fn presentation(&self) -> &FindingPresentation {
        &self.presentation
    }

    /// Whether this finding warns at the instant it was computed for.
    ///
    /// False whenever the user's standing disposition suppresses it —
    /// `NOT_RELEVANT` permanently, `HIDE_UNTIL` until its deadline.
    #[must_use]
    pub const fn warns(&self) -> bool {
        self.warns
    }

    /// Section 23's schema block, as data.
    #[must_use]
    pub fn to_wire(&self) -> BlindSpotFindingWire {
        BlindSpotFindingWire {
            field: self.field,
            scope: self.scope.clone(),
            exposure_evidence_count: self.exposure_evidence_count,
            evidence_diversity: self.evidence_diversity,
            classification: self.classification(),
            relevance_to_active_goals: self.relevance_to_active_goals.clone(),
            likely_cause: self.likely_cause.clone(),
            user_disposition: self.user_disposition,
        }
    }
}

/// The wire shape of a [`BlindSpotFinding`], with section 23's own field names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlindSpotFindingWire {
    /// Section 23's `field`.
    pub field: EntityId,
    /// Section 23's `scope`.
    pub scope: String,
    /// Section 23's `exposureEvidenceCount`.
    #[serde(rename = "exposureEvidenceCount")]
    pub exposure_evidence_count: u32,
    /// Section 23's `evidenceDiversity`.
    #[serde(rename = "evidenceDiversity")]
    pub evidence_diversity: EvidenceDiversity,
    /// Section 23's `classification`.
    pub classification: BlindSpotState,
    /// Section 23's `relevanceToActiveGoals`.
    #[serde(rename = "relevanceToActiveGoals")]
    pub relevance_to_active_goals: GoalRelevance,
    /// Section 23's `likelyCause`.
    #[serde(rename = "likelyCause")]
    pub likely_cause: SkewExplanation,
    /// Section 23's `userDisposition`.
    #[serde(rename = "userDisposition")]
    pub user_disposition: Option<UserDisposition>,
}
