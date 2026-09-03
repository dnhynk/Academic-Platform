//! An optimistic value cannot be read back out of its seal.
//!
//! Every spelling of "give me the value" is attempted here. If any one of them
//! compiled, the surface could render an unaccepted edit as canonical state and
//! the seal would be decorative.

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

    let _by_into_inner: u32 = update.into_inner();
    let _by_value: u32 = update.value();
    let _by_get: u32 = update.get();
    let _by_field: u32 = update.value;
}
