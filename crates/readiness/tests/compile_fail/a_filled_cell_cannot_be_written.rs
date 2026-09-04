//! A cell says what evidence settled it, and a caller does not get to say it.
//!
//! `AxisCell::read` is the one producer, and it is a pure function of the
//! column, the competency and the placements. The two variants that carry
//! something are `#[non_exhaustive]`, so an *empty* filled cell — the shape
//! that would say a column is settled by nothing — is not a value another
//! crate can write.

use academic_readiness::AxisCell;

fn shape() -> AxisCell {
    AxisCell::Evidenced(Vec::new())
}

fn main() {
    let _ = shape;
}
