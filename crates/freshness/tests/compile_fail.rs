//! Section 13.3's limits, held by compilation rather than by a check.
//!
//! Every case here is a program that tries to move a band without the input
//! section 13.3 requires for it, or to move something section 13.3 says a band
//! may not touch, and every one fails to compile. The suite passes only when
//! each case fails **and** fails with the committed diagnostic, so a case that
//! stopped proving anything — because a type grew a constructor, or because the
//! case itself was mistyped into a different error — is a failure rather than a
//! silent pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `decay_cannot_take_a_mastery` | time decay applied to a mastery level |
//! | `a_spillover_is_not_a_neighbour_use` | a received contribution passed on |
//! | `a_projection_cannot_be_dated_evidence` | a band offered as a neighbour's use |
//! | `a_personalization_speed_has_no_default` | personalization with nobody's decision |
//! | `a_recall_statement_has_no_unchecked_constructor` | a recall confirmation minted past ADR-003 |
//! | `a_projection_has_no_setter` | a band edited where it stands |
//! | `calibration_reads_only_the_users_own_record` | a projected band used as recall data |
//! | `a_cited_edge_has_no_unchecked_constructor` | a spillover edge assembled past its allowlist |
//! | `blocked_evidence_cannot_be_dated` | ineligible evidence freshening a concept |
//!
//! One case per shape. `P2-R4` measured why: bundling two private-field
//! constructions into one case produced **no** diagnostic for one of them,
//! because an `E0560` for an unknown field suppressed the `E0451` for the
//! private ones.

/// The section 13.3 limits that are types.
#[test]
fn a_band_cannot_move_without_its_input() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
