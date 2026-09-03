//! `offering_boundary_rejects_session_transcript`.
//!
//! Section 9: a `CourseOffering` does not contain 매 수업시간의 실제 발화 — the
//! actual utterance of each class hour. Section 8.2 gives it `lectureRefs`,
//! which is a list of identifiers; the session and its transcript belong to
//! `Lecture`, which is `P2-U7`'s and `P2-L2`'s aggregate.
//!
//! `CourseOfferingDraft` therefore takes no transcript, no segment, no
//! utterance, no audio locator, no speaker and no caption, and `CourseOffering`
//! hands none of them back.

use academic_curriculum::{CourseOfferingDraft, OfferingStatus, SectionCode, TermCode};
use academic_domain::{CourseRevisionId, EntityId, OfferingId, TimestampMillis};

fn main() {
    let offering: OfferingId = "01900000-0000-7000-8000-000000000001".parse().unwrap();
    let revision: CourseRevisionId = "01900000-0000-7000-8000-000000000002".parse().unwrap();
    let lecture: EntityId = "01900000-0000-7000-8000-000000000003".parse().unwrap();

    let draft = CourseOfferingDraft::new(
        offering,
        revision,
        TermCode::parse("2026_FALL").unwrap(),
        SectionCode::parse("001").unwrap(),
        OfferingStatus::Confirmed,
        TimestampMillis::new(0),
    )
    .lecture_ref(lecture);

    let _transcript = draft.clone().transcript("what the instructor said");
    let _segments = draft.clone().transcript_segments(Vec::new());
    let _utterances = draft.clone().utterances(Vec::new());
    let _audio = draft.clone().audio_locator(lecture);
    let _speakers = draft.clone().speakers(Vec::new());
    let _captions = draft.clone().captions(Vec::new());

    let built = draft.build();
    let _read_transcript = built.transcript();
    let _read_segments = built.transcript_segments();
    let _read_utterances = built.utterances();
    let _read_speakers = built.speakers();
}
