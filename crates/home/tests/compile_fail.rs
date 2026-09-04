//! The type half of four of this task's claims, proved by compilation.
//!
//! A running test cannot observe that a route does not exist: there is no value
//! to construct and no call to make. The cases under `tests/compile_fail` are
//! those calls, written as programs that do not exist to run.
//!
//! * **A prerequisite item carries a reason and a time or it does not exist.**
//!   Both are parameters and the fields are private, so there is no state in
//!   which an item exists and either is missing.
//! * **A permission word is not built from text.** Section 25.2's four are a
//!   closed set; if a fifth were expressible, this case would compile.
//! * **A freshness alert needs an upcoming use.** The only producer of one is
//!   `UpcomingUse::declare`, which refuses an occasion that is not ahead of the
//!   instant it was judged from.
//! * **A grouped bucket cannot be shortened.** `GroupedAlerts` hands out
//!   `&[HomeCard]` and owns its three lists, so a caller has nothing to
//!   truncate, drain or retain over.
//!
//! The suite passes only when each case fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! constructor was added, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.
//!
//! The list of routes each case tries is not the evidence on its own: a case
//! that quietly dropped one would still fail to compile on the others.
//! `permission_status_is_exactly_four_values` and
//! `no_gpa_or_streak_hero_component` in `tests/home.rs` are what hold the
//! lists, by comparing whole sets read out of the source in both directions.

/// The four cases, in one `trybuild` pass.
#[test]
fn the_home_surfaces_typed_doors_have_no_second_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
