//! Section 20's and section 21's separations, held by compilation rather than
//! by a check.
//!
//! Every case here tries to do something this task exists to prevent, and every
//! one of them fails to compile. The suite passes only when each case fails
//! **and** fails with the committed diagnostic, so a case that stopped proving
//! anything — because a type grew a constructor, or because the case itself was
//! mistyped into a different error — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `success_criteria_cannot_be_assembled_from_an_empty_list` | a goal stated before its success conditions |
//! | `a_technology_slate_has_no_bare_constructor` | a technology list that precedes the criteria |
//! | `an_architecture_branch_cannot_precede_its_responsibilities` | a branch derived before the capability is decomposed |
//! | `a_branch_member_cannot_be_declared_unconditional` | an `OR` member published without its condition |
//! | `a_learning_item_has_no_evidenceless_constructor` | a learning item with nothing to run |
//! | `a_selection_cannot_be_approved_before_the_simulation` | `선택 승인` before `최소 simulation test` |
//! | `a_motivation_display_does_not_add_up` | the three motivation edges summed |
//! | `a_mapping_cannot_be_assembled_field_by_field` | current-course support with no observed coverage |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two refusals into
//! one program produced **no** diagnostic for one of them, because the first
//! error suppressed the second. A bundled case can hide one of its own halves.

/// The section 20 and section 21 separations that are types.
#[test]
fn the_order_of_a_build_to_learn_plan_cannot_be_written_by_a_caller() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
