//! Coordinate-bound, generation-aware reads from the disposable sidecar.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use academic_domain::{DomainId, EntityId};
use academic_store::{
    connection::{ReaderConnection, open_reader},
    queries::canonical_snapshot,
};
use rusqlite::{Connection, TransactionBehavior};

use crate::{
    fts::{ExactSymbolHit, SearchHit, read_exact_symbol_hits, read_ranked_hits},
    generation::{
        ActiveGeneration, ProjectionAvailability, ProjectionCoordinates, ProjectionKind,
        ProjectionPage,
    },
    graph::{GraphEdge, read_graph_edges},
    resolution::{CANONICAL_RESOLVER_VERSION, PredicatePolicies},
    runner::{
        ProjectionError, ProjectionResult, Watermark, open_projection_reader,
        read_active_generation,
    },
};

/// Deterministic test barrier after active metadata is read and before rows are
/// read from the same SQLite snapshot.
#[doc(hidden)]
pub trait ProjectionReadBarrier: fmt::Debug {
    fn after_active_metadata(&self) -> ProjectionResult<()>;
}

#[derive(Debug, Clone, Copy, Default)]
struct NoProjectionReadBarrier;

impl ProjectionReadBarrier for NoProjectionReadBarrier {
    fn after_active_metadata(&self) -> ProjectionResult<()> {
        Ok(())
    }
}

/// Read-only facade that exposes only atomically active VERIFIED generations.
#[derive(Debug, Clone)]
pub struct ProjectionReader {
    canonical_database_path: PathBuf,
    projection_database_path: PathBuf,
}

impl ProjectionReader {
    /// Binds an already-verified canonical reader to a disposable projection sidecar.
    #[must_use]
    pub fn new(
        canonical_reader: &ReaderConnection,
        projection_database_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            canonical_database_path: canonical_reader.database_path().to_path_buf(),
            projection_database_path: projection_database_path.as_ref().to_path_buf(),
        }
    }

    /// Returns authority only for an exact known/valid coordinate and policy registry.
    pub fn availability(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
    ) -> ProjectionResult<ProjectionAvailability> {
        let (canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let active = read_active_generation(&projection_transaction, kind, domain)?;
        let latest = canonical_watermark(&canonical)?;
        let availability = bind_availability(latest, active, coordinates, policies)?;
        projection_transaction.commit()?;
        Ok(availability)
    }

    /// Reads graph neighbors at exact canonical resolver coordinates.
    pub fn graph_neighbors(
        &self,
        domain: DomainId,
        source_entity_id: EntityId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
    ) -> ProjectionResult<ProjectionPage<GraphEdge>> {
        self.graph_neighbors_with_barrier(
            domain,
            source_entity_id,
            coordinates,
            policies,
            &NoProjectionReadBarrier,
        )
    }

    /// Barrier-enabled form used to prove metadata/row snapshot atomicity.
    #[doc(hidden)]
    pub fn graph_neighbors_with_barrier<B: ProjectionReadBarrier + ?Sized>(
        &self,
        domain: DomainId,
        source_entity_id: EntityId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        barrier: &B,
    ) -> ProjectionResult<ProjectionPage<GraphEdge>> {
        let (canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let active =
            read_active_generation(&projection_transaction, ProjectionKind::Graph, domain)?;
        let latest = canonical_watermark(&canonical)?;
        let availability = bind_availability(latest, active, coordinates, policies)?;
        barrier.after_active_metadata()?;
        let records = if let Some(active) = availability.active() {
            read_graph_edges(
                &projection_transaction,
                active.generation_id,
                coordinates,
                source_entity_id,
            )?
        } else {
            Vec::new()
        };
        projection_transaction.commit()?;
        Ok(ProjectionPage {
            availability,
            records,
        })
    }

    /// Runs FTS5 ranking at exact canonical resolver coordinates.
    pub fn search_ranked(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        query: &str,
        limit: usize,
    ) -> ProjectionResult<ProjectionPage<SearchHit>> {
        self.search_ranked_with_barrier(
            kind,
            domain,
            coordinates,
            policies,
            query,
            limit,
            &NoProjectionReadBarrier,
        )
    }

    /// Barrier-enabled ranked search snapshot proof boundary.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn search_ranked_with_barrier<B: ProjectionReadBarrier + ?Sized>(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        query: &str,
        limit: usize,
        barrier: &B,
    ) -> ProjectionResult<ProjectionPage<SearchHit>> {
        if kind == ProjectionKind::Graph {
            return Err(ProjectionError::InvalidQuery(
                "graph generations do not support lexical search",
            ));
        }
        let (canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let active = read_active_generation(&projection_transaction, kind, domain)?;
        let latest = canonical_watermark(&canonical)?;
        let availability = bind_availability(latest, active, coordinates, policies)?;
        barrier.after_active_metadata()?;
        let records = if let Some(active) = availability.active() {
            read_ranked_hits(
                &projection_transaction,
                kind,
                active.generation_id,
                coordinates,
                query,
                limit,
            )?
        } else {
            Vec::new()
        };
        projection_transaction.commit()?;
        Ok(ProjectionPage {
            availability,
            records,
        })
    }

    /// Looks up one case-sensitive symbol at exact resolver coordinates.
    pub fn exact_symbol_lookup(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        symbol: &str,
    ) -> ProjectionResult<ProjectionPage<ExactSymbolHit>> {
        self.exact_symbol_lookup_with_barrier(
            kind,
            domain,
            coordinates,
            policies,
            symbol,
            &NoProjectionReadBarrier,
        )
    }

    /// Barrier-enabled exact-symbol snapshot proof boundary.
    #[doc(hidden)]
    pub fn exact_symbol_lookup_with_barrier<B: ProjectionReadBarrier + ?Sized>(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        symbol: &str,
        barrier: &B,
    ) -> ProjectionResult<ProjectionPage<ExactSymbolHit>> {
        if kind == ProjectionKind::Graph {
            return Err(ProjectionError::InvalidQuery(
                "graph generations do not support exact symbol lookup",
            ));
        }
        let (canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let active = read_active_generation(&projection_transaction, kind, domain)?;
        let latest = canonical_watermark(&canonical)?;
        let availability = bind_availability(latest, active, coordinates, policies)?;
        barrier.after_active_metadata()?;
        let records = if let Some(active) = availability.active() {
            read_exact_symbol_hits(
                &projection_transaction,
                active.generation_id,
                coordinates,
                symbol,
            )?
        } else {
            Vec::new()
        };
        projection_transaction.commit()?;
        Ok(ProjectionPage {
            availability,
            records,
        })
    }

    fn open_snapshot_connections(&self) -> ProjectionResult<(ReaderConnection, Connection)> {
        Ok((
            open_reader(&self.canonical_database_path)?,
            open_projection_reader(&self.projection_database_path)?,
        ))
    }
}

fn canonical_watermark(reader: &ReaderConnection) -> ProjectionResult<Watermark> {
    let snapshot = canonical_snapshot(reader)?;
    Ok(Watermark {
        source_accept_seq: snapshot.accept_seq_head,
        source_outbox_seq: snapshot.outbox_head,
    })
}

fn bind_availability(
    latest: Watermark,
    active: Option<ActiveGeneration>,
    coordinates: ProjectionCoordinates,
    policies: &PredicatePolicies,
) -> ProjectionResult<ProjectionAvailability> {
    let Some(active) = active else {
        return Ok(ProjectionAvailability::NoActive {
            latest_known_at_accept_seq: latest.source_accept_seq,
            latest_source_outbox_seq: latest.source_outbox_seq,
        });
    };
    if active.coordinates.known_at_accept_seq > latest.source_accept_seq
        || active.source_outbox_seq > latest.source_outbox_seq
    {
        return Err(ProjectionError::Corrupt(
            "active projection watermark is ahead of the canonical outbox".to_owned(),
        ));
    }
    if active.coordinates != coordinates {
        return Err(ProjectionError::AuthorityMismatch(format!(
            "requested known_at_accept_seq={}/valid_at={} but active generation binds {}/{}",
            coordinates.known_at_accept_seq,
            coordinates.valid_at.value(),
            active.coordinates.known_at_accept_seq,
            active.coordinates.valid_at.value(),
        )));
    }
    if active.resolver_version != CANONICAL_RESOLVER_VERSION
        || active.policy_registry_version != policies.version()
        || active.policy_registry_hash != policies.canonical_hash()
    {
        return Err(ProjectionError::AuthorityMismatch(
            "requested resolver/policy registry does not exactly match active authority".to_owned(),
        ));
    }
    if active.coordinates.known_at_accept_seq == latest.source_accept_seq
        && active.source_outbox_seq == latest.source_outbox_seq
    {
        Ok(ProjectionAvailability::Current { active })
    } else {
        Ok(ProjectionAvailability::Lagging {
            active,
            latest_known_at_accept_seq: latest.source_accept_seq,
            latest_source_outbox_seq: latest.source_outbox_seq,
        })
    }
}
