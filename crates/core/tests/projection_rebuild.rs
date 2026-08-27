mod support;

use academic_domain::{
    AuthorityClass, ClaimId, ClaimRelation, ClaimRelationKind, ContentDigest, DomainId,
    EpistemicStatus, EventPayload, PredicateId, ScopeId,
};
use academic_ledger::{AuthorityPolicy, ResolutionQuery};
use academic_projections::{
    checksum::order_stable_checksum,
    generation::{GenerationId, ProjectionAvailability, ProjectionCoordinates, ProjectionKind},
    resolution::PredicatePolicies,
};
use academic_store::queries::resolve;

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, policies,
    text_claim,
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
    let old_evidence = fixture.register_scope_evidence(1, 1, b"old projection observation")?;
    let new_evidence =
        fixture.register_evidence(1, old_evidence.scope_id, 2, b"new projection observation")?;
    let graph_subject = entity(501)?;
    let text_subject = entity(502)?;
    let graph_predicate = PredicateId::parse("graph.related")?;
    let text_predicate = PredicateId::parse("code.symbol")?;
    let old_graph_id = claim_id(501)?;
    let new_graph_id = claim_id(502)?;
    let old_text_id = claim_id(503)?;
    let new_text_id = claim_id(504)?;
    let old_known = fixture.accept_payloads(
        importer_actor(),
        old_evidence.domain_id,
        vec![
            EventPayload::ClaimAsserted(observed_entity_claim(
                old_graph_id,
                graph_subject,
                graph_predicate.as_str(),
                entity(511)?,
                old_evidence.scope_id,
                old_evidence.evidence_id,
                0,
                None,
            )?),
            EventPayload::ClaimAsserted(text_claim(
                old_text_id,
                text_subject,
                text_predicate.as_str(),
                "ProjectionOracleLegacy",
                old_evidence.scope_id,
                old_evidence.evidence_id,
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                0,
                None,
            )?),
        ],
    )?;
    fixture.accept_payloads(
        importer_actor(),
        old_evidence.domain_id,
        vec![
            EventPayload::ClaimAsserted(observed_entity_claim(
                new_graph_id,
                graph_subject,
                graph_predicate.as_str(),
                entity(512)?,
                old_evidence.scope_id,
                new_evidence.evidence_id,
                0,
                None,
            )?),
            EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: new_graph_id,
                target_claim_id: old_graph_id,
                kind: ClaimRelationKind::Supersedes,
                scope_id: old_evidence.scope_id,
            }),
            EventPayload::ClaimAsserted(text_claim(
                new_text_id,
                text_subject,
                text_predicate.as_str(),
                "ProjectionOracleCurrent",
                old_evidence.scope_id,
                new_evidence.evidence_id,
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                0,
                None,
            )?),
            EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: new_text_id,
                target_claim_id: old_text_id,
                kind: ClaimRelationKind::Supersedes,
                scope_id: old_evidence.scope_id,
            }),
        ],
    )?;
    let policies = policies(&[
        (
            graph_predicate.as_str(),
            AuthorityPolicy::ImplementationObservation,
        ),
        (
            text_predicate.as_str(),
            AuthorityPolicy::ImplementationObservation,
        ),
    ])?;
    let historical_coordinates =
        ProjectionCoordinates::new(old_known, academic_domain::TimestampMillis::new(100));
    let current_coordinates = fixture.coordinates(100);
    let runner = fixture.runner()?;
    for kind in [
        ProjectionKind::Graph,
        ProjectionKind::Unicode61,
        ProjectionKind::Trigram,
    ] {
        assert!(
            runner
                .rebuild_at(kind, old_evidence.domain_id, current_coordinates, &policies)?
                .activated
        );
    }
    let current_ids = assert_projection_matches_canonical(
        &fixture,
        graph_subject,
        text_subject,
        &graph_predicate,
        &text_predicate,
        old_evidence.scope_id,
        old_evidence.domain_id,
        current_coordinates,
        &policies,
        "ProjectionOracleCurrent",
        ExpectedAvailability::Current,
    )?;
    for kind in [
        ProjectionKind::Graph,
        ProjectionKind::Unicode61,
        ProjectionKind::Trigram,
    ] {
        assert!(
            !runner
                .rebuild_at(
                    kind,
                    old_evidence.domain_id,
                    historical_coordinates,
                    &policies,
                )?
                .activated
        );
    }
    assert_projection_matches_canonical(
        &fixture,
        graph_subject,
        text_subject,
        &graph_predicate,
        &text_predicate,
        old_evidence.scope_id,
        old_evidence.domain_id,
        historical_coordinates,
        &policies,
        "ProjectionOracleLegacy",
        ExpectedAvailability::Historical(current_ids),
    )?;
    let current_ids_after = assert_projection_matches_canonical(
        &fixture,
        graph_subject,
        text_subject,
        &graph_predicate,
        &text_predicate,
        old_evidence.scope_id,
        old_evidence.domain_id,
        current_coordinates,
        &policies,
        "ProjectionOracleCurrent",
        ExpectedAvailability::Current,
    )?;
    assert_eq!(current_ids_after, current_ids);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ExpectedAvailability {
    Current,
    Historical([GenerationId; 3]),
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn assert_projection_matches_canonical(
    fixture: &Fixture,
    graph_subject: academic_domain::EntityId,
    text_subject: academic_domain::EntityId,
    graph_predicate: &PredicateId,
    text_predicate: &PredicateId,
    scope_id: ScopeId,
    domain_id: DomainId,
    coordinates: ProjectionCoordinates,
    policies: &PredicatePolicies,
    exact_symbol: &str,
    expected_availability: ExpectedAvailability,
) -> TestResult<[GenerationId; 3]> {
    let canonical_graph = resolve(
        &fixture.store_reader()?,
        &ResolutionQuery {
            subject_entity_id: graph_subject,
            scope_id,
            predicate_id: graph_predicate.clone(),
            valid_at: coordinates.valid_at,
            known_at_accept_seq: coordinates.known_at_accept_seq,
            policy: AuthorityPolicy::ImplementationObservation,
        },
    )?;
    let canonical_text = resolve(
        &fixture.store_reader()?,
        &ResolutionQuery {
            subject_entity_id: text_subject,
            scope_id,
            predicate_id: text_predicate.clone(),
            valid_at: coordinates.valid_at,
            known_at_accept_seq: coordinates.known_at_accept_seq,
            policy: AuthorityPolicy::ImplementationObservation,
        },
    )?;
    let reader = fixture.projection_reader()?;
    let graph = reader.graph_neighbors(domain_id, graph_subject, coordinates, policies)?;
    let graph_projected = graph
        .records
        .iter()
        .map(|edge| edge.claim_id)
        .collect::<Vec<_>>();
    assert_eq!(graph_projected, canonical_graph.active_claim_ids);
    let graph_generation = selected_generation(
        &graph.availability,
        expected_availability,
        ProjectionKind::Graph,
    )?;

    let mut generation_ids = [graph_generation, graph_generation, graph_generation];
    for (index, kind) in [ProjectionKind::Unicode61, ProjectionKind::Trigram]
        .into_iter()
        .enumerate()
    {
        let ranked =
            reader.search_ranked(kind, domain_id, coordinates, policies, exact_symbol, 10)?;
        let ranked_ids = ranked
            .records
            .iter()
            .map(|hit| hit.claim_id)
            .collect::<Vec<_>>();
        assert_eq!(ranked_ids, canonical_text.active_claim_ids);
        let ranked_generation =
            selected_generation(&ranked.availability, expected_availability, kind)?;

        let exact =
            reader.exact_symbol_lookup(kind, domain_id, coordinates, policies, exact_symbol)?;
        let exact_ids = exact
            .records
            .iter()
            .map(|hit| hit.claim_id)
            .collect::<Vec<ClaimId>>();
        assert_eq!(exact_ids, canonical_text.active_claim_ids);
        assert_eq!(
            selected_generation(&exact.availability, expected_availability, kind)?,
            ranked_generation
        );
        generation_ids[index + 1] = ranked_generation;
    }
    Ok(generation_ids)
}

fn selected_generation(
    availability: &ProjectionAvailability,
    expected: ExpectedAvailability,
    kind: ProjectionKind,
) -> TestResult<GenerationId> {
    match (availability, expected) {
        (ProjectionAvailability::Current { active }, ExpectedAvailability::Current) => {
            Ok(active.generation_id)
        }
        (
            ProjectionAvailability::Historical {
                generation,
                current_generation_id,
                ..
            },
            ExpectedAvailability::Historical(current),
        ) => {
            let index = match kind {
                ProjectionKind::Graph => 0,
                ProjectionKind::Unicode61 => 1,
                ProjectionKind::Trigram => 2,
            };
            assert_eq!(*current_generation_id, Some(current[index]));
            assert_ne!(generation.generation_id, current[index]);
            Ok(generation.generation_id)
        }
        (actual, expected) => Err(format!("expected {expected:?}, observed {actual:?}").into()),
    }
}
