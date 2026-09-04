//! Section 24.1's separations, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to state a competency without the
//! parts section 24.1 requires, or to move a value across a boundary this task
//! exists to keep, and every one of them fails to compile. The suite passes
//! only when each case fails **and** fails with the committed diagnostic, so a
//! case that stopped proving anything — because a type grew a constructor, or
//! because the case itself was mistyped into a different error — is a failure
//! rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_competency_statement_has_no_constructor` | a sentence handed to `declare` beside the parts |
//! | `a_concept_reference_is_not_a_competency_identity` | a concept turned into a competency |
//! | `a_competency_identity_is_not_a_concept_reference` | a competency read as one of its own concepts |
//! | `a_criterion_cannot_be_assembled_field_by_field` | a criterion that names no concept |
//! | `a_project_claim_founds_no_stage_evidence` | `ProjectSnapshot OBSERVES Concept` at the personal door |
//! | `stage_evidence_cannot_be_assembled_field_by_field` | evidence filed under a concept that did not found it |
//! | `promoting_evidence_has_no_constructor_beside_of` | section 13.2's ceiling left unasked |
//! | `a_rubric_sheet_cannot_be_deserialized` | a filled cell read back out of a document |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two private-field
//! constructions produced **no** diagnostic for one of them, because the `E0560`
//! for an unknown field suppressed the `E0451` for the private ones. A bundled
//! case can hide one of its own halves.

/// The section 24.1 and section 24.3 separations that are types.
#[test]
fn a_competency_cannot_be_stated_without_its_parts() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
