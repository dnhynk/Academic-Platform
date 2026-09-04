//! Section 22.2's lane is produced by the engine and by nothing else.
//!
//! `DeterministicResults::of` is crate-private, so a caller cannot state a
//! credit total, a conflict list or a rule contribution the engine did not
//! compute from the frozen inputs.

use academic_what_if::DeterministicResults;

fn main() {
    let _results = DeterministicResults::of(
        todo!(),
        todo!(),
        todo!(),
        todo!(),
        todo!(),
        todo!(),
        todo!(),
    );
}
