//! The operating-system containment, and what it is measured to refuse.
//!
//! # Two halves, in two processes
//!
//! [`enter`] runs *inside* the sandboxed process, at the top of the probe's
//! `main`, before it reads a single job byte. On Linux it is the whole
//! backend: `setrlimit`, then Landlock, then a seccomp filter, all of which a
//! process may apply to itself unprivileged.
//!
//! [`launch`] runs in the parent. On Windows it is the whole backend, because
//! an AppContainer token and a job object are things a *creator* applies:
//! `CreateProcessW` with a `SECURITY_CAPABILITIES` attribute and no capability
//! SIDs, inside a job object with `ActiveProcessLimit = 1`, a process memory
//! limit, and a per-process user-time limit. On Linux the parent still owns the
//! wall clock and the reaping, because a process cannot be trusted to time
//! itself.
//!
//! # What each backend was measured to refuse
//!
//! | | Linux (seccomp + Landlock + rlimits) | Windows (AppContainer + job object) |
//! |---|---|---|
//! | home or vault read | `EACCES` from Landlock | `ERROR_ACCESS_DENIED` (5) — no ACE for the container SID |
//! | socket | `EPERM` at `socket(2)`, from the filter | the handle is created; every `connect` is refused `WSAEACCES` (10013) |
//! | child process | `EPERM` at `clone`/`fork`/`execve` | `ERROR_NOT_ENOUGH_QUOTA` (1816) from the job's process limit |
//! | CPU | `RLIMIT_CPU` | `PerProcessUserTimeLimit` |
//! | memory | `RLIMIT_AS` | `ProcessMemoryLimit` |
//! | wall time | parent deadline and kill | parent deadline and `TerminateJobObject` |
//! | output bytes | `RLIMIT_FSIZE`, and the parent's measurement | the parent's measurement |
//!
//! The Windows socket row is the one asymmetry and it is not a gap in the
//! implementation: no user-mode Windows mechanism refuses the creation of a
//! socket handle, so what is measured is that the sandboxed process cannot
//! reach any endpoint. The contract says the same thing and no more.
//!
//! # Availability is measured, not assumed
//!
//! [`availability`] asks the running kernel rather than the target triple.
//! Landlock is a version query; seccomp is a `prctl` probe. A backend that
//! reports [`Availability::Unavailable`] makes every execution test record
//! `NOT_RUN` with that reason instead of passing.

use std::{fmt, path::PathBuf};

#[cfg(all(feature = "native-sandbox", target_os = "linux"))]
mod linux;
#[cfg(all(feature = "native-sandbox", windows))]
mod windows;

/// Which containment mechanism ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackendId {
    /// No operating-system containment was applied.
    None,
    /// `setrlimit`, Landlock, and a seccomp filter, applied by the child.
    LinuxSeccompLandlock,
    /// An AppContainer token and a job object, applied by the parent.
    WindowsAppContainerJob,
}

impl BackendId {
    /// The backend this build would use on this platform.
    ///
    /// It is what the build *selects*, not what the kernel *permits*; see
    /// [`availability`] for the second question.
    #[must_use]
    pub const fn compiled() -> Self {
        #[cfg(all(feature = "native-sandbox", target_os = "linux"))]
        {
            Self::LinuxSeccompLandlock
        }
        #[cfg(all(feature = "native-sandbox", windows))]
        {
            Self::WindowsAppContainerJob
        }
        #[cfg(not(all(feature = "native-sandbox", any(target_os = "linux", windows))))]
        {
            Self::None
        }
    }

    /// Stable spelling, for a receipt.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::LinuxSeccompLandlock => "LINUX_SECCOMP_LANDLOCK",
            Self::WindowsAppContainerJob => "WINDOWS_APPCONTAINER_JOB",
        }
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why a backend cannot run here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxUnavailable {
    /// The backend that was asked for.
    pub backend: BackendId,
    /// What the kernel or the build reported, in words a `NOT_RUN` row can
    /// carry verbatim.
    pub reason: String,
}

/// Whether this kernel supports the compiled backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// The backend can run.
    Available(BackendId),
    /// It cannot, for this reason.
    Unavailable(SandboxUnavailable),
}

impl Availability {
    /// The backend if it is available.
    #[must_use]
    pub const fn backend(&self) -> Option<BackendId> {
        match self {
            Self::Available(backend) => Some(*backend),
            Self::Unavailable(_) => None,
        }
    }

    /// The reason, if it is not.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable(unavailable) => Some(&unavailable.reason),
        }
    }
}

/// What went wrong applying or launching a sandbox.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SandboxError {
    /// This build has no backend for this platform, or the kernel lacks one.
    #[error("no sandbox backend: {}", .0.reason)]
    Unavailable(SandboxUnavailable),
    /// A syscall the backend needs failed.
    #[error("{step} failed: {code}")]
    Syscall {
        /// Which step failed, named after the call it makes.
        step: &'static str,
        /// The platform error number.
        code: i64,
    },
    /// A path the sandbox has to grant does not exist or cannot be opened.
    #[error("staged path {path} could not be opened: {detail}")]
    StagedPath {
        /// The path.
        path: PathBuf,
        /// What the filesystem reported.
        detail: String,
    },
    /// The probe binary could not be started.
    #[error("probe {path} could not be launched: {detail}")]
    Launch {
        /// The probe binary.
        path: PathBuf,
        /// What the platform reported.
        detail: String,
    },
}

/// Everything the parent hands the launcher.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// The probe or job binary to run inside the sandbox.
    pub program: PathBuf,
    /// The job plan, including the descriptor and the script.
    pub plan: crate::job::JobPlan,
    /// The directory the child writes its report into. It is granted write
    /// alongside the staged output directory and is *not* counted against the
    /// output bound, because it is the control channel rather than the result.
    pub report_dir: PathBuf,
}

/// Whether this kernel supports the compiled backend.
#[must_use]
pub fn availability() -> Availability {
    #[cfg(all(feature = "native-sandbox", target_os = "linux"))]
    {
        linux::availability()
    }
    #[cfg(all(feature = "native-sandbox", windows))]
    {
        windows::availability()
    }
    #[cfg(not(all(feature = "native-sandbox", any(target_os = "linux", windows))))]
    {
        Availability::Unavailable(SandboxUnavailable {
            backend: BackendId::None,
            reason: String::from(
                "this build has no operating-system backend: either the \
                 native-sandbox feature is off or this platform has no backend",
            ),
        })
    }
}

/// Applies the child-side half of the sandbox to the current process.
///
/// On Linux this is the whole backend. On Windows it is a no-op that reports
/// the backend, because the parent already contained the process before it
/// started: there is nothing left for the child to drop.
///
/// # Errors
///
/// Returns [`SandboxError::Unavailable`] when there is no backend and
/// [`SandboxError::Syscall`] when a step the backend needs fails. A partial
/// application is never reported as success: each step is checked before the
/// next is attempted, and the first failure returns.
#[allow(unused_variables)]
pub fn enter(
    descriptor: &crate::capability::CapabilityDescriptor,
    report_dir: &std::path::Path,
) -> Result<BackendId, SandboxError> {
    #[cfg(all(feature = "native-sandbox", target_os = "linux"))]
    {
        linux::enter(descriptor, report_dir)
    }
    #[cfg(all(feature = "native-sandbox", windows))]
    {
        windows::enter()
    }
    #[cfg(not(all(feature = "native-sandbox", any(target_os = "linux", windows))))]
    {
        match availability() {
            Availability::Available(backend) => Ok(backend),
            Availability::Unavailable(unavailable) => Err(SandboxError::Unavailable(unavailable)),
        }
    }
}

/// Launches one job inside the sandbox and reaps it.
///
/// Returns the measurement and the child's report. A run that was killed still
/// returns both: `PJ01` says a killed job records a resource receipt, and the
/// receipt is the return value rather than something a caller may skip.
///
/// # Errors
///
/// Returns [`SandboxError::Unavailable`] when there is no backend, and
/// [`SandboxError::Launch`] when the child could not be started. Every other
/// outcome — including a child killed for a bound — is a successful launch
/// with a receipt that says so.
#[allow(unused_variables)]
pub fn launch(
    spec: &LaunchSpec,
) -> Result<(crate::receipt::ResourceReceipt, crate::job::ProbeReport), SandboxError> {
    #[cfg(all(feature = "native-sandbox", target_os = "linux"))]
    {
        linux::launch(spec)
    }
    #[cfg(all(feature = "native-sandbox", windows))]
    {
        windows::launch(spec)
    }
    #[cfg(not(all(feature = "native-sandbox", any(target_os = "linux", windows))))]
    {
        match availability() {
            Availability::Available(_) => unreachable!("no backend reports available"),
            Availability::Unavailable(unavailable) => Err(SandboxError::Unavailable(unavailable)),
        }
    }
}

/// Reads the report file a sandboxed child wrote.
///
/// A missing or unreadable file is an empty report rather than an error: a
/// child killed for a bound before it wrote anything is the normal case, and
/// [`crate::ProbeReport::outcome_of`] turns every absent operation into
/// [`crate::OperationOutcome::NotReached`].
#[must_use]
pub fn read_report(report_dir: &std::path::Path) -> crate::job::ProbeReport {
    let path = report_dir.join(REPORT_FILE);
    std::fs::read_to_string(path)
        .map(|text| crate::job::ProbeReport::parse(&text))
        .unwrap_or_default()
}

/// The fixed name of the child's report file inside the report directory.
pub const REPORT_FILE: &str = "probe-report.txt";

/// The fixed name of the descriptor file inside the staged input directory.
pub const DESCRIPTOR_FILE: &str = "descriptor.txt";

/// The fixed name of the job script inside the staged input directory.
pub const JOB_FILE: &str = "job.txt";

/// The prefix every variable this crate sets shares.
///
/// The Windows launcher hands the child the parent's environment, so a variable
/// with this prefix that the parent happens to hold is dropped before the four
/// below are appended: the child must read the values this launch chose, not a
/// leftover from an outer one.
pub const VAR_PREFIX: &str = "ACADEMIC_WORKER_";

/// Environment variable naming the staged input directory.
///
/// The child needs one path before it can read anything, because the descriptor
/// that names every other path is itself inside the staged input directory.
/// This is that one path. It is not a capability: a child that is handed a
/// different directory finds no descriptor there and stops.
pub const INPUT_DIR_VAR: &str = "ACADEMIC_WORKER_INPUT_DIR";

/// Environment variable naming the report directory.
pub const REPORT_DIR_VAR: &str = "ACADEMIC_WORKER_REPORT_DIR";

/// Environment variable naming the home canary the corpus targets.
pub const HOME_CANARY_VAR: &str = "ACADEMIC_WORKER_HOME_CANARY";

/// Environment variable naming the vault canary the corpus targets.
pub const VAULT_CANARY_VAR: &str = "ACADEMIC_WORKER_VAULT_CANARY";

#[cfg(all(feature = "native-sandbox", any(target_os = "linux", windows)))]
/// Writes the descriptor and the job script into the staged input directory.
fn write_job_inputs(spec: &LaunchSpec) -> Result<(), SandboxError> {
    let input = spec.plan.descriptor.staged_input();
    let staged_path = |path: PathBuf, error: std::io::Error| SandboxError::StagedPath {
        path,
        detail: error.to_string(),
    };
    std::fs::create_dir_all(input).map_err(|error| staged_path(input.to_path_buf(), error))?;
    std::fs::create_dir_all(spec.plan.descriptor.staged_output())
        .map_err(|error| staged_path(spec.plan.descriptor.staged_output().to_path_buf(), error))?;
    std::fs::create_dir_all(&spec.report_dir)
        .map_err(|error| staged_path(spec.report_dir.clone(), error))?;
    let descriptor_path = input.join(DESCRIPTOR_FILE);
    std::fs::write(
        &descriptor_path,
        spec.plan.descriptor.to_wire().as_str().as_bytes(),
    )
    .map_err(|error| staged_path(descriptor_path, error))?;
    let job_path = input.join(JOB_FILE);
    std::fs::write(&job_path, spec.plan.request.to_script().as_bytes())
        .map_err(|error| staged_path(job_path, error))?;
    Ok(())
}

#[cfg(all(feature = "native-sandbox", any(target_os = "linux", windows)))]
/// Measures the staged output directory the way the parent, not the child,
/// sees it.
fn staged_output_bytes(
    descriptor: &crate::capability::CapabilityDescriptor,
) -> Result<u64, SandboxError> {
    let dirs =
        crate::staging::StagedJobDirs::new(descriptor.staged_input(), descriptor.staged_output());
    dirs.output_bytes()
        .map_err(|error| SandboxError::StagedPath {
            path: descriptor.staged_output().to_path_buf(),
            detail: error.to_string(),
        })
}

#[cfg(all(feature = "native-sandbox", any(target_os = "linux", windows)))]
/// Turns an over-bound staged output into the outcome that says so.
///
/// A run that finished inside every kernel bound can still have written more
/// than the descriptor allowed — on Windows nothing in a job object bounds a
/// file's size, and on Linux `RLIMIT_FSIZE` bounds one file rather than a
/// directory. The parent measures the directory and this is where that
/// measurement wins. An outcome that already names a limit is left alone: the
/// first bound a run hit is the one worth reporting.
fn apply_output_bound(
    outcome: crate::receipt::RunOutcome,
    output_bytes: u64,
    limits: &crate::receipt::ResourceLimits,
) -> crate::receipt::RunOutcome {
    if matches!(outcome, crate::receipt::RunOutcome::KilledByLimit(_)) {
        return outcome;
    }
    if output_bytes > limits.output_bytes() {
        return crate::receipt::RunOutcome::KilledByLimit(crate::receipt::LimitKind::OutputBytes);
    }
    outcome
}
