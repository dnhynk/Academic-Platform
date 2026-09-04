//! Section 21.2's two offering-bound statuses, held by private fields.
//!
//! `CourseProjectMapping::publish` refuses a status that asserts a particular
//! offering covers the subject when no coverage was observed. A caller who
//! wanted the value anyway has no field to write it into.

use academic_build_learn::{CourseProjectMapping, MappingStatus, NonEmptyText};
use academic_domain::EntityId;

fn assemble(subject: EntityId, reason: NonEmptyText) -> CourseProjectMapping {
    CourseProjectMapping {
        subject,
        designed: None,
        actual: None,
        status: MappingStatus::CanBeSupportedByCurrentCourse,
        reason,
    }
}

fn main() {
    let _ = assemble;
}
