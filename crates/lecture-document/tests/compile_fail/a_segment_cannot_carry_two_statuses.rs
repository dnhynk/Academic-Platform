//! One segment, one status. `SegmentStatus` is an enum and a `SegmentAccount`
//! has one field of it, so a value naming two variants at once does not exist.

use academic_lecture_document::SegmentStatus;

fn main() {
    let _ = SegmentStatus::Mapped {
        nodes: Vec::new(),
        evidence: (),
    };
}
