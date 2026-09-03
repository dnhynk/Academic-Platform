//! The type half of the optimistic seal and of the command allowlist.
//!
//! Each case under `tests/compile_fail` is a program that tries to get a value
//! out of an unaccepted update, or to name a local-core capability the typed
//! allowlist does not hold. The suite passes only when each one fails to
//! compile *and* fails with the committed diagnostic, so a case that stopped
//! proving anything -- because the seal grew an accessor, or because the case
//! was mistyped into a different error -- is a failure rather than a silent
//! pass.

/// `optimistic_update_is_not_canonical_before_receipt`.
#[test]
fn optimistic_update_has_no_exit_but_a_receipt() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
