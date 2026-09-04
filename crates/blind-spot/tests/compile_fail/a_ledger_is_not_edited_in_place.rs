//! Section 39: `NOT_RELEVANT는 경고와 추천에서 제외한다`.
//!
//! Every operation that changes a `DispositionLedger` consumes it and returns a
//! new one, so a recomputation cannot edit the ledger it was handed.

use academic_blind_spot::{DispositionLedger, UserDispositionChoice};

fn edit(ledger: &mut DispositionLedger, choice: UserDispositionChoice) {
    ledger.record(choice);
}

fn main() {}
