//! A `ScoredForecast` cannot be written out by hand.
//!
//! It is the value a `HISTORICALLY_LIKELY` standing takes, and it holds two
//! things at once: a `CalibratedConfidence`, which `P2-M1`'s registry is the
//! only producer of, and a `PredictionMetadata`, whose constructor refuses a
//! zero positive-sample count. Assembling one directly would get round both,
//! so the fields are private and there is no public constructor.

use academic_domain::{PredictionMetadata, PredictionObservationWindow, TimestampMillis};
use academic_model_run::CalibratedConfidence;
use academic_offering::{HistoricallyLikelyStanding, ScoredForecast};

fn assemble(calibrated: CalibratedConfidence) {
    let window = PredictionObservationWindow::new(
        TimestampMillis::new(0),
        TimestampMillis::new(1),
    )
    .unwrap();
    let metadata = PredictionMetadata::new(window, 1).unwrap();
    let _scored = ScoredForecast {
        calibrated,
        metadata,
    };
}

fn declare(scored: ScoredForecast) {
    // And the standing that holds one cannot be written out either.
    let _likely = HistoricallyLikelyStanding { scored };
}

fn main() {}
