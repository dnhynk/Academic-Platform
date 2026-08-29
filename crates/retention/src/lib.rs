//! Key rotation, recipient revocation, crypto-shredding, and the retention
//! result vocabulary (`P2-K5`).
//!
//! Four things live here because each one is a consequence of the others.
//!
//! 1. **The rotation journal.** `KEK_d` and `SKEY_p` are deterministic
//!    functions of the Vault Master Key, so rotating a domain key means
//!    rotating the master key, so one rotation moves every object and the
//!    database together and cannot be atomic. An append-only journal is what
//!    makes an interrupted rotation resumable and what makes the invariant
//!    below decidable without holding a key.
//! 2. **Recipient add and revoke.** A rotation is only meaningful if the new
//!    generation can be withheld from someone. Revocation is what withholds it,
//!    and [`recipients::REVOCATION_SCOPE_STATEMENT`] is the exact and only
//!    claim it makes.
//! 3. **Crypto-shredding.** Revocation cannot reach a single artifact and
//!    cannot reach plaintext that was already read. Destroying an object's key
//!    slot can reach the artifact, and `academic-vault` owns that write because
//!    the object format is its contract.
//! 4. **The deletion plan and its four-word result.** A shred that reached the
//!    object and missed its transcript, its embedding, its cache, and the
//!    backup it is copied into is not a deletion. The plan enumerates every
//!    derivative class, always, and `PARTIAL` names exactly what is left.
//!
//! # The invariant
//!
//! > After an interruption at any point, exactly one of the old and new keys
//! > opens any object or database.
//!
//! [`rotation`] states how the ordering makes it hold, and
//! [`engine::observe_reachable_opening`] is the executable check. Both "both
//! open" and "neither opens" are violations, and both are reachable if the
//! ordering rules are removed, which is what the acceptance suite injects.
//!
//! # What this crate does not do
//!
//! It opens no database, wraps no key, reaches no key broker, and reads no
//! clock. The store database is a planned, journalled rotation unit whose
//! executor is the encrypted store lane's `PRAGMA rekey`; the two lanes cannot
//! link into one binary, so this crate refuses to record that unit as migrated
//! rather than pretending to have moved it.
//!
//! # Posture
//!
//! Nothing here is ADR-002, ADR-004, ADR-005, or ADR-012 acceptance.
//! `adr_002_accepted` stays `false` and `production_data_allowed` stays
//! `false`. Shredding a synthetic object is not permission to store a real one.

pub mod entry;
pub mod execute;
pub mod fault;
pub mod journal;
pub mod plan;
pub mod recipients;
pub mod rotation;
pub mod tombstone;

#[cfg(feature = "rotation-engine")]
pub mod engine;

pub use entry::{JournalEntry, PlannedUnit, UnitKind};
pub use execute::{ActionId, ExecutionFailure, RetentionExecutor, settle};
pub use fault::{
    FAULT_ACTION_VARIABLE, FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE, FAULT_SELECTORS,
};
pub use journal::{
    AppendOnlyJournal, DELETION_JOURNAL_RELATIVE_PATH, JOURNAL_VERSION, JournalError,
    JournalRecord, ROTATION_JOURNAL_RELATIVE_PATH,
};
pub use plan::{
    ActionKind, ClassResolution, DERIVATIVE_CLASSES, DeletionPlan, DerivativeClass,
    DerivativeResolver, GATE_38_026_STATEMENT, OriginalVoiceAuthority, PlanNode, PlannedAction,
    RETENTION_OUTCOMES, RetentionOutcome, RetentionSubject, SubjectScope, UnresolvedLocator,
    UnresolvedReason, UnresolvedSet, VoiceSpan,
};
pub use recipients::{
    RECIPIENTS_RELATIVE_PATH, REVOCATION_SCOPE_STATEMENT, RecipientError, RevocationOutcome,
};
pub use rotation::{
    KeyGeneration, OpeningGeneration, RotationError, RotationId, RotationPlan, RotationState,
    RotationUnit, UnitProgress, UnitState,
};
pub use tombstone::{BackupTombstone, TOMBSTONE_DIRECTORY, TombstoneError};

/// The `KY` rows of the Phase 2 fault matrix this task owns.
///
/// `KY01`, `KY02`, `KY06`-`KY08` are `P2-K1`'s and live in `academic-crypto`.
pub const PHASE2_ROTATION_FAULT_IDS: &[&str] = &["KY03", "KY04", "KY05"];

/// The `RB` rows of the Phase 2 fault matrix this task's acceptance covers.
///
/// t068 section 7 lists `RB02`-`RB04` under `P2-P2`, which wires the real
/// derivative subsystems; section 5 requires `P2-K5`'s acceptance to cover
/// `RB01`-`RB04`. Both are true: the mechanism and its outcomes are proved
/// here, and `P2-P2` replaces the synthetic resolver and executor with real
/// ones. `RB01`'s failpoint is in `academic-vault`, beside the key slot it
/// destroys.
pub const PHASE2_RETENTION_FAULT_IDS: &[&str] = &["RB01", "RB02", "RB03", "RB04"];

/// Test-only feature name that may activate the `KY03`-`KY05` and `RB02` failpoints.
pub const FAULT_INJECTION_FEATURE: &str = "phase2-fault-injection";
