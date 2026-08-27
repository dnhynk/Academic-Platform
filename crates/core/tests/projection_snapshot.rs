mod support;

use std::{
    io,
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    thread,
};

use academic_domain::{AuthorityClass, EpistemicStatus};
use academic_ledger::AuthorityPolicy;
use academic_projections::{
    generation::{ProjectionAvailability, ProjectionKind},
    query::ProjectionReadBarrier,
    runner::{ProjectionError, ProjectionResult},
};

use support::{
    Fixture, TestResult, claim_id, entity, importer_actor, observed_entity_claim, policies,
    text_claim,
};

#[derive(Debug)]
struct BlockingRead {
    reached: SyncSender<()>,
    release: Receiver<()>,
}

impl ProjectionReadBarrier for BlockingRead {
    fn after_active_metadata(&self) -> ProjectionResult<()> {
        self.reached
            .send(())
            .map_err(|_| ProjectionError::Corrupt("snapshot coordinator disappeared".to_owned()))?;
        self.release
            .recv()
            .map_err(|_| ProjectionError::Corrupt("snapshot release disappeared".to_owned()))?;
        Ok(())
    }
}

#[test]
fn interleaved_drop_cannot_split_active_metadata_from_rows() -> TestResult {
    let mut fixture = Fixture::new("read-snapshot")?;
    let graph_evidence = fixture.register_scope_evidence(6, 1, b"snapshot graph evidence")?;
    let symbol_evidence =
        fixture.register_evidence(6, graph_evidence.scope_id, 2, b"snapshot symbol evidence")?;
    let subject = entity(6_001)?;
    fixture.accept_claim(
        importer_actor(),
        graph_evidence.domain_id,
        observed_entity_claim(
            claim_id(6_001)?,
            subject,
            "graph.related",
            entity(6_002)?,
            graph_evidence.scope_id,
            graph_evidence.evidence_id,
            0,
            None,
        )?,
    )?;
    fixture.accept_claim(
        importer_actor(),
        graph_evidence.domain_id,
        text_claim(
            claim_id(6_002)?,
            entity(6_003)?,
            "code.symbol",
            "SnapshotService.commitRows",
            graph_evidence.scope_id,
            symbol_evidence.evidence_id,
            AuthorityClass::DirectObservation,
            EpistemicStatus::CodeObserved,
            0,
            None,
        )?,
    )?;
    let policies = policies(&[
        ("code.symbol", AuthorityPolicy::ImplementationObservation),
        ("graph.related", AuthorityPolicy::ImplementationObservation),
    ])?;
    let coordinates = fixture.coordinates(100);

    fixture.runner()?.rebuild_at(
        ProjectionKind::Graph,
        graph_evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let reader = fixture.projection_reader()?;
    let thread_reader = reader.clone();
    let thread_policies = policies.clone();
    let domain_id = graph_evidence.domain_id;
    let (reached_sender, reached_receiver) = sync_channel(0);
    let (release_sender, release_receiver) = sync_channel(0);
    let handle = thread::spawn(move || {
        thread_reader.graph_neighbors_with_barrier(
            domain_id,
            subject,
            coordinates,
            &thread_policies,
            &BlockingRead {
                reached: reached_sender,
                release: release_receiver,
            },
        )
    });
    reached_receiver.recv()?;
    let drop_result = fixture
        .runner()?
        .drop_projection(ProjectionKind::Graph, graph_evidence.domain_id);
    release_sender.send(())?;
    drop_result?;
    let page = handle
        .join()
        .map_err(|_| io::Error::other("graph snapshot reader panicked"))??;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { .. }
    ));
    assert_eq!(page.records.len(), 1);
    assert!(matches!(
        reader
            .graph_neighbors(graph_evidence.domain_id, subject, coordinates, &policies,)?
            .availability,
        ProjectionAvailability::NoActive { .. }
    ));

    fixture.runner()?.rebuild_at(
        ProjectionKind::Unicode61,
        graph_evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let thread_reader = reader.clone();
    let thread_policies = policies.clone();
    let (reached_sender, reached_receiver) = sync_channel(0);
    let (release_sender, release_receiver) = sync_channel(0);
    let handle = thread::spawn(move || {
        thread_reader.search_ranked_with_barrier(
            ProjectionKind::Unicode61,
            domain_id,
            coordinates,
            &thread_policies,
            "SnapshotService",
            20,
            &BlockingRead {
                reached: reached_sender,
                release: release_receiver,
            },
        )
    });
    reached_receiver.recv()?;
    let drop_result = fixture
        .runner()?
        .drop_projection(ProjectionKind::Unicode61, graph_evidence.domain_id);
    release_sender.send(())?;
    drop_result?;
    let page = handle
        .join()
        .map_err(|_| io::Error::other("ranked snapshot reader panicked"))??;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { .. }
    ));
    assert_eq!(page.records.len(), 1);

    fixture.runner()?.rebuild_at(
        ProjectionKind::Trigram,
        graph_evidence.domain_id,
        coordinates,
        &policies,
    )?;
    let thread_reader = reader.clone();
    let thread_policies = policies.clone();
    let (reached_sender, reached_receiver) = sync_channel(0);
    let (release_sender, release_receiver) = sync_channel(0);
    let handle = thread::spawn(move || {
        thread_reader.exact_symbol_lookup_with_barrier(
            ProjectionKind::Trigram,
            domain_id,
            coordinates,
            &thread_policies,
            "SnapshotService.commitRows",
            &BlockingRead {
                reached: reached_sender,
                release: release_receiver,
            },
        )
    });
    reached_receiver.recv()?;
    let drop_result = fixture
        .runner()?
        .drop_projection(ProjectionKind::Trigram, graph_evidence.domain_id);
    release_sender.send(())?;
    drop_result?;
    let page = handle
        .join()
        .map_err(|_| io::Error::other("exact-symbol snapshot reader panicked"))??;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { .. }
    ));
    assert_eq!(page.records.len(), 1);
    assert!(matches!(
        reader
            .exact_symbol_lookup(
                ProjectionKind::Trigram,
                graph_evidence.domain_id,
                coordinates,
                &policies,
                "SnapshotService.commitRows",
            )?
            .availability,
        ProjectionAvailability::NoActive { .. }
    ));
    Ok(())
}
