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

use std::{fmt, io, time::Duration};

use academic_core::local_service::LocalServiceError;
use academic_rpc::RpcError;
use academic_store::error::StoreError;
use thiserror::Error;

pub use readers::ReaderFactory;
pub use runtime_meta::SessionNonce;
pub use service::{DaemonConfig, RunningDaemon};
pub use transport::LocalEndpoint;
#[cfg(unix)]
pub use transport::MAX_UNIX_ENDPOINT_PATH_LEN;
pub use writer::{AdmissionError, AdmittedMutation, WriterQueue};

/// Product binary name for the local-core daemon.
pub const DAEMON_BINARY_NAME: &str = "academicd";
/// Reversible Phase 1 bounded-writer queue default.
pub const WRITER_QUEUE_CAPACITY: usize = 64;
/// Maximum number of connections served at the same time.
///
/// One connection carries at most one queued mutation, so half
/// [`WRITER_QUEUE_CAPACITY`] cannot starve the writer lane, and half the
/// 64-instance Windows named-pipe ceiling always leaves the listener room to
/// create its replacement instance. Above this bound the listener stops
/// accepting and only drains, so held-open connections can no longer grow the
/// number of live serve tasks, descriptors, or transport instances without
/// limit.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 32;
/// Bounded wait for one client frame before the connection is closed.
///
/// Both frames of the one-request-per-connection protocol arrive from a
/// same-host process over a named pipe or Unix socket, so a healthy client
/// needs milliseconds and never a network round trip. Ten seconds is far above
/// any local scheduling delay and still stops a stalled or hostile client from
/// holding a served slot for the lifetime of the daemon.
pub const CLIENT_FRAME_TIMEOUT: Duration = Duration::from_secs(10);
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
    /// The assembled local endpoint path exceeds the platform address bound.
    ///
    /// On Unix the whole absolute socket path travels in `sun_path`, so a
    /// runtime root that leaves no room for the per-profile suffix cannot host
    /// a profile. Startup reports the bound, the measured length, and the path
    /// it refused rather than shortening it or letting `bind` fail obscurely.
    #[error("local endpoint path is {length} bytes, above the {limit}-byte platform limit: {path}")]
    EndpointPathTooLong {
        /// Longest endpoint path the platform address can carry.
        limit: usize,
        /// Measured length of the assembled endpoint path.
        length: usize,
        /// The offending assembled path, never truncated to fit.
        path: String,
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
