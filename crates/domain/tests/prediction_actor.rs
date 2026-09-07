use academic_domain::{
    Actor, AuthorityClass as A, Claim, ClaimObject, ConfidencePermille, ContentDigest,
    EpistemicStatus as S, PredictionMetadata, PredictionObservationWindow, TimestampMillis,
    ValidInterval,
};
use std::error::Error;

#[test]
fn total_actor_authority_status_matrix() -> Result<(), Box<dyn Error>> {
    let actors = [
        Actor::User {
            user_id: "01900000-0000-7000-8000-000000000001".parse()?,
        },
        Actor::DeterministicEngine {
            name: "engine".into(),
            version: "1".into(),
        },
        Actor::ModelRun {
            run_id: "01900000-0000-7000-8000-000000000002".parse()?,
        },
        Actor::Importer {
            name: "importer".into(),
            version: "1".into(),
        },
        Actor::DeterministicPrediction {
            name: "offering.forecast".into(),
            version: "1".into(),
            frozen_inputs_digest: ContentDigest::sha256(b"synthetic history"),
            rule_set_digest: ContentDigest::sha256(b"synthetic rule"),
        },
    ];
    let authorities = [
        A::Official,
        A::UserExplicit,
        A::DirectObservation,
        A::DeterministicEngine,
        A::Curated,
        A::ModelInference,
        A::Prediction,
        A::Unknown,
    ];
    let statuses = [
        S::OfficialConfirmed,
        S::UserConfirmed,
        S::CodeObserved,
        S::DeterministicDerived,
        S::AiInferred,
        S::Prediction,
        S::Disputed,
        S::Superseded,
        S::Unknown,
    ];
    let active_pairs = [
        (A::Official, S::OfficialConfirmed),
        (A::UserExplicit, S::UserConfirmed),
        (A::DirectObservation, S::CodeObserved),
        (A::DeterministicEngine, S::DeterministicDerived),
        (A::Curated, S::DeterministicDerived),
        (A::ModelInference, S::AiInferred),
        (A::Prediction, S::Prediction),
        (A::Unknown, S::Unknown),
    ];
    let owned = [
        vec![A::UserExplicit],
        vec![A::DeterministicEngine],
        vec![A::ModelInference, A::Prediction],
        vec![A::Official, A::DirectObservation, A::Curated, A::Unknown],
        vec![A::Prediction],
    ];
    let mut cells = 0;
    for (index, actor) in actors.iter().enumerate() {
        for authority in authorities {
            for status in statuses {
                let prediction = status == S::Prediction;
                let claim = Claim {
                    id: "01900000-0000-7000-8000-000000000003".parse()?,
                    subject_entity_id: "01900000-0000-7000-8000-000000000004".parse()?,
                    predicate_id: academic_domain::PredicateId::parse("academic.offering.status")?,
                    object: ClaimObject::Boolean(true),
                    scope_id: "01900000-0000-7000-8000-000000000005".parse()?,
                    authority_class: authority,
                    epistemic_status: status,
                    confidence: prediction
                        .then(|| ConfidencePermille::new(720))
                        .transpose()?,
                    prediction_metadata: prediction
                        .then(|| {
                            PredictionMetadata::new(
                                PredictionObservationWindow::new(
                                    TimestampMillis::new(10),
                                    TimestampMillis::new(20),
                                )?,
                                1,
                            )
                        })
                        .transpose()?,
                    valid_time: ValidInterval::new(
                        TimestampMillis::new(100),
                        Some(TimestampMillis::new(200)),
                    )?,
                    evidence_ids: vec!["01900000-0000-7000-8000-000000000006".parse()?],
                };
                let expected = owned[index].contains(&authority)
                    && (active_pairs.contains(&(authority, status))
                        || matches!(status, S::Disputed | S::Superseded))
                    && (index != 4 || status == S::Prediction);
                assert_eq!(
                    claim.validate_for_actor(actor).is_ok(),
                    expected,
                    "{} {authority:?} {status:?}",
                    actor.kind_name()
                );
                cells += 1;
            }
        }
    }
    assert_eq!(cells, 360);
    Ok(())
}
