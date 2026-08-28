//! `academic export` — one deterministic open export directory.
//!
//! Export is a read. When a daemon owns the profile the CLI first completes a
//! read-only handshake, so the owning daemon is the authority on whether the
//! profile may be read at all: a locked or repair-required profile stops the
//! command before anything is written. When no daemon owns the profile the
//! export runs offline against the same read-only boundary.
//!
//! The frozen Phase 1 protocol has no export command — `MutableRequest.command`
//! is a closed oneof of ingest, backup, and restore — so the export bytes
//! themselves cannot travel over IPC in this phase. The handshake is still
//! required whenever a daemon is present, and the produced directory is
//! identical either way because it is a function of the committed watermark.

use std::path::Path;

use academic_core::operations::{SYNTHETIC_EXPORT_CAPABILITY, export_synthetic_profile};
use serde_json::json;

use crate::{
    commands::{classify, display, ownership::consult_owning_daemon},
    output::CommandResult,
};

/// Writes one deterministic export directory.
pub async fn run(profile_root: &Path, destination: &Path, runtime_root: &Path) -> CommandResult {
    let ownership =
        consult_owning_daemon(runtime_root, profile_root, SYNTHETIC_EXPORT_CAPABILITY).await?;
    let receipt = export_synthetic_profile(profile_root, destination)
        .map_err(|error| classify("EXPORT_FAILED", &error))?;

    Ok(json!({
        "profile_root": display(profile_root),
        "destination": display(&receipt.destination),
        "format": academic_core::operations::EXPORT_FORMAT,
        "ownership": ownership,
        "semantic_digest": receipt.manifest.semantic_digest,
        "canonical_semantic_digest": receipt.manifest.semantic.canonical_semantic_digest,
        "watermark": {
            "accept_seq_head": receipt.manifest.semantic.watermark.accept_seq_head,
            "outbox_head": receipt.manifest.semantic.watermark.outbox_head,
        },
        "file_count": receipt.manifest.semantic.files.len(),
        "projections_included": receipt.manifest.semantic.projections_included,
        "encrypted": receipt.manifest.semantic.encrypted,
    }))
}

/// Renders the human lines for `export`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    vec![
        "Academic OS deterministic synthetic export".to_owned(),
        format!(
            "destination: {}",
            value["destination"].as_str().unwrap_or("")
        ),
        format!("format: {}", value["format"].as_str().unwrap_or("")),
        format!(
            "daemon consulted: {}",
            value["ownership"]["daemon_owns_profile"]
        ),
        format!(
            "semantic digest: {}",
            value["semantic_digest"].as_str().unwrap_or("")
        ),
        format!(
            "canonical semantic digest: {}",
            value["canonical_semantic_digest"].as_str().unwrap_or("")
        ),
        format!("accept_seq head: {}", value["watermark"]["accept_seq_head"]),
        format!("files: {}", value["file_count"]),
        format!("projections included: {}", value["projections_included"]),
        format!("encrypted: {}", value["encrypted"]),
    ]
}
