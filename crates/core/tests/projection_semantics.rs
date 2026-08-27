mod support;

use academic_domain::{
    AuthorityClass, ClaimObject, ClaimRelation, ClaimRelationKind, DecisionAction, EpistemicStatus,
    PredicateId, ResolutionSlot, TimestampMillis, UserDecision, ValidInterval,
};
use academic_ledger::AuthorityPolicy;
use academic_projections::{generation::ProjectionKind, runner::ProjectionError};

use support::{
    Fixture, TestResult, claim, claim_id, decision_id, entity, importer_actor, model_actor,
    policies, text_claim,
};

#[test]
fn non_active_canonical_states_never_project_as_truth() -> TestResult {
    let mut fixture = Fixture::new("resolver-filtering")?;
    let base = fixture.register_scope_evidence(5, 1, b"resolver filtering evidence 1")?;
    let mut evidence_ids = vec![base.evidence_id];
    for seed in 2_u16..=14 {
        evidence_ids.push(
            fixture
                .register_evidence(
                    5,
                    base.scope_id,
                    seed,
                    format!("resolver filtering evidence {seed}").as_bytes(),
                )?
                .evidence_id,
        );
    }

    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_001)?,
            entity(1_001)?,
            "state.superseded",
            "supersededleak",
            base.scope_id,
            evidence_ids[0],
            AuthorityClass::DirectObservation,
            EpistemicStatus::Superseded,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_002)?,
            entity(1_002)?,
            "state.disputed",
            "disputedleak",
            base.scope_id,
            evidence_ids[1],
            AuthorityClass::DirectObservation,
            EpistemicStatus::Disputed,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_003)?,
            entity(1_003)?,
            "time.future",
            "futureleak",
            base.scope_id,
            evidence_ids[2],
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            1_000,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_004)?,
            entity(1_004)?,
            "time.expired",
            "expiredleak",
            base.scope_id,
            evidence_ids[3],
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            Some(400),
        )?,
    )?;

    let retracted_subject = entity(1_005)?;
    let retracted_id = claim_id(1_005)?;
    let replacement_id = claim_id(1_006)?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            retracted_id,
            retracted_subject,
            "relation.retracted",
            "retractedleak",
            base.scope_id,
            evidence_ids[4],
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            replacement_id,
            retracted_subject,
            "relation.retracted",
            "replacementtruth",
            base.scope_id,
            evidence_ids[5],
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    fixture.accept_relation(
        importer_actor(),
        base.domain_id,
        ClaimRelation {
            source_claim_id: replacement_id,
            target_claim_id: retracted_id,
            kind: ClaimRelationKind::Retracts,
            scope_id: base.scope_id,
        },
    )?;

    let rejected_subject = entity(1_007)?;
    let rejected_id = claim_id(1_007)?;
    let rejected_object = ClaimObject::Text("userrejectedleak".to_owned());
    fixture.accept_claim(
        fixture.user_actor(),
        base.domain_id,
        claim(
            rejected_id,
            rejected_subject,
            "decision.rejected",
            rejected_object.clone(),
            base.scope_id,
            evidence_ids[6],
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed,
            0,
            None,
        )?,
    )?;
    fixture.accept_decision(
        base.domain_id,
        UserDecision {
            id: decision_id(1_007)?,
            target_claim_id: rejected_id,
            target_object: rejected_object,
            resolution_slot: ResolutionSlot {
                subject_entity_id: rejected_subject,
                predicate_id: PredicateId::parse("decision.rejected")?,
                scope_id: base.scope_id,
            },
            action: DecisionAction::Reject,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
            rationale_evidence_ids: vec![evidence_ids[6]],
            decided_at: TimestampMillis::new(200),
            reversible_until: None,
        },
    )?;

    let authority_subject = entity(1_008)?;
    fixture.accept_claim(
        model_actor(1)?,
        base.domain_id,
        text_claim(
            claim_id(1_008)?,
            authority_subject,
            "authority.fact",
            "lowerauthorityleak",
            base.scope_id,
            evidence_ids[7],
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_009)?,
            authority_subject,
            "authority.fact",
            "officialtruth",
            base.scope_id,
            evidence_ids[8],
            AuthorityClass::Official,
            EpistemicStatus::OfficialConfirmed,
            0,
            None,
        )?,
    )?;

    let equal_subject = entity(1_010)?;
    for (seed, body, evidence_id) in [
        (1_010, "equaloneleak", evidence_ids[9]),
        (1_011, "equaltwoleak", evidence_ids[10]),
    ] {
        fixture.accept_claim(
            importer_actor(),
            base.domain_id,
            text_claim(
                claim_id(seed)?,
                equal_subject,
                "authority.equal",
                body,
                base.scope_id,
                evidence_id,
                AuthorityClass::Official,
                EpistemicStatus::OfficialConfirmed,
                0,
                None,
            )?,
        )?;
    }

    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_012)?,
            entity(1_012)?,
            "mixed.observed",
            "mixedactive",
            base.scope_id,
            evidence_ids[11],
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        base.domain_id,
        text_claim(
            claim_id(1_013)?,
            entity(1_013)?,
            "mixed.official",
            "mixedactive",
            base.scope_id,
            evidence_ids[12],
            AuthorityClass::Official,
            EpistemicStatus::OfficialConfirmed,
            0,
            None,
        )?,
    )?;

    let policies = policies(&[
        ("authority.equal", AuthorityPolicy::OfficialFact),
        ("authority.fact", AuthorityPolicy::OfficialFact),
        ("decision.rejected", AuthorityPolicy::UserOwned),
        ("mixed.observed", AuthorityPolicy::ImplementationObservation),
        ("mixed.official", AuthorityPolicy::OfficialFact),
        (
            "relation.retracted",
            AuthorityPolicy::ImplementationObservation,
        ),
        ("state.disputed", AuthorityPolicy::ImplementationObservation),
        (
            "state.superseded",
            AuthorityPolicy::ImplementationObservation,
        ),
        ("time.expired", AuthorityPolicy::ImplementationObservation),
        ("time.future", AuthorityPolicy::ImplementationObservation),
    ])?;
    let coordinates = fixture.coordinates(500);
    fixture.runner()?.rebuild_at(
        ProjectionKind::Unicode61,
        base.domain_id,
        coordinates,
        &policies,
    )?;
    let reader = fixture.projection_reader()?;
    for forbidden in [
        "supersededleak",
        "disputedleak",
        "futureleak",
        "expiredleak",
        "retractedleak",
        "userrejectedleak",
        "lowerauthorityleak",
        "equaloneleak",
        "equaltwoleak",
    ] {
        let page = reader.search_ranked(
            ProjectionKind::Unicode61,
            base.domain_id,
            coordinates,
            &policies,
            forbidden,
            20,
        )?;
        assert!(page.records.is_empty(), "forbidden token {forbidden}");
    }
    for active in ["replacementtruth", "officialtruth"] {
        assert_eq!(
            reader
                .search_ranked(
                    ProjectionKind::Unicode61,
                    base.domain_id,
                    coordinates,
                    &policies,
                    active,
                    20,
                )?
                .records
                .len(),
            1
        );
    }

    let mixed = reader.search_ranked(
        ProjectionKind::Unicode61,
        base.domain_id,
        coordinates,
        &policies,
        "mixedactive",
        20,
    )?;
    assert_eq!(mixed.records.len(), 2);
    let applied = mixed
        .records
        .iter()
        .map(|record| record.resolution.authority_policy)
        .collect::<Vec<_>>();
    assert!(applied.contains(&AuthorityPolicy::OfficialFact));
    assert!(applied.contains(&AuthorityPolicy::ImplementationObservation));

    let mismatched = academic_projections::resolution::PredicatePolicies::new(
        "projection-test-policies-v1",
        [
            (
                PredicateId::parse("mixed.observed")?,
                AuthorityPolicy::OfficialFact,
            ),
            (
                PredicateId::parse("mixed.official")?,
                AuthorityPolicy::OfficialFact,
            ),
        ],
    )?;
    assert!(matches!(
        reader.search_ranked(
            ProjectionKind::Unicode61,
            base.domain_id,
            coordinates,
            &mismatched,
            "mixedactive",
            20,
        ),
        Err(ProjectionError::AuthorityMismatch(_))
    ));
    assert!(matches!(
        reader.search_ranked(
            ProjectionKind::Unicode61,
            base.domain_id,
            academic_projections::generation::ProjectionCoordinates::new(
                coordinates.known_at_accept_seq,
                TimestampMillis::new(501),
            ),
            &policies,
            "mixedactive",
            20,
        ),
        Err(ProjectionError::AuthorityMismatch(_))
    ));
    Ok(())
}
