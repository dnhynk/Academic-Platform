//! One module per command surface.
//!
//! Every command returns a structured value plus the human lines that describe
//! it, so `output` can render either representation from one source and both
//! carry the policy object.

pub mod admission;
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
        FailureClass::PathRejected => ExitClass::PathRejected,
        FailureClass::Internal => ExitClass::Internal,
    };
    CliFailure::new(class, operation, error.to_string())
}

/// Normalizes a caller-supplied path to the host's native absolute form.
///
/// Two of the three things this does are load-bearing for the vault below it,
/// on Windows.
///
/// Absolutization: the Windows durability layer applies its verbatim prefix
/// only to a rooted spelling and leaves a non-absolute one to Win32, and its
/// handle-rename builder refuses a non-absolute spelling outright. A relative
/// `--profile profile` therefore has to become absolute before it reaches a
/// durable primitive, and the argument boundary is the only place that owns
/// the process working directory it is relative to.
///
/// Dot resolution: `.` and `..` are a typed vault error rather than something
/// the verbatim namespace can resolve, deliberately, because collapsing
/// `a\..\b` lexically is only correct when `a` is not a link. `std::path::absolute`
/// resolves them here through `GetFullPathNameW`, which is the composition-root
/// resolution that error defers to.
///
/// Separator spelling is *not* one of those things. `crates/vault` normalizes
/// separators itself before it applies any prefix, for every caller, so a
/// forward-slash argument is addressed correctly with or without this call;
/// `cli_accepts_forward_slash_paths_on_every_host` holds that from the CLI side.
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
