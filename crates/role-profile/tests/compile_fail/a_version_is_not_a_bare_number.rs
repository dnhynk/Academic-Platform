//! Section 7.2's `role_profile_version` qualifier, which has a type.
//!
//! It is a positive integer, and `RoleProfileVersion::new` is the one door that
//! refuses zero. A bare `u32` handed where a version belongs would carry a zero
//! straight past that door, so there is no such argument.

use academic_role_profile::{RoleProfileId, RoleProfileRef};

fn main() {
    let _ = RoleProfileRef::of(
        RoleProfileId::new("backend_engineer_profile").unwrap_or_else(|_| unreachable!()),
        0,
    );
}
