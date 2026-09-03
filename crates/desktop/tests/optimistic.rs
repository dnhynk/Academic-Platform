//! `optimistic_update_is_not_canonical_before_receipt`, the runtime half.
//!
//! The type half is `tests/compile_fail/`: the seal is enforced by there being
//! no accessor, conversion or `Serialize` to call. What is left to observe at
//! runtime is that the one exit compares every bound field, and that `Debug`
//! does not print the value the seal exists to keep.

use academic_desktop::{DesktopCommand, NotCanonical, Optimistic, SubmittedRequest};
use academic_rpc::generated::{AcceptanceRange, ImmutableReceipt};

fn bytes<const N: usize>(start: u8) -> [u8; N] {
    let mut out = [0_u8; N];
    let mut index = 0_usize;
    while index < N {
        out[index] = start.wrapping_add(index as u8);
        index += 1;
    }
    out
}

fn request() -> SubmittedRequest {
    SubmittedRequest {
        request_id: bytes(0),
        client_instance_id: bytes(16),
        idempotency_key: bytes(32),
        request_digest: bytes(64),
    }
}

fn receipt_for(request: &SubmittedRequest) -> ImmutableReceipt {
    ImmutableReceipt {
        receipt_id: bytes::<16>(96).to_vec(),
        request_id: request.request_id.to_vec(),
        client_instance_id: request.client_instance_id.to_vec(),
        idempotency_key: request.idempotency_key.to_vec(),
        request_digest: request.request_digest.to_vec(),
        profile_revision: u64::MAX,
        acceptance_range: Some(AcceptanceRange {
            accept_seq_start: 1,
            accept_seq_end: 2,
        }),
    }
}

/// A matching receipt is the promotion, and it yields the value.
#[test]
fn a_matching_receipt_promotes() {
    let submitted = request();
    let update = Optimistic::new(DesktopCommand::SyntheticBackup, submitted.clone());
    assert_eq!(update.request(), &submitted);

    let canonical = update.confirm(&receipt_for(&submitted));
    let canonical = match canonical {
        Ok(value) => value,
        Err(error) => unreachable!("a matching receipt was refused: {error}"),
    };
    assert_eq!(canonical.value(), &DesktopCommand::SyntheticBackup);
    assert_eq!(canonical.receipt().profile_revision, u64::MAX);
    assert_eq!(canonical.into_value(), DesktopCommand::SyntheticBackup);
}

/// Every bound field is compared. Matching three of four promotes nothing.
#[test]
fn a_receipt_that_differs_in_any_bound_field_promotes_nothing() {
    let submitted = request();
    type Mutation = fn(&mut ImmutableReceipt);
    let mutations: [(Mutation, NotCanonical); 4] = [
        (
            |receipt| receipt.request_id = vec![0xff; 16],
            NotCanonical::RequestIdMismatch,
        ),
        (
            |receipt| receipt.client_instance_id = vec![0xff; 16],
            NotCanonical::ClientInstanceIdMismatch,
        ),
        (
            |receipt| receipt.idempotency_key = vec![0xff; 32],
            NotCanonical::IdempotencyKeyMismatch,
        ),
        (
            |receipt| receipt.request_digest = vec![0xff; 32],
            NotCanonical::RequestDigestMismatch,
        ),
    ];
    for (mutate, expected) in mutations {
        let mut receipt = receipt_for(&submitted);
        mutate(&mut receipt);
        let update = Optimistic::new(7_u32, submitted.clone());
        assert_eq!(update.confirm(&receipt), Err(expected));
    }

    // A receipt whose field is merely the wrong length is a mismatch too, so a
    // truncated or padded value cannot compare equal to a fixed-size array.
    for shorter in [15_usize, 17] {
        let mut receipt = receipt_for(&submitted);
        receipt.request_id = vec![0_u8; shorter];
        let update = Optimistic::new(7_u32, submitted.clone());
        assert_eq!(
            update.confirm(&receipt),
            Err(NotCanonical::RequestIdMismatch)
        );
    }

    // And the unmutated receipt still promotes, so the refusals above are the
    // mutations rather than a `confirm` that refuses everything.
    let update = Optimistic::new(7_u32, submitted.clone());
    assert!(update.confirm(&receipt_for(&submitted)).is_ok());
}

/// `Debug` redacts the unaccepted value.
#[test]
fn debug_does_not_print_the_unaccepted_value() {
    let update = Optimistic::new("the user renamed this course", request());
    let rendered = format!("{update:?}");
    assert!(
        !rendered.contains("the user renamed this course"),
        "the optimistic value reached a log line: {rendered}"
    );
    assert!(rendered.contains("<unaccepted>"), "{rendered}");

    // The canonical value is not redacted; the seal is on the pending state.
    let submitted = request();
    let confirmed =
        Optimistic::new("accepted text", submitted.clone()).confirm(&receipt_for(&submitted));
    let confirmed = match confirmed {
        Ok(value) => value,
        Err(error) => unreachable!("a matching receipt was refused: {error}"),
    };
    assert!(format!("{confirmed:?}").contains("accepted text"));
}
