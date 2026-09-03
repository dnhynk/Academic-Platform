//! A non-mapped status carries its evidence as a field of the variant, so a
//! redaction with no policy reference is not a value.

use academic_lecture_document::SegmentStatus;

fn main() {
    let _ = SegmentStatus::RedactedWithPolicy {};
}
