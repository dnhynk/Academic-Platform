//! The enumerated Phase 1 fault matrix as something a harness can execute.
//!
//! `crates/test-support/src/lib.rs` freezes the twenty-six identifiers and
//! `crates/cli/src/commands/crash_replay.rs` freezes the outcome letters a
//! restart must produce. Neither can *take* a fault. This module adds the third
//! piece: for each identifier, which subsystem owns the failpoint, at which
//! stage of the exit sequence the harness must be standing when it fires, and
//! how it is activated in a child process the harness owns.
//!
//! There are exactly three activation shapes, because the repository already
//! has exactly three:
//!
//! - **Environment selection** (`Activation::Environment`) for the failpoints
//!   that `academic-vault` and `academic-portability` compile under
//!   `phase1-fault-injection`. The child sets one selection variable and one
//!   ready-marker variable, and the owning crate aborts the process itself.
//! - **Injected callback** (`Activation::Injector`) for the failpoints that
//!   `academic-store` and `academic-projections` expose as a trait. The harness
//!   supplies an implementation that writes the ready marker and aborts.
//! - **External seam** (`Activation::ExternalSeam`) for `IPC01` only. See
//!   [`IPC01_REALIZATION`]; this one is not an injected failpoint and the
//!   difference is deliberately carried in the type rather than hidden.
//!
//! Nothing here is reachable from a product build. The environment lookups live
//! inside the owning crates behind their non-default feature, the injector
//! traits are only implemented by this harness, and no product entry point
//! passes anything but the no-fault value.

#![allow(dead_code)]

use std::{
    fs::OpenOptions,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use academic_projections::runner::{
    ProjectionError, ProjectionFaultInjector, ProjectionFaultPoint, ProjectionResult,
};
use academic_store::fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault};

/// How `IPC01` is realized, stated once and quoted by the report and the guide.
///
/// Every other fault is an injected failpoint inside the crate that owns the
/// ordering it protects. `IPC01` is not: the Phase 1 daemon carries no
/// failpoint between reading a complete request and admitting it to the writer
/// queue, and adding one would put a crash switch into the product's own serve
/// path for a single matrix row. The harness therefore composes the same two
/// public steps the daemon composes — `academic_rpc::read_envelope` to
/// completion, then `WriterQueue::try_admit` — and aborts the child between
/// them.
///
/// What that proves: reading a complete, authorized request frame consumes no
/// sequence, writes nothing canonical, and leaves the profile in a state where
/// the client's retry is a fresh admission carrying the same idempotency key.
///
/// What it does not prove: that the product `serve_connection` body has no
/// additional side effect between those two calls. That body is covered by the
/// daemon's own connection tests, not by this fault.
pub const IPC01_REALIZATION: &str = "IPC01 is driven from outside over the public read_envelope -> WriterQueue::try_admit seam \
     rather than from an injected failpoint, because the product daemon carries no crash switch \
     in its serve path";

/// Whether one failpoint can be reached by the Phase 1 exit corpus at all.
///
/// The exit ingests only the single repository-allowlisted synthetic fixture,
/// and that fixture registers exactly one artifact. A failpoint placed strictly
/// between two reachable objects therefore has no position to occupy. The plan
/// requires such a case to be recorded as `NOT_RUN` with a reason rather than
/// coerced to `PASS`, so the reason travels in the matrix itself and the exit
/// report prints it beside the twenty-five rows that did run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    /// The exit corpus can reach this failpoint.
    Reachable,
    /// The exit corpus cannot reach it, for this exact reason.
    NotRunInExitCorpus {
        /// Why the single-artifact exit corpus cannot reach the failpoint.
        reason: &'static str,
        /// The suite that does cover it, against a corpus that can.
        covered_by: &'static str,
    },
}

impl Reachability {
    /// Returns whether the exit harness executes this row.
    #[must_use]
    pub const fn is_reachable(self) -> bool {
        matches!(self, Self::Reachable)
    }
}

/// The artifact whose ordering one failpoint protects.
///
/// Each matrix row's letter describes the disposition of exactly one thing: the
/// row's own termination point names it. `V05` protects the temp file, `V06`
/// protects the already-sealed object, `DB03` protects the canonical
/// transaction, and so on. The physical end state of two adjacent rows can be
/// identical — an interrupted `V06` and an interrupted `DB01` both leave one
/// sealed object with no canonical reference — so the oracle cannot read the
/// letter off the filesystem alone. It measures the disposition of the named
/// subject, which is what the row actually asserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaultSubject {
    /// The unpublished `*.partial` an interrupted seal was writing.
    VaultTemp,
    /// A complete sealed object that no canonical row references yet.
    SealedObject,
    /// The atomic canonical acceptance unit.
    CanonicalTransaction,
    /// A projection generation that was still being built or activated.
    ProjectionGeneration,
    /// An unpublished backup directory.
    BackupDirectory,
    /// An unpublished restore destination.
    RestoreDestination,
    /// A complete request that was read but never admitted to the writer.
    QueuedRequest,
}

impl FaultSubject {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VaultTemp => "VAULT_TEMP",
            Self::SealedObject => "SEALED_OBJECT",
            Self::CanonicalTransaction => "CANONICAL_TRANSACTION",
            Self::ProjectionGeneration => "PROJECTION_GENERATION",
            Self::BackupDirectory => "BACKUP_DIRECTORY",
            Self::RestoreDestination => "RESTORE_DESTINATION",
            Self::QueuedRequest => "QUEUED_REQUEST",
        }
    }
}

/// Subsystem that owns one failpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaultOwner {
    /// `academic-vault` object sealing.
    Vault,
    /// `academic-store` canonical acceptance.
    Store,
    /// `academic-projections` generation build and activation.
    Projections,
    /// `academic-portability` backup and restore.
    Portability,
    /// The local-IPC admission seam.
    Daemon,
}

impl FaultOwner {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vault => "vault",
            Self::Store => "store",
            Self::Projections => "projections",
            Self::Portability => "portability",
            Self::Daemon => "daemon",
        }
    }
}

/// Where in the exit sequence the harness stands when the fault fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaultStage {
    /// The child is sealing and accepting the one allowlisted fixture.
    Ingest,
    /// The child has accepted the fixture and is building a generation.
    ProjectionBuild,
    /// The child has accepted the fixture and is publishing a backup.
    Backup,
    /// The child has a published backup and is restoring it into a new root.
    Restore,
    /// The child has read a complete request and has not admitted it.
    Admission,
}

impl FaultStage {
    /// Returns the stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ingest => "INGEST",
            Self::ProjectionBuild => "PROJECTION_BUILD",
            Self::Backup => "BACKUP",
            Self::Restore => "RESTORE",
            Self::Admission => "ADMISSION",
        }
    }

    /// Returns whether the canonical fixture is already accepted when the child
    /// reaches this stage.
    #[must_use]
    pub const fn ingest_precedes(self) -> bool {
        matches!(self, Self::ProjectionBuild | Self::Backup | Self::Restore)
    }
}

/// How the harness activates one failpoint in the child it owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    /// The owning crate reads a selection variable and aborts itself.
    Environment {
        /// Variable naming the single fault the child must take.
        selection: &'static str,
        /// Variable naming the file the child creates before aborting.
        ready: &'static str,
    },
    /// The harness passes an implementation of the owning crate's trait.
    Injector,
    /// The harness drives a public seam and aborts between two calls.
    ExternalSeam,
}

/// Outcome letter the enumerated matrix assigns to a restart.
///
/// The four letters are the contract, restated here so the oracle can compute
/// them from observations and compare. `crash_replay.rs` holds the same four
/// for the report the CLI prints, and `phase1_exit.rs` asserts that this
/// catalog's letters equal that catalog's letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// No canonical reference, and a recoverable temp or orphan.
    NoReference,
    /// A complete sealed object plus a complete canonical transaction.
    Complete,
    /// Explicit quarantine or repair-required disposition.
    Quarantine,
    /// An idempotent retry returns the original receipt.
    IdempotentRetry,
}

impl Outcome {
    /// Returns the stable single-letter code used by the fault matrix.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NoReference => "N",
            Self::Complete => "C",
            Self::Quarantine => "Q",
            Self::IdempotentRetry => "R",
        }
    }

    /// Renders an ordered outcome list as the matrix spells it, for example
    /// `C+R`.
    #[must_use]
    pub fn render(outcomes: &[Self]) -> String {
        outcomes
            .iter()
            .map(|outcome| outcome.code())
            .collect::<Vec<_>>()
            .join("+")
    }
}

/// One executable row of the enumerated Phase 1 fault matrix.
#[derive(Debug, Clone, Copy)]
pub struct FaultSpec {
    /// Stable fault identifier.
    pub id: &'static str,
    /// Subsystem that owns the failpoint.
    pub owner: FaultOwner,
    /// Stage of the exit sequence the child is in when the fault fires.
    pub stage: FaultStage,
    /// The artifact whose disposition this row's letter describes.
    pub subject: FaultSubject,
    /// How the harness activates the failpoint.
    pub activation: Activation,
    /// Outcome letters the restart must produce, in matrix order.
    pub expected: &'static [Outcome],
    /// Whether the exit corpus can reach this failpoint at all.
    pub reachability: Reachability,
}

impl FaultSpec {
    /// Returns whether this fault must leave a complete canonical transaction.
    #[must_use]
    pub fn expects_commit(&self) -> bool {
        self.expected.contains(&Outcome::Complete)
    }

    /// Returns whether this fault must leave an explicit quarantine or
    /// repair-required disposition.
    #[must_use]
    pub fn expects_quarantine(&self) -> bool {
        self.expected.contains(&Outcome::Quarantine)
    }

    /// Returns the environment pair this fault needs, when it has one.
    #[must_use]
    pub fn environment(&self, ready_marker: &Path) -> Vec<(&'static str, String)> {
        match self.activation {
            Activation::Environment { selection, ready } => vec![
                (selection, self.id.to_owned()),
                (ready, ready_marker.display().to_string()),
            ],
            Activation::Injector | Activation::ExternalSeam => Vec::new(),
        }
    }
}

/// Selection variable `academic-vault` reads under `phase1-fault-injection`.
///
/// The vault keeps its own copy as a private literal inside
/// `crates/vault/src/fault.rs`, so `phase1_exit.rs` scans that source and fails
/// if the two spellings ever diverge. That is the same drift control the
/// repository already applies to the frozen fault-identifier list.
pub const VAULT_FAULT_SELECTION_VARIABLE: &str = "ACADEMIC_VAULT_TEST_FAULT";
/// Ready-marker variable `academic-vault` reads under the same feature.
pub const VAULT_FAULT_READY_MARKER_VARIABLE: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";

/// Selection variable `academic-portability` exports for its own harness.
pub const PORTABILITY_FAULT_SELECTION_VARIABLE: &str =
    academic_portability::fault::FAULT_SELECTION_VARIABLE;
/// Ready-marker variable `academic-portability` exports for its own harness.
pub const PORTABILITY_FAULT_READY_MARKER_VARIABLE: &str =
    academic_portability::fault::FAULT_READY_MARKER_VARIABLE;

const VAULT_ENVIRONMENT: Activation = Activation::Environment {
    selection: VAULT_FAULT_SELECTION_VARIABLE,
    ready: VAULT_FAULT_READY_MARKER_VARIABLE,
};

const PORTABILITY_ENVIRONMENT: Activation = Activation::Environment {
    selection: PORTABILITY_FAULT_SELECTION_VARIABLE,
    ready: PORTABILITY_FAULT_READY_MARKER_VARIABLE,
};

use Outcome::{Complete, IdempotentRetry, NoReference, Quarantine};

/// The complete executable Phase 1 fault matrix, in the frozen matrix order.
pub const PHASE1_EXIT_FAULTS: &[FaultSpec] = &[
    FaultSpec {
        id: "V01",
        owner: FaultOwner::Vault,
        stage: FaultStage::Ingest,
        subject: FaultSubject::VaultTemp,
        activation: VAULT_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "V02",
        owner: FaultOwner::Vault,
        stage: FaultStage::Ingest,
        subject: FaultSubject::VaultTemp,
        activation: VAULT_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "V03",
        owner: FaultOwner::Vault,
        stage: FaultStage::Ingest,
        subject: FaultSubject::VaultTemp,
        activation: VAULT_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "V04",
        owner: FaultOwner::Vault,
        stage: FaultStage::Ingest,
        subject: FaultSubject::VaultTemp,
        activation: VAULT_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "V05",
        owner: FaultOwner::Vault,
        stage: FaultStage::Ingest,
        subject: FaultSubject::VaultTemp,
        activation: VAULT_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "V06",
        owner: FaultOwner::Vault,
        stage: FaultStage::Ingest,
        subject: FaultSubject::SealedObject,
        activation: VAULT_ENVIRONMENT,
        expected: &[Quarantine],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB01",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB02",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB03",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB04",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB05",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB06",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "DB07",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[Complete, IdempotentRetry],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "PR01",
        owner: FaultOwner::Projections,
        stage: FaultStage::ProjectionBuild,
        subject: FaultSubject::ProjectionGeneration,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "PR02",
        owner: FaultOwner::Projections,
        stage: FaultStage::ProjectionBuild,
        subject: FaultSubject::ProjectionGeneration,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "PR03",
        owner: FaultOwner::Projections,
        stage: FaultStage::ProjectionBuild,
        subject: FaultSubject::ProjectionGeneration,
        activation: Activation::Injector,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "BK01",
        owner: FaultOwner::Portability,
        stage: FaultStage::Backup,
        subject: FaultSubject::BackupDirectory,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "BK02",
        owner: FaultOwner::Portability,
        stage: FaultStage::Backup,
        subject: FaultSubject::BackupDirectory,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "BK03",
        owner: FaultOwner::Portability,
        stage: FaultStage::Backup,
        subject: FaultSubject::BackupDirectory,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        // `academic_portability::backup` trips BK03 at `index == 1` of the
        // reachable-object copy, which is the only honest position for "midway
        // through" that copy. The one allowlisted synthetic fixture registers a
        // single artifact, so the exit corpus never reaches index 1 and there is
        // no midpoint to interrupt. Reaching it would need a second allowlisted
        // fixture, which the synthetic-only data policy does not admit, or a
        // moved failpoint, which would change product ordering to make a test
        // pass. Neither is permitted, so this row is NOT_RUN here.
        reachability: Reachability::NotRunInExitCorpus {
            reason: "the single allowlisted synthetic fixture registers one artifact, so the \
                     reachable-object copy has no midpoint for BK03 to interrupt",
            covered_by: "crates/portability/tests/crash.rs::\
                         bk01_bk04_leave_no_partially_published_backup, against a two-artifact \
                         corpus that crate builds for itself",
        },
    },
    FaultSpec {
        id: "BK04",
        owner: FaultOwner::Portability,
        stage: FaultStage::Backup,
        subject: FaultSubject::BackupDirectory,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "RS01",
        owner: FaultOwner::Portability,
        stage: FaultStage::Restore,
        subject: FaultSubject::RestoreDestination,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "RS02",
        owner: FaultOwner::Portability,
        stage: FaultStage::Restore,
        subject: FaultSubject::RestoreDestination,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "RS03",
        owner: FaultOwner::Portability,
        stage: FaultStage::Restore,
        subject: FaultSubject::RestoreDestination,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "RS04",
        owner: FaultOwner::Portability,
        stage: FaultStage::Restore,
        subject: FaultSubject::RestoreDestination,
        activation: PORTABILITY_ENVIRONMENT,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "IPC01",
        owner: FaultOwner::Daemon,
        stage: FaultStage::Admission,
        subject: FaultSubject::QueuedRequest,
        activation: Activation::ExternalSeam,
        expected: &[NoReference],
        reachability: Reachability::Reachable,
    },
    FaultSpec {
        id: "IPC02",
        owner: FaultOwner::Store,
        stage: FaultStage::Ingest,
        subject: FaultSubject::CanonicalTransaction,
        activation: Activation::Injector,
        expected: &[Complete, IdempotentRetry],
        reachability: Reachability::Reachable,
    },
];

/// Returns one row by identifier.
#[must_use]
pub fn spec(id: &str) -> Option<&'static FaultSpec> {
    PHASE1_EXIT_FAULTS.iter().find(|fault| fault.id == id)
}

/// Maps a `DBxx`/`IPC02` identifier onto the store's acceptance checkpoint.
#[must_use]
pub fn acceptance_point(id: &str) -> Option<AcceptanceFaultPoint> {
    Some(match id {
        "DB01" => AcceptanceFaultPoint::Db01,
        "DB02" => AcceptanceFaultPoint::Db02,
        "DB03" => AcceptanceFaultPoint::Db03,
        "DB04" => AcceptanceFaultPoint::Db04,
        "DB05" => AcceptanceFaultPoint::Db05,
        "DB06" => AcceptanceFaultPoint::Db06,
        "DB07" => AcceptanceFaultPoint::Db07,
        "IPC02" => AcceptanceFaultPoint::Ipc02,
        _ => return None,
    })
}

/// Maps a `PRxx` identifier onto the projection builder's checkpoint.
#[must_use]
pub fn projection_point(id: &str) -> Option<ProjectionFaultPoint> {
    Some(match id {
        "PR01" => ProjectionFaultPoint::Pr01MidWrite,
        "PR02" => ProjectionFaultPoint::Pr02AfterChecksum,
        "PR03" => ProjectionFaultPoint::Pr03DuringActivation,
        _ => return None,
    })
}

/// Writes the ready marker that proves the child reached its named checkpoint.
///
/// The marker holds the fault identifier and nothing else. Creation is
/// exclusive, so a second write cannot mask a checkpoint reached twice, and the
/// file is synced before the process dies so the parent always observes it.
pub fn write_ready_marker(path: &Path, fault_id: &str) -> io::Result<()> {
    let mut marker = OpenOptions::new().create_new(true).write(true).open(path)?;
    marker.write_all(fault_id.as_bytes())?;
    marker.sync_all()
}

/// Aborts the current process at one named checkpoint after recording it.
///
/// This is the harness side of a fault, never a product path. `abort` is used
/// rather than `exit` so no destructor, buffered writer, or SQLite cleanup hook
/// can run: the point of the matrix is what survives an ungraceful stop.
fn abort_at(ready_marker: &Path, fault_id: &str) -> ! {
    let _ignored = write_ready_marker(ready_marker, fault_id);
    std::process::abort()
}

/// Terminates the child at one `academic-store` acceptance checkpoint.
#[derive(Debug)]
pub struct AbortAtAcceptance {
    point: AcceptanceFaultPoint,
    fault_id: &'static str,
    ready_marker: PathBuf,
}

impl AbortAtAcceptance {
    /// Builds an injector for one enumerated `DBxx` or `IPC02` identifier.
    #[must_use]
    pub fn new(
        point: AcceptanceFaultPoint,
        fault_id: &'static str,
        ready_marker: impl Into<PathBuf>,
    ) -> Self {
        Self {
            point,
            fault_id,
            ready_marker: ready_marker.into(),
        }
    }
}

impl AcceptanceFaultInjector for AbortAtAcceptance {
    fn hit(&self, point: AcceptanceFaultPoint) -> Result<(), InjectedFault> {
        if point == self.point {
            abort_at(&self.ready_marker, self.fault_id);
        }
        Ok(())
    }
}

/// Terminates the child at one `academic-projections` build checkpoint.
#[derive(Debug)]
pub struct AbortAtProjection {
    point: ProjectionFaultPoint,
    fault_id: &'static str,
    ready_marker: PathBuf,
}

impl AbortAtProjection {
    /// Builds an injector for one enumerated `PRxx` identifier.
    #[must_use]
    pub fn new(
        point: ProjectionFaultPoint,
        fault_id: &'static str,
        ready_marker: impl Into<PathBuf>,
    ) -> Self {
        Self {
            point,
            fault_id,
            ready_marker: ready_marker.into(),
        }
    }
}

impl ProjectionFaultInjector for AbortAtProjection {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == self.point {
            abort_at(&self.ready_marker, self.fault_id);
        }
        Ok::<(), ProjectionError>(())
    }
}

/// Rows the exit corpus cannot reach, with the reason each carries.
#[must_use]
pub fn not_run_in_exit_corpus() -> Vec<&'static FaultSpec> {
    PHASE1_EXIT_FAULTS
        .iter()
        .filter(|fault| !fault.reachability.is_reachable())
        .collect()
}
