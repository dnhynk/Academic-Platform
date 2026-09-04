//! Section 25.11's refusal, as a value that does not exist.
//!
//! `InterestStanding` has three arms and neither of the two `REFUSED_STANDINGS`
//! names is among them. There is nothing to write here, which is the point:
//! the standing that would mean *this is my career* has no spelling.

use academic_role_profile::InterestStanding;

fn main() {
    let _ = InterestStanding::Chosen;
}
