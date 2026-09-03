//! `P2-L4`: section 12.5's lossless lecture document, section 12.6's
//! deterministic coverage validator, and the render QA that stands between a
//! rendering and the word "complete".
//!
//! This crate holds seven things:
//!
//! * [`LectureDocument`] — section 12.5's machine-readable record, whose every
//!   node maps to source segments and character ranges;
//! * [`PreservationTransform`] — the nine transforms section 12.5 allows, as a
//!   closed set compared against the specification's own sentence;
//! * [`CoverageValidator`] — section 12.6's five checks, run as `P2-C5`'s
//!   `TRANSCRIPT_COVERAGE` deterministic engine so that "deterministic" is a
//!   committed byte comparison;
//! * [`RenderQa`] — the four defects section 12.6 names after rendering;
//! * [`PdfArtifact`] — a rendering that is `INCOMPLETE` unless a witness says
//!   otherwise, and that nothing reads back as a source;
//! * [`StudyIndex`] — a separate artifact that carries a disclosure it cannot
//!   drop; and
//! * [`ReviewQueue`] — equations, code and low-confidence spans, each with the
//!   audio it came from.
//!
//! # What holds the invariants
//!
//! **Exactly one status per segment is a type.** [`SegmentStatus`] has four
//! variants, [`SegmentAccount`] has one field of that type, and each non-mapped
//! variant carries its evidence by value. There is no set, no `Option`, and no
//! `#[non_exhaustive]`, so zero statuses, two statuses, an unknown status and a
//! redaction with no policy are all unrepresentable. The one property that is
//! genuinely about two inputs — a segment both mapped and declared — is a total
//! `match` whose fourth arm is a refusal.
//!
//! **Incomplete is the only value with no measurement behind it.**
//! `PdfArtifact::render` writes [`DocumentCompleteness::Incomplete`] and
//! replaces it only when it holds a [`CompletenessWitness`], whose one producer
//! is `CoverageReport::completeness_witness`. There is no completeness
//! parameter and no setter.
//!
//! **The transform allow-list is a closed set, and the rule that matters does
//! not read it.** A mapping is admitted only when every token it covers still
//! occurs, in order, in the rendered text, so a deletion or a paraphrase is
//! refused under all nine transforms.
//!
//! **Nothing here can shrink the coverage denominator.** The eligible segment
//! set is walked off the lineage rather than taken as an argument, and there is
//! no salience, ranking or importance value anywhere in the document or
//! coverage modules. [`Salience`] exists, in [`StudyIndex`], which is the
//! artifact that is allowed to leave things out and says so.
//!
//! # What this crate is not
//!
//! **It renders nothing.** There is no PDF engine, no font and no layout in
//! this repository. [`RenderQa::inspect`] reads measurements a renderer took,
//! and every measurement in this crate's test tree is a committed literal.
//!
//! **It writes no raw token, and it names no raw type.** `P2-L3` holds a
//! workspace rule that no file outside `crates/transcription/` names
//! `RawToken`, `RawSegment` or `RawTranscript`, and recorded it as a tripwire
//! for this task. This crate does not trip it: the document is built over
//! `TranscriptSegment` and `EffectiveToken` at one version, which is the same
//! discipline stated as a graph fact instead of a sentence.
//!
//! **It persists nothing.** There is no `academic-store` edge and this task
//! adds no migration: `0013` and `0016` stay unclaimed and `0018` is unwritten.
//!
//! **It transmits nothing and opens nothing.** No socket, no clock, no
//! filesystem path. The one instant it needs — a calibration dataset's
//! freshness — is an argument.
//!
//! # Where the section 38 gates stand
//!
//! None are opened or closed here. The two thresholds section 12.6 leaves open
//! are versioned configuration with recorded defaults ([`COVERAGE_CONFIG_V1`]),
//! not a gate: a threshold that can be superseded and dated is a decision the
//! user makes per profile, not a product question waiting on an answer.

mod config;
mod coverage;
mod disposition;
mod document;
mod engine;
mod fault;
pub mod harness;
mod pdf;
mod render;
mod review;
mod study_index;

pub use config::{COVERAGE_CONFIG_V1, ConfigFault, CoverageConfig};
pub use coverage::{
    CompletenessWitness, CoverageInputs, CoverageReport, CoverageValidator, DispositionLedger,
    GapFinding, OrderingException, OrderingFinding, Ratio, SegmentAccount, SegmentDisposition,
    SegmentStatus, UnaccountedCapture, UnmappedSegment,
};
pub use disposition::{
    CaptureExclusion, CaptureExclusionLedger, CaptureExclusionReason, NonSpeechEvidence,
    NonSpeechReason, RedactionBasis, RedactionPolicyRef, TranscriptionFailure,
};
pub use document::{
    CrossReference, CrossReferenceReason, DocumentAnnotation, DocumentBuilder, DocumentId,
    DocumentNode, LectureDocument, NodeDraft, NodeId, NodeKind, SourceMapping, token_spans,
};
pub use engine::{
    RULE_CAPTURES, RULE_COMPLETE, RULE_GAPS, RULE_ORDERING, RULE_PARTITION, RULE_RENDER,
    RULE_SEGMENT_COVERAGE, RULE_TOKEN_COVERAGE, RULES, RULESET_TEXT, TRANSCRIPT_COVERAGE_ENGINE_ID,
    TRANSCRIPT_COVERAGE_ENGINE_VERSION, TranscriptCoverageEngine, freeze, ruleset_hash,
};
pub use fault::{CoverageFault, DocumentFault, RenderFault, StudyIndexFault};
pub use pdf::{DocumentCompleteness, PdfArtifact};
pub use render::{
    RenderDefect, RenderFinding, RenderQa, RenderQaReport, RenderedImage, RenderedNode,
    RenderedPage,
};
pub use review::{AudioLocator, ReviewItem, ReviewQueue, RiskClass};
pub use study_index::{
    STUDY_INDEX_DISCLOSURE, Salience, StudyIndex, StudyIndexBuilder, StudyIndexEntry, StudyIndexId,
};
pub use transform::PreservationTransform;

mod transform;
