//! Section 20.3's `합산 점수로 숨기지 않고`, held by the absence of arithmetic.
//!
//! `Motivation` implements no numeric conversion and no arithmetic trait, so
//! two motivation labels have no `+`. The whole-set impl-header inventory in
//! `crates/build-learn/tests/build_learn_scans.rs` is what states that over
//! every type pair rather than over a list of names; this is the compiled half.

use academic_build_learn::Motivation;

fn total() -> Motivation {
    Motivation::School + Motivation::Role
}

fn main() {
    let _ = total;
}
