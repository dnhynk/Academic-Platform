//! Section 25.11, as an argument that does not exist.
//!
//! `fork` is the one act section 24.2 says a user performs on a bundle, and it
//! takes a bundle. A favourite is a `RoleInterest`, which is not one, so
//! `role을 즐겨찾기해도 "진로 확정"으로 간주하지 않는다` is a type error rather
//! than a rule somebody could forget to apply.

use academic_role_profile::{
    BundleScope, InterestStanding, RecordedOn, RoleInterest, RoleLabel, RoleProfileId, fork,
};

fn main() {
    let interest = RoleInterest::in_role(
        RoleProfileId::new("backend_engineer_profile").unwrap_or_else(|_| unreachable!()),
        InterestStanding::Favorited,
    );
    let _ = fork(
        &interest,
        RoleProfileId::new("north_org_backend").unwrap_or_else(|_| unreachable!()),
        RoleLabel::new("Backend Engineer, North Org").unwrap_or_else(|_| unreachable!()),
        RecordedOn::parse("2026-09-03").unwrap_or_else(|_| unreachable!()),
        BundleScope::new("north_org_platform_team").unwrap_or_else(|_| unreachable!()),
        Vec::new(),
    );
}
