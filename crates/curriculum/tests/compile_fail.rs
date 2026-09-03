//! The type half of the three section 9 boundaries, proved by compilation.
//!
//! Section 9's table says what each aggregate does not contain, and a running
//! test cannot observe an absence: there is no value to construct and no call
//! to make. The cases under `tests/compile_fail` are those calls, written as
//! programs that do not exist to run.
//!
//! Each one sets, or reads, a field the specification puts on a *different*
//! aggregate. They fail with `E0599` — "no method named … found" — which is the
//! diagnostic that says the field is absent rather than private, hidden behind
//! a feature, or refused at run time. The suite passes only when each case
//! fails to compile *and* fails with the committed diagnostic, so a case that
//! stopped proving anything — because a setter was added, or because the case
//! itself was mistyped into a different error — is a failure rather than a
//! silent pass.
//!
//! The list of names each case tries is not the evidence on its own: a case
//! that quietly dropped one would still fail to compile on the others.
//! `the_forbidden_fields_are_the_specifications_own` in
//! `tests/curriculum_scans.rs` is what holds the list, by reading section 8.2's
//! own blocks and section 9's own table out of the authoritative specification
//! and requiring every name on them to be absent from the aggregate that must
//! not have it, and present on the one that must.

/// `course_boundary_rejects_offering_fields`,
/// `revision_boundary_rejects_section_fields` and
/// `offering_boundary_rejects_session_transcript`, plus the two relation
/// boundaries, all in one `trybuild` pass.
#[test]
fn aggregate_boundaries_are_compile_errors() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
