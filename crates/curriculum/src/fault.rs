//! Deterministic publish checkpoints for an external test harness.
//!
//! This is `academic_store::fault`'s shape, reused rather than reinvented: a
//! callback trait, a production [`NoFault`] whose every checkpoint is a no-op,
//! and no environment-variable, command-line, or process-exit switch anywhere.
//! `academic-retention`'s environment-selected abort is the other shape this
//! repository has, and it is the right one for a fault that has to kill a
//! process mid-write. A curriculum publication is a single in-process append
//! sequence, so the failure that matters is a returned error part-way through
//! it and the callback shape is what expresses that.
//!
//! [`CurriculumPublisher::publish`] consults the injector at each checkpoint in
//! [`PublishCheckpoint::ALL`]. `curriculum_publish_is_atomic_under_injected_failure`
//! walks that list, fails one checkpoint at a time, and requires the ledger to
//! be the value it was before the call — not a subset of it, the same value.
//!
//! [`CurriculumPublisher::publish`]: crate::publish::CurriculumPublisher::publish

use std::fmt;

use crate::error::CurriculumError;

/// A point inside one publication at which a harness may fail the publish.
///
/// The list is the append sequence itself: each checkpoint sits immediately
/// before the write that would make the publication partial if the rewind were
/// wrong. `AfterOffering` and `AfterRelation` are inside loops, so a
/// publication carrying three offerings passes `AfterOffering` three times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PublishCheckpoint {
    /// Before anything is appended.
    BeforeAnything,
    /// After the curriculum version row, before the first course.
    AfterCurriculumVersion,
    /// After one course, before the next aggregate.
    AfterCourse,
    /// After one revision, before the next aggregate.
    AfterRevision,
    /// After one offering, before the next aggregate.
    AfterOffering,
    /// After one relation, before the next.
    AfterRelation,
    /// After every append, before the receipt is returned.
    BeforeReceipt,
}

impl PublishCheckpoint {
    /// Exhaustive listing, in the order a publication reaches them.
    pub const ALL: [Self; 7] = [
        Self::BeforeAnything,
        Self::AfterCurriculumVersion,
        Self::AfterCourse,
        Self::AfterRevision,
        Self::AfterOffering,
        Self::AfterRelation,
        Self::BeforeReceipt,
    ];

    /// Stable spelling used by a report and by a failing assertion.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BeforeAnything => "before-anything",
            Self::AfterCurriculumVersion => "after-curriculum-version",
            Self::AfterCourse => "after-course",
            Self::AfterRevision => "after-revision",
            Self::AfterOffering => "after-offering",
            Self::AfterRelation => "after-relation",
            Self::BeforeReceipt => "before-receipt",
        }
    }
}

/// Callback boundary implemented only by an explicitly supplied harness.
pub trait PublishFaultInjector: fmt::Debug {
    /// Returns `Err` to fail the publication at this checkpoint.
    ///
    /// # Errors
    ///
    /// Whatever the harness decides. [`NoFault`] never returns one.
    fn hit(&self, point: PublishCheckpoint) -> Result<(), CurriculumError>;
}

/// Production injector: every checkpoint is a no-op.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoFault;

impl PublishFaultInjector for NoFault {
    fn hit(&self, _point: PublishCheckpoint) -> Result<(), CurriculumError> {
        Ok(())
    }
}
