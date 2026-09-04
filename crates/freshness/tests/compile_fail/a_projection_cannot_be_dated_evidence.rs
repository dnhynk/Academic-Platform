//! Section 13.3: `전파는 한 단계`, one layer further out.
//!
//! A concept whose band came from a neighbour has nothing to offer a third
//! concept, and the reason is that a `FreshnessProjection` is not a
//! `DatedEvidence`: there is no conversion in either direction, so a projection
//! cannot be handed to `NeighborUse::direct` as the neighbour's own use.

use academic_freshness::{DatedEvidence, FreshnessProjection};

fn as_use(projection: FreshnessProjection) -> DatedEvidence {
    projection
}

fn main() {
    let _ = as_use;
}
