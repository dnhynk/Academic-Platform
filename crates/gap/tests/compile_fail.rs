//! Section 15's limits, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to reach a gap without the input
//! section 15 requires for it, and every one fails to compile. The suite passes
//! only when each case fails **and** fails with the committed diagnostic, so a
//! case that stopped proving anything — because a type grew a constructor, or
//! because the case itself was mistyped into a different error — is a failure
//! rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_goal_has_no_default_criteria` | expansion started without success criteria |
//! | `goal_criteria_have_no_struct_literal` | a criteria set assembled past its empty check |
//! | `a_search_cannot_take_a_bare_concept` | a gap found for a concept with no goal |
//! | `an_explanation_has_no_struct_literal` | an explanation assembled past the specificity validator |
//! | `a_prerequisite_edge_has_no_unchecked_constructor` | an edge assembled past `P2-C4`'s registry |
//! | `a_concept_state_has_no_setter` | a state dimension edited where it stands |
//! | `a_gap_case_has_no_single_root_accessor` | a tie resolved by taking one |
//! | `a_root_candidate_has_no_struct_literal` | a candidate assembled past its explanation check |
//!
//! One case per shape. `P2-R4` measured why: bundling two private-field
//! constructions into one case produced **no** diagnostic for one of them,
//! because an `E0560` for an unknown field suppressed the `E0451` for the
//! private ones.

/// The section 15 limits that are types.
#[test]
fn a_gap_cannot_be_reached_without_its_input() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
