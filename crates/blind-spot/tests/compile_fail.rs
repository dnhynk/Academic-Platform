//! Section 23's limits, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to turn a coverage reading into a
//! score, to decide for the user something section 23 says the user decides, or
//! to make a blind spot ask for something — and every one fails to compile. The
//! suite passes only when each case fails **and** fails with the committed
//! diagnostic, so a case that stopped proving anything — because a type grew a
//! constructor, or because the case itself was mistyped into a different error —
//! is a failure rather than a silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `a_coverage_reading_is_not_ordered` | two fields' coverage ranked |
//! | `a_diversity_is_not_ordered` | two fields' diversity ranked |
//! | `a_scope_has_no_default` | a scope nobody chose |
//! | `a_neutral_presentation_has_no_action` | an action read off a neutral finding |
//! | `a_taste_path_is_not_a_list` | a taste path of two steps |
//! | `a_disposition_has_no_unchecked_constructor` | a disposition minted past ADR-003 |
//! | `a_ledger_is_not_edited_in_place` | a standing choice edited where it stands |
//! | `a_below_minimum_has_no_public_field` | an `UNOBSERVED` basis for adequate coverage |
//! | `a_low_recency_has_no_public_field` | a `STALE` basis carrying a fresh band |
//! | `a_finding_has_no_setter` | a classification overwritten in place |
//! | `an_exposure_item_takes_only_admitted_evidence` | inadmissible evidence counted as exposure |
//!
//! One case per shape. `P2-R4` measured why: bundling two private-field
//! constructions into one case produced **no** diagnostic for one of them,
//! because an `E0560` for an unknown field suppressed the `E0451` for the
//! private ones.

/// The section 23 limits that are types.
#[test]
fn a_blind_spot_cannot_become_a_demand() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
