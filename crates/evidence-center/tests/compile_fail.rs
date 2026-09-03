//! The type half of three of this task's claims, proved by compilation.
//!
//! A running test cannot observe that a route does not exist: there is no value
//! to construct and no call to make. The cases under `tests/compile_fail` are
//! those calls, written as programs that do not exist to run.
//!
//! * **The four proposal classes are four types.** A string does not produce a
//!   class, and one payload does not stand in for another. If the four were
//!   distinguished by a tag, both would compile.
//! * **A permission that has lapsed blocks its dependents.** `LivePermission`
//!   has no public constructor and no `Clone`, so an expired permission does
//!   not fail a check — it fails to produce the argument.
//! * **A conflict is settled by the user alone.** `settle` takes `P2-M2`'s
//!   `UserDecision`, which `UserDecision::by` issues only for `Actor::User`, so
//!   there is no actor for `settle` to refuse.
//!
//! The suite passes only when each case fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! constructor was added, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.
//!
//! The list of routes each case tries is not the evidence on its own: a case
//! that quietly dropped one would still fail to compile on the others.
//! `the_class_of_an_entry_is_its_payloads_type` and
//! `nothing_but_a_user_settles_a_conflict_or_extends_an_expiry` in
//! `tests/evidence_center_scans.rs` are what hold the lists, by comparing the
//! whole `impl` set naming each type against a pinned one and by counting each
//! private-field type's construction sites.

/// The three cases, in one `trybuild` pass.
#[test]
fn the_centres_typed_doors_have_no_second_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
