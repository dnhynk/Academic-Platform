//! Section 24.2's `backend_engineer_profile_v4`, which is display only.
//!
//! `RoleProfileRef::rendered` writes that spelling for a reader and has no
//! inverse. There is no `TryFrom<String>`, no `FromStr` and no parser, because
//! two different pairs render the same text and reading one back would be the
//! `P2-R4` collision this crate exists to avoid.

use academic_role_profile::RoleProfileRef;

fn main() {
    let _ = RoleProfileRef::try_from("backend_engineer_profile_v4".to_owned());
}
