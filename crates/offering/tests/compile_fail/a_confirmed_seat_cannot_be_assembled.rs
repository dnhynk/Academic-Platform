//! A `ConfirmedSeat` cannot be written out as a struct literal.
//!
//! Writing the literal is the obvious way around `ConfirmedStanding::seat`, so
//! it is written here and required to fail.
//!
//! This case holds **only** the literal. Privacy is checked after type
//! checking, so a file whose type checking already failed never reaches the
//! privacy pass and the `E0451` this case exists for would never be emitted --
//! which is how a compile-fail case that proves nothing still fails to compile
//! and still passes. The `default` and setter attempts are
//! `a_confirmed_seat_has_no_default_and_no_setter.rs` for that reason.

use academic_curriculum::CourseCode;
use academic_domain::TimestampMillis;
use academic_offering::ConfirmedSeat;
use academic_record::term::TermKey;

fn assemble(course: CourseCode, term: TermKey) {
    let _literal = ConfirmedSeat {
        course,
        term,
        verified_at: TimestampMillis::new(0),
        capacity: None,
        meetings: Vec::new(),
    };
}

fn main() {}
