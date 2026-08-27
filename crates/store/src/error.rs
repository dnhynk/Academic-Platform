//! Errors returned by the synthetic-only profile and SQLite boundary.

use std::{error::Error, fmt, io, path::PathBuf};

use crate::path_policy::PathPolicyViolation;

/// Result type used by the store boundary.
pub type StoreResult<T> = Result<T, StoreError>;

/// Fail-closed errors from profile policy, migration, and connection setup.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// The requested profile location failed a path-policy check.
    UnsafeProfilePath(PathPolicyViolation),
    /// A filesystem operation failed.
    Io {
        /// Operation being attempted.
        operation: &'static str,
        /// Path involved in the operation.
        path: PathBuf,
        /// Original operating-system error.
        source: io::Error,
    },
    /// SQLite rejected an operation or returned an unexpected result.
    Sqlite(rusqlite::Error),
    /// The profile contains an explicit interrupted-bootstrap marker.
    IncompleteProfile(PathBuf),
    /// The mandatory plaintext warning marker is absent or has different bytes.
    InvalidPolicyMarker(PathBuf),
    /// The profile is not in the state required by the requested operation.
    InvalidProfileState {
        /// Profile root being inspected.
        path: PathBuf,
        /// Stable diagnostic reason.
        reason: &'static str,
    },
    /// A manifest field did not match the single allowlisted synthetic fixture.
    ManifestRejected {
        /// Rejected manifest field.
        field: &'static str,
    },
    /// SQLite connection configuration did not read back exactly.
    PragmaMismatch {
        /// PRAGMA whose observed value was wrong.
        pragma: &'static str,
        /// Expected normalized value.
        expected: String,
        /// Observed normalized value.
        actual: String,
    },
    /// The file is not an Academic Platform store or has inconsistent identity.
    SchemaIdentityMismatch {
        /// Identity component that disagreed.
        component: &'static str,
        /// Expected normalized value.
        expected: String,
        /// Observed normalized value.
        actual: String,
    },
    /// A database with a newer schema cannot be migrated by this binary.
    NewerSchema {
        /// Schema version observed in SQLite.
        found: u32,
        /// Newest schema version supported by this binary.
        supported: u32,
    },
    /// A database is neither new nor exactly the current supported schema.
    UnsupportedMigrationState {
        /// SQLite `application_id` value.
        application_id: i64,
        /// SQLite `user_version` value.
        user_version: i64,
    },
    /// SQLite cannot represent the supplied unsigned value without loss.
    UnsignedIntegerOverflow(u64),
    /// Required bundled SQLite behavior is unavailable.
    UnsupportedSqliteBuild(&'static str),
}

impl StoreError {
    /// Builds an I/O error without discarding the failed path or operation.
    #[must_use]
    pub fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeProfilePath(reason) => write!(formatter, "unsafe profile path: {reason}"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{operation} failed for {}: {source}",
                path.display()
            ),
            Self::Sqlite(source) => write!(formatter, "SQLite error: {source}"),
            Self::IncompleteProfile(path) => {
                write!(
                    formatter,
                    "profile bootstrap is incomplete: {}",
                    path.display()
                )
            }
            Self::InvalidPolicyMarker(path) => write!(
                formatter,
                "mandatory synthetic-only profile marker is missing or invalid: {}",
                path.display()
            ),
            Self::InvalidProfileState { path, reason } => {
                write!(
                    formatter,
                    "invalid profile state at {}: {reason}",
                    path.display()
                )
            }
            Self::ManifestRejected { field } => {
                write!(formatter, "synthetic manifest rejected at field {field}")
            }
            Self::PragmaMismatch {
                pragma,
                expected,
                actual,
            } => write!(
                formatter,
                "SQLite PRAGMA {pragma} mismatch: expected {expected}, observed {actual}"
            ),
            Self::SchemaIdentityMismatch {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "store schema identity {component} mismatch: expected {expected}, observed {actual}"
            ),
            Self::NewerSchema { found, supported } => write!(
                formatter,
                "store schema {found} is newer than supported schema {supported}"
            ),
            Self::UnsupportedMigrationState {
                application_id,
                user_version,
            } => write!(
                formatter,
                "database is not new or exactly current (application_id={application_id}, user_version={user_version})"
            ),
            Self::UnsignedIntegerOverflow(value) => write!(
                formatter,
                "unsigned value {value} exceeds SQLite's signed 64-bit integer range"
            ),
            Self::UnsupportedSqliteBuild(reason) => {
                write!(formatter, "unsupported bundled SQLite build: {reason}")
            }
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Sqlite(source) => Some(source),
            Self::UnsafeProfilePath(_)
            | Self::IncompleteProfile(_)
            | Self::InvalidPolicyMarker(_)
            | Self::InvalidProfileState { .. }
            | Self::ManifestRejected { .. }
            | Self::PragmaMismatch { .. }
            | Self::SchemaIdentityMismatch { .. }
            | Self::NewerSchema { .. }
            | Self::UnsupportedMigrationState { .. }
            | Self::UnsignedIntegerOverflow(_)
            | Self::UnsupportedSqliteBuild(_) => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(source: rusqlite::Error) -> Self {
        Self::Sqlite(source)
    }
}

impl From<PathPolicyViolation> for StoreError {
    fn from(source: PathPolicyViolation) -> Self {
        Self::UnsafeProfilePath(source)
    }
}
