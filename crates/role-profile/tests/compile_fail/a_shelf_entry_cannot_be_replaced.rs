//! One organisation's bundle cannot overwrite another's.
//!
//! `BundleShelf::shelve` consumes the shelf and returns a new one, and it
//! refuses an occupied pair. There is no `insert`, no `remove` and no
//! `&mut self` method, so there is no way to replace what is already there.

use academic_role_profile::{BundleShelf, RoleProfile};

fn shape(shelf: &mut BundleShelf, profile: RoleProfile) {
    shelf.insert(profile);
}

fn main() {
    let _ = shape;
}
