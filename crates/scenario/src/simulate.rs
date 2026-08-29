//! The pure what-if projection engine.

use std::collections::BTreeSet;

use academic_domain::{
    ConfidencePermille, ContentDigest, EntityId, ModelRunId, OfferingId, TimestampMillis,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    SCENARIO_ENGINE_VERSION,
    error::ScenarioError,
    opportunity::{
        LikelihoodBand, OpportunityBasis, OpportunityKind, ProjectedEvidenceOpportunity,
    },
    proposed::ProposalProvenance,
    workload::{ProjectedWorkloadRange, WorkloadHoursRange},
};

/// Domain separator for the frozen-input digest.
const INPUTS_DIGEST_DOMAIN: &str = "academic-scenario/scenario-inputs/v1";

/// Frozen inputs of one what-if run.
///
/// Everything the engine reads is in here. There is no clock, no RNG, no
/// network, and no ambient state, so the same inputs always produce the same
/// projection. That determinism is what lets the envelope bind a projection to
/// the inputs it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ScenarioInputs {
    /// Identity of the plan being simulated.
    pub scenario_id: EntityId,
    /// The model run that stands behind the projection.
    pub model_run_id: ModelRunId,
    /// The knowledge-state cut the plan was built against.
    pub knowledge_state_as_of: TimestampMillis,
    /// Digest of the requirement set the plan was built against.
    pub requirement_set_digest: ContentDigest,
    /// Digest of the offering catalogue snapshot the plan chose from.
    pub offering_catalog_digest: ContentDigest,
    /// The offerings this plan chooses, in the caller's order.
    pub choices: Vec<ScenarioChoice>,
    /// Assumptions the user or a review model supplied, recorded verbatim.
    pub assumptions: Vec<ScenarioAssumption>,
}

/// One chosen offering and the published material behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ScenarioChoice {
    /// The chosen offering.
    pub offering_id: OfferingId,
    /// Credit units the offering carries, used by the deterministic lane.
    pub credit_units: u16,
    /// Assumed weekly hours for this offering.
    pub assumed_weekly_hours: WorkloadHoursRange,
    /// Concept signals read from the offering's published material.
    pub syllabus_concepts: Vec<SyllabusConceptSignal>,
}

/// One concept the published material of an offering mentions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SyllabusConceptSignal {
    /// The concept the material mentions.
    pub concept_entity_id: EntityId,
    /// Where the mention was read from.
    pub basis: OpportunityBasis,
    /// How much of the published material the concept occupies, in permille.
    ///
    /// A permille rather than a float: binary floating point is deliberately
    /// absent from every value that reaches a comparison or a digest.
    pub coverage_permille: u16,
    /// Whether the material states the concept is assessed.
    pub assessed: bool,
}

/// One assumption the plan was built on, recorded rather than inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ScenarioAssumption {
    /// Stable name of the assumption.
    pub name: String,
    /// The assumed value, as the user or model stated it.
    pub value: String,
}

/// The output of one what-if run.
///
/// Note what the struct does not have: no mastery field, no freshness field, no
/// claim, and no claim object. Section 22.3 fixes the shape of a
/// future-knowledge statement, and [`ProjectedEvidenceOpportunity`] is the only
/// one this type can carry. A field here that named an attained level would
/// defeat the whole crate, so the omission is the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ScenarioProjection {
    /// The plan this projection belongs to.
    pub scenario_id: EntityId,
    /// Engine version that produced it.
    pub engine_version: u32,
    /// Digest of the frozen inputs it was computed from.
    pub inputs_digest: ContentDigest,
    /// Every projected evidence opportunity, in a stable order.
    pub opportunities: Vec<ProjectedEvidenceOpportunity>,
    /// The plan's projected weekly workload range.
    pub workload: ProjectedWorkloadRange,
}

/// Projects one plan.
///
/// Deterministic and total: identical inputs always yield an identical
/// projection, and every rejection is a typed error rather than a panic.
pub fn project(inputs: &ScenarioInputs) -> Result<ScenarioProjection, ScenarioError> {
    if inputs.choices.is_empty() {
        return Err(ScenarioError::EmptyScenario);
    }
    let mut seen_offerings = BTreeSet::new();
    let mut opportunities = Vec::new();
    let mut workload = WorkloadHoursRange::new(0, 0)?;
    for choice in &inputs.choices {
        if !seen_offerings.insert(choice.offering_id) {
            return Err(ScenarioError::DuplicateOfferingChoice(
                choice.offering_id.to_string(),
            ));
        }
        if choice.syllabus_concepts.is_empty() {
            return Err(ScenarioError::EmptyConceptSignals(
                choice.offering_id.to_string(),
            ));
        }
        let mut seen_concepts = BTreeSet::new();
        for signal in &choice.syllabus_concepts {
            if !seen_concepts.insert(signal.concept_entity_id) {
                return Err(ScenarioError::DuplicateConceptSignal {
                    offering: choice.offering_id.to_string(),
                    concept: signal.concept_entity_id.to_string(),
                });
            }
            opportunities.extend(project_signal(choice.offering_id, signal)?);
        }
        workload = workload.saturating_add(choice.assumed_weekly_hours);
    }
    opportunities.sort_by_key(|opportunity| {
        (
            *opportunity.offering_id.as_bytes(),
            *opportunity.concept_entity_id.as_bytes(),
            opportunity.kind,
        )
    });
    let inputs_digest = scenario_inputs_digest(inputs);
    let provenance = ProposalProvenance::new(
        inputs.model_run_id,
        inputs_digest,
        SCENARIO_ENGINE_VERSION,
        inputs.knowledge_state_as_of,
    );
    Ok(ScenarioProjection {
        scenario_id: inputs.scenario_id,
        engine_version: SCENARIO_ENGINE_VERSION,
        inputs_digest,
        opportunities,
        workload: ProjectedWorkloadRange::new(workload, provenance),
    })
}

/// Turns one published-material signal into its opportunities.
///
/// Coverage decides how likely the concept is to be presented and practised. An
/// assessment opportunity is projected only where the material actually says
/// the concept is assessed: inferring assessment from coverage would be the
/// overprediction of section 22.5, one step removed.
fn project_signal(
    offering_id: OfferingId,
    signal: &SyllabusConceptSignal,
) -> Result<Vec<ProjectedEvidenceOpportunity>, ScenarioError> {
    let exposure = likelihood_from_coverage(signal.coverage_permille);
    let practice = match signal.basis {
        OpportunityBasis::AssignmentBrief => exposure,
        OpportunityBasis::Syllabus
        | OpportunityBasis::AssessmentPlan
        | OpportunityBasis::HistoricalOffering => step_down(exposure),
    };
    let confidence = confidence_from_basis(signal.basis)?;
    let mut projected = vec![
        opportunity(
            offering_id,
            signal,
            OpportunityKind::Exposure,
            exposure,
            confidence,
        ),
        opportunity(
            offering_id,
            signal,
            OpportunityKind::Practice,
            practice,
            confidence,
        ),
    ];
    if signal.assessed {
        projected.push(opportunity(
            offering_id,
            signal,
            OpportunityKind::Assessment,
            exposure,
            confidence,
        ));
    }
    Ok(projected)
}

fn opportunity(
    offering_id: OfferingId,
    signal: &SyllabusConceptSignal,
    kind: OpportunityKind,
    likelihood: LikelihoodBand,
    confidence: ConfidencePermille,
) -> ProjectedEvidenceOpportunity {
    ProjectedEvidenceOpportunity {
        offering_id,
        concept_entity_id: signal.concept_entity_id,
        kind,
        likelihood,
        basis: signal.basis,
        confidence,
    }
}

/// Bands published coverage.
///
/// Coverage above one thousand permille still bands as `High` rather than
/// failing: a malformed catalogue entry must not stop a plan from being
/// projected at all.
const fn likelihood_from_coverage(coverage_permille: u16) -> LikelihoodBand {
    match coverage_permille {
        0 => LikelihoodBand::Unknown,
        1..=99 => LikelihoodBand::Low,
        100..=299 => LikelihoodBand::Moderate,
        _ => LikelihoodBand::High,
    }
}

const fn step_down(band: LikelihoodBand) -> LikelihoodBand {
    match band {
        LikelihoodBand::High => LikelihoodBand::Moderate,
        LikelihoodBand::Moderate => LikelihoodBand::Low,
        LikelihoodBand::Low | LikelihoodBand::Unknown => LikelihoodBand::Unknown,
    }
}

/// Confidence in the band itself, fixed per basis.
///
/// A published assignment brief states what will be built; a historical pattern
/// only says what used to happen. The gap between the two is what a reader
/// needs in order to discount the projection.
fn confidence_from_basis(basis: OpportunityBasis) -> Result<ConfidencePermille, ScenarioError> {
    let permille = match basis {
        OpportunityBasis::AssignmentBrief | OpportunityBasis::AssessmentPlan => 800,
        OpportunityBasis::Syllabus => 650,
        OpportunityBasis::HistoricalOffering => 400,
    };
    // `unwrap_used` and `expect_used` are denied workspace-wide. The bound
    // belongs to the domain, so the constructor stays fallible here rather than
    // being asserted away with a second copy of the range check.
    Ok(ConfidencePermille::new(permille)?)
}

/// Digests the frozen inputs with length-delimited fields.
///
/// Length delimiting is what stops two different input sets from digesting
/// alike by moving a byte across a field boundary. The envelope binding rests
/// on this digest, so a collision here would be a forgery path.
#[must_use]
pub fn scenario_inputs_digest(inputs: &ScenarioInputs) -> ContentDigest {
    let mut hasher = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(bytes);
    };
    field(INPUTS_DIGEST_DOMAIN.as_bytes());
    field(inputs.scenario_id.as_bytes());
    field(inputs.model_run_id.as_bytes());
    field(&inputs.knowledge_state_as_of.value().to_be_bytes());
    field(inputs.requirement_set_digest.as_bytes());
    field(inputs.offering_catalog_digest.as_bytes());
    field(&length(inputs.choices.len()).to_be_bytes());
    for choice in &inputs.choices {
        field(choice.offering_id.as_bytes());
        field(&choice.credit_units.to_be_bytes());
        field(&choice.assumed_weekly_hours.low_hours().to_be_bytes());
        field(&choice.assumed_weekly_hours.high_hours().to_be_bytes());
        field(&length(choice.syllabus_concepts.len()).to_be_bytes());
        for signal in &choice.syllabus_concepts {
            field(signal.concept_entity_id.as_bytes());
            field(basis_tag(signal.basis).as_bytes());
            field(&signal.coverage_permille.to_be_bytes());
            field(if signal.assessed { b"true" } else { b"false" });
        }
    }
    field(&length(inputs.assumptions.len()).to_be_bytes());
    for assumption in &inputs.assumptions {
        field(assumption.name.as_bytes());
        field(assumption.value.as_bytes());
    }
    ContentDigest::from_sha256_bytes(hasher.finalize().into())
}

fn length(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

const fn basis_tag(basis: OpportunityBasis) -> &'static str {
    match basis {
        OpportunityBasis::Syllabus => "SYLLABUS",
        OpportunityBasis::AssignmentBrief => "ASSIGNMENT_BRIEF",
        OpportunityBasis::AssessmentPlan => "ASSESSMENT_PLAN",
        OpportunityBasis::HistoricalOffering => "HISTORICAL_OFFERING",
    }
}
