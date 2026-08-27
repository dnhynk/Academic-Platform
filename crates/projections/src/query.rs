//! Generation-aware reads from the disposable projection sidecar.

use std::path::{Path, PathBuf};

use academic_domain::{DomainId, EntityId};
use academic_store::connection::ReaderConnection;

use crate::{
    fts::{ExactSymbolHit, SearchHit, read_exact_symbol_hits, read_ranked_hits},
    generation::{ProjectionAvailability, ProjectionKind, ProjectionPage},
    graph::{GraphEdge, read_graph_edges},
    runner::{
        ProjectionError, ProjectionResult, latest_watermark, open_canonical_reader,
        open_projection_reader, read_active_generation,
    },
};

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

    /// Returns the authority state for one projection kind and security domain.
    pub fn availability(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
    ) -> ProjectionResult<ProjectionAvailability> {
        let canonical = open_canonical_reader(&self.canonical_database_path)?;
        let latest = latest_watermark(&canonical)?;
        let projection = open_projection_reader(&self.projection_database_path)?;
        let active = read_active_generation(&projection, kind, domain)?;
        if active.as_ref().is_some_and(|active| {
            active.source_accept_seq > latest.source_accept_seq
                || active.source_outbox_seq > latest.source_outbox_seq
        }) {
            return Err(ProjectionError::Corrupt(
                "active projection watermark is ahead of the canonical outbox".to_owned(),
            ));
        }
        Ok(match active {
            None => ProjectionAvailability::NoActive {
                latest_source_accept_seq: latest.source_accept_seq,
                latest_source_outbox_seq: latest.source_outbox_seq,
            },
            Some(active)
                if active.source_accept_seq == latest.source_accept_seq
                    && active.source_outbox_seq == latest.source_outbox_seq =>
            {
                ProjectionAvailability::Current { active }
            }
            Some(active) => ProjectionAvailability::Lagging {
                active,
                latest_source_accept_seq: latest.source_accept_seq,
                latest_source_outbox_seq: latest.source_outbox_seq,
            },
        })
    }

    /// Reads graph neighbors from the old active generation even while rebuilding.
    pub fn graph_neighbors(
        &self,
        domain: DomainId,
        source_entity_id: EntityId,
    ) -> ProjectionResult<ProjectionPage<GraphEdge>> {
        let availability = self.availability(ProjectionKind::Graph, domain)?;
        let records = if let Some(active) = availability.active() {
            let projection = open_projection_reader(&self.projection_database_path)?;
            read_graph_edges(
                &projection,
                active.generation_id,
                active.source_accept_seq,
                source_entity_id,
            )?
        } else {
            Vec::new()
        };
        Ok(ProjectionPage {
            availability,
            records,
        })
    }

    /// Runs FTS5 ranking within the active generation and exact domain.
    pub fn search_ranked(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        query: &str,
        limit: usize,
    ) -> ProjectionResult<ProjectionPage<SearchHit>> {
        if kind == ProjectionKind::Graph {
            return Err(ProjectionError::InvalidQuery(
                "graph generations do not support lexical search",
            ));
        }
        let availability = self.availability(kind, domain)?;
        let records = if let Some(active) = availability.active() {
            let projection = open_projection_reader(&self.projection_database_path)?;
            read_ranked_hits(
                &projection,
                kind,
                active.generation_id,
                active.source_accept_seq,
                query,
                limit,
            )?
        } else {
            Vec::new()
        };
        Ok(ProjectionPage {
            availability,
            records,
        })
    }

    /// Looks up an exact, case-sensitive symbol without invoking text ranking.
    pub fn exact_symbol_lookup(
        &self,
        kind: ProjectionKind,
        domain: DomainId,
        symbol: &str,
    ) -> ProjectionResult<ProjectionPage<ExactSymbolHit>> {
        if kind == ProjectionKind::Graph {
            return Err(ProjectionError::InvalidQuery(
                "graph generations do not support exact symbol lookup",
            ));
        }
        let availability = self.availability(kind, domain)?;
        let records = if let Some(active) = availability.active() {
            let projection = open_projection_reader(&self.projection_database_path)?;
            read_exact_symbol_hits(
                &projection,
                active.generation_id,
                active.source_accept_seq,
                symbol,
            )?
        } else {
            Vec::new()
        };
        Ok(ProjectionPage {
            availability,
            records,
        })
    }
}
