//! `revision_boundary_rejects_section_fields`.
//!
//! Section 9: a `CourseRevision` does not contain 특정 분반의 현실 — the reality
//! of a particular section. Section 8.2's `CourseOffering` block is that
//! reality: term, section, instructors, meetings, capacity, grading mode,
//! syllabus artifact, the four reference lists, the official status and the
//! observation instant.
//!
//! `CourseRevisionDraft` is the only route to a `CourseRevision` and has a
//! setter for none of them.

use academic_curriculum::{CourseCode, CourseRevisionDraft, CourseTitle, Credits};
use academic_domain::{
    ArtifactId, CourseId, CourseRevisionId, CurriculumVersionId, TimestampMillis, ValidInterval,
};

fn main() {
    let revision: CourseRevisionId = "01900000-0000-7000-8000-000000000001".parse().unwrap();
    let course: CourseId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    let version: CurriculumVersionId = "01900000-0000-7000-8000-000000000003".parse().unwrap();
    let artifact: ArtifactId = "01900000-0000-7000-8000-000000000004".parse().unwrap();
    let code = CourseCode::parse("M1522.001800").unwrap();
    let interval = ValidInterval::open_ended(TimestampMillis::new(0));

    let draft = CourseRevisionDraft::new(revision, course, version, code, interval)
        .title(CourseTitle::parse("데이터베이스").unwrap())
        .credits(Credits::new(3).unwrap());

    let _term = draft.clone().term("2026_FALL");
    let _section = draft.clone().section("001");
    let _instructors = draft.clone().instructor("Instructor");
    let _meetings = draft.clone().meeting(1_u16);
    let _capacity = draft.clone().capacity(40);
    let _grading_mode = draft.clone().grading_mode("LETTER");
    let _syllabus = draft.clone().syllabus_artifact(artifact);
    let _lecture_refs = draft.clone().lecture_ref(course);
    let _assessment_refs = draft.clone().assessment_ref(course);
    let _review_refs = draft.clone().review_ref(course);
    let _status = draft.clone().official_status("CONFIRMED");
    let _observed = draft.clone().observed_at(TimestampMillis::new(0));

    let built = draft.build().unwrap();
    let _read_term = built.term();
    let _read_section = built.section();
    let _read_instructors = built.instructors();
    let _read_capacity = built.capacity();
    let _read_status = built.official_status();
}
