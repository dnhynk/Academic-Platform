//! `P2-L5`'s refusals that are compilation rather than checks.
//!
//! Every case here is a program that tries to reach an editing claim, a
//! capture's bytes, or a wider retention without what this task requires for
//! it, and every one fails to compile. The suite passes only when each case
//! fails **and** fails with the committed diagnostic, so a case that stopped
//! proving anything -- because a type grew a constructor, or because the case
//! was mistyped into a different error -- is a failure rather than a silent
//! pass.
//!
//! | Case | What has no representation |
//! |---|---|
//! | `an_automatic_redaction_needs_a_witness` | an automatic redaction mode with the measurement left out |
//! | `an_accuracy_witness_cannot_be_forged` | a witness from a `Default`, a struct literal or a `new` |
//! | `a_redaction_cannot_reach_an_original` | a policy scope that edits the original recording |
//! | `a_held_capture_has_no_bytes` | the content of a capture that has not been reviewed |
//! | `a_reviewed_capture_cannot_be_assembled` | an admitted capture written past the hold that would have produced it |
//! | `an_access_grant_is_spent_by_being_used` | a second read on one authorization |
//! | `a_derivative_cannot_be_given_wider_terms` | a retention pair written onto a derivative after the fact |
//!
//! One case per shape, for `P2-R4`'s measured reason: bundling two private-field
//! constructions produced **no** diagnostic for one of them, because the `E0560`
//! for an unknown field suppressed the `E0451` for the private ones. A bundled
//! case can hide one of its own halves.

/// The `P2-L5` separations that are types.
#[test]
fn an_editing_claim_cannot_be_reached_without_its_measurement() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
