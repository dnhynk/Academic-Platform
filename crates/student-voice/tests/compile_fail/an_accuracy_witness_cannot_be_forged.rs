//! A witness is a measurement that cleared a threshold, and nothing else.
//!
//! Its fields are private, it implements no `Default`, and it has no `new`. The
//! one producer is `DiarizationMeasurement::witness`, which reads a corpus run
//! and a configured permille.

use academic_student_voice::{AccuracyWitness, DiarizationThreshold};

fn main() {
    let _from_default = AccuracyWitness::default();
    let _from_new = AccuracyWitness::new(
        "student-voice-diarization",
        1,
        DiarizationThreshold::new(1, 990, 0),
        1000,
        0,
    );
}
