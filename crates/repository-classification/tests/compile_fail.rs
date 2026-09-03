//! Section 18's proofs, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to reach a section 18 classification
//! without the evidence section 18 requires for it, and every one of them fails
//! to compile. The suite passes only when each case fails **and** fails with the
//! committed diagnostic, so a case that stopped proving anything — because a
//! type grew a constructor, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_chain_step_cannot_be_skipped` | any of section 18.2's five links, built without its predecessor |
//! | `proof_chain_cannot_be_assembled_field_by_field` | a `ProofChain` from a `Default`, a struct literal or a `new` |
//! | `a_required_concept_has_no_unchecked_constructor` | a required concept built past `realizing`'s tier refusal |
//! | `user_evidence_gap_has_no_sufficient_value` | a fifth step meaning *the user already knows it* |
//! | `required_and_benefit_cannot_share_one_scope` | an outlook holding both, or a stance with a field for each |
//! | `a_concept_stance_cannot_be_assembled` | a stance built outside the classifier that derives it |
//! | `a_benefit_contract_has_no_partial_constructor` | a section 18.3 contract from the concept alone |
//!
//! One case per shape, and measured: bundling
//! `a_concept_stance_cannot_be_assembled`'s private-field construction into
//! `required_and_benefit_cannot_share_one_scope` produced **no** diagnostic for
//! it, because the `E0560` for the unknown field suppressed the `E0451` for the
//! private ones. A bundled case can hide one of its own halves.
//!
//! The cases live here rather than in `academic-scenario`'s suite, where
//! `P2-R2`'s went, because this crate builds on `P2-R3` and therefore reaches
//! `academic-ledger`, which `scenario_crate_has_no_writer_dependency` refuses
//! `academic-scenario` — through a dev edge as much as a product one.

/// The section 18 proofs that are types.
#[test]
fn a_classification_cannot_be_reached_without_its_proof() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
