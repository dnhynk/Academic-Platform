//! `a_snapshot_needs_a_confirmation_of_this_scope`.
//!
//! Section 33's IDE row says the changed scope is confirmed before a snapshot.
//! A confirmation carries the digest of the scope it was recorded for, and the
//! routes that would let one exist without a scope are tried here.

use academic_domain::TimestampMillis;
use academic_integrations::{ChangedScope, ScopeConfirmation, WorkspacePath};

fn main() {
    let path = WorkspacePath::new("src/lib.rs").unwrap();
    let scope = ChangedScope::new(TimestampMillis::new(0), vec![path]);

    // The fields are private, so a confirmation cannot be written as a literal
    // for a digest nobody computed.
    let _literal = ScopeConfirmation {
        scope_digest: scope.digest(),
        actor_id: String::from("student"),
        at: TimestampMillis::new(1),
    };

    // Nor recorded from a digest directly: `record` takes the scope.
    let _from_digest = ScopeConfirmation::record(scope.digest(), "student", TimestampMillis::new(1));

    // And the digest a confirmation carries is read-only: there is no setter to
    // move it onto a scope the user did not see.
    let mut confirmation = ScopeConfirmation::record(&scope, "student", TimestampMillis::new(1));
    confirmation.scope_digest = scope.digest();
}
