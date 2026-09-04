//! Section 22's limits that are types, proved by compilation.
//!
//! Each case under `tests/compile_fail` is a program a reader might reasonably
//! write and that must not compile. Two of them fail because a *module* does
//! not resolve — the canonical writer and `P2-U3`'s graduation verdict are
//! outside this package's dependency closure entirely — and the rest fail
//! because the value they want has no public constructor, no accessor, or no
//! `Default`.
//!
//! The suite passes only when each case fails to compile **and** fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! type grew an accessor, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.

/// `plan_scenario_never_writes_actual_state`, and the eight limits beside it.
#[test]
fn a_plan_cannot_reach_actual_state() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
