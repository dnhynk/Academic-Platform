//! Section 13.3: `실제 사용자의 회상 확인으로 calibration한다`.
//!
//! `calibrate` takes `RecallCheck`s, which are built from the user's own
//! statement or from a contrary event. A projected band is neither, so
//! calibration cannot become a second path by which one concept's state reaches
//! another's.

use academic_freshness::{FreshnessProjection, PersonalizationSpeed, UNCALIBRATED_PRIOR_V1};

fn calibrate_from_band(projection: FreshnessProjection, speed: PersonalizationSpeed) {
    let _ = UNCALIBRATED_PRIOR_V1.calibrate(&[projection], speed);
}

fn main() {
    let _ = calibrate_from_band;
}
