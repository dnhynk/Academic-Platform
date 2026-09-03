//! The type half of two refusals, proved by compilation.
//!
//! `record_fail_closed` observes that no path in this crate opens a device
//! without a live permission, and `violation_risk_blocks_share_and_ai_processing`
//! observes that a quarantined artefact hands out nothing. What neither can
//! observe is a caller who assembles a session field by field, or who reads a
//! quarantined artefact's bytes, because those programs do not exist to run.
//!
//! The two cases under `tests/compile_fail` are those programs. The suite
//! passes only when each fails to compile *and* fails with the committed
//! diagnostic, so a case that stopped proving anything -- because a
//! constructor was added, or because the case itself was mistyped into a
//! different error -- is a failure rather than a silent pass.

/// `a_capture_cannot_be_assembled_without_a_token`.
#[test]
fn a_capture_cannot_be_assembled_without_a_token() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
