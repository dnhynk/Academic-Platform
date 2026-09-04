//! Section 34.5's `missing/unknown과 freshness를 별도 표시`, as a type error.
//!
//! `AxisCell::Unknown` and `FreshnessBand::Unknown` are spelled the same and
//! mean different things — one is *something arrived and settles nothing*, the
//! other is *nothing datable was ever admitted*. There is no conversion between
//! the reading and the band in either direction.

use academic_readiness::{AxisCell, FreshnessCell};

fn shape(cell: AxisCell) -> FreshnessCell {
    FreshnessCell::of(cell)
}

fn main() {
    let _ = shape;
}
