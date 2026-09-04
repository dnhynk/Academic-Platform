//! Section 17.6's separations, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to reach a `User APPLIED Concept`
//! without the evidence section 17.6 requires for it, and every one of them
//! fails to compile. The suite passes only when each case fails **and** fails
//! with the committed diagnostic, so a case that stopped proving anything —
//! because a type grew a constructor, or because the case itself was mistyped
//! into a different error — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_review_is_never_an_authorship_mode` | a review or a read in the field a claim serializes its authorship into |
//! | `a_contribution_kind_is_not_an_authorship_mode` | a conversion between the two vocabularies beside the one door |
//! | `a_warrant_step_cannot_be_skipped` | an explanation over generated code the user never modified |
//! | `a_generated_warrant_has_no_partial_constructor` | a warrant from a `Default`, a struct literal or a `new` |
//! | `a_code_origin_cannot_be_generated_without_a_warrant` | generated code with the warrant missing |
//! | `an_authored_work_cannot_be_assembled_field_by_field` | an eligible contribution written past the checks |
//! | `a_personal_claim_has_no_constructor_outside_this_crate` | a personal claim minted beside the promotion that derived it |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two private-field
//! constructions produced **no** diagnostic for one of them, because the `E0560`
//! for an unknown field suppressed the `E0451` for the private ones. A bundled
//! case can hide one of its own halves.

/// The section 17.6 separations that are types.
#[test]
fn a_personal_claim_cannot_be_reached_without_its_evidence() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
