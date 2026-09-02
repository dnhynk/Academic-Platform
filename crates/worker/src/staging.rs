//! The staged input and output directories, and the acceptance boundary.
//!
//! # Why only the core can accept
//!
//! A sandboxed worker writes bytes into a directory. Turning those bytes into
//! something the rest of the system will read is a separate act, and it happens
//! in a different process.
//!
//! [`AcceptedOutput`] has private fields and exactly one producer:
//! [`StagingAuthority::accept`]. A [`StagingAuthority`] holds a random
//! per-profile secret that is never written into a
//! [`crate::WireDescriptor`] and never written into either staged directory, so
//! the sandboxed process does not hold one and cannot construct one — it has
//! nothing to construct it from. The `compile_fail` case below is the other
//! half: an `AcceptedOutput` cannot be assembled field-by-field from outside
//! this crate either.
//!
//! ```compile_fail
//! use academic_worker::AcceptedOutput;
//!
//! fn forge() -> AcceptedOutput {
//!     AcceptedOutput { digest: String::new(), bytes: Vec::new() }
//! }
//! ```
//!
//! `worker_cannot_publish_a_canonical_claim` is the executable half: a job that
//! writes a well-formed claim into its staged output directory produces a
//! [`StagedOutput`] and nothing else, and the same bytes offered without the
//! authority are refused.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{
    capability::{CapabilityDescriptor, DescriptorError, JobCapability, hex_lower},
    receipt::{ResourceReceipt, RunOutcome},
};

/// The two directories one job runs against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedJobDirs {
    input: PathBuf,
    output: PathBuf,
}

impl StagedJobDirs {
    /// Names the pair without touching the filesystem.
    #[must_use]
    pub fn new(input: impl Into<PathBuf>, output: impl Into<PathBuf>) -> Self {
        Self {
            input: input.into(),
            output: output.into(),
        }
    }

    /// Creates both directories under a job root and names them.
    ///
    /// # Errors
    ///
    /// Propagates the filesystem error that stopped a directory being created.
    pub fn create_under(root: &Path) -> io::Result<Self> {
        let input = root.join("in");
        let output = root.join("out");
        fs::create_dir_all(&input)?;
        fs::create_dir_all(&output)?;
        Ok(Self { input, output })
    }

    /// The directory the job may read.
    #[must_use]
    pub fn input(&self) -> &Path {
        &self.input
    }

    /// The directory the job may write.
    #[must_use]
    pub fn output(&self) -> &Path {
        &self.output
    }

    /// Total bytes of every regular file under the output directory.
    ///
    /// The walk descends, because a job that wrote into a subdirectory it
    /// created has still written those bytes. A directory entry that cannot be
    /// read is an error rather than a zero: a size the walk could not measure
    /// must not be reported as a size within the bound.
    ///
    /// # Errors
    ///
    /// Propagates the filesystem error that stopped the walk.
    pub fn output_bytes(&self) -> io::Result<u64> {
        directory_bytes(&self.output)
    }
}

fn directory_bytes(root: &Path) -> io::Result<u64> {
    let mut total = 0_u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                total = total.saturating_add(entry.metadata()?.len());
            }
        }
    }
    Ok(total)
}

/// Bytes a run left in its staged output directory.
///
/// A `StagedOutput` is not a result. It is a candidate: it carries the bytes
/// and their digest and nothing that says they may be read.
///
/// `Debug` is written by hand and prints no staged byte. The buffer is a job's
/// output, which is exactly what an audit row, a log line, or a panic message
/// must not carry — `tools/secret-debug-policy.test.mjs` is the net that says
/// so for a field named `payload`, and this field is named `bytes`, which that
/// net's vocabulary does not reach.
#[derive(Clone, PartialEq, Eq)]
pub struct StagedOutput {
    job: String,
    relative_path: PathBuf,
    bytes: Vec<u8>,
    digest: String,
}

impl StagedOutput {
    /// Reads one file from a run's staged output directory.
    ///
    /// # Errors
    ///
    /// Returns [`AcceptError::EscapesStagedOutput`] for a relative path that
    /// climbs out of the directory or is absolute, and propagates the
    /// filesystem error otherwise.
    pub fn read(
        descriptor: &CapabilityDescriptor,
        relative_path: &Path,
    ) -> Result<Self, AcceptError> {
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(AcceptError::EscapesStagedOutput(
                relative_path.to_path_buf(),
            ));
        }
        let full = descriptor.staged_output().join(relative_path);
        let bytes = fs::read(&full).map_err(|error| AcceptError::Unreadable {
            path: full,
            detail: error.to_string(),
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        Ok(Self {
            job: descriptor.job().as_str().to_owned(),
            relative_path: relative_path.to_path_buf(),
            digest: hex_lower(&hasher.finalize()),
            bytes,
        })
    }

    /// The job that produced the bytes.
    #[must_use]
    pub fn job(&self) -> &str {
        &self.job
    }

    /// The path inside the staged output directory.
    #[must_use]
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Lowercase SHA-256 of the staged bytes.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// How many bytes were staged.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the staged file is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Bytes the core has accepted.
///
/// The only producer is [`StagingAuthority::accept`]. `Debug` is written by
/// hand for the same reason [`StagedOutput`]'s is: accepting the bytes does not
/// make them printable.
#[derive(Clone, PartialEq, Eq)]
pub struct AcceptedOutput {
    digest: String,
    bytes: Vec<u8>,
}

impl AcceptedOutput {
    /// The accepted bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Lowercase SHA-256 of the accepted bytes.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Why staged bytes were not accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcceptError {
    /// The staged path climbs out of the staged output directory.
    #[error("{0} is not inside the staged output directory")]
    EscapesStagedOutput(PathBuf),
    /// A staged file could not be read.
    #[error("staged output {path} could not be read: {detail}")]
    Unreadable {
        /// Path the read was attempted on.
        path: PathBuf,
        /// What the filesystem reported.
        detail: String,
    },
    /// The run did not complete, so there is nothing to accept. `PJ01`.
    #[error("run ended as {outcome}, so no staged output is acceptable")]
    RunNotCompleted {
        /// How the run actually ended.
        outcome: String,
    },
    /// The run wrote more than its output bound allowed.
    #[error("staged output is {staged} bytes against a bound of {bound}")]
    OverOutputBound {
        /// Bytes measured in the staged output directory.
        staged: u64,
        /// Bound the descriptor carried.
        bound: u64,
    },
    /// The staged bytes belong to a different job than the descriptor names.
    #[error("staged output belongs to job {staged}, descriptor names {expected}")]
    JobMismatch {
        /// Job recorded on the staged output.
        staged: String,
        /// Job the descriptor names.
        expected: String,
    },
    /// The descriptor does not hold the write capability, so it could not have
    /// produced a staged output at all.
    #[error(transparent)]
    Descriptor(#[from] DescriptorError),
}

/// The core-side capability to accept staged bytes.
///
/// `Debug` is written by hand and prints the identity digest rather than the
/// secret. A derived one would put the acceptance capability into every log
/// line that formatted an error holding it.
///
/// The secret is what makes this a capability rather than a name: it is
/// generated by the parent, never serialized into a descriptor, and never
/// written into either staged directory. `the_wire_descriptor_carries_no_authority_secret`
/// checks the first, and `worker_cannot_publish_a_canonical_claim` the rest.
pub struct StagingAuthority {
    secret: [u8; 32],
}

impl fmt::Debug for StagingAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No field, not even the identity digest: the guard on hand-written
        // `Debug` impls refuses a call it cannot read through, and a secret's
        // redacted form is not worth a hole in that rule.
        formatter
            .debug_struct("StagingAuthority")
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for StagedOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedOutput")
            .field("job", &self.job)
            .field("relative_path", &self.relative_path)
            .field("len", &self.bytes.len())
            .field("digest_len", &self.digest.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for AcceptedOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptedOutput")
            .field("len", &self.bytes.len())
            .field("digest_len", &self.digest.len())
            .finish_non_exhaustive()
    }
}

impl StagingAuthority {
    /// Builds an authority from caller-supplied secret bytes.
    ///
    /// The caller is the core process. Tests supply a fixed value so the suite
    /// stays deterministic; nothing in this crate reads the bytes for anything
    /// but identity, so a fixed value weakens no runtime claim.
    #[must_use]
    pub const fn from_secret(secret: [u8; 32]) -> Self {
        Self { secret }
    }

    /// Lowercase SHA-256 of the authority's secret, for audit identity.
    #[must_use]
    pub fn identity(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"academic-worker-staging-authority-v1\0");
        hasher.update(self.secret);
        hex_lower(&hasher.finalize())
    }

    /// Accepts staged bytes, or refuses them.
    ///
    /// Four refusals, in this order: the descriptor must hold
    /// [`JobCapability::WriteStagedOutput`] — bytes under a descriptor that
    /// could not write are bytes from somewhere else; the run must have
    /// completed; the staged directory must be within the output bound; and the
    /// staged bytes must carry the descriptor's own job.
    ///
    /// The bound is re-measured here from the receipt rather than trusted from
    /// the child, because the child is the untrusted party.
    ///
    /// # Errors
    ///
    /// One [`AcceptError`] per refusal above.
    pub fn accept(
        &self,
        descriptor: &CapabilityDescriptor,
        receipt: &ResourceReceipt,
        staged: StagedOutput,
    ) -> Result<AcceptedOutput, AcceptError> {
        descriptor.require(JobCapability::WriteStagedOutput)?;
        if !receipt.outcome().is_acceptable() {
            return Err(AcceptError::RunNotCompleted {
                outcome: outcome_label(receipt.outcome()),
            });
        }
        let bound = descriptor.limits().output_bytes();
        if receipt.output_bytes() > bound {
            return Err(AcceptError::OverOutputBound {
                staged: receipt.output_bytes(),
                bound,
            });
        }
        if staged.job() != descriptor.job().as_str() {
            return Err(AcceptError::JobMismatch {
                staged: staged.job().to_owned(),
                expected: descriptor.job().as_str().to_owned(),
            });
        }
        Ok(AcceptedOutput {
            digest: staged.digest,
            bytes: staged.bytes,
        })
    }
}

fn outcome_label(outcome: &RunOutcome) -> String {
    match outcome {
        RunOutcome::Completed => "COMPLETED".to_owned(),
        RunOutcome::KilledByLimit(kind) => format!("KILLED_BY_{}", kind.as_str()),
        RunOutcome::Failed { exit_code } => format!("FAILED_{exit_code}"),
        RunOutcome::NotStarted { detail } => format!("NOT_STARTED_{detail}"),
    }
}
