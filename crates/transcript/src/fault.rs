//! Deterministic `IN04` failpoints, compiled only by the explicit test feature.
//!
//! Default product builds contain no environment lookup and no crash switch,
//! exactly as `academic-vault` and `academic-retention` do it.
//!
//! `IN03` — a transcript row checksum mismatch — is error-induced, not
//! kill-induced. It needs no failpoint: it is driven through
//! [`crate::reconcile::reconcile`]'s public seam with a corpus whose two
//! readings disagree.

/// A point at which the harness may terminate this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "phase2-fault-injection"), allow(dead_code))]
pub(crate) enum FaultPoint {
    /// `IN04`: staging bytes are durable in a temporary file, not yet renamed.
    StagingTemporaryWritten,
    /// `IN04`: the staged set is durable; nothing is published.
    SetStaged,
    /// `IN04`: the confirmed set is durable; the lease is still held.
    SetPublished,
}

impl FaultPoint {
    #[cfg(feature = "phase2-fault-injection")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::StagingTemporaryWritten => "IN04:before-staging-rename",
            Self::SetStaged => "IN04:after-staging-rename",
            Self::SetPublished => "IN04:after-publish-rename",
        }
    }
}

#[cfg(feature = "phase2-fault-injection")]
pub(crate) fn trip(point: FaultPoint) {
    use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf, thread};

    if env::var(FAULT_SELECTION_VARIABLE).ok().as_deref() != Some(point.as_str()) {
        return;
    }
    if let Some(path) = env::var_os(FAULT_READY_MARKER_VARIABLE).map(PathBuf::from)
        && let Ok(mut marker) = OpenOptions::new().create_new(true).write(true).open(path)
    {
        let _ = marker.write_all(point.as_str().as_bytes());
        let _ = marker.sync_all();
    }
    if env::var(FAULT_ACTION_VARIABLE).ok().as_deref() == Some("hold") {
        loop {
            thread::park();
        }
    }
    std::process::abort();
}

#[cfg(not(feature = "phase2-fault-injection"))]
pub(crate) const fn trip(_point: FaultPoint) {}

/// Environment variable naming the failpoint the child process must take.
pub const FAULT_SELECTION_VARIABLE: &str = "ACADEMIC_TRANSCRIPT_TEST_FAULT";
/// Environment variable naming the file the child creates before aborting.
pub const FAULT_READY_MARKER_VARIABLE: &str = "ACADEMIC_TRANSCRIPT_TEST_READY_MARKER";
/// Environment variable selecting `hold` instead of `abort`.
pub const FAULT_ACTION_VARIABLE: &str = "ACADEMIC_TRANSCRIPT_TEST_FAULT_ACTION";

/// Every failpoint selector this crate implements, in fault-matrix order.
pub const FAULT_SELECTORS: &[&str] = &[
    "IN04:before-staging-rename",
    "IN04:after-staging-rename",
    "IN04:after-publish-rename",
];
