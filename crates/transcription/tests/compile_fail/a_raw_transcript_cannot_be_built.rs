//! And one level up again: a raw transcript names the run that produced it, and
//! a caller cannot assemble one that names a run nothing performed.

use academic_transcription::RawTranscript;

fn transcript() -> RawTranscript {
    RawTranscript {
        segments: Vec::new(),
    }
}

fn main() {}
