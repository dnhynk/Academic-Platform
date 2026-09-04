//! The type half of three of this task's claims, proved by compilation.
//!
//! A running test cannot observe that a route does not exist: there is no value
//! to construct and no call to make. The cases under `tests/compile_fail` are
//! those calls, written as programs that do not exist to run.
//!
//! * **A decision event is issued, not written.** `DecisionEvent` has private
//!   fields and one producer, which issues `P2-M2`'s `UserDecision` and so
//!   refuses every automatic actor. The struct literal is the route that would
//!   skip it.
//! * **An authorised proposal is produced by the layer, not by its caller.**
//!   `AuthorizedProposal` has private fields and no public constructor, so a
//!   caller cannot manufacture the conclusion that a generation was authorised.
//! * **A graduation verdict is not assembled outside `academic-audit`.**
//!   `graduation_result_cannot_come_from_generation` argues that a graduation
//!   result is on a different axis from the six actions. The half of that
//!   argument a running test cannot make is that no crate outside `P2-U3` can
//!   build a verdict at all: `DeterminateVerdict::new` and the three witnesses'
//!   `establish` are `pub(crate)`.
//!
//! The suite passes only when each case fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! constructor or a conversion was added, or because the case itself was
//! mistyped into a different error — is a failure rather than a silent pass.
//!
//! The list of routes is not the evidence on its own: a case that quietly
//! dropped one would still fail to compile on the others.
//! `the_impl_blocks_naming_the_gate_types_are_these` in
//! `tests/non_delegable_scans.rs` is what holds the list, by comparing the whole
//! `impl` header set naming each gate type against a pinned one.

/// The three cases, in one `trybuild` pass.
#[test]
fn the_non_delegable_doors_have_no_second_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
