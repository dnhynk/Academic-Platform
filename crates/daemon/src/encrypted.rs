//! D1 refuses encrypted transport startup before any service side effect.
//!
//! D3 must supply the truthful non-admitted encrypted posture, reconcile the
//! selected service, and acquire the existing process singleton. D2/D4 must
//! supply complete recognized source closure. A profile marker is not readiness.

use std::{convert::Infallible, path::Path};

/// The selected encrypted service has no reviewed startup composition yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("encrypted synthetic service unavailable: selected-service admission is not implemented")]
pub struct EncryptedSyntheticStartupUnavailable;

/// Refuses before opening a profile, creating runtime files, binding a listener,
/// or publishing a nonce/capability/posture. There is no success value in D1.
pub fn start(
    _profile_root: &Path,
    _runtime_root: &Path,
) -> Result<Infallible, EncryptedSyntheticStartupUnavailable> {
    Err(EncryptedSyntheticStartupUnavailable)
}
