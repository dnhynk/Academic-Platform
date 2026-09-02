//! The type half of two refusals, proved by compilation.
//!
//! Section 3.7 says a self-assessment cannot produce `PERMITTED`, and section
//! 12.1 says the same of a user's belief that personal use makes a recording
//! acceptable. `oral_attestation_cannot_create_permission` and
//! `personal_use_text_cannot_create_permission` in `tests/consent.rs` observe
//! that filing either one moves no status; what they cannot observe is a caller
//! who tries to pass an attestation where a written authority belongs, because
//! that program does not exist to run.
//!
//! The two cases under `tests/compile_fail` are those programs. The suite
//! passes only when each fails to compile *and* fails with the committed
//! diagnostic, so a case that stopped proving anything -- because a `From` was
//! added, or because the case itself was mistyped into a different error -- is
//! a failure rather than a silent pass.

/// `attestation_cannot_create_a_permission`.
#[test]
fn attestation_cannot_create_a_permission() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
