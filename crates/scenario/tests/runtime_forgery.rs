//! The runtime half of the isolation.
//!
//! The compile-fail suite proves no *expression* carries a projected value into
//! an actual-state write. Bytes are not expressions: a payload can spell any
//! field, so every projection that arrives as bytes is re-checked here before
//! anything downstream is allowed to treat it as a projection at all.

use std::error::Error;

use academic_domain::{ContentDigest, EntityId, ModelRunId, OfferingId, TimestampMillis};
use academic_scenario::{
    LikelihoodBand, OpportunityBasis, ProjectionAdmissionError, ProjectionEnvelope, ScenarioChoice,
    ScenarioInputs, SyllabusConceptSignal, WorkloadHoursRange, project,
};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn id<T: std::str::FromStr>(suffix: u16) -> Result<T, T::Err> {
    format!("01936f2a-0000-7000-8000-{suffix:012x}").parse()
}

fn digest(seed: u8) -> Result<ContentDigest, academic_domain::DomainError> {
    format!("sha256:{}", hex_pair(seed).repeat(32)).parse()
}

fn hex_pair(seed: u8) -> String {
    format!("{seed:02x}")
}

fn inputs() -> TestResult<ScenarioInputs> {
    Ok(ScenarioInputs {
        scenario_id: id::<EntityId>(1)?,
        model_run_id: id::<ModelRunId>(2)?,
        knowledge_state_as_of: TimestampMillis::new(1_760_000_000_000),
        requirement_set_digest: digest(0x11)?,
        offering_catalog_digest: digest(0x22)?,
        choices: vec![
            ScenarioChoice {
                offering_id: id::<OfferingId>(0x10)?,
                credit_units: 3,
                assumed_weekly_hours: WorkloadHoursRange::new(9, 14)?,
                syllabus_concepts: vec![
                    SyllabusConceptSignal {
                        concept_entity_id: id::<EntityId>(0x100)?,
                        basis: OpportunityBasis::Syllabus,
                        coverage_permille: 400,
                        assessed: true,
                    },
                    SyllabusConceptSignal {
                        concept_entity_id: id::<EntityId>(0x101)?,
                        basis: OpportunityBasis::AssignmentBrief,
                        coverage_permille: 150,
                        assessed: false,
                    },
                ],
            },
            ScenarioChoice {
                offering_id: id::<OfferingId>(0x11)?,
                credit_units: 3,
                assumed_weekly_hours: WorkloadHoursRange::new(11, 18)?,
                syllabus_concepts: vec![SyllabusConceptSignal {
                    concept_entity_id: id::<EntityId>(0x102)?,
                    basis: OpportunityBasis::HistoricalOffering,
                    coverage_permille: 50,
                    assessed: false,
                }],
            },
        ],
        assumptions: vec![],
    })
}

fn genuine_payload() -> TestResult<Value> {
    let envelope = ProjectionEnvelope::seal(project(&inputs()?)?);
    Ok(serde_json::to_value(&envelope)?)
}

/// Applies one forgery to a genuine payload and returns the refusal.
fn admit(payload: &Value) -> Result<(), ProjectionAdmissionError> {
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    academic_scenario::admit_projection_payload(&bytes).map(|_| ())
}

/// `forged_projected_payload_is_rejected_at_runtime`.
///
/// Each case is a payload a forger would actually write: keep the shape a
/// reader expects and change the one field that turns a projection into a
/// statement of fact. The genuine payload is admitted first and last, so a
/// refusal here is a refusal of the forgery and not of the format.
#[test]
fn forged_projected_payload_is_rejected_at_runtime() -> TestResult {
    let genuine = genuine_payload()?;
    assert_eq!(
        admit(&genuine),
        Ok(()),
        "a genuine sealed projection must be admitted, or every rejection below is vacuous"
    );

    // Dropping the hypothetical marker is the whole forgery in one field: it
    // turns "this might happen" into "this is so".
    let mut forged = genuine.clone();
    forged["hypothetical"] = json!(false);
    assert_eq!(
        admit(&forged),
        Err(ProjectionAdmissionError::NotHypothetical)
    );

    // Claiming an authority that asserts something was seen, decided, or
    // derived from facts. Every canonical class is refused by name.
    for authority in [
        "OFFICIAL",
        "USER_EXPLICIT",
        "DIRECT_OBSERVATION",
        "DETERMINISTIC_ENGINE",
        "CURATED",
        "MODEL_INFERENCE",
        "UNKNOWN",
    ] {
        let mut forged = genuine.clone();
        forged["authority_class"] = json!(authority);
        assert!(
            matches!(
                admit(&forged),
                Err(ProjectionAdmissionError::CanonicalAuthorityClaimed { .. })
            ),
            "authority {authority} must be refused"
        );
    }

    // The same forgery from the status side.
    for status in [
        "OFFICIAL_CONFIRMED",
        "USER_CONFIRMED",
        "CODE_OBSERVED",
        "DETERMINISTIC_DERIVED",
        "AI_INFERRED",
        "DISPUTED",
        "SUPERSEDED",
        "UNKNOWN",
    ] {
        let mut forged = genuine.clone();
        forged["epistemic_status"] = json!(status);
        assert!(
            matches!(
                admit(&forged),
                Err(ProjectionAdmissionError::CanonicalStatusClaimed { .. })
            ),
            "status {status} must be refused"
        );
    }

    // Smuggling an attained mastery level alongside the fields a reader
    // expects. `deny_unknown_fields` makes this a refusal rather than a parse
    // that silently drops the extra key and admits the rest.
    let mut forged = genuine.clone();
    forged["projection"]["mastery_level"] = json!("FLUENT");
    assert!(matches!(
        admit(&forged),
        Err(ProjectionAdmissionError::Malformed(_))
    ));

    let mut forged = genuine.clone();
    forged["projection"]["opportunities"][0]["mastery_level"] = json!("APPLIED");
    assert!(matches!(
        admit(&forged),
        Err(ProjectionAdmissionError::Malformed(_))
    ));

    // Editing the numbers inside an otherwise genuine envelope. The binding
    // covers the projection, so an edit no longer verifies.
    let mut forged = genuine.clone();
    forged["projection"]["workload"]["range"]["high_hours"] = json!(60);
    assert_eq!(
        admit(&forged),
        Err(ProjectionAdmissionError::BindingMismatch)
    );

    let mut forged = genuine.clone();
    forged["projection"]["opportunities"][0]["likelihood"] = json!("HIGH");
    forged["projection"]["opportunities"][1]["likelihood"] = json!("HIGH");
    assert_eq!(
        admit(&forged),
        Err(ProjectionAdmissionError::BindingMismatch)
    );

    let mut forged = genuine.clone();
    forged["projection"]["opportunities"][0]["confidence"] = json!(1000);
    assert_eq!(
        admit(&forged),
        Err(ProjectionAdmissionError::BindingMismatch)
    );

    // Re-attributing a genuine projection to a different plan.
    let mut forged = genuine.clone();
    forged["projection"]["scenario_id"] = json!(id::<EntityId>(0x7f)?.to_string());
    assert_eq!(
        admit(&forged),
        Err(ProjectionAdmissionError::BindingMismatch)
    );

    // Replaying a projection under a version this build does not admit.
    let mut forged = genuine.clone();
    forged["envelope_version"] = json!(2);
    assert!(matches!(
        admit(&forged),
        Err(ProjectionAdmissionError::UnsupportedEnvelopeVersion { .. })
    ));

    // Detaching a projection from the inputs it names, so a stale projection
    // could be presented as the answer to a newer question.
    let mut forged = genuine.clone();
    forged["projection"]["inputs_digest"] = json!(digest(0x33)?.to_string());
    assert_eq!(
        admit(&forged),
        Err(ProjectionAdmissionError::InputsDigestMismatch)
    );

    let mut forged = genuine.clone();
    forged["projection"]["engine_version"] = json!(9);
    assert!(matches!(
        admit(&forged),
        Err(ProjectionAdmissionError::EngineVersionMismatch { .. })
    ));

    // A range no week can hold, which would read as an impossible commitment.
    let mut forged = genuine.clone();
    forged["projection"]["workload"]["range"]["low_hours"] = json!(46);
    forged["projection"]["workload"]["range"]["high_hours"] = json!(34);
    assert!(matches!(
        admit(&forged),
        Err(ProjectionAdmissionError::InvalidWorkloadRange { .. })
    ));

    // The stated limit of the binding. It is a digest, not a signature, so a
    // forger who re-seals from scratch produces a payload that verifies. That
    // is the correct outcome and not a hole: what such a forger has produced is
    // an invented *projection*, and admission never says a projection is true.
    // Impersonating a fact still requires the declarations above, and those are
    // refused by name before the binding is reached.
    let mut invented = project(&inputs()?)?;
    for opportunity in &mut invented.opportunities {
        opportunity.likelihood = LikelihoodBand::High;
    }
    let resealed = serde_json::to_value(ProjectionEnvelope::seal(invented))?;
    assert_eq!(
        admit(&resealed),
        Ok(()),
        "a re-sealed projection is admitted as a projection"
    );
    assert_eq!(resealed["hypothetical"], json!(true));
    assert_eq!(resealed["authority_class"], json!("PREDICTION"));
    assert_eq!(resealed["epistemic_status"], json!("PREDICTION"));

    // Truncated and empty bytes fail closed rather than defaulting.
    assert!(matches!(
        academic_scenario::admit_projection_payload(b""),
        Err(ProjectionAdmissionError::Malformed(_))
    ));
    assert!(matches!(
        academic_scenario::admit_projection_payload(b"{\"envelope_version\":1}"),
        Err(ProjectionAdmissionError::Malformed(_))
    ));

    assert_eq!(
        admit(&genuine),
        Ok(()),
        "the genuine payload must still be admitted after every forgery"
    );
    Ok(())
}
