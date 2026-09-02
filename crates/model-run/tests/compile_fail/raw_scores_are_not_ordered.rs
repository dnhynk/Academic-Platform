//! Two providers' raw numbers have no comparison between them.
//!
//! Every spelling of "which is bigger" is attempted here: the operators, the
//! `Ord` methods, the sort, the max, and the ordered collection. If any one of
//! them compiled, a ranking across providers would exist and the prohibition
//! would be a comment.

use std::collections::BTreeSet;

use academic_model_run::{ModelVersion, ProviderId, RawScore};

fn score(provider: &str, version: &str, units: u32) -> RawScore {
    RawScore::new(
        ProviderId::new(provider).unwrap(),
        ModelVersion::new(version).unwrap(),
        units,
    )
}

fn main() {
    let left = score("provider-y", "y-1", 900);
    let right = score("provider-z", "z-1", 300);

    let _by_operator: bool = left < right;
    let _by_ge: bool = left >= right;
    let _by_cmp = left.cmp(&right);
    let _by_partial_cmp = left.partial_cmp(&right);
    let _by_max = left.clone().max(right.clone());

    let mut ranked = vec![left.clone(), right.clone()];
    ranked.sort();
    let _by_iter_max = ranked.iter().max();

    let _ordered: BTreeSet<RawScore> = BTreeSet::from([left, right]);
}
