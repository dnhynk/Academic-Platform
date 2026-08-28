//! `academic backup` — one plaintext synthetic backup directory.
//!
//! The Phase 1 backup protects nothing. It is not encrypted, not confidential,
//! and not evidence for any at-rest gate; the manifest and this command both
//! say so. It exists to prove watermark fixing, reachable-object closure, and
//! atomic publication.
//!
//! Like `export`, it consults the owning daemon first when one exists. The
//! frozen `SyntheticBackupCommand` carries no destination field, so a
//! destination-bearing backup cannot be expressed over IPC in this phase.

use std::path::Path;

use academic_core::operations::{
    BACKUP_PLAINTEXT_WARNING, SYNTHETIC_EXPORT_CAPABILITY, backup_synthetic_profile,
};
use serde_json::json;

use crate::{
    commands::{classify, display, ownership::consult_owning_daemon},
    output::CommandResult,
};

/// Publishes one plaintext synthetic backup directory.
pub async fn run(profile_root: &Path, destination: &Path, runtime_root: &Path) -> CommandResult {
    let ownership =
        consult_owning_daemon(runtime_root, profile_root, SYNTHETIC_EXPORT_CAPABILITY).await?;
    let receipt = backup_synthetic_profile(profile_root, destination)
        .map_err(|error| classify("BACKUP_FAILED", &error))?;

    Ok(json!({
        "profile_root": display(profile_root),
        "destination": display(&receipt.destination),
        "format": academic_core::operations::BACKUP_FORMAT,
        "ownership": ownership,
        "encrypted": receipt.manifest.semantic.encrypted,
        "confidentiality_warning": BACKUP_PLAINTEXT_WARNING,
        "semantic_digest": receipt.manifest.semantic_digest,
        "canonical_semantic_digest": receipt.manifest.semantic.canonical_semantic_digest,
        "watermark": {
            "accept_seq_head": receipt.manifest.semantic.watermark.accept_seq_head,
            "outbox_head": receipt.manifest.semantic.watermark.outbox_head,
        },
        "object_count": receipt.manifest.semantic.objects.len(),
        "device_head_count": receipt.manifest.semantic.device_heads.len(),
    }))
}

/// Renders the human lines for `backup`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    vec![
        "Academic OS plaintext synthetic backup".to_owned(),
        format!(
            "destination: {}",
            value["destination"].as_str().unwrap_or("")
        ),
        format!("format: {}", value["format"].as_str().unwrap_or("")),
        format!("encrypted: {}", value["encrypted"]),
        format!(
            "warning: {}",
            value["confidentiality_warning"].as_str().unwrap_or("")
        ),
        format!(
            "daemon consulted: {}",
            value["ownership"]["daemon_owns_profile"]
        ),
        format!(
            "semantic digest: {}",
            value["semantic_digest"].as_str().unwrap_or("")
        ),
        format!("accept_seq head: {}", value["watermark"]["accept_seq_head"]),
        format!("sealed objects: {}", value["object_count"]),
    ]
}
