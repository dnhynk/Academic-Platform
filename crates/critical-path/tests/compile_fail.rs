//! Section 16's limits, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to do the thing section 16 forbids,
//! and every one fails to compile. The suite passes only when each case fails
//! **and** fails with the committed diagnostic, so a case that stopped proving
//! anything -- because a type grew a constructor, or because the case itself
//! was mistyped into a different error -- is a failure rather than a silent
//! pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_cost_vector_has_no_setter` | a fact edited where it stands |
//! | `a_cost_vector_does_not_compare` | a vector folded into an order |
//! | `a_ranking_cannot_mutate_its_front` | a preference rewriting a fact |
//! | `a_pareto_front_has_no_bare_constructor` | ranking a list elimination never saw |
//! | `a_disclosure_has_no_struct_literal` | a result assembled with a group missing |
//! | `a_slider_has_no_default` | an ordering the engine chose on the user's behalf |
//! | `an_acquisition_option_yields_no_mastery` | a course read as an acquisition |
//! | `a_cost_estimate_has_no_point_accessor` | an unknown cost read as a number |
//!
//! One case per shape. `P2-R4` measured why: bundling two private-field
//! constructions into one case produced **no** diagnostic for one of them,
//! because an `E0560` for an unknown field suppressed the `E0451` for the
//! private ones.

/// The section 16 limits that are types.
#[test]
fn a_vector_cannot_be_folded_or_rewritten() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
