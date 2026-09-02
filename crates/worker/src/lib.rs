//! `P2-G4` worker and provider sandbox: the containment boundary a pipeline job
//! and a provider SDK call run inside.
//!
//! Four things are fixed here, and they are separate on purpose.
//!
//! [`capability`] is the descriptor a job runs under: one job, one capability
//! set, one expiry, one use. It names the staged input and output directories
//! and the resource limits, and it is the only thing the parent hands the
//! sandboxed process.
//!
//! [`staging`] is the pair of directories and the acceptance boundary above
//! them. A worker writes bytes; it cannot turn them into anything the rest of
//! the system will read. Only a [`staging::StagingAuthority`] — which the
//! parent holds and never serializes into the descriptor — produces an
//! [`staging::AcceptedOutput`].
//!
//! [`receipt`] is the measurement. A run that produced anything produced a
//! [`receipt::ResourceReceipt`] with it, because [`receipt::WorkerRun`] pairs
//! the two in one value with no other constructor.
//!
//! [`sandbox`] is the operating system. It has two measured backends behind the
//! non-default `native-sandbox` feature — seccomp plus Landlock plus rlimits on
//! Linux, an AppContainer plus a job object on Windows — and reports
//! [`sandbox::Availability`] rather than assuming either.
//!
//! # What this crate does not claim
//!
//! The default feature set installs nothing. With `native-sandbox` off, every
//! type here is bookkeeping and the operating system permits the worker
//! everything it permits any process. The claim that the OS refuses a read, a
//! socket, or a child is carried by the probe binary and the execution tests
//! behind that feature, on the platform they ran on, and nowhere else.

#![deny(missing_docs)]

pub mod capability;
pub mod job;
pub mod receipt;
pub mod sandbox;
pub mod staging;

pub use capability::{
    CapabilityDescriptor, DescriptorError, DescriptorRegistry, JobCapability, JobCapabilitySet,
    JobId, WireDescriptor,
};
pub use job::{JobOperation, JobPlan, JobRequest, OperationOutcome, ProbeReport};
pub use receipt::{LimitKind, ResourceLimits, ResourceReceipt, RunOutcome, WorkerRun};
pub use sandbox::{Availability, BackendId, SandboxError, SandboxUnavailable};
pub use staging::{AcceptError, AcceptedOutput, StagedJobDirs, StagedOutput, StagingAuthority};
