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

fn pending() -> Optimistic<u32> {
    Optimistic::new(7, submitted())
}

fn main() {
    let update = pending();

    let _by_into: u32 = update.into();
    let _by_as_ref: &u32 = pending().as_ref();
    let _by_borrow: &u32 = Borrow::borrow(&pending());
    let _by_deref: u32 = *pending();
}
