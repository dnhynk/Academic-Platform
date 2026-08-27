//! Deterministic acceptance checkpoints for an external test harness.
//!
//! This module contains no environment-variable, CLI, or process-exit switch.
//! Production acceptance always uses [`NoFault`]. Tests may supply a callback
//! that exits their own child process at one named ordering boundary.

use std::{error::Error, fmt};

/// S2/IPC fault checkpoints fixed by the Phase 1 execution contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AcceptanceFaultPoint {
    Db01,
    Db02,
    Db03,
    Db04,
    Db05,
    Db06,
    Db07,
    Ipc02,
}

impl AcceptanceFaultPoint {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Db01 => "DB01",
            Self::Db02 => "DB02",
            Self::Db03 => "DB03",
            Self::Db04 => "DB04",
            Self::Db05 => "DB05",
            Self::Db06 => "DB06",
            Self::Db07 => "DB07",
            Self::Ipc02 => "IPC02",
        }
    }
}

/// A non-crashing injected failure used by focused rollback tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InjectedFault {
    pub point: AcceptanceFaultPoint,
}

impl fmt::Display for InjectedFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "injected acceptance fault at {}",
            self.point.as_str()
        )
    }
}

impl Error for InjectedFault {}

/// Callback boundary implemented only by an explicitly supplied harness.
pub trait AcceptanceFaultInjector: fmt::Debug + Send + Sync {
    fn hit(&self, point: AcceptanceFaultPoint) -> Result<(), InjectedFault>;
}

/// Production injector: every checkpoint is a no-op.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoFault;

impl AcceptanceFaultInjector for NoFault {
    fn hit(&self, _point: AcceptanceFaultPoint) -> Result<(), InjectedFault> {
        Ok(())
    }
}
