//! Resource limits, what a run actually spent, and the binding that makes a
//! receipt non-optional.
//!
//! # Why [`WorkerRun`] exists
//!
//! `P2-M1` owns the twelve section 27.3 `ModelRun` fields and the event arm
//! that records them; this crate does not add a thirteenth or edit a signed
//! envelope. What `P2-G4` owns is narrower and is a type rather than a
//! convention: a [`WorkerRun`] is a [`academic_domain::ModelRunId`] and a
//! [`ResourceReceipt`] in one value, it has one constructor, that constructor
//! takes both, and neither field can be replaced afterwards. So there is no
//! order of calls in which a model run leaves this crate's seam without its
//! measurement — `resource_receipt_is_recorded_per_run` observes that for a
//! completed run, a killed run, and a failed run alike, and the `compile_fail`
//! case below is what stops a caller assembling one without a receipt.
//!
//! ```compile_fail
//! use academic_worker::WorkerRun;
//! use academic_domain::ModelRunId;
//!
//! fn forge(id: ModelRunId) -> WorkerRun {
//!     WorkerRun { model_run_id: id, receipt: todo!() }
//! }
//! ```

use academic_domain::ModelRunId;

/// The four bounds every job runs under.
///
/// Zero is not a permitted bound: a limit of zero is indistinguishable from
/// "unset" at a syscall boundary, and both operating-system backends read these
/// numbers directly into a kernel structure. [`ResourceLimits::new`] therefore
/// raises a zero to one rather than passing it through, so a caller cannot
/// disable a limit by leaving a field at its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    cpu_millis: u64,
    memory_bytes: u64,
    wall_millis: u64,
    output_bytes: u64,
}

impl ResourceLimits {
    /// Builds a limit set, raising any zero to one.
    #[must_use]
    pub const fn new(
        cpu_millis: u64,
        memory_bytes: u64,
        wall_millis: u64,
        output_bytes: u64,
    ) -> Self {
        Self {
            cpu_millis: if cpu_millis == 0 { 1 } else { cpu_millis },
            memory_bytes: if memory_bytes == 0 { 1 } else { memory_bytes },
            wall_millis: if wall_millis == 0 { 1 } else { wall_millis },
            output_bytes: if output_bytes == 0 { 1 } else { output_bytes },
        }
    }

    /// CPU time bound, in milliseconds.
    #[must_use]
    pub const fn cpu_millis(&self) -> u64 {
        self.cpu_millis
    }

    /// Address-space bound, in bytes.
    #[must_use]
    pub const fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    /// Wall-clock bound, in milliseconds.
    #[must_use]
    pub const fn wall_millis(&self) -> u64 {
        self.wall_millis
    }

    /// Staged-output bound, in bytes.
    #[must_use]
    pub const fn output_bytes(&self) -> u64 {
        self.output_bytes
    }

    /// Canonical bytes, for the descriptor digest.
    #[must_use]
    pub fn canonical_bytes(&self) -> [u8; 32] {
        let mut out = [0_u8; 32];
        out[0..8].copy_from_slice(&self.cpu_millis.to_be_bytes());
        out[8..16].copy_from_slice(&self.memory_bytes.to_be_bytes());
        out[16..24].copy_from_slice(&self.wall_millis.to_be_bytes());
        out[24..32].copy_from_slice(&self.output_bytes.to_be_bytes());
        out
    }
}

/// Which bound a run hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LimitKind {
    /// CPU time.
    Cpu,
    /// Resident or address-space memory.
    Memory,
    /// Wall-clock time.
    WallTime,
    /// Bytes written into the staged output directory.
    OutputBytes,
}

impl LimitKind {
    /// Every bound a run can hit.
    pub const ALL: [Self; 4] = [Self::Cpu, Self::Memory, Self::WallTime, Self::OutputBytes];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "MEMORY",
            Self::WallTime => "WALL_TIME",
            Self::OutputBytes => "OUTPUT_BYTES",
        }
    }
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    /// The job ran to completion inside every bound.
    Completed,
    /// The job was killed for exceeding a bound. `PJ01`.
    KilledByLimit(LimitKind),
    /// The job exited non-zero without hitting a bound.
    Failed {
        /// Platform exit status, as reported by the operating system.
        exit_code: i64,
    },
    /// The job could not be started at all.
    NotStarted {
        /// Why the launch failed.
        detail: String,
    },
}

impl RunOutcome {
    /// Whether a staged output may be considered for acceptance.
    ///
    /// Only a completed run may. `PJ01`'s row says a killed job produces no
    /// partial claim, and this is where that is decided: a killed, failed, or
    /// unstarted run's staged bytes are never offered to the acceptance
    /// boundary, whatever is in the directory.
    #[must_use]
    pub const fn is_acceptable(&self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// What one run spent, and how it ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceReceipt {
    backend: crate::sandbox::BackendId,
    limits: ResourceLimits,
    cpu_millis: u64,
    peak_memory_bytes: u64,
    wall_millis: u64,
    output_bytes: u64,
    outcome: RunOutcome,
}

impl ResourceReceipt {
    /// Records one run's measurement.
    #[must_use]
    pub const fn new(
        backend: crate::sandbox::BackendId,
        limits: ResourceLimits,
        cpu_millis: u64,
        peak_memory_bytes: u64,
        wall_millis: u64,
        output_bytes: u64,
        outcome: RunOutcome,
    ) -> Self {
        Self {
            backend,
            limits,
            cpu_millis,
            peak_memory_bytes,
            wall_millis,
            output_bytes,
            outcome,
        }
    }

    /// Which backend contained the run.
    #[must_use]
    pub const fn backend(&self) -> crate::sandbox::BackendId {
        self.backend
    }

    /// The bounds the run was given.
    #[must_use]
    pub const fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// CPU milliseconds the operating system attributed to the run.
    #[must_use]
    pub const fn cpu_millis(&self) -> u64 {
        self.cpu_millis
    }

    /// Peak memory the operating system attributed to the run.
    #[must_use]
    pub const fn peak_memory_bytes(&self) -> u64 {
        self.peak_memory_bytes
    }

    /// Wall milliseconds between launch and reap.
    #[must_use]
    pub const fn wall_millis(&self) -> u64 {
        self.wall_millis
    }

    /// Bytes found in the staged output directory when the run was reaped.
    #[must_use]
    pub const fn output_bytes(&self) -> u64 {
        self.output_bytes
    }

    /// How the run ended.
    #[must_use]
    pub const fn outcome(&self) -> &RunOutcome {
        &self.outcome
    }
}

/// One model run and its measurement, inseparable.
///
/// There is no constructor that takes an identifier alone, no `Default`, and no
/// setter. See the module documentation for why that is the whole mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRun {
    model_run_id: ModelRunId,
    receipt: ResourceReceipt,
}

impl WorkerRun {
    /// Pairs a model run with the receipt of the run that produced it.
    #[must_use]
    pub const fn new(model_run_id: ModelRunId, receipt: ResourceReceipt) -> Self {
        Self {
            model_run_id,
            receipt,
        }
    }

    /// The model run's identity.
    #[must_use]
    pub const fn model_run_id(&self) -> &ModelRunId {
        &self.model_run_id
    }

    /// The measurement of the run.
    #[must_use]
    pub const fn receipt(&self) -> &ResourceReceipt {
        &self.receipt
    }
}
