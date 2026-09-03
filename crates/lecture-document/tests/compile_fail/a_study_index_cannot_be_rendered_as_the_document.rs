//! A summary is a separate artifact. `PdfArtifact::render` takes a
//! `LectureDocument`, and a `StudyIndex` is not one -- there is no conversion
//! in either direction.

use academic_domain::ContentDigest;
use academic_lecture_document::{CoverageReport, PdfArtifact, RenderQaReport, StudyIndex};

fn render(index: &StudyIndex, report: &CoverageReport, qa: &RenderQaReport) -> PdfArtifact {
    PdfArtifact::render(index, report, qa, ContentDigest::sha256(b"pdf"))
}

fn main() {}
