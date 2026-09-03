//! `MAPPED` is derived from the document and a caller cannot declare it.
//!
//! The three declaring constructors exist and are exercised in
//! `segment_status_exhaustive`; there is no fourth. This is `P2-U1`'s "the
//! forbidden field has no setter" applied to a status.

use academic_lecture_document::SegmentDisposition;

fn main() {
    let _ = SegmentDisposition::mapped(0);
}
