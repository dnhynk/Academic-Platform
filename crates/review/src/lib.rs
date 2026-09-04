//! `P2-U8`: course review ingestion, and the bias an aggregate has to admit to.
//!
//! Section 29.5 is short and it says four things. A review is attached to a
//! `CourseOffering`, an instructor, a term and a source. Promoting one to the
//! course needs an explicit aggregation. Login bypass, account sharing and
//! anti-bot evasion are not features. The raw text is kept privately for
//! provenance and never redistributed. Each of the four is a type here rather
//! than a rule somebody remembers.
//!
//! # A review is not attached to a course
//!
//! [`ReviewScope`] has no `CourseId` field, no constructor that takes one, and
//! no accessor that returns one. The course-level value is
//! [`CourseAggregate`], its one producer is [`CourseAggregate::promote`], and
//! the only thing that opens that door is an [`AggregationClaim`] — private
//! fields, no `Default`, no `Clone`, one constructor, and the first argument is
//! a named [`AggregationMethod`]. So a promotion without an explicit
//! aggregation is not a call that fails; it is a call with no argument to make.
//!
//! # Nothing here reaches a source
//!
//! This crate has no transport, no HTTP client, no browser driver and no
//! decoder. Its whole external import set is pinned, its whole signature set is
//! pinned, and every field of every type in it is classified — so a module that
//! spells none of the words somebody thought to forbid still fails, as an entry
//! nobody wrote down. What the executed claim is, and what it is *not*, is in
//! [the review-ingestion contract](../../../docs/contracts/review-ingestion.md).
//!
//! [`SourceAccessMode`]'s three arms are section 29.5's own union and every one
//! of them is a public page or an act a person performs.
//! [`SourceAccessMode::presents_a_credential`] is `false` for all three, as a
//! `match` rather than a constant, so a fourth arm has to answer the question.
//!
//! # The text is retained and never handed out
//!
//! [`RawReviewText`] holds the bytes. It has no `Display`, no `ToString`, no
//! `Serialize`, no `AsRef<str>` and no `From<..> for String`; the one internal
//! reader is `pub(crate)` and is called from exactly one file; and the one
//! public route out is [`RawReviewText::seal`], which produces `P2-G5`'s
//! `Untrusted<IngestedDocument>` — a value that implements no unwrapping trait.
//! So the extraction a model performs can happen and no `String` of somebody
//! else's writing exists at any point on the way to a bundle.
//!
//! # Six disclosures, and no single score
//!
//! [`BiasDisclosure`] carries one finding for each of [`BiasDimension::ALL`],
//! its only producer names the first dimension nothing disclosed, and both
//! aggregate constructors take one by value. A course reading is a
//! [`BandDistribution`] — the count of reviews in each band — and there is no
//! mean, no median and no representative band anywhere in this crate, because
//! section 29.5 ends by refusing exactly that.
//!
//! # What this crate does not have
//!
//! **No store edge and no migration.** It persists nothing. The typed rows a
//! review ingestion writes belong to whichever aggregate owner writes them;
//! this crate produces the values, the way `P2-U6` does.
//!
//! **No trust label of its own.** `P2-G5` owns `Untrusted<T>` and this crate
//! reuses it, sealing as the `SourceKind::ReviewText` variant that crate
//! already carries.
//!
//! **No fallback list of its own.** `P2-U6` owns section 29.5's four and the
//! one denial route, and [`crate::access::permit`] produces `P2-U6`'s
//! `Denial` rather than a second value shaped like one.
#![deny(missing_docs)]

pub mod access;
pub mod aggregate;
pub mod bias;
pub mod dimension;
pub mod duplicate;
pub mod error;
pub mod gate;
pub mod record;
pub mod scope;
pub mod text;

pub use access::{PermittedCollection, SourceAccessMode, SourceTermsLedger, permit};
pub use aggregate::{
    AggregationClaim, AggregationMethod, BandDistribution, CourseAggregate, CourseReading,
    OfferingAggregate, OfferingReading,
};
pub use bias::{BiasDimension, BiasDisclosure, BiasDisclosureDraft, BiasFinding, BiasStrength};
pub use dimension::{DimensionBand, DimensionReading, ReviewDimension, ReviewExtraction};
pub use duplicate::{
    DuplicateFinding, SimilarityPermille, duplicate_findings, duplicated_record_count, similarity,
};
pub use error::ReviewError;
pub use gate::OpenGate;
pub use record::{ReviewRecord, SampleBias};
pub use scope::{ReviewScope, ScopeDimension};
pub use text::{MAX_REVIEW_BYTES, ProvenanceSpan, RawReviewText};
