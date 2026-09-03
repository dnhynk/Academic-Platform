//! `P2-L2`: the desktop host's one-action Record/Capture/Mark surface.
//!
//! # One clock, and it is a type
//!
//! Section 34.1 prevents timestamp misalignment with "capture와 audio process
//! 공통 session clock". A suite that reads an audio instant and an image
//! instant and compares them passes whether there was one clock or two that
//! agreed, so the sharing is structural here: [`SessionTick`] has no public
//! constructor, [`SessionClock`] is the only producer, every tick carries the
//! domain of the clock that minted it, and [`CaptureRecorder`] holds exactly
//! one clock. An anchor offered from outside is admitted through that clock and
//! refused if it came from another.
//!
//! # A label never moves a mark
//!
//! [`Mark`] has one instant, no label field and no `&mut self` method.
//! A label is a separate frame in the journal and the mark's frame is already
//! chain-digested when it is written, so ADR-003's append-only correction shape
//! is what holds the rule rather than a second mechanism.
//!
//! # Beyond tolerance is low confidence
//!
//! A drift past the effective tolerance is neither refused nor ignored: the
//! estimate still carries its ± range and the confidence becomes
//! [`AlignmentConfidence::Low`] with the [`ALIGNMENT_LOW_CONFIDENCE`] badge.
//! The tolerance is a field of an effective-dated [`CapturePolicyRow`] rather
//! than a constant, so it can be superseded and dated.
//!
//! # What this crate is not
//!
//! **It records nothing.** No sample is read and every chunk in every fixture
//! in its test tree is a committed literal. No device is opened: its product
//! closure is `academic-consent` and `academic-domain` and is pinned whole, the
//! workspace's `unsafe_code = "forbid"` applies here, and no foreign function is
//! declared anywhere in the workspace -- opening one needs one of those three.
//! Which device is an authorized recorder is an open product question under
//! section 12; Phase 2 ships the desktop host only.
//!
//! **It reads no clock.** Every elapsed reading arrives as an argument, as in
//! `academic-consent` and `academic-capture-gate`, so the acceptance rows can
//! name the instants they assert against. What [`SessionClock`] owns is the
//! refusal of a reading below one it already accepted.
//!
//! **It opens no socket and no database.** The journal is one local file. There
//! is no upload path to lose, which is why offline continuity needs no network
//! to sever.
//!
//! **It links no vault.** The journal frames are plaintext on disk under the
//! current posture — `storage_encryption=NONE`, `production_data_allowed=false`,
//! ADR-002 unaccepted — and sealing them under `AEAD_CHUNKED_V2` is an open item
//! in [the capture subsystem contract](../../../docs/contracts/capture-subsystem.md).

pub mod align;
pub mod capture;
pub mod clock;
pub mod fault;
pub mod journal;
pub mod mark;
pub mod policy;
pub mod preflight;
pub mod recorder;

pub use align::{
    ALIGNMENT_LOW_CONFIDENCE, AlignmentConfidence, AlignmentFault, Anchor, DriftEstimate,
    MappingLedger, MappingVersion, estimate_drift,
};
pub use capture::{CaptureBytes, Orientation};
pub use clock::{ClockFault, SessionClock, SessionClockDomain, SessionTick};
pub use fault::{
    FAULT_FRAME_VARIABLE, FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE, FAULT_SELECTORS,
};
pub use journal::{
    ChunkJournal, GapCause, JOURNAL_MAGIC, JournalFault, JournalHeader, JournalRecord,
    JournalRecovery, MAX_BODY_BYTES, RecordBody, mapping_version_body,
};
pub use mark::{LabelledMark, Mark, MarkFault, MarkLabel, MarkLabelKind, MarkLedger};
pub use policy::{CapturePolicyBook, CapturePolicyRow, PUBLISHED_EFFECTIVE_FROM};
pub use preflight::{
    FailureKind, FailureSignal, MicrophoneState, PreflightReading, SignalDelivery,
};
pub use recorder::{CaptureFault, CaptureRecorder, SealedCapture, begin, resume};
