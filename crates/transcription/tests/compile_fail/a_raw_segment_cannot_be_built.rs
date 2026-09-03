//! The same rule one level up: a raw segment is the decoder's to build.

use academic_transcription::{RawSegment, Speaker};

fn segment() -> RawSegment {
    RawSegment {
        id: String::new(),
        start_nanos: 0,
        end_nanos: 1,
        speaker: Speaker::Unresolved,
        verbatim_text: String::new(),
        tokens: Vec::new(),
        source_audio_chunks: Vec::new(),
    }
}

fn main() {}
