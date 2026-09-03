//! `CONTRIBUTING.md`'s third rule, as three types.
//!
//! Origin order, the retrieval clock, and valid time are three different
//! things, and nothing converts one into another. An ingest position handed to
//! something expecting a wall-clock reading is a type error, not a subtle bug
//! in a report six months later.

use academic_ingestion::{IngestSeq, NextVerification};

fn main() {
    let order = IngestSeq::at(7);
    let _due = NextVerification::due_at(order);
}
