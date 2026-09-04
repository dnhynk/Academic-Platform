//! The one type with a byte accessor has no public constructor.
//!
//! `ReviewedCapture` is built in exactly one place, inside `dispatch`, after
//! the hold state admitted. Holding one is proof the hold was passed; writing
//! one is not a thing a caller can do.

use academic_capture::CaptureBytes;
use academic_domain::ContentDigest;
use academic_student_voice::{IngestionJobKind, ReviewedCapture};

fn main() {
    let bytes = CaptureBytes::of(vec![1, 2, 3]);
    let _admitted = ReviewedCapture {
        digest: ContentDigest::sha256(b"anything"),
        kind: IngestionJobKind::OcrIngestion,
        bytes: &bytes,
    };
}
