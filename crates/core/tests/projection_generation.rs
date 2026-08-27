#![cfg(feature = "phase1-fault-injection")]

mod support;

use std::{
    env, io,
    path::PathBuf,
    process::Command,
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    thread,
};

use academic_domain::{ContentDigest, DomainId, TimestampMillis};
use academic_ledger::AuthorityPolicy;
use academic_projections::{
    generation::{GenerationState, ProjectionAvailability, ProjectionCoordinates, ProjectionKind},
    runner::{
        ProjectionError, ProjectionFaultInjector, ProjectionFaultPoint, ProjectionResult,
        ProjectionRunner,
    },
};
use academic_store::connection;

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, policies,
};

const CHILD_ENV: &str = "ACADEMIC_PROJECTION_FAULT_CHILD";
const CANONICAL_ENV: &str = "ACADEMIC_PROJECTION_FAULT_CANONICAL";
const SIDECAR_ENV: &str = "ACADEMIC_PROJECTION_FAULT_SIDECAR";
const DOMAIN_ENV: &str = "ACADEMIC_PROJECTION_FAULT_DOMAIN";
const KNOWN_ENV: &str = "ACADEMIC_PROJECTION_FAULT_KNOWN";

#[derive(Debug)]
struct ErrorFault(ProjectionFaultPoint);

impl ProjectionFaultInjector for ErrorFault {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == self.0 {
            Err(ProjectionError::InjectedFault(point))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
struct ExitFault(ProjectionFaultPoint);

impl ProjectionFaultInjector for ExitFault {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == self.0 {
            std::process::exit(fault_exit_code(point));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct BlockingChecksumFault {
    reached: SyncSender<()>,
    release: Receiver<()>,
}

impl ProjectionFaultInjector for BlockingChecksumFault {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == ProjectionFaultPoint::Pr02AfterChecksum {
            self.reached.send(()).map_err(|_| {
                ProjectionError::Corrupt("stale-build coordinator disappeared".to_owned())
            })?;
            self.release.recv().map_err(|_| {
                ProjectionError::Corrupt("stale-build release disappeared".to_owned())
            })?;
        }
        Ok(())
    }
}

#[test]
fn outbox_rebuild_reaches_source_watermark() -> TestResult {
    let mut fixture = Fixture::new("generation-watermark")?;
    let evidence = fixture.register_scope_evidence(1, 1, b"graph watermark evidence one")?;
    let second =
        fixture.register_evidence(1, evidence.scope_id, 2, b"graph watermark evidence two")?;
    let subject = entity(21)?;
    for (seed, predicate, target, evidence_id) in [
        (11, "graph.related", 31, evidence.evidence_id),
        (12, "graph.dependency", 32, second.evidence_id),
    ] {
        fixture.accept_claim(
            importer_actor(),
            evidence.domain_id,
            observed_entity_claim(
                claim_id(seed)?,
                subject,
                predicate,
                entity(target)?,
                evidence.scope_id,
                evidence_id,
                0,
                None,
            )?,
        )?;
    }
    let coordinates = fixture.coordinates(100);
    let policies = graph_policies()?;
    let receipt = fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        evidence.domain_id,
        coordinates,
        &policies,
    )?;
    assert!(receipt.activated);
    assert_eq!(receipt.metadata.coordinates, coordinates);
    assert_eq!(receipt.metadata.record_count, Some(2));
    assert_eq!(receipt.metadata.state, GenerationState::Verified);
    assert!(receipt.metadata.source_outbox_seq > 0);

    let page = fixture.projection_reader()?.graph_neighbors(
        evidence.domain_id,
        subject,
        coordinates,
        &policies,
    )?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { .. }
    ));
    assert_eq!(page.records.len(), 2);
    assert!(
        page.records
            .iter()
            .all(|edge| edge.resolution.coordinates == coordinates)
    );
    Ok(())
}

#[test]
fn failed_generation_is_never_active() -> TestResult {
    let mut fixture = Fixture::new("generation-failed")?;
    let evidence = fixture.register_scope_evidence(2, 1, b"failed generation evidence")?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(21)?,
            entity(22)?,
            "graph.related",
            entity(23)?,
            evidence.scope_id,
            evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    let coordinates = fixture.coordinates(100);
    let policies = graph_policies()?;
    let runner = fixture.runner()?;
    let result = runner.rebuild_at_with_faults(
        ProjectionKind::Graph,
        evidence.domain_id,
        coordinates,
        &policies,
        &ErrorFault(ProjectionFaultPoint::Pr01MidWrite),
    );
    assert!(matches!(
        result,
        Err(ProjectionError::InjectedFault(
            ProjectionFaultPoint::Pr01MidWrite
        ))
    ));

    assert_eq!(
        runner.audit_generation_state_count(
            ProjectionKind::Graph,
            evidence.domain_id,
            GenerationState::Failed,
            None,
        )?,
        1
    );
    assert!(
        runner
            .audit_active_generation(ProjectionKind::Graph, evidence.domain_id)?
            .is_none()
    );
    let availability = fixture.projection_reader()?.availability(
        ProjectionKind::Graph,
        evidence.domain_id,
        coordinates,
        &policies,
    )?;
    assert!(matches!(
        availability,
        ProjectionAvailability::NoActive { .. }
    ));
    Ok(())
}

#[test]
fn generation_switch_is_atomic() -> TestResult {
    for point in [
        ProjectionFaultPoint::Pr01MidWrite,
        ProjectionFaultPoint::Pr02AfterChecksum,
        ProjectionFaultPoint::Pr03DuringActivation,
    ] {
        assert_process_crash_preserves_atomic_authority(point)?;
    }
    assert_stale_generation_cannot_regress_authority()?;
    Ok(())
}

#[test]
fn projection_fault_child() -> TestResult {
    let Ok(point) = env::var(CHILD_ENV) else {
        return Ok(());
    };
    let point = parse_fault(&point)?;
    let canonical = PathBuf::from(required_env_os(CANONICAL_ENV)?);
    let sidecar = PathBuf::from(required_env_os(SIDECAR_ENV)?);
    let domain: DomainId = env::var(DOMAIN_ENV)?.parse()?;
    let known_at_accept_seq = env::var(KNOWN_ENV)?.parse()?;
    let reader = connection::open_reader(&canonical)?;
    let runner = ProjectionRunner::open(
        &reader,
        sidecar,
        ContentDigest::sha256(b"projection-real-acceptance-test-builder"),
        ContentDigest::sha256(b"projection-real-acceptance-test-config"),
    )?;
    let result = runner.rebuild_at_with_faults(
        ProjectionKind::Graph,
        domain,
        ProjectionCoordinates::new(known_at_accept_seq, TimestampMillis::new(100)),
        &graph_policies()?,
        &ExitFault(point),
    );
    Err(io::Error::other(format!("child failpoint did not exit: {result:?}")).into())
}

fn assert_process_crash_preserves_atomic_authority(point: ProjectionFaultPoint) -> TestResult {
    let mut fixture = Fixture::new(point.as_str())?;
    let evidence = fixture.register_scope_evidence(3, 1, b"crash generation evidence one")?;
    let second =
        fixture.register_evidence(3, evidence.scope_id, 2, b"crash generation evidence two")?;
    let subject = entity(33)?;
    for (seed, predicate, target, evidence_id) in [
        (31, "graph.related", 41, evidence.evidence_id),
        (32, "graph.dependency", 42, second.evidence_id),
    ] {
        fixture.accept_claim(
            importer_actor(),
            evidence.domain_id,
            observed_entity_claim(
                claim_id(seed)?,
                subject,
                predicate,
                entity(target)?,
                evidence.scope_id,
                evidence_id,
                0,
                None,
            )?,
        )?;
    }
    let old_coordinates = fixture.coordinates(100);
    let policies = graph_policies()?;
    let original = fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        evidence.domain_id,
        old_coordinates,
        &policies,
    )?;
    let third =
        fixture.register_evidence(3, evidence.scope_id, 3, b"crash generation evidence three")?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(33)?,
            subject,
            "graph.contains",
            entity(43)?,
            evidence.scope_id,
            third.evidence_id,
            0,
            None,
        )?,
    )?;
    let new_coordinates = fixture.coordinates(100);

    let status = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("projection_fault_child")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ENV, point.as_str())
        .env(CANONICAL_ENV, fixture.canonical_path())
        .env(SIDECAR_ENV, fixture.sidecar_path())
        .env(DOMAIN_ENV, evidence.domain_id.to_string())
        .env(KNOWN_ENV, new_coordinates.known_at_accept_seq.to_string())
        .status()?;
    assert_eq!(status.code(), Some(fault_exit_code(point)));

    let runner = fixture.runner()?;
    let authority = runner
        .audit_active_generation(ProjectionKind::Graph, evidence.domain_id)?
        .ok_or("crash recovery lost the prior active generation")?;
    assert_eq!(authority.generation_id, original.metadata.generation_id);
    assert_eq!(authority.coordinates, old_coordinates);
    assert_eq!(
        authority.source_outbox_seq,
        original.metadata.source_outbox_seq
    );
    let verified_inactive = runner.audit_generation_state_count(
        ProjectionKind::Graph,
        evidence.domain_id,
        GenerationState::Verified,
        Some(original.metadata.generation_id),
    )?;
    match point {
        ProjectionFaultPoint::Pr01MidWrite => assert_eq!(verified_inactive, 0),
        ProjectionFaultPoint::Pr02AfterChecksum | ProjectionFaultPoint::Pr03DuringActivation => {
            assert_eq!(verified_inactive, 1);
        }
    }
    let page = fixture.projection_reader()?.graph_neighbors(
        evidence.domain_id,
        subject,
        old_coordinates,
        &policies,
    )?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Lagging {
            latest_known_at_accept_seq,
            ..
        } if latest_known_at_accept_seq == new_coordinates.known_at_accept_seq
    ));
    assert_eq!(page.records.len(), 2);
    Ok(())
}

fn assert_stale_generation_cannot_regress_authority() -> TestResult {
    let mut fixture = Fixture::new("stale-generation")?;
    let evidence = fixture.register_scope_evidence(4, 1, b"stale generation evidence one")?;
    let subject = entity(44)?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(41)?,
            subject,
            "graph.dependency",
            entity(45)?,
            evidence.scope_id,
            evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    let stale_coordinates = fixture.coordinates(100);
    let policies = graph_policies()?;
    let stale_runner = fixture.runner()?;
    let stale_policies = policies.clone();
    let (reached_sender, reached_receiver) = sync_channel(0);
    let (release_sender, release_receiver) = sync_channel(0);
    let domain_id = evidence.domain_id;
    let stale_thread = thread::spawn(move || {
        stale_runner.rebuild_at_with_faults(
            ProjectionKind::Graph,
            domain_id,
            stale_coordinates,
            &stale_policies,
            &BlockingChecksumFault {
                reached: reached_sender,
                release: release_receiver,
            },
        )
    });
    reached_receiver.recv()?;

    let second =
        fixture.register_evidence(4, evidence.scope_id, 2, b"stale generation evidence two")?;
    fixture.accept_claim(
        importer_actor(),
        evidence.domain_id,
        observed_entity_claim(
            claim_id(42)?,
            subject,
            "graph.related",
            entity(46)?,
            evidence.scope_id,
            second.evidence_id,
            0,
            None,
        )?,
    )?;
    let newer_coordinates = fixture.coordinates(100);
    let newer = fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        evidence.domain_id,
        newer_coordinates,
        &policies,
    )?;
    assert!(newer.activated);
    release_sender.send(())?;
    let stale = stale_thread
        .join()
        .map_err(|_| io::Error::other("stale generation worker panicked"))??;
    assert!(!stale.activated);

    let page = fixture.projection_reader()?.graph_neighbors(
        evidence.domain_id,
        subject,
        newer_coordinates,
        &policies,
    )?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { ref active }
            if active.coordinates == newer_coordinates
                && active.generation_id == newer.metadata.generation_id
    ));
    assert_eq!(page.records.len(), 2);
    Ok(())
}

fn graph_policies() -> TestResult<academic_projections::resolution::PredicatePolicies> {
    policies(&[
        ("graph.contains", AuthorityPolicy::ImplementationObservation),
        (
            "graph.dependency",
            AuthorityPolicy::ImplementationObservation,
        ),
        ("graph.related", AuthorityPolicy::ImplementationObservation),
    ])
}

fn parse_fault(value: &str) -> TestResult<ProjectionFaultPoint> {
    Ok(match value {
        "PR01" => ProjectionFaultPoint::Pr01MidWrite,
        "PR02" => ProjectionFaultPoint::Pr02AfterChecksum,
        "PR03" => ProjectionFaultPoint::Pr03DuringActivation,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown child fault point {value}"),
            )
            .into());
        }
    })
}

fn required_env_os(key: &str) -> TestResult<std::ffi::OsString> {
    env::var_os(key).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("child {key} missing")).into()
    })
}

const fn fault_exit_code(point: ProjectionFaultPoint) -> i32 {
    match point {
        ProjectionFaultPoint::Pr01MidWrite => 71,
        ProjectionFaultPoint::Pr02AfterChecksum => 72,
        ProjectionFaultPoint::Pr03DuringActivation => 73,
    }
}
