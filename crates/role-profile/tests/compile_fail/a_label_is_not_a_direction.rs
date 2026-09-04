//! Section 24.2's `role 이름을 시장의 단일 진리로 두지 않는다`, as a missing
//! conversion.
//!
//! Reading the words `Backend Engineer` as `RoleDirection::Backend` is exactly
//! the inference the sentence refuses. There is no `From<RoleLabel>` for a
//! direction and none back: a bundle's direction is a field the user set.

use academic_role_profile::{RoleDirection, RoleLabel};

fn main() {
    let label = RoleLabel::new("Backend Engineer").unwrap_or_else(|_| unreachable!());
    let _: RoleDirection = RoleDirection::from(label);
}
