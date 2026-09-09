//! Encrypted transport startup remains unavailable before any service side effect.
//!
//! D3 supplies an opt-in RPC identity description, not an operational service.
//! D2/D4 must supply complete recognized source closure before selected-service
//! admission can be composed with the existing path and process-singleton rules.
//! Neither a keyed D1 session, a scope-only read, nor a profile marker is readiness.

use std::{convert::Infallible, path::Path};

/// The selected encrypted service has no reviewed startup composition yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("encrypted synthetic service unavailable: selected-service admission is not implemented")]
pub struct EncryptedSyntheticStartupUnavailable;

/// Refuses before opening a profile, creating runtime files, binding a listener,
/// or publishing a nonce/capability/posture. D4 is absent, so there is no success
/// value and the negotiated RPC scaffold cannot authorize this function to start.
pub fn start(
    _profile_root: &Path,
    _runtime_root: &Path,
) -> Result<Infallible, EncryptedSyntheticStartupUnavailable> {
    Err(EncryptedSyntheticStartupUnavailable)
}
