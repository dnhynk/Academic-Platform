//! The type half of the `P2-L4` boundary, proved by compilation.
//!
//! `tests/lecture_document.rs` observes what the validator does and
//! `tests/lecture_document_scans.rs` observes what its source does not contain.
//! Neither can observe a caller that declares a segment `MAPPED`, marks a
//! rendering `COMPLETE`, or builds a redaction with no policy, because those
//! programs do not exist to run.
//!
//! The cases under `tests/compile_fail` are those programs. The harness is
//! `P2-L3`'s, unchanged: `trybuild::TestCases` against
//! `tests/compile_fail/*.rs` with the diagnostic committed beside each case, so
//! the suite passes only when each program fails to compile **and** fails with
//! the committed diagnostic.
//!
//! Each case is written beside the variant that **does** compile, in
//! `docs/contracts/lecture-document.md`, because a program that fails for a
//! typo proves nothing about the rule it was aimed at.

/// `segment_status_exhaustive`, `unmapped_forces_incomplete` and
/// `study_index_disclosure`, the type halves.
#[test]
fn the_status_the_witness_and_the_disclosure_are_closed() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
