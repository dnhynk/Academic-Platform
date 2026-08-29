//! Bounded child-process control for the X1 Phase 1 exit harness.
//!
//! Every fault in the enumerated matrix is taken by a child process the harness
//! owns. This module is the only place that starts one, and it guarantees three
//! things the exit receipt depends on.
//!
//! 1. **Every wait is bounded.** A child that never reaches its checkpoint is
//!    terminated and reported as `TimedOut` rather than parking the suite.
//! 2. **Identity is recorded, content is not.** A [`ChildRecord`] carries the
//!    operating-system process identifier, the disposable profile root, how the
//!    child ended, and how long it took. Child streams are attached to the null
//!    device, so no ingested bytes, no environment value, and no diagnostic text
//!    can reach a receipt through this path.
//! 3. **The parent never inherits a stream.** `stdin`, `stdout`, and `stderr`
//!    are null, so a killed child cannot leave a half-written pipe behind.
//!
//! This file is included with `#[path]` by the crate that owns the harness
//! test, exactly like `synthetic_artifacts.rs`, so `academic-test-support`
//! itself keeps no dependency edge.

#![allow(dead_code)]

use std::{
    fmt, io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Longest one harness child may run before the harness terminates it.
///
/// Each child creates a profile, seals one synthetic artifact, and runs at most
/// one acceptance, backup, restore, or projection build before it reaches its
/// checkpoint. On the slowest lane measured that is seconds, so two minutes is
/// far above any real duration and still turns a hang into a failure.
pub const CHILD_TIMEOUT: Duration = Duration::from_secs(120);

/// Longest the harness waits for a child to die after it asks it to stop.
pub const CHILD_KILL_GRACE: Duration = Duration::from_secs(10);

/// Interval between liveness polls while a bounded wait is outstanding.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How one harness child ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildEnd {
    /// The child returned this process exit code.
    Exited(i32),
    /// The child was terminated by a signal, which is how `abort` ends on Unix.
    Signalled,
    /// The bounded wait expired and the harness terminated the child.
    TimedOut,
}

impl ChildEnd {
    /// Returns whether the child ended without completing successfully.
    ///
    /// A fault child must never succeed: reaching the end of its work means the
    /// selected checkpoint was not compiled in or not reached.
    #[must_use]
    pub const fn is_failure(self) -> bool {
        !matches!(self, Self::Exited(0))
    }

    /// Returns whether the bounded wait expired.
    #[must_use]
    pub const fn timed_out(self) -> bool {
        matches!(self, Self::TimedOut)
    }
}

impl fmt::Display for ChildEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exited(code) => write!(formatter, "exited({code})"),
            Self::Signalled => write!(formatter, "signalled"),
            Self::TimedOut => write!(formatter, "timed_out"),
        }
    }
}

/// Non-sensitive record of one harness child.
///
/// This is what the exit receipt is allowed to carry. It deliberately holds no
/// captured output, no command line, and no environment.
#[derive(Debug, Clone)]
pub struct ChildRecord {
    /// Stable label, which for a fault child is the fault identifier.
    pub label: String,
    /// Operating-system process identifier of the child.
    pub pid: u32,
    /// Disposable synthetic profile root the child was given.
    pub profile_root: PathBuf,
    /// How the child ended.
    pub end: ChildEnd,
    /// Bounded wall-clock duration in milliseconds.
    pub elapsed_ms: u128,
}

impl ChildRecord {
    /// Renders the one receipt line this record is allowed to produce.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        format!(
            "{} pid={} profile={} end={} elapsed_ms={}",
            self.label,
            self.pid,
            self.profile_root.display(),
            self.end,
            self.elapsed_ms
        )
    }
}

/// Runs one child to completion under a bounded deadline.
///
/// The command is configured here rather than by the caller so no call site can
/// forget to detach the streams. On expiry the child is killed and reaped, and
/// the record says `TimedOut` instead of pretending the run finished.
pub fn run_bounded(
    command: &mut Command,
    label: &str,
    profile_root: &Path,
    timeout: Duration,
) -> io::Result<ChildRecord> {
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let pid = child.id();
    let end = wait_bounded(&mut child, timeout)?;
    Ok(ChildRecord {
        label: label.to_owned(),
        pid,
        profile_root: profile_root.to_path_buf(),
        end,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Waits for one already-spawned child under a bounded deadline.
fn wait_bounded(child: &mut Child, timeout: Duration) -> io::Result<ChildEnd> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(match status.code() {
                Some(code) => ChildEnd::Exited(code),
                None => ChildEnd::Signalled,
            });
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }
    terminate(child)?;
    Ok(ChildEnd::TimedOut)
}

/// Terminates and reaps a child that outlived its bounded wait.
///
/// The reap is itself bounded: a child that cannot be collected is reported as
/// an error rather than leaving the harness blocked in `wait`.
fn terminate(child: &mut Child) -> io::Result<()> {
    child.kill()?;
    let deadline = Instant::now() + CHILD_KILL_GRACE;
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "harness child did not terminate within the kill grace period",
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Builds a re-invocation of the current test binary at one exact test name.
///
/// This is the established pattern in `crates/vault/tests/crash.rs` and
/// `crates/portability/tests/crash.rs`: the harness child is the same test
/// binary entered at a named entry point, so the child links exactly the crates
/// and features the harness was built with and no product binary needs a crash
/// switch.
pub fn self_invocation(test_name: &str) -> io::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .arg("--test-threads")
        .arg("1");
    Ok(command)
}
