mod support;

use academic_ledger::AuthorityPolicy;
use academic_projections::generation::{ProjectionAvailability, ProjectionKind};

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, policies,
};

#[test]
fn graph_drop_and_rebuild_matches() -> TestResult {
    let mut fixture = Fixture::new("graph-drop-rebuild")?;
    let first_evidence = fixture.register_scope_evidence(1, 1, b"synthetic graph evidence one")?;
    let second_evidence = fixture.register_evidence(
        1,
        first_evidence.scope_id,
        2,
        b"synthetic graph evidence two",
    )?;
    let subject = entity(21)?;
    fixture.accept_claim(
        importer_actor(),
        first_evidence.domain_id,
        observed_entity_claim(
            claim_id(11)?,
            subject,
            "graph.dependency",
            entity(31)?,
            first_evidence.scope_id,
            first_evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        first_evidence.domain_id,
        observed_entity_claim(
            claim_id(12)?,
            subject,
            "graph.related",
            entity(32)?,
            first_evidence.scope_id,
            second_evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    let coordinates = fixture.coordinates(100);
    let policies = policies(&[
        (
            "graph.dependency",
            AuthorityPolicy::ImplementationObservation,
        ),
        ("graph.related", AuthorityPolicy::ImplementationObservation),
    ])?;

    let first_receipt = fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        first_evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let first = fixture.projection_reader()?.graph_neighbors(
        first_evidence.domain_id,
        subject,
        coordinates,
        &policies,
    )?;
    assert_eq!(first.records.len(), 2);
    assert!(matches!(
        first.availability,
        ProjectionAvailability::Current { .. }
    ));

    fixture
        .runner()?
        .drop_projection(ProjectionKind::Graph, first_evidence.domain_id)?;
    let dropped = fixture.projection_reader()?.graph_neighbors(
        first_evidence.domain_id,
        subject,
        coordinates,
        &policies,
    )?;
    assert!(matches!(
        dropped.availability,
        ProjectionAvailability::NoActive { .. }
    ));
    assert!(dropped.records.is_empty());

    let second_receipt = fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        first_evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let second = fixture.projection_reader()?.graph_neighbors(
        first_evidence.domain_id,
        subject,
        coordinates,
        &policies,
    )?;
    assert_eq!(
        first_receipt.metadata.canonical_checksum,
        second_receipt.metadata.canonical_checksum
    );
    assert_eq!(first.records.len(), second.records.len());
    for (left, right) in first.records.iter().zip(&second.records) {
        assert_eq!(left.source_entity_id, right.source_entity_id);
        assert_eq!(left.predicate_id, right.predicate_id);
        assert_eq!(left.target_entity_id, right.target_entity_id);
        assert_eq!(left.claim_id, right.claim_id);
        assert_eq!(left.evidence_ids, right.evidence_ids);
        assert_eq!(left.scope_id, right.scope_id);
        assert_eq!(left.domain, right.domain);
        assert_eq!(left.authority_class, right.authority_class);
        assert_eq!(left.epistemic_status, right.epistemic_status);
        assert_eq!(left.valid_time, right.valid_time);
        assert_eq!(left.resolution, right.resolution);
        assert_eq!(
            left.source_record_accept_seq,
            right.source_record_accept_seq
        );
        assert_eq!(left.stable_tiebreaker, right.stable_tiebreaker);
    }
    Ok(())
}
