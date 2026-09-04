//! The closed refusal set of export schema v2.
//!
//! Every arm names the exact item that failed. A bundle that cannot be read is
//! useless in proportion to how little it says about why, and the reader is the
//! half that runs on a machine where nobody can add a print statement.

use std::{fmt, path::Path, path::PathBuf};

/// Result alias for every operation in this crate.
pub type ExportResult<T> = Result<T, ExportError>;

/// What a bundle write or read refused, and over which item.
#[derive(Debug)]
pub enum ExportError {
    /// A filesystem operation failed, naming the operation and the path.
    Io {
        /// What was being done.
        operation: &'static str,
        /// The path it was being done to.
        path: PathBuf,
        /// The underlying failure.
        source: std::io::Error,
    },
    /// JSON encoding or decoding failed.
    Json {
        /// What was being encoded or decoded.
        operation: &'static str,
        /// The underlying failure.
        source: serde_json::Error,
    },
    /// An observed value differs from the one the bundle records.
    Mismatch {
        /// The item compared.
        item: &'static str,
        /// What was recorded.
        expected: String,
        /// What was observed.
        observed: String,
    },
    /// A destination that must not exist already does.
    DestinationExists(PathBuf),
    /// A relative path a bundle may not contain.
    UnportablePath {
        /// Why it is refused.
        reason: &'static str,
        /// The offending relative path.
        path: String,
    },
    /// A path the manifest references that the file inventory does not list.
    DanglingLocator {
        /// Where the reference was found.
        referenced_by: &'static str,
        /// The path that resolves to nothing.
        path: String,
    },
    /// A file on disk that the manifest does not list.
    UnlistedFile(String),
    /// A domain with no recorded source copyright notice.
    NoticeAbsent {
        /// The security domain that has no notice.
        domain_id: String,
    },
    /// A required item is absent from the bundle.
    Absent {
        /// The item.
        item: &'static str,
        /// Its identity.
        value: String,
    },
    /// A recorded value is not of the shape the format admits.
    Malformed {
        /// The item.
        item: &'static str,
        /// The offending text.
        value: String,
    },
    /// The audit could not be re-run from what the bundle carries.
    AuditNotReproduced(&'static str),
    /// The graduation audit engine refused the re-run.
    Audit(academic_audit::AuditError),
    /// A deterministic engine refused the re-run.
    Engine(academic_domain::engines::EngineError),
}

impl ExportError {
    /// Builds an I/O refusal.
    pub fn io(operation: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            operation,
            path: path.to_path_buf(),
            source,
        }
    }

    /// Builds a comparison refusal.
    pub fn mismatch(
        item: &'static str,
        expected: impl fmt::Display,
        observed: impl fmt::Display,
    ) -> Self {
        Self::Mismatch {
            item,
            expected: expected.to_string(),
            observed: observed.to_string(),
        }
    }
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation, path, ..
            } => write!(formatter, "{operation} failed for {}", path.display()),
            Self::Json { operation, .. } => write!(formatter, "{operation} failed"),
            Self::Mismatch {
                item,
                expected,
                observed,
            } => write!(
                formatter,
                "{item} recorded {expected} and observed {observed}"
            ),
            Self::DestinationExists(path) => {
                write!(formatter, "destination {} already exists", path.display())
            }
            Self::UnportablePath { reason, path } => {
                write!(formatter, "relative path {path} is refused: {reason}")
            }
            Self::DanglingLocator {
                referenced_by,
                path,
            } => write!(
                formatter,
                "{referenced_by} references {path}, which the file inventory does not list"
            ),
            Self::UnlistedFile(path) => {
                write!(
                    formatter,
                    "{path} is present but the manifest does not list it"
                )
            }
            Self::NoticeAbsent { domain_id } => write!(
                formatter,
                "security domain {domain_id} has no recorded source copyright notice"
            ),
            Self::Absent { item, value } => write!(formatter, "{item} {value} is absent"),
            Self::Malformed { item, value } => {
                write!(formatter, "{item} is malformed: {value}")
            }
            Self::AuditNotReproduced(reason) => {
                write!(formatter, "the recorded audit was not reproduced: {reason}")
            }
            Self::Audit(source) => write!(formatter, "graduation audit refused: {source}"),
            Self::Engine(source) => write!(formatter, "deterministic engine refused: {source}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Audit(source) => Some(source),
            Self::Engine(source) => Some(source),
            _ => None,
        }
    }
}

impl From<academic_audit::AuditError> for ExportError {
    fn from(source: academic_audit::AuditError) -> Self {
        Self::Audit(source)
    }
}

impl From<academic_domain::engines::EngineError> for ExportError {
    fn from(source: academic_domain::engines::EngineError) -> Self {
        Self::Engine(source)
    }
}
