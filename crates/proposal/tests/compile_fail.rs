//! The type half of the proposal boundary, proved by compilation.
//!
//! `tests/proposals.rs` observes what the doors do. What it cannot observe is a
//! caller that takes the payload out without passing one, because that program
//! does not exist to run.
//!
//! The six cases under `tests/compile_fail` are those programs: the writer this
//! crate cannot name, the accessor and the field a payload does not come out
//! of, the five unwrapping traits that are not implemented, the two saved
//! records that cannot be assembled by hand, the user decision that cannot be
//! forged, and pending as a state that is not a decision.
//!
//! The harness is `academic-scenario`'s, unchanged: `trybuild::TestCases`
//! against `tests/compile_fail/*.rs` with the diagnostic committed beside each
//! case. The suite passes only when each program fails to compile *and* fails
//! with the committed diagnostic, so a case that stopped proving anything --
//! because a `From` was added, or because the case itself was mistyped into a
//! different error -- is a failure rather than a silent pass.

/// `proposed_type_cannot_reach_canonical_writer`.
#[test]
fn proposed_type_cannot_reach_canonical_writer() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
