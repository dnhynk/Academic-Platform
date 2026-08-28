//! `academic ingest` — the only canonical mutation this binary can request.
//!
//! Ingest always travels over local IPC. There is no offline path, because the
//! daemon is the only canonical writer. `--fixture` names one entry in the
//! repository allowlist; it is not a file path and cannot select arbitrary
//! bytes. The name is refused here before a connection is opened and refused
//! again by the daemon, so neither side is the only guard.

use std::path::Path;

use academic_core::{
    local_service::PHASE1_SYNTHETIC_FIXTURE_ID,
    operations::{SYNTHETIC_INGEST_CAPABILITY, synthetic_ingest_request},
};
use academic_rpc::generated::{MutableResponse, MutationStatus};
use serde_json::json;

use crate::{
    client::send_mutation,
    commands::{classify, daemon::handshake_json, daemon::require_session, display},
    output::{CliFailure, CommandResult, ExitClass},
};

/// Every fixture identifier this binary may ask the daemon to accept.
///
/// The list is a compile-time constant. No flag, environment variable, or
/// configuration key extends it, and an entry outside it is refused before any
/// connection is opened.
pub const ALLOWLISTED_FIXTURE_IDS: &[&str] = &[PHASE1_SYNTHETIC_FIXTURE_ID];

/// Returns whether one fixture identifier is on the compile-time allowlist.
#[must_use]
pub fn is_allowlisted(fixture_id: &str) -> bool {
    ALLOWLISTED_FIXTURE_IDS.contains(&fixture_id)
}

fn status_name(value: i32) -> &'static str {
    match MutationStatus::try_from(value) {
        Ok(MutationStatus::Accepted) => "ACCEPTED",
        Ok(MutationStatus::Duplicate) => "DUPLICATE",
        Ok(MutationStatus::Rejected) => "REJECTED",
        _ => "UNSPECIFIED",
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn receipt_json(response: &MutableResponse) -> serde_json::Value {
    json!({
        "status": status_name(response.status),
        "reason": response.reason,
        "profile_revision": response.profile_revision,
        "response_digest": hex_lower(&response.response_digest),
        "acceptance_range": response.acceptance_range.as_ref().map(|range| json!({
            "accept_seq_start": range.accept_seq_start,
            "accept_seq_end": range.accept_seq_end,
        })),
        "receipt": response.receipt.as_ref().map(|receipt| json!({
            "receipt_id": hex_lower(&receipt.receipt_id),
            "request_id": hex_lower(&receipt.request_id),
            "client_instance_id": hex_lower(&receipt.client_instance_id),
            "idempotency_key": hex_lower(&receipt.idempotency_key),
            "request_digest": hex_lower(&receipt.request_digest),
            "profile_revision": receipt.profile_revision,
        })),
    })
}

/// Classifies a daemon rejection reason into an outcome class.
///
/// The reasons are the daemon's stable codes, so this mapping is exhaustive
/// over what the Phase 1 writer can answer and falls back to `INTERNAL` rather
/// than guessing.
fn classify_rejection(reason: &str) -> ExitClass {
    match reason {
        "FIXTURE_NOT_ALLOWLISTED"
        | "BACKUP_NOT_AVAILABLE_UNTIL_B1"
        | "RESTORE_NOT_AVAILABLE_UNTIL_B1" => ExitClass::PolicyDenied,
        "REVISION_CONFLICT" | "IDEMPOTENCY_KEY_COLLISION" => ExitClass::Conflict,
        "REQUEST_DIGEST_MISMATCH" => ExitClass::Incompatible,
        _ => ExitClass::Internal,
    }
}

/// Sends one synthetic-ingest command over IPC and reports the receipt.
pub async fn run(
    profile_root: &Path,
    runtime_root: &Path,
    fixture_id: &str,
    expected_revision: Option<u64>,
) -> CommandResult {
    if !is_allowlisted(fixture_id) {
        return Err(CliFailure::new(
            ExitClass::PolicyDenied,
            "FIXTURE_NOT_ALLOWLISTED",
            format!(
                "{fixture_id} is not a repository-allowlisted synthetic fixture; \
                 only deterministic committed fixtures may be ingested"
            ),
        ));
    }
    let session = require_session(runtime_root, profile_root)?;
    let request = synthetic_ingest_request(fixture_id, expected_revision)
        .map_err(|error| classify("INGEST_REQUEST_BUILD_FAILED", &error))?;
    let (handshake, response) =
        send_mutation(&session, SYNTHETIC_INGEST_CAPABILITY, request).await?;

    let value = json!({
        "fixture_id": fixture_id,
        "profile_root": display(profile_root),
        "transport": "LOCAL_IPC",
        "endpoint": session.endpoint.display_value(),
        "handshake": handshake_json(&handshake),
        "acceptance": receipt_json(&response),
    });

    if MutationStatus::try_from(response.status) == Ok(MutationStatus::Rejected) {
        return Err(CliFailure::new(
            classify_rejection(&response.reason),
            response.reason.clone(),
            "the daemon rejected the synthetic ingest command",
        )
        .with_result(value));
    }
    Ok(value)
}

/// Renders the human lines for `ingest`.
pub fn lines(value: &serde_json::Value) -> Vec<String> {
    let acceptance = &value["acceptance"];
    let mut lines = vec![
        "Academic OS synthetic ingest".to_owned(),
        format!("fixture: {}", value["fixture_id"].as_str().unwrap_or("")),
        format!("transport: {}", value["transport"].as_str().unwrap_or("")),
        format!("status: {}", acceptance["status"].as_str().unwrap_or("")),
        format!("profile revision: {}", acceptance["profile_revision"]),
    ];
    if let Some(range) = acceptance["acceptance_range"].as_object() {
        lines.push(format!(
            "accept_seq range: {}..={}",
            range["accept_seq_start"], range["accept_seq_end"]
        ));
    }
    if let Some(receipt) = acceptance["receipt"].as_object() {
        lines.push(format!(
            "receipt id: {}",
            receipt["receipt_id"].as_str().unwrap_or("")
        ));
        lines.push(format!(
            "idempotency key: {}",
            receipt["idempotency_key"].as_str().unwrap_or("")
        ));
    }
    lines.push(format!(
        "response digest: {}",
        acceptance["response_digest"].as_str().unwrap_or("")
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allowlist_holds_only_the_committed_synthetic_fixture() {
        assert_eq!(ALLOWLISTED_FIXTURE_IDS, [PHASE1_SYNTHETIC_FIXTURE_ID]);
        assert!(is_allowlisted(PHASE1_SYNTHETIC_FIXTURE_ID));
    }

    #[test]
    fn no_real_or_arbitrary_input_is_allowlisted() {
        for candidate in [
            "",
            "real-data",
            "production",
            "../../etc/passwd",
            "C:/Users/someone/transcript.pdf",
            "phase0-synthetic-bitemporal-ledger-v1",
            "PHASE0-SYNTHETIC-BITEMPORAL-LEDGER-V2",
        ] {
            assert!(
                !is_allowlisted(candidate),
                "{candidate} must never be accepted"
            );
        }
    }

    #[test]
    fn every_daemon_rejection_reason_maps_to_a_nonzero_class() {
        for reason in [
            "FIXTURE_NOT_ALLOWLISTED",
            "BACKUP_NOT_AVAILABLE_UNTIL_B1",
            "RESTORE_NOT_AVAILABLE_UNTIL_B1",
            "REVISION_CONFLICT",
            "IDEMPOTENCY_KEY_COLLISION",
            "REQUEST_DIGEST_MISMATCH",
            "RESOURCE_EXHAUSTED",
            "SHUTTING_DOWN",
        ] {
            assert_ne!(classify_rejection(reason).code(), 0, "{reason}");
        }
        assert_eq!(
            classify_rejection("FIXTURE_NOT_ALLOWLISTED"),
            ExitClass::PolicyDenied
        );
        assert_eq!(classify_rejection("REVISION_CONFLICT"), ExitClass::Conflict);
    }
}
