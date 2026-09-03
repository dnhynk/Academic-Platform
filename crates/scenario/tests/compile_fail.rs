//! The type half of several isolations, proved by compilation.
//!
//! The cases under `tests/compile_fail` are of four kinds. Most are programs
//! that try to carry a projected mastery, opportunity, or workload value to a
//! canonical writer. One — `admitted_posture_requires_verified_receipt` —
//! assembles an admitted `Posture` with a struct literal instead of the
//! `VerifiedAdmission` that `AdmissionVerifier::verify` issues. Two are
//! `P2-R2`'s: a finding assembled field by field, and a finding scope naming
//! the repository. The rest are `P2-R4`'s section 18 proofs — a chain step
//! skipped, a proof chain or a benefit contract assembled without its parts, a
//! `SUFFICIENT` user-evidence value, and a stance carrying both outlooks.
//!
//! The suite passes only when each one fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything — because the
//! seal grew an accessor, or because the case itself was mistyped into a
//! different error — is a failure rather than a silent pass.
//!
//! One case per shape, and measured: bundling
//! `a_concept_stance_cannot_be_assembled`'s private-field construction into
//! `required_and_benefit_cannot_share_one_scope` produced **no** diagnostic for
//! it, because the `E0560` for the unknown field suppressed the `E0451` for the
//! private ones. A bundled case can hide one of its own halves.

/// `projected_type_cannot_call_actual_writer`.
#[test]
fn projected_type_cannot_call_actual_writer() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
