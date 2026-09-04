//! An indeterminate plan with an empty refusal list is not a call that can be
//! written.
//!
//! Section 8.3's `UNCERTAIN` row requires 경고와 대체 경로 -- a warning *and* an
//! alternative path -- so a refusal list nobody filled in would satisfy the
//! letter and lose the point. The first refusal is a parameter, which is the
//! shape `P2-U3` used for `IndeterminateVerdict`.

use academic_offering::IndeterminatePlan;

fn empty() {
    let _outstanding = IndeterminatePlan::new(Vec::new());
}

fn main() {}
