//! Section 22.4: a slider reorders and never rewrites.
//!
//! `ComparisonView` holds shared borrows of the plans it orders, so a ranking
//! cannot be assigned through. `P2-N6` holds section 16.2 the same way, and the
//! guarantee here is the same one: the borrow checker, not a rule somebody
//! remembers.

use academic_what_if::{ComparisonView, PlanScenario};

fn overwrite(view: &ComparisonView<'_>, replacement: PlanScenario) {
    *view.ranked()[0] = replacement;
}

fn main() {
    let _ = overwrite;
}
