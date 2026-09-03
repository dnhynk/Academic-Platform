//! The type half of `P2-U6`, proved by compilation.
//!
//! `tests/ingestion.rs` observes what the pipeline does. What it cannot observe
//! is a caller that publishes an undated document, skips a stage, ranks two
//! source categories, or builds a fetch target out of a page it just read —
//! because those programs do not exist to run.
//!
//! The eight cases under `tests/compile_fail` are those programs. Two of them
//! carry this task's most load-bearing claims:
//! `an_unscoped_source_cannot_publish` is the type-level half of
//! `unscoped_official_source_cannot_publish`, and `a_stage_cannot_be_skipped`
//! is the type-level half of `ingestion_stage_order_is_strict`.
//!
//! The harness is `academic-proposal`'s, unchanged: `trybuild::TestCases`
//! against `tests/compile_fail/*.rs` with the diagnostic committed beside each
//! case. The suite passes only when each program fails to compile *and* fails
//! with the committed diagnostic, so a case that stopped proving anything —
//! because a `From` was added, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.

/// The type-level half of `unscoped_official_source_cannot_publish`.
#[test]
fn unscoped_official_source_cannot_publish() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
