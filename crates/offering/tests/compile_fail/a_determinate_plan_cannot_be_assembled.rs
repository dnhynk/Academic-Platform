//! A `DeterminatePlan` cannot be written out as a struct literal.
//!
//! The plan is what says *these seats are real*, so writing one out by hand
//! would get round every seat the commit would otherwise have had to be handed.
//!
//! This case holds only the literal, for the reason
//! `a_confirmed_seat_cannot_be_assembled.rs` records: the privacy pass runs
//! after type checking, so a type error anywhere in the file would suppress the
//! `E0451` this case exists for.

use academic_offering::{ConfirmedSeat, DeterminatePlan};

fn assemble(seats: Vec<ConfirmedSeat>) {
    let _plan = DeterminatePlan { seats };
}

fn main() {}
