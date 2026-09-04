//! Section 23: `taxonomy granularity와 기간을 사용자가 선택한다`.
//!
//! `BlindSpotScope` implements no `Default` and no constant of the type exists
//! in this crate, so a scope nobody chose has no representation.

use academic_blind_spot::BlindSpotScope;

fn shipped() -> BlindSpotScope {
    BlindSpotScope::default()
}

fn main() {}
