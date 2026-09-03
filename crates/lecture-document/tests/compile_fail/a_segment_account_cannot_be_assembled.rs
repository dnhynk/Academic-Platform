//! Every field of a `SegmentAccount` is private and its one producer is the
//! validator, so an account that exists was measured.
//!
//! The status value below **does** construct: an enum's variant fields are as
//! public as the enum, and Rust has no way to make them otherwise. That costs
//! nothing, because a bare `SegmentStatus` is a value with nowhere to go — the
//! account that would carry it is closed, and `SegmentDisposition` has no
//! constructor that takes one.

use academic_lecture_document::{SegmentAccount, SegmentStatus};

fn main() {
    let status = SegmentStatus::Mapped { nodes: Vec::new() };
    let _ = SegmentAccount {
        segment_index: 0,
        segment_id: String::new(),
        token_count: 0,
        status,
    };
}
