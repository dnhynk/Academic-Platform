//! Section 25.4's fifth line: the attempt timeline, and its six facets.
//!
//! > 수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.
//!
//! Six facets, read off section 10's own attempt record rather than stored
//! beside it. Section 10's first sentence is what this exists for: *`TakenCourse`
//! 하나로 재수강과 취소를 덮어쓰지 않고 매 시도를 보존한다.*
//!
//! # 예정 is not an attempt, and that is why the timeline has two sources
//!
//! `academic_record::attempt::AttemptStatus::Planned` exists in the schema and
//! **no constructor in `academic-record` produces it** — the two constructors
//! take a confirmed registration or a confirmed transcript row. Section 10 says
//! why: *`PlannedCourse`는 CourseAttempt와도 분리한다*, and a plan candidate is
//! a `PlanScenarioChoice`. So a timeline built only from the attempt ledger
//! would read `예정` as absent on every entry forever, and the facet would be a
//! constant that no test could fail.
//!
//! [`AttemptTimeline::of`] therefore reads **two** sources: the ledger's
//! current attempts and the plan's choices. A planned entry is a
//! `PlanScenarioChoice` and never becomes an attempt on the way in —
//! [`TimelineEntry::planned`] holds no `AttemptId` and there is no method here
//! that turns one entry kind into the other.
//!
//! # What the timeline shows, and what the ledger keeps
//!
//! It reads `AttemptHistory::current`, which is the entries no later entry
//! superseded. A **correction** supersedes, so a corrected attempt is not on
//! the timeline and is still in the ledger — `attempt_history_append_only` in
//! `academic-record` is where that is checked. A **repeat** supersedes nothing:
//! it is a new attempt of the same course, both entries are current, and
//! `attempt_timeline_preserves_six_lifecycle_facets` appends one and requires
//! the earlier entry's six readings to be unchanged afterwards.
//!
//! # Three readings, not two
//!
//! A facet whose input the record does not carry yet reads
//! [`FacetReading::Unknown`] rather than [`FacetReading::Absent`]. Section 30's
//! own line is *`UNKNOWN`: 필요한 정보가 없음*, and an ungraded attempt read as
//! *not S/U* would be the surface answering a question the record has not.

use academic_record::{
    attempt::{AttemptHistory, AttemptStatus, CourseAttempt, RepeatStatus},
    grade::GradeSymbol,
    plan::PlanScenarioChoice,
    policy::RecognitionDecision,
    term::TermKey,
};

use academic_domain::AttemptId;

/// One of section 25.4's six attempt-lifecycle facets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LifecycleFacet {
    /// 예정 — a plan candidate that is not an academic fact.
    Planned,
    /// 수강 — registered, under way, or finished.
    Taken,
    /// 취소 — withdrawn after the deadline, or cancelled before the attempt.
    Cancelled,
    /// 재수강 — a later attempt of a course already attempted, or one such an
    /// attempt displaced.
    Repeated,
    /// S-U — graded outside the average.
    SatisfactoryUnsatisfactory,
    /// 인정 — carried in on transfer, or recognized from elsewhere.
    Recognized,
}

impl LifecycleFacet {
    /// Every facet, in section 25.4's own order.
    ///
    /// `attempt_timeline_preserves_six_lifecycle_facets` splits section 25.4's
    /// own line on its own slashes and compares the pieces with
    /// [`LifecycleFacet::spec_word`] position by position and as sets in both
    /// directions.
    pub const ALL: [Self; 6] = [
        Self::Planned,
        Self::Taken,
        Self::Cancelled,
        Self::Repeated,
        Self::SatisfactoryUnsatisfactory,
        Self::Recognized,
    ];

    /// The word section 25.4 spells this facet with.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::Planned => "예정",
            Self::Taken => "수강",
            Self::Cancelled => "취소",
            Self::Repeated => "재수강",
            Self::SatisfactoryUnsatisfactory => "S-U",
            Self::Recognized => "인정",
        }
    }
}

/// What one entry says about one facet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FacetReading {
    /// The record says this facet applies.
    Present,
    /// The record says this facet does not apply.
    Absent,
    /// The record does not say yet. Not a synonym for `Absent`.
    Unknown,
}

/// Where one timeline entry came from.
///
/// Private, and there is no accessor returning it: the two kinds are told apart
/// by [`TimelineEntry::attempt`], which is `None` for a plan candidate. A
/// public arm would be a second place to write the `예정`/`수강` distinction.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EntryKind {
    /// A `PlanScenarioChoice`. Carries no attempt identity, no grade and no
    /// status, because a plan has no answer for any of them.
    Planned,
    /// A `CourseAttempt`, with the four fields the six facets read.
    Attempted {
        attempt: AttemptId,
        status: AttemptStatus,
        repeat: RepeatStatus,
        grade: Option<GradeSymbol>,
        recognition: RecognitionDecision,
    },
}

/// One row of the attempt timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    course_code: String,
    term: TermKey,
    kind: EntryKind,
}

impl TimelineEntry {
    /// Reads a plan candidate as a `예정` row.
    ///
    /// Takes the `P2-U4` plan choice itself, so the row is a reading of the
    /// plan rather than a second copy of it. Nothing here can turn the row back
    /// into a `CourseAttempt`: it holds no attempt identity to build one from.
    #[must_use]
    pub fn planned(choice: &PlanScenarioChoice) -> Self {
        Self {
            course_code: choice.course_code().to_owned(),
            term: choice.intended_term(),
            kind: EntryKind::Planned,
        }
    }

    /// Reads one attempt of record as a timeline row.
    #[must_use]
    pub fn attempted(attempt: &CourseAttempt) -> Self {
        Self {
            course_code: attempt.course_code().to_owned(),
            term: attempt.term(),
            kind: EntryKind::Attempted {
                attempt: attempt.id(),
                status: attempt.status(),
                repeat: attempt.repeat_status(),
                grade: attempt.grade(),
                recognition: attempt.recognition(),
            },
        }
    }

    /// The course this row is about.
    #[must_use]
    pub fn course_code(&self) -> &str {
        &self.course_code
    }

    /// The term it falls in, or is intended for.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The attempt this row reads, or `None` for a plan candidate.
    #[must_use]
    pub const fn attempt(&self) -> Option<AttemptId> {
        match self.kind {
            EntryKind::Planned => None,
            EntryKind::Attempted { attempt, .. } => Some(attempt),
        }
    }

    /// What this row says about one facet.
    ///
    /// Six independent readings over four record fields. The `match` on the
    /// facet has no wildcard arm and neither does any `match` inside it, so a
    /// seventh facet or a ninth attempt status stops this crate compiling.
    #[must_use]
    pub const fn facet(&self, facet: LifecycleFacet) -> FacetReading {
        let EntryKind::Attempted {
            status,
            repeat,
            grade,
            recognition,
            ..
        } = self.kind
        else {
            // A plan candidate is `예정` and is nothing else. It carries no
            // status, no grade and no recognition decision to read.
            return match facet {
                LifecycleFacet::Planned => FacetReading::Present,
                LifecycleFacet::Taken
                | LifecycleFacet::Cancelled
                | LifecycleFacet::Repeated
                | LifecycleFacet::SatisfactoryUnsatisfactory
                | LifecycleFacet::Recognized => FacetReading::Absent,
            };
        };
        match facet {
            LifecycleFacet::Planned => match status {
                AttemptStatus::Planned => FacetReading::Present,
                AttemptStatus::Registered
                | AttemptStatus::InProgress
                | AttemptStatus::Completed
                | AttemptStatus::Withdrawn
                | AttemptStatus::Cancelled
                | AttemptStatus::Transferred
                | AttemptStatus::Recognized => FacetReading::Absent,
            },
            LifecycleFacet::Taken => match status {
                AttemptStatus::Registered
                | AttemptStatus::InProgress
                | AttemptStatus::Completed => FacetReading::Present,
                AttemptStatus::Planned
                | AttemptStatus::Withdrawn
                | AttemptStatus::Cancelled
                | AttemptStatus::Transferred
                | AttemptStatus::Recognized => FacetReading::Absent,
            },
            LifecycleFacet::Cancelled => match status {
                AttemptStatus::Withdrawn | AttemptStatus::Cancelled => FacetReading::Present,
                AttemptStatus::Planned
                | AttemptStatus::Registered
                | AttemptStatus::InProgress
                | AttemptStatus::Completed
                | AttemptStatus::Transferred
                | AttemptStatus::Recognized => FacetReading::Absent,
            },
            LifecycleFacet::Repeated => match repeat {
                RepeatStatus::Repeat | RepeatStatus::Replaced => FacetReading::Present,
                RepeatStatus::Original => FacetReading::Absent,
                // 재수강 handling does not reach this course, which is not the
                // same as knowing it was not repeated.
                RepeatStatus::NotApplicable => FacetReading::Unknown,
            },
            LifecycleFacet::SatisfactoryUnsatisfactory => match grade {
                Some(GradeSymbol::S | GradeSymbol::U) => FacetReading::Present,
                Some(_) => FacetReading::Absent,
                // No grade recorded yet. Section 30: an absent input is not an
                // answer.
                None => FacetReading::Unknown,
            },
            LifecycleFacet::Recognized => match status {
                AttemptStatus::Transferred | AttemptStatus::Recognized => match recognition {
                    RecognitionDecision::Recognized => FacetReading::Present,
                    RecognitionDecision::NotRecognized => FacetReading::Absent,
                    // `GATE-38-006`: the recognition decision is a user input.
                    RecognitionDecision::Undecided => FacetReading::Unknown,
                },
                AttemptStatus::Planned
                | AttemptStatus::Registered
                | AttemptStatus::InProgress
                | AttemptStatus::Completed
                | AttemptStatus::Withdrawn
                | AttemptStatus::Cancelled => FacetReading::Absent,
            },
        }
    }
}

/// Section 25.4's attempt timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptTimeline {
    entries: Vec<TimelineEntry>,
}

impl AttemptTimeline {
    /// Reads the ledger and the plan into one ordered timeline.
    ///
    /// Ordered by term and then by course code, so the sequence is a function
    /// of the record rather than of the order the caller happened to build it
    /// in. There is no `push`, no `&mut` accessor and no removal: a timeline is
    /// read from its two sources or it does not exist.
    #[must_use]
    pub fn of(history: &AttemptHistory, planned: &[PlanScenarioChoice]) -> Self {
        let mut entries: Vec<TimelineEntry> = history
            .current()
            .into_iter()
            .map(TimelineEntry::attempted)
            .chain(planned.iter().map(TimelineEntry::planned))
            .collect();
        entries.sort_by(|left, right| {
            left.term()
                .canonical_text()
                .cmp(&right.term().canonical_text())
                .then_with(|| left.course_code().cmp(right.course_code()))
                .then_with(|| left.attempt().cmp(&right.attempt()))
        });
        Self { entries }
    }

    /// The rows, in timeline order.
    #[must_use]
    pub fn entries(&self) -> &[TimelineEntry] {
        &self.entries
    }

    /// The row for one attempt, when the timeline holds it.
    #[must_use]
    pub fn entry_for(&self, attempt: AttemptId) -> Option<&TimelineEntry> {
        self.entries
            .iter()
            .find(|entry| entry.attempt() == Some(attempt))
    }
}
