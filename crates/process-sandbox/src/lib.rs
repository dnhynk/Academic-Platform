//! `P2-RF21`. The process-class capability declaration, enforced or refused.
//!
//! # The contract
//!
//! [`academic_policy::ProcessClass::capabilities`] is a declaration. Before
//! this crate existed, six process binaries computed that declaration and
//! discarded it, so a `REPOSITORY_ANALYZER` process declared
//! `OpenOutboundSocket = false` and connected to a remote host anyway. The
//! contract this crate fixes is one sentence:
//!
//! > A process runs only while the capabilities it holds are the capabilities
//! > its class declares. Where that cannot be enforced, the process refuses to
//! > start.
//!
//! [`enter`] is the whole of it. It is called at the top of `main`, before any
//! work, and it either returns an [`Enforcement`] — meaning the refusals were
//! installed *and* re-observed from the kernel — or an [`EnforcementError`],
//! and a caller that gets the error must exit without doing work. There is no
//! third outcome: a partial application is an error, and a failed verification
//! is an error.
//!
//! # The contract is one; the mechanism is per platform
//!
//! On Linux the whole backend is self-applied and unprivileged: a Landlock
//! ruleset with no write rule, and a seccomp filter over the socket family.
//!
//! On Windows there is **no** mechanism. A process cannot replace its own
//! primary token, and no user-mode call refuses the creation of a socket
//! handle to the process that asks for it; `docs/contracts/worker-sandbox.md`
//! records the same measurement for `P2-G4`, where the answer is an
//! AppContainer applied by the *parent* that calls `CreateProcessW`. **No
//! launcher in this repository launches a process class**, so on Windows a
//! process class has no enforcing parent and [`enter`] returns
//! [`EnforcementError::Unavailable`]. That is the contract holding, not the
//! contract missing: the declaration and the process agree, because the
//! process does not run.
//!
//! # What is enforced, and what is not
//!
//! [`basis`] answers that for **every** member of
//! [`academic_policy::ProcessCapability::ALL`], through an exhaustive `match`,
//! so a capability added to that vocabulary stops this crate compiling rather
//! than silently joining the unenforced remainder.
//!
//! Two capabilities are [`EnforcementBasis::ProcessBoundary`] —
//! `OpenOutboundSocket` and `WriteStagedArtifact` — and they are enforced at
//! the granularity the operating system offers, which is coarser than the
//! capability name in one direction only:
//!
//! * a class that does not declare `OpenOutboundSocket` gets **no socket at
//!   all**, not "no outbound socket";
//! * a class that does not declare `WriteStagedArtifact` gets **no filesystem
//!   write at all**, not "no staged write".
//!
//! Both refusals are therefore at least as strong as the declaration. The
//! converse is where the remaining gap is and it is named rather than hidden:
//! a class that *does* declare `WriteStagedArtifact` is left unrestricted,
//! because scoping the write to the staged directory needs that directory's
//! path and no process class is handed one. The contract page
//! `docs/contracts/process-capability-enforcement.md` records what would close
//! that.
//!
//! Everything else in the vocabulary is not a thing an operating system can be
//! asked to refuse — `ReadArtifactRange` and `AssembleExport` are shapes of a
//! read, not a syscall — or is refused by a different, named mechanism.
//! [`EnforcementBasis`] carries which, and the reason, as data.

#![deny(missing_docs)]

use std::fmt;

use academic_policy::{ProcessCapability, ProcessClass};

#[cfg(all(feature = "native-enforcement", target_os = "linux"))]
mod linux;

/// Why this crate does or does not refuse one capability at the process
/// boundary.
///
/// Every [`ProcessCapability`] has exactly one of these, from an exhaustive
/// `match` in [`basis`]. The two payload strings are the review: a capability
/// this crate does not enforce says who does, or says why nothing can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EnforcementBasis {
    /// [`enter`] refuses it inside the process, before any work runs, whenever
    /// the class does not declare it.
    ProcessBoundary,
    /// A different mechanism in this repository refuses it. The string names
    /// that mechanism.
    Elsewhere(&'static str),
    /// Nothing refuses it at a process boundary. The string says why, and the
    /// broker's default-deny decision table is the only thing that governs it.
    BrokerOnly(&'static str),
}

impl EnforcementBasis {
    /// Stable spelling, for a receipt line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProcessBoundary => "PROCESS_BOUNDARY",
            Self::Elsewhere(_) => "ELSEWHERE",
            Self::BrokerOnly(_) => "BROKER_ONLY",
        }
    }
}

/// Which capabilities this crate refuses at the process boundary, and why each
/// of the others is not one of them.
///
/// This is a whole-set classification rather than a list of names to refuse:
/// the `match` is exhaustive, so the answer for a capability added later is a
/// compile error here instead of an omission.
#[must_use]
pub const fn basis(capability: ProcessCapability) -> EnforcementBasis {
    match capability {
        // The two the operating system can be asked about.
        ProcessCapability::OpenOutboundSocket | ProcessCapability::WriteStagedArtifact => {
            EnforcementBasis::ProcessBoundary
        }
        // Refused by the device layer P2-L1 measured, between fork and exec on
        // Linux and by CreateProcessW on Windows. Not this crate's boundary.
        ProcessCapability::CaptureDevice => EnforcementBasis::Elsewhere(
            "academic-capture-gate's device ruleset, measured on both hosts by its own probe",
        ),
        // A read this process is already able to perform. Confining it to "the
        // named ranges" is a property of which bytes the core hands over, not
        // of a syscall an operating system can refuse by name.
        ProcessCapability::ReadArtifactRange => EnforcementBasis::BrokerOnly(
            "no operating-system mechanism distinguishes a named artifact range from any other \
             read; what bounds it is which bytes the core stages",
        ),
        ProcessCapability::WriteSearchIndex => EnforcementBasis::BrokerOnly(
            "a search projection is a staged write; refusing it separately from \
             WriteStagedArtifact would need the staged path this process is not handed",
        ),
        ProcessCapability::AnalyzeRepository => EnforcementBasis::BrokerOnly(
            "analysis is computation over bytes already read; no syscall names it",
        ),
        ProcessCapability::AssembleExport => EnforcementBasis::BrokerOnly(
            "assembly is computation over bytes already read; no syscall names it",
        ),
        ProcessCapability::CreateClaim => EnforcementBasis::BrokerOnly(
            "a claim reaches the core over the local transport; what refuses it is the broker's \
             decision table on the receiving side",
        ),
        ProcessCapability::BorrowConnectorCredential => EnforcementBasis::BrokerOnly(
            "a credential handle is minted by the broker; a process that is never handed one \
             holds nothing to refuse",
        ),
        ProcessCapability::StageExternalPayload => EnforcementBasis::BrokerOnly(
            "staging is a write into the core-owned boundary and is governed by \
             WriteStagedArtifact plus the core's acceptance",
        ),
        ProcessCapability::ReadKeyMaterial => EnforcementBasis::BrokerOnly(
            "no process class declares it, and the key hierarchy is not in any process-class \
             crate's dependency closure",
        ),
    }
}

/// The capabilities this crate refuses for one class, in
/// [`ProcessCapability::ALL`] order.
///
/// A capability is refused when this crate enforces it at all and the class
/// does not declare it. The set is therefore the complement of the declaration
/// inside the enforced subset, and never anything else: there is no list of
/// per-class exceptions here to fall out of date.
#[must_use]
pub fn refusals(class: ProcessClass) -> Vec<ProcessCapability> {
    ProcessCapability::ALL
        .into_iter()
        .filter(|capability| matches!(basis(*capability), EnforcementBasis::ProcessBoundary))
        .filter(|capability| !class.allows(*capability))
        .collect()
}

/// Which mechanism installed the refusals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackendId {
    /// A Landlock ruleset and a seccomp filter, applied by this process to
    /// itself.
    LinuxLandlockSeccomp,
}

impl BackendId {
    /// Stable spelling, for a receipt line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinuxLandlockSeccomp => "LINUX_LANDLOCK_SECCOMP",
        }
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why a process must not start.
///
/// Every variant is fail-closed: a caller that receives one has no enforcement
/// and must exit without doing work.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnforcementError {
    /// This build, or this platform, has no mechanism that can refuse what the
    /// class does not declare.
    #[error("no enforcement backend: {reason}")]
    Unavailable {
        /// What was asked and what answered, in words a refusal line can carry
        /// verbatim.
        reason: String,
    },
    /// A syscall the backend needs failed. Nothing is reported as installed.
    #[error("{step} failed: {code}")]
    Syscall {
        /// The step, named after the call it makes.
        step: &'static str,
        /// The platform error number.
        code: i64,
    },
    /// The backend reported success and the kernel disagreed.
    ///
    /// This is the variant that keeps [`enter`] from being a declaration of its
    /// own: what it returns is not "the calls returned zero" but "the kernel,
    /// asked afterwards, says the restriction is in force".
    #[error("enforcement was not confirmed by the kernel: {detail}")]
    NotVerified {
        /// What was asked of the kernel and what it answered.
        detail: String,
    },
}

/// Proof that one process's capabilities are the ones its class declares.
///
/// It has no public constructor other than [`enter`], so a value of this type
/// cannot be produced by a caller that skipped the enforcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enforcement {
    class: ProcessClass,
    backend: BackendId,
    refused: Vec<ProcessCapability>,
    verification: String,
}

impl Enforcement {
    /// The class this enforcement was applied for.
    #[must_use]
    pub const fn class(&self) -> ProcessClass {
        self.class
    }

    /// Which mechanism installed it.
    #[must_use]
    pub const fn backend(&self) -> BackendId {
        self.backend
    }

    /// The capabilities the process no longer holds, in canonical order.
    #[must_use]
    pub fn refused(&self) -> &[ProcessCapability] {
        &self.refused
    }

    /// What the kernel answered when it was asked whether the refusals hold.
    #[must_use]
    pub fn verification(&self) -> &str {
        &self.verification
    }

    /// One line naming the class, the backend, the refusals and the kernel's
    /// answer.
    #[must_use]
    pub fn receipt_line(&self) -> String {
        let refused = self
            .refused
            .iter()
            .map(|capability| capability.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let refused = if refused.is_empty() {
            String::from("<none>")
        } else {
            refused
        };
        format!(
            "{} enforced by {} refusing [{}] verified by {}",
            self.class.as_str(),
            self.backend.as_str(),
            refused,
            self.verification
        )
    }
}

/// The reason [`enter`] refuses when this build compiled no backend.
pub const NO_BACKEND_COMPILED: &str = "this build compiled no enforcement backend: the native-enforcement feature is off, so \
     nothing can refuse a capability this class does not declare";

/// The reason [`enter`] refuses on Windows.
///
/// It is a statement about the platform *and* about this repository, and both
/// halves have to stay true.
/// `the_windows_reason_names_the_parent_that_would_have_to_apply_it` is what
/// fails if a launcher is added and this sentence is not.
pub const WINDOWS_HAS_NO_SELF_APPLIED_MECHANISM: &str = "windows has no user-mode mechanism a process can apply to itself to refuse a socket handle \
     or a file write: a process cannot replace its own primary token, and an AppContainer is \
     applied by the parent that calls CreateProcessW. No launcher in this repository launches a \
     process class, so this process has no enforcing parent";

/// The reason [`enter`] refuses on a platform with no backend.
pub const UNSUPPORTED_PLATFORM: &str =
    "this platform has no enforcement backend in this repository";

/// Applies this class's refusals to the calling process, or refuses to let it
/// start.
///
/// Call it at the top of `main`, before any work. On success the process holds
/// exactly the capabilities in [`ProcessCapability::ALL`] that its class
/// declares, for the two this crate enforces; on failure the caller must exit
/// without doing work.
///
/// # It has to be the main thread, and that is checked rather than asked for
///
/// Both Linux mechanisms are applied to the *calling thread* and inherited by
/// every thread created after it, while the verification reads the thread group
/// leader's `/proc/self/status`. A call from anywhere but the main thread —
/// or after a thread has already been spawned — therefore cannot be confirmed
/// and returns [`EnforcementError::NotVerified`].
/// `entering_off_the_main_thread_is_not_confirmed_by_the_kernel` is that
/// observation.
///
/// # Errors
///
/// [`EnforcementError::Unavailable`] when this build or this platform has no
/// backend, [`EnforcementError::Syscall`] when a step fails — no later step is
/// attempted and nothing partial is reported as success — and
/// [`EnforcementError::NotVerified`] when the kernel, asked afterwards, does
/// not confirm the restriction.
pub fn enter(class: ProcessClass) -> Result<Enforcement, EnforcementError> {
    let refused = refusals(class);
    #[cfg(all(feature = "native-enforcement", target_os = "linux"))]
    {
        let verification = linux::enter(&refused)?;
        Ok(Enforcement {
            class,
            backend: BackendId::LinuxLandlockSeccomp,
            refused,
            verification,
        })
    }
    #[cfg(all(feature = "native-enforcement", windows))]
    {
        let _unused = &refused;
        Err(EnforcementError::Unavailable {
            reason: String::from(WINDOWS_HAS_NO_SELF_APPLIED_MECHANISM),
        })
    }
    #[cfg(all(feature = "native-enforcement", not(target_os = "linux"), not(windows)))]
    {
        let _unused = &refused;
        Err(EnforcementError::Unavailable {
            reason: String::from(UNSUPPORTED_PLATFORM),
        })
    }
    #[cfg(not(feature = "native-enforcement"))]
    {
        let _unused = &refused;
        Err(EnforcementError::Unavailable {
            reason: String::from(NO_BACKEND_COMPILED),
        })
    }
}

/// The line a process-class binary writes to standard error before it exits
/// without doing work.
#[must_use]
pub fn refusal_line(class: ProcessClass, error: &EnforcementError) -> String {
    format!("{} refuses to start: {error}", class.as_str())
}
