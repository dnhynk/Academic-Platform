//! Every refusal this crate makes, as a typed value.
//!
//! Nothing here panics and nothing carries lecture text: a fault names an
//! index, an identifier or a frame sequence, so a log of one is not a copy of
//! the lecture.

/// What a malformed document is refused with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentFault {
    /// An identifier is empty, too long, or holds a byte outside the shape.
    #[error("{kind} is not identifier-shaped: {value}")]
    MalformedIdentifier {
        /// Which identifier.
        kind: &'static str,
        /// What was offered.
        value: String,
    },
    /// The lineage has no such version.
    #[error("the transcript has no version {0}")]
    NoSuchVersion(u32),
    /// The manifest was authorized for another lecture.
    #[error("the input manifest is for another lecture")]
    LectureMismatch,
    /// Two nodes share an identifier.
    #[error("two nodes are called {0}")]
    DuplicateNodeId(String),
    /// A node's rendered text is empty or holds a control character.
    #[error("node {0} has no readable rendered text")]
    MalformedRenderedText(String),
    /// A node maps no source at all.
    #[error("node {0} maps no source segment")]
    NodeMapsNothing(String),
    /// A capture placement node names no capture, or more than one.
    #[error("capture placement node {0} does not name exactly one capture")]
    CapturePlacementNamesNoCapture(String),
    /// A node names a capture the manifest does not hold.
    #[error("node {node} places capture {frame_seq}, which is not an authorized input")]
    DanglingCapture {
        /// Which node.
        node: String,
        /// Which frame.
        frame_seq: u32,
    },
    /// A node maps a segment the transcript does not have at that version.
    #[error("node {node} maps segment {segment_index}, which does not exist")]
    DanglingSegment {
        /// Which node.
        node: String,
        /// Which segment.
        segment_index: usize,
    },
    /// A character range is empty or reaches past the verbatim text.
    #[error(
        "node {node} maps characters {char_start}..{char_end} of segment {segment_index}, \
         which is not a range inside it"
    )]
    CharRangeOutOfBounds {
        /// Which node.
        node: String,
        /// Which segment.
        segment_index: usize,
        /// Where the range starts.
        char_start: usize,
        /// Where it ends.
        char_end: usize,
    },
    /// A character range covers no token.
    #[error("node {node} maps a range of segment {segment_index} that covers no token")]
    MappingCoversNoToken {
        /// Which node.
        node: String,
        /// Which segment.
        segment_index: usize,
    },
    /// The rendered text does not carry a token the mapping covers.
    ///
    /// This is the losslessness refusal. It is not about the transform's name:
    /// a deletion and a paraphrase both land here under every one of section
    /// 12.5's nine.
    #[error("node {node} does not preserve token {token_position} of segment {segment_index}")]
    TokenNotPreserved {
        /// Which node.
        node: String,
        /// Which segment.
        segment_index: usize,
        /// Which token.
        token_position: usize,
    },
    /// A segment's verbatim text does not contain its own tokens in order.
    #[error("segment {segment_id} does not contain token {token_position} in its verbatim text")]
    VerbatimDoesNotContainTokens {
        /// Which segment.
        segment_id: String,
        /// Which token.
        token_position: usize,
    },
    /// Nothing was pushed.
    #[error("a lecture document has at least one node")]
    EmptyDocument,
}

/// What a coverage run is refused with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoverageFault {
    /// A malformed document reached the validator.
    #[error(transparent)]
    Document(#[from] DocumentFault),
    /// The document renders another version.
    #[error("the document renders version {document} and version {requested} was asked for")]
    DocumentIsForAnotherVersion {
        /// The document's version.
        document: u32,
        /// The version the run asked for.
        requested: u32,
    },
    /// The document is for another lecture.
    #[error("the document is for another lecture")]
    LectureMismatch,
    /// The document was built over a different transcript.
    #[error("the document was built over a different transcript")]
    DocumentIsForAnotherTranscript,
    /// The lineage has no such version.
    #[error("the transcript has no version {0}")]
    NoSuchVersion(u32),
    /// The document maps a segment that also carries a declared status.
    ///
    /// Refused rather than resolved. A report that exists partitions its
    /// segments, so there is no report in which a segment carries two statuses.
    #[error("segment {segment_index} is both mapped and declared")]
    SegmentHasTwoStatuses {
        /// Which segment.
        segment_index: usize,
    },
    /// Two declarations for one segment.
    #[error("segment {0} already has a declared status")]
    DuplicateDisposition(usize),
    /// A declaration names a segment that does not exist.
    #[error("segment {0} does not exist and cannot carry a status")]
    DispositionForNoSuchSegment(usize),
    /// Only a person may declare a span absent.
    #[error("an automatic actor cannot exclude a span of a lecture")]
    AutomaticActorCannotExclude,
    /// Two exclusions for one capture.
    #[error("capture {0} already has an exclusion")]
    DuplicateCaptureExclusion(u32),
    /// An exclusion names a capture the manifest does not hold.
    #[error("capture {0} is excluded and is not an authorized input")]
    ExclusionForNoSuchCapture(u32),
    /// A capture is both placed and excluded.
    #[error("capture {0} is both placed and excluded")]
    CaptureIsPlacedAndExcluded(u32),
    /// The journal has no gap frame with that sequence.
    #[error("journal frame {0} is not a gap")]
    NoSuchGapFrame(u32),
}

/// What a render QA run is refused with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderFault {
    /// A measurement names a node the document does not have.
    #[error("the render names node {0}, which is not in the document")]
    UnknownNode(String),
    /// A measurement names a capture the document does not place.
    #[error("the render names capture {0}, which the document does not place")]
    UnknownImage(u32),
    /// The render covers fewer nodes than the document has.
    #[error("the render measured {measured} of the document's {expected} nodes")]
    IncompleteRender {
        /// How many were measured.
        measured: usize,
        /// How many there are.
        expected: usize,
    },
    /// The render has no page at all.
    #[error("a render has at least one page")]
    NoPages,
}

/// What a study index is refused with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StudyIndexFault {
    /// An identifier is not identifier-shaped.
    #[error("{kind} is not identifier-shaped: {value}")]
    MalformedIdentifier {
        /// Which identifier.
        kind: &'static str,
        /// What was offered.
        value: String,
    },
    /// An entry names a node the document does not have.
    ///
    /// Section 35's row for a navigation index says it sits *above* the
    /// lossless document; an entry that points nowhere is an index that has
    /// stopped being navigation.
    #[error("entry {entry} names node {node}, which is not in the document")]
    DanglingNode {
        /// Which entry.
        entry: String,
        /// Which node.
        node: String,
    },
    /// Two entries share an identifier.
    #[error("two entries are called {0}")]
    DuplicateEntryId(String),
    /// An entry has no heading.
    #[error("entry {0} has no readable heading")]
    MalformedHeading(String),
    /// Nothing was added.
    #[error("a study index has at least one entry")]
    EmptyIndex,
}
