//! Two providers' results are compared and never ranked.
//!
//! `P2-M1` forbids ordering one provider's raw score against another's. The
//! same prohibition one level up is that two *runs* carry no order either:
//! `ProviderRun` implements neither `PartialOrd` nor `Ord`, so `<`, `sort`,
//! `max` and a `BTreeSet` are each a type error rather than a policy somebody
//! has to remember.

use std::collections::BTreeSet;

use academic_transcription::{ProviderRun, RetranscriptionComparison};

fn less(left: &ProviderRun, right: &ProviderRun) -> bool {
    left < right
}

fn ranked(mut runs: Vec<ProviderRun>) {
    runs.sort();
}

fn best(runs: Vec<ProviderRun>) -> Option<ProviderRun> {
    runs.into_iter().max()
}

fn ordered(runs: Vec<ProviderRun>) -> BTreeSet<ProviderRun> {
    runs.into_iter().collect()
}

fn compare_comparisons(a: &RetranscriptionComparison, b: &RetranscriptionComparison) -> bool {
    a > b
}

fn main() {}
