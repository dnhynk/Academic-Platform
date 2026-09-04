//! Section 24.3's and 24.4's separations, held by compilation rather than by a
//! check.
//!
//! Every case here tries to do something this task exists to prevent, and every
//! one of them fails to compile. The suite passes only when each case fails
//! **and** fails with the committed diagnostic, so a case that stopped proving
//! anything — because a type grew a conversion, or because the case itself was
//! mistyped into a different error — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_filled_cell_cannot_be_written` | a column settled by nothing |
//! | `a_score_cannot_be_assembled_field_by_field` | a number beside disclosures nobody derived |
//! | `a_score_cannot_be_published_without_its_weights` | section 24.3's fourth disclosure left out |
//! | `a_score_cannot_be_deserialized` | a score read back out of a document |
//! | `a_view_cannot_be_deserialized` | a matrix and a notice a document supplied |
//! | `a_notice_is_not_a_caller_s_sentence` | a non-guarantee notice somebody wrote |
//! | `a_band_is_not_a_cell` | freshness folded into missing and unknown |
//! | `a_stage_is_not_an_axis` | a depth read as a column |
//! | `a_termination_cannot_be_empty` | a navigation direction that ends nowhere |
//! | `a_row_cannot_be_edited_in_place` | a setter on a matrix row |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two refusals into
//! one program produced **no** diagnostic for one of them, because the first
//! error suppressed the second. A bundled case can hide one of its own halves.

/// The section 24.3 and 24.4 separations that are types.
#[test]
fn a_score_a_cell_and_a_walk_cannot_be_written_by_a_caller() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
