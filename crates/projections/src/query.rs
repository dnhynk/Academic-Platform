//! Coordinate-bound, generation-aware reads from the disposable sidecar.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use academic_domain::{DomainId, EntityId};
use academic_store::{
    connection::{ReaderConnection, open_reader},
    queries::projection_source_authority,
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
        read_active_generation, read_verified_generation,
    },
};

mod read_boundary {
    use super::{ProjectionResult, fmt};

    pub(crate) trait ProjectionReadBehavior: fmt::Debug {
        fn after_active_metadata(&self) -> ProjectionResult<()>;
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub(crate) struct NoProjectionReadBarrier;

    impl ProjectionReadBehavior for NoProjectionReadBarrier {
        fn after_active_metadata(&self) -> ProjectionResult<()> {
            Ok(())
        }
    }

    /// Deterministic test barrier after generation metadata is selected and
    /// before rows are read from the same SQLite snapshot.
    #[cfg(feature = "phase1-fault-injection")]
    #[doc(hidden)]
    pub trait ProjectionReadBarrier: fmt::Debug {
        fn after_active_metadata(&self) -> ProjectionResult<()>;
    }

    #[cfg(feature = "phase1-fault-injection")]
    impl<T> ProjectionReadBehavior for T
    where
        T: ProjectionReadBarrier + ?Sized,
    {
        fn after_active_metadata(&self) -> ProjectionResult<()> {
            ProjectionReadBarrier::after_active_metadata(self)
        }
    }
}

#[cfg(feature = "phase1-fault-injection")]
pub use read_boundary::ProjectionReadBarrier;
use read_boundary::{NoProjectionReadBarrier, ProjectionReadBehavior};

#[derive(Debug, Clone, Copy)]
struct CanonicalBinding {
    requested: Watermark,
    latest_accept_seq: u64,
    latest_outbox_seq: u64,
}

/// Read-only facade that exposes only source-bound VERIFIED generations.
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
        let (mut canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let availability = select_availability(
            &mut canonical,
            &projection_transaction,
            kind,
            domain,
            coordinates,
            policies,
        )?;
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
        self.graph_neighbors_impl(
            domain,
            source_entity_id,
            coordinates,
            policies,
            &NoProjectionReadBarrier,
        )
    }

    /// Barrier-enabled form used to prove metadata/row snapshot atomicity.
    #[cfg(feature = "phase1-fault-injection")]
    #[doc(hidden)]
    pub fn graph_neighbors_with_barrier<B: ProjectionReadBarrier + ?Sized>(
        &self,
        domain: DomainId,
        source_entity_id: EntityId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        barrier: &B,
    ) -> ProjectionResult<ProjectionPage<GraphEdge>> {
        self.graph_neighbors_impl(domain, source_entity_id, coordinates, policies, barrier)
    }

    fn graph_neighbors_impl<B: ProjectionReadBehavior + ?Sized>(
        &self,
        domain: DomainId,
        source_entity_id: EntityId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        barrier: &B,
    ) -> ProjectionResult<ProjectionPage<GraphEdge>> {
        let (mut canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let availability = select_availability(
            &mut canonical,
            &projection_transaction,
            ProjectionKind::Graph,
            domain,
            coordinates,
            policies,
        )?;
        barrier.after_active_metadata()?;
        let records = if let Some(generation) = availability.selected() {
            read_graph_edges(
                &projection_transaction,
                generation.generation_id,
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
        self.search_ranked_impl(
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
    #[cfg(feature = "phase1-fault-injection")]
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
        self.search_ranked_impl(kind, domain, coordinates, policies, query, limit, barrier)
    }

    #[allow(clippy::too_many_arguments)]
    fn search_ranked_impl<B: ProjectionReadBehavior + ?Sized>(
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
        let (mut canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let availability = select_availability(
            &mut canonical,
            &projection_transaction,
            kind,
            domain,
            coordinates,
            policies,
        )?;
        barrier.after_active_metadata()?;
        let records = if let Some(generation) = availability.selected() {
            read_ranked_hits(
                &projection_transaction,
                kind,
                generation.generation_id,
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
        self.exact_symbol_lookup_impl(
            kind,
            domain,
            coordinates,
            policies,
            symbol,
            &NoProjectionReadBarrier,
        )
    }

    /// Barrier-enabled exact-symbol snapshot proof boundary.
    #[cfg(feature = "phase1-fault-injection")]
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
        self.exact_symbol_lookup_impl(kind, domain, coordinates, policies, symbol, barrier)
    }

    fn exact_symbol_lookup_impl<B: ProjectionReadBehavior + ?Sized>(
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
        let (mut canonical, mut projection) = self.open_snapshot_connections()?;
        let projection_transaction =
            projection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let availability = select_availability(
            &mut canonical,
            &projection_transaction,
            kind,
            domain,
            coordinates,
            policies,
        )?;
        barrier.after_active_metadata()?;
        let records = if let Some(generation) = availability.selected() {
            read_exact_symbol_hits(
                &projection_transaction,
                generation.generation_id,
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

fn select_availability(
    canonical: &mut ReaderConnection,
    projection: &Connection,
    kind: ProjectionKind,
    domain: DomainId,
    coordinates: ProjectionCoordinates,
    policies: &PredicatePolicies,
) -> ProjectionResult<ProjectionAvailability> {
    let active = read_active_generation(projection, kind, domain)?;
    let binding = canonical_binding(canonical, domain, coordinates)?;
    bind_availability(
        projection,
        binding,
        active,
        kind,
        domain,
        coordinates,
        policies,
    )
}

fn canonical_binding(
    reader: &mut ReaderConnection,
    domain: DomainId,
    coordinates: ProjectionCoordinates,
) -> ProjectionResult<CanonicalBinding> {
    let authority = projection_source_authority(reader, domain, coordinates.known_at_accept_seq)?;
    Ok(CanonicalBinding {
        requested: Watermark {
            source_accept_seq: coordinates.known_at_accept_seq,
            source_outbox_seq: authority.source_outbox_seq,
            source_ledger_digest: authority.source_ledger_digest,
        },
        latest_accept_seq: authority.latest_accept_seq,
        latest_outbox_seq: authority.latest_outbox_seq,
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_availability(
    projection: &Connection,
    binding: CanonicalBinding,
    active: Option<ActiveGeneration>,
    kind: ProjectionKind,
    domain: DomainId,
    coordinates: ProjectionCoordinates,
    policies: &PredicatePolicies,
) -> ProjectionResult<ProjectionAvailability> {
    if let Some(active) = active.as_ref()
        && (active.coordinates.known_at_accept_seq > binding.latest_accept_seq
            || active.source_outbox_seq > binding.latest_outbox_seq)
    {
        return Err(ProjectionError::Corrupt(
            "active projection watermark is ahead of the canonical ledger".to_owned(),
        ));
    }

    let authority_matches = |generation: &ActiveGeneration| {
        generation.coordinates == coordinates
            && generation.resolver_version == CANONICAL_RESOLVER_VERSION
            && generation.policy_registry_version == policies.version()
            && generation.policy_registry_hash == policies.canonical_hash()
    };
    if let Some(active) = active.as_ref()
        && authority_matches(active)
    {
        if active.source_outbox_seq != binding.requested.source_outbox_seq
            || active.source_ledger_digest != binding.requested.source_ledger_digest
        {
            return Err(ProjectionError::AuthorityMismatch(
                "active generation does not bind the attached canonical source ledger".to_owned(),
            ));
        }
        let active = active.clone();
        return if active.coordinates.known_at_accept_seq == binding.latest_accept_seq
            && active.source_outbox_seq == binding.latest_outbox_seq
        {
            Ok(ProjectionAvailability::Current { active })
        } else {
            Ok(ProjectionAvailability::Lagging {
                active,
                latest_known_at_accept_seq: binding.latest_accept_seq,
                latest_source_outbox_seq: binding.latest_outbox_seq,
            })
        };
    }

    if let Some(generation) = read_verified_generation(
        projection,
        kind,
        domain,
        coordinates,
        binding.requested,
        policies,
    )? {
        return Ok(ProjectionAvailability::Historical {
            generation,
            current_generation_id: active.as_ref().map(|generation| generation.generation_id),
            latest_known_at_accept_seq: binding.latest_accept_seq,
            latest_source_outbox_seq: binding.latest_outbox_seq,
        });
    }

    let Some(active) = active else {
        return Ok(ProjectionAvailability::NoActive {
            latest_known_at_accept_seq: binding.latest_accept_seq,
            latest_source_outbox_seq: binding.latest_outbox_seq,
        });
    };
    Err(ProjectionError::AuthorityMismatch(format!(
        "requested known_at_accept_seq={}/valid_at={} has no exact source-bound VERIFIED generation; current pointer binds {}/{}",
        coordinates.known_at_accept_seq,
        coordinates.valid_at.value(),
        active.coordinates.known_at_accept_seq,
        active.coordinates.valid_at.value(),
    )))
}
