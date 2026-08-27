#![cfg(feature = "phase1-fault-injection")]

mod support;

use academic_domain::{AuthorityClass, EpistemicStatus};
use academic_ledger::AuthorityPolicy;
use academic_projections::{
    generation::{GenerationState, ProjectionAvailability, ProjectionKind},
    runner::{
        ProjectionFaultInjector, ProjectionFaultPoint, ProjectionResult,
        ProjectionVerificationCorruption,
    },
};

use support::{Fixture, TestResult, claim_id, entity, importer_actor, policies, text_claim};

#[derive(Debug, Clone, Copy)]
enum Corruption {
    WrongNamedTokenizer,
    MissingFtsRow,
    WrongPersistedTiebreaker,
}

impl ProjectionFaultInjector for Corruption {
    fn hit(&self, _point: ProjectionFaultPoint) -> ProjectionResult<()> {
        Ok(())
    }

    fn verification_corruption(&self) -> Option<ProjectionVerificationCorruption> {
        Some(match self {
            Self::WrongNamedTokenizer => ProjectionVerificationCorruption::WrongNamedTokenizer,
            Self::MissingFtsRow => ProjectionVerificationCorruption::MissingFtsRow,
            Self::WrongPersistedTiebreaker => {
                ProjectionVerificationCorruption::WrongPersistedTiebreaker
            }
        })
    }
}

#[test]
fn wrong_named_tokenizer_never_verifies() -> TestResult {
    assert_corruption_never_verifies(Corruption::WrongNamedTokenizer)
}

#[test]
fn missing_fts_row_never_verifies() -> TestResult {
    assert_corruption_never_verifies(Corruption::MissingFtsRow)
}

#[test]
fn wrong_persisted_tiebreaker_never_verifies() -> TestResult {
    assert_corruption_never_verifies(Corruption::WrongPersistedTiebreaker)
}

fn assert_corruption_never_verifies(corruption: Corruption) -> TestResult {
    let mut fixture = Fixture::new(match corruption {
        Corruption::WrongNamedTokenizer => "wrong-tokenizer",
        Corruption::MissingFtsRow => "missing-fts-row",
        Corruption::WrongPersistedTiebreaker => "wrong-tiebreaker",
    })?;
    let evidence = fixture.register_scope_evidence(7, 1, b"verification corruption evidence")?;
    let active_known = fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        text_claim(
            claim_id(7_001)?,
            entity(7_001)?,
            "note.body",
            "activepreservationtoken",
            evidence.scope_id,
            evidence.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    let policies = policies(&[("note.body", AuthorityPolicy::ImplementationObservation)])?;
    let runner = fixture.runner()?;
    let active_coordinates = academic_projections::generation::ProjectionCoordinates::new(
        active_known,
        academic_domain::TimestampMillis::new(100),
    );
    let active_receipt = runner.rebuild_at(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        active_coordinates,
        &policies,
    )?;
    assert!(active_receipt.activated);
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        text_claim(
            claim_id(7_002)?,
            entity(7_002)?,
            "note.body",
            "candidatecorruptiontoken",
            evidence.scope_id,
            evidence.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    let result = runner.rebuild_at_with_faults(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        fixture.coordinates(100),
        &policies,
        &corruption,
    );
    assert!(result.is_err());
    assert_eq!(
        runner.audit_generation_state_count(
            ProjectionKind::Unicode61,
            evidence.domain_id,
            GenerationState::Failed,
            None,
        )?,
        1
    );
    let active_after = runner
        .audit_active_generation(ProjectionKind::Unicode61, evidence.domain_id)?
        .ok_or("injected candidate failure removed the active generation")?;
    assert_eq!(
        active_after.generation_id,
        active_receipt.metadata.generation_id
    );
    let page = fixture.projection_reader()?.search_ranked(
        ProjectionKind::Unicode61,
        evidence.domain_id,
        active_coordinates,
        &policies,
        "activepreservationtoken",
        10,
    )?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Lagging { .. }
    ));
    assert_eq!(page.records.len(), 1);
    assert_eq!(page.records[0].claim_id, claim_id(7_001)?);
    Ok(())
}
