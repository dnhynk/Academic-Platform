//! `P2-L5`: student voice, a measured diarization number, and the capture PII
//! hold.
//!
//! This crate is about other people. A lecture room holds students who never
//! agreed to this product, so every rule here is fail-closed and the one that
//! matters most is that **no automatic editing claim exists without a
//! measurement**.
//!
//! It holds six things:
//!
//! * [`DiarizationCorpus`] — a named, versioned, synthetic corpus with a digest
//!   over its whole content;
//! * [`DiarizationMeasurement`] — what a run over that corpus measured, with
//!   one producer and no constructor taking a number;
//! * [`RedactionPolicy`] — the policy `P2-L4`'s `RedactionPolicyRef` cites,
//!   resolved, whose scope enum has no value for an original;
//! * [`RedactedDerivative`] and [`RestrictedOriginal`] — the two halves of one
//!   redaction: what a reader gets, and what stays restricted;
//! * [`CaptureUnderReview`] — a capture with student faces, a roster or a
//!   personal screen in it, which has no byte accessor at all; and
//! * [`LectureDeletionPreview`] — `P2-G6`'s expiry preview with the concept and
//!   evidence projections section 32.5 asks for listed on top of it.
//!
//! # What holds the invariants
//!
//! **An automatic redaction claim is a type that needs a measurement.**
//! [`RedactionMode::Automatic`] carries an [`AccuracyWitness`] **by value**,
//! and the one producer of a witness is [`DiarizationMeasurement::witness`],
//! which compares a measured permille against a configured one. There is no
//! setter, no `Default`, and no constructor that takes an accuracy. A
//! below-threshold corpus therefore does not produce a weaker claim; it
//! produces no claim, and the only plan left is [`RedactionMode::Manual`],
//! whose every exclusion a person decided.
//!
//! **Configuration cannot empty that guard.** [`DiarizationThreshold::new`]
//! refuses a required accuracy below [`ABSOLUTE_ACCURACY_FLOOR`] and an allowed
//! missed-student fraction above [`ABSOLUTE_MISSED_STUDENT_CEILING`]. Which
//! number inside that band is right is the user's decision; the band is not.
//!
//! **The hold is the absence of a method.** [`CaptureUnderReview`] holds its
//! `CaptureBytes` privately and hands out a digest and a length. The only type
//! here with a byte accessor is [`ReviewedCapture`], it has no public
//! constructor, and its one producer is inside [`dispatch`] after the hold
//! state admitted. `P2-L1`'s `QuarantinedArtifact` is the same shape one layer
//! down.
//!
//! **A derivative can only narrow.** Every retention pair in this crate is
//! produced by [`inherit_terms`], which calls `P2-G6`'s one inheritance
//! function. There is no second inheritance path and no argument that reverses
//! the comparison.
//!
//! **A redaction produces two values, not one.** The derivative excludes the
//! targeted speakers and holds no text for them; the original keeps the text,
//! has no accessor for it, and hands it out only against a grant it consumes.
//!
//! # Where `GATE-38-026` stands
//!
//! **Partially discharged, and the open half is stated rather than filled.**
//! What this task answers is the measurable half: the accuracy figure is a
//! measurement on a named versioned corpus rather than an estimate, and the
//! default behaviour is fail-closed derivative-only redaction. What stays open
//! is whether student voices may be removed from the **originals**, which is a
//! decision for the user and their institution.
//!
//! The way this crate does not answer it is structural: [`RedactionScope`] has
//! one variant, `DerivativeOnly`, so a policy authorising removal from an
//! original has no spelling here. `academic-retention` holds the mechanism for
//! such a removal behind an `OriginalVoiceAuthority` a caller must state; this
//! crate never produces one, and `no_original_voice_authority_is_produced_here`
//! measures that in both directions.
//!
//! # What this crate is not
//!
//! **It runs no diarizer.** There is no speech engine, no audio decoder and no
//! model in this repository. The corpus is two committed timelines per case and
//! the "hypothesis" is what a diarizer would have said, written down. What that
//! bounds the number to is on the contract page.
//!
//! **It holds no lecture media.** `CONTRIBUTING.md` rule 1 forbids it. Every
//! byte in this crate's test tree is a committed literal and every transcript
//! it redacts comes out of the real `academic_transcription::run` over
//! synthetic input.
//!
//! **It persists nothing.** There is no `academic-store` edge and this task
//! adds no migration.
//!
//! **It reads no clock, opens no socket and opens no file.** Every instant
//! arrives as an argument, which is why the acceptance rows can name the ones
//! they assert against.
//!
//! **It deletes nothing.** [`apply_deletion`] records an expiry through
//! `academic-consent`'s ledger; destroying a key slot is `P2-K5`'s and there is
//! no product edge to it.

mod corpus;
mod derivative;
mod fault;
pub mod harness;
mod hold;
mod measure;
mod policy;
mod preview;

pub use corpus::{
    CORPUS_ID, CORPUS_ROOT, CORPUS_VERSION, DiarizationCase, DiarizationCorpus, VoiceClass,
    VoiceSpan, corpus_v1,
};
pub use derivative::{
    DerivedArtifact, DisclosedOriginal, ExclusionRecord, KeptUtterance, LectureSource,
    ManualExclusion, ORIGINAL_CLASSIFICATION, RawAccessGrant, RawAccessLog, RawAccessRecord,
    RedactedDerivative, Redaction, RedactionMode, RedactionPlan, RestrictedOriginal,
    SourceUtterance, inherit_terms, redact,
};
pub use fault::{
    AccessRefusal, AccuracyRefusal, CorpusFault, DeletionFault, HoldRefusal, RedactionFault,
    ThresholdFault,
};
pub use hold::{
    CaptureUnderReview, HoldState, IngestionJobKind, IngestionReceipt, IngestionStage, PiiClass,
    PiiFinding, ReviewDecision, ReviewOutcome, ReviewedCapture, dispatch,
};
pub use measure::{
    ABSOLUTE_ACCURACY_FLOOR, ABSOLUTE_MISSED_STUDENT_CEILING, AccuracyWitness, CaseMeasurement,
    DIARIZATION_THRESHOLD_V1, DiarizationMeasurement, DiarizationThreshold, SCORER_VERSION,
    measure, measure_case,
};
pub use policy::{GATE_38_026_OPEN, RedactionPolicy, RedactionScope, SpeakerTargeting};
pub use preview::{
    AffectedProjection, AffectedProjectionKind, DeletionOutcome, EvidenceIndex,
    LectureDeletionPlan, LectureDeletionPreview, ProjectionEffect, ProjectionRecord,
    affected_projections, apply_deletion, preview_deletion, unreferenced_objects,
};
