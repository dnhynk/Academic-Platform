//! Section 24.2's separations, held by compilation rather than by a check.
//!
//! Every case here tries to do something this task exists to prevent, and every
//! one of them fails to compile. The suite passes only when each case fails
//! **and** fails with the committed diagnostic, so a case that stopped proving
//! anything — because a type grew a conversion, or because the case itself was
//! mistyped into a different error — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `an_interest_cannot_be_forked` | a favourite handed to the one act that commits something |
//! | `an_interest_has_no_chosen_standing` | the standing that would mean *this is my career* |
//! | `a_label_is_not_a_direction` | `Backend Engineer` read as `RoleDirection::Backend` |
//! | `a_lineage_identifier_is_not_an_identity` | a layer bound to a lineage rather than a version |
//! | `a_rendered_name_is_not_an_identity` | section 24.2's `_v4` spelling parsed back into a pair |
//! | `a_base_bundle_has_no_adjustment_field` | the user's changes inside the base bundle |
//! | `a_bundle_cannot_be_edited_in_place` | a setter on a version |
//! | `a_version_is_not_a_bare_number` | a zero carried past the positive-integer door |
//! | `a_shelf_entry_cannot_be_replaced` | one organisation's bundle overwriting another's |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two refusals into
//! one program produced **no** diagnostic for one of them, because the first
//! error suppressed the second. A bundled case can hide one of its own halves.

/// The section 24.2 separations that are types.
#[test]
fn a_bundle_cannot_be_edited_forked_or_favourited_into_a_decision() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
