//! `P2-G6`: the consent ledger and the section 3.7 capture-permission model.
//!
//! # What is unknown is refused
//!
//! A new offering has no permission record, [`CaptureStatus::Unknown`] is what
//! the absence of a record resolves to, and `Unknown` mints nothing. That is
//! not a policy this crate applies on top of a permissive base: there is no
//! base. [`ConsentLedger`] starts empty, [`permission_for`] returns `None` for
//! every scope until a written grant is appended, and
//! [`mint_capture_capability`] refuses on anything but a live grant. Nothing in
//! this crate has a `Default` that permits, and the two open section 38 cells
//! are typed values with no default at all — see [`gate`].
//!
//! [`permission_for`]: ConsentLedger::permission_for
//!
//! # Evidence is not authority, and the difference is a type
//!
//! Section 12.1 of the authoritative spec says a user attestation records *when
//! and under what conditions* a user heard an oral permission, and is not an
//! override that lets the user create one. Section 3.7 says the same thing as a
//! rule: `user_attestation` is an evidence kind, never a status transition.
//!
//! Here that is two unrelated types. [`AttestationRecord`] is what a user
//! files; [`WrittenAuthority`] is what an instructor, an institution, or an
//! accessibility determination issues; and [`AuthorityGrant::record`] takes the
//! second. There is no `From`, no `TryFrom`, no fallible upgrade, and no
//! function anywhere in this workspace that takes an attestation and returns an
//! authority — the first two are refused by
//! [`the whole set of impl blocks`][impl-set], and the last by a workspace-wide
//! signature rule. `attestation_cannot_be_written_as_authority` in
//! `crates/consent/tests/compile_fail/` is the compiler saying the same thing.
//!
//! [impl-set]: https://docs.rs/ "see crates/consent/tests/consent_scans.rs"
//!
//! # Two retention values, not one
//!
//! [`RetentionTerms`] holds an audio bound and a transcript bound, and no
//! accessor returns one for the other. A lecture may be one where the recording
//! goes but the transcript stays, or the reverse, and a model with one value
//! cannot express either. [`RetentionTerms::inherit`] is how a derivative gets
//! its own pair: independently per axis, and never wider than the parent's.
//!
//! # A legal exception is a task, not a conclusion
//!
//! Section 12.1: an exception needing a legal judgement is left as an item for
//! the institution or a professional, and the system does not infer it. So
//! [`ExternalReviewTask`] is an output with no resolution API in this crate: no
//! function takes one and returns a status, an authority, or a capability, and
//! [`open_external_review`] leaves the scope exactly as unknown as it was.
//!
//! [`open_external_review`]: external::open_external_review
//!
//! # What this crate is not
//!
//! It holds no device handle and opens no database. `P2-L1` owns the daemon
//! evaluation that turns a [`CaptureCapabilityToken`] into a microphone, and
//! the section 3.7 canonical columns are migration
//! `0006_phase2_consent_and_capture.sql`. What this crate owns is the decision
//! and the ledger the decision reads.

pub mod capability;
pub mod checklist;
pub mod evidence;
pub mod expiry;
pub mod external;
pub mod gate;
pub mod ledger;
pub mod permission;
pub mod retention;
pub mod status;

pub use capability::{
    BoundPermission, CaptureCapabilityToken, CaptureDenial, CaptureDenialReason, CaptureRequest,
    bind_permission, continue_capture, mint_capture_capability,
};
pub use checklist::{
    CHECKLIST_DIMENSIONS, Checklist, ChecklistDimension, ChecklistEntry, NotApplicableReason,
};
pub use evidence::{
    AttestationKind, AttestationRecord, AuthorityGrant, EvidenceArtifact, GrantAuthority,
    RefusalRecord, WrittenAuthority, WrittenEvidenceKind,
};
pub use expiry::{
    DERIVATIVE_CLASSES, DeletionImpact, DerivativeClass, DerivativeImpact, ExpiryPlan,
    ExpiryRefusal, MediumImpact, SubjectInventory, apply_expiry, preview_expiry,
};
pub use external::{ExternalReviewTask, LegalQuestion, ReferralTarget, open_external_review};
pub use gate::{OpenGate, UnfilledCell};
pub use ledger::{ConsentEventKind, ConsentLedger, LedgerEntry, RecheckItem};
pub use permission::{
    CaptureMedium, CaptureProcessing, Condition, Disposition, PermissionRecord, PermissionScope,
    PermittedUse, ScopeGrain, Season, TermKey,
};
pub use retention::{RetentionBound, RetentionTerms};
pub use status::CaptureStatus;

use thiserror::Error;

/// Every way this crate refuses to record something.
///
/// A refusal to *record* is separate from a refusal to *capture*: the second is
/// [`CaptureDenial`], which carries the section 3.7 reason a device stays shut.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ConsentError {
    /// A half-open interval was empty or inverted.
    #[error("scope interval is not half-open and non-empty")]
    EmptyInterval,
    /// A grant would outlive the scope that bounds it.
    #[error("grant expiry lies outside the scope interval")]
    GrantOutlivesScope,
    /// A checklist dimension was answered twice.
    #[error("checklist dimension already answered")]
    DimensionAlreadyAnswered,
    /// A permission was appended for a scope that already holds a live one.
    #[error("scope already holds a live permission record")]
    ScopeAlreadyRecorded,
    /// An academic year outside the representable range.
    #[error("term year is out of range")]
    TermYearOutOfRange,
}
