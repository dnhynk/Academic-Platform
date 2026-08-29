//! The runtime half of the isolation: admission of a projection payload.
//!
//! Types isolate values, not bytes. A projection that has been serialised is
//! just JSON, and anyone can write JSON. The compile-fail suite proves no
//! *expression* carries a projected value into an actual-state write; this
//! module covers the other route, where a payload is hand-written to look like
//! something it is not.
//!
//! Every projection that crosses a process, a file, or an IPC boundary is
//! wrapped in a [`ProjectionEnvelope`] and comes back through
//! [`admit_projection_payload`], which fails closed.

use academic_domain::{AuthorityClass, ContentDigest, EpistemicStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    opportunity::ProjectedEvidenceOpportunity,
    simulate::ScenarioProjection,
    workload::{ProjectedWorkloadRange, WorkloadHoursRange},
};

/// Envelope format version. Bumping it invalidates every existing binding.
pub const PROJECTION_ENVELOPE_VERSION: u32 = 1;

/// Domain separator for the envelope binding.
const BINDING_DOMAIN: &str = "academic-scenario/projection-envelope/v1";

/// The authority a projection is allowed to claim.
///
/// A projection is a prediction and nothing else. `OFFICIAL`, `USER_EXPLICIT`,
/// `DIRECT_OBSERVATION`, `DETERMINISTIC_ENGINE`, and `CURATED` all assert that
/// something was seen, decided, or derived from facts, which is exactly the
/// impersonation this admission exists to refuse.
const ADMITTED_AUTHORITY: AuthorityClass = AuthorityClass::Prediction;

/// The epistemic status a projection is allowed to claim.
const ADMITTED_STATUS: EpistemicStatus = EpistemicStatus::Prediction;

/// A projection in transit.
///
/// The envelope states what the payload is (`hypothetical`), what it is allowed
/// to claim (`authority_class`, `epistemic_status`), and what it was computed
/// from (`binding_digest`). Every one of those is re-derived on admission, so
/// a field is a declaration to be checked rather than a value to be trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProjectionEnvelope {
    /// Envelope format version.
    pub envelope_version: u32,
    /// Always true. A payload that says otherwise is claiming to be a fact.
    pub hypothetical: bool,
    /// The authority the payload claims.
    pub authority_class: AuthorityClass,
    /// The epistemic status the payload claims.
    pub epistemic_status: EpistemicStatus,
    /// The projection itself.
    pub projection: ScenarioProjection,
    /// Binding over the domain separator, the declarations, and the projection.
    pub binding_digest: ContentDigest,
}

impl ProjectionEnvelope {
    /// Wraps a projection for transit and computes its binding.
    #[must_use]
    pub fn seal(projection: ScenarioProjection) -> Self {
        let binding_digest = binding_digest(
            PROJECTION_ENVELOPE_VERSION,
            true,
            ADMITTED_AUTHORITY,
            ADMITTED_STATUS,
            &projection,
        );
        Self {
            envelope_version: PROJECTION_ENVELOPE_VERSION,
            hypothetical: true,
            authority_class: ADMITTED_AUTHORITY,
            epistemic_status: ADMITTED_STATUS,
            projection,
            binding_digest,
        }
    }
}

/// Why a payload was refused.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProjectionAdmissionError {
    /// The bytes were not a well-formed envelope, or carried an unknown field.
    ///
    /// `deny_unknown_fields` is what makes this a refusal rather than a silent
    /// drop: a payload that smuggles `mastery_level` alongside the real fields
    /// fails here instead of being parsed with the extra field ignored.
    #[error("projection payload is malformed: {0}")]
    Malformed(String),
    /// The envelope format version is not the one this build admits.
    #[error("unsupported projection envelope version {found}, expected {expected}")]
    UnsupportedEnvelopeVersion { found: u32, expected: u32 },
    /// The payload dropped the hypothetical marker.
    #[error("projection payload is not marked hypothetical")]
    NotHypothetical,
    /// The payload claimed an authority a projection may never hold.
    #[error("projection payload claimed canonical authority {found:?}")]
    CanonicalAuthorityClaimed { found: AuthorityClass },
    /// The payload claimed an epistemic status a projection may never hold.
    #[error("projection payload claimed canonical epistemic status {found:?}")]
    CanonicalStatusClaimed { found: EpistemicStatus },
    /// The engine version does not match the projection's own provenance.
    #[error("projection engine version {projection} disagrees with provenance {provenance}")]
    EngineVersionMismatch { projection: u32, provenance: u32 },
    /// The inputs digest does not match the projection's own provenance.
    #[error("projection inputs digest disagrees with its workload provenance")]
    InputsDigestMismatch,
    /// A workload range was reversed or longer than a week.
    #[error("projected workload range {low}..={high} is not an ordered in-week range")]
    InvalidWorkloadRange { low: u16, high: u16 },
    /// The declared binding does not cover the payload as it now stands.
    #[error("projection binding does not match the payload it arrived with")]
    BindingMismatch,
}

/// Admits a serialised projection payload, or refuses it.
///
/// The checks run in the order a forger would have to defeat them: parse, then
/// declared shape, then internal agreement, then the binding. A payload that
/// passes every one of them is a projection this build produced from the inputs
/// it names, and it is still only ever a projection.
pub fn admit_projection_payload(
    payload: &[u8],
) -> Result<ScenarioProjection, ProjectionAdmissionError> {
    let envelope: ProjectionEnvelope = serde_json::from_slice(payload)
        .map_err(|error| ProjectionAdmissionError::Malformed(error.to_string()))?;
    if envelope.envelope_version != PROJECTION_ENVELOPE_VERSION {
        return Err(ProjectionAdmissionError::UnsupportedEnvelopeVersion {
            found: envelope.envelope_version,
            expected: PROJECTION_ENVELOPE_VERSION,
        });
    }
    if !envelope.hypothetical {
        return Err(ProjectionAdmissionError::NotHypothetical);
    }
    if envelope.authority_class != ADMITTED_AUTHORITY {
        return Err(ProjectionAdmissionError::CanonicalAuthorityClaimed {
            found: envelope.authority_class,
        });
    }
    if envelope.epistemic_status != ADMITTED_STATUS {
        return Err(ProjectionAdmissionError::CanonicalStatusClaimed {
            found: envelope.epistemic_status,
        });
    }
    let provenance = envelope.projection.workload.provenance();
    if envelope.projection.engine_version != provenance.engine_version() {
        return Err(ProjectionAdmissionError::EngineVersionMismatch {
            projection: envelope.projection.engine_version,
            provenance: provenance.engine_version(),
        });
    }
    if envelope.projection.inputs_digest != provenance.inputs_digest() {
        return Err(ProjectionAdmissionError::InputsDigestMismatch);
    }
    let range = workload_range(&envelope.projection.workload);
    if range.low_hours() > range.high_hours()
        || range.high_hours() > WorkloadHoursRange::MAXIMUM_WEEKLY_HOURS
    {
        return Err(ProjectionAdmissionError::InvalidWorkloadRange {
            low: range.low_hours(),
            high: range.high_hours(),
        });
    }
    let expected = binding_digest(
        envelope.envelope_version,
        envelope.hypothetical,
        envelope.authority_class,
        envelope.epistemic_status,
        &envelope.projection,
    );
    if expected != envelope.binding_digest {
        return Err(ProjectionAdmissionError::BindingMismatch);
    }
    Ok(envelope.projection)
}

/// Reads the sealed range for the bounds check.
///
/// The wrapper has no public accessor, and this module is inside the crate, so
/// the check can be made without adding one.
fn workload_range(workload: &ProjectedWorkloadRange) -> WorkloadHoursRange {
    *workload.sealed_value()
}

/// Computes the binding over every field of the envelope.
///
/// The declarations are inside the digest, not beside it. If they were outside,
/// a forger could take a genuine envelope, flip `hypothetical` to `false`, and
/// keep a binding that still verified.
fn binding_digest(
    envelope_version: u32,
    hypothetical: bool,
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
    projection: &ScenarioProjection,
) -> ContentDigest {
    let mut hasher = Sha256::new();
    let mut field = |bytes: &[u8]| {
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(bytes);
    };
    field(BINDING_DOMAIN.as_bytes());
    field(&envelope_version.to_be_bytes());
    field(if hypothetical { b"true" } else { b"false" });
    field(authority_tag(authority_class).as_bytes());
    field(status_tag(epistemic_status).as_bytes());
    field(projection.scenario_id.as_bytes());
    field(&projection.engine_version.to_be_bytes());
    field(projection.inputs_digest.as_bytes());
    let range = workload_range(&projection.workload);
    field(&range.low_hours().to_be_bytes());
    field(&range.high_hours().to_be_bytes());
    let provenance = projection.workload.provenance();
    field(provenance.model_run_id().as_bytes());
    field(provenance.inputs_digest().as_bytes());
    field(&provenance.engine_version().to_be_bytes());
    field(&provenance.proposed_at().value().to_be_bytes());
    field(
        &u64::try_from(projection.opportunities.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for opportunity in &projection.opportunities {
        field(opportunity.offering_id.as_bytes());
        field(opportunity.concept_entity_id.as_bytes());
        field(kind_tag(opportunity).as_bytes());
        field(likelihood_tag(opportunity).as_bytes());
        field(basis_tag(opportunity).as_bytes());
        field(&opportunity.confidence.value().to_be_bytes());
    }
    ContentDigest::from_sha256_bytes(hasher.finalize().into())
}

/// Stable wire tags for the enums the binding covers.
///
/// The tags are spelled here rather than taken from `Debug`, so a rename of a
/// Rust variant cannot silently change a digest that other builds verify.
const fn authority_tag(authority: AuthorityClass) -> &'static str {
    match authority {
        AuthorityClass::Official => "OFFICIAL",
        AuthorityClass::UserExplicit => "USER_EXPLICIT",
        AuthorityClass::DirectObservation => "DIRECT_OBSERVATION",
        AuthorityClass::DeterministicEngine => "DETERMINISTIC_ENGINE",
        AuthorityClass::Curated => "CURATED",
        AuthorityClass::ModelInference => "MODEL_INFERENCE",
        AuthorityClass::Prediction => "PREDICTION",
        AuthorityClass::Unknown => "UNKNOWN",
    }
}

const fn status_tag(status: EpistemicStatus) -> &'static str {
    match status {
        EpistemicStatus::OfficialConfirmed => "OFFICIAL_CONFIRMED",
        EpistemicStatus::UserConfirmed => "USER_CONFIRMED",
        EpistemicStatus::CodeObserved => "CODE_OBSERVED",
        EpistemicStatus::DeterministicDerived => "DETERMINISTIC_DERIVED",
        EpistemicStatus::AiInferred => "AI_INFERRED",
        EpistemicStatus::Prediction => "PREDICTION",
        EpistemicStatus::Disputed => "DISPUTED",
        EpistemicStatus::Superseded => "SUPERSEDED",
        EpistemicStatus::Unknown => "UNKNOWN",
    }
}

const fn kind_tag(opportunity: &ProjectedEvidenceOpportunity) -> &'static str {
    use crate::opportunity::OpportunityKind;

    match opportunity.kind {
        OpportunityKind::Exposure => "EXPOSURE",
        OpportunityKind::Practice => "PRACTICE",
        OpportunityKind::Assessment => "ASSESSMENT",
    }
}

const fn likelihood_tag(opportunity: &ProjectedEvidenceOpportunity) -> &'static str {
    use crate::opportunity::LikelihoodBand;

    match opportunity.likelihood {
        LikelihoodBand::Unknown => "UNKNOWN",
        LikelihoodBand::Low => "LOW",
        LikelihoodBand::Moderate => "MODERATE",
        LikelihoodBand::High => "HIGH",
    }
}

const fn basis_tag(opportunity: &ProjectedEvidenceOpportunity) -> &'static str {
    use crate::opportunity::OpportunityBasis;

    match opportunity.basis {
        OpportunityBasis::Syllabus => "SYLLABUS",
        OpportunityBasis::AssignmentBrief => "ASSIGNMENT_BRIEF",
        OpportunityBasis::AssessmentPlan => "ASSESSMENT_PLAN",
        OpportunityBasis::HistoricalOffering => "HISTORICAL_OFFERING",
    }
}
