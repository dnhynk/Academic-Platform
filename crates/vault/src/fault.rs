//! Deterministic V01-V06 and OB01-OB09 failpoints, compiled only by the
//! explicit test features.
//!
//! Default product builds contain no environment lookup and no crash switch.
//! The two families are separate features: the Phase 1 plaintext lane and the
//! Phase 2 encrypted lane are exercised by different harnesses, and neither
//! harness can fire the other's failpoint by setting the wrong environment
//! variable.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection")),
    allow(dead_code)
)]
pub(crate) enum FaultPoint {
    V01,
    V02,
    V03,
    V04,
    V05,
    V06,
    // The OB family is constructed only by the encrypted object lane, so the
    // variants exist exactly where that lane does.
    #[cfg(feature = "aead-objects")]
    Ob01,
    #[cfg(feature = "aead-objects")]
    Ob02,
    #[cfg(feature = "aead-objects")]
    Ob03,
    #[cfg(feature = "aead-objects")]
    Ob04,
    #[cfg(feature = "aead-objects")]
    Ob05,
    #[cfg(feature = "aead-objects")]
    Ob08,
    #[cfg(feature = "aead-objects")]
    Ob09,
    /// `P2-K5`'s `RB01`: between reading a live key slot and destroying it.
    #[cfg(feature = "aead-objects")]
    Rb01,
}

impl FaultPoint {
    #[cfg(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection"))]
    const fn as_str(self) -> &'static str {
        match self {
            Self::V01 => "V01",
            Self::V02 => "V02",
            Self::V03 => "V03",
            Self::V04 => "V04",
            Self::V05 => "V05",
            Self::V06 => "V06",
            #[cfg(feature = "aead-objects")]
            Self::Ob01 => "OB01",
            #[cfg(feature = "aead-objects")]
            Self::Ob02 => "OB02",
            #[cfg(feature = "aead-objects")]
            Self::Ob03 => "OB03",
            #[cfg(feature = "aead-objects")]
            Self::Ob04 => "OB04",
            #[cfg(feature = "aead-objects")]
            Self::Ob05 => "OB05",
            #[cfg(feature = "aead-objects")]
            Self::Ob08 => "OB08",
            #[cfg(feature = "aead-objects")]
            Self::Ob09 => "OB09",
            #[cfg(feature = "aead-objects")]
            Self::Rb01 => "RB01",
        }
    }

    /// Reports which feature may activate this failpoint.
    #[cfg(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection"))]
    const fn is_phase2(self) -> bool {
        #[cfg(feature = "aead-objects")]
        {
            matches!(
                self,
                Self::Ob01
                    | Self::Ob02
                    | Self::Ob03
                    | Self::Ob04
                    | Self::Ob05
                    | Self::Ob08
                    | Self::Ob09
                    | Self::Rb01
            )
        }
        #[cfg(not(feature = "aead-objects"))]
        {
            let _ = self;
            false
        }
    }
}

#[cfg(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection"))]
pub(crate) fn trip(point: FaultPoint) {
    use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf, thread};

    const SELECTED_FAULT: &str = "ACADEMIC_VAULT_TEST_FAULT";
    const FAULT_ACTION: &str = "ACADEMIC_VAULT_TEST_FAULT_ACTION";
    const READY_MARKER: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";

    if point.is_phase2() {
        if !cfg!(feature = "phase2-fault-injection") {
            return;
        }
    } else if !cfg!(feature = "phase1-fault-injection") {
        return;
    }
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

#[cfg(not(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection")))]
pub(crate) const fn trip(_point: FaultPoint) {}
