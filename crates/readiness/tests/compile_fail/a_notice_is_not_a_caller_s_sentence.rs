//! The non-guarantee notice is the design document's own words.
//!
//! `NonGuaranteeNotice::rendered` takes no argument and the type has no public
//! field, so there is no expression anywhere that produces a *different*
//! notice. That is what lets `published_notice` compare a restored document
//! against a value with one producer.

use academic_readiness::NonGuaranteeNotice;

fn shape() -> NonGuaranteeNotice {
    NonGuaranteeNotice::rendered("a number that guarantees nothing")
}

fn main() {
    let _ = shape;
}
