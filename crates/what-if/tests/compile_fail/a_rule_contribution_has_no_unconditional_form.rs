//! Section 22.2's fourth bullet is `이수한다고 가정했을 때의`.
//!
//! `RuleContribution::under` is crate-private and takes the completion
//! assumption by value. There is no public constructor at all, so a caller
//! cannot state what a plan contributes without the engine and without the
//! assumption.

use academic_what_if::{HypotheticalCompletion, RuleContribution};

fn main() {
    let _contribution = RuleContribution::under(&[], HypotheticalCompletion);
}
