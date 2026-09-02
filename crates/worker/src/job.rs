//! The vocabulary a job is written in, and the report a run comes back with.
//!
//! A job is a script of [`JobOperation`]s. That is deliberate: it makes the
//! malicious-plugin corpus a set of *scripts* rather than a set of binaries, so
//! the corpus is synthetic, deterministic, and composed at run time from this
//! file's own vocabulary, and the thing under test stays the sandbox rather
//! than a parser.
//!
//! Six of the operations exist only to be refused — reading a home file,
//! reading a vault file, opening a socket, spawning a child, writing outside
//! the staged output directory, and each of the three ways to run past a bound.
//! The probe binary attempts them for real. What it reports is what the
//! operating system answered, and
//! [`OperationOutcome::Permitted`] appearing for any of them is the failure the
//! acceptance suite is looking for.

use std::{fmt, path::PathBuf};

use crate::capability::DescriptorError;

/// One step of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOperation {
    /// Read a named file from the staged input directory. Expected to succeed.
    ReadStagedInput {
        /// File name inside the staged input directory.
        name: String,
    },
    /// Write a named file of `bytes` length into the staged output directory.
    /// Expected to succeed while within the output bound.
    WriteStagedOutput {
        /// File name inside the staged output directory.
        name: String,
        /// How many bytes to write.
        bytes: u64,
    },
    /// Read the canary file the harness planted in the user's home directory.
    ReadHome,
    /// Read the canary file the harness planted in the profile's vault
    /// directory.
    ReadVault,
    /// Create a socket and attempt to reach a destination.
    OpenSocket,
    /// Attempt to create a child process.
    SpawnChild,
    /// Attempt to write to an absolute path outside both staged directories.
    WriteOutsideStagedOutput {
        /// The absolute path to attempt.
        path: PathBuf,
    },
    /// Spin on the CPU until killed or until the bound is passed.
    BurnCpu,
    /// Allocate and touch memory until killed.
    ExhaustMemory,
    /// Sleep until killed.
    SleepUntilKilled,
    /// Write far past the staged-output bound.
    OverrunOutput {
        /// File name inside the staged output directory.
        name: String,
    },
}

impl JobOperation {
    /// The wire spelling written into the staged input script.
    #[must_use]
    pub fn to_line(&self) -> String {
        match self {
            Self::ReadStagedInput { name } => format!("read_staged_input {name}"),
            Self::WriteStagedOutput { name, bytes } => {
                format!("write_staged_output {name} {bytes}")
            }
            Self::ReadHome => "read_home".to_owned(),
            Self::ReadVault => "read_vault".to_owned(),
            Self::OpenSocket => "open_socket".to_owned(),
            Self::SpawnChild => "spawn_child".to_owned(),
            Self::WriteOutsideStagedOutput { path } => {
                format!("write_outside {}", path.display())
            }
            Self::BurnCpu => "burn_cpu".to_owned(),
            Self::ExhaustMemory => "exhaust_memory".to_owned(),
            Self::SleepUntilKilled => "sleep_until_killed".to_owned(),
            Self::OverrunOutput { name } => format!("overrun_output {name}"),
        }
    }

    /// Reads a wire spelling back.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::MalformedWire`] for an unknown verb, a
    /// missing argument, or a non-numeric length.
    pub fn parse(line: &str) -> Result<Self, DescriptorError> {
        let mut parts = line.split(' ');
        let verb = parts
            .next()
            .ok_or(DescriptorError::MalformedWire("empty operation"))?;
        let rest: Vec<&str> = parts.collect();
        let one = || -> Result<String, DescriptorError> {
            rest.first()
                .map(|value| (*value).to_owned())
                .ok_or(DescriptorError::MalformedWire(
                    "operation needs an argument",
                ))
        };
        match verb {
            "read_staged_input" => Ok(Self::ReadStagedInput { name: one()? }),
            "write_staged_output" => {
                let bytes = rest
                    .get(1)
                    .and_then(|value| value.parse::<u64>().ok())
                    .ok_or(DescriptorError::MalformedWire("non-numeric length"))?;
                Ok(Self::WriteStagedOutput {
                    name: one()?,
                    bytes,
                })
            }
            "read_home" => Ok(Self::ReadHome),
            "read_vault" => Ok(Self::ReadVault),
            "open_socket" => Ok(Self::OpenSocket),
            "spawn_child" => Ok(Self::SpawnChild),
            "write_outside" => Ok(Self::WriteOutsideStagedOutput {
                path: PathBuf::from(rest.join(" ")),
            }),
            "burn_cpu" => Ok(Self::BurnCpu),
            "exhaust_memory" => Ok(Self::ExhaustMemory),
            "sleep_until_killed" => Ok(Self::SleepUntilKilled),
            "overrun_output" => Ok(Self::OverrunOutput { name: one()? }),
            _ => Err(DescriptorError::MalformedWire("unknown operation")),
        }
    }

    /// Whether the operation is one the sandbox is supposed to refuse.
    ///
    /// The three bound-exceeding operations are not on this list: they are not
    /// refused, they are *killed*, and `cpu_memory_time_output_limits_are_enforced`
    /// reads the receipt rather than the per-operation outcome for those.
    #[must_use]
    pub const fn must_be_refused(&self) -> bool {
        matches!(
            self,
            Self::ReadHome
                | Self::ReadVault
                | Self::OpenSocket
                | Self::SpawnChild
                | Self::WriteOutsideStagedOutput { .. }
        )
    }
}

/// What the operating system answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationOutcome {
    /// The operation succeeded. For an operation [`JobOperation::must_be_refused`]
    /// names, this is a containment failure.
    Permitted {
        /// Anything the probe measured, such as a byte count.
        detail: String,
    },
    /// The operating system refused.
    Refused {
        /// The platform error number the refusal came back as.
        code: i64,
        /// The probe's own spelling of what it attempted and what came back.
        detail: String,
    },
    /// The probe did not reach the operation, because the run was killed first.
    NotReached,
}

impl OperationOutcome {
    /// Whether the operating system refused.
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }
}

impl fmt::Display for OperationOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Permitted { detail } => write!(formatter, "PERMITTED {detail}"),
            Self::Refused { code, detail } => write!(formatter, "REFUSED {code} {detail}"),
            Self::NotReached => formatter.write_str("NOT_REACHED"),
        }
    }
}

/// The script one job runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    operations: Vec<JobOperation>,
}

impl JobRequest {
    /// Builds a request from an operation list.
    #[must_use]
    pub fn new(operations: Vec<JobOperation>) -> Self {
        Self { operations }
    }

    /// The operations, in order.
    #[must_use]
    pub fn operations(&self) -> &[JobOperation] {
        &self.operations
    }

    /// The script as written into the staged input directory.
    #[must_use]
    pub fn to_script(&self) -> String {
        let mut text = String::from("academic-worker-job-v1\n");
        for operation in &self.operations {
            text.push_str(&operation.to_line());
            text.push('\n');
        }
        text
    }

    /// Reads a script back.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::MalformedWire`] when the version line is
    /// wrong, and propagates [`JobOperation::parse`] otherwise.
    pub fn parse(text: &str) -> Result<Self, DescriptorError> {
        let mut lines = text.lines();
        if lines.next() != Some("academic-worker-job-v1") {
            return Err(DescriptorError::MalformedWire("job version line"));
        }
        let mut operations = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            operations.push(JobOperation::parse(line)?);
        }
        Ok(Self { operations })
    }
}

/// Everything the parent needs to launch one job.
#[derive(Debug, Clone)]
pub struct JobPlan {
    /// The descriptor the job runs under.
    pub descriptor: crate::capability::CapabilityDescriptor,
    /// The script the job runs.
    pub request: JobRequest,
    /// Absolute path of the home canary the `read_home` operation targets.
    pub home_canary: PathBuf,
    /// Absolute path of the vault canary the `read_vault` operation targets.
    pub vault_canary: PathBuf,
}

/// What one run reported, operation by operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProbeReport {
    outcomes: Vec<(String, OperationOutcome)>,
}

impl ProbeReport {
    /// Reads a probe's stdout.
    ///
    /// Lines the probe did not write are absent rather than
    /// [`OperationOutcome::NotReached`]; [`ProbeReport::outcome_of`] is what
    /// turns an absent line into `NotReached`, so a truncated report from a
    /// killed run reads as "did not get there" rather than as a missing key.
    #[must_use]
    pub fn parse(stdout: &str) -> Self {
        let mut outcomes = Vec::new();
        for line in stdout.lines() {
            let Some(rest) = line.strip_prefix("op ") else {
                continue;
            };
            let Some((operation, answer)) = rest.split_once(" -> ") else {
                continue;
            };
            let outcome = if let Some(detail) = answer.strip_prefix("PERMITTED ") {
                OperationOutcome::Permitted {
                    detail: detail.to_owned(),
                }
            } else if let Some(detail) = answer.strip_prefix("REFUSED ") {
                let (code, detail) = detail.split_once(' ').unwrap_or((detail, ""));
                OperationOutcome::Refused {
                    code: code.parse::<i64>().unwrap_or(-1),
                    detail: detail.to_owned(),
                }
            } else {
                continue;
            };
            outcomes.push((operation.to_owned(), outcome));
        }
        Self { outcomes }
    }

    /// What the probe reported for one operation.
    #[must_use]
    pub fn outcome_of(&self, operation: &JobOperation) -> OperationOutcome {
        let line = operation.to_line();
        self.outcomes
            .iter()
            .find(|(name, _)| *name == line)
            .map_or(OperationOutcome::NotReached, |(_, outcome)| outcome.clone())
    }

    /// Every reported outcome, in probe order.
    #[must_use]
    pub fn outcomes(&self) -> &[(String, OperationOutcome)] {
        &self.outcomes
    }
}
