//! Section 12.5's machine-readable `LectureDocument`.
//!
//! # The document is layered over the transcript, not written into it
//!
//! Every mapping names a segment of a [`TranscriptLineage`] read at one
//! version, and the tokens a mapping preserves are `EffectiveToken`s. No type
//! in this crate names `RawToken`, `RawSegment` or `RawTranscript`; `P2-L3`
//! holds a workspace-wide rule that no file outside `crates/transcription/`
//! does, and this crate — the first one the plan expected to break it — does
//! not, because a document that reads the versioned view can never be the thing
//! that writes a raw token.
//!
//! # What a mapping has to survive
//!
//! Three checks, and the third is the one that matters:
//!
//! 1. the segment index exists at that version and its identifier matches;
//! 2. the character range is inside the segment's verbatim text; and
//! 3. **every token the range covers still occurs, in order, in the rendered
//!    text.**
//!
//! The third is what makes the transform allow-list more than a vocabulary. A
//! rendering that drops a word, or replaces it with a paraphrase, fails it
//! whichever of section 12.5's nine transforms it declared. What it does not
//! catch is *insertion*, and that is deliberate: punctuation, headings,
//! timestamps and speaker labels are insertions, and they are the allow-list's
//! whole content.

use core::fmt;

use academic_domain::{ContentDigest, LectureSessionId};
use academic_transcription::{InputManifest, TranscriptLineage, TranscriptSegment};

use crate::{fault::DocumentFault, transform::PreservationTransform};

/// The identifier of one lecture document.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(String);

/// The identifier of one node inside a document.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(String);

macro_rules! identifier {
    ($name:ident, $label:literal) => {
        impl $name {
            /// Builds an identifier.
            ///
            /// # Errors
            ///
            /// [`DocumentFault::MalformedIdentifier`] when the value is empty,
            /// longer than 128 bytes, or holds a byte outside the ASCII
            /// alphanumerics, `.`, `_` and `-`. That is `P2-C5`'s identifier
            /// shape, reused rather than restated, because these identifiers
            /// travel into frozen engine inputs.
            pub fn new(value: &str) -> Result<Self, DocumentFault> {
                if value.is_empty()
                    || value.len() > 128
                    || !value.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
                {
                    return Err(DocumentFault::MalformedIdentifier {
                        kind: $label,
                        value: value.to_owned(),
                    });
                }
                Ok(Self(value.to_owned()))
            }

            /// The identifier's text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!($label, "("))?;
                formatter.write_str(&self.0)?;
                formatter.write_str(")")
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier!(DocumentId, "DocumentId");
identifier!(NodeId, "NodeId");

/// The five node kinds section 12.5 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// A section heading.
    Section,
    /// A paragraph of speech.
    Paragraph,
    /// A mathematical expression.
    Equation,
    /// A block of code.
    CodeBlock,
    /// A capture placed beside the span it belongs to.
    CapturePlacement,
}

impl NodeKind {
    /// Every kind, in the order section 12.5 lists them.
    pub const ALL: [Self; 5] = [
        Self::Section,
        Self::Paragraph,
        Self::Equation,
        Self::CodeBlock,
        Self::CapturePlacement,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Section => "SECTION",
            Self::Paragraph => "PARAGRAPH",
            Self::Equation => "EQUATION",
            Self::CodeBlock => "CODE_BLOCK",
            Self::CapturePlacement => "CAPTURE_PLACEMENT",
        }
    }
}

/// What a node says about the span it renders.
///
/// A closed set. `Repetition`, `Example` and `Digression` are here because
/// section 12.5 names exactly those three as the things a "tidying" step
/// deletes: annotating a span as one of them is the whole of what this system
/// does about it, and `no_low_importance_deletion` is the rule that annotating
/// changes nothing about whether the span is mapped and rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocumentAnnotation {
    /// The instructor emphasised this.
    InstructorEmphasis,
    /// The provider's confidence over this span is low.
    LowSttConfidence,
    /// The instructor said this again.
    Repetition,
    /// A worked example.
    Example,
    /// An aside.
    Digression,
    /// A term of art.
    Terminology,
    /// Section 34.1's badge for an equation nothing has verified.
    UnverifiedEquation,
    /// Section 34.1's badge for code nothing has verified.
    UnverifiedCode,
}

impl DocumentAnnotation {
    /// Every annotation.
    pub const ALL: [Self; 8] = [
        Self::InstructorEmphasis,
        Self::LowSttConfidence,
        Self::Repetition,
        Self::Example,
        Self::Digression,
        Self::Terminology,
        Self::UnverifiedEquation,
        Self::UnverifiedCode,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstructorEmphasis => "INSTRUCTOR_EMPHASIS",
            Self::LowSttConfidence => "LOW_STT_CONFIDENCE",
            Self::Repetition => "REPETITION",
            Self::Example => "EXAMPLE",
            Self::Digression => "DIGRESSION",
            Self::Terminology => "TERMINOLOGY",
            Self::UnverifiedEquation => "UNVERIFIED_EQUATION",
            Self::UnverifiedCode => "UNVERIFIED_CODE",
        }
    }
}

/// Why a node may render a span that is earlier than the one before it.
///
/// Section 12.6's ordering check is monotonic "unless explicitly
/// cross-referenced", and this closed set is what "explicitly" means: a reason
/// a reader can act on, not a boolean that turns the check off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrossReferenceReason {
    /// The instructor went back to a point made earlier.
    InstructorReturnedToEarlierPoint,
    /// This span answers a question asked earlier.
    AnswerToEarlierQuestion,
    /// A section opens by recapping what came before it.
    RecapAtSectionOpen,
}

impl CrossReferenceReason {
    /// Every reason.
    pub const ALL: [Self; 3] = [
        Self::InstructorReturnedToEarlierPoint,
        Self::AnswerToEarlierQuestion,
        Self::RecapAtSectionOpen,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstructorReturnedToEarlierPoint => "INSTRUCTOR_RETURNED_TO_EARLIER_POINT",
            Self::AnswerToEarlierQuestion => "ANSWER_TO_EARLIER_QUESTION",
            Self::RecapAtSectionOpen => "RECAP_AT_SECTION_OPEN",
        }
    }
}

/// The explicit exception section 12.6's ordering check admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrossReference {
    to_segment_index: usize,
    reason: CrossReferenceReason,
}

impl CrossReference {
    /// Records a cross-reference to an earlier segment.
    #[must_use]
    pub const fn to_segment(to_segment_index: usize, reason: CrossReferenceReason) -> Self {
        Self {
            to_segment_index,
            reason,
        }
    }

    /// Which earlier segment.
    #[must_use]
    pub const fn to_segment_index(self) -> usize {
        self.to_segment_index
    }

    /// Why.
    #[must_use]
    pub const fn reason(self) -> CrossReferenceReason {
        self.reason
    }
}

/// One node's claim on one span of one source segment.
///
/// Private fields and one producer — [`DocumentBuilder::push`] — so a mapping
/// that exists was checked against the transcript it names.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceMapping {
    segment_index: usize,
    segment_id: String,
    char_start: usize,
    char_end: usize,
    transform: PreservationTransform,
    covered_tokens: Vec<usize>,
}

impl SourceMapping {
    /// Which segment, by index at the document's version.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// The provider's identifier for that segment.
    #[must_use]
    pub fn segment_id(&self) -> &str {
        &self.segment_id
    }

    /// The character range inside the segment's verbatim text, end exclusive.
    #[must_use]
    pub const fn char_range(&self) -> (usize, usize) {
        (self.char_start, self.char_end)
    }

    /// Which of section 12.5's nine transforms this mapping declares.
    #[must_use]
    pub const fn transform(&self) -> PreservationTransform {
        self.transform
    }

    /// The token positions the range covers, ascending.
    ///
    /// This is what token coverage counts, and it is derived from the
    /// transcript rather than declared by the caller.
    #[must_use]
    pub fn covered_tokens(&self) -> &[usize] {
        &self.covered_tokens
    }
}

// The character range is a range into the lecture. `S-10`'s decision here is
// the same one `P2-L3` made for its own records: nothing that reaches the
// formatter is lecture text.
impl fmt::Debug for SourceMapping {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceMapping")
            .field("segment_index", &self.segment_index)
            .field("segment_id", &self.segment_id)
            .field("char_start", &self.char_start)
            .field("char_end", &self.char_end)
            .field("transform", &self.transform)
            .field("covered_token_count", &self.covered_tokens.len())
            .finish()
    }
}

/// What a caller offers the builder.
///
/// The builder is what turns one of these into a [`DocumentNode`]; there is no
/// other producer, which is why a node that exists has a checked mapping.
#[derive(Debug, Clone)]
pub struct NodeDraft {
    /// The node's identifier.
    pub id: NodeId,
    /// Which of section 12.5's five kinds.
    pub kind: NodeKind,
    /// The rendered text.
    pub rendered_text: String,
    /// One entry per mapped span: segment index, character range, transform.
    pub mappings: Vec<(usize, usize, usize, PreservationTransform)>,
    /// Journal frame sequences of the captures placed beside this node.
    pub nearby_captures: Vec<u32>,
    /// What the node says about the span.
    pub annotations: Vec<DocumentAnnotation>,
    /// The explicit exception to the ordering check, if this node is one.
    pub cross_reference: Option<CrossReference>,
}

/// One node of a lecture document.
#[derive(Clone, PartialEq, Eq)]
pub struct DocumentNode {
    id: NodeId,
    kind: NodeKind,
    rendered_text: String,
    mappings: Vec<SourceMapping>,
    nearby_captures: Vec<u32>,
    annotations: Vec<DocumentAnnotation>,
    cross_reference: Option<CrossReference>,
}

impl DocumentNode {
    /// The node's identifier.
    #[must_use]
    pub const fn id(&self) -> &NodeId {
        &self.id
    }

    /// Which of section 12.5's five kinds.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        self.kind
    }

    /// The rendered text.
    #[must_use]
    pub fn rendered_text(&self) -> &str {
        &self.rendered_text
    }

    /// Its mappings, in the order they were offered.
    #[must_use]
    pub fn mappings(&self) -> &[SourceMapping] {
        &self.mappings
    }

    /// The captures placed beside it.
    #[must_use]
    pub fn nearby_captures(&self) -> &[u32] {
        &self.nearby_captures
    }

    /// What it says about the span.
    #[must_use]
    pub fn annotations(&self) -> &[DocumentAnnotation] {
        &self.annotations
    }

    /// Its ordering exception, if it has one.
    #[must_use]
    pub const fn cross_reference(&self) -> Option<CrossReference> {
        self.cross_reference
    }

    /// The lowest segment index this node maps.
    #[must_use]
    pub fn first_segment_index(&self) -> Option<usize> {
        self.mappings.iter().map(SourceMapping::segment_index).min()
    }
}

// The rendered text is the lecture in words. Same decision as `RawSegment` one
// crate over: the formatter reaches it through a length only.
impl fmt::Debug for DocumentNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DocumentNode")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("rendered_byte_len", &self.rendered_text.len())
            .field("mappings", &self.mappings)
            .field("nearby_captures", &self.nearby_captures)
            .field("annotations", &self.annotations)
            .field("cross_reference", &self.cross_reference)
            .finish()
    }
}

/// Section 12.5's machine-readable document.
///
/// Private fields, no `&mut self` method, and one producer. A correction to the
/// transcript is a new version in `P2-L3`'s lineage and a new document over it,
/// never an edit here.
#[derive(Clone, PartialEq, Eq)]
pub struct LectureDocument {
    id: DocumentId,
    lecture: LectureSessionId,
    version: u32,
    nodes: Vec<DocumentNode>,
    transcript_token_digest: ContentDigest,
}

impl LectureDocument {
    /// The document's identifier.
    #[must_use]
    pub const fn id(&self) -> &DocumentId {
        &self.id
    }

    /// Which lecture session.
    #[must_use]
    pub const fn lecture(&self) -> LectureSessionId {
        self.lecture
    }

    /// Which transcript version it renders.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Its nodes, in document order.
    #[must_use]
    pub fn nodes(&self) -> &[DocumentNode] {
        &self.nodes
    }

    /// The raw token digest of the transcript this document was built over.
    ///
    /// `P2-L3` owns that digest and this crate only carries it, which is how a
    /// reader can tell that a document and a transcript belong together without
    /// this crate holding a second copy of a token.
    #[must_use]
    pub const fn transcript_token_digest(&self) -> &ContentDigest {
        &self.transcript_token_digest
    }

    /// One digest over the whole document.
    ///
    /// Length-prefixed rather than delimited, so a rendered text that spells a
    /// separator cannot collide with two nodes that do not.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-lecture-document-v1\0");
        push_str(&mut material, self.id.as_str());
        material.extend_from_slice(self.lecture.to_string().as_bytes());
        material.extend_from_slice(&self.version.to_be_bytes());
        material.extend_from_slice(self.transcript_token_digest.to_string().as_bytes());
        material.extend_from_slice(&be_len(self.nodes.len()));
        for node in &self.nodes {
            push_str(&mut material, node.id.as_str());
            push_str(&mut material, node.kind.as_str());
            push_str(&mut material, &node.rendered_text);
            material.extend_from_slice(&be_len(node.mappings.len()));
            for mapping in &node.mappings {
                material.extend_from_slice(&be_len(mapping.segment_index));
                push_str(&mut material, &mapping.segment_id);
                material.extend_from_slice(&be_len(mapping.char_start));
                material.extend_from_slice(&be_len(mapping.char_end));
                push_str(&mut material, mapping.transform.as_str());
                material.extend_from_slice(&be_len(mapping.covered_tokens.len()));
                for position in &mapping.covered_tokens {
                    material.extend_from_slice(&be_len(*position));
                }
            }
            material.extend_from_slice(&be_len(node.nearby_captures.len()));
            for capture in &node.nearby_captures {
                material.extend_from_slice(&capture.to_be_bytes());
            }
            material.extend_from_slice(&be_len(node.annotations.len()));
            for annotation in &node.annotations {
                push_str(&mut material, annotation.as_str());
            }
            match node.cross_reference {
                Some(reference) => {
                    material.push(1);
                    material.extend_from_slice(&be_len(reference.to_segment_index));
                    push_str(&mut material, reference.reason.as_str());
                }
                None => material.push(0),
            }
        }
        ContentDigest::sha256(&material)
    }
}

// The document is the lecture. Same decision as its nodes.
impl fmt::Debug for LectureDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LectureDocument")
            .field("id", &self.id)
            .field("lecture", &self.lecture)
            .field("version", &self.version)
            .field("node_count", &self.nodes.len())
            .finish()
    }
}

/// Builds a [`LectureDocument`] over one transcript version.
///
/// The builder holds the lineage and the manifest for the whole of its life, so
/// every mapping is checked against the transcript it names rather than against
/// a description of it. That ordering is `P2-L3`'s repaired
/// `AuthorizationBinding` applied here: a check whose expected value comes out
/// of the thing being checked agrees with itself.
#[derive(Debug)]
pub struct DocumentBuilder<'a> {
    id: DocumentId,
    lineage: &'a TranscriptLineage,
    manifest: &'a InputManifest,
    version: u32,
    nodes: Vec<DocumentNode>,
}

impl<'a> DocumentBuilder<'a> {
    /// Opens a builder over one version of one transcript.
    ///
    /// # Errors
    ///
    /// [`DocumentFault::NoSuchVersion`] when the lineage has no such version,
    /// and [`DocumentFault::LectureMismatch`] when the manifest was authorized
    /// for another lecture.
    pub fn over(
        id: DocumentId,
        lineage: &'a TranscriptLineage,
        version: u32,
        manifest: &'a InputManifest,
    ) -> Result<Self, DocumentFault> {
        if lineage.segment_at(version, 0).is_none() {
            return Err(DocumentFault::NoSuchVersion(version));
        }
        if manifest.binding().lecture() != lineage.raw().lecture() {
            return Err(DocumentFault::LectureMismatch);
        }
        Ok(Self {
            id,
            lineage,
            manifest,
            version,
            nodes: Vec::new(),
        })
    }

    /// Admits one node, or refuses it.
    ///
    /// # Errors
    ///
    /// A [`DocumentFault`] naming the first rule the draft broke.
    pub fn push(&mut self, draft: NodeDraft) -> Result<(), DocumentFault> {
        if self.nodes.iter().any(|node| node.id == draft.id) {
            return Err(DocumentFault::DuplicateNodeId(draft.id.as_str().to_owned()));
        }
        if draft.rendered_text.is_empty() || draft.rendered_text.chars().any(char::is_control) {
            return Err(DocumentFault::MalformedRenderedText(
                draft.id.as_str().to_owned(),
            ));
        }
        if draft.mappings.is_empty() {
            return Err(DocumentFault::NodeMapsNothing(draft.id.as_str().to_owned()));
        }
        if draft.kind == NodeKind::CapturePlacement && draft.nearby_captures.len() != 1 {
            return Err(DocumentFault::CapturePlacementNamesNoCapture(
                draft.id.as_str().to_owned(),
            ));
        }
        for frame_seq in &draft.nearby_captures {
            if !self
                .manifest
                .captures()
                .iter()
                .any(|capture| capture.frame_seq() == *frame_seq)
            {
                return Err(DocumentFault::DanglingCapture {
                    node: draft.id.as_str().to_owned(),
                    frame_seq: *frame_seq,
                });
            }
        }
        let rendered: Vec<char> = draft.rendered_text.chars().collect();
        let mut cursor = 0_usize;
        let mut mappings = Vec::with_capacity(draft.mappings.len());
        for (segment_index, char_start, char_end, transform) in draft.mappings {
            let segment = self.lineage.segment_at(self.version, segment_index).ok_or(
                DocumentFault::DanglingSegment {
                    node: draft.id.as_str().to_owned(),
                    segment_index,
                },
            )?;
            let spans = token_spans(&segment)?;
            let verbatim_len = segment.verbatim_text().chars().count();
            if char_start >= char_end || char_end > verbatim_len {
                return Err(DocumentFault::CharRangeOutOfBounds {
                    node: draft.id.as_str().to_owned(),
                    segment_index,
                    char_start,
                    char_end,
                });
            }
            let mut covered_tokens = Vec::new();
            for (position, (start, end)) in spans.iter().enumerate() {
                if *start >= char_start && *end <= char_end {
                    covered_tokens.push(position);
                }
            }
            if covered_tokens.is_empty() {
                return Err(DocumentFault::MappingCoversNoToken {
                    node: draft.id.as_str().to_owned(),
                    segment_index,
                });
            }
            // The rule the transform name cannot carry. Every covered token has
            // to still be readable, in order, in what the reader sees.
            for position in &covered_tokens {
                let token = &segment.tokens()[*position];
                let needle: Vec<char> = token.text().chars().collect();
                let found = find_chars(&rendered, &needle, cursor).ok_or_else(|| {
                    DocumentFault::TokenNotPreserved {
                        node: draft.id.as_str().to_owned(),
                        segment_index,
                        token_position: *position,
                    }
                })?;
                cursor = found.saturating_add(needle.len());
            }
            mappings.push(SourceMapping {
                segment_index,
                segment_id: segment.id().to_owned(),
                char_start,
                char_end,
                transform,
                covered_tokens,
            });
        }
        let mut annotations = draft.annotations;
        annotations.sort_unstable();
        annotations.dedup();
        self.nodes.push(DocumentNode {
            id: draft.id,
            kind: draft.kind,
            rendered_text: draft.rendered_text,
            mappings,
            nearby_captures: draft.nearby_captures,
            annotations,
            cross_reference: draft.cross_reference,
        });
        Ok(())
    }

    /// Closes the builder.
    ///
    /// # Errors
    ///
    /// [`DocumentFault::EmptyDocument`] when nothing was pushed.
    pub fn finish(self) -> Result<LectureDocument, DocumentFault> {
        if self.nodes.is_empty() {
            return Err(DocumentFault::EmptyDocument);
        }
        Ok(LectureDocument {
            id: self.id,
            lecture: self.lineage.raw().lecture(),
            version: self.version,
            nodes: self.nodes,
            transcript_token_digest: self.lineage.raw().token_sequence_digest(),
        })
    }
}

/// Where each of a segment's tokens sits in its verbatim text.
///
/// A left-to-right scan: each token is located at or after the end of the one
/// before it. That is deterministic, and it is also the token alignment section
/// 12.6 asks for — a segment whose verbatim text does not contain its own
/// tokens in order is refused rather than mapped.
///
/// # Errors
///
/// [`DocumentFault::VerbatimDoesNotContainTokens`] when a token is not there.
pub fn token_spans(segment: &TranscriptSegment<'_>) -> Result<Vec<(usize, usize)>, DocumentFault> {
    let verbatim: Vec<char> = segment.verbatim_text().chars().collect();
    let mut spans = Vec::with_capacity(segment.tokens().len());
    let mut cursor = 0_usize;
    for (position, token) in segment.tokens().iter().enumerate() {
        let needle: Vec<char> = token.text().chars().collect();
        let start = find_chars(&verbatim, &needle, cursor).ok_or_else(|| {
            DocumentFault::VerbatimDoesNotContainTokens {
                segment_id: segment.id().to_owned(),
                token_position: position,
            }
        })?;
        let end = start.saturating_add(needle.len());
        spans.push((start, end));
        cursor = end;
    }
    Ok(spans)
}

/// Finds `needle` in `haystack` at or after `from`, in characters.
fn find_chars(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let last = haystack.len() - needle.len();
    (from..=last).find(|start| &haystack[*start..*start + needle.len()] == needle)
}

pub(crate) fn be_len(value: usize) -> [u8; 8] {
    (value as u64).to_be_bytes()
}

pub(crate) fn push_str(material: &mut Vec<u8>, value: &str) {
    material.extend_from_slice(&be_len(value.len()));
    material.extend_from_slice(value.as_bytes());
}
