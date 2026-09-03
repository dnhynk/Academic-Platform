//! The PDF, which is a rendering and not the truth.
//!
//! # Incomplete is what a PDF is
//!
//! [`PdfArtifact::render`] writes [`DocumentCompleteness::Incomplete`] first
//! and replaces it only when it holds a [`CompletenessWitness`], whose one
//! producer is `CoverageReport::completeness_witness` and whose fields are
//! private. There is no constructor that takes a completeness, no setter, and
//! no other function in the workspace that returns a witness. So "the default
//! is incomplete" is not a default value somebody could change: it is the only
//! value there is a route to without a measurement.
//!
//! # The PDF is a sink
//!
//! Nothing in this crate takes a [`PdfArtifact`] and returns a
//! [`LectureDocument`], a `CoverageReport`, or anything derived from one.
//! `pdf_non_authority` holds that as a whole-set rule over every public
//! signature in `crates/`, rather than as a list of function names nobody may
//! write: a route from the rendering back to the record is what "the PDF is the
//! source of truth" would look like, and there is none.

use academic_domain::ContentDigest;

use crate::{
    coverage::{CompletenessWitness, CoverageReport},
    document::{DocumentId, LectureDocument, be_len, push_str},
    render::RenderQaReport,
};

/// What a rendering is allowed to say about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocumentCompleteness {
    /// Something is not accounted for. The count is section 34.1's banner.
    Incomplete {
        /// How many eligible segments have no status.
        unmapped_segments: usize,
        /// How many render defects were found.
        render_defects: usize,
    },
    /// Every eligible segment has one of the four statuses and the render is
    /// clean.
    Complete,
}

impl DocumentCompleteness {
    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incomplete { .. } => "INCOMPLETE",
            Self::Complete => "COMPLETE",
        }
    }

    /// Whether a "complete" badge may be shown.
    #[must_use]
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// One preservation rendering of one lecture document.
///
/// It holds no page and no byte of a PDF: what it records is which document was
/// rendered, the digest of the bytes a renderer produced, and what the
/// measurement says about them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfArtifact {
    document: DocumentId,
    document_digest: ContentDigest,
    rendered_bytes_digest: ContentDigest,
    completeness: DocumentCompleteness,
}

impl PdfArtifact {
    /// Records one rendering.
    ///
    /// The completeness starts at [`DocumentCompleteness::Incomplete`] and is
    /// replaced only by a witness. A report for another document, or a render
    /// QA run over another document, cannot upgrade it: both digests are
    /// compared against the document's own.
    #[must_use]
    pub fn render(
        document: &LectureDocument,
        report: &CoverageReport,
        qa: &RenderQaReport,
        rendered_bytes_digest: ContentDigest,
    ) -> Self {
        let document_digest = document.digest();
        let mut completeness = DocumentCompleteness::Incomplete {
            unmapped_segments: report.unmapped_count(),
            render_defects: qa.findings().len(),
        };
        if report.document_digest() == &document_digest
            && qa.document_digest() == &document_digest
            && qa.is_clean()
            && let Some(witness) = report.completeness_witness()
        {
            completeness = Self::upgrade(&document_digest, report, witness);
        }
        Self {
            document: document.id().clone(),
            document_digest,
            rendered_bytes_digest,
            completeness,
        }
    }

    /// The one place a completeness stops being `Incomplete`.
    ///
    /// It takes the witness by value, so there is no path here that does not
    /// hold one.
    fn upgrade(
        document_digest: &ContentDigest,
        report: &CoverageReport,
        witness: CompletenessWitness,
    ) -> DocumentCompleteness {
        if witness.report_digest() == &report.digest()
            && report.document_digest() == document_digest
        {
            DocumentCompleteness::Complete
        } else {
            DocumentCompleteness::Incomplete {
                unmapped_segments: report.unmapped_count(),
                render_defects: 0,
            }
        }
    }

    /// Which document.
    #[must_use]
    pub const fn document(&self) -> &DocumentId {
        &self.document
    }

    /// The digest of that document.
    #[must_use]
    pub const fn document_digest(&self) -> &ContentDigest {
        &self.document_digest
    }

    /// The digest of the bytes the renderer produced.
    #[must_use]
    pub const fn rendered_bytes_digest(&self) -> &ContentDigest {
        &self.rendered_bytes_digest
    }

    /// What the rendering may say about itself.
    #[must_use]
    pub const fn completeness(&self) -> DocumentCompleteness {
        self.completeness
    }

    /// The artifact's canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-lecture-pdf-v1\0");
        push_str(&mut material, self.document.as_str());
        material.extend_from_slice(self.document_digest.to_string().as_bytes());
        material.extend_from_slice(self.rendered_bytes_digest.to_string().as_bytes());
        push_str(&mut material, self.completeness.as_str());
        match self.completeness {
            DocumentCompleteness::Incomplete {
                unmapped_segments,
                render_defects,
            } => {
                material.extend_from_slice(&be_len(unmapped_segments));
                material.extend_from_slice(&be_len(render_defects));
            }
            DocumentCompleteness::Complete => {}
        }
        material
    }
}
