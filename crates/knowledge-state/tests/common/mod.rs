//! Synthetic fixtures for the `P2-N2` acceptance suite.
//!
//! Two halves, and neither of them invents an input this crate is supposed to
//! read from a boundary below it.
//!
//! * [`lecture`] is `crates/lecture-document/tests/common/mod.rs` restated, the
//!   way that file is `crates/transcription/tests/common/mod.rs` restated — a
//!   test module is not a library target. The capture is written by the real
//!   `academic_capture::begin`, the transcript comes out of the real
//!   `academic_transcription::run` over a table-lookup provider, and the
//!   document is built by the real `DocumentBuilder`, so a `TeachingSite` in
//!   this suite names a node of a document `P2-L4` produced.
//! * [`project`] is `crates/repository-classification/tests/classification_lanes.rs`'s
//!   harness restated for the same reason: the snapshot is `P2-R1`'s capture,
//!   the findings are `P2-R2`'s ladder and the stances are `P2-R4`'s `classify`.
//!
//! **Nothing here records, transcribes, analyses or renders anything.** Every
//! audio chunk is a committed byte string, every repository file is a literal,
//! the provider is a table lookup, and no clock is read.

pub mod lecture;
pub mod project;
