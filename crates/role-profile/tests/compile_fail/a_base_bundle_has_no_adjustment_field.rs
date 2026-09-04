//! Section 24.2's `userAdjustments`, which is a second document.
//!
//! `RoleProfile` has no adjustment field, no accessor for one, and no
//! constructor that takes one. The user's changes are an `AdjustmentLayer`
//! bound to the exact version they were written over, which is what keeps an
//! organisation's bundle byte-identical whatever the user did to it.

use academic_role_profile::RoleProfile;

fn shape(profile: &RoleProfile) {
    let _ = profile.user_adjustments();
}

fn main() {
    let _ = shape;
}
