mod support;

use academic_domain::{ClaimRelation, ClaimRelationKind, ContentDigest, PredicateId};
use academic_ledger::{AuthorityPolicy, ResolutionQuery};
use academic_projections::{
    checksum::order_stable_checksum,
    generation::{ProjectionCoordinates, ProjectionKind},
};
use academic_store::queries::resolve;

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, policies,
};

#[test]
fn projection_checksum_is_order_stable() {
    let first = vec![b"beta".to_vec(), b"alpha".to_vec(), b"gamma".to_vec()];
    let second = vec![b"gamma".to_vec(), b"beta".to_vec(), b"alpha".to_vec()];
    assert_eq!(order_stable_checksum(first), order_stable_checksum(second));
    assert_ne!(
        order_stable_checksum([b"alpha".to_vec(), b"betagamma".to_vec()]),
        order_stable_checksum([b"alphabeta".to_vec(), b"gamma".to_vec()])
    );
    assert_ne!(
        order_stable_checksum([b"alpha".to_vec()]),
        ContentDigest::sha256(b"alpha")
    );
}

#[test]
fn as_known_rebuild_matches_ledger() -> TestResult {
    let mut fixture = Fixture::new("as-known-oracle")?;
    let old_evidence = fixture.register_scope_evidence(1, 1, b"old graph observation")?;
    let new_evidence =
        fixture.register_evidence(1, old_evidence.scope_id, 2, b"new graph observation")?;
    let subject = entity(501)?;
    let predicate = PredicateId::parse("graph.related")?;
    let old_claim_id = claim_id(501)?;
    let new_claim_id = claim_id(502)?;
    let old_known = fixture.accept_claim(
        importer_actor(),
        old_evidence.domain_id,
        observed_entity_claim(
            old_claim_id,
            subject,
            predicate.as_str(),
            entity(511)?,
            old_evidence.scope_id,
            old_evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    let policies = policies(&[(
        predicate.as_str(),
        AuthorityPolicy::ImplementationObservation,
    )])?;
    let historical_coordinates =
        ProjectionCoordinates::new(old_known, academic_domain::TimestampMillis::new(100));
    fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        old_evidence.domain_id,
        historical_coordinates,
        &policies,
    )?;
    assert_projection_matches_canonical(
        &fixture,
        subject,
        &predicate,
        old_evidence.scope_id,
        old_evidence.domain_id,
        historical_coordinates,
        &policies,
    )?;

    fixture.accept_claim(
        importer_actor(),
        old_evidence.domain_id,
        observed_entity_claim(
            new_claim_id,
            subject,
            predicate.as_str(),
            entity(512)?,
            old_evidence.scope_id,
            new_evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    fixture.accept_relation(
        importer_actor(),
        old_evidence.domain_id,
        ClaimRelation {
            source_claim_id: new_claim_id,
            target_claim_id: old_claim_id,
            kind: ClaimRelationKind::Supersedes,
            scope_id: old_evidence.scope_id,
        },
    )?;
    let current_coordinates = fixture.coordinates(100);
    fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        old_evidence.domain_id,
        current_coordinates,
        &policies,
    )?;
    assert_projection_matches_canonical(
        &fixture,
        subject,
        &predicate,
        old_evidence.scope_id,
        old_evidence.domain_id,
        current_coordinates,
        &policies,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn assert_projection_matches_canonical(
    fixture: &Fixture,
    subject: academic_domain::EntityId,
    predicate: &PredicateId,
    scope_id: academic_domain::ScopeId,
    domain_id: academic_domain::DomainId,
    coordinates: ProjectionCoordinates,
    policies: &academic_projections::resolution::PredicatePolicies,
) -> TestResult {
    let canonical = resolve(
        &fixture.store_reader()?,
        &ResolutionQuery {
            subject_entity_id: subject,
            scope_id,
            predicate_id: predicate.clone(),
            valid_at: coordinates.valid_at,
            known_at_accept_seq: coordinates.known_at_accept_seq,
            policy: AuthorityPolicy::ImplementationObservation,
        },
    )?;
    let page =
        fixture
            .projection_reader()?
            .graph_neighbors(domain_id, subject, coordinates, policies)?;
    let projected = page
        .records
        .iter()
        .map(|edge| edge.claim_id)
        .collect::<Vec<_>>();
    assert_eq!(projected, canonical.active_claim_ids);
    Ok(())
}
