//! What this crate refuses, and the name it refuses it under.
//!
//! Every arm names the thing that was wrong rather than a category. A caller
//! that has to parse a message to find out which dimension was missing is a
//! caller the acceptance evidence cannot be per-dimension for, and every one of
//! the eight tests this crate carries is per-item rather than per-count.

use crate::{bias::BiasDimension, dimension::ReviewDimension, scope::ScopeDimension};

/// Why this crate refused a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReviewError {
    /// The review text was empty.
    #[error("a review holds text")]
    EmptyText,
    /// The review text was longer than the ingest bound.
    #[error("the review text is {0} bytes, longer than the ingest bound")]
    TextTooLong(usize),
    /// A provenance span pointed outside the text or off a character boundary.
    #[error("the provenance span {start}..{end} is not a range of this text")]
    SpanOutOfRange {
        /// First byte of the offered range.
        start: usize,
        /// One past the last byte of the offered range.
        end: usize,
    },
    /// A provenance span's digest was not the digest of the bytes it covers.
    #[error("the provenance span {start}..{end} does not digest to what it claims")]
    SpanDigestMismatch {
        /// First byte of the span.
        start: usize,
        /// One past the last byte of the span.
        end: usize,
    },
    /// An extraction offered no reading for one of section 29.5's dimensions.
    #[error("the extraction reads nothing for {}", .0.spec_key())]
    DimensionMissing(ReviewDimension),
    /// An extraction offered two readings for one dimension.
    #[error("the extraction reads {} twice", .0.spec_key())]
    DimensionRepeated(ReviewDimension),
    /// A disclosure was built without one of section 29.5's six bias
    /// dimensions.
    #[error("the aggregate discloses nothing for {}", .0.as_str())]
    BiasDimensionMissing(BiasDimension),
    /// A disclosure was offered two findings for one bias dimension.
    #[error("the aggregate discloses {} twice", .0.as_str())]
    BiasDimensionRepeated(BiasDimension),
    /// An aggregate was asked for over no reviews at all.
    #[error("an aggregate is taken over at least one review")]
    NoReviews,
    /// Two reviews offered to one offering aggregate had different scopes.
    #[error("this aggregate is scoped to one offering; {} differs", .0.as_str())]
    ScopeMixed(ScopeDimension),
    /// A course promotion was offered aggregates of two different courses, or
    /// none.
    #[error("a course aggregation names the offering aggregates of one course")]
    PromotionScopeMixed,
    /// A course promotion was offered the same offering aggregate twice.
    #[error("a course aggregation names each offering aggregate once")]
    PromotionInputRepeated,
    /// A similarity threshold was outside 0..=1000.
    #[error("a similarity is 0..=1000 permille, not {0}")]
    SimilarityOutOfRange(u16),
}
