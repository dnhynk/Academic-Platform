//! Deterministic BK01-BK04 and RS01-RS04 failpoints for an external harness.
//!
//! Default product builds compile [`trip`] to a no-op with no environment
//! lookup, no CLI switch, and no process-exit path. Only the explicit
//! `phase1-fault-injection` feature compiles the harness body, and even then a
//! fault fires solely when a test has set the selection variable in the child
//! process it owns.

/// Named backup and restore termination points fixed by the Phase 1 fault matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PortabilityFaultPoint {
    /// Midway through the SQLite Online Backup copy.
    Bk01,
    /// Database snapshot complete, before the first object copy.
    Bk02,
    /// Midway through the reachable-object copy.
    Bk03,
    /// Manifest temp synced, before the publish rename.
    Bk04,
    /// Empty destination staging root and marker created.
    Rs01,
    /// Database copied, before integrity and ledger checks.
    Rs02,
    /// Objects copied, before closure checks and projection rebuild.
    Rs03,
    /// All checks passed, before the final directory publish.
    Rs04,
}

impl PortabilityFaultPoint {
    /// Returns the stable external spelling used by reports and the harness.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bk01 => "BK01",
            Self::Bk02 => "BK02",
            Self::Bk03 => "BK03",
            Self::Bk04 => "BK04",
            Self::Rs01 => "RS01",
            Self::Rs02 => "RS02",
            Self::Rs03 => "RS03",
            Self::Rs04 => "RS04",
        }
    }

    /// Parses the closed BK/RS vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "BK01" => Some(Self::Bk01),
            "BK02" => Some(Self::Bk02),
            "BK03" => Some(Self::Bk03),
            "BK04" => Some(Self::Bk04),
            "RS01" => Some(Self::Rs01),
            "RS02" => Some(Self::Rs02),
            "RS03" => Some(Self::Rs03),
            "RS04" => Some(Self::Rs04),
            _ => None,
        }
    }
}

/// Complete ordered BK/RS catalog owned by this crate.
pub const PORTABILITY_FAULT_POINTS: &[PortabilityFaultPoint] = &[
    PortabilityFaultPoint::Bk01,
    PortabilityFaultPoint::Bk02,
    PortabilityFaultPoint::Bk03,
    PortabilityFaultPoint::Bk04,
    PortabilityFaultPoint::Rs01,
    PortabilityFaultPoint::Rs02,
    PortabilityFaultPoint::Rs03,
    PortabilityFaultPoint::Rs04,
];

/// Environment variable naming the single fault a harness child must take.
pub const FAULT_SELECTION_VARIABLE: &str = "ACADEMIC_PORTABILITY_TEST_FAULT";
/// Environment variable naming the file a harness child creates before aborting.
pub const FAULT_READY_MARKER_VARIABLE: &str = "ACADEMIC_PORTABILITY_TEST_READY_MARKER";

/// Aborts the current process at one named checkpoint when a harness selected it.
#[cfg(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection"))]
pub(crate) fn trip(point: PortabilityFaultPoint) {
    use std::{env, fs::OpenOptions, io::Write as _, path::PathBuf};

    if env::var(FAULT_SELECTION_VARIABLE).ok().as_deref() != Some(point.as_str()) {
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

/// Production build: every checkpoint compiles away.
#[cfg(not(any(feature = "phase1-fault-injection", feature = "phase2-fault-injection")))]
pub(crate) const fn trip(_point: PortabilityFaultPoint) {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn fault_catalog_is_complete_unique_and_stable() {
        let spellings = PORTABILITY_FAULT_POINTS
            .iter()
            .copied()
            .map(PortabilityFaultPoint::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            spellings,
            [
                "BK01", "BK02", "BK03", "BK04", "RS01", "RS02", "RS03", "RS04"
            ]
        );
        assert_eq!(spellings.iter().collect::<BTreeSet<_>>().len(), 8);
        for point in PORTABILITY_FAULT_POINTS {
            assert_eq!(PortabilityFaultPoint::parse(point.as_str()), Some(*point));
        }
        assert_eq!(PortabilityFaultPoint::parse("DB01"), None);
    }
}
