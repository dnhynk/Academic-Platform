//! Disposable Phase 1 graph and lexical projection generations.
//!
//! The canonical store is opened read-only. Every graph and search record is
//! written to a separate disposable SQLite sidecar, verified, and made visible
//! only by an atomic active-generation pointer change.

pub mod bitemporal;
pub mod checksum;
pub mod fts;
pub mod generation;
pub mod graph;
pub mod query;
pub mod resolution;
pub mod runner;

/// First relational graph generation contract.
pub const GRAPH_PROJECTION_KIND: &str = "relational-graph-v1";
/// FTS5 Unicode tokenizer baseline name; this is not a relevance acceptance claim.
pub const UNICODE_LEXICAL_PROJECTION_KIND: &str = "fts5-unicode61-v1";
/// FTS5 trigram baseline name; this is not a relevance acceptance claim.
pub const TRIGRAM_LEXICAL_PROJECTION_KIND: &str = "fts5-trigram-v1";
/// Source-ledger-bound projection-generation schema version.
pub const PROJECTION_SCHEMA_VERSION: u32 = 2;

/// Projection names in stable lexical order for receipts and handshakes.
pub const PHASE1_PROJECTION_KINDS: &[&str] = &[
    TRIGRAM_LEXICAL_PROJECTION_KIND,
    UNICODE_LEXICAL_PROJECTION_KIND,
    GRAPH_PROJECTION_KIND,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_names_do_not_claim_canonical_authority() {
        assert!(PHASE1_PROJECTION_KINDS.is_sorted());
        assert!(
            PHASE1_PROJECTION_KINDS
                .iter()
                .all(|name| !name.contains("canonical"))
        );
    }
}
