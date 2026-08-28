//! `academic daemon serve` and `academic daemon status`.
//!
//! `serve` hosts one foreground daemon: it is not a service manager, it
//! installs nothing, and it exits when the terminal interrupts it. `status`
//! completes a read-only handshake against a running daemon and pairs the
//! negotiated protocol facts with the canonical and projection watermarks read
//! from the profile.

use std::path::Path;

use academic_core::operations::{
    DIAGNOSTICS_CAPABILITY, diagnose_profile, ensure_synthetic_profile,
};
use academic_daemon::{DaemonConfig, RunningDaemon};
use academic_rpc::generated::{ProfileLockState, ServerHandshake, WriteDisposition};
use serde_json::json;

use crate::{
    client::{DAEMON_UNREACHABLE, SessionMetadata, handshake_only, read_session_metadata},
    commands::{classify, display},
    output::{CliFailure, CommandResult, ExitClass},
};

/// Renders the negotiated protocol facts of one handshake.
pub fn handshake_json(handshake: &ServerHandshake) -> serde_json::Value {
    json!({
        "protocol_name": handshake.protocol_name,
        "protocol_version": handshake.protocol_version.as_ref().map(version_json),
        "negotiated_protocol_version": handshake
            .negotiated_protocol_version
            .as_ref()
            .map(version_json),
        "minimum_client_version": handshake.minimum_client_version.as_ref().map(version_json),
        "daemon_build": handshake.daemon_build,
        "storage_schema": handshake.storage_schema.as_ref().map(|schema| json!({
            "number": schema.number,
            "semantic_version": schema.semantic_version,
        })),
        "vault_read_formats": handshake.vault_read_formats,
        "vault_write_format": handshake.vault_write_format,
        "lock_state": lock_state_name(handshake.lock_state),
        "write_disposition": write_disposition_name(handshake.write_disposition),
        "write_denial_reason": handshake.write_denial_reason,
        "negotiated_capability_ids": handshake.capability_ids,
    })
}

fn version_json(version: &academic_rpc::generated::ProtocolVersion) -> serde_json::Value {
    json!({ "major": version.major, "minor": version.minor })
}

fn lock_state_name(value: i32) -> &'static str {
    match ProfileLockState::try_from(value) {
        Ok(ProfileLockState::Unlocked) => "UNLOCKED",
        Ok(ProfileLockState::Locked) => "LOCKED",
        Ok(ProfileLockState::RepairRequired) => "REPAIR_REQUIRED",
        _ => "UNSPECIFIED",
    }
}

fn write_disposition_name(value: i32) -> &'static str {
    match WriteDisposition::try_from(value) {
        Ok(WriteDisposition::Allowed) => "ALLOWED",
        Ok(WriteDisposition::DeniedMajorVersion) => "DENIED_MAJOR_VERSION",
        Ok(WriteDisposition::DeniedUnknownCapability) => "DENIED_UNKNOWN_CAPABILITY",
        Ok(WriteDisposition::DeniedClientTooOld) => "DENIED_CLIENT_TOO_OLD",
        _ => "UNSPECIFIED",
    }
}

/// Requires that a daemon currently owns the profile.
pub fn require_session(
    runtime_root: &Path,
    profile_root: &Path,
) -> Result<SessionMetadata, CliFailure> {
    read_session_metadata(runtime_root, profile_root)?.ok_or_else(|| {
        CliFailure::new(
            ExitClass::Unavailable,
            "NO_DAEMON_OWNS_PROFILE",
            "no daemon has published session metadata for this profile",
        )
    })
}

/// Hosts one foreground daemon until the terminal interrupts it.
pub async fn serve(profile_root: &Path, runtime_root: &Path) -> CommandResult {
    // A throwaway profile is created on first serve. Creation goes through the
    // same fail-closed path policy as every other profile and writes the
    // synthetic-only marker; it never converts an existing directory.
    let disposition = ensure_synthetic_profile(profile_root)
        .map_err(|error| classify("PROFILE_OPEN_FAILED", &error))?;
    let daemon = RunningDaemon::start(DaemonConfig::new(profile_root, runtime_root))
        .await
        .map_err(|error| CliFailure::internal("DAEMON_START_FAILED", error))?;
    let endpoint = daemon.endpoint().display_value();
    let metadata_path = display(daemon.metadata_path());
    let reconciliation = daemon.startup().reconciliation().records().len();
    let profile_revision = daemon.startup().profile_revision();

    // Printed before the wait so a supervising harness can observe readiness
    // on standard error while standard output stays reserved for the result.
    eprintln!("READY endpoint={endpoint}");

    tokio::signal::ctrl_c()
        .await
        .map_err(|error| CliFailure::internal("SIGNAL_WAIT_FAILED", error))?;
    daemon
        .shutdown()
        .await
        .map_err(|error| CliFailure::internal("DAEMON_SHUTDOWN_FAILED", error))?;

    Ok(json!({
        "profile_root": display(profile_root),
        "runtime_root": display(runtime_root),
        "endpoint": endpoint,
        "session_metadata": metadata_path,
        "startup_profile_revision": profile_revision,
        "startup_reconciliation_records": reconciliation,
        "profile_disposition": disposition,
        "served": true,
        "shutdown": "GRACEFUL",
    }))
}

/// Reports negotiated protocol facts plus canonical and projection watermarks.
pub async fn status(profile_root: &Path, runtime_root: &Path) -> CommandResult {
    let session = require_session(runtime_root, profile_root)?;
    // Stale metadata means the daemon died without cleaning up, which is a
    // different fact from never having been started. Both leave the profile
    // unowned, so both are unavailable, but the reason distinguishes them.
    let handshake = match handshake_only(&session, &[DIAGNOSTICS_CAPABILITY]).await {
        Ok(handshake) => handshake,
        Err(failure) if failure.reason() == DAEMON_UNREACHABLE => {
            return Err(CliFailure::new(
                ExitClass::Unavailable,
                "DAEMON_NOT_RUNNING",
                "session metadata is present but no daemon answers the endpoint",
            ));
        }
        Err(failure) => return Err(failure),
    };

    // The handshake carries protocol and build identity. The watermarks come
    // from a read-only pass over the profile, because the Phase 1 handshake
    // does not yet carry a projection state block.
    let diagnosis = diagnose_profile(profile_root, true)
        .map_err(|error| classify("PROFILE_READ_FAILED", &error))?;

    Ok(json!({
        "running": true,
        "profile_root": display(profile_root),
        "session_metadata": display(&session.path),
        "endpoint": session.endpoint.display_value(),
        "handshake": handshake_json(&handshake),
        "watermarks": {
            "accept_seq_head": diagnosis.canonical.accept_seq_head,
            "outbox_head": diagnosis.canonical.outbox_head,
            "profile_revision": diagnosis.canonical.profile_revision,
        },
        "canonical": diagnosis.canonical,
        "store": diagnosis.store,
        "projections": diagnosis.projections,
    }))
}

/// Renders the human lines for `daemon status`.
pub fn status_lines(value: &serde_json::Value) -> Vec<String> {
    let mut lines = vec![
        "Academic OS local-core daemon status".to_owned(),
        format!("running: {}", value["running"]),
        format!("endpoint: {}", value["endpoint"].as_str().unwrap_or("")),
        format!(
            "daemon build: {}",
            value["handshake"]["daemon_build"].as_str().unwrap_or("")
        ),
        format!(
            "protocol: {}.{}",
            value["handshake"]["protocol_version"]["major"],
            value["handshake"]["protocol_version"]["minor"]
        ),
        format!(
            "minimum client: {}.{}",
            value["handshake"]["minimum_client_version"]["major"],
            value["handshake"]["minimum_client_version"]["minor"]
        ),
        format!(
            "storage schema: {} ({})",
            value["handshake"]["storage_schema"]["number"],
            value["handshake"]["storage_schema"]["semantic_version"]
                .as_str()
                .unwrap_or("")
        ),
        format!(
            "vault write format: {}",
            value["handshake"]["vault_write_format"]
                .as_str()
                .unwrap_or("")
        ),
        format!(
            "lock state: {}",
            value["handshake"]["lock_state"].as_str().unwrap_or("")
        ),
        format!(
            "write disposition: {}",
            value["handshake"]["write_disposition"]
                .as_str()
                .unwrap_or("")
        ),
        format!(
            "accept_seq head: {}",
            value["watermarks"]["accept_seq_head"]
        ),
        format!("outbox head: {}", value["watermarks"]["outbox_head"]),
        format!(
            "profile revision: {}",
            value["watermarks"]["profile_revision"]
        ),
    ];
    if let Some(projections) = value["projections"].as_array() {
        for projection in projections {
            lines.push(format!(
                "projection {}: active={} lag={}",
                projection["kind"].as_str().unwrap_or(""),
                projection["active"],
                projection["lag"]
            ));
        }
    }
    lines
}

/// Renders the human lines for `daemon serve`.
pub fn serve_lines(value: &serde_json::Value) -> Vec<String> {
    vec![
        "Academic OS local-core daemon".to_owned(),
        format!("endpoint: {}", value["endpoint"].as_str().unwrap_or("")),
        format!(
            "session metadata: {}",
            value["session_metadata"].as_str().unwrap_or("")
        ),
        format!(
            "profile: {} ({})",
            value["profile_root"].as_str().unwrap_or(""),
            value["profile_disposition"].as_str().unwrap_or("")
        ),
        format!(
            "startup profile revision: {}",
            value["startup_profile_revision"]
        ),
        format!("shutdown: {}", value["shutdown"].as_str().unwrap_or("")),
    ]
}
