//! Section 23: `과거 evidence는 있으나 최근성 낮음`.
//!
//! `LowRecency`'s field is private and `of` refuses every band outside
//! `LOW_RECENCY_BANDS`, so a `STALE` basis carrying a fresh band has no
//! representation.

use academic_blind_spot::LowRecency;
use academic_domain::FreshnessBand;

fn forge() -> LowRecency {
    LowRecency {
        band: FreshnessBand::VeryHigh,
    }
}

fn main() {}
