//! An optimistic value has no conversion out of its seal.
//!
//! `Into`, `AsRef`, `Borrow` and `Deref` are each a way to obtain the inner
//! value under another name. None of them exists.

use std::borrow::Borrow;

use academic_desktop::{Optimistic, SubmittedRequest};

fn submitted() -> SubmittedRequest {
    SubmittedRequest {
        request_id: [0; 16],
        client_instance_id: [1; 16],
        idempotency_key: [2; 32],
        request_digest: [3; 32],
    }
}

// A local type keeps diagnostics independent of optional crates implementing From<u32>.
struct Value;

fn pending() -> Optimistic<Value> {
    Optimistic::new(Value, submitted())
}

fn main() {
    let update = pending();

    let _by_into: Value = update.into();
    let _by_as_ref: &Value = pending().as_ref();
    let _by_borrow: &Value = Borrow::borrow(&pending());
    let _by_deref: Value = *pending();
}
