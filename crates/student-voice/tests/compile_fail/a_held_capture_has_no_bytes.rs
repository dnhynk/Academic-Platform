//! The hold is the absence of a method.
//!
//! A capture that has been screened holds its bytes privately and offers a
//! digest and a length. There is nothing to hand an OCR pass, so a downstream
//! job that reached past the hold is a program that does not compile rather
//! than one that reads a flag and behaves.

use academic_capture::CaptureBytes;
use academic_student_voice::CaptureUnderReview;

fn main() {
    let capture = CaptureUnderReview::screened(CaptureBytes::of(vec![1, 2, 3]), Vec::new());
    let _bytes = capture.bytes();
}
