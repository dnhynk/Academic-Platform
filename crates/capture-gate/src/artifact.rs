//! What a finished capture is, and the state that quarantines one.
//!
//! # Quarantine is a state, not a log line
//!
//! Section 34.1's unpermitted-recording row asks for `PERMISSION_VIOLATION_RISK`
//! with sharing and AI processing blocked. A boolean beside the bytes would be
//! a flag every reader has to remember to consult. So the two outcomes are two
//! types: [`ReleasableArtifact`] has a byte accessor and [`QuarantinedArtifact`]
//! has none, and [`CaptureArtifact`] is the sum of them.
//!
//! There is nothing to remember. A caller holding a [`QuarantinedArtifact`] has
//! no method that yields a `&[u8]`, a `String` or a `&str`, so there is no
//! `SourceDocument` to hand `academic-egress-boundary` and no
//! `IngestedDocument` to hand a `PromptEnvelope`. That is what
//! `violation_risk_blocks_share_and_ai_processing` observes against both real
//! boundaries, and what
//! `no_public_signature_hands_out_a_quarantined_capture` refuses across the
//! whole workspace, because the type is public and any crate could otherwise
//! declare the accessor this one does not.
//!
//! # A boundary stop is not a violation
//!
//! Fault `CP01` says a permission that expires mid-lecture stops the capture at
//! the boundary, retains the prior chunks under the expired scope, and records
//! an explicit timeline gap. Those chunks were recorded while the permission was
//! live, so they are releasable and the artefact carries a [`TimelineGap`]. What
//! quarantines an artefact is a chunk that does **not** re-bind: one recorded
//! outside the permission that covered it, or one whose offering an authority
//! has since refused in writing. [`crate::session::CaptureSession::seal`] is
//! where that is decided, by re-running the section 3.7 binding at every
//! recorded chunk's own instant.

use academic_consent::{CaptureDenialReason, CaptureStatus, RetentionTerms};
use std::fmt;

use academic_domain::ContentDigest;

use crate::audit::CaptureRefusalReason;

/// The section 34.1 marker an artefact carries while it is quarantined.
pub const PERMISSION_VIOLATION_RISK: &str = "PERMISSION_VIOLATION_RISK";

/// One recorded chunk's identity. No sample, no frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRecord {
    seq: u32,
    started_at: u64,
    byte_len: usize,
    digest: ContentDigest,
}

impl ChunkRecord {
    pub(crate) const fn build(
        seq: u32,
        started_at: u64,
        byte_len: usize,
        digest: ContentDigest,
    ) -> Self {
        Self {
            seq,
            started_at,
            byte_len,
            digest,
        }
    }

    /// Position in the session, from zero.
    #[must_use]
    pub const fn seq(&self) -> u32 {
        self.seq
    }

    /// The session-clock instant the chunk opened at.
    #[must_use]
    pub const fn started_at(&self) -> u64 {
        self.started_at
    }

    /// How many bytes it holds.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// The digest of those bytes.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }
}

/// The explicit gap `CP01` requires.
///
/// It is open-ended on purpose. The system knows the instant it stopped; it
/// does not know when the lecture ended, and writing an end it inferred would
/// be the silent re-timestamping section 34.1 forbids one row above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineGap {
    from: u64,
    cause: CaptureRefusalReason,
    denial: Option<CaptureDenialReason>,
}

impl TimelineGap {
    pub(crate) const fn opened(
        from: u64,
        cause: CaptureRefusalReason,
        denial: Option<CaptureDenialReason>,
    ) -> Self {
        Self {
            from,
            cause,
            denial,
        }
    }

    /// The instant capture stopped.
    #[must_use]
    pub const fn from(&self) -> u64 {
        self.from
    }

    /// Which of the device layer's comparisons stopped it.
    #[must_use]
    pub const fn cause(&self) -> CaptureRefusalReason {
        self.cause
    }

    /// The section 3.7 comparison behind it, when there is one.
    #[must_use]
    pub const fn denial(&self) -> Option<CaptureDenialReason> {
        self.denial
    }
}

/// Why an artefact is quarantined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViolationRisk {
    chunk_seq: u32,
    chunk_at: u64,
    denial: CaptureDenialReason,
    status: CaptureStatus,
}

impl ViolationRisk {
    pub(crate) const fn raised(
        chunk_seq: u32,
        chunk_at: u64,
        denial: CaptureDenialReason,
        status: CaptureStatus,
    ) -> Self {
        Self {
            chunk_seq,
            chunk_at,
            denial,
            status,
        }
    }

    /// The section 34.1 marker.
    #[must_use]
    pub const fn state(&self) -> &'static str {
        PERMISSION_VIOLATION_RISK
    }

    /// The first chunk that did not re-bind.
    #[must_use]
    pub const fn chunk_seq(&self) -> u32 {
        self.chunk_seq
    }

    /// The instant that chunk opened at.
    #[must_use]
    pub const fn chunk_at(&self) -> u64 {
        self.chunk_at
    }

    /// Which section 3.7 comparison it failed.
    #[must_use]
    pub const fn denial(&self) -> CaptureDenialReason {
        self.denial
    }

    /// The section 3.7 status at that instant.
    #[must_use]
    pub const fn status(&self) -> CaptureStatus {
        self.status
    }
}

/// What every artefact carries whichever arm it is in.
///
/// Identifiers, counts and digests. Never a sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureManifest {
    chunks: Vec<ChunkRecord>,
    byte_len: usize,
    digest: ContentDigest,
    retention: RetentionTerms,
    gap: Option<TimelineGap>,
}

impl CaptureManifest {
    /// Every chunk that was recorded, in session order.
    #[must_use]
    pub fn chunks(&self) -> &[ChunkRecord] {
        &self.chunks
    }

    /// How many bytes the whole capture holds.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// The digest over the whole capture.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// The two retention bounds the grant attached, carried onto the artefact
    /// so the deletion the grant asked for reaches it without a second lookup.
    #[must_use]
    pub const fn retention(&self) -> RetentionTerms {
        self.retention
    }

    /// The gap, when the capture stopped before the caller stopped it.
    #[must_use]
    pub const fn gap(&self) -> Option<TimelineGap> {
        self.gap
    }
}

/// A capture whose every chunk re-bound against the permission that covered it.
///
/// The only type in this crate with a byte accessor.
#[derive(Clone, PartialEq, Eq)]
pub struct ReleasableArtifact {
    manifest: CaptureManifest,
    bytes: Vec<u8>,
}

impl fmt::Debug for ReleasableArtifact {
    /// Redacting: the released capture reaches the formatter only as a length.
    /// The manifest is left out rather than printed: it carries a
    /// `ContentDigest` over these bytes, and this file's own
    /// `no_hand_written_debug_prints_a_secret_field` rule is that a formatter
    /// over secret bytes reaches a secret-bearing field only through a length.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReleasableArtifact")
            .field("len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl ReleasableArtifact {
    /// What was captured.
    ///
    /// The one byte accessor in this crate.
    /// `the_only_byte_accessor_is_on_the_releasable_arm` compares the whole set
    /// of signatures that return bytes against this one.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The identifiers, counts and digests.
    #[must_use]
    pub const fn manifest(&self) -> &CaptureManifest {
        &self.manifest
    }
}

/// A capture holding a chunk that did not re-bind.
///
/// It has no byte accessor, so nothing can be staged for egress from it and
/// nothing can be quoted into a prompt from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedArtifact {
    manifest: CaptureManifest,
    risk: ViolationRisk,
}

impl QuarantinedArtifact {
    /// Why it is quarantined.
    #[must_use]
    pub const fn risk(&self) -> ViolationRisk {
        self.risk
    }

    /// The section 34.1 marker a surface displays.
    #[must_use]
    pub const fn state(&self) -> &'static str {
        PERMISSION_VIOLATION_RISK
    }

    /// The identifiers, counts and digests. The bytes are not among them.
    #[must_use]
    pub const fn manifest(&self) -> &CaptureManifest {
        &self.manifest
    }
}

/// A sealed capture: either releasable or quarantined, never both and never
/// neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureArtifact {
    /// Every chunk re-bound.
    Releasable(ReleasableArtifact),
    /// At least one chunk did not.
    Quarantined(QuarantinedArtifact),
}

impl CaptureArtifact {
    /// Builds the releasable arm. Crate-private; `CaptureSession::seal` is the
    /// one caller and the struct-literal count is pinned at one.
    pub(crate) const fn releasable(manifest: CaptureManifest, bytes: Vec<u8>) -> Self {
        Self::Releasable(ReleasableArtifact { manifest, bytes })
    }

    /// Builds the quarantined arm. The bytes are dropped here rather than
    /// carried into a private field: a field a future accessor could read is a
    /// path this crate would then have to keep refusing.
    pub(crate) const fn quarantined(manifest: CaptureManifest, risk: ViolationRisk) -> Self {
        Self::Quarantined(QuarantinedArtifact { manifest, risk })
    }

    pub(crate) fn manifest_of(
        chunks: Vec<ChunkRecord>,
        byte_len: usize,
        digest: ContentDigest,
        retention: RetentionTerms,
        gap: Option<TimelineGap>,
    ) -> CaptureManifest {
        CaptureManifest {
            chunks,
            byte_len,
            digest,
            retention,
            gap,
        }
    }

    /// The identifiers, counts and digests, whichever arm this is.
    #[must_use]
    pub const fn manifest(&self) -> &CaptureManifest {
        match self {
            Self::Releasable(artifact) => artifact.manifest(),
            Self::Quarantined(artifact) => artifact.manifest(),
        }
    }

    /// The releasable arm, when this is one.
    #[must_use]
    pub const fn as_releasable(&self) -> Option<&ReleasableArtifact> {
        match self {
            Self::Releasable(artifact) => Some(artifact),
            Self::Quarantined(_) => None,
        }
    }

    /// The quarantined arm, when this is one.
    #[must_use]
    pub const fn as_quarantined(&self) -> Option<&QuarantinedArtifact> {
        match self {
            Self::Quarantined(artifact) => Some(artifact),
            Self::Releasable(_) => None,
        }
    }

    /// Whether this artefact is in the section 34.1 state.
    #[must_use]
    pub const fn is_quarantined(&self) -> bool {
        matches!(self, Self::Quarantined(_))
    }
}
