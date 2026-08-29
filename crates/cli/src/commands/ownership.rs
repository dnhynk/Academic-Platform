//! Daemon ownership consultation shared by the read-only data commands.
//!
//! `export` and `backup` read a profile the daemon may currently own. They
//! never bypass it silently: if a daemon has published session metadata, the
//! CLI completes the versioned handshake first, which proves a live daemon owns
//! the profile, accepted the published session nonce, and granted the
//! capability. That keeps the running daemon the authority over its own profile
//! even though the frozen Phase 1 protocol carries no export or
//! destination-bearing backup command.
//!
//! The locked and repair-required refusals below cannot fire against a Phase 1
//! daemon: it answers every handshake from `ServerHandshakeConfig::default()`,
//! and it refuses to start at all on a repair-required profile, so no runtime
//! state can move `lock_state` off `UNLOCKED`. They are kept so a daemon that
//! does report either state is refused rather than silently accepted; they are
//! not a defence this phase provides. `docs/contracts/phase1-cli.md`, *What the
//! handshake does not carry*, records why that is deferred to Phase 2.

use std::path::Path;

use academic_rpc::generated::ProfileLockState;
use serde_json::json;

use crate::{
    client::{DAEMON_UNREACHABLE, handshake_only, read_session_metadata},
    commands::daemon::handshake_json,
    output::{CliFailure, ExitClass},
};

/// Consults the daemon that owns a profile, when one does.
///
/// Returns the ownership block recorded in the command result.
pub async fn consult_owning_daemon(
    runtime_root: &Path,
    profile_root: &Path,
    capability: &str,
) -> Result<serde_json::Value, CliFailure> {
    let Some(session) = read_session_metadata(runtime_root, profile_root)? else {
        return Ok(json!({
            "daemon_owns_profile": false,
            "mode": "OFFLINE",
        }));
    };
    // A daemon killed abruptly leaves its session file behind. The profile is
    // then unowned, not locked: falling back to the offline read is what makes
    // export and backup usable after a crash, which is exactly when they are
    // needed most. Anything other than an unreachable endpoint still fails.
    let handshake = match handshake_only(&session, &[capability]).await {
        Ok(handshake) => handshake,
        Err(failure) if failure.reason() == DAEMON_UNREACHABLE => {
            return Ok(json!({
                "daemon_owns_profile": false,
                "mode": "OFFLINE",
                "stale_session_metadata": true,
            }));
        }
        Err(failure) => return Err(failure),
    };
    let lock_state = ProfileLockState::try_from(handshake.lock_state);
    match lock_state {
        Ok(ProfileLockState::RepairRequired) => {
            return Err(CliFailure::new(
                ExitClass::RepairRequired,
                "PROFILE_REPAIR_REQUIRED",
                "the owning daemon reports the profile needs repair",
            ));
        }
        Ok(ProfileLockState::Locked) => {
            return Err(CliFailure::new(
                ExitClass::Conflict,
                "PROFILE_LOCKED",
                "the owning daemon reports the profile is locked",
            ));
        }
        _ => {}
    }
    if !handshake
        .capability_ids
        .iter()
        .any(|negotiated| negotiated == capability)
    {
        return Err(CliFailure::new(
            ExitClass::Incompatible,
            "CAPABILITY_NOT_NEGOTIATED",
            format!("the daemon did not negotiate {capability}"),
        ));
    }
    Ok(json!({
        "daemon_owns_profile": true,
        "mode": "DAEMON_CONSULTED",
        "endpoint": session.endpoint.display_value(),
        "handshake": handshake_json(&handshake),
    }))
}
