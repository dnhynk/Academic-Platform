//! The type half of five of this task's claims, proved by compilation.
//!
//! A running test cannot observe that a route does not exist: there is no value
//! to construct and no call to make. The cases under `tests/compile_fail` are
//! those calls, written as programs that do not exist to run.
//!
//! * **A percentage is not built from a number.** Section 25.4's last line asks
//!   for the breakdown to be attached always, and `SecondaryPercentage::over`
//!   takes one by value and is the only producer. `P2-Y3` fixed the same shape
//!   with one producer taking four disclosures by value, and `P2-N6` with a
//!   result that does not exist without five public groups.
//! * **A saved plan cannot be restated in place.** Section 25.5 licenses
//!   *무엇이 stale해졌는지만 표시한다* and nothing more, so `restate` takes
//!   `&self` and returns a marking.
//! * **An average has no route without its proof.**
//! * **An audit state is not built from a word.** Section 25.4's four are a
//!   closed set, and a reading is derived from the engine's five rather than
//!   chosen, so the difference between `NEEDS` and `NOT_SATISFIED` cannot be
//!   discarded by constructing a reading directly.
//! * **No surface type has a public field.** That one is alone in its own case
//!   for a measured reason: **E0451 is reported by the privacy pass, which does
//!   not run once type checking has failed.** Two of the four privacy probes in
//!   the first version of this suite were written beside method probes for the
//!   same type and produced no diagnostic at all — the committed `.stderr`
//!   files held the method errors and nothing else, so those two probes were
//!   carrying no load. Reading the committed diagnostic is what found it.
//!
//! The suite passes only when each case fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! constructor was added, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.
//!
//! The list of routes each case tries is not the evidence on its own: a case
//! that quietly dropped one would still fail to compile on the others.
//! `percentage_is_secondary_with_breakdown` and `audit_states_are_exactly_four`
//! in `tests/dashboard.rs`, and
//! `every_capitalized_identifier_in_this_crate_is_in_the_inventory` in
//! `tests/dashboard_scans.rs`, are what hold the whole sets.

/// The five cases, in one `trybuild` pass.
#[test]
fn the_academic_surfaces_typed_doors_have_no_second_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
