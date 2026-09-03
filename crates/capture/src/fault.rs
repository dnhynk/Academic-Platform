//! Deterministic `CP05` failpoints, compiled only by the explicit test feature.
//!
//! Default product builds contain no environment lookup and no crash switch,
//! exactly as `academic-vault`, `academic-retention` and `academic-transcript`
//! do it. `P2-K5` fixed that shape and this reuses it rather than inventing a
//! second one.
//!
//! `CP02` and `CP03` need no failpoint. They are error-induced: a
//! [`crate::preflight::PreflightReading`] below the effective floor is a value
//! the public seam already takes, so both are driven through
//! [`crate::recorder::CaptureRecorder::observe`] with a committed reading.
//! `CP04` is error-induced for the same reason — two anchors are values.

/// A point at which the harness may terminate this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "phase2-fault-injection"), allow(dead_code))]
pub(crate) enum FaultPoint {
    /// `CP05`: the frame is built and nothing of it is on disk.
    BeforeFrameWrite,
    /// `CP05`: the frame header and body are on disk; the trailing digest is
    /// not, so the frame covers nothing.
    AfterBodyBeforeTrailer,
    /// `CP05`: the whole frame is durable and the writer has not returned.
    AfterFrameSynced,
}

impl FaultPoint {
    #[cfg(feature = "phase2-fault-injection")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::BeforeFrameWrite => "CP05:before-frame-write",
            Self::AfterBodyBeforeTrailer => "CP05:after-body-before-trailer",
            Self::AfterFrameSynced => "CP05:after-frame-synced",
        }
    }
}

#[cfg(feature = "phase2-fault-injection")]
pub(crate) fn trip(point: FaultPoint, frame_seq: u32) {
    use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf};

    if env::var(FAULT_SELECTION_VARIABLE).ok().as_deref() != Some(point.as_str()) {
        return;
    }
    // Which frame, so a row can leave synced frames behind the one it
    // interrupts. Without it every row would abort on the first append and
    // "recovers to the last synced chunk" would have nothing to recover to.
    if let Ok(selected) = env::var(FAULT_FRAME_VARIABLE)
        && selected.parse::<u32>().ok() != Some(frame_seq)
    {
        return;
    }
    if let Some(path) = env::var_os(FAULT_READY_MARKER_VARIABLE).map(PathBuf::from)
        && let Ok(mut marker) = OpenOptions::new().create_new(true).write(true).open(path)
    {
        let _ = marker.write_all(point.as_str().as_bytes());
        let _ = marker.sync_all();
    }
    std::process::abort();
}

#[cfg(not(feature = "phase2-fault-injection"))]
pub(crate) const fn trip(_point: FaultPoint, _frame_seq: u32) {}

/// Environment variable naming the failpoint the child process must take.
pub const FAULT_SELECTION_VARIABLE: &str = "ACADEMIC_CAPTURE_TEST_FAULT";
/// Environment variable naming which frame the failpoint applies to.
///
/// Absent means every frame, which is the shape that aborts on the first one.
pub const FAULT_FRAME_VARIABLE: &str = "ACADEMIC_CAPTURE_TEST_FAULT_FRAME";
/// Environment variable naming the file the child creates before aborting.
pub const FAULT_READY_MARKER_VARIABLE: &str = "ACADEMIC_CAPTURE_TEST_READY_MARKER";

/// Every failpoint selector this crate implements, in fault-matrix order.
pub const FAULT_SELECTORS: &[&str] = &[
    "CP05:before-frame-write",
    "CP05:after-body-before-trailer",
    "CP05:after-frame-synced",
];
