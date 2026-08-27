//! Synthetic-only Phase 1 local-core daemon.
//!
//! Startup validates one profile, acquires one current-user/profile singleton,
//! reconciles V1 on the dedicated writer thread, publishes a fresh session
//! nonce, and only then exposes a current-user-only local transport.

mod readers;
mod runtime_meta;
mod service;
mod shutdown;
mod singleton;
mod transport;
mod writer;

use std::{fmt, io};

use academic_core::local_service::LocalServiceError;
use academic_rpc::RpcError;
use academic_store::error::StoreError;
use thiserror::Error;

pub use readers::ReaderFactory;
pub use runtime_meta::SessionNonce;
pub use service::{DaemonConfig, RunningDaemon};
pub use transport::LocalEndpoint;
pub use writer::{AdmissionError, AdmittedMutation, WriterQueue};

/// Product binary name for the local-core daemon.
pub const DAEMON_BINARY_NAME: &str = "academicd";
/// Reversible Phase 1 bounded-writer queue default.
pub const WRITER_QUEUE_CAPACITY: usize = 64;
/// Capability prefix carrying the fresh current-session nonce.
pub const SESSION_NONCE_CAPABILITY_PREFIX: &str = "learning-platform.local.session-nonce.";

/// Fail-closed daemon startup or transport error.
#[derive(Debug, Error)]
pub enum DaemonError {
    /// The profile failed S1 validation or opening.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The V1/S2 local composition failed.
    #[error(transparent)]
    LocalService(#[from] LocalServiceError),
    /// P1 framing, validation, negotiation, or authorization failed.
    #[error(transparent)]
    Rpc(#[from] RpcError),
    /// An operating-system boundary failed.
    #[error("{operation} failed for {path}: {source}")]
    Io {
        /// Stable operation label.
        operation: &'static str,
        /// Relevant local path or endpoint.
        path: String,
        /// Native error.
        #[source]
        source: io::Error,
    },
    /// Another daemon owns this current-user/profile identity.
    #[error("another daemon already owns this current-user profile")]
    AlreadyRunning,
    /// The session metadata was malformed or could not be published safely.
    #[error("session metadata is invalid: {0}")]
    InvalidSessionMetadata(&'static str),
    /// The listener task failed or was cancelled unexpectedly.
    #[error("local listener task failed: {0}")]
    ListenerTask(String),
}

pub(crate) fn daemon_io(
    operation: &'static str,
    path: impl fmt::Display,
    source: io::Error,
) -> DaemonError {
    DaemonError::Io {
        operation,
        path: path.to_string(),
        source,
    }
}
