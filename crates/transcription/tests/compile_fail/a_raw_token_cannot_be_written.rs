//! Nothing hands out a mutable raw token.
//!
//! `RawSegment` returns `&[RawToken]` and has no `&mut` counterpart, its own
//! `tokens` field is private, and `RawTranscript` has no mutating method at
//! all. A correction is a new version in `TranscriptLineage`, never an edit
//! here.

use academic_transcription::{RawSegment, RawTranscript};

fn write_token(segment: &mut RawSegment) {
    segment.tokens_mut();
    segment.tokens = Vec::new();
}

fn write_transcript(transcript: &mut RawTranscript) {
    transcript.segments_mut();
    transcript.segments = Vec::new();
}

fn main() {}
