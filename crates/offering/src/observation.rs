//! What the forecast reads: one course's observed offering history.
//!
//! # An unobserved term is not a term with no offering
//!
//! Section 8.3's closing sentence is the whole reason this module has two
//! absences rather than one: *과거에 한 번도 관찰하지 못한 것은 `UNCERTAIN`이며
//! 미개설 확정이 아니다*. A term nobody read the registration system for is
//! **absent from the map**; a term somebody read and found no section in is
//! present with [`Offered::No`]. Folding the first into the second is how an
//! absence of evidence becomes a claim that a course was not offered, so the
//! two are different values here and the forecast counts them differently:
//! only the second enters the seasonal rate, and neither can produce a
//! negative official claim.
//!
//! # Terms, not instants
//!
//! Everything here is ordered on `academic_record::term::TermKey`. `P2-U4`
//! already recorded why: an effective date in this domain is written as
//! *2015학년도 1학기 이수 교과목부터*, so the unit that orders one fact against
//! another is the academic term. `academic_curriculum::CourseRelations` is
//! effective-dated on a `TimestampMillis` instead, and crossing the two axes
//! would need a term-to-date table no confirmed source supplies. So
//! [`CourseLifecycle`] is term-scoped and read from the official course-change
//! record, and this crate holds no conversion between a term and an instant.

use std::collections::BTreeMap;

use academic_curriculum::{CourseCode, InstructorName};
use academic_domain::TimestampMillis;
use academic_record::term::TermKey;

use crate::error::OfferingError;

/// Whether a term that **was** read held a section of this course.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Offered {
    /// The registration system listed a section.
    Yes,
    /// The registration system was read and listed none.
    No,
}

impl Offered {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::Yes, Self::No];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "OFFERED",
            Self::No => "NOT_OFFERED",
        }
    }
}

/// One term somebody actually read, and what it held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermObservation {
    term: TermKey,
    read_at: TimestampMillis,
    offered: Offered,
    instructors: Vec<InstructorName>,
    irregular: bool,
}

impl TermObservation {
    /// Records a term that held a section, taught by the named instructors.
    ///
    /// `read_at` is section 8.2's `observedAt`: the instant somebody read the
    /// registration system for this term. It is the one instant this crate
    /// holds, and it is what
    /// `academic_domain::PredictionObservationWindow` is built from -- so the
    /// disclosed window is the span of readings that actually happened rather
    /// than a term range converted through a table no source supplies.
    ///
    /// `irregular` is section 8.3's 불규칙 특강 여부: a one-off intensive or a
    /// special-topics run that happened once and says little about the next
    /// term.
    #[must_use]
    pub fn offered(
        term: TermKey,
        read_at: TimestampMillis,
        instructors: Vec<InstructorName>,
        irregular: bool,
    ) -> Self {
        Self {
            term,
            read_at,
            offered: Offered::Yes,
            instructors,
            irregular,
        }
    }

    /// Records a term somebody read and found no section in.
    ///
    /// There is deliberately no instructor list and no irregular flag on this
    /// constructor: a term with no section has no instructor to have changed
    /// and no special run to have been.
    #[must_use]
    pub const fn not_offered(term: TermKey, read_at: TimestampMillis) -> Self {
        Self {
            term,
            read_at,
            offered: Offered::No,
            instructors: Vec::new(),
            irregular: false,
        }
    }

    /// The term read.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// When the registration system was read for this term.
    #[must_use]
    pub const fn read_at(&self) -> TimestampMillis {
        self.read_at
    }

    /// What the reading found.
    #[must_use]
    pub const fn outcome(&self) -> Offered {
        self.offered
    }

    /// Who taught it, in the order the listing printed them.
    #[must_use]
    pub fn instructors(&self) -> &[InstructorName] {
        &self.instructors
    }

    /// Section 8.3's 불규칙 특강 flag.
    #[must_use]
    pub const fn is_irregular(&self) -> bool {
        self.irregular
    }
}

/// What an official notice inside the window said.
///
/// A notice about the **forecast term itself** is not one of these. Section
/// 8.3 says a future official notice activates a separate official claim
/// rather than promoting the prediction, so that reading arrives as
/// [`crate::source::OfficialTermReading`] and decides the standing directly.
/// What is here is the 최근 공지 *feature*: notices issued inside the
/// observation window about how this course is run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoticeEffect {
    /// The department announced the course would run more often.
    OfferingAnnounced,
    /// The department announced the course would run less often, or paused it.
    OfferingSuspended,
    /// The course's curriculum standing changed -- its category, its
    /// requirement status, or its place in a standard form.
    CurriculumChange,
}

impl NoticeEffect {
    /// Exhaustive listing.
    pub const ALL: [Self; 3] = [
        Self::OfferingAnnounced,
        Self::OfferingSuspended,
        Self::CurriculumChange,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OfferingAnnounced => "OFFERING_ANNOUNCED",
            Self::OfferingSuspended => "OFFERING_SUSPENDED",
            Self::CurriculumChange => "CURRICULUM_CHANGE",
        }
    }
}

/// One official notice, and the term it was issued in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentNotice {
    issued_in: TermKey,
    effect: NoticeEffect,
}

impl RecentNotice {
    /// Records one notice.
    #[must_use]
    pub const fn new(issued_in: TermKey, effect: NoticeEffect) -> Self {
        Self { issued_in, effect }
    }

    /// The term the notice was issued in.
    #[must_use]
    pub const fn issued_in(&self) -> TermKey {
        self.issued_in
    }

    /// What it said.
    #[must_use]
    pub const fn effect(&self) -> NoticeEffect {
        self.effect
    }
}

/// Section 8.3's 교과목 신설·폐지·대체 status, on the term axis.
///
/// [`Self::Unknown`] is the value an unread official course-change record
/// holds. It is not [`Self::Established`]: a course whose lifecycle nobody has
/// checked and a course somebody checked and found running are different
/// states, and only the second is evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CourseLifecycle {
    /// Nobody has read the official course-change record for this course.
    Unknown,
    /// The official record shows the course running with no pending change.
    Established,
    /// The course is new, first offered in the named term.
    NewFrom(TermKey),
    /// The course is retired from the named term, with no replacement named.
    RetiredFrom(TermKey),
    /// The course is retired from the named term and replaced by another.
    ReplacedFrom {
        /// The first term the retirement applies to.
        from: TermKey,
        /// The course the official record named as the replacement.
        by: CourseCode,
    },
}

impl CourseLifecycle {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Established => "ESTABLISHED",
            Self::NewFrom(_) => "NEW_FROM",
            Self::RetiredFrom(_) => "RETIRED_FROM",
            Self::ReplacedFrom { .. } => "REPLACED_FROM",
        }
    }

    /// The term the change takes effect in, when the record names one.
    #[must_use]
    pub const fn effective_from(&self) -> Option<TermKey> {
        match self {
            Self::Unknown | Self::Established => None,
            Self::NewFrom(term)
            | Self::RetiredFrom(term)
            | Self::ReplacedFrom { from: term, .. } => Some(*term),
        }
    }
}

/// One course's observed history: the terms somebody read, what each held, the
/// notices issued inside them, and the course's official lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseHistory {
    course: CourseCode,
    observations: BTreeMap<TermKey, TermObservation>,
    notices: Vec<RecentNotice>,
    lifecycle: CourseLifecycle,
}

impl CourseHistory {
    /// Starts a history for one course with nothing read yet.
    ///
    /// A course with no observations is exactly section 8.3's
    /// *한 번도 관찰하지 못한* case, and it is representable on purpose: the
    /// forecast has to be able to be handed one and abstain.
    #[must_use]
    pub fn new(course: CourseCode) -> Self {
        Self {
            course,
            observations: BTreeMap::new(),
            notices: Vec::new(),
            lifecycle: CourseLifecycle::Unknown,
        }
    }

    /// Records one term's reading, refusing a second reading of one term.
    ///
    /// Two readings of one term would let a caller weight a term twice, which
    /// is a majority vote with a thumb on it.
    pub fn observe(&mut self, observation: TermObservation) -> Result<(), OfferingError> {
        if self.observations.contains_key(&observation.term()) {
            return Err(OfferingError::DuplicateObservation {
                course: self.course.as_str().to_owned(),
                term: observation.term().canonical_text(),
            });
        }
        self.observations.insert(observation.term(), observation);
        Ok(())
    }

    /// Records one official notice.
    pub fn notice(&mut self, notice: RecentNotice) {
        self.notices.push(notice);
        self.notices
            .sort_by_key(|entry| (entry.issued_in(), entry.effect()));
    }

    /// Records the official lifecycle reading.
    pub fn set_lifecycle(&mut self, lifecycle: CourseLifecycle) {
        self.lifecycle = lifecycle;
    }

    /// The course this history is about.
    #[must_use]
    pub const fn course(&self) -> &CourseCode {
        &self.course
    }

    /// Every term read, in term order.
    pub fn observations(&self) -> impl Iterator<Item = &TermObservation> {
        self.observations.values()
    }

    /// One term's reading, when that term was read.
    #[must_use]
    pub fn observation(&self, term: TermKey) -> Option<&TermObservation> {
        self.observations.get(&term)
    }

    /// Every notice, in issue order.
    #[must_use]
    pub fn notices(&self) -> &[RecentNotice] {
        &self.notices
    }

    /// The official lifecycle reading.
    #[must_use]
    pub const fn lifecycle(&self) -> &CourseLifecycle {
        &self.lifecycle
    }
}
