mod support;

use academic_domain::{AuthorityClass, EpistemicStatus};
use academic_ledger::AuthorityPolicy;
use academic_projections::generation::{ProjectionAvailability, ProjectionKind};

use support::{Fixture, TestResult, claim_id, entity, importer_actor, policies, text_claim};

#[test]
fn fts_drop_and_rebuild_matches() -> TestResult {
    let mut fixture = Fixture::new("fts-drop-rebuild")?;
    let evidence = fixture.register_scope_evidence(1, 1, b"synthetic Korean text evidence")?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        text_claim(
            claim_id(101)?,
            entity(101)?,
            "note.body",
            "합성 트랜잭션 복구 연습",
            evidence.scope_id,
            evidence.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    let coordinates = fixture.coordinates(100);
    let policies = policies(&[("note.body", AuthorityPolicy::ImplementationObservation)])?;

    let first_receipt = fixture.runner()?.rebuild_at(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let first = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        coordinates,
        &policies,
        "트랜잭션",
        20,
    )?;
    assert_eq!(first.records.len(), 1);

    fixture
        .runner()?
        .drop_projection(ProjectionKind::Unicode61, evidence.domain_id)?;
    let dropped = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        coordinates,
        &policies,
        "트랜잭션",
        20,
    )?;
    assert!(matches!(
        dropped.availability,
        ProjectionAvailability::NoActive { .. }
    ));
    assert!(dropped.records.is_empty());

    let second_receipt = fixture.runner()?.rebuild_at(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let second = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        coordinates,
        &policies,
        "트랜잭션",
        20,
    )?;
    assert_eq!(
        first_receipt.metadata.canonical_checksum,
        second_receipt.metadata.canonical_checksum
    );
    assert_eq!(first.records.len(), second.records.len());
    assert_eq!(first.records[0].claim_id, second.records[0].claim_id);
    assert_eq!(
        first.records[0].stable_tiebreaker,
        second.records[0].stable_tiebreaker
    );
    assert_eq!(first.records[0].resolution, second.records[0].resolution);
    Ok(())
}

#[test]
fn fts_domain_isolation() -> TestResult {
    let mut fixture = Fixture::new("fts-domain-isolation")?;
    let first = fixture.register_scope_evidence(1, 1, b"domain one synthetic evidence")?;
    let second = fixture.register_scope_evidence(2, 1, b"domain two synthetic evidence")?;
    for (seed, evidence, body) in [
        (201, &first, "isolatedtoken domain one"),
        (202, &second, "isolatedtoken domain two"),
    ] {
        fixture.accept_claim(
            importer_actor(),
            evidence.domain_id,
            text_claim(
                claim_id(seed)?,
                entity(seed)?,
                "note.body",
                body,
                evidence.scope_id,
                evidence.evidence_id,
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                0,
                None,
            )?,
        )?;
    }
    let policies = policies(&[("note.body", AuthorityPolicy::ImplementationObservation)])?;
    let coordinates = fixture.coordinates(100);
    for domain in [first.domain_id, second.domain_id] {
        fixture
            .runner()?
            .rebuild_at(ProjectionKind::Unicode61, domain, coordinates, &policies)?;
    }
    let first_page = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        first.domain_id,
        coordinates,
        &policies,
        "isolatedtoken",
        20,
    )?;
    let second_page = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        second.domain_id,
        coordinates,
        &policies,
        "isolatedtoken",
        20,
    )?;
    assert_eq!(first_page.records.len(), 1);
    assert_eq!(second_page.records.len(), 1);
    assert_eq!(first_page.records[0].domain, first.domain_id);
    assert_eq!(second_page.records[0].domain, second.domain_id);
    assert_ne!(
        first_page.records[0].claim_id,
        second_page.records[0].claim_id
    );
    Ok(())
}

#[test]
fn fts_korean_code_baseline() -> TestResult {
    let mut fixture = Fixture::new("fts-korean-code")?;
    let korean = fixture.register_scope_evidence(3, 1, b"Korean baseline evidence")?;
    let code =
        fixture.register_evidence(3, korean.scope_id, 2, b"code symbol baseline evidence")?;
    fixture.accept_claim(
        importer_actor(),
        korean.domain_id,
        text_claim(
            claim_id(301)?,
            entity(301)?,
            "note.body",
            "분산 트랜잭션 원자성 복구",
            korean.scope_id,
            korean.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        korean.domain_id,
        text_claim(
            claim_id(302)?,
            entity(302)?,
            "code.symbol",
            "OrderService.updateStatus",
            korean.scope_id,
            code.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    let policies = policies(&[
        ("code.symbol", AuthorityPolicy::ImplementationObservation),
        ("note.body", AuthorityPolicy::ImplementationObservation),
    ])?;
    let coordinates = fixture.coordinates(100);
    fixture.runner()?.rebuild_at(
        ProjectionKind::Unicode61,
        korean.domain_id,
        coordinates,
        &policies,
    )?;
    fixture.runner()?.rebuild_at(
        ProjectionKind::Trigram,
        korean.domain_id,
        coordinates,
        &policies,
    )?;

    let korean_page = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        korean.domain_id,
        coordinates,
        &policies,
        "트랜잭션",
        20,
    )?;
    assert_eq!(korean_page.records.len(), 1);
    assert!(korean_page.records[0].text.contains("원자성"));
    let code_page = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Trigram,
        korean.domain_id,
        coordinates,
        &policies,
        "updateStatus",
        20,
    )?;
    assert_eq!(code_page.records.len(), 1);
    assert_eq!(code_page.records[0].text, "OrderService.updateStatus");
    Ok(())
}

#[test]
fn exact_symbol_lookup_is_not_ranked_text() -> TestResult {
    let mut fixture = Fixture::new("exact-symbol")?;
    let symbol = fixture.register_scope_evidence(4, 1, b"exact symbol evidence")?;
    let prose = fixture.register_evidence(4, symbol.scope_id, 2, b"prose evidence")?;
    fixture.accept_claim(
        importer_actor(),
        symbol.domain_id,
        text_claim(
            claim_id(401)?,
            entity(401)?,
            "code.symbol",
            "OrderService.updateStatus",
            symbol.scope_id,
            symbol.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        symbol.domain_id,
        text_claim(
            claim_id(402)?,
            entity(402)?,
            "note.body",
            "Call OrderService.updateStatus after validation",
            symbol.scope_id,
            prose.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    let policies = policies(&[
        ("code.symbol", AuthorityPolicy::ImplementationObservation),
        ("note.body", AuthorityPolicy::ImplementationObservation),
    ])?;
    let coordinates = fixture.coordinates(100);
    fixture.runner()?.rebuild_at(
        ProjectionKind::Trigram,
        symbol.domain_id,
        coordinates,
        &policies,
    )?;
    let ranked = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Trigram,
        symbol.domain_id,
        coordinates,
        &policies,
        "OrderService.updateStatus",
        20,
    )?;
    assert_eq!(ranked.records.len(), 2);
    let exact = fixture.projection_reader()?.exact_symbol_lookup(
        ProjectionKind::Trigram,
        symbol.domain_id,
        coordinates,
        &policies,
        "OrderService.updateStatus",
    )?;
    assert_eq!(exact.records.len(), 1);
    assert_eq!(exact.records[0].claim_id, claim_id(401)?);
    let wrong_case = fixture.projection_reader()?.exact_symbol_lookup(
        ProjectionKind::Trigram,
        symbol.domain_id,
        coordinates,
        &policies,
        "orderservice.updateStatus",
    )?;
    assert!(wrong_case.records.is_empty());
    Ok(())
}
