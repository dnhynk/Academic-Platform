//! `P2-L1`: the daemon-side capture evaluation and the device-layer
//! enforcement under it.
//!
//! # The recorder holds no device by default
//!
//! Section 3.7 says the recorder holds no microphone capability by default and
//! that the capture process obtains the operating-system handle only while it
//! holds a live `CaptureCapabilityToken`. Here that is a type: [`DeviceRuleset`]
//! has one constructor and it takes a token, so there is no value of it a
//! caller can assemble from a device class, and [`CaptureSession`] has no
//! public constructor at all. `academic-consent` owns the decision that mints
//! the token -- one binding, every path runs it -- and this crate adds no
//! second comparison beside it.
//!
//! # Allowed media is enforced where a device would open
//!
//! [`DeviceClass::of`] maps section 3.7's four media onto the three device
//! kinds an operating system hands out, and a ruleset holds only the classes
//! the token's own media set names. A grant listing `AUDIO` therefore refuses a
//! camera at [`open_device`], and with the `native-capture` feature the same
//! ruleset is what the platform backend installs, so the refusal is the
//! operating system's. What each platform actually refuses, and what it does
//! not, is measured per platform in
//! [the capture device gate contract](../../../docs/contracts/capture-device-gate.md).
//!
//! # A quarantine is a state
//!
//! Section 34.1's unpermitted-recording row asks for `PERMISSION_VIOLATION_RISK`
//! with sharing and AI processing blocked. [`QuarantinedArtifact`] has no byte
//! accessor, so there is no `SourceDocument` to stage and no `IngestedDocument`
//! to quote: the block is the absence of a method rather than a flag a reader
//! has to consult. [`ReleasableArtifact::bytes`] is the one accessor in this
//! crate, and a workspace-wide signature rule refuses a second one written
//! anywhere else.
//!
//! # What this crate is not
//!
//! It opens no socket and no database. It reads no clock: every instant it
//! compares arrives as an argument, which is why the acceptance rows can name
//! the instants they assert against. With `native-capture` off it installs
//! nothing, holds no operating-system handle, and every type in it is
//! bookkeeping -- [`DeviceLayer::Bookkeeping`] is what a session records, and
//! an artefact says so.
//!
//! It records nothing. Every chunk in every fixture in this crate's test tree
//! is a committed literal; no device is opened by any default-lane test, and
//! the `native-capture` probe opens a device handle and closes it without
//! reading a sample.

pub mod artifact;
pub mod audit;
pub mod daemon;
pub mod device;
pub mod native;
pub mod session;

pub use artifact::{
    CaptureArtifact, CaptureManifest, ChunkRecord, PERMISSION_VIOLATION_RISK, QuarantinedArtifact,
    ReleasableArtifact, TimelineGap, ViolationRisk,
};
pub use audit::{
    CaptureAudit, CaptureAuditRow, CaptureRefusal, CaptureRefusalReason, REFUSAL_REASONS,
};
pub use daemon::{CaptureAuthorization, authorize};
pub use device::{BackendId, DEVICE_CLASSES, DeviceClass, DeviceLayer, DeviceRuleset};
pub use session::{CaptureSession, open_device, releasable_bytes};
