//! The type half of the `P2-L3` boundary, proved by compilation.
//!
//! `tests/transcription.rs` observes what the pipeline does and
//! `tests/transcription_scans.rs` observes what its source does not contain.
//! What neither can observe is a caller that writes a raw token, reads a
//! provider response's bytes, or ranks two providers, because those programs do
//! not exist to run.
//!
//! The cases under `tests/compile_fail` are those programs. The harness is
//! `academic-proposal`'s, unchanged: `trybuild::TestCases` against
//! `tests/compile_fail/*.rs` with the diagnostic committed beside each case, so
//! the suite passes only when each program fails to compile **and** fails with
//! the committed diagnostic. A case that stopped proving anything -- because a
//! `From` was added, or because the case itself was mistyped into a different
//! error -- is a failure rather than a silent pass.

/// `raw_token_write_protection`, the type half.
#[test]
fn raw_tokens_and_provider_responses_are_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
