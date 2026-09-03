//! Stages three and four: the immutable snapshot, and what it retains.
//!
//! Section 29.1 asks for an *immutable raw snapshot with a hash*, then *source
//! metadata and retrieval time*. [`RawSnapshot`] is both, and
//! `rule_source_snapshot_metadata` reads all five things back off one: the
//! retrieval instant, the HTTP metadata, the raw bytes, the content hash, and
//! the parser version.
//!
//! # `IN01`
//!
//! The transport reports the digest it computed while reading. [`store`]
//! recomputes over the bytes it was handed and refuses the snapshot when the
//! two disagree, which is the fault matrix's *source bytes change between the
//! conditional GET and the store*. Nothing is written, and the retry is an
//! ordinary second fetch that produces a new immutable version.
//!
//! # The bytes
//!
//! `source_bytes` is private and there is no accessor that returns it. The one
//! public route out is [`RawSnapshot::seal`], which returns
//! `Untrusted<IngestedDocument>` — `P2-G5`'s label, reused rather than
//! reinvented. `the_only_public_route_to_snapshot_bytes_is_the_untrusted_seal`
//! pins this type's whole public method set, so a second accessor fails as an
//! extra key.
//!
//! The field is named `source_bytes` on purpose: that name is in
//! `tools/secret-debug-policy.test.mjs`'s vocabulary, so the existing discovery
//! net refuses a derived `Debug` over this struct rather than a rule this task
//! invented. The hand-written one below prints the byte count and never the
//! bytes, and `RawSnapshot` is registered in that scan's own list of types that
//! hold bytes behind a redacting `Debug`.

use core::fmt;

use academic_domain::ContentDigest;
use academic_untrusted_content::{
    IngestError, IngestedDocument, SourceId, SourceKind, Untrusted, ingest,
};

use crate::{
    fetch::{FetchOutcome, HttpMetadata, Validators},
    identifier::ConnectorId,
    manifest::{DeclaredTarget, ParserVersion, RetrievalInstant},
};

/// Why bytes did not become a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// `IN01`. The digest the transport observed while reading is not the
    /// digest of the bytes it handed over.
    #[error(
        "the source changed under the read: transport observed {observed}, bytes hash to {stored}"
    )]
    BytesChangedUnderTheRead {
        /// What the transport reported.
        observed: ContentDigest,
        /// What the assembled bytes actually hash to.
        stored: ContentDigest,
    },
    /// The outcome carried no body, so there is nothing to store.
    #[error("a not-modified response creates no snapshot version")]
    NotModified,
}

/// One retrieval, kept whole and never edited.
///
/// Every field is private, every accessor returns a copy or a borrow of
/// metadata, and there is no setter. A later retrieval of the same document is
/// a second snapshot, never a mutation of this one.
pub struct RawSnapshot {
    connector: ConnectorId,
    target: DeclaredTarget,
    retrieved_at: RetrievalInstant,
    http: HttpMetadata,
    source_bytes: Vec<u8>,
    digest: ContentDigest,
    parser_version: ParserVersion,
}

impl RawSnapshot {
    /// Which connector retrieved it.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Which declared document.
    #[must_use]
    pub const fn target(&self) -> DeclaredTarget {
        self.target
    }

    /// When it was retrieved, on the wall clock.
    #[must_use]
    pub const fn retrieved_at(&self) -> RetrievalInstant {
        self.retrieved_at
    }

    /// What the response said about itself.
    #[must_use]
    pub const fn http(&self) -> &HttpMetadata {
        &self.http
    }

    /// The content hash over the retained bytes.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// How many bytes were retained.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.source_bytes.len()
    }

    /// Which parser reads them.
    #[must_use]
    pub const fn parser_version(&self) -> ParserVersion {
        self.parser_version
    }

    /// The validators to send on the next conditional request.
    #[must_use]
    pub fn next_validators(&self) -> Validators {
        self.http.next_validators()
    }

    /// Whether another retrieval produced the same bytes.
    ///
    /// This is the hash half of section 29.2's *conditional fetch and hash
    /// diff*: a source that answers `200` with an unchanged body is still an
    /// unchanged document.
    #[must_use]
    pub fn has_same_content_as(&self, other: &Self) -> bool {
        self.digest == other.digest
    }

    /// The retained bytes, labelled.
    ///
    /// The one public route from a snapshot to what it holds. `P2-G5` decides
    /// what a caller may then do with the result: the wrapper implements no
    /// unwrapping trait, its accessor is `pub(crate)` to that crate, and a
    /// rendered prompt may carry it only in a quoted data record.
    ///
    /// The kind is the caller's declaration rather than this crate's guess. A
    /// connector that collects syllabi passes `SourceKind::Syllabus`; one that
    /// collects reviews passes `SourceKind::ReviewText`.
    ///
    /// # Errors
    ///
    /// [`IngestError`] when the retained bytes are not UTF-8 or exceed
    /// `academic_untrusted_content::MAX_SOURCE_BYTES`.
    pub fn seal(
        &self,
        source_id: SourceId,
        kind: SourceKind,
        ingest_seq: u64,
    ) -> Result<Untrusted<IngestedDocument>, IngestError> {
        ingest(source_id, kind, ingest_seq, &self.source_bytes)
    }

    /// The retained bytes, for this crate's deterministic parser.
    ///
    /// `pub(crate)`. The parser at stage five is trusted code reading untrusted
    /// bytes, which is what a deterministic parse is; what does not exist is a
    /// public signature that hands the same bytes to anyone else.
    pub(crate) fn source_bytes(&self) -> &[u8] {
        &self.source_bytes
    }
}

impl fmt::Debug for RawSnapshot {
    /// Prints provenance and the byte count. Never the bytes, and never the
    /// digest of them.
    ///
    /// Hand-written for the reason `Untrusted<T>`'s is:
    /// `missing_debug_implementations = "deny"` demands an implementation and
    /// the one-line way to satisfy it is the derive that prints the document.
    ///
    /// The digest is deliberately absent. `tools/secret-debug-policy.test.mjs`
    /// classifies `ContentDigest` as a type that carries raw bytes -- it is a
    /// `[u8; 32]` tuple payload -- and its rule for a type that hides bytes is
    /// that nothing derived from them reaches the formatter except a length.
    /// `RawSnapshot::digest` returns it to a caller that wants it; what does
    /// not happen is a digest appearing in a log line by accident. The
    /// alternative was an exception entry in that shared net, and a rule this
    /// crate can simply satisfy is worth more than one more exception in
    /// somebody else's list.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawSnapshot")
            .field("connector", &self.connector)
            .field("target", &self.target)
            .field("retrieved_at", &self.retrieved_at)
            .field("http", &self.http)
            .field("byte_len", &self.source_bytes.len())
            .field("parser_version", &self.parser_version)
            .finish()
    }
}

/// Turns one fetch outcome into one immutable snapshot.
///
/// # Errors
///
/// [`SnapshotError::NotModified`] when the outcome carried no body, and
/// [`SnapshotError::BytesChangedUnderTheRead`] for `IN01`.
pub fn store(
    connector: ConnectorId,
    target: DeclaredTarget,
    parser_version: ParserVersion,
    outcome: FetchOutcome,
) -> Result<RawSnapshot, SnapshotError> {
    let (retrieved_at, http, source_bytes, observed) = match outcome {
        FetchOutcome::NotModified { .. } => return Err(SnapshotError::NotModified),
        FetchOutcome::Body {
            at,
            http,
            source_bytes,
            observed,
        } => (at, http, source_bytes, observed),
    };

    // `IN01`. The transport hashed what it read; this hashes what arrived. A
    // source that changed mid-read makes those two different, and the snapshot
    // is refused rather than stored under a hash that describes neither
    // reading.
    let stored = ContentDigest::sha256(&source_bytes);
    if stored != observed {
        return Err(SnapshotError::BytesChangedUnderTheRead { observed, stored });
    }

    Ok(RawSnapshot {
        connector,
        target,
        retrieved_at,
        http,
        source_bytes,
        digest: stored,
        parser_version,
    })
}
