//! Projection generation identities and query-visible state.

use std::fmt;

use crate::resolution::AuthorityPolicy;
use academic_domain::{ContentDigest, DomainId};

use crate::{
    GRAPH_PROJECTION_KIND, TRIGRAM_LEXICAL_PROJECTION_KIND, UNICODE_LEXICAL_PROJECTION_KIND,
};

/// One supported disposable projection kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectionKind {
    /// Relational entity-to-entity claim adjacency.
    Graph,
    /// FTS5 `unicode61` Korean/general-text baseline.
    Unicode61,
    /// FTS5 trigram code/substring baseline.
    Trigram,
}

impl ProjectionKind {
    /// Returns the stable persisted kind name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Graph => GRAPH_PROJECTION_KIND,
            Self::Unicode61 => UNICODE_LEXICAL_PROJECTION_KIND,
            Self::Trigram => TRIGRAM_LEXICAL_PROJECTION_KIND,
        }
    }

    /// Parses the closed Phase 1 kind vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            GRAPH_PROJECTION_KIND => Some(Self::Graph),
            UNICODE_LEXICAL_PROJECTION_KIND => Some(Self::Unicode61),
            TRIGRAM_LEXICAL_PROJECTION_KIND => Some(Self::Trigram),
            _ => None,
        }
    }

    /// Records the actual tokenizer or the explicit absence of one.
    #[must_use]
    pub const fn tokenizer_version(self) -> &'static str {
        match self {
            Self::Graph => "none",
            Self::Unicode61 => "sqlite-fts5-unicode61-v1",
            Self::Trigram => "sqlite-fts5-trigram-v1",
        }
    }
}

impl fmt::Display for ProjectionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Opaque 16-byte identifier for one disposable generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationId([u8; 16]);

impl GenerationId {
    /// Constructs an identifier from exact persisted bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Returns the exact persisted bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for GenerationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Persisted lifecycle of a generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationState {
    /// Records may be partial and are never queryable.
    Building,
    /// Count/provenance/checksum verification completed.
    Verified,
    /// A caught build error made this generation permanently ineligible.
    Failed,
}

impl GenerationState {
    /// Returns the stable SQL spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Building => "BUILDING",
            Self::Verified => "VERIFIED",
            Self::Failed => "FAILED",
        }
    }

    /// Parses the closed lifecycle vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "BUILDING" => Some(Self::Building),
            "VERIFIED" => Some(Self::Verified),
            "FAILED" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Metadata recorded for every generation before any projected row is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationMetadata {
    pub generation_id: GenerationId,
    pub kind: ProjectionKind,
    pub schema_version: u32,
    pub builder_binary_digest: ContentDigest,
    pub algorithm_version: String,
    pub tokenizer_version: String,
    pub effective_config_hash: ContentDigest,
    pub coordinates: ProjectionCoordinates,
    pub source_outbox_seq: u64,
    pub source_ledger_digest: ContentDigest,
    pub resolver_version: String,
    pub policy_registry_version: String,
    pub policy_registry_hash: ContentDigest,
    pub security_domain: DomainId,
    pub built_at_unix_ms: i64,
    pub state: GenerationState,
    pub record_count: Option<u64>,
    pub canonical_checksum: Option<ContentDigest>,
}

/// A VERIFIED generation selected by the active pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveGeneration {
    pub generation_id: GenerationId,
    pub kind: ProjectionKind,
    pub security_domain: DomainId,
    pub coordinates: ProjectionCoordinates,
    pub source_outbox_seq: u64,
    pub source_ledger_digest: ContentDigest,
    pub resolver_version: String,
    pub policy_registry_version: String,
    pub policy_registry_hash: ContentDigest,
    pub record_count: u64,
    pub canonical_checksum: ContentDigest,
}

/// The two mandatory bitemporal coordinates for an active projection result.
///
/// This is the canonical coordinate pair from the domain vocabulary rather than
/// a second copy of it: the canonical store's aggregate timeline read and this
/// sidecar's generation reads take the same value, so there is one place where
/// "both coordinates are required" is stated and no way for the two surfaces to
/// drift into differently shaped coordinates.
pub use academic_domain::temporal::TimeCoordinates as ProjectionCoordinates;

/// Resolver provenance copied onto every active projection record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionProvenance {
    pub authority_policy: AuthorityPolicy,
    pub coordinates: ProjectionCoordinates,
}

/// Explicit query authority/lag state; BUILDING or FAILED is never represented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionAvailability {
    /// No generation has ever been atomically activated for this kind/domain.
    NoActive {
        latest_known_at_accept_seq: u64,
        latest_source_outbox_seq: u64,
    },
    /// The active generation covers the latest committed outbox watermark.
    Current { active: ActiveGeneration },
    /// The old active generation remains queryable while canonical input is ahead.
    Lagging {
        active: ActiveGeneration,
        latest_known_at_accept_seq: u64,
        latest_source_outbox_seq: u64,
    },
    /// An exact source-bound VERIFIED generation was selected without changing
    /// the monotonic default-current pointer.
    Historical {
        generation: ActiveGeneration,
        current_generation_id: Option<GenerationId>,
        latest_known_at_accept_seq: u64,
        latest_source_outbox_seq: u64,
    },
}

impl ProjectionAvailability {
    /// Returns the exact generation selected for this query.
    #[must_use]
    pub const fn selected(&self) -> Option<&ActiveGeneration> {
        match self {
            Self::NoActive { .. } => None,
            Self::Current { active } | Self::Lagging { active, .. } => Some(active),
            Self::Historical { generation, .. } => Some(generation),
        }
    }
}

/// Query results coupled to the exact active-generation authority state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionPage<T> {
    pub availability: ProjectionAvailability,
    pub records: Vec<T>,
}
