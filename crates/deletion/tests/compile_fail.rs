//! The type half of two of this task's claims, proved by compilation.
//!
//! A running test cannot observe that a route does not exist: there is no value
//! to construct and no call to make. The cases under `tests/compile_fail` are
//! those calls, written as programs that do not exist to run.
//!
//! * **A deletion is confirmed by a user, for one preview.**
//!   `DeletionConfirmation` has private fields and one producer, which takes a
//!   preview by value and issues `P2-M2`'s `UserDecision`. The struct literal
//!   is the route that would skip both, and `P2-M4` — the task that forces
//!   non-delegable actions generally — has not merged, so this crate closes its
//!   own case here rather than waiting for it.
//! * **A leak incident is not closed by a claim correction.** Section 34.6's
//!   fifth principle. `IncidentClosure` has one producer that refuses until
//!   every recovery step has happened, and there is no conversion into it or
//!   into the closed state from a `CorrectionRecord`.
//!
//! The suite passes only when each case fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! constructor or a conversion was added, or because the case itself was
//! mistyped into a different error — is a failure rather than a silent pass.
//!
//! The list of routes is not the evidence on its own: a case that quietly
//! dropped one would still fail to compile on the others.
//! `the_impl_blocks_naming_the_gate_types_are_these` in
//! `tests/deletion_scans.rs` is what holds the list, by comparing the whole
//! `impl` header set naming each gate type against a pinned one.

/// The three cases, in one `trybuild` pass.
#[test]
fn the_deletion_flows_typed_doors_have_no_second_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
