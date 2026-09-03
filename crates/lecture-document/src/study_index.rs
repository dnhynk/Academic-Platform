//! The `StudyIndex`, which is a separate artifact and says so.
//!
//! # What section 12.5 asks for, and what is here
//!
//! "요약이 필요하면 별도 `StudyIndex`로 만들고 PDF의 대체물이 아님을 표시한다."
//! Three obligations, and each one is a structure rather than a sentence:
//!
//! * **Separate artifact.** [`StudyIndexId`] is a distinct type from
//!   `DocumentId`; neither converts to the other, and there is no function
//!   anywhere that takes a `StudyIndex` and returns a `LectureDocument`, a
//!   `CoverageReport` or a `PdfArtifact`.
//! * **Visible disclosure.** [`STUDY_INDEX_DISCLOSURE`] is a constant, and
//!   [`StudyIndex`] carries it as a field with no setter and no constructor
//!   parameter. There is no route to a study index whose disclosure is missing,
//!   empty, or something else.
//! * **Not a replacement.** A study index has no completeness of any kind. It
//!   is not a value `PdfArtifact::render` can be handed, and
//!   `a_study_index_cannot_be_rendered_as_the_document` observes that as a
//!   compile error rather than as a runtime refusal.
//!
//! # Salience lives here and nowhere else
//!
//! [`Salience`] is what a summary ranks by, and it is the reason
//! `no_low_importance_deletion` has something to test rather than an absence to
//! assert: an index that drops every low-salience entry is a legitimate index,
//! and the document it points into is unchanged by it. Nothing in
//! `crate::document` or `crate::coverage` names this type, and no eligible
//! segment leaves the coverage denominator because an index ranked it low.

use academic_domain::ContentDigest;

use crate::{
    document::{DocumentId, LectureDocument, NodeId, be_len, push_str},
    fault::StudyIndexFault,
};

/// The sentence a study index carries about itself.
///
/// A constant, not a caller's string: an index whose disclosure said something
/// else, or nothing, is not a value this module produces.
pub const STUDY_INDEX_DISCLOSURE: &str = "This study index is a navigation aid over the lecture document. \
     It is not the lecture document, it is not a preservation rendering, \
     and it does not replace either.";

/// How a summary ranks one span.
///
/// Ranking is what an index is for. It is deliberately *not* an input to
/// anything in `crate::document` or `crate::coverage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Salience {
    /// The index would show this first.
    High,
    /// The index would show this.
    Medium,
    /// The index would leave this out.
    Low,
}

impl Salience {
    /// Every rank.
    pub const ALL: [Self; 3] = [Self::High, Self::Medium, Self::Low];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
        }
    }
}

/// The identifier of one study index.
///
/// A distinct type from `DocumentId` with no conversion in either direction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StudyIndexId(String);

impl StudyIndexId {
    /// Builds an identifier.
    ///
    /// # Errors
    ///
    /// [`StudyIndexFault::MalformedIdentifier`] for the same shape a document
    /// identifier has.
    pub fn new(value: &str) -> Result<Self, StudyIndexFault> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(StudyIndexFault::MalformedIdentifier {
                kind: "StudyIndexId",
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

/// One entry of a study index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyIndexEntry {
    id: String,
    heading: String,
    node: NodeId,
    salience: Salience,
}

impl StudyIndexEntry {
    /// The entry's identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What the index shows.
    #[must_use]
    pub fn heading(&self) -> &str {
        &self.heading
    }

    /// The document node it points into.
    ///
    /// Section 35's row wants an index whose links round-trip to the full
    /// source span without omission, and this is that link.
    #[must_use]
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// How the index ranks it.
    #[must_use]
    pub const fn salience(&self) -> Salience {
        self.salience
    }
}

/// A navigation index over one lecture document.
///
/// It has no completeness, no coverage, and no route back to the document as an
/// artifact. What it has is a link per entry and one disclosure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyIndex {
    id: StudyIndexId,
    document: DocumentId,
    document_digest: ContentDigest,
    entries: Vec<StudyIndexEntry>,
    disclosure: &'static str,
}

impl StudyIndex {
    /// The index's identifier.
    #[must_use]
    pub const fn id(&self) -> &StudyIndexId {
        &self.id
    }

    /// The document it indexes. A required link, not an option.
    #[must_use]
    pub const fn document(&self) -> &DocumentId {
        &self.document
    }

    /// The digest of that document, so a reader can tell a stale index.
    #[must_use]
    pub const fn document_digest(&self) -> &ContentDigest {
        &self.document_digest
    }

    /// Its entries, in the order they were added.
    #[must_use]
    pub fn entries(&self) -> &[StudyIndexEntry] {
        &self.entries
    }

    /// The sentence it carries about itself.
    ///
    /// There is no setter and no constructor parameter: this returns
    /// [`STUDY_INDEX_DISCLOSURE`] for every study index that exists.
    #[must_use]
    pub const fn disclosure(&self) -> &'static str {
        self.disclosure
    }

    /// Whether every entry still points at a node of the given document.
    ///
    /// Section 35's round-trip, checkable after the fact as well as at build
    /// time, because a document can be rebuilt.
    #[must_use]
    pub fn round_trips(&self, document: &LectureDocument) -> bool {
        self.document == *document.id()
            && self.entries.iter().all(|entry| {
                document
                    .nodes()
                    .iter()
                    .any(|node| node.id() == entry.node())
            })
    }

    /// The index's canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-study-index-v1\0");
        push_str(&mut material, self.id.as_str());
        push_str(&mut material, self.document.as_str());
        material.extend_from_slice(self.document_digest.to_string().as_bytes());
        push_str(&mut material, self.disclosure);
        material.extend_from_slice(&be_len(self.entries.len()));
        for entry in &self.entries {
            push_str(&mut material, &entry.id);
            push_str(&mut material, &entry.heading);
            push_str(&mut material, entry.node.as_str());
            push_str(&mut material, entry.salience.as_str());
        }
        material
    }
}

/// Builds a [`StudyIndex`] over one document.
#[derive(Debug)]
pub struct StudyIndexBuilder<'a> {
    id: StudyIndexId,
    document: &'a LectureDocument,
    entries: Vec<StudyIndexEntry>,
}

impl<'a> StudyIndexBuilder<'a> {
    /// Opens a builder over one document.
    #[must_use]
    pub const fn over(id: StudyIndexId, document: &'a LectureDocument) -> Self {
        Self {
            id,
            document,
            entries: Vec::new(),
        }
    }

    /// Adds one entry.
    ///
    /// # Errors
    ///
    /// [`StudyIndexFault`] when the heading is unreadable, the entry
    /// identifier repeats, or the node is not in the document.
    pub fn add(
        &mut self,
        id: &str,
        heading: &str,
        node: NodeId,
        salience: Salience,
    ) -> Result<(), StudyIndexFault> {
        if heading.is_empty() || heading.chars().any(char::is_control) {
            return Err(StudyIndexFault::MalformedHeading(id.to_owned()));
        }
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(StudyIndexFault::DuplicateEntryId(id.to_owned()));
        }
        if !self
            .document
            .nodes()
            .iter()
            .any(|candidate| candidate.id() == &node)
        {
            return Err(StudyIndexFault::DanglingNode {
                entry: id.to_owned(),
                node: node.as_str().to_owned(),
            });
        }
        self.entries.push(StudyIndexEntry {
            id: id.to_owned(),
            heading: heading.to_owned(),
            node,
            salience,
        });
        Ok(())
    }

    /// Closes the builder.
    ///
    /// # Errors
    ///
    /// [`StudyIndexFault::EmptyIndex`] when nothing was added.
    pub fn finish(self) -> Result<StudyIndex, StudyIndexFault> {
        if self.entries.is_empty() {
            return Err(StudyIndexFault::EmptyIndex);
        }
        Ok(StudyIndex {
            id: self.id,
            document: self.document.id().clone(),
            document_digest: self.document.digest(),
            entries: self.entries,
            disclosure: STUDY_INDEX_DISCLOSURE,
        })
    }
}
