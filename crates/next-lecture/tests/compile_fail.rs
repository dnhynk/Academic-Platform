//! Section 12.7's limits, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to reach a preparation without the
//! input section 12.7 requires for it, and every one fails to compile. The
//! suite passes only when each case fails **and** fails with the committed
//! diagnostic, so a case that stopped proving anything -- because a type grew a
//! constructor, or because the case itself was mistyped into a different error
//! -- is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `an_expected_concept_claim_has_no_struct_literal` | a claim assembled past its checks, with any standing at all |
//! | `a_prep_uncertainty_has_no_single_confidence` | the three axes folded into one number |
//! | `a_preparation_brief_has_no_push` | a fourth foundation added after the bound was checked |
//! | `a_material_reference_cannot_be_relabelled` | a place relabelled after its document-node rule ran |
//!
//! One case per shape. `P2-R4` measured why: bundling two private-field
//! constructions into one case produced **no** diagnostic for one of them,
//! because an `E0560` for an unknown field suppressed the `E0451` for the
//! private ones.

/// The section 12.7 limits that are types.
#[test]
fn a_preparation_cannot_be_reached_without_its_input() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
