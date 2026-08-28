//! One module per command surface.
//!
//! Every command returns a structured value plus the human lines that describe
//! it, so `output` can render either representation from one source and both
//! carry the policy object.

pub mod backup;
pub mod crash_replay;
pub mod daemon;
pub mod doctor;
pub mod export;
pub mod fixture;
pub mod ingest;
pub mod ownership;
pub mod restore;

use std::path::{Path, PathBuf};

use academic_core::operations::{FailureClass, OperationError};

use crate::output::{CliFailure, ExitClass};

/// Maps a composition failure onto the CLI outcome taxonomy.
///
/// The classification itself lives beside the errors in `academic-core`; this
/// is only the one-to-one translation into process exit classes.
pub fn classify(operation: &'static str, error: &OperationError) -> CliFailure {
    let class = match error.classify() {
        FailureClass::PolicyDenied => ExitClass::PolicyDenied,
        FailureClass::Conflict => ExitClass::Conflict,
        FailureClass::RepairRequired => ExitClass::RepairRequired,
        FailureClass::Incompatible => ExitClass::Incompatible,
        FailureClass::Internal => ExitClass::Internal,
    };
    CliFailure::new(class, operation, error.to_string())
}

/// Normalizes a caller-supplied path to the host's native absolute form.
///
/// This matters on Windows. The durability layer addresses files through
/// verbatim `\?\` paths, and Windows performs **no** normalization on a
/// verbatim path: a forward slash a caller typed is then read as an ordinary
/// filename character rather than a separator, and every open below that
/// profile fails with `ERROR_PATH_NOT_FOUND`. Normalizing once here, at the
/// argument boundary, keeps every downstream primitive on a native path.
///
/// The path is not required to exist, so a destination that must be absent
/// normalizes just as well as a profile that already exists.
pub fn native_path(path: &Path) -> Result<PathBuf, CliFailure> {
    std::path::absolute(path).map_err(|error| {
        CliFailure::new(
            ExitClass::Internal,
            "PATH_NOT_RESOLVABLE",
            format!("{}: {error}", path.display()),
        )
    })
}

/// Renders a path for output without leaking anything beyond the path itself.
pub fn display(path: &Path) -> String {
    path.display().to_string()
}
