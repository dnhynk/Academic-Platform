//! `a_recording_permission_is_not_built_from_a_string`.
//!
//! Section 25.2's third line names four words and no fifth. They are four arms
//! of a closed enum, and every route from text into one is tried here.

use std::str::FromStr;

use academic_home::RecordingPermission;

fn main() {
    // There is no `FromStr`.
    let _parsed = RecordingPermission::from_str("허용");

    // Nor a `str::parse` through it.
    let _turbofished = "조건부".parse::<RecordingPermission>();

    // Nor a `TryFrom<&str>`.
    let _tried = RecordingPermission::try_from("확인 필요");

    // Nor a `From<&str>`.
    let _converted = RecordingPermission::from("금지");

    // And `spec_words` reads in one direction only: it takes a status and
    // returns text, so it cannot be run backwards to make a fifth value.
    let _backwards: RecordingPermission = RecordingPermission::spec_words("보류");
}
