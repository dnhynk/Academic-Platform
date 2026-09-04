//! One grant is one read.
//!
//! `RestrictedOriginal::open` takes the grant **by value**, so a second read on
//! one authorization is a use of a moved value. There is no spent flag to
//! forget to set and no `AccessRefusal` variant for it, because the compiler
//! answers first.

use academic_student_voice::{RawAccessGrant, RawAccessLog, RestrictedOriginal};

fn twice(original: &RestrictedOriginal, grant: RawAccessGrant, log: &mut RawAccessLog) {
    let _first = original.open(grant, log);
    let _second = original.open(grant, log);
}

fn main() {}
