//! An uncalibrated score has no route to a reader.
//!
//! The display constructor takes a `CalibratedConfidence`, the raw score has no
//! accessor that returns its number, and it implements no `Display`. Each line
//! below is one of the three ways somebody would reach for the number anyway.

use academic_model_run::{DisplayedConfidence, ModelVersion, ProviderId, RawScore};

fn main() {
    let score = RawScore::new(
        ProviderId::new("provider-y").unwrap(),
        ModelVersion::new("y-1").unwrap(),
        900,
    );

    let _displayed = DisplayedConfidence::of(&score);
    let _formatted: String = format!("{score}");
    let _units: u32 = score.units();
}
