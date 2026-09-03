//! Section 13.2's eighth row: `과목 grade → concept별 직접 승격 없음`.
//!
//! There is no `ConceptEvidence` variant for a grade, so a grade cannot be
//! attributed to a concept at all — not because a check refuses it, but because
//! there is no value that would carry it there.

use academic_knowledge_state::{ConceptEvidence, CourseGradeSignal};

fn attribute(signal: CourseGradeSignal) -> ConceptEvidence {
    ConceptEvidence::CourseGrade(signal)
}

fn main() {}
