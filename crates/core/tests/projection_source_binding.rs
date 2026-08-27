mod support;

use academic_domain::{AuthorityClass, BatchId, EpistemicStatus, EventPayload, TimestampMillis};
use academic_ledger::AuthorityPolicy;
use academic_projections::{
    generation::{ProjectionCoordinates, ProjectionKind},
    query::ProjectionReader,
    runner::ProjectionError,
};
use academic_store::queries::{
    QueryError, batch_material, canonical_snapshot, projection_source_authority,
};
use rusqlite::Connection;

use support::{
    Fixture, TestResult, claim_id, entity, id, importer_actor, observed_entity_claim, policies,
    text_claim,
};

#[test]
fn equal_head_sidecar_swap_fails_every_query_path_even_inside_a_batch() -> TestResult {
    let mut ledger_a = Fixture::new("source-ledger-a")?;
    let mut ledger_b = Fixture::new("source-ledger-b")?;
    let evidence_a = ledger_a.register_scope_evidence(9, 1, b"source binding evidence")?;
    let evidence_b = ledger_b.register_scope_evidence(9, 1, b"source binding evidence")?;
    assert_eq!(evidence_a.domain_id, evidence_b.domain_id);
    assert_eq!(evidence_a.scope_id, evidence_b.scope_id);
    assert_eq!(evidence_a.evidence_id, evidence_b.evidence_id);

    let graph_subject = entity(9_001)?;
    let text_subject = entity(9_002)?;
    let graph_claim = observed_entity_claim(
        claim_id(9_001)?,
        graph_subject,
        "graph.related",
        entity(9_011)?,
        evidence_a.scope_id,
        evidence_a.evidence_id,
        0,
        None,
    )?;
    let symbol_claim = text_claim(
        claim_id(9_002)?,
        text_subject,
        "code.symbol",
        "LedgerBoundSymbol",
        evidence_a.scope_id,
        evidence_a.evidence_id,
        AuthorityClass::DirectObservation,
        EpistemicStatus::CodeObserved,
        0,
        None,
    )?;
    let accepted_end_a = ledger_a.accept_payloads(
        importer_actor(),
        evidence_a.domain_id,
        vec![
            EventPayload::ClaimAsserted(graph_claim.clone()),
            EventPayload::ClaimAsserted(symbol_claim.clone()),
            EventPayload::ClaimAsserted(text_claim(
                claim_id(9_003)?,
                entity(9_003)?,
                "note.body",
                "signed ledger A tail",
                evidence_a.scope_id,
                evidence_a.evidence_id,
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                0,
                None,
            )?),
        ],
    )?;
    let accepted_end_b = ledger_b.accept_payloads(
        importer_actor(),
        evidence_b.domain_id,
        vec![
            EventPayload::ClaimAsserted(graph_claim),
            EventPayload::ClaimAsserted(symbol_claim),
            EventPayload::ClaimAsserted(text_claim(
                claim_id(9_003)?,
                entity(9_003)?,
                "note.body",
                "signed ledger B tail",
                evidence_b.scope_id,
                evidence_b.evidence_id,
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                0,
                None,
            )?),
        ],
    )?;
    assert_eq!(accepted_end_a, accepted_end_b);
    let requested_known = accepted_end_a - 1;
    let coordinates = ProjectionCoordinates::new(requested_known, TimestampMillis::new(100));
    let policies = policies(&[
        ("graph.related", AuthorityPolicy::ImplementationObservation),
        ("code.symbol", AuthorityPolicy::ImplementationObservation),
    ])?;

    let snapshot_a = canonical_snapshot(&ledger_a.store_reader()?)?;
    let snapshot_b = canonical_snapshot(&ledger_b.store_reader()?)?;
    assert_eq!(snapshot_a.accept_seq_head, snapshot_b.accept_seq_head);
    assert_eq!(snapshot_a.outbox_head, snapshot_b.outbox_head);
    let batch_id = id::<BatchId>(0xb000_0002)?;
    assert_ne!(
        batch_material(&ledger_a.store_reader()?, batch_id)?.payload_hash,
        batch_material(&ledger_b.store_reader()?, batch_id)?.payload_hash
    );
    let mut store_a = ledger_a.store_reader()?;
    let mut store_b = ledger_b.store_reader()?;
    let source_a =
        projection_source_authority(&mut store_a, evidence_a.domain_id, requested_known)?;
    let source_b =
        projection_source_authority(&mut store_b, evidence_b.domain_id, requested_known)?;
    assert_eq!(source_a.source_outbox_seq, source_b.source_outbox_seq);
    assert_ne!(source_a.source_ledger_digest, source_b.source_ledger_digest);

    let runner = ledger_a.runner()?;
    for kind in [
        ProjectionKind::Graph,
        ProjectionKind::Unicode61,
        ProjectionKind::Trigram,
    ] {
        runner.rebuild_at(kind, evidence_a.domain_id, coordinates, &policies)?;
    }

    let canonical_b = ledger_b.store_reader()?;
    let swapped = ProjectionReader::new(&canonical_b, ledger_a.sidecar_path());
    for kind in [
        ProjectionKind::Graph,
        ProjectionKind::Unicode61,
        ProjectionKind::Trigram,
    ] {
        assert_authority_mismatch(swapped.availability(
            kind,
            evidence_b.domain_id,
            coordinates,
            &policies,
        ));
    }
    assert_authority_mismatch(swapped.graph_neighbors(
        evidence_b.domain_id,
        graph_subject,
        coordinates,
        &policies,
    ));
    for kind in [ProjectionKind::Unicode61, ProjectionKind::Trigram] {
        assert_authority_mismatch(swapped.search_ranked(
            kind,
            evidence_b.domain_id,
            coordinates,
            &policies,
            "LedgerBoundSymbol",
            10,
        ));
        assert_authority_mismatch(swapped.exact_symbol_lookup(
            kind,
            evidence_b.domain_id,
            coordinates,
            &policies,
            "LedgerBoundSymbol",
        ));
    }
    Ok(())
}

#[test]
fn truncated_outbox_prefix_cannot_hash_as_source_authority() -> TestResult {
    let mut fixture = Fixture::new("truncated-source-prefix")?;
    let evidence = fixture.register_scope_evidence(11, 1, b"prefix coverage evidence")?;
    let requested_known = fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(11_001)?,
            entity(11_001)?,
            "graph.related",
            entity(11_011)?,
            evidence.scope_id,
            evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    let connection = Connection::open(fixture.canonical_path())?;
    connection.execute_batch(concat!(
        "DROP TRIGGER guard_projection_outbox_delete; ",
        "DELETE FROM projection_outbox WHERE outbox_seq = ",
        "(SELECT max(outbox_seq) FROM projection_outbox); ",
        "CREATE TRIGGER guard_projection_outbox_delete BEFORE DELETE ON projection_outbox ",
        "BEGIN SELECT RAISE(ABORT, 'canonical table is append-only'); END;"
    ))?;
    drop(connection);

    let mut reader = fixture.store_reader()?;
    let Err(error) = projection_source_authority(&mut reader, evidence.domain_id, requested_known)
    else {
        return Err("truncated outbox prefix produced source authority".into());
    };
    assert!(matches!(
        error,
        QueryError::Corrupt(
            "projection source outbox prefix does not cover the requested acceptance coordinate"
        )
    ));
    Ok(())
}

fn assert_authority_mismatch<T>(result: Result<T, ProjectionError>) {
    assert!(
        matches!(result, Err(ProjectionError::AuthorityMismatch(_))),
        "sidecar swap must fail with typed authority mismatch"
    );
}
