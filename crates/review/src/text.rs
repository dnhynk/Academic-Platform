//! Somebody else's writing, and the one route out of this crate it has.
//!
//! Section 29.5: *원문은 provenance 용도로 private하게 보존하되 재배포하지
//! 않는다.* Both halves are here.
//!
//! # Retained
//!
//! [`RawReviewText`] holds the bytes. It is what makes a [`ProvenanceSpan`]
//! resolvable at all: a span is an offset range plus the digest of the bytes in
//! it, and the constructor checks the range against the text and the digest
//! against those bytes, so a span that does not point at what it claims is
//! refused where it is made rather than believed.
//!
//! # Not redistributed
//!
//! The field is private and there is no setter, no `Display`, no `ToString`, no
//! `Serialize`, no `AsRef<str>` and no `From<RawReviewText> for String`. Those
//! are the shapes that put text into a bundle at a call site that reads like
//! nothing happened; `raw_review_text_is_excluded_from_export_and_share`
//! compares this crate's whole `impl` inventory against a pinned list, so a new
//! one fails whatever it is named.
//!
//! Inside this crate the bytes are reachable through one `pub(crate)`
//! accessor, [`RawReviewText::content`], and the same test requires it to be
//! called from exactly one file: `duplicate.rs`, which shingles it. A
//! deterministic near-duplicate check reading the text is what such a check
//! *is*; what does not exist is a second reader, or a public signature that
//! hands the same bytes to anyone else.
//!
//! The one **public** route is [`RawReviewText::seal`], which returns
//! `academic_untrusted_content::Untrusted<IngestedDocument>` -- `P2-G5`'s
//! label, reused rather than reinvented, and sealed as `SourceKind::ReviewText`,
//! the variant that crate already carries for exactly this. `P2-G5` then
//! decides what a caller may do with the result: the wrapper implements no
//! unwrapping trait, its accessor is `pub(crate)` to that crate, and a rendered
//! prompt may carry it only in a quoted data record. So the extraction that
//! produces `AI_INFERRED` dimensions can happen, and no `String` of the text
//! exists anywhere on the way.
//!
//! # `Debug`
//!
//! Hand-written, and it prints the digest and the byte count. The field is
//! named `source_bytes`, which is a name `tools/secret-debug-policy.test.mjs`
//! already classifies, but this crate does not rely on that: its own
//! `every_field_of_every_type_is_classified` enumerates every field of every
//! type here and requires each one carrying review content to live behind a
//! hand-written `Debug`. A registration that is merely present is what `S-18`
//! on `docs/contracts/policy-source-scans.md` is about.

use core::fmt;

use academic_untrusted_content::{
    IngestError, IngestedDocument, SourceId, Untrusted, ingest_review_text,
};

use crate::error::ReviewError;

/// The longest review text this crate holds in one piece.
///
/// `academic_untrusted_content::MAX_SOURCE_BYTES` is the boundary's own bound
/// and this is not a second one: [`RawReviewText::retain`] refuses anything
/// longer, so a value that exists here is a value [`RawReviewText::seal`] can
/// always hand over.
pub const MAX_REVIEW_BYTES: usize = academic_untrusted_content::MAX_SOURCE_BYTES;

/// Lowercase hexadecimal of `bytes`.
fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

/// A digest of `bytes` that is not a substring of them.
///
/// FNV-1a over 128 bits, rendered as 32 hexadecimal characters. This crate
/// links no hash crate on purpose: the digest here answers "are these the same
/// bytes I recorded a span over", which is an integrity check inside one
/// process against no adversary who chooses the bytes afterwards. Where a
/// digest has to survive one it is a SHA-256, and that is what
/// [`RawReviewText::seal`] hands over: `Untrusted` computes one over the same
/// bytes.
fn span_digest(bytes: &[u8]) -> String {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u128::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hex_lower(&hash.to_be_bytes())
}

/// A byte range of one review's text, and a digest of what is in it.
///
/// Section 29.5's `provenanceSpans`. It carries offsets and a digest and never
/// the bytes: a consumer of a span learns *where* an extracted dimension was
/// read from, and reading it is [`RawReviewText`]'s business.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProvenanceSpan {
    start: usize,
    end: usize,
    digest: String,
}

impl ProvenanceSpan {
    /// First byte of the span.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// One past the last byte of the span.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// How many bytes the span covers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span covers nothing.
    ///
    /// Always `false` for a span [`RawReviewText::retain`] made, which refuses
    /// an empty range. It exists because a length accessor without one is a
    /// clippy lint, and it is honest rather than unreachable: a caller holding
    /// a span reads it without having to know that.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The digest of the bytes in the span.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// One review's text, retained for provenance.
///
/// See the module documentation for what this deliberately does not implement
/// and where the one internal reader is enumerated.
pub struct RawReviewText {
    source_bytes: String,
    digest: String,
    spans: Vec<ProvenanceSpan>,
}

impl RawReviewText {
    /// Retains `text` and records the spans an extraction read it at.
    ///
    /// Each span is checked twice: its range has to lie inside the text and on
    /// character boundaries, and its digest has to be the digest of the bytes
    /// it covers. A span that survives both is one a reader can resolve later
    /// without the text having to be handed to them.
    ///
    /// # Errors
    ///
    /// [`ReviewError::EmptyText`] for empty text,
    /// [`ReviewError::TextTooLong`] above [`MAX_REVIEW_BYTES`],
    /// [`ReviewError::SpanOutOfRange`] for a range outside the text or off a
    /// character boundary, and [`ReviewError::SpanDigestMismatch`] for a digest
    /// that is not the digest of the covered bytes.
    pub fn retain(text: &str, spans: &[(usize, usize, &str)]) -> Result<Self, ReviewError> {
        if text.is_empty() {
            return Err(ReviewError::EmptyText);
        }
        if text.len() > MAX_REVIEW_BYTES {
            return Err(ReviewError::TextTooLong(text.len()));
        }
        let mut retained = Vec::with_capacity(spans.len());
        for (start, end, digest) in spans {
            let (start, end) = (*start, *end);
            let inside = start < end
                && end <= text.len()
                && text.is_char_boundary(start)
                && text.is_char_boundary(end);
            if !inside {
                return Err(ReviewError::SpanOutOfRange { start, end });
            }
            let actual = span_digest(&text.as_bytes()[start..end]);
            if actual != *digest {
                return Err(ReviewError::SpanDigestMismatch { start, end });
            }
            retained.push(ProvenanceSpan {
                start,
                end,
                digest: actual,
            });
        }
        Ok(Self {
            digest: span_digest(text.as_bytes()),
            source_bytes: text.to_owned(),
            spans: retained,
        })
    }

    /// The digest a span records against, for a caller composing one.
    ///
    /// It takes the bytes the caller already has. It is not a route to this
    /// crate's text: the argument is the caller's, and nothing here returns
    /// text.
    #[must_use]
    pub fn digest_of(bytes: &[u8]) -> String {
        span_digest(bytes)
    }

    /// The digest of the whole retained text.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Length of the retained text in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.source_bytes.len()
    }

    /// The spans an extraction read this text at.
    #[must_use]
    pub fn spans(&self) -> &[ProvenanceSpan] {
        &self.spans
    }

    /// The retained text. Crate-private; see the module documentation.
    pub(crate) fn content(&self) -> &str {
        &self.source_bytes
    }

    /// Hands the text to `P2-G5`'s boundary as untrusted content.
    ///
    /// The one public route out. What comes back implements no unwrapping
    /// trait, so this is a route to the model boundary and not to a `String`.
    ///
    /// # Errors
    ///
    /// [`IngestError`] as `academic_untrusted_content::ingest`. The oversize
    /// arm is unreachable from here because [`Self::retain`] already refuses
    /// anything longer, and it is propagated rather than unwrapped so a change
    /// to either bound stays visible at compile time.
    pub fn seal(
        &self,
        source_id: SourceId,
        ingest_seq: u64,
    ) -> Result<Untrusted<IngestedDocument>, IngestError> {
        ingest_review_text(source_id, ingest_seq, self.source_bytes.as_bytes())
    }
}

impl fmt::Debug for RawReviewText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawReviewText")
            .field("digest", &self.digest)
            .field("byte_len", &self.source_bytes.len())
            .field("span_count", &self.spans.len())
            .field(
                "source_bytes",
                &format_args!("<retained:{} bytes>", self.source_bytes.len()),
            )
            .finish()
    }
}
