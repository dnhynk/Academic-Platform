//! `a_deletion_confirmation_has_no_struct_literal`.
//!
//! `deletion_confirmation_is_non_delegable` drives every automatic actor
//! through `DeletionConfirmation::given` and observes the refusal. That case
//! cannot observe the route that skips `given` altogether: a struct literal
//! naming the fields. This is that route, and it does not exist.

use academic_deletion::DeletionConfirmation;
use academic_domain::TimestampMillis;

fn main() {
    let _forged = DeletionConfirmation {
        preview: unimplemented!(),
        decision: unimplemented!(),
        shown_digest: unimplemented!(),
        confirmed_at: TimestampMillis::new(0),
    };
}
