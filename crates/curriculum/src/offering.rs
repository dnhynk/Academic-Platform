//! Section 8.2's `CourseOffering`: the actual section that ran in one term.
//!
//! # What is not here
//!
//! Section 9's boundary table gives `CourseOffering` one row and states what it
//! does not contain: *매 수업시간의 실제 발화* — the actual utterance of each
//! class hour. A `Lecture` is the session and a transcript belongs to it;
//! `P2-U7` and `P2-L2` own those aggregates. Nothing on
//! [`CourseOfferingDraft`] takes a transcript, a segment, an utterance, an
//! audio locator, a speaker, or a caption, and
//! `tests/compile_fail/offering_boundary_rejects_session_transcript.rs`
//! observes it.
//!
//! [`CourseOffering::lecture_refs`] is section 8.2's `lectureRefs`, which is a
//! list of identifiers and not a list of sessions. A reference names a lecture;
//! it carries none of that lecture's content, and there is no accessor here
//! that turns one into text.
//!
//! # The status field is this aggregate's; the prediction behind it is not
//!
//! Section 8.3 fixes four statuses and `P2-U5` owns the calibrated prediction,
//! the feature families, and the per-term evaluation that decide which one a
//! never-confirmed offering carries. [`OfferingStatus`] is the field section
//! 8.2 puts on the aggregate; this crate computes no probability, holds no
//! observation window, and has no function that promotes a prediction into
//! `Confirmed`.

use academic_domain::{ArtifactId, CourseRevisionId, EntityId, OfferingId, TimestampMillis};

use crate::{
    error::CurriculumError,
    text::{InstructorName, SectionCode, TermCode},
};

/// Section 8.3's four offering statuses.
///
/// `P2-U5` decides which one an unconfirmed offering carries and on what
/// evidence. What is fixed here is that the four are distinct values and that
/// `Confirmed` is not reachable from any of the others by a method on this
/// type: there is no `promote`, no `upgrade`, and no `From`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OfferingStatus {
    /// Present in the term's official registration system and recently checked.
    Confirmed,
    /// A reproducible pattern across past terms, with no official future notice.
    HistoricallyLikely,
    /// Too few samples, irregular, or the instructor changed.
    Uncertain,
    /// An official cancellation or withdrawal notice exists.
    Cancelled,
}

impl OfferingStatus {
    /// Exhaustive listing in section 8.3's table order.
    pub const ALL: [Self; 4] = [
        Self::Confirmed,
        Self::HistoricallyLikely,
        Self::Uncertain,
        Self::Cancelled,
    ];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "CONFIRMED",
            Self::HistoricallyLikely => "HISTORICALLY_LIKELY",
            Self::Uncertain => "UNCERTAIN",
            Self::Cancelled => "CANCELLED",
        }
    }
}

/// Section 8.2's `gradingMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GradingMode {
    /// The unconfirmed value. Absence of an official record, not a choice.
    Unknown,
    /// Letter grades on the university's versioned scheme.
    Letter,
    /// Satisfactory / unsatisfactory, excluded from the grade-point average.
    SatisfactoryUnsatisfactory,
}

impl GradingMode {
    /// Exhaustive listing, `Unknown` first.
    pub const ALL: [Self; 3] = [
        Self::Unknown,
        Self::Letter,
        Self::SatisfactoryUnsatisfactory,
    ];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Letter => "LETTER",
            Self::SatisfactoryUnsatisfactory => "SATISFACTORY_UNSATISFACTORY",
        }
    }
}

/// One entry of section 8.2's `meetings`: a weekday and a minute range.
///
/// Minutes from local midnight, half-open, so a meeting that ends when another
/// begins does not overlap it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Meeting {
    weekday: Weekday,
    from_minute: u16,
    to_minute: u16,
}

impl Meeting {
    /// The only constructor.
    pub fn new(
        weekday: Weekday,
        from_minute: u16,
        to_minute: u16,
    ) -> Result<Self, CurriculumError> {
        if to_minute <= from_minute || to_minute > 24 * 60 {
            return Err(CurriculumError::Malformed {
                field: "meeting",
                reason: "a meeting is a half-open minute range inside one day",
            });
        }
        Ok(Self {
            weekday,
            from_minute,
            to_minute,
        })
    }

    /// Which weekday.
    #[must_use]
    pub const fn weekday(self) -> Weekday {
        self.weekday
    }

    /// Start minute from local midnight, inclusive.
    #[must_use]
    pub const fn from_minute(self) -> u16 {
        self.from_minute
    }

    /// End minute from local midnight, exclusive.
    #[must_use]
    pub const fn to_minute(self) -> u16 {
        self.to_minute
    }
}

/// A weekday a section meets on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

impl Weekday {
    /// Exhaustive listing.
    pub const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];
}

/// Section 8.2's `capacity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Capacity(u16);

impl Capacity {
    /// The only constructor.
    #[must_use]
    pub const fn new(seats: u16) -> Self {
        Self(seats)
    }

    /// The announced seat count.
    #[must_use]
    pub const fn seats(self) -> u16 {
        self.0
    }
}

/// Section 8.2's `CourseOffering`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseOffering {
    id: OfferingId,
    course_revision: CourseRevisionId,
    term: TermCode,
    section: SectionCode,
    instructors: Vec<InstructorName>,
    meetings: Vec<Meeting>,
    capacity: Option<Capacity>,
    grading_mode: GradingMode,
    syllabus_artifact: Option<ArtifactId>,
    material_refs: Vec<EntityId>,
    lecture_refs: Vec<EntityId>,
    assessment_refs: Vec<EntityId>,
    review_refs: Vec<EntityId>,
    official_status: OfferingStatus,
    observed_at: TimestampMillis,
}

impl CourseOffering {
    /// This offering's identifier.
    #[must_use]
    pub const fn id(&self) -> OfferingId {
        self.id
    }

    /// The catalogue definition this section ran against.
    #[must_use]
    pub const fn course_revision(&self) -> CourseRevisionId {
        self.course_revision
    }

    /// Section 8.2's `term`.
    #[must_use]
    pub const fn term(&self) -> &TermCode {
        &self.term
    }

    /// Section 8.2's `section`.
    #[must_use]
    pub const fn section(&self) -> &SectionCode {
        &self.section
    }

    /// Section 8.2's `instructors`.
    #[must_use]
    pub fn instructors(&self) -> &[InstructorName] {
        &self.instructors
    }

    /// Section 8.2's `meetings`.
    #[must_use]
    pub fn meetings(&self) -> &[Meeting] {
        &self.meetings
    }

    /// Section 8.2's `capacity`, when the source stated one.
    #[must_use]
    pub const fn capacity(&self) -> Option<Capacity> {
        self.capacity
    }

    /// Section 8.2's `gradingMode`.
    #[must_use]
    pub const fn grading_mode(&self) -> GradingMode {
        self.grading_mode
    }

    /// Section 8.2's `syllabusArtifact`, when one was captured.
    #[must_use]
    pub const fn syllabus_artifact(&self) -> Option<ArtifactId> {
        self.syllabus_artifact
    }

    /// Section 8.2's `materialRefs`. Identifiers, never content.
    #[must_use]
    pub fn material_refs(&self) -> &[EntityId] {
        &self.material_refs
    }

    /// Section 8.2's `lectureRefs`. Identifiers, never session content.
    #[must_use]
    pub fn lecture_refs(&self) -> &[EntityId] {
        &self.lecture_refs
    }

    /// Section 8.2's `assessmentRefs`. Identifiers, never items.
    #[must_use]
    pub fn assessment_refs(&self) -> &[EntityId] {
        &self.assessment_refs
    }

    /// Section 8.2's `reviewRefs`. Identifiers, never review text.
    #[must_use]
    pub fn review_refs(&self) -> &[EntityId] {
        &self.review_refs
    }

    /// Section 8.2's `officialStatus`.
    #[must_use]
    pub const fn official_status(&self) -> OfferingStatus {
        self.official_status
    }

    /// Section 8.2's `observedAt`.
    #[must_use]
    pub const fn observed_at(&self) -> TimestampMillis {
        self.observed_at
    }
}

/// The only route to a [`CourseOffering`].
#[derive(Debug, Clone)]
pub struct CourseOfferingDraft {
    id: OfferingId,
    course_revision: CourseRevisionId,
    term: TermCode,
    section: SectionCode,
    instructors: Vec<InstructorName>,
    meetings: Vec<Meeting>,
    capacity: Option<Capacity>,
    grading_mode: GradingMode,
    syllabus_artifact: Option<ArtifactId>,
    material_refs: Vec<EntityId>,
    lecture_refs: Vec<EntityId>,
    assessment_refs: Vec<EntityId>,
    review_refs: Vec<EntityId>,
    official_status: OfferingStatus,
    observed_at: TimestampMillis,
}

impl CourseOfferingDraft {
    /// Starts a draft for one revision, one term, and one section.
    ///
    /// `grading_mode` starts at [`GradingMode::Unknown`], which is what an
    /// unrecorded official grading mode reads as.
    #[must_use]
    pub const fn new(
        id: OfferingId,
        course_revision: CourseRevisionId,
        term: TermCode,
        section: SectionCode,
        official_status: OfferingStatus,
        observed_at: TimestampMillis,
    ) -> Self {
        Self {
            id,
            course_revision,
            term,
            section,
            instructors: Vec::new(),
            meetings: Vec::new(),
            capacity: None,
            grading_mode: GradingMode::Unknown,
            syllabus_artifact: None,
            material_refs: Vec::new(),
            lecture_refs: Vec::new(),
            assessment_refs: Vec::new(),
            review_refs: Vec::new(),
            official_status,
            observed_at,
        }
    }

    /// Appends one instructor.
    #[must_use]
    pub fn instructor(mut self, name: InstructorName) -> Self {
        self.instructors.push(name);
        self
    }

    /// Appends one meeting.
    #[must_use]
    pub fn meeting(mut self, meeting: Meeting) -> Self {
        self.meetings.push(meeting);
        self
    }

    /// Records the announced capacity.
    #[must_use]
    pub const fn capacity(mut self, capacity: Capacity) -> Self {
        self.capacity = Some(capacity);
        self
    }

    /// Records the official grading mode.
    #[must_use]
    pub const fn grading_mode(mut self, mode: GradingMode) -> Self {
        self.grading_mode = mode;
        self
    }

    /// Records the captured syllabus artifact.
    #[must_use]
    pub const fn syllabus_artifact(mut self, artifact: ArtifactId) -> Self {
        self.syllabus_artifact = Some(artifact);
        self
    }

    /// Appends one material reference.
    #[must_use]
    pub fn material_ref(mut self, reference: EntityId) -> Self {
        self.material_refs.push(reference);
        self
    }

    /// Appends one lecture reference. An identifier, never a session's content.
    #[must_use]
    pub fn lecture_ref(mut self, reference: EntityId) -> Self {
        self.lecture_refs.push(reference);
        self
    }

    /// Appends one assessment reference.
    #[must_use]
    pub fn assessment_ref(mut self, reference: EntityId) -> Self {
        self.assessment_refs.push(reference);
        self
    }

    /// Appends one review reference.
    #[must_use]
    pub fn review_ref(mut self, reference: EntityId) -> Self {
        self.review_refs.push(reference);
        self
    }

    /// Builds the offering.
    #[must_use]
    pub fn build(self) -> CourseOffering {
        CourseOffering {
            id: self.id,
            course_revision: self.course_revision,
            term: self.term,
            section: self.section,
            instructors: self.instructors,
            meetings: self.meetings,
            capacity: self.capacity,
            grading_mode: self.grading_mode,
            syllabus_artifact: self.syllabus_artifact,
            material_refs: self.material_refs,
            lecture_refs: self.lecture_refs,
            assessment_refs: self.assessment_refs,
            review_refs: self.review_refs,
            official_status: self.official_status,
            observed_at: self.observed_at,
        }
    }
}
