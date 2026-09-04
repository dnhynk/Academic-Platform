//! Section 25.2's `Home / Today`: the eight priority groups, in order.
//!
//! `P2-X1` fixed the frame — every route in the section 25.1 tree has a titled
//! view with a breadcrumb, at least one section and the evidence drawer, and
//! each section names the task that fills it. This crate is `P2-X2` filling the
//! one it named `Today`.
//!
//! # What this is not evidence for
//!
//! **No window opens.** `P2-X1` merged with no Tauri runtime linked and that
//! decision is still open under the user gate. Nothing here depends on a window
//! and nothing here is evidence that one exists: this crate is a set of typed
//! records and the rules between them, checked by compiling it, running its
//! tests, or reading its source. `packages/ui/src/home.ts` is the shell half,
//! and it adds that opening `/` yields sections naming section 25.2's own eight
//! groups instead of a promise that a later task will supply some. That is a
//! structure, not a rendering.
//!
//! **An upcoming use is a value the caller supplies.** [`UpcomingUse`] refuses
//! an occasion that is not strictly after the reference instant, and that is
//! all it can check. It is not a claim that the occasion is on anybody's real
//! timetable; the surface that composes a card is what knows that, and this
//! crate has no edge to it.
//!
//! # What the eight groups are, and where the order comes from
//!
//! [`HomeGroup::ALL`] is section 25.2's own numbered list and
//! [`HomeGroup::spec_words`] is each item's own text. The order is not a layout
//! decision taken here: `home_group_order_is_stable_one_to_eight` parses the
//! numbered list out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`
//! and compares the two position by position and as sets in both directions, so
//! a group renamed, reordered, added or dropped fails against the document
//! rather than against a second list written beside it.
//!
//! # It persists nothing
//!
//! No `academic-store` edge, no `academic-vault` edge, no migration number. It
//! reads no clock: every instant it compares arrives as an argument, which is
//! also why its tests can name the instants they assert against.

#![forbid(unsafe_code)]

mod alert;
mod card;
mod freshness;
mod occasion;
mod permission;
mod prerequisite;
mod screen;

pub use alert::{AlertBucket, DayWindow, GroupedAlerts};
pub use card::{KnowledgeNeed, NextStep, OfficialCondition, OpenItem, OpenItemKind};
pub use freshness::FreshnessAlert;
pub use occasion::{ScheduledItem, ScheduledOccasion, UpcomingUse};
pub use permission::RecordingPermission;
pub use prerequisite::{EstimatedMinutes, PrerequisiteBrief, PrerequisiteItem};
pub use screen::{HomeCard, HomeScreen, HomeSection};

/// Everything this surface refuses.
///
/// Each variant is one of section 25.2's own rules. There is no catch-all arm:
/// a refusal this crate cannot name is a refusal it does not make.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HomeError {
    /// Section 25.2's second line bounds the brief at `최대 1–3개`.
    #[error(
        "a prerequisite brief holds {count} items, and section 25.2 allows {LOWEST_BRIEF} to {HIGHEST_BRIEF}"
    )]
    PrerequisiteCountOutOfBounds {
        /// How many were offered.
        count: usize,
    },
    /// An estimate of no time at all is not an estimate.
    #[error("a prerequisite item was offered no estimated time")]
    EstimateIsZero,
    /// Section 25.2's eighth line: `실제 upcoming use가 있을 때만`.
    #[error("an occasion at {} is not upcoming at {}", occasion_at.value(), reference.value())]
    OccasionIsNotUpcoming {
        /// When the occasion falls.
        occasion_at: academic_domain::TimestampMillis,
        /// The instant it was offered against.
        reference: academic_domain::TimestampMillis,
    },
    /// A day window whose end precedes its own start buckets nothing sensibly.
    #[error("a day window ends at {} before it starts at {}", end.value(), start.value())]
    DayWindowEndsBeforeItStarts {
        /// The window's reference instant.
        start: academic_domain::TimestampMillis,
        /// The instant the window claims today ends at.
        end: academic_domain::TimestampMillis,
    },
}

/// The lower bound section 25.2's second line puts on a prerequisite brief.
pub const LOWEST_BRIEF: usize = 1;

/// The upper bound section 25.2's second line puts on a prerequisite brief.
///
/// Both bounds are read back out of the specification by
/// `prerequisite_count_is_within_one_to_three_with_reason_and_time`, which
/// splits `최대 1–3개` on the document's own en dash. Neither is a number this
/// crate chose.
pub const HIGHEST_BRIEF: usize = 3;

/// The eight priority groups section 25.2 composes one screen from.
///
/// The order is the specification's own numbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HomeGroup {
    /// Section 25.2's first line: today's real schedule.
    TodaysSchedule,
    /// Section 25.2's second line: the minimum prerequisite before class.
    MinimumPrerequisite,
    /// Section 25.2's third line: the recording permission status.
    RecordingPermissionStatus,
    /// Section 25.2's fourth line: the user's own open questions and marks.
    OpenQuestionAndMarkMoment,
    /// Section 25.2's fifth line: what blocks the current project.
    ProjectBlockingKnowledgeNeed,
    /// Section 25.2's sixth line: official conditions and stale-source warnings.
    OfficialConditionAndStaleWarning,
    /// Section 25.2's seventh line: the active critical path's next step.
    CriticalPathNextStep,
    /// Section 25.2's eighth line: a freshness alert with an upcoming use.
    ConceptFreshnessAlert,
}

impl HomeGroup {
    /// Exhaustive listing, in section 25.2's own numbered order.
    pub const ALL: [Self; 8] = [
        Self::TodaysSchedule,
        Self::MinimumPrerequisite,
        Self::RecordingPermissionStatus,
        Self::OpenQuestionAndMarkMoment,
        Self::ProjectBlockingKnowledgeNeed,
        Self::OfficialConditionAndStaleWarning,
        Self::CriticalPathNextStep,
        Self::ConceptFreshnessAlert,
    ];

    /// How many groups there are.
    ///
    /// Derived from [`Self::ALL`] rather than written, so the two cannot
    /// disagree, and [`Self::ALL`] itself is compared against the document.
    pub const COUNT: usize = Self::ALL.len();

    /// The specification's own words for this group, without its number.
    ///
    /// Compared line for line against section 25.2's numbered list by
    /// `home_group_order_is_stable_one_to_eight`. A paraphrase fails.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::TodaysSchedule => "오늘 실제 일정: 수업, assessment deadline, project event.",
            Self::MinimumPrerequisite => {
                "수업 전 최소 prerequisite: 최대 1–3개, “왜 지금”과 예상 시간."
            }
            Self::RecordingPermissionStatus => {
                "녹음 permission 상태: `허용`, `조건부`, `확인 필요`, `금지`."
            }
            Self::OpenQuestionAndMarkMoment => "사용자가 직접 남긴 열린 질문과 Mark Moment review.",
            Self::ProjectBlockingKnowledgeNeed => "현재 project를 막는 가장 가까운 knowledge need.",
            Self::OfficialConditionAndStaleWarning => {
                "deadline이 있는 공식 학사 condition과 stale official data 경고."
            }
            Self::CriticalPathNextStep => "활성 Critical Path의 사용자 선택 다음 단계.",
            Self::ConceptFreshnessAlert => {
                "중요한 concept의 freshness 알림은 실제 upcoming use가 있을 때만."
            }
        }
    }

    /// This group's number in section 25.2's list, counting from one.
    ///
    /// Written out rather than derived from [`Self::ALL`], for the reason
    /// `P2-X1`'s view registry is written out rather than derived from its
    /// route manifest: a derived number would agree with the listing because it
    /// *was* the listing, and the comparison would assert nothing. Written out,
    /// the two are independent enumerations, and
    /// `home_group_order_is_stable_one_to_eight` compares them against each
    /// other and against the document's own numbering, all three ways.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::TodaysSchedule => 1,
            Self::MinimumPrerequisite => 2,
            Self::RecordingPermissionStatus => 3,
            Self::OpenQuestionAndMarkMoment => 4,
            Self::ProjectBlockingKnowledgeNeed => 5,
            Self::OfficialConditionAndStaleWarning => 6,
            Self::CriticalPathNextStep => 7,
            Self::ConceptFreshnessAlert => 8,
        }
    }

    /// The `SCREAMING_SNAKE_CASE` identifier the shell shows this group under.
    ///
    /// `packages/ui/src/home.ts` holds the same eight and
    /// `the_home_sections_are_the_crates_own` compares them, so a group renamed
    /// here fails there rather than drifting.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TodaysSchedule => "TODAYS_SCHEDULE",
            Self::MinimumPrerequisite => "MINIMUM_PREREQUISITE",
            Self::RecordingPermissionStatus => "RECORDING_PERMISSION_STATUS",
            Self::OpenQuestionAndMarkMoment => "OPEN_QUESTION_AND_MARK_MOMENT",
            Self::ProjectBlockingKnowledgeNeed => "PROJECT_BLOCKING_KNOWLEDGE_NEED",
            Self::OfficialConditionAndStaleWarning => "OFFICIAL_CONDITION_AND_STALE_WARNING",
            Self::CriticalPathNextStep => "CRITICAL_PATH_NEXT_STEP",
            Self::ConceptFreshnessAlert => "CONCEPT_FRESHNESS_ALERT",
        }
    }
}
