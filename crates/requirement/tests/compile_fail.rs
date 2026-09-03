//! The type half of the review gate, proved by compilation.
//!
//! Section 11.2: *사람이 검토한 executable rule만 production audit에
//! 사용한다*. A running test cannot observe that a route does not exist --
//! there is no value to construct and no call to make. The cases under
//! `tests/compile_fail` are those calls, written as programs that do not exist
//! to run.
//!
//! Each one tries to get from a `RuleCandidate` -- what a model extracted -- to
//! something an audit executes, by every route the crate's surface offers:
//! constructing the intermediate directly, constructing the executable rule
//! directly, handing the candidate to the draft, handing it to the evaluator,
//! and coercing between the three. The suite passes only when each case fails
//! to compile *and* fails with the committed diagnostic, so a case that stopped
//! proving anything -- because a constructor was added, or because the case
//! itself was mistyped into a different error -- is a failure rather than a
//! silent pass.
//!
//! The list of routes each case tries is not the evidence on its own: a case
//! that quietly dropped one would still fail to compile on the others.
//! `the_only_route_to_an_executable_rule_is_the_gate` in
//! `tests/requirement_scans.rs` is what holds the list, by pinning the gate
//! whole, counting the construction sites of both private-field types at one
//! each, and comparing the whole `impl` set naming either of them against a
//! pinned list -- so a `From` nobody predicted appears as an extra key.

/// `a_candidate_cannot_be_published`, `a_candidate_cannot_be_evaluated` and
/// `an_executable_rule_has_no_public_constructor`, in one `trybuild` pass.
#[test]
fn a_model_candidate_cannot_reach_an_audit() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
