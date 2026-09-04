//! `missing_admission_no_remaining`, the half a running test cannot observe.
//!
//! Section 11.4: *이 문서는 130학점 등 공개된 공통 사실을 예시로 사용할 뿐,
//! 개인의 "남은 학점"을 산출하지 않는다.* A public floor carries the published
//! threshold and nothing else: there is no attained figure, no remaining
//! figure, and no constructor that takes a transcript to derive one from.

use academic_audit::{CommonRuleExample, CommonRuleExamples, TranscriptSnapshot};

fn main() {
    let example: CommonRuleExample = unimplemented!();

    let _attained = example.attained();
    let _remaining = example.remaining();

    let transcript: TranscriptSnapshot = unimplemented!();
    let _personalized = CommonRuleExamples::of_transcript(&transcript);
}
