use std::{collections::BTreeSet, error::Error, str::FromStr};

use academic_domain::{
    AuthorityClass, Claim, ClaimObject, ContentDigest, DecisionAction, DecisionId, DomainError,
    EntityId, EpistemicStatus, EvidenceId, PredicateId, ResolutionSlot, ScopeId, TimestampMillis,
    UserDecision, ValidInterval,
};
use academic_ledger::{
    ClaimSourceProvenance, ConflictReason, CorroborationReasonCode, IndependenceBasis,
    NEW_EVIDENCE_CONFLICT, NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE, ProductClaimType,
    ProductResolutionQuery, RelationSupportTier, ResolutionClaim, ResolutionDecision,
    ResolutionRelation, ResolverActorKind, SourceIndependenceAttestation, resolve_product_snapshot,
};

fn id<T: FromStr<Err = DomainError>>(suffix: u32) -> Result<T, DomainError> {
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}

fn predicate() -> Result<PredicateId, DomainError> {
    PredicateId::parse("product.authority")
}

fn claim(
    suffix: u32,
    scope_id: ScopeId,
    object: &str,
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
    accept_seq: u64,
) -> Result<ResolutionClaim, DomainError> {
    Ok(ResolutionClaim {
        claim: Claim {
            id: id(suffix)?,
            subject_entity_id: id(1)?,
            predicate_id: predicate()?,
            object: ClaimObject::Text(object.to_owned()),
            scope_id,
            authority_class,
            epistemic_status,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
            evidence_ids: vec![id::<EvidenceId>(900)?],
        },
        accept_seq,
    })
}

fn query(
    claim_type: ProductClaimType,
    scope_id: ScopeId,
    known_at_accept_seq: u64,
) -> Result<ProductResolutionQuery, DomainError> {
    Ok(ProductResolutionQuery {
        subject_entity_id: id::<EntityId>(1)?,
        scope_id,
        predicate_id: predicate()?,
        valid_at: TimestampMillis::new(10),
        known_at_accept_seq,
        claim_type,
    })
}

fn source(claim: &ResolutionClaim, bytes: &[u8]) -> ClaimSourceProvenance {
    ClaimSourceProvenance {
        claim_id: claim.claim.id,
        source_digest: Some(ContentDigest::sha256(bytes)),
    }
}

#[test]
fn authority_differs_by_claim_type() -> Result<(), Box<dyn Error>> {
    let scope = id::<ScopeId>(2)?;
    let claims = vec![
        claim(
            10,
            scope,
            "official",
            AuthorityClass::Official,
            EpistemicStatus::OfficialConfirmed,
            1,
        )?,
        claim(
            11,
            scope,
            "user",
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed,
            2,
        )?,
        claim(
            12,
            scope,
            "direct",
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            3,
        )?,
        claim(
            13,
            scope,
            "curated",
            AuthorityClass::Curated,
            EpistemicStatus::DeterministicDerived,
            4,
        )?,
    ];
    let expected = [
        (
            ProductClaimType::OfficialAcademicFact,
            vec![claims[0].claim.id],
        ),
        (ProductClaimType::PersonalIntent, vec![claims[1].claim.id]),
        (
            ProductClaimType::MasteryQuestionResolution,
            vec![claims[1].claim.id],
        ),
        (
            ProductClaimType::CurrentImplementation,
            vec![claims[2].claim.id],
        ),
        (ProductClaimType::ProjectIntent, vec![claims[3].claim.id]),
        (ProductClaimType::RelationPrerequisite, Vec::new()),
    ];
    for (claim_type, active) in expected {
        let actual =
            resolve_product_snapshot(&query(claim_type, scope, 4)?, &claims, &[], &[], &[], &[]);
        assert_eq!(actual.resolution.active_claim_ids, active, "{claim_type:?}");
    }

    let expected_tables = [
        [800, 400, 600, 350, 500, 200, 100, 0],
        [600, 800, 500, 400, 600, 200, 100, 0],
        [350, 800, 700, 600, 400, 200, 100, 0],
        [400, 600, 800, 350, 400, 200, 100, 0],
        [750, 600, 400, 350, 800, 200, 100, 0],
        [700, 800, 600, 500, 800, 200, 100, 0],
    ];
    let mut distinct_tables = BTreeSet::new();
    for (claim_type, expected_ranks) in ProductClaimType::ALL.into_iter().zip(expected_tables) {
        let actual_ranks: Vec<u16> = claim_type
            .authority_table()
            .entries
            .into_iter()
            .map(|entry| entry.rank)
            .collect();
        assert_eq!(actual_ranks, expected_ranks, "{claim_type:?}");
        distinct_tables.insert(actual_ranks);
    }
    assert_eq!(distinct_tables.len(), ProductClaimType::ALL.len());
    Ok(())
}

#[test]
fn ai_rerun_never_removes_an_override() -> Result<(), Box<dyn Error>> {
    let scope = id::<ScopeId>(2)?;
    let selected = claim(
        20,
        scope,
        "installed-not-used",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        1,
    )?;
    let rerun = claim(
        21,
        scope,
        "used",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        3,
    )?;
    let decision = ResolutionDecision {
        decision: UserDecision {
            id: id::<DecisionId>(22)?,
            target_claim_id: selected.claim.id,
            target_object: selected.claim.object.clone(),
            resolution_slot: ResolutionSlot {
                subject_entity_id: selected.claim.subject_entity_id,
                predicate_id: selected.claim.predicate_id.clone(),
                scope_id: scope,
            },
            action: DecisionAction::Confirm,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
            rationale_evidence_ids: Vec::new(),
            decided_at: TimestampMillis::new(2),
            reversible_until: None,
        },
        accept_seq: 2,
    };
    let superseding_rerun = ResolutionRelation {
        relation: academic_domain::ClaimRelation {
            source_claim_id: rerun.claim.id,
            target_claim_id: selected.claim.id,
            kind: academic_domain::ClaimRelationKind::Supersedes,
            scope_id: scope,
        },
        accept_seq: 4,
        actor_kind: ResolverActorKind::ModelRun,
    };

    let before = resolve_product_snapshot(
        &query(ProductClaimType::PersonalIntent, scope, 2)?,
        std::slice::from_ref(&selected),
        &[],
        std::slice::from_ref(&decision),
        &[],
        &[],
    );
    assert_eq!(before.resolution.active_claim_ids, vec![selected.claim.id]);

    let after = resolve_product_snapshot(
        &query(ProductClaimType::PersonalIntent, scope, 4)?,
        &[selected.clone(), rerun.clone()],
        &[superseding_rerun],
        &[decision],
        &[],
        &[],
    );
    assert_eq!(after.resolution.active_claim_ids, vec![selected.claim.id]);
    assert_eq!(after.resolution.conflicting_claim_ids, vec![rerun.claim.id]);
    Ok(())
}

#[test]
fn contrary_evidence_creates_a_conflict_card() -> Result<(), Box<dyn Error>> {
    let scope = id::<ScopeId>(2)?;
    let confirmed = claim(
        30,
        scope,
        "understood",
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        1,
    )?;
    let contrary = claim(
        31,
        scope,
        "unresolved",
        AuthorityClass::DirectObservation,
        EpistemicStatus::CodeObserved,
        2,
    )?;
    let before = resolve_product_snapshot(
        &query(ProductClaimType::MasteryQuestionResolution, scope, 1)?,
        std::slice::from_ref(&confirmed),
        &[],
        &[],
        &[],
        &[],
    );
    assert!(before.conflict_cards.is_empty());

    let after = resolve_product_snapshot(
        &query(ProductClaimType::MasteryQuestionResolution, scope, 2)?,
        &[confirmed.clone(), contrary.clone()],
        &[],
        &[],
        &[],
        &[],
    );
    assert_eq!(after.resolution.active_claim_ids, vec![confirmed.claim.id]);
    assert_eq!(after.conflict_cards.len(), 1);
    assert_eq!(
        after.conflict_cards[0].reason.canonical_token(),
        NEW_EVIDENCE_CONFLICT
    );
    assert_eq!(
        ConflictReason::from_token(NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE),
        Some(ConflictReason::NewEvidenceConflict)
    );
    assert_eq!(
        ConflictReason::from_token(NEW_EVIDENCE_CONFLICT),
        Some(ConflictReason::NewEvidenceConflict)
    );
    assert_eq!(
        after.conflict_cards[0].conflicting_claim_ids,
        vec![contrary.claim.id]
    );
    Ok(())
}

#[test]
fn official_fact_is_not_mutated_by_user_dispute() -> Result<(), Box<dyn Error>> {
    let scope = id::<ScopeId>(2)?;
    let official = claim(
        40,
        scope,
        "applies-globally",
        AuthorityClass::Official,
        EpistemicStatus::OfficialConfirmed,
        1,
    )?;
    let disputed = claim(
        41,
        scope,
        "not-applicable-to-me",
        AuthorityClass::UserExplicit,
        EpistemicStatus::Disputed,
        2,
    )?;
    let unchanged = official.clone();
    let result = resolve_product_snapshot(
        &query(ProductClaimType::OfficialAcademicFact, scope, 2)?,
        &[official.clone(), disputed.clone()],
        &[],
        &[],
        &[],
        &[],
    );
    assert_eq!(official, unchanged);
    assert_eq!(result.resolution.active_claim_ids, vec![official.claim.id]);
    assert_eq!(
        result.resolution.conflicting_claim_ids,
        vec![disputed.claim.id]
    );
    Ok(())
}

#[test]
fn scoped_relations_coexist_without_promotion() -> Result<(), Box<dyn Error>> {
    let first_scope = id::<ScopeId>(2)?;
    let second_scope = id::<ScopeId>(3)?;
    let first = claim(
        50,
        first_scope,
        "helpful",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        1,
    )?;
    let second = claim(
        51,
        second_scope,
        "required",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        2,
    )?;
    let claims = [first.clone(), second.clone()];
    let provenance = [source(&first, b"one"), source(&second, b"two")];

    for (scope, expected) in [
        (first_scope, first.claim.id),
        (second_scope, second.claim.id),
    ] {
        let result = resolve_product_snapshot(
            &query(ProductClaimType::RelationPrerequisite, scope, 2)?,
            &claims,
            &[],
            &[],
            &provenance,
            &[],
        );
        assert_eq!(result.resolution.active_claim_ids, vec![expected]);
        assert_eq!(result.relation_support.len(), 1);
        assert_eq!(
            result.relation_support[0].tier,
            RelationSupportTier::SingleSourceInference
        );
    }
    Ok(())
}

#[test]
fn duplicated_upstream_sources_count_as_one_corroboration() -> Result<(), Box<dyn Error>> {
    let scope = id::<ScopeId>(2)?;
    let first = claim(
        60,
        scope,
        "relation-a",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        1,
    )?;
    let second = claim(
        61,
        scope,
        "relation-a",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        2,
    )?;
    let competitor = claim(
        62,
        scope,
        "relation-b",
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        3,
    )?;
    let claims = [first.clone(), second.clone(), competitor.clone()];

    let same_digest = ContentDigest::sha256(b"same-upstream");
    let duplicated = [
        ClaimSourceProvenance {
            claim_id: first.claim.id,
            source_digest: Some(same_digest),
        },
        ClaimSourceProvenance {
            claim_id: second.claim.id,
            source_digest: Some(same_digest),
        },
        source(&competitor, b"competitor"),
    ];
    let duplicate_result = resolve_product_snapshot(
        &query(ProductClaimType::RelationPrerequisite, scope, 3)?,
        &claims,
        &[],
        &[],
        &duplicated,
        &[SourceIndependenceAttestation {
            first_claim_id: first.claim.id,
            second_claim_id: second.claim.id,
            basis: IndependenceBasis::DistinctSignedOrigins,
        }],
    );
    let duplicate_assessment = duplicate_result
        .relation_support
        .iter()
        .find(|assessment| assessment.object == first.claim.object)
        .ok_or("missing duplicate assessment")?;
    assert_eq!(
        duplicate_assessment.tier,
        RelationSupportTier::SingleSourceInference
    );
    assert!(
        duplicate_assessment
            .reason_codes
            .contains(&CorroborationReasonCode::DuplicateUpstreamSource)
    );
    assert!(duplicate_result.resolution.active_claim_ids.is_empty());

    let different_but_unestablished = [
        source(&first, b"reformatted-upstream-a"),
        source(&second, b"reformatted-upstream-b"),
        source(&competitor, b"competitor"),
    ];
    let unestablished_result = resolve_product_snapshot(
        &query(ProductClaimType::RelationPrerequisite, scope, 3)?,
        &claims,
        &[],
        &[],
        &different_but_unestablished,
        &[],
    );
    let unestablished_assessment = unestablished_result
        .relation_support
        .iter()
        .find(|assessment| assessment.object == first.claim.object)
        .ok_or("missing unestablished assessment")?;
    assert_eq!(
        unestablished_assessment.tier,
        RelationSupportTier::SingleSourceInference
    );
    assert!(
        unestablished_assessment
            .reason_codes
            .contains(&CorroborationReasonCode::IndependenceUnestablished)
    );
    assert!(unestablished_result.resolution.active_claim_ids.is_empty());

    for basis in [
        IndependenceBasis::DistinctSignedOrigins,
        IndependenceBasis::SeparateDirectObservations,
    ] {
        let established = SourceIndependenceAttestation {
            first_claim_id: first.claim.id,
            second_claim_id: second.claim.id,
            basis,
        };
        let corroborated_result = resolve_product_snapshot(
            &query(ProductClaimType::RelationPrerequisite, scope, 3)?,
            &claims,
            &[],
            &[],
            &different_but_unestablished,
            &[established],
        );
        let corroborated_assessment = corroborated_result
            .relation_support
            .iter()
            .find(|assessment| assessment.object == first.claim.object)
            .ok_or("missing corroborated assessment")?;
        assert_eq!(
            corroborated_assessment.tier,
            RelationSupportTier::CorroboratedInference
        );
        assert_eq!(
            corroborated_result.resolution.active_claim_ids,
            vec![first.claim.id, second.claim.id]
        );
    }

    let missing_digest = [
        ClaimSourceProvenance {
            claim_id: first.claim.id,
            source_digest: None,
        },
        source(&second, b"second"),
        source(&competitor, b"competitor"),
    ];
    let missing_result = resolve_product_snapshot(
        &query(ProductClaimType::RelationPrerequisite, scope, 3)?,
        &claims,
        &[],
        &[],
        &missing_digest,
        &[SourceIndependenceAttestation {
            first_claim_id: first.claim.id,
            second_claim_id: second.claim.id,
            basis: IndependenceBasis::DistinctSignedOrigins,
        }],
    );
    let missing_assessment = missing_result
        .relation_support
        .iter()
        .find(|assessment| assessment.object == first.claim.object)
        .ok_or("missing absent-digest assessment")?;
    assert_eq!(
        missing_assessment.tier,
        RelationSupportTier::SingleSourceInference
    );
    assert!(
        missing_assessment
            .reason_codes
            .contains(&CorroborationReasonCode::MissingSourceDigest)
    );
    Ok(())
}

#[test]
fn terminal_status_is_never_active() -> Result<(), Box<dyn Error>> {
    let scope = id::<ScopeId>(2)?;
    let active = claim(
        70,
        scope,
        "active",
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        1,
    )?;
    let disputed = claim(
        71,
        scope,
        "disputed",
        AuthorityClass::ModelInference,
        EpistemicStatus::Disputed,
        2,
    )?;
    let superseded = claim(
        72,
        scope,
        "superseded",
        AuthorityClass::Official,
        EpistemicStatus::Superseded,
        3,
    )?;
    let result = resolve_product_snapshot(
        &query(ProductClaimType::PersonalIntent, scope, 3)?,
        &[active.clone(), disputed.clone(), superseded.clone()],
        &[],
        &[],
        &[],
        &[],
    );
    assert_eq!(result.resolution.active_claim_ids, vec![active.claim.id]);
    assert_eq!(
        result.resolution.conflicting_claim_ids,
        vec![disputed.claim.id]
    );
    assert_eq!(
        result.resolution.rejected_claim_ids,
        vec![superseded.claim.id]
    );
    assert!(
        !result
            .resolution
            .active_claim_ids
            .contains(&disputed.claim.id)
    );
    assert!(
        !result
            .resolution
            .active_claim_ids
            .contains(&superseded.claim.id)
    );
    Ok(())
}
