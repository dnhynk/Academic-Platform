//! The section 3.7 aggregate: one record, one scope, one written disposition.
//!
//! # The scope is the semester recheck
//!
//! Section 12.1 asks whether a consent covers a whole term or a single lecture,
//! and section 38.1 lists recording permission per offering as something to
//! confirm every term. Both are the same field here: a [`PermissionScope`] pins
//! an offering, a [`TermKey`], a [`ScopeGrain`], and a half-open interval, and
//! [`PermissionScope::answers`] is false for anything outside all four.
//!
//! That makes the recheck structural rather than scheduled. A record written
//! for `2026-1` does not answer a request in `2026-2` -- not because a timer
//! fired, but because the request names a term the record does not cover -- so
//! the second term starts at `UNKNOWN` and
//! [`ConsentLedger`](crate::ConsentLedger) queues a recheck for it. There is no
//! carry-forward path to disable, because there is no carry-forward.
//!
//! # `GATE-38-009` and `GATE-38-019` live in this file
//!
//! Both are open per offering and per term, and both are represented as the
//! absence of a value rather than as a default:
//!
//! * `GATE-38-009` is the record itself. A new offering has none, and
//!   [`crate::CaptureStatus::Unknown`] is what a missing record resolves to.
//! * `GATE-38-019` is the allowed-media and allowed-processing sets on the
//!   grant. Both are required arguments of
//!   [`AuthorityGrant::record`](crate::AuthorityGrant::record) with no default
//!   and no "all" spelling, so an offering whose conditions the user has not
//!   confirmed has empty sets and matches no request.
//!
//! See [`crate::gate`] for the statements those two identifiers carry.

use academic_domain::{CapturePermissionId, ContentDigest, LectureSessionId, OfferingId};

use crate::{
    ConsentError,
    checklist::Checklist,
    evidence::{AuthorityGrant, RefusalRecord},
};

/// The half of an academic year a term sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Season {
    /// The first regular semester.
    First,
    /// The summer session.
    Summer,
    /// The second regular semester.
    Second,
    /// The winter session.
    Winter,
}

impl Season {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::First => "1",
            Self::Summer => "S",
            Self::Second => "2",
            Self::Winter => "W",
        }
    }
}

/// One academic term, as the pair a permission is confirmed against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TermKey {
    year: u16,
    season: Season,
}

impl TermKey {
    /// Names a term.
    pub const fn new(year: u16, season: Season) -> Result<Self, ConsentError> {
        if year < 1_900 || year > 2_999 {
            return Err(ConsentError::TermYearOutOfRange);
        }
        Ok(Self { year, season })
    }

    /// The academic year.
    #[must_use]
    pub const fn year(self) -> u16 {
        self.year
    }

    /// Which part of that year.
    #[must_use]
    pub const fn season(self) -> Season {
        self.season
    }
}

/// Whether a consent covers a whole term or one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ScopeGrain {
    /// Every session of the offering inside the interval.
    WholeTerm,
    /// Exactly one session.
    SingleLecture(LectureSessionId),
}

impl ScopeGrain {
    /// Whether this grain reaches `lecture_id`.
    #[must_use]
    pub fn reaches(self, lecture_id: LectureSessionId) -> bool {
        match self {
            Self::WholeTerm => true,
            Self::SingleLecture(named) => named == lecture_id,
        }
    }

    /// The stable external spelling of the variant.
    #[must_use]
    pub const fn kind_str(self) -> &'static str {
        match self {
            Self::WholeTerm => "WHOLE_TERM",
            Self::SingleLecture(_) => "SINGLE_LECTURE",
        }
    }
}

/// What one recorded permission covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionScope {
    offering_id: OfferingId,
    term: TermKey,
    grain: ScopeGrain,
    valid_from: u64,
    valid_to: u64,
}

impl PermissionScope {
    /// States a scope over a half-open interval.
    pub const fn new(
        offering_id: OfferingId,
        term: TermKey,
        grain: ScopeGrain,
        valid_from: u64,
        valid_to: u64,
    ) -> Result<Self, ConsentError> {
        if valid_to <= valid_from {
            return Err(ConsentError::EmptyInterval);
        }
        Ok(Self {
            offering_id,
            term,
            grain,
            valid_from,
            valid_to,
        })
    }

    /// The offering this scope is pinned to.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// The term this scope is pinned to.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// Whether this scope is a whole term or one session.
    #[must_use]
    pub const fn grain(&self) -> ScopeGrain {
        self.grain
    }

    /// The inclusive start of the interval.
    #[must_use]
    pub const fn valid_from(&self) -> u64 {
        self.valid_from
    }

    /// The exclusive end of the interval.
    #[must_use]
    pub const fn valid_to(&self) -> u64 {
        self.valid_to
    }

    /// Whether `at` falls inside the half-open interval.
    #[must_use]
    pub const fn contains(&self, at: u64) -> bool {
        self.valid_from <= at && at < self.valid_to
    }

    /// Whether this scope answers for exactly this offering, term, and session.
    ///
    /// All three are compared. Two of them would be enough today, because an
    /// offering is already a term's section of a course -- but the two
    /// identifiers travel separately through every surface above this one, and
    /// a permission that answered on the offering alone would answer a request
    /// whose term field said something else. The comparison is the cheap half
    /// of `permission_scope_does_not_cross_offering_or_term`.
    #[must_use]
    pub fn answers(
        &self,
        offering_id: OfferingId,
        term: TermKey,
        lecture: LectureSessionId,
    ) -> bool {
        self.offering_id == offering_id && self.term == term && self.grain.reaches(lecture)
    }
}

/// A medium a capture may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CaptureMedium {
    /// Room audio.
    Audio,
    /// A still photograph of a board or screen.
    PhotoOfBoard,
    /// A capture of the presented screen.
    ScreenCapture,
    /// Moving pictures of the room.
    Video,
}

impl CaptureMedium {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "AUDIO",
            Self::PhotoOfBoard => "PHOTO_OF_BOARD",
            Self::ScreenCapture => "SCREEN_CAPTURE",
            Self::Video => "VIDEO",
        }
    }
}

/// A processing step a capture may be put through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CaptureProcessing {
    /// Speech to text on this device.
    LocalStt,
    /// Optical character recognition on this device.
    LocalOcr,
    /// Speech to text through an external provider.
    ExternalStt,
    /// Summarisation through an external provider.
    ExternalSummarisation,
}

impl CaptureProcessing {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalStt => "LOCAL_STT",
            Self::LocalOcr => "LOCAL_OCR",
            Self::ExternalStt => "EXTERNAL_STT",
            Self::ExternalSummarisation => "EXTERNAL_SUMMARISATION",
        }
    }

    /// Whether this step leaves the device.
    ///
    /// A grant whose `external_processing_allowed` is false covers none of the
    /// steps this returns true for, whatever its `allowed_processing` list
    /// says. The two fields are both section 3.7's and the narrower one wins.
    #[must_use]
    pub const fn leaves_the_device(self) -> bool {
        match self {
            Self::LocalStt | Self::LocalOcr => false,
            Self::ExternalStt | Self::ExternalSummarisation => true,
        }
    }
}

/// Exactly what a grant covers: the two sets and the two flags.
///
/// This is `GATE-38-019`'s cell as a type. All four are required arguments,
/// there is no `Default`, and there is no "everything" spelling for either set:
/// an offering whose conditions the user has not confirmed is one whose sets
/// are empty, and an empty set matches no request.
///
/// The two flags carry section 3.7's defaults of `0` by being arguments a
/// caller has to write rather than fields a caller can omit. `P2-K5`'s
/// `OriginalVoiceAuthority` refuses a default for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermittedUse {
    allowed_media: Vec<CaptureMedium>,
    allowed_processing: Vec<CaptureProcessing>,
    external_processing_allowed: bool,
    sharing_allowed: bool,
}

impl PermittedUse {
    /// States what a grant covers.
    ///
    /// Both sets are sorted and deduplicated so two spellings of the same set
    /// produce the same `conditions_hash` input and the same comparison.
    #[must_use]
    pub fn new(
        allowed_media: Vec<CaptureMedium>,
        allowed_processing: Vec<CaptureProcessing>,
        external_processing_allowed: bool,
        sharing_allowed: bool,
    ) -> Self {
        let mut media = allowed_media;
        media.sort_unstable();
        media.dedup();
        let mut processing = allowed_processing;
        processing.sort_unstable();
        processing.dedup();
        Self {
            allowed_media: media,
            allowed_processing: processing,
            external_processing_allowed,
            sharing_allowed,
        }
    }

    /// The media this grant covers.
    #[must_use]
    pub fn allowed_media(&self) -> &[CaptureMedium] {
        &self.allowed_media
    }

    /// The processing this grant covers.
    #[must_use]
    pub fn allowed_processing(&self) -> &[CaptureProcessing] {
        &self.allowed_processing
    }

    /// Whether the grant reaches processing off this device.
    #[must_use]
    pub const fn external_processing_allowed(&self) -> bool {
        self.external_processing_allowed
    }

    /// Whether the grant reaches sharing.
    #[must_use]
    pub const fn sharing_allowed(&self) -> bool {
        self.sharing_allowed
    }
}

/// A condition a written authority attached to its grant.
///
/// Closed: a condition nobody can name is a condition nothing can enforce, and
/// section 3.7 hashes this list into `conditions_hash`. A grant with a
/// requirement outside this set is a contract change rather than a free-text
/// field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Condition {
    /// Student voices must not be retained.
    NoStudentVoices,
    /// The instructor reviews anything before it is shared.
    InstructorReviewBeforeSharing,
    /// Processing stays on this device.
    LocalProcessingOnly,
    /// The capture may not be passed to anyone else.
    NoRedistribution,
    /// The original is deleted once a transcript exists.
    DeleteOriginalAfterTranscription,
    /// The capture exists for an accessibility accommodation only.
    AccessibilityUseOnly,
}

impl Condition {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoStudentVoices => "NO_STUDENT_VOICES",
            Self::InstructorReviewBeforeSharing => "INSTRUCTOR_REVIEW_BEFORE_SHARING",
            Self::LocalProcessingOnly => "LOCAL_PROCESSING_ONLY",
            Self::NoRedistribution => "NO_REDISTRIBUTION",
            Self::DeleteOriginalAfterTranscription => "DELETE_ORIGINAL_AFTER_TRANSCRIPTION",
            Self::AccessibilityUseOnly => "ACCESSIBILITY_USE_ONLY",
        }
    }
}

/// What a written authority said. There is no third arm.
///
/// A permission is not a tri-state with an "unset" member: the unset case is
/// the absence of a [`PermissionRecord`], which is why
/// [`crate::CaptureStatus::Unknown`] is unreachable from this enum.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Disposition {
    /// The authority refused, in writing.
    Prohibited(RefusalRecord),
    /// The authority granted, in writing.
    Granted(AuthorityGrant),
}

/// One row of the section 3.7 aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRecord {
    permission_id: CapturePermissionId,
    permission_seq: u32,
    scope: PermissionScope,
    disposition: Disposition,
    checklist: Checklist,
    verified_at: u64,
    verification_source_digest: ContentDigest,
}

impl PermissionRecord {
    /// Records what a written authority said about one scope.
    ///
    /// `verified_at` and `verification_source_digest` are section 3.7 columns
    /// and are required for a refusal as much as for a grant: a `PROHIBITED`
    /// nobody verified is as unfounded as a `PERMITTED` nobody verified.
    ///
    /// A grant whose `not_after` falls outside the scope interval is refused
    /// here rather than clamped, because clamping would silently rewrite what
    /// the authority wrote. A verification recorded *before* the interval
    /// begins is not refused here: it is a live record whose status
    /// [`status_of`](crate::status::status_of) reports as `EXPIRED`, so it
    /// leaves the audit row section 3.7 requires instead of being unwritable.
    pub fn record(
        permission_id: CapturePermissionId,
        permission_seq: u32,
        scope: PermissionScope,
        disposition: Disposition,
        checklist: Checklist,
        verified_at: u64,
        verification_source_digest: ContentDigest,
    ) -> Result<Self, ConsentError> {
        if permission_seq < 1 {
            return Err(ConsentError::PermissionSequenceOutOfRange);
        }
        if let Disposition::Granted(grant) = &disposition {
            grant.check_against(scope.valid_from(), scope.valid_to())?;
        }
        Ok(Self {
            permission_id,
            permission_seq,
            scope,
            disposition,
            checklist,
            verified_at,
            verification_source_digest,
        })
    }

    /// The aggregate identifier.
    #[must_use]
    pub const fn permission_id(&self) -> CapturePermissionId {
        self.permission_id
    }

    /// The second half of the section 3.7 key.
    #[must_use]
    pub const fn permission_seq(&self) -> u32 {
        self.permission_seq
    }

    /// What this record covers.
    #[must_use]
    pub const fn scope(&self) -> &PermissionScope {
        &self.scope
    }

    /// What the authority said.
    #[must_use]
    pub const fn disposition(&self) -> &Disposition {
        &self.disposition
    }

    /// The seven-dimension checklist as it stands for this record.
    #[must_use]
    pub const fn checklist(&self) -> &Checklist {
        &self.checklist
    }

    /// When the verification happened.
    #[must_use]
    pub const fn verified_at(&self) -> u64 {
        self.verified_at
    }

    /// The digest of what was verified.
    #[must_use]
    pub const fn verification_source_digest(&self) -> &ContentDigest {
        &self.verification_source_digest
    }

    /// The grant, when there is one.
    #[must_use]
    pub const fn grant(&self) -> Option<&AuthorityGrant> {
        match &self.disposition {
            Disposition::Granted(grant) => Some(grant),
            Disposition::Prohibited(_) => None,
        }
    }
}
