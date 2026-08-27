//! Deterministic V01-V06 failpoints compiled only by the explicit test feature.
//!
//! Default product builds contain no environment lookup and no crash switch.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaultPoint {
    V01,
    V02,
    V03,
    V04,
    V05,
    V06,
}

impl FaultPoint {
    #[cfg(feature = "phase1-fault-injection")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::V01 => "V01",
            Self::V02 => "V02",
            Self::V03 => "V03",
            Self::V04 => "V04",
            Self::V05 => "V05",
            Self::V06 => "V06",
        }
    }
}

#[cfg(feature = "phase1-fault-injection")]
pub(crate) fn trip(point: FaultPoint) {
    use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf, thread};

    const SELECTED_FAULT: &str = "ACADEMIC_VAULT_TEST_FAULT";
    const FAULT_ACTION: &str = "ACADEMIC_VAULT_TEST_FAULT_ACTION";
    const READY_MARKER: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";

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

#[cfg(not(feature = "phase1-fault-injection"))]
pub(crate) const fn trip(_point: FaultPoint) {}
