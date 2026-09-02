//! The type half of the two calibration prohibitions, proved by compilation.
//!
//! `cross_provider_raw_scores_are_not_ordered` and
//! `uncalibrated_score_cannot_be_displayed` in `tests/model_run.rs` observe what
//! the types do. These two cases observe what they refuse: the suite passes only
//! when each program fails to compile *and* fails with the committed diagnostic,
//! so a case that stopped proving anything -- because `RawScore` grew an
//! accessor, or an ordering trait, or because the case itself was mistyped into a
//! different error -- is a failure rather than a silent pass.

/// `raw_scores_are_not_ordered`, `uncalibrated_score_is_not_displayable`.
#[test]
fn raw_score_reaches_neither_an_ordering_nor_a_display() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
