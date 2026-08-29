//! The type half of the projected/actual isolation, proved by compilation.
//!
//! Every case under `tests/compile_fail` is a program that tries to carry a
//! projected mastery, opportunity, or workload value to a canonical writer.
//! The suite passes only when each one fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because the
//! seal grew an accessor, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.

/// `projected_type_cannot_call_actual_writer`.
#[test]
fn projected_type_cannot_call_actual_writer() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
