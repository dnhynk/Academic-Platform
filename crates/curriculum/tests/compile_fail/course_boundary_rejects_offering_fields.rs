//! `course_boundary_rejects_offering_fields`.
//!
//! Section 9: a `Course` does not contain 특정 교수·학기·시간표·실제 설명 — a
//! particular instructor, a term, a timetable, an actual description. Section
//! 8.2 puts each of those on `CourseOffering`.
//!
//! `CourseDraft` is the only route to a `Course`, and it has no setter for any
//! of them. Neither does `Course` have an accessor. There is no run-time check
//! that refuses an instructor here, because there is no field for one to be
//! refused into.

use academic_curriculum::{CourseCode, CourseDraft};
use academic_domain::{CourseId, EntityId};

fn identifiers() -> (CourseId, EntityId) {
    let course: CourseId = "01900000-0000-7000-8000-000000000001".parse().unwrap();
    let entity: EntityId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    (course, entity)
}

fn main() {
    let (course_id, entity) = identifiers();
    let code = CourseCode::parse("M1522.001800").unwrap();
    let draft = CourseDraft::new(course_id, code).canonical_identity(entity);

    // Every one of these is a `CourseOffering` field in section 8.2.
    let _instructors = draft.clone().instructors(Vec::new());
    let _term = draft.clone().term("2026_FALL");
    let _section = draft.clone().section("001");
    let _meetings = draft.clone().meetings(Vec::new());
    let _capacity = draft.clone().capacity(40);
    let _grading_mode = draft.clone().grading_mode("LETTER");
    let _syllabus = draft.clone().syllabus_artifact(entity);

    let course = draft.build().unwrap();
    let _read_instructors = course.instructors();
    let _read_term = course.term();
    let _read_meetings = course.meetings();
    let _read_capacity = course.capacity();
}
