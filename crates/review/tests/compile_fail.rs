//! The type half of `P2-U8`, proved by compilation.
//!
//! `tests/reviews.rs` observes what the pipeline does and `tests/review_scans.rs`
//! observes what is absent from the source. What neither can observe is a
//! caller that promotes a course without an aggregation claim, scopes a review
//! to a course, assembles a permitted collection for a source nobody reviewed,
//! or reads somebody else's writing out of a record -- because those programs
//! do not exist to run.
//!
//! The five cases under `tests/compile_fail` are those programs.
//! `a_course_promotion_needs_a_claim` and
//! `an_aggregation_claim_cannot_be_assembled` are the type-level half of
//! `course_promotion_requires_explicit_aggregation`;
//! `a_review_scope_has_no_course` is the type-level half of
//! `review_default_scope_is_offering_instructor_term_source`; and
//! `retained_text_has_no_public_reader` is the type-level half of
//! `raw_review_text_is_excluded_from_export_and_share`.
//!
//! The harness is `academic-ingestion`'s, unchanged: `trybuild::TestCases`
//! against `tests/compile_fail/*.rs` with the diagnostic committed beside each
//! case. The suite passes only when each program fails to compile *and* fails
//! with the committed diagnostic, so a case that stopped proving anything --
//! because a `From` was added, or because the case itself was mistyped into a
//! different error -- is a failure rather than a silent pass.

/// The type-level half of `course_promotion_requires_explicit_aggregation`.
#[test]
fn course_promotion_requires_explicit_aggregation() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
