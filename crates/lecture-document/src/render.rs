//! Section 12.6's render QA: the four defects a preservation rendering can
//! have that the mapping cannot see.
//!
//! # This crate renders nothing
//!
//! There is no PDF engine in this repository, no font, and no layout. What
//! reaches [`RenderQa::inspect`] is a *measurement* a renderer took — how tall
//! a page's content is against its frame, whether a code box was clipped,
//! whether an image resolved, how many glyphs came back missing — and every
//! measurement in this crate's test tree is a committed literal.
//!
//! Saying that plainly is the honest half. What this module adds is that the
//! four defects are a closed set compared against section 12.6's own sentence,
//! that every node of the document has to be measured before a report exists,
//! and that any defect at all denies the completeness witness.

use academic_domain::ContentDigest;

use crate::{
    document::{LectureDocument, NodeId, be_len, push_str},
    fault::RenderFault,
};

/// One of the four defects section 12.6 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderDefect {
    /// A page's content is taller than its frame.
    PageOverflow,
    /// A code block was cut off.
    ClippedCode,
    /// A placed capture did not resolve to an image.
    MissingImage,
    /// A glyph run came back with characters the font has no glyph for.
    BrokenGlyph,
}

impl RenderDefect {
    /// Every defect, in the order section 12.6 lists them.
    pub const ALL: [Self; 4] = [
        Self::PageOverflow,
        Self::ClippedCode,
        Self::MissingImage,
        Self::BrokenGlyph,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PageOverflow => "PAGE_OVERFLOW",
            Self::ClippedCode => "CLIPPED_CODE",
            Self::MissingImage => "MISSING_IMAGE",
            Self::BrokenGlyph => "BROKEN_GLYPH",
        }
    }

    /// The phrase section 12.6 uses for this defect, verbatim.
    ///
    /// `lecture_render_qa` reads the specification's sentence and requires
    /// these four phrases to be exactly its members, in order.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::PageOverflow => "page overflow",
            Self::ClippedCode => "잘린 code",
            Self::MissingImage => "누락 image",
            Self::BrokenGlyph => "깨진 glyph",
        }
    }
}

/// One page, as a renderer measured it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderedPage {
    /// Which page, from one.
    pub number: u32,
    /// How tall the content is, in the renderer's own units.
    pub content_height_units: u32,
    /// How tall the frame is, in the same units.
    pub frame_height_units: u32,
}

/// One node's box, as a renderer measured it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedNode {
    /// Which node.
    pub node: NodeId,
    /// Which page it landed on.
    pub page: u32,
    /// Whether the renderer had to cut it off.
    pub clipped: bool,
    /// How many characters the font had no glyph for.
    pub missing_glyphs: u32,
}

/// One placed capture, as a renderer resolved it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderedImage {
    /// Which capture, by journal frame sequence.
    pub capture_frame_seq: u32,
    /// Whether the image resolved.
    pub resolved: bool,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFinding {
    defect: RenderDefect,
    node: Option<NodeId>,
    page: Option<u32>,
    capture_frame_seq: Option<u32>,
}

impl RenderFinding {
    /// Which defect.
    #[must_use]
    pub const fn defect(&self) -> RenderDefect {
        self.defect
    }

    /// Which node, when the defect belongs to one.
    #[must_use]
    pub const fn node(&self) -> Option<&NodeId> {
        self.node.as_ref()
    }

    /// Which page, when the defect belongs to one.
    #[must_use]
    pub const fn page(&self) -> Option<u32> {
        self.page
    }

    /// Which capture, when the defect belongs to one.
    #[must_use]
    pub const fn capture_frame_seq(&self) -> Option<u32> {
        self.capture_frame_seq
    }
}

/// What a render QA run found.
///
/// Private fields and one producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderQaReport {
    document_digest: ContentDigest,
    page_count: usize,
    node_count: usize,
    findings: Vec<RenderFinding>,
}

impl RenderQaReport {
    /// The document that was rendered.
    #[must_use]
    pub const fn document_digest(&self) -> &ContentDigest {
        &self.document_digest
    }

    /// How many pages were measured.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.page_count
    }

    /// How many node boxes were measured.
    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    /// Every finding, grouped by defect in [`RenderDefect::ALL`] order.
    #[must_use]
    pub fn findings(&self) -> &[RenderFinding] {
        &self.findings
    }

    /// Whether the render is clean.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// The findings of one defect.
    #[must_use]
    pub fn of(&self, defect: RenderDefect) -> Vec<&RenderFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.defect == defect)
            .collect()
    }

    /// The report's canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-render-qa-v1\0");
        material.extend_from_slice(self.document_digest.to_string().as_bytes());
        material.extend_from_slice(&be_len(self.page_count));
        material.extend_from_slice(&be_len(self.node_count));
        material.extend_from_slice(&be_len(self.findings.len()));
        for finding in &self.findings {
            push_str(&mut material, finding.defect.as_str());
            match &finding.node {
                Some(node) => {
                    material.push(1);
                    push_str(&mut material, node.as_str());
                }
                None => material.push(0),
            }
            match finding.page {
                Some(page) => {
                    material.push(1);
                    material.extend_from_slice(&page.to_be_bytes());
                }
                None => material.push(0),
            }
            match finding.capture_frame_seq {
                Some(frame_seq) => {
                    material.push(1);
                    material.extend_from_slice(&frame_seq.to_be_bytes());
                }
                None => material.push(0),
            }
        }
        material
    }
}

/// Section 12.6's render QA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderQa;

impl RenderQa {
    /// Inspects one render of one document.
    ///
    /// # Errors
    ///
    /// [`RenderFault`] when the measurement does not describe this document:
    /// an unknown node, an unknown image, a node the render did not measure, or
    /// no page at all. A render QA run over a partial measurement would report
    /// "clean" for the half nobody looked at, so a partial measurement is a
    /// refusal and not a clean report.
    pub fn inspect(
        document: &LectureDocument,
        pages: &[RenderedPage],
        nodes: &[RenderedNode],
        images: &[RenderedImage],
    ) -> Result<RenderQaReport, RenderFault> {
        if pages.is_empty() {
            return Err(RenderFault::NoPages);
        }
        for measured in nodes {
            if !document
                .nodes()
                .iter()
                .any(|node| node.id() == &measured.node)
            {
                return Err(RenderFault::UnknownNode(measured.node.as_str().to_owned()));
            }
        }
        let mut measured_ids: Vec<&NodeId> = nodes.iter().map(|node| &node.node).collect();
        measured_ids.sort_unstable();
        measured_ids.dedup();
        if measured_ids.len() != document.nodes().len() {
            return Err(RenderFault::IncompleteRender {
                measured: measured_ids.len(),
                expected: document.nodes().len(),
            });
        }
        let mut placed: Vec<u32> = document
            .nodes()
            .iter()
            .flat_map(|node| node.nearby_captures().iter().copied())
            .collect();
        placed.sort_unstable();
        placed.dedup();
        for image in images {
            if !placed.contains(&image.capture_frame_seq) {
                return Err(RenderFault::UnknownImage(image.capture_frame_seq));
            }
        }

        let mut findings = Vec::new();
        for page in pages {
            if page.content_height_units > page.frame_height_units {
                findings.push(RenderFinding {
                    defect: RenderDefect::PageOverflow,
                    node: None,
                    page: Some(page.number),
                    capture_frame_seq: None,
                });
            }
        }
        for measured in nodes {
            if measured.clipped {
                findings.push(RenderFinding {
                    defect: RenderDefect::ClippedCode,
                    node: Some(measured.node.clone()),
                    page: Some(measured.page),
                    capture_frame_seq: None,
                });
            }
        }
        // A placed capture the render did not resolve, and a placed capture the
        // render did not mention at all, are the same defect. The second is the
        // one a report over a partial image list would hide.
        for frame_seq in &placed {
            let resolved = images
                .iter()
                .find(|image| image.capture_frame_seq == *frame_seq)
                .is_some_and(|image| image.resolved);
            if !resolved {
                findings.push(RenderFinding {
                    defect: RenderDefect::MissingImage,
                    node: None,
                    page: None,
                    capture_frame_seq: Some(*frame_seq),
                });
            }
        }
        for measured in nodes {
            if measured.missing_glyphs > 0 {
                findings.push(RenderFinding {
                    defect: RenderDefect::BrokenGlyph,
                    node: Some(measured.node.clone()),
                    page: Some(measured.page),
                    capture_frame_seq: None,
                });
            }
        }
        Ok(RenderQaReport {
            document_digest: document.digest(),
            page_count: pages.len(),
            node_count: measured_ids.len(),
            findings,
        })
    }
}
