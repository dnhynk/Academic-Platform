//! Section 13's promotions, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to reach a knowledge state without
//! the evidence section 13 requires for it, and every one fails to compile. The
//! suite passes only when each case fails **and** fails with the committed
//! diagnostic, so a case that stopped proving anything — because a type grew a
//! constructor, or because the case itself was mistyped into a different error
//! — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `an_automatic_projection_cannot_name_fluent` | a sixth `AutomaticLevel` |
//! | `an_assertion_has_no_setter` | an assertion edited where it stands |
//! | `a_grade_is_not_concept_evidence` | a course grade attributed to a concept |
//! | `evidence_sufficiency_is_not_a_skill_score` | sufficiency ranked, or read as a level |
//! | `a_user_confirmation_has_no_unchecked_constructor` | a confirmation assembled past `verify` |
//! | `a_fluent_authorization_is_consumed` | one authorization applied twice |
//! | `ineligible_evidence_cannot_be_projected` | raw evidence handed to `project` |
//! | `a_history_is_not_mutated_in_place` | a history read after it was retracted from |
//!
//! One case per shape. `P2-R4` measured why: bundling two private-field
//! constructions into one case produced **no** diagnostic for one of them,
//! because an `E0560` for an unknown field suppressed the `E0451` for the
//! private ones. A bundled case can hide one of its own halves.
//!
//! The cases live here rather than in `academic-scenario`'s suite because this
//! crate reaches `academic-ledger`, which `scenario_crate_has_no_writer_dependency`
//! refuses `academic-scenario` through a dev edge as much as a product one.

/// The section 13 promotions that are types.
#[test]
fn a_state_cannot_be_reached_without_its_proof() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
