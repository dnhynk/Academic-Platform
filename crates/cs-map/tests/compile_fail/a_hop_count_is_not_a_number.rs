//! Section 26.4's `1–3 hop`, held by privacy.
//!
//! `HopCount`'s field is private and `HopCount::new` is the only producer, so a
//! caller cannot build a zero-hop or a four-hop neighbourhood by writing the
//! number down.

use academic_cs_map::HopCount;

fn far() -> HopCount {
    HopCount(9)
}

fn main() {
    let _ = far;
}
