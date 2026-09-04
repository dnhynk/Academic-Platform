//! A forecast has nothing to hand `ConfirmationEvidence::from_registration_system`.
//!
//! Section 8.3: *공식 향후 공지가 생기면 예측을 사실로 "승격"하지 않고 별도
//! official Claim을 활성화한다.* The constructor's first argument is a
//! registration-system reading, and a forecast holds none -- so the promotion
//! is not a check that could be skipped, it is an argument that cannot be
//! supplied.

use academic_offering::{
    ConfirmationEvidence, Forecast, OfferingStanding, ScoredForecast, VerificationRecency,
};
use academic_domain::TimestampMillis;

fn promote(forecast: &Forecast, recency: VerificationRecency) {
    // The forecast is not a listing.
    let _evidence = ConfirmationEvidence::from_registration_system(
        forecast,
        Vec::new(),
        recency,
        TimestampMillis::new(0),
    );
}

fn convert(scored: ScoredForecast) {
    // And there is no conversion in either direction.
    let _into: ConfirmationEvidence = scored.into();
}

fn relabel(standing: OfferingStanding) {
    // Nor a setter on the standing.
    standing.status = academic_curriculum::OfferingStatus::Confirmed;
}

fn main() {}
