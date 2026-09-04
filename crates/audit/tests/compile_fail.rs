//! The type half of every fail-closed contract in this crate, proved by
//! compilation.
//!
//! A running test cannot observe that a route does not exist -- there is no
//! value to construct and no call to make. The cases under
//! `tests/compile_fail` are those calls, written as programs that do not exist
//! to run, and the suite passes only when each fails to compile **and** fails
//! with the committed diagnostic. A case that stopped proving anything --
//! because a constructor was added, or because the case itself was mistyped
//! into a different error -- is a failure rather than a silent pass.
//!
//! | case | what has to be absent |
//! |---|---|
//! | `a_plan_cannot_reach_an_audit` | `DegreeAudit::evaluate` has no plan parameter, and the annotated view produces no audit |
//! | `a_projected_value_cannot_enter_an_audit` | `P2-C7`'s `Proposed<T>` has no exit, so no projection becomes a course fact |
//! | `a_determinate_verdict_has_no_public_constructor` | neither the verdict nor any of its three witnesses can be built from outside |
//! | `a_determinate_verdict_has_no_struct_literal` | its four fields are private |
//! | `a_proof_leaf_has_no_shorter_form` | section 11.3's four parts are four parameters, and there is no setter |
//! | `a_proof_leaf_has_no_struct_literal` | its nine fields are private |
//! | `a_common_rule_example_has_no_remaining_credits` | a public floor carries a threshold and nothing personal |
//! | `an_unrecorded_profile_field_has_no_default` | `Recorded` has no `unwrap_or` and `StudentProfile` has no `Default` |
//!
//! **The two literal cases are separate files on purpose.** `E0451` comes from
//! the privacy pass, which rustc does not reach once type checking has already
//! failed, so a literal sharing a file with any other refused route would be
//! invisible. That is the lesson `crates/requirement/tests/compile_fail`
//! records, applied here.

/// Every absence this crate's contracts rest on, in one `trybuild` pass.
#[test]
fn no_absence_this_crate_relies_on_has_a_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
