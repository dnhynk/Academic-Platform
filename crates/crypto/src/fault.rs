//! Deterministic `KY02` and `KY08` failpoints, compiled only by the explicit
//! test feature.
//!
//! Default product builds contain no environment lookup and no crash switch,
//! exactly as `academic-vault`'s Phase 1 failpoints do. `KY01`, `KY06`, and
//! `KY07` are error-induced rather than kill-induced and need no failpoint:
//! they are driven through the [`DeviceKeystore`](crate::DeviceKeystore) seam.

/// A point at which the harness may terminate this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaultPoint {
    /// Between the broker returning the device wrapping key and the VMK
    /// becoming available.
    Ky02,
    /// Between generating first-run key material and returning it for persistence.
    Ky08,
}

impl FaultPoint {
    #[cfg(feature = "phase2-fault-injection")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ky02 => "KY02",
            Self::Ky08 => "KY08",
        }
    }
}

#[cfg(feature = "phase2-fault-injection")]
pub(crate) fn trip(point: FaultPoint) {
    use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf, thread};

    const SELECTED_FAULT: &str = "ACADEMIC_CRYPTO_TEST_FAULT";
    const FAULT_ACTION: &str = "ACADEMIC_CRYPTO_TEST_FAULT_ACTION";
    const READY_MARKER: &str = "ACADEMIC_CRYPTO_TEST_READY_MARKER";

    if env::var(SELECTED_FAULT).ok().as_deref() != Some(point.as_str()) {
        return;
    }
    if let Some(path) = env::var_os(READY_MARKER).map(PathBuf::from)
        && let Ok(mut marker) = OpenOptions::new().create_new(true).write(true).open(path)
    {
        let _ = marker.write_all(point.as_str().as_bytes());
        let _ = marker.sync_all();
    }
    if env::var(FAULT_ACTION).ok().as_deref() == Some("hold") {
        loop {
            thread::park();
        }
    }
    std::process::abort();
}

#[cfg(not(feature = "phase2-fault-injection"))]
pub(crate) const fn trip(_point: FaultPoint) {}

/// Environment variable naming the fault the child process must take.
pub const FAULT_SELECTION_VARIABLE: &str = "ACADEMIC_CRYPTO_TEST_FAULT";
/// Environment variable naming the file the child creates before aborting.
pub const FAULT_READY_MARKER_VARIABLE: &str = "ACADEMIC_CRYPTO_TEST_READY_MARKER";
/// Environment variable selecting `hold` instead of `abort`.
pub const FAULT_ACTION_VARIABLE: &str = "ACADEMIC_CRYPTO_TEST_FAULT_ACTION";
