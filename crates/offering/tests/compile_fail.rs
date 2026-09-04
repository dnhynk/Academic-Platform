//! The type half of the promotion prohibition, proved by compilation.
//!
//! Section 8.3 forbids promoting a prediction into an official fact and forbids
//! a `HISTORICALLY_LIKELY` offering entering a confirmed graduation plan. A
//! running test cannot observe either absence: there is no value to construct
//! and no call to make. The cases under `tests/compile_fail` are those calls,
//! written as programs that do not exist to run.
//!
//! Each one is a different route somebody could take. Four try to reach a
//! `ConfirmedSeat` -- through the likely standing, through a struct literal,
//! through a `Default`, and through the plan itself. One tries to make a
//! forecast into confirmation evidence, one tries to assemble the scored
//! forecast a likely standing needs without going through the calibration
//! registry, and one tries to leave a plan refusal empty. The suite passes only when
//! each case fails to compile *and* fails with the committed diagnostic, so a
//! case that stopped proving anything -- because a constructor was added, or
//! because the case was mistyped into a different error -- is a failure rather
//! than a silent pass.
//!
//! The list of routes is not the evidence on its own: a case that quietly
//! dropped one would still fail on the others.
//! `no_product_file_promotes_a_prediction` in `tests/offering_scans.rs` is what
//! holds the list, by sweeping every signature and every `impl` header in the
//! crate as whole sets rather than by naming the routes anybody thought of.
//!
//! # One case per privacy error
//!
//! Rust runs the privacy pass **after** type checking, so a file whose type
//! checking already failed never reaches it and an `E0451` the case exists for
//! is never emitted. A case that bundled a struct literal with a wrong-arity
//! call would therefore still fail to compile, still pass the suite, and prove
//! only the arity. The two literal cases here hold nothing but the literal for
//! that reason, and the committed `.stderr` beside each is what says the
//! diagnostic is the one intended.

/// All seven routes, in one `trybuild` pass.
#[test]
fn a_prediction_cannot_reach_a_confirmation() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
