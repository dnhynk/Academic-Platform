//! Shared synthetic-only test vocabulary.
//!
//! Fault activation is absent from product crates. This crate freezes names for
//! later process-harness work and has no product dependency edge.

/// Non-default test-only feature that may activate later fault harnesses.
pub const FAULT_INJECTION_FEATURE: &str = "phase1-fault-injection";

/// Deterministic Phase 1 process fault identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaultId {
    V01,
    V02,
    V03,
    V04,
    V05,
    V06,
    Db01,
    Db02,
    Db03,
    Db04,
    Db05,
    Db06,
    Db07,
    Pr01,
    Pr02,
    Pr03,
    Bk01,
    Bk02,
    Bk03,
    Bk04,
    Rs01,
    Rs02,
    Rs03,
    Rs04,
    Ipc01,
    Ipc02,
}

impl FaultId {
    /// Returns the stable external spelling used by the test harness and reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V01 => "V01",
            Self::V02 => "V02",
            Self::V03 => "V03",
            Self::V04 => "V04",
            Self::V05 => "V05",
            Self::V06 => "V06",
            Self::Db01 => "DB01",
            Self::Db02 => "DB02",
            Self::Db03 => "DB03",
            Self::Db04 => "DB04",
            Self::Db05 => "DB05",
            Self::Db06 => "DB06",
            Self::Db07 => "DB07",
            Self::Pr01 => "PR01",
            Self::Pr02 => "PR02",
            Self::Pr03 => "PR03",
            Self::Bk01 => "BK01",
            Self::Bk02 => "BK02",
            Self::Bk03 => "BK03",
            Self::Bk04 => "BK04",
            Self::Rs01 => "RS01",
            Self::Rs02 => "RS02",
            Self::Rs03 => "RS03",
            Self::Rs04 => "RS04",
            Self::Ipc01 => "IPC01",
            Self::Ipc02 => "IPC02",
        }
    }
}

/// Complete ordered Phase 1 fault catalog.
pub const PHASE1_FAULT_IDS: &[FaultId] = &[
    FaultId::V01,
    FaultId::V02,
    FaultId::V03,
    FaultId::V04,
    FaultId::V05,
    FaultId::V06,
    FaultId::Db01,
    FaultId::Db02,
    FaultId::Db03,
    FaultId::Db04,
    FaultId::Db05,
    FaultId::Db06,
    FaultId::Db07,
    FaultId::Pr01,
    FaultId::Pr02,
    FaultId::Pr03,
    FaultId::Bk01,
    FaultId::Bk02,
    FaultId::Bk03,
    FaultId::Bk04,
    FaultId::Rs01,
    FaultId::Rs02,
    FaultId::Rs03,
    FaultId::Rs04,
    FaultId::Ipc01,
    FaultId::Ipc02,
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn fault_ids_are_complete_unique_and_stable() {
        let ordered_spellings = PHASE1_FAULT_IDS
            .iter()
            .copied()
            .map(FaultId::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            ordered_spellings,
            [
                "V01", "V02", "V03", "V04", "V05", "V06", "DB01", "DB02", "DB03", "DB04", "DB05",
                "DB06", "DB07", "PR01", "PR02", "PR03", "BK01", "BK02", "BK03", "BK04", "RS01",
                "RS02", "RS03", "RS04", "IPC01", "IPC02",
            ]
        );
        let unique_spellings = ordered_spellings.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique_spellings.len(), 26);
    }
}
