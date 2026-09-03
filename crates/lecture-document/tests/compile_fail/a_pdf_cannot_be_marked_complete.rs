//! `PdfArtifact` has no setter for its completeness and no constructor that
//! takes one. `PdfArtifact::render` is the only producer and it writes
//! `INCOMPLETE` unless it holds a witness.

use academic_lecture_document::{DocumentCompleteness, PdfArtifact};

fn mark(pdf: &mut PdfArtifact) {
    pdf.set_completeness(DocumentCompleteness::Complete);
}

fn main() {}
