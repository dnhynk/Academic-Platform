//! Section 13.3: `시간 decay는 freshness projection에만 적용한다`.
//!
//! The decay function takes an elapsed span and a persistence window. A mastery
//! level is not either of those, so a decay that consumed one is a program that
//! does not compile — and `the_freshness_crate_cannot_name_a_mastery` observes
//! that the crate has no name for one to begin with.

use academic_domain::MasteryLevel;
use academic_freshness::{PersistenceClass, UNCALIBRATED_PRIOR_V1, decay};

fn main() {
    let window = UNCALIBRATED_PRIOR_V1.window_of(PersistenceClass::ExposureOrReview);
    let _band = decay(MasteryLevel::Applied, window);
}
