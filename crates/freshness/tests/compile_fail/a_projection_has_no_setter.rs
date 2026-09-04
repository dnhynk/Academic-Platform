//! A band is recomputed, never edited.
//!
//! `FreshnessProjection` has no public field and no `&mut self` method, so a
//! caller cannot move a band without re-running `project` over the inputs that
//! produced it.

use academic_domain::FreshnessBand;
use academic_freshness::FreshnessProjection;

fn raise(projection: &mut FreshnessProjection) {
    projection.band = FreshnessBand::VeryHigh;
}

fn main() {
    let _ = raise;
}
