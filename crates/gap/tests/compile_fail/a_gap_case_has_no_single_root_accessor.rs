//! Section 15.2 step 5: `root 후보가 여러 개면 모두 유지하고`.
//!
//! `GapCase::roots` returns every tied root. There is no `primary`, no `best`
//! and no `first_root`, so a caller cannot take one and drop the other.

use academic_gap::{GapCase, RootCandidate};

fn chosen(case: &GapCase) -> &RootCandidate {
    case.primary_root()
}

fn main() {}
