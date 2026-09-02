//! The section 3.7 status set, and the one function that decides it.
//!
//! # Why the deciding function lives beside the enum
//!
//! [`CaptureStatus`] has five variants and two of them permit. Nothing else in
//! this crate returns one: [`status_of`] is the whole derivation, it is pinned
//! as whole text by `consent_scans.rs`, and its call sites are counted there,
//! because `docs/contracts/policy-source-scans.md` records that a pin on a
//! decision says nothing about whether the decision runs.
//!
//! # Absence is `UNKNOWN` and `UNKNOWN` is a refusal
//!
//! [`status_of`] is not reachable without a [`PermissionRecord`], and a record
//! exists only where a written authority issued one. The status of a scope with
//! no record is [`CaptureStatus::Unknown`], which
//! [`ConsentLedger::status`](crate::ConsentLedger::status) returns by having
//! nothing to hand this function. That is the whole of the default: there is no
//! branch that turns an unset field into a permission, because there is no
//! unset field to turn.

use crate::{
    evidence::AuthorityGrant,
    permission::{Disposition, PermissionRecord},
};

/// The section 3.7 status set.
///
/// `Unknown` is [`Default`] deliberately: a value of this type that nobody set
/// is the refusing one. The remaining four are set by [`status_of`] and by
/// nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum CaptureStatus {
    /// No written authority has answered for this scope.
    #[default]
    Unknown,
    /// A written authority refused.
    Prohibited,
    /// A written authority granted, unconditionally, and the checklist is whole.
    Permitted,
    /// A written authority granted and something remains to be satisfied.
    PermittedWithConditions,
    /// A written authority granted and the grant no longer covers this instant.
    Expired,
}

impl CaptureStatus {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Prohibited => "PROHIBITED",
            Self::Permitted => "PERMITTED",
            Self::PermittedWithConditions => "PERMITTED_WITH_CONDITIONS",
            Self::Expired => "EXPIRED",
        }
    }

    /// Whether this status is one a capture may be bound to at all.
    ///
    /// `PermittedWithConditions` is included and that is not a widening: the
    /// conditions themselves are checked separately by
    /// [`bind_permission`](crate::capability::bind_permission), which refuses
    /// while any of them is outstanding. Folding the two questions together
    /// here would make an unsatisfied condition indistinguishable from a
    /// satisfied one at the only place the difference is recorded.
    #[must_use]
    pub const fn is_permitting(self) -> bool {
        matches!(self, Self::Permitted | Self::PermittedWithConditions)
    }
}

/// Derives the section 3.7 status of one recorded permission at one instant.
///
/// The order of the tests is the order section 3.7 states them: a refusal is a
/// refusal whatever else is true; then the two ways a grant stops covering this
/// instant — its own expiry and the scope interval — then the stale
/// verification section 3.7 names beside them; then, and only then, whether
/// anything is outstanding.
///
/// A stale verification is one recorded before the interval it is recorded
/// against began. That is the semester recheck stated as a fact rather than as
/// a timer: a grant verified in one term and carried into the next has a
/// `verified_at` below the new term's `valid_from`, so it expires rather than
/// continuing.
#[must_use]
pub fn status_of(record: &PermissionRecord, at: u64) -> CaptureStatus {
    let grant: &AuthorityGrant = match record.disposition() {
        Disposition::Prohibited(_) => return CaptureStatus::Prohibited,
        Disposition::Granted(grant) => grant,
    };
    if at >= grant.not_after() || !record.scope().contains(at) {
        return CaptureStatus::Expired;
    }
    if record.verified_at() < record.scope().valid_from() {
        return CaptureStatus::Expired;
    }
    if grant.conditions().is_empty() && record.checklist().is_complete() {
        return CaptureStatus::Permitted;
    }
    CaptureStatus::PermittedWithConditions
}
