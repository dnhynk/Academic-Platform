//! An optimistic value cannot leave as bytes.
//!
//! Serialising the wrapper would put the unaccepted value into an export, a log
//! or an IPC frame, which is the same leak as an accessor with more steps.

use academic_desktop::{Optimistic, SubmittedRequest};

fn submitted() -> SubmittedRequest {
    SubmittedRequest {
        request_id: [0; 16],
        client_instance_id: [1; 16],
        idempotency_key: [2; 32],
        request_digest: [3; 32],
    }
}

fn main() {
    let update: Optimistic<u32> = Optimistic::new(7, submitted());
    let _bytes = serde_json::to_string(&update);
}
