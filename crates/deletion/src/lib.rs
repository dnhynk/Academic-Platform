//! `P2-P2` — the deletion and retention product flow.
//!
//! `P2-K5` built the mechanism: a plan that enumerates seven derivative
//! classes, a four-word result vocabulary, an append-only journal, a keyless
//! backup tombstone, and two seams — a resolver and an executor — with the
//! sentence "`P2-P2` supplies the real implementation" beside each. This crate
//! is those implementations and the product flow around them.
//!
//! # The order the flow runs in, and what each step refuses
//!
//! | step | type | what it refuses |
//! |---|---|---|
//! | dry run | [`DeletionDryRun`] | nothing; it always carries one node per class, in registry order |
//! | protection | [`ProtectionDecision`] | a refusal with no policy reason — the refusing arm carries one |
//! | impact preview | [`DeletionImpactPreview`] | a citation map that does not cover every artifact the dry run reaches |
//! | confirmation | [`DeletionConfirmation`] | an automatic actor, and a digest that is not the preview's |
//! | execution | [`execute_deletion`] | a plan that is no longer the dry run it came from |
//! | provider | [`ProviderErasureLog`] | a receipt for a request nobody made |
//! | leakage | [`ExternalLeakIncident`] | a close that no recovery step backs |
//!
//! # A locator is not an identity, and this is the layer that had to fix it
//!
//! The fifth `P2-A1` audit found `P1-G1`: deleting two artifacts that held the
//! same bytes in one domain made the second tombstone replace the first, made a
//! restore republish the artifact deleted first as readable, and made the
//! receipt report it as a copy the deletion had deliberately spared. `P2-K5`
//! closed the tombstone record and the tombstone file name, and left the rest
//! open as item `P3-G10` — "adding the artifact to these records is a journal
//! format change and is left for whoever writes the executor".
//!
//! Every identity in this crate is [`DeletionTarget`], the artifact **and** the
//! locator. Every map is keyed by it, the unresolved list names it, the
//! provider log is keyed by it, and [`execute::TargetAdapter`] compares each of
//! `P2-K5`'s locator-keyed actions against the target it is about to run rather
//! than looking one up by locator.
//!
//! # What this is not evidence for
//!
//! **Nothing persists.** There is no `academic-store` edge, this task claims no
//! migration number, and the provider deletion receipt this crate links is the
//! row `academic-policy` already persists — the acceptance suite drives that
//! broker through a dev edge to prove the two are one fact rather than adding a
//! second table for it.
//!
//! **No key is destroyed in the default lane.** The crypto-shred is
//! `academic-vault`'s positioned write and is behind the `deletion-engine`
//! feature, which selects `academic-retention`'s own non-default object lane.
//! A default build of this crate resolves neither the object namespace nor an
//! AEAD, and `deletion_lane_is_not_default` proves it in both directions.
//!
//! **Posture is unchanged.** `adr_002_accepted` stays `false` and
//! `production_data_allowed` stays `false`. Deleting a synthetic artifact is not
//! permission to store a real one.

#![doc(test(attr(deny(warnings))))]

pub mod confirm;
pub mod dry_run;
pub mod error;
pub mod execute;
pub mod executors;
pub mod incident;
pub mod preview;
pub mod protection;
pub mod provider;
pub mod target;

#[cfg(feature = "deletion-engine")]
pub mod engine;

pub use confirm::DeletionConfirmation;
pub use dry_run::{
    ClassTargets, DeletionDryRun, DerivativeIndex, DryRunNode, SPEC_DERIVATIVE_SENTENCE_HEAD,
    SPEC_DERIVATIVE_SENTENCE_TAIL, SPEC_DERIVATIVE_WORDS,
};
pub use error::DeletionFlowError;
pub use execute::{
    ArtifactDeletionReceipt, TargetAdapter, TargetExecutor, UnresolvedTarget, execute_deletion,
};
pub use executors::{DeletionPaths, FilesystemExecutor, KeySlotShredder};
pub use incident::{
    ExposureScope, ExternalLeakIncident, IncidentClosure, IncidentError, LeakIncidentState,
    RecoveryStep,
};
pub use preview::{DeletionImpactPreview, EvidenceCitations};
pub use protection::{
    ProtectionDecision, ProtectionPolicyKind, ProtectionReason, ProtectionRegistry,
};
pub use provider::{ProviderErasureEntry, ProviderErasureLog, ProviderErasureRequest};
pub use target::DeletionTarget;

/// The `RB` rows of the Phase 2 fault matrix this task's acceptance executes.
///
/// t068 section 7 assigns `RB02`-`RB04` to `P2-P2` and `RB01` to `P2-K5`.
/// `P2-K5`'s contract page records that both are true — the mechanism is proved
/// there, and this task replaces the synthetic resolver and executor with real
/// ones — so all four run again here, over the product flow.
pub const PHASE2_DELETION_FAULT_IDS: &[&str] = &["RB01", "RB02", "RB03", "RB04"];

/// Test-only feature name that may activate the `RB01` and `RB02` failpoints.
///
/// Both live in the crates that own the write each one interrupts:
/// `academic-vault` beside the key slot, `academic-retention` beside the
/// tombstone. This crate adds no failpoint of its own.
pub const FAULT_INJECTION_FEATURE: &str = "phase2-fault-injection";

/// Section 34.6's fifth recovery principle, in the specification's own words.
///
/// It is what [`ExternalLeakIncident`] exists for, and
/// `the_leak_principle_is_section_34_6s_own` requires this exact sentence to be
/// in the design document.
pub const EXTERNAL_LEAKAGE_PRINCIPLE: &str =
    "외부 유출은 일반 correction이 아니라 security incident lifecycle로 처리한다.";
