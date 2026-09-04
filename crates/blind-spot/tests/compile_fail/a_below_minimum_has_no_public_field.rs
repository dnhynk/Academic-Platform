//! Section 23: `evidence가 거의 없어 실력을 말할 수 없음`.
//!
//! `BelowMinimum`'s fields are private and `of` refuses a count that is not
//! below the minimum, so an `UNOBSERVED` basis for adequate coverage has no
//! representation.

use academic_blind_spot::BelowMinimum;

fn forge() -> BelowMinimum {
    BelowMinimum {
        observed: 9,
        minimum: 2,
    }
}

fn main() {}
