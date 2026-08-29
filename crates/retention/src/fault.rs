//! Deterministic `KY03`-`KY05` and `RB02` failpoints, compiled only by the
//! explicit test feature.
//!
//! Default product builds contain no environment lookup and no crash switch,
//! exactly as `academic-crypto` and `academic-vault` do it.
//!
//! `KY03` is a single row of the fault matrix with four distinguishable
//! on-disk states, so the selector is `KY03:<stage>`. The row identifier stays
//! greppable and no new fault identifier is invented.
//!
//! `RB01` lives in `academic-vault`, beside the key slot it destroys.
//! `RB03` and `RB04` are error-induced rather than kill-induced and need no
//! failpoint: they are driven through the resolver and executor seams.

/// A point at which the harness may terminate this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "phase2-fault-injection"), allow(dead_code))]
pub(crate) enum FaultPoint {
    /// `KY03`: before the target object exists at all.
    Ky03BeforeReseal,
    /// `KY03`: target object durable and verified, nothing journalled.
    Ky03AfterReseal,
    /// `KY03`: `UnitResealed` durable, reachability not yet moved.
    Ky03AfterResealRecord,
    /// `KY03`: `UnitMigrated` durable, reachability moved.
    Ky03AfterMigrateRecord,
    /// `KY04` and `KY05`: after the replacement recipient set is durable and
    /// before it is renamed over the live one.
    RecipientSetRename,
    /// `RB02`: before a backup tombstone is written into a backup.
    Rb02BeforeTombstone,
}

impl FaultPoint {
    #[cfg(feature = "phase2-fault-injection")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ky03BeforeReseal => "KY03:before-reseal",
            Self::Ky03AfterReseal => "KY03:after-reseal",
            Self::Ky03AfterResealRecord => "KY03:after-resealed-record",
            Self::Ky03AfterMigrateRecord => "KY03:after-migrated-record",
            Self::RecipientSetRename => "KY04:recipient-set-rename",
            Self::Rb02BeforeTombstone => "RB02:before-tombstone",
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
pub const FAULT_SELECTION_VARIABLE: &str = "ACADEMIC_RETENTION_TEST_FAULT";
/// Environment variable naming the file the child creates before aborting.
pub const FAULT_READY_MARKER_VARIABLE: &str = "ACADEMIC_RETENTION_TEST_READY_MARKER";
/// Environment variable selecting `hold` instead of `abort`.
pub const FAULT_ACTION_VARIABLE: &str = "ACADEMIC_RETENTION_TEST_FAULT_ACTION";

/// Every failpoint selector this crate implements, in fault-matrix order.
pub const FAULT_SELECTORS: &[&str] = &[
    "KY03:before-reseal",
    "KY03:after-reseal",
    "KY03:after-resealed-record",
    "KY03:after-migrated-record",
    "KY04:recipient-set-rename",
    "RB02:before-tombstone",
];
