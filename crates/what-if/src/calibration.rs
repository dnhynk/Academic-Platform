//! Section 22.5's third bullet: *학기 종료 시 projected vs actual evidence를
//! 비교해 모델을 calibration하되 사용자를 평가하지 않는다.*
//!
//! # The subject of the report is the model
//!
//! [`ModelCalibrationReport`] carries the engine version, the frozen-input
//! digest and the model run behind the projection it is calibrating. It carries
//! no student identifier, no grade, no mastery level, no completion percentage
//! and no accuracy figure of any kind about a person.
//!
//! That is not a promise in a comment. `end_of_term_calibration_emits_no_user_score`
//! compares the whole declared field inventory of this module against a
//! reviewed one in both directions, and every entry in it names the model, the
//! offering, the concept or the opportunity. A field about the user, spelling
//! nothing anybody thought to forbid, fails as an entry nobody wrote down.
//!
//! # The direction vocabulary is `P2-C7`'s
//!
//! [`academic_scenario::ProjectionCalibration`] already says *the projection
//! ran ahead of, matched, or lagged what actually happened*, and its
//! documentation already says the subject of the result is the model and never
//! the user. This crate reuses it rather than declaring a second one, so there
//! is one vocabulary in this repository for how wrong a projection was.
//!
//! # An unprojected occasion is an under-projection, not a user failing
//!
//! An occasion that happened and was never projected is
//! [`academic_scenario::ProjectionCalibration::Underprojected`]. An occasion
//! that was projected and did not happen is `Overprojected`. Both are
//! statements about the projection. Neither says anything about whether the
//! user did the work, which is the reading section 34.5's *Blind Spot을 공부
//! 압박으로 변환* row is the general case of.

use std::collections::BTreeSet;

use academic_domain::{ContentDigest, EntityId, ModelRunId, OfferingId};
use academic_scenario::{LikelihoodBand, OpportunityKind, ProjectionCalibration};

use crate::{error::WhatIfError, scenario::PlanScenario};

/// One occasion that actually happened during the term.
///
/// An *occasion*, not an outcome: that a concept was presented, practised or
/// assessed. What the user learned from it is decided by the evidence the term
/// produced and is not this type's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservedOccasion {
    offering_id: OfferingId,
    concept_entity_id: EntityId,
    kind: OpportunityKind,
}

impl ObservedOccasion {
    /// Records one occasion.
    #[must_use]
    pub const fn of(
        offering_id: OfferingId,
        concept_entity_id: EntityId,
        kind: OpportunityKind,
    ) -> Self {
        Self {
            offering_id,
            concept_entity_id,
            kind,
        }
    }

    /// Which offering.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Which concept.
    #[must_use]
    pub const fn concept_entity_id(&self) -> EntityId {
        self.concept_entity_id
    }

    /// Which kind of occasion.
    #[must_use]
    pub const fn kind(&self) -> OpportunityKind {
        self.kind
    }
}

/// One projected opportunity set against what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalibrationEntry {
    offering_id: OfferingId,
    concept_entity_id: EntityId,
    kind: OpportunityKind,
    projected: Option<LikelihoodBand>,
    observed: bool,
    direction: ProjectionCalibration,
}

impl CalibrationEntry {
    /// Which offering.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// Which concept.
    #[must_use]
    pub const fn concept_entity_id(&self) -> EntityId {
        self.concept_entity_id
    }

    /// Which kind of occasion.
    #[must_use]
    pub const fn kind(&self) -> OpportunityKind {
        self.kind
    }

    /// The band the plan projected, absent when the plan projected nothing.
    #[must_use]
    pub const fn projected(&self) -> Option<LikelihoodBand> {
        self.projected
    }

    /// Whether the occasion happened.
    #[must_use]
    pub const fn observed(&self) -> bool {
        self.observed
    }

    /// How wrong the projection was, in `P2-C7`'s own vocabulary.
    #[must_use]
    pub const fn direction(&self) -> ProjectionCalibration {
        self.direction
    }
}

/// What one term said about one plan's projections.
///
/// Every field names the model, the plan or an occasion. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCalibrationReport {
    plan_id: EntityId,
    engine_version: u32,
    inputs_digest: ContentDigest,
    model_run_id: ModelRunId,
    entries: Vec<CalibrationEntry>,
}

impl ModelCalibrationReport {
    /// The plan whose projections were calibrated.
    #[must_use]
    pub const fn plan_id(&self) -> EntityId {
        self.plan_id
    }

    /// The engine version that produced them.
    #[must_use]
    pub const fn engine_version(&self) -> u32 {
        self.engine_version
    }

    /// The frozen inputs they were produced from.
    #[must_use]
    pub const fn inputs_digest(&self) -> ContentDigest {
        self.inputs_digest
    }

    /// The model run behind them.
    #[must_use]
    pub const fn model_run_id(&self) -> ModelRunId {
        self.model_run_id
    }

    /// Every comparison, in a stable order.
    #[must_use]
    pub fn entries(&self) -> &[CalibrationEntry] {
        &self.entries
    }

    /// How many comparisons landed in one direction.
    ///
    /// Derived from the entries rather than stored, and a count rather than a
    /// ratio: a percentage of anything here would be one number standing for a
    /// term, which is the shape section 22.4 refuses and section 24 refuses
    /// again for competency.
    #[must_use]
    pub fn count_of(&self, direction: ProjectionCalibration) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.direction() == direction)
            .count()
    }
}

/// Compares a plan's projections against the occasions the term produced.
///
/// # Errors
///
/// [`WhatIfError::CalibrationNamesAnotherPlan`] when the plan identity given
/// does not match the plan.
pub fn calibrate(
    plan: &PlanScenario,
    plan_id: EntityId,
    observed: &[ObservedOccasion],
) -> Result<ModelCalibrationReport, WhatIfError> {
    if plan.id() != plan_id {
        return Err(WhatIfError::CalibrationNamesAnotherPlan);
    }
    let seen: BTreeSet<ObservedOccasion> = observed.iter().copied().collect();
    let mut entries = Vec::new();
    let mut matched_projections = BTreeSet::new();
    for opportunity in &plan.projections().opportunities().opportunities {
        let occasion = ObservedOccasion::of(
            opportunity.offering_id,
            opportunity.concept_entity_id,
            opportunity.kind,
        );
        matched_projections.insert(occasion);
        let happened = seen.contains(&occasion);
        entries.push(CalibrationEntry {
            offering_id: occasion.offering_id(),
            concept_entity_id: occasion.concept_entity_id(),
            kind: occasion.kind(),
            projected: Some(opportunity.likelihood),
            observed: happened,
            direction: direction_of(Some(opportunity.likelihood), happened),
        });
    }
    for occasion in &seen {
        if matched_projections.contains(occasion) {
            continue;
        }
        entries.push(CalibrationEntry {
            offering_id: occasion.offering_id(),
            concept_entity_id: occasion.concept_entity_id(),
            kind: occasion.kind(),
            projected: None,
            observed: true,
            direction: direction_of(None, true),
        });
    }
    entries.sort_unstable();
    Ok(ModelCalibrationReport {
        plan_id,
        engine_version: crate::WHAT_IF_ENGINE_VERSION,
        inputs_digest: plan.inputs_digest(),
        model_run_id: plan
            .projections()
            .workload()
            .proposed()
            .provenance()
            .model_run_id(),
        entries,
    })
}

/// How wrong one projection was.
///
/// A band that expected the occasion and did not get it ran ahead; a band that
/// did not expect it and got it lagged. `Unknown` and `Low` are the bands that
/// did not expect it, which is why an occasion under either of them is an
/// under-projection rather than a match.
const fn direction_of(projected: Option<LikelihoodBand>, observed: bool) -> ProjectionCalibration {
    let expected = match projected {
        None | Some(LikelihoodBand::Unknown | LikelihoodBand::Low) => false,
        Some(LikelihoodBand::Moderate | LikelihoodBand::High) => true,
    };
    match (expected, observed) {
        (true, true) | (false, false) => ProjectionCalibration::Matched,
        (true, false) => ProjectionCalibration::Overprojected,
        (false, true) => ProjectionCalibration::Underprojected,
    }
}
