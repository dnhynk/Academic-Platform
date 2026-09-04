//! A `ConfirmedSeat` has no `Default` and no setter.
//!
//! The two shapes a caller reaches for when the struct literal fails: a
//! `Default` that would fabricate a seat out of nothing, and a field write that
//! would move a legitimately obtained seat onto another term.

use academic_domain::TimestampMillis;
use academic_offering::ConfirmedSeat;

fn defaulted() {
    let _seat = ConfirmedSeat::default();
}

fn overwrite(seat: &mut ConfirmedSeat) {
    seat.verified_at = TimestampMillis::new(0);
}

fn main() {}
