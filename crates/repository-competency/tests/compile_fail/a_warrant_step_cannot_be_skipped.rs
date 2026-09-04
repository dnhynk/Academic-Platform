//! Section 17.6's fifth bullet is three verbs in an order, and each is the
//! next type's argument taken by value.
//!
//! Explaining code the user verified and never modified is exactly the shape
//! `unmodified_generated_code_creates_no_applied_claim` is named for, and it is
//! a program that does not compile: `ExplainedByUser::after` takes a
//! `ModifiedByUser` and a `VerifiedByUser` is not one.

use academic_repository_competency::{ExplainedByUser, GeneratedCodeWarrant, VerifiedByUser};

fn main() {
    let verified: VerifiedByUser = unimplemented!();
    let explained = ExplainedByUser::after(verified, "explained but never modified");
    let _warrant = GeneratedCodeWarrant::sealed(explained);
}
