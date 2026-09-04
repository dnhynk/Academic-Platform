//! Section 24.2's `id`, which names half of an identity.
//!
//! An adjustment layer is bound to the exact version it was written over, so
//! `AdjustmentLayer::over` takes a `RoleProfileRef` — the lineage-and-version
//! pair. A lineage identifier alone is not one, and there is no conversion:
//! a layer that named only a lineage would be applied to whichever version
//! happened to be in hand.

use academic_role_profile::{AdjustmentLayer, RoleProfileId};

fn main() {
    let _ = AdjustmentLayer::over(
        RoleProfileId::new("backend_engineer_profile").unwrap_or_else(|_| unreachable!()),
        Vec::new(),
    );
}
