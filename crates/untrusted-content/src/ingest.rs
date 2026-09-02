//! Parse-time tagging, and the index a span is later resolved against.
//!
//! Every constructor in this module returns [`Untrusted`]. There is no other
//! way to make an [`IngestedDocument`]: its fields are private, it has no
//! `Default`, and the `compile_fail` case below closes assembling one from
//! outside the crate.
//!
//! ```compile_fail
//! # use academic_untrusted_content::IngestedDocument;
//! fn forge() -> IngestedDocument {
//!     IngestedDocument { source_bytes: String::new() }
//! }
//! ```
//!
//! # The provider-response constructor takes a scanned value
//!
//! [`ingest_provider_response`] takes `&AcceptedResponse`, which
//! `academic-egress-boundary`'s `EgressProxy::accept_response` is the only
//! producer of. A response that was not put through `P2-G2`'s canary and
//! rulepack scan is therefore not a value this crate can be handed, and this
//! crate scans nothing of its own: the reuse is the argument type.

use academic_egress_boundary::AcceptedResponse;

use crate::label::{Provenance, SourceId, SourceKind, Untrusted};

/// One document, as parsed.
///
/// The field is named `source_bytes` on purpose. That name is in
/// `tools/secret-debug-policy.test.mjs`'s `SECRET_FIELD_NAMES`, so a derived
/// `Debug` over this struct is refused by the existing discovery net rather
/// than by a rule this task invented. The attribute is not spelled out here:
/// that net reads attributes with a regular expression over the whole file, so
/// prose naming one is read as the attribute itself.
pub struct IngestedDocument {
    source_bytes: String,
}

impl core::fmt::Debug for IngestedDocument {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("IngestedDocument")
            .field(
                "source_bytes",
                &format_args!("<untrusted:{} bytes>", self.source_bytes.len()),
            )
            .finish()
    }
}

impl IngestedDocument {
    /// Length of the parsed text in bytes.
    pub(crate) fn byte_len(&self) -> usize {
        self.source_bytes.len()
    }

    /// The parsed text. Crate-private; see [`crate::label`].
    pub(crate) fn text(&self) -> &str {
        &self.source_bytes
    }
}

/// Why bytes were not ingested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IngestError {
    /// The bytes were not UTF-8. A scanner cannot report what it cannot read,
    /// and a quoter cannot escape what it cannot decode.
    #[error("the source is not UTF-8 text")]
    NotUtf8,
    /// The source was longer than [`MAX_SOURCE_BYTES`].
    #[error("the source is longer than the ingest bound")]
    Oversize,
}

/// The largest document this boundary parses in one piece.
pub const MAX_SOURCE_BYTES: usize = 1 << 20;

/// Tags bytes from a named source at a position in the ingest order.
///
/// This is the single parse-time tagging point. The five non-provider
/// constructors below are thin, named wrappers over it so a caller reads the
/// source kind at the call site rather than passing it as a parameter that can
/// be defaulted.
///
/// # Errors
///
/// [`IngestError`] when the bytes are not UTF-8 or exceed [`MAX_SOURCE_BYTES`].
pub fn ingest(
    source_id: SourceId,
    kind: SourceKind,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(IngestError::Oversize);
    }
    let Ok(text) = core::str::from_utf8(bytes) else {
        return Err(IngestError::NotUtf8);
    };
    Ok(Untrusted::seal(
        IngestedDocument {
            source_bytes: text.to_owned(),
        },
        Provenance::new(source_id, kind, ingest_seq),
        bytes,
    ))
}

/// Tags a syllabus.
///
/// # Errors
///
/// As [`ingest`].
pub fn ingest_syllabus(
    source_id: SourceId,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    ingest(source_id, SourceKind::Syllabus, ingest_seq, bytes)
}

/// Tags a README.
///
/// # Errors
///
/// As [`ingest`].
pub fn ingest_readme(
    source_id: SourceId,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    ingest(source_id, SourceKind::Readme, ingest_seq, bytes)
}

/// Tags an issue body.
///
/// # Errors
///
/// As [`ingest`].
pub fn ingest_issue(
    source_id: SourceId,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    ingest(source_id, SourceKind::Issue, ingest_seq, bytes)
}

/// Tags a comment lifted out of source code.
///
/// # Errors
///
/// As [`ingest`].
pub fn ingest_code_comment(
    source_id: SourceId,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    ingest(source_id, SourceKind::CodeComment, ingest_seq, bytes)
}

/// Tags free-text review content.
///
/// # Errors
///
/// As [`ingest`].
pub fn ingest_review_text(
    source_id: SourceId,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    ingest(source_id, SourceKind::ReviewText, ingest_seq, bytes)
}

/// Tags a provider response that `P2-G2`'s scan already accepted.
///
/// The argument is the reuse. `AcceptedResponse` has one producer,
/// `EgressProxy::accept_response`, so a response this crate is handed has been
/// through the shipped rulepack and the caller's registered canary corpus. A
/// response that failed either is an `Incident` and never reaches here; see
/// [`crate::quarantine_incident`].
///
/// # Errors
///
/// As [`ingest`].
pub fn ingest_provider_response(
    source_id: SourceId,
    ingest_seq: u64,
    accepted: &AcceptedResponse,
) -> Result<Untrusted<IngestedDocument>, IngestError> {
    ingest(
        source_id,
        SourceKind::ProviderResponse,
        ingest_seq,
        accepted.bytes(),
    )
}

/// The set of documents a model output's spans may point at.
///
/// A span that names a document not in the index is unresolvable, which is a
/// quarantine. Documents are added in ingest order and never replaced: an
/// identifier already present is refused, so a later document cannot silently
/// redefine what an earlier span resolved to.
#[derive(Debug, Default)]
pub struct SourceIndex {
    documents: Vec<Untrusted<IngestedDocument>>,
}

/// Why a document was not indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    /// The identifier is already in the index.
    #[error("the source identifier is already indexed")]
    DuplicateSourceId,
}

impl SourceIndex {
    /// An empty index.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            documents: Vec::new(),
        }
    }

    /// Adds a document.
    ///
    /// # Errors
    ///
    /// [`IndexError::DuplicateSourceId`] when the identifier is already present.
    pub fn insert(&mut self, document: Untrusted<IngestedDocument>) -> Result<(), IndexError> {
        if self.get(document.provenance().source_id()).is_some() {
            return Err(IndexError::DuplicateSourceId);
        }
        self.documents.push(document);
        Ok(())
    }

    /// The document with this identifier.
    #[must_use]
    pub fn get(&self, source_id: &SourceId) -> Option<&Untrusted<IngestedDocument>> {
        self.documents
            .iter()
            .find(|document| document.provenance().source_id() == source_id)
    }

    /// Every indexed document, in ingest order.
    #[must_use]
    pub fn documents(&self) -> &[Untrusted<IngestedDocument>] {
        &self.documents
    }

    /// How many documents are indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}
