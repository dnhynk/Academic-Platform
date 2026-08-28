//! `academic restore` — rebuild one verified backup into a new empty profile.
//!
//! Restore is the one data command that is offline by contract: it targets a
//! destination no daemon owns and no daemon has ever opened, so there is
//! nothing to negotiate with. It refuses a destination that already holds
//! anything, verifies the backup manifest and every file hash, replays every
//! signed envelope against trust anchors taken from this build rather than from
//! the restored bytes, rebuilds every projection generation from empty, and
//! only then publishes the profile directory.
//!
//! There is no in-place restore and no way to point it at a live profile.

use std::path::Path;

use academic_core::operations::restore_synthetic_profile;
use serde_json::json;

use crate::{
    client::read_session_metadata,
    commands::{classify, display},
    output::{CliFailure, CommandResult, ExitClass},
};

/// Restores one verified backup into a new empty profile directory.
pub fn run(backup_root: &Path, new_profile: &Path, runtime_root: &Path) -> CommandResult {
    // A destination a daemon already owns is a conflict, not a restore target.
    if read_session_metadata(runtime_root, new_profile)?.is_some() {
        return Err(CliFailure::new(
            ExitClass::Conflict,
            "DESTINATION_OWNED_BY_DAEMON",
            "a daemon already owns the restore destination",
        ));
    }
    let receipt = restore_synthetic_profile(backup_root, new_profile)
        .map_err(|error| classify("RESTORE_FAILED", &error))?;

    Ok(json!({
        "backup_root": display(backup_root),
        "new_profile": display(&receipt.destination),
        "mode": "OFFLINE_NEW_EMPTY_PROFILE",
        "canonical_semantic_digest": receipt.canonical_semantic_digest,
        "watermark": {
            "accept_seq_head": receipt.manifest.semantic.watermark.accept_seq_head,
            "outbox_head": receipt.manifest.semantic.watermark.outbox_head,
        },
        "replay": {
            "verified_batches": receipt.replay.verified_batches,
            "verified_events": receipt.replay.verified_events,
            "device_heads": receipt.replay.device_heads,
        },
        "projections": receipt
            .projections
            .iter()
            .map(|projection| json!({
                "kind": projection.kind,
                "domain": projection.domain,
                "record_count": projection.record_count,
                "canonical_checksum": projection.canonical_checksum,
                "activated": projection.activated,
            }))
            .collect::<Vec<_>>(),
    }))
}

/// Renders the human lines for `restore`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    let mut lines = vec![
        "Academic OS empty-profile restore".to_owned(),
        format!(
            "backup source: {}",
            value["backup_root"].as_str().unwrap_or("")
        ),
        format!(
            "new profile: {}",
            value["new_profile"].as_str().unwrap_or("")
        ),
        format!("mode: {}", value["mode"].as_str().unwrap_or("")),
        format!("accept_seq head: {}", value["watermark"]["accept_seq_head"]),
        format!(
            "replayed batches: {} events: {} device heads: {}",
            value["replay"]["verified_batches"],
            value["replay"]["verified_events"],
            value["replay"]["device_heads"]
        ),
        format!(
            "canonical semantic digest: {}",
            value["canonical_semantic_digest"].as_str().unwrap_or("")
        ),
    ];
    if let Some(projections) = value["projections"].as_array() {
        for projection in projections {
            lines.push(format!(
                "rebuilt projection {}: records={} activated={}",
                projection["kind"].as_str().unwrap_or(""),
                projection["record_count"],
                projection["activated"]
            ));
        }
    }
    lines
}
