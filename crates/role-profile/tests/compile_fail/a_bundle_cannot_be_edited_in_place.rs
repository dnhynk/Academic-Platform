//! `P2-N2`'s `assertion 은 제자리에서 변경되지 않는다`, one stage over.
//!
//! A revision is `revise`, which borrows its base and returns the next version.
//! There is no setter, no public field and no `&mut self` method anywhere on a
//! bundle, so an edit that wants to be stored has to take a version it did not
//! hold.

use academic_role_profile::{BundleImportance, RoleProfile};

fn shape(profile: &mut RoleProfile) {
    profile.set_importance(BundleImportance::Core);
}

fn main() {
    let _ = shape;
}
