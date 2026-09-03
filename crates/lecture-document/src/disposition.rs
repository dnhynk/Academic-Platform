//! The three statuses a segment can carry that the document does not derive,
//! and the evidence each one needs to exist.
//!
//! # There is no `mapped` constructor here, and that is the point
//!
//! `MAPPED` is derived: a segment is mapped when the document maps it, and no
//! caller can declare it. The other three are declarations, so each one is a
//! constructor that **takes its evidence by value**. A redaction without a
//! policy reference and an exclusion without a decider are not values this
//! module can produce, which is `P2-U1`'s "the forbidden field has no setter"
//! applied to a status rather than to a field.
//!
//! # The failure status reads the journal
//!
//! [`TranscriptionFailure`] is built from a `P2-L2` journal recovery and a
//! frame sequence, and it fails unless that frame really is a gap. `P2-L3`
//! shipped an `AuthorizationBinding` that read its expected value out of the
//! journal it was about to admit, so it agreed with itself; the shape to avoid
//! is a constructor that takes the caller's word for the evidence. Here the
//! caller supplies only *which* frame, and the frame's body decides whether
//! there is a failure to cite.

use academic_capture::{GapCause, JournalRecovery, RecordBody};
use academic_domain::{Actor, ContentDigest};

use crate::fault::CoverageFault;

/// Why a segment holds no speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NonSpeechReason {
    /// Nothing was said.
    Silence,
    /// Room noise with no speech in it.
    Noise,
    /// Music or applause.
    MusicOrApplause,
    /// Equipment or building sound.
    RoomAmbience,
}

impl NonSpeechReason {
    /// Every reason.
    pub const ALL: [Self; 4] = [
        Self::Silence,
        Self::Noise,
        Self::MusicOrApplause,
        Self::RoomAmbience,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Silence => "SILENCE",
            Self::Noise => "NOISE",
            Self::MusicOrApplause => "MUSIC_OR_APPLAUSE",
            Self::RoomAmbience => "ROOM_AMBIENCE",
        }
    }
}

/// Why one segment was excluded as non-speech, and who decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonSpeechEvidence {
    reason: NonSpeechReason,
    decided_by: Actor,
}

impl NonSpeechEvidence {
    /// Records a non-speech exclusion.
    ///
    /// # Errors
    ///
    /// [`CoverageFault::AutomaticActorCannotExclude`] for every automatic
    /// actor. Removing a span of a lecture from the coverage denominator is a
    /// judgement about what was said, and section 27.2 does not let a model
    /// make one. The match is exhaustive over `academic-domain`'s closed
    /// `Actor`, so a fifth actor class stops this crate compiling until it is
    /// classified.
    pub fn declared(reason: NonSpeechReason, decided_by: Actor) -> Result<Self, CoverageFault> {
        match &decided_by {
            Actor::User { .. } => Ok(Self { reason, decided_by }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(CoverageFault::AutomaticActorCannotExclude)
            }
        }
    }

    /// Why.
    #[must_use]
    pub const fn reason(&self) -> NonSpeechReason {
        self.reason
    }

    /// Who decided.
    #[must_use]
    pub const fn decided_by(&self) -> &Actor {
        &self.decided_by
    }
}

/// What a redaction rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RedactionBasis {
    /// A condition attached to the section 3.7 permission.
    PermissionCondition,
    /// A rights request from the person the speech is about.
    RightsRequest,
    /// An institutional rule.
    InstitutionalPolicy,
}

impl RedactionBasis {
    /// Every basis.
    pub const ALL: [Self; 3] = [
        Self::PermissionCondition,
        Self::RightsRequest,
        Self::InstitutionalPolicy,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermissionCondition => "PERMISSION_CONDITION",
            Self::RightsRequest => "RIGHTS_REQUEST",
            Self::InstitutionalPolicy => "INSTITUTIONAL_POLICY",
        }
    }
}

/// The policy a redaction cites.
///
/// The digest is not verified here. `P2-L5` owns redaction and this crate holds
/// the reference so that a `REDACTED_WITH_POLICY` status cannot exist without
/// one; what the digest resolves to is that task's, and the contract page says
/// so rather than implying this crate checked it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionPolicyRef {
    policy_digest: ContentDigest,
    basis: RedactionBasis,
    decided_by: Actor,
}

impl RedactionPolicyRef {
    /// Records the policy a redaction rests on.
    ///
    /// # Errors
    ///
    /// [`CoverageFault::AutomaticActorCannotExclude`] for every automatic
    /// actor, for the same reason [`NonSpeechEvidence::declared`] gives.
    pub fn citing(
        policy_digest: ContentDigest,
        basis: RedactionBasis,
        decided_by: Actor,
    ) -> Result<Self, CoverageFault> {
        match &decided_by {
            Actor::User { .. } => Ok(Self {
                policy_digest,
                basis,
                decided_by,
            }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(CoverageFault::AutomaticActorCannotExclude)
            }
        }
    }

    /// Which policy.
    #[must_use]
    pub const fn policy_digest(&self) -> &ContentDigest {
        &self.policy_digest
    }

    /// What it rests on.
    #[must_use]
    pub const fn basis(&self) -> RedactionBasis {
        self.basis
    }

    /// Who decided.
    #[must_use]
    pub const fn decided_by(&self) -> &Actor {
        &self.decided_by
    }
}

/// A recording failure a segment's absence of speech is explained by.
///
/// One producer, and it reads the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptionFailure {
    frame_seq: u32,
    cause: GapCause,
}

impl TranscriptionFailure {
    /// Cites one gap frame of one capture journal.
    ///
    /// # Errors
    ///
    /// [`CoverageFault::NoSuchGapFrame`] when the journal has no frame with
    /// that sequence, or when the frame is not a gap. The cause comes out of
    /// the frame, so a caller cannot name one the recording did not have.
    pub fn citing_journal_gap(
        recovery: &JournalRecovery,
        frame_seq: u32,
    ) -> Result<Self, CoverageFault> {
        let record = recovery
            .records()
            .iter()
            .find(|record| record.seq() == frame_seq)
            .ok_or(CoverageFault::NoSuchGapFrame(frame_seq))?;
        match record.body() {
            RecordBody::Gap { cause, .. } => Ok(Self {
                frame_seq,
                cause: *cause,
            }),
            _ => Err(CoverageFault::NoSuchGapFrame(frame_seq)),
        }
    }

    /// Which frame.
    #[must_use]
    pub const fn frame_seq(self) -> u32 {
        self.frame_seq
    }

    /// The cause the journal recorded.
    #[must_use]
    pub const fn cause(self) -> GapCause {
        self.cause
    }
}

/// Why one authorized capture is not placed in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptureExclusionReason {
    /// The same board, already placed from another capture.
    DuplicateOfPlacedCapture,
    /// Not a photograph of the teaching surface.
    NotOfTheTeachingSurface,
    /// The image cannot be read.
    UnreadableImage,
    /// Taken outside the lecture.
    OutsideTheLectureWindow,
}

impl CaptureExclusionReason {
    /// Every reason.
    pub const ALL: [Self; 4] = [
        Self::DuplicateOfPlacedCapture,
        Self::NotOfTheTeachingSurface,
        Self::UnreadableImage,
        Self::OutsideTheLectureWindow,
    ];

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateOfPlacedCapture => "DUPLICATE_OF_PLACED_CAPTURE",
            Self::NotOfTheTeachingSurface => "NOT_OF_THE_TEACHING_SURFACE",
            Self::UnreadableImage => "UNREADABLE_IMAGE",
            Self::OutsideTheLectureWindow => "OUTSIDE_THE_LECTURE_WINDOW",
        }
    }
}

/// One authorized capture the document does not place, and why.
///
/// The reason is a closed enum rather than free text, so "excluded with a
/// reason" is a set membership rather than a string-length check. `P2-R2`
/// measured five guards failing in a row because they asked whether a token was
/// on a list; a closed set has no off-list spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureExclusion {
    frame_seq: u32,
    reason: CaptureExclusionReason,
    decided_by: Actor,
}

impl CaptureExclusion {
    /// Records an exclusion.
    ///
    /// # Errors
    ///
    /// [`CoverageFault::AutomaticActorCannotExclude`] for every automatic
    /// actor: dropping a board photograph out of the document is the same class
    /// of judgement as dropping a span of speech.
    pub fn declared(
        frame_seq: u32,
        reason: CaptureExclusionReason,
        decided_by: Actor,
    ) -> Result<Self, CoverageFault> {
        match &decided_by {
            Actor::User { .. } => Ok(Self {
                frame_seq,
                reason,
                decided_by,
            }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(CoverageFault::AutomaticActorCannotExclude)
            }
        }
    }

    /// Which capture.
    #[must_use]
    pub const fn frame_seq(&self) -> u32 {
        self.frame_seq
    }

    /// Why.
    #[must_use]
    pub const fn reason(&self) -> CaptureExclusionReason {
        self.reason
    }

    /// Who decided.
    #[must_use]
    pub const fn decided_by(&self) -> &Actor {
        &self.decided_by
    }
}

/// Every capture exclusion for one document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureExclusionLedger {
    entries: Vec<CaptureExclusion>,
}

impl CaptureExclusionLedger {
    /// An empty ledger. Nothing is excluded until something says so.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Records one exclusion.
    ///
    /// # Errors
    ///
    /// [`CoverageFault::DuplicateCaptureExclusion`] when the frame already has
    /// one.
    pub fn record(&mut self, exclusion: CaptureExclusion) -> Result<(), CoverageFault> {
        if self
            .entries
            .iter()
            .any(|entry| entry.frame_seq == exclusion.frame_seq)
        {
            return Err(CoverageFault::DuplicateCaptureExclusion(
                exclusion.frame_seq,
            ));
        }
        self.entries.push(exclusion);
        Ok(())
    }

    /// Every exclusion, in record order.
    #[must_use]
    pub fn entries(&self) -> &[CaptureExclusion] {
        &self.entries
    }

    /// The exclusion for one frame, if there is one.
    #[must_use]
    pub fn for_frame(&self, frame_seq: u32) -> Option<&CaptureExclusion> {
        self.entries
            .iter()
            .find(|entry| entry.frame_seq == frame_seq)
    }
}
