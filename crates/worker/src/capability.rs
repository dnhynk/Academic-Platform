//! The capability descriptor a single job runs under.
//!
//! One job, one descriptor. A descriptor names the job, the capabilities it
//! holds, the two staged directories it may reach, the resource limits it runs
//! under, and the instant after which it is worthless. It is issued once,
//! consumed once, and refused after either its expiry or its consumption —
//! `capability_expires_and_cannot_replay` observes both.
//!
//! # The capability set is closed and small
//!
//! [`JobCapability`] has two variants: read the staged input directory, and
//! write the staged output directory. There is deliberately no variant for
//! creating a claim, opening a socket, or reading key material. `P2-G7`'s
//! [`academic_policy::ProcessClass`] matrix is where a process-level capability
//! lives; this enum is one level below it, inside a process that holds none of
//! them. `worker_cannot_publish_a_canonical_claim` reads this enum through a
//! compiler-checked witness `match`, so a variant added later stops that suite
//! compiling rather than silently widening what a job may do.
//!
//! [`academic_policy::ProcessClass`]: https://docs.rs/academic-policy

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::receipt::ResourceLimits;

/// Opaque identity of one job.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(String);

impl JobId {
    /// Wraps a caller-supplied identifier.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::MalformedJobId`] for an empty identifier or
    /// one carrying a byte outside `[0-9a-zA-Z_-]`, because the identifier is
    /// written into the wire descriptor as a line and a separator inside it
    /// would let one field be read as another.
    pub fn new(value: &str) -> Result<Self, DescriptorError> {
        if value.is_empty() || value.len() > 128 {
            return Err(DescriptorError::MalformedJobId);
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(DescriptorError::MalformedJobId);
        }
        Ok(Self(value.to_owned()))
    }

    /// The identifier as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Everything one job may do.
///
/// The list is exhaustive by construction: [`JobCapability::ALL`] is compared
/// against a witness `match` in the acceptance suite, so a new variant fails to
/// compile there before it can be issued anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JobCapability {
    /// Read the descriptor's staged input directory.
    ReadStagedInput,
    /// Write the descriptor's staged output directory.
    WriteStagedOutput,
}

impl JobCapability {
    /// Every capability a job can hold.
    pub const ALL: [Self; 2] = [Self::ReadStagedInput, Self::WriteStagedOutput];

    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadStagedInput => "READ_STAGED_INPUT",
            Self::WriteStagedOutput => "WRITE_STAGED_OUTPUT",
        }
    }

    /// Parses a wire spelling.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::UnknownCapability`] for anything not spelled
    /// by [`JobCapability::as_str`].
    pub fn parse(value: &str) -> Result<Self, DescriptorError> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == value)
            .ok_or(DescriptorError::UnknownCapability)
    }
}

/// A deduplicated, ordered capability set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobCapabilitySet(Vec<JobCapability>);

impl JobCapabilitySet {
    /// Builds a set from any iteration order, sorted and deduplicated.
    #[must_use]
    pub fn new(capabilities: impl IntoIterator<Item = JobCapability>) -> Self {
        let mut held: Vec<JobCapability> = capabilities.into_iter().collect();
        held.sort_unstable();
        held.dedup();
        Self(held)
    }

    /// Whether the set holds one capability.
    #[must_use]
    pub fn holds(&self, capability: JobCapability) -> bool {
        self.0.contains(&capability)
    }

    /// The held capabilities, in canonical order.
    #[must_use]
    pub fn as_slice(&self) -> &[JobCapability] {
        &self.0
    }
}

/// What a descriptor cannot be used for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    /// The identifier is empty, over-long, or carries a separator byte.
    #[error("job identifier is empty, over-long, or not [0-9a-zA-Z_-]")]
    MalformedJobId,
    /// A staged directory path is not absolute, so the child and the parent
    /// could resolve it differently.
    #[error("staged directory {0} is not an absolute path")]
    RelativeStagedDirectory(PathBuf),
    /// The staged input and output directories are the same path.
    #[error("the staged input and output directories are the same path")]
    StagedDirectoriesAlias,
    /// A second descriptor was issued for a job identifier already issued.
    #[error("job {0} already has a descriptor")]
    DuplicateJob(JobId),
    /// No descriptor was ever issued for this job.
    #[error("job {0} has no descriptor")]
    UnknownJob(JobId),
    /// The descriptor's expiry is at or before the instant it was issued.
    #[error("expiry {expires_at} is not after issue time {issued_at}")]
    ExpiryNotAfterIssue {
        /// Issue instant supplied by the caller.
        issued_at: u64,
        /// Expiry instant supplied by the caller.
        expires_at: u64,
    },
    /// The descriptor's expiry has passed.
    #[error("descriptor for job {job} expired at {expires_at}, now {now}")]
    Expired {
        /// Job the descriptor names.
        job: JobId,
        /// Expiry the descriptor carries.
        expires_at: u64,
        /// Instant the use was attempted at.
        now: u64,
    },
    /// The descriptor was already consumed by an earlier run.
    #[error("descriptor for job {job} was already consumed at {consumed_at}")]
    AlreadyConsumed {
        /// Job the descriptor names.
        job: JobId,
        /// Instant of the first consumption.
        consumed_at: u64,
    },
    /// The presented descriptor bytes do not match the issued descriptor.
    #[error("descriptor for job {0} does not match the issued descriptor")]
    DigestMismatch(JobId),
    /// The descriptor does not hold the capability the operation needs.
    #[error("descriptor for job {job} does not hold {capability}")]
    CapabilityNotHeld {
        /// Job the descriptor names.
        job: JobId,
        /// Capability the operation needed.
        capability: &'static str,
    },
    /// A wire descriptor could not be read.
    #[error("wire descriptor is malformed: {0}")]
    MalformedWire(&'static str),
    /// A wire descriptor named a capability this build does not know.
    #[error("wire descriptor names an unknown capability")]
    UnknownCapability,
}

/// One job's capability descriptor.
///
/// A descriptor is issued by a [`DescriptorRegistry`] and carries no secret: it
/// is written into the staged input directory for the sandboxed process to
/// read, and `the_wire_descriptor_carries_no_authority_secret` is what keeps
/// that true. What makes it unforgeable is not its contents but the registry:
/// consumption checks the descriptor's digest against the issued one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    job: JobId,
    capabilities: JobCapabilitySet,
    staged_input: PathBuf,
    staged_output: PathBuf,
    limits: ResourceLimits,
    issued_at: u64,
    expires_at: u64,
}

impl CapabilityDescriptor {
    /// The job this descriptor is for.
    #[must_use]
    pub const fn job(&self) -> &JobId {
        &self.job
    }

    /// The capabilities the job holds.
    #[must_use]
    pub const fn capabilities(&self) -> &JobCapabilitySet {
        &self.capabilities
    }

    /// The directory the job may read.
    #[must_use]
    pub fn staged_input(&self) -> &Path {
        &self.staged_input
    }

    /// The directory the job may write.
    #[must_use]
    pub fn staged_output(&self) -> &Path {
        &self.staged_output
    }

    /// The limits the job runs under.
    #[must_use]
    pub const fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// When the descriptor was issued.
    #[must_use]
    pub const fn issued_at(&self) -> u64 {
        self.issued_at
    }

    /// The instant at and after which the descriptor is worthless.
    #[must_use]
    pub const fn expires_at(&self) -> u64 {
        self.expires_at
    }

    /// Whether the descriptor holds a capability.
    #[must_use]
    pub fn holds(&self, capability: JobCapability) -> bool {
        self.capabilities.holds(capability)
    }

    /// Refuses unless the descriptor holds the capability.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::CapabilityNotHeld`] when it does not.
    pub fn require(&self, capability: JobCapability) -> Result<(), DescriptorError> {
        if self.holds(capability) {
            return Ok(());
        }
        Err(DescriptorError::CapabilityNotHeld {
            job: self.job.clone(),
            capability: capability.as_str(),
        })
    }

    /// The canonical bytes a digest is taken over.
    ///
    /// Fields are length-prefixed so no value can be read as part of another,
    /// and paths are encoded as their lossless byte form on Unix and their
    /// UTF-16 code units on Windows, so a path that is not valid Unicode still
    /// has exactly one encoding.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"academic-worker-descriptor-v1\0");
        push_field(&mut out, self.job.as_str().as_bytes());
        out.extend_from_slice(&(self.capabilities.as_slice().len() as u64).to_be_bytes());
        for capability in self.capabilities.as_slice() {
            push_field(&mut out, capability.as_str().as_bytes());
        }
        push_field(&mut out, &path_bytes(&self.staged_input));
        push_field(&mut out, &path_bytes(&self.staged_output));
        out.extend_from_slice(&self.limits.canonical_bytes());
        out.extend_from_slice(&self.issued_at.to_be_bytes());
        out.extend_from_slice(&self.expires_at.to_be_bytes());
        out
    }

    /// Lowercase SHA-256 of [`CapabilityDescriptor::canonical_bytes`].
    #[must_use]
    pub fn digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_bytes());
        hex_lower(&hasher.finalize())
    }

    /// The form handed to the sandboxed process.
    #[must_use]
    pub fn to_wire(&self) -> WireDescriptor {
        let mut text = String::new();
        text.push_str("academic-worker-descriptor-v1\n");
        text.push_str(&format!("job={}\n", self.job));
        for capability in self.capabilities.as_slice() {
            text.push_str(&format!("capability={}\n", capability.as_str()));
        }
        text.push_str(&format!("input={}\n", self.staged_input.display()));
        text.push_str(&format!("output={}\n", self.staged_output.display()));
        text.push_str(&format!("cpu_millis={}\n", self.limits.cpu_millis()));
        text.push_str(&format!("memory_bytes={}\n", self.limits.memory_bytes()));
        text.push_str(&format!("wall_millis={}\n", self.limits.wall_millis()));
        text.push_str(&format!("output_bytes={}\n", self.limits.output_bytes()));
        text.push_str(&format!("issued_at={}\n", self.issued_at));
        text.push_str(&format!("expires_at={}\n", self.expires_at));
        WireDescriptor(text)
    }
}

/// The descriptor as the sandboxed process receives it.
///
/// It is a newtype rather than a bare `String` so a test can name the exact
/// bytes that cross into the sandbox and assert what is not in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireDescriptor(String);

impl WireDescriptor {
    /// The wire text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reads a wire descriptor back.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::MalformedWire`] when a required line is
    /// missing, repeated where it may not repeat, or unparsable, and
    /// [`DescriptorError::UnknownCapability`] for a capability this build does
    /// not know. An unknown *line* is refused too: a descriptor written by a
    /// newer parent must not be read as a smaller one by an older child.
    pub fn parse(text: &str) -> Result<CapabilityDescriptor, DescriptorError> {
        let mut lines = text.lines();
        if lines.next() != Some("academic-worker-descriptor-v1") {
            return Err(DescriptorError::MalformedWire("version line"));
        }
        let mut single: BTreeMap<&str, &str> = BTreeMap::new();
        let mut capabilities = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(DescriptorError::MalformedWire("line without a separator"));
            };
            if key == "capability" {
                capabilities.push(JobCapability::parse(value)?);
                continue;
            }
            if !matches!(
                key,
                "job"
                    | "input"
                    | "output"
                    | "cpu_millis"
                    | "memory_bytes"
                    | "wall_millis"
                    | "output_bytes"
                    | "issued_at"
                    | "expires_at"
            ) {
                return Err(DescriptorError::MalformedWire("unknown key"));
            }
            if single.insert(key, value).is_some() {
                return Err(DescriptorError::MalformedWire("repeated key"));
            }
        }
        let take = |key: &str| -> Result<&str, DescriptorError> {
            single
                .get(key)
                .copied()
                .ok_or(DescriptorError::MalformedWire("missing key"))
        };
        let number = |key: &str| -> Result<u64, DescriptorError> {
            take(key)?
                .parse::<u64>()
                .map_err(|_| DescriptorError::MalformedWire("non-numeric value"))
        };
        let limits = ResourceLimits::new(
            number("cpu_millis")?,
            number("memory_bytes")?,
            number("wall_millis")?,
            number("output_bytes")?,
        );
        Ok(CapabilityDescriptor {
            job: JobId::new(take("job")?)?,
            capabilities: JobCapabilitySet::new(capabilities),
            staged_input: PathBuf::from(take("input")?),
            staged_output: PathBuf::from(take("output")?),
            limits,
            issued_at: number("issued_at")?,
            expires_at: number("expires_at")?,
        })
    }
}

#[derive(Debug, Clone)]
struct Issued {
    digest: String,
    expires_at: u64,
    consumed_at: Option<u64>,
}

/// The parent-side record of every descriptor issued.
///
/// A descriptor is a value the sandboxed process can copy, so the registry —
/// not the value — is what makes it single-use. [`DescriptorRegistry::consume`]
/// is the only transition, it refuses an expiry that has passed and a job
/// already consumed, and it compares the presented descriptor's digest against
/// the issued one so a re-encoded descriptor with a longer expiry is a
/// mismatch rather than a fresh grant.
#[derive(Debug, Default)]
pub struct DescriptorRegistry {
    issued: BTreeMap<String, Issued>,
}

impl DescriptorRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            issued: BTreeMap::new(),
        }
    }

    /// Issues one descriptor for one job.
    ///
    /// # Errors
    ///
    /// Refuses a relative staged directory, an input and output directory that
    /// are the same path, an expiry that is not after the issue instant, and a
    /// second issue for a job identifier already issued.
    pub fn issue(
        &mut self,
        job: JobId,
        capabilities: JobCapabilitySet,
        dirs: &crate::staging::StagedJobDirs,
        limits: ResourceLimits,
        issued_at: u64,
        expires_at: u64,
    ) -> Result<CapabilityDescriptor, DescriptorError> {
        if expires_at <= issued_at {
            return Err(DescriptorError::ExpiryNotAfterIssue {
                issued_at,
                expires_at,
            });
        }
        for directory in [dirs.input(), dirs.output()] {
            if !directory.is_absolute() {
                return Err(DescriptorError::RelativeStagedDirectory(
                    directory.to_path_buf(),
                ));
            }
        }
        if dirs.input() == dirs.output() {
            return Err(DescriptorError::StagedDirectoriesAlias);
        }
        if self.issued.contains_key(job.as_str()) {
            return Err(DescriptorError::DuplicateJob(job));
        }
        let descriptor = CapabilityDescriptor {
            job: job.clone(),
            capabilities,
            staged_input: dirs.input().to_path_buf(),
            staged_output: dirs.output().to_path_buf(),
            limits,
            issued_at,
            expires_at,
        };
        self.issued.insert(
            job.as_str().to_owned(),
            Issued {
                digest: descriptor.digest(),
                expires_at,
                consumed_at: None,
            },
        );
        Ok(descriptor)
    }

    /// Consumes a descriptor, once.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError::UnknownJob`] when nothing was issued,
    /// [`DescriptorError::DigestMismatch`] when the presented descriptor is not
    /// the issued one, [`DescriptorError::Expired`] at or after the expiry, and
    /// [`DescriptorError::AlreadyConsumed`] on every use after the first.
    ///
    /// The order matters: an expired descriptor is refused as expired whether
    /// or not it was also consumed, so a replay after expiry cannot be reported
    /// as a fresh descriptor that merely ran twice.
    pub fn consume(
        &mut self,
        descriptor: &CapabilityDescriptor,
        now: u64,
    ) -> Result<(), DescriptorError> {
        let job = descriptor.job().clone();
        let record = self
            .issued
            .get_mut(job.as_str())
            .ok_or_else(|| DescriptorError::UnknownJob(job.clone()))?;
        if record.digest != descriptor.digest() {
            return Err(DescriptorError::DigestMismatch(job));
        }
        if now >= record.expires_at {
            return Err(DescriptorError::Expired {
                job,
                expires_at: record.expires_at,
                now,
            });
        }
        if let Some(consumed_at) = record.consumed_at {
            return Err(DescriptorError::AlreadyConsumed { job, consumed_at });
        }
        record.consumed_at = Some(now);
        Ok(())
    }

    /// When a job's descriptor was consumed, if it was.
    #[must_use]
    pub fn consumed_at(&self, job: &JobId) -> Option<u64> {
        self.issued
            .get(job.as_str())
            .and_then(|record| record.consumed_at)
    }
}

fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_be_bytes)
        .collect()
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
