//! Section 8.2's `Course`: the identity a university keeps across time.
//!
//! # What is not here, and why that is the whole point
//!
//! Section 9's boundary table gives `Course` one row and states what it does
//! not contain: *특정 교수·학기·시간표·실제 설명* — a particular instructor, a
//! term, a timetable, the actual description. Those are
//! [`crate::offering::CourseOffering`]'s and
//! [`crate::revision::CourseRevision`]'s.
//!
//! They are absent rather than refused. [`CourseDraft`] is the only route to a
//! [`Course`] and it has no setter for any of them, so
//! `tests/compile_fail/course_boundary_rejects_offering_fields.rs` is a
//! compiler diagnostic and not an assertion: there is no field to reject.
//! `crates/curriculum/tests/curriculum_scans.rs` reads the specification's own
//! two lists back out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`
//! and requires every name on them to be absent from this struct, so dropping
//! one from the Rust list fails against the specification rather than passing
//! quietly.
//!
//! # A course code is not an identity
//!
//! [`Course::code`] is what the catalogue prints. Whether two occurrences of
//! one code are one course is [`crate::relation::IdentityDecision`]'s question,
//! and with no decision recorded the answer is
//! [`crate::relation::CourseCodeReuse::Unknown`]. Nothing here infers it.

use academic_domain::{CourseId, EntityId};

use crate::{error::CurriculumError, text::CourseCode};

/// Section 8.2's `Course`: durable identity, and nothing that changes with a
/// revision or a term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Course {
    id: CourseId,
    code: CourseCode,
    canonical_identity: EntityId,
}

impl Course {
    /// The durable identifier.
    #[must_use]
    pub const fn id(&self) -> CourseId {
        self.id
    }

    /// The code the catalogue prints. Not an identity; see
    /// [`crate::relation::CourseCodeReuse`].
    #[must_use]
    pub const fn code(&self) -> &CourseCode {
        &self.code
    }

    /// Section 8.2's `canonicalIdentity`: the registry entity this course means.
    #[must_use]
    pub const fn canonical_identity(&self) -> EntityId {
        self.canonical_identity
    }
}

/// The only route to a [`Course`].
///
/// Private fields and one constructor, the shape
/// `academic_ingestion::ManifestDraft` uses. What it has no setter for is the
/// evidence: an offering field cannot be set here because no method takes one.
#[derive(Debug, Clone)]
pub struct CourseDraft {
    id: CourseId,
    code: CourseCode,
    canonical_identity: Option<EntityId>,
}

impl CourseDraft {
    /// Starts a draft for one identifier and one catalogue code.
    #[must_use]
    pub const fn new(id: CourseId, code: CourseCode) -> Self {
        Self {
            id,
            code,
            canonical_identity: None,
        }
    }

    /// Records which registry entity this course means.
    #[must_use]
    pub const fn canonical_identity(mut self, entity: EntityId) -> Self {
        self.canonical_identity = Some(entity);
        self
    }

    /// Builds the course, naming the first unset attribute.
    pub fn build(self) -> Result<Course, CurriculumError> {
        let canonical_identity = self.canonical_identity.ok_or(CurriculumError::Missing {
            aggregate: "course",
            field: "canonical identity",
        })?;
        Ok(Course {
            id: self.id,
            code: self.code,
            canonical_identity,
        })
    }
}
