//! The type half of three of this task's claims, proved by compilation.
//!
//! A running test cannot observe that a route does not exist: there is no value
//! to construct and no call to make. The cases under `tests/compile_fail` are
//! those calls, written as programs that do not exist to run.
//!
//! * **An external identifier never becomes a canonical one.** Every route from
//!   text and from an `ExternalId` into a `CanonicalRef` is tried, including the
//!   struct-literal route into the mapping that holds one.
//! * **A snapshot needs the user's confirmation of the exact changed scope.**
//!   `ScopeConfirmation` has private fields and one producer that takes the
//!   scope itself, so a confirmation for a digest nobody computed is not a value
//!   that exists.
//! * **Generated code carries its provenance or it does not exist.**
//!   `GeneratedCode`'s fields are private and its one producer takes a
//!   `P2-M1` `ModelRun` by reference.
//!
//! The suite passes only when each case fails to compile *and* fails with the
//! committed diagnostic, so a case that stopped proving anything -- because a
//! constructor was added, or because the case itself was mistyped into a
//! different error -- is a failure rather than a silent pass.
//!
//! The list of routes each case tries is not the evidence on its own: a case
//! that quietly dropped one would still fail to compile on the others.
//! `external_id_is_never_canonical` and
//! `every_public_signature_is_in_the_inventory` are what hold the lists, by
//! comparing whole sets read out of the source in both directions.

/// The three cases, in one `trybuild` pass.
#[test]
fn the_integration_boundarys_typed_doors_have_no_second_route() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
