//! The two lanes of section 22, and the two UI sections they render into.
//!
//! Section 22.1: *`deterministicResults`와 `projections`는 UI section과 데이터
//! type 모두에서 분리한다.* Both halves of that sentence are held here.
//!
//! # The data types are separate
//!
//! [`crate::deterministic::DeterministicResults`] and
//! [`crate::projected::ProjectedResults`] share no field, no constructor and no
//! conversion. There is no `From` between them, no method on either that
//! returns the other, and no type that holds a value of both except
//! [`crate::scenario::PlanScenario`], which holds them in two named positions.
//! A projected value cannot be read as a deterministic result because the
//! deterministic type has no field with a projected type in it, and the reverse
//! for the same reason.
//!
//! # The UI sections are separate
//!
//! [`SectionView`] is an enumeration whose two arms borrow *different types*.
//! A renderer that put a projection under the deterministic heading would have
//! to name [`crate::deterministic::DeterministicResults`] and hand it a value
//! it has no field for. The section is therefore not a label a caller chooses
//! for a value: it is a consequence of which value the caller holds.
//!
//! # The two lists are the design document's own
//!
//! [`DETERMINISTIC_LANE`] is section 22.2's bullets and [`PROJECTED_LANE`] is
//! section 22.3's, each carried verbatim in `spec_phrase` and compared against
//! the document in both directions by
//! `deterministic_and_projected_are_separate_types_and_sections`. Neither
//! constant states a count: the count is whatever the document lists, and the
//! test fails if a bullet is added, removed or moved between the two sections.

use crate::{deterministic::DeterministicResults, projected::ProjectedResults};

/// One bullet of section 22.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeterministicItem {
    /// `신청학점과 과목별 학점`.
    RequestedAndPerCourseCredits,
    /// `공식 시간표 충돌`.
    OfficialScheduleConflict,
    /// `공식 선수과목·수강 제한 충족 여부`.
    OfficialPrerequisiteAndEnrolmentLimit,
    /// `이수한다고 가정했을 때의 졸업 rule contribution`.
    RuleContributionUnderCompletionAssumption,
    /// `required/elective/category allocation과 proof`.
    AllocationAndProof,
    /// `후속 Course의 공식 prerequisite unlock`.
    DownstreamOfficialUnlock,
    /// `GPA scenario는 사용자가 명시한 grade 가정에 한해서만 계산`.
    GpaUnderStatedGradeAssumptions,
}

/// Section 22.2's bullets, in the document's own order.
pub const DETERMINISTIC_LANE: [DeterministicItem; 7] = [
    DeterministicItem::RequestedAndPerCourseCredits,
    DeterministicItem::OfficialScheduleConflict,
    DeterministicItem::OfficialPrerequisiteAndEnrolmentLimit,
    DeterministicItem::RuleContributionUnderCompletionAssumption,
    DeterministicItem::AllocationAndProof,
    DeterministicItem::DownstreamOfficialUnlock,
    DeterministicItem::GpaUnderStatedGradeAssumptions,
];

impl DeterministicItem {
    /// The bullet section 22.2 writes, verbatim.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::RequestedAndPerCourseCredits => "신청학점과 과목별 학점",
            Self::OfficialScheduleConflict => "공식 시간표 충돌",
            Self::OfficialPrerequisiteAndEnrolmentLimit => "공식 선수과목·수강 제한 충족 여부",
            Self::RuleContributionUnderCompletionAssumption => {
                "이수한다고 가정했을 때의 졸업 rule contribution"
            }
            Self::AllocationAndProof => "required/elective/category allocation과 proof",
            Self::DownstreamOfficialUnlock => "후속 Course의 공식 prerequisite unlock",
            Self::GpaUnderStatedGradeAssumptions => {
                "GPA scenario는 사용자가 명시한 grade 가정에 한해서만 계산"
            }
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestedAndPerCourseCredits => "REQUESTED_AND_PER_COURSE_CREDITS",
            Self::OfficialScheduleConflict => "OFFICIAL_SCHEDULE_CONFLICT",
            Self::OfficialPrerequisiteAndEnrolmentLimit => {
                "OFFICIAL_PREREQUISITE_AND_ENROLMENT_LIMIT"
            }
            Self::RuleContributionUnderCompletionAssumption => {
                "RULE_CONTRIBUTION_UNDER_COMPLETION_ASSUMPTION"
            }
            Self::AllocationAndProof => "ALLOCATION_AND_PROOF",
            Self::DownstreamOfficialUnlock => "DOWNSTREAM_OFFICIAL_UNLOCK",
            Self::GpaUnderStatedGradeAssumptions => "GPA_UNDER_STATED_GRADE_ASSUMPTIONS",
        }
    }
}

/// One bullet of section 22.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectedItem {
    /// `syllabus 기반 concept exposure opportunity`.
    SyllabusExposureOpportunity,
    /// `assignment 기반 practice opportunity`.
    AssignmentPracticeOpportunity,
    /// `assessment opportunity`.
    AssessmentOpportunity,
    /// `project/career relevance`.
    ProjectCareerRelevance,
    /// `workload range와 review bias`.
    WorkloadRangeAndReviewBias,
    /// `Critical Path coverage 가능성`.
    CriticalPathCoverage,
    /// `후속 비공식 권장 지식의 readiness`.
    InformalDownstreamReadiness,
}

/// Section 22.3's bullets, in the document's own order.
pub const PROJECTED_LANE: [ProjectedItem; 7] = [
    ProjectedItem::SyllabusExposureOpportunity,
    ProjectedItem::AssignmentPracticeOpportunity,
    ProjectedItem::AssessmentOpportunity,
    ProjectedItem::ProjectCareerRelevance,
    ProjectedItem::WorkloadRangeAndReviewBias,
    ProjectedItem::CriticalPathCoverage,
    ProjectedItem::InformalDownstreamReadiness,
];

impl ProjectedItem {
    /// The bullet section 22.3 writes, verbatim.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::SyllabusExposureOpportunity => "syllabus 기반 concept exposure opportunity",
            Self::AssignmentPracticeOpportunity => "assignment 기반 practice opportunity",
            Self::AssessmentOpportunity => "assessment opportunity",
            Self::ProjectCareerRelevance => "project/career relevance",
            Self::WorkloadRangeAndReviewBias => "workload range와 review bias",
            Self::CriticalPathCoverage => "Critical Path coverage 가능성",
            Self::InformalDownstreamReadiness => "후속 비공식 권장 지식의 readiness",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SyllabusExposureOpportunity => "SYLLABUS_EXPOSURE_OPPORTUNITY",
            Self::AssignmentPracticeOpportunity => "ASSIGNMENT_PRACTICE_OPPORTUNITY",
            Self::AssessmentOpportunity => "ASSESSMENT_OPPORTUNITY",
            Self::ProjectCareerRelevance => "PROJECT_CAREER_RELEVANCE",
            Self::WorkloadRangeAndReviewBias => "WORKLOAD_RANGE_AND_REVIEW_BIAS",
            Self::CriticalPathCoverage => "CRITICAL_PATH_COVERAGE",
            Self::InformalDownstreamReadiness => "INFORMAL_DOWNSTREAM_READINESS",
        }
    }
}

/// One item of section 22, on whichever side of the split it sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LaneItem {
    /// A section 22.2 bullet.
    Deterministic(DeterministicItem),
    /// A section 22.3 bullet.
    Projected(ProjectedItem),
}

impl LaneItem {
    /// Which UI section this item renders into.
    ///
    /// A total `match` over the arm rather than a lookup, so the section of an
    /// item is decided by which lane it belongs to and by nothing else.
    #[must_use]
    pub const fn section(self) -> UiSection {
        match self {
            Self::Deterministic(_) => UiSection::DeterministicResults,
            Self::Projected(_) => UiSection::Projections,
        }
    }

    /// The design document's own phrase for this item.
    #[must_use]
    pub const fn spec_phrase(self) -> &'static str {
        match self {
            Self::Deterministic(item) => item.spec_phrase(),
            Self::Projected(item) => item.spec_phrase(),
        }
    }
}

/// The two sections section 22.1 separates.
///
/// Named after the two keys of its `PlanScenario` block rather than after the
/// two words of its prose, so the section a reader sees and the field a plan
/// carries have one name between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UiSection {
    /// Section 22.1's `deterministicResults`.
    DeterministicResults,
    /// Section 22.1's `projections`.
    Projections,
}

/// The two, in section 22.1's own order.
pub const UI_SECTIONS: [UiSection; 2] = [UiSection::DeterministicResults, UiSection::Projections];

impl UiSection {
    /// The key section 22.1's YAML block writes for this section.
    #[must_use]
    pub const fn spec_key(self) -> &'static str {
        match self {
            Self::DeterministicResults => "deterministicResults",
            Self::Projections => "projections",
        }
    }

    /// Every item this section renders, in the design document's order.
    #[must_use]
    pub fn items(self) -> Vec<LaneItem> {
        match self {
            Self::DeterministicResults => DETERMINISTIC_LANE
                .into_iter()
                .map(LaneItem::Deterministic)
                .collect(),
            Self::Projections => PROJECTED_LANE
                .into_iter()
                .map(LaneItem::Projected)
                .collect(),
        }
    }
}

/// One rendered section of a plan.
///
/// The two arms borrow two different types. That is the UI half of section
/// 22.1's split: a caller cannot put a projection under the deterministic
/// heading, because the arm that carries the deterministic heading has no
/// position a projected value fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionView<'a> {
    /// Section 22.1's `deterministicResults`.
    DeterministicResults(&'a DeterministicResults),
    /// Section 22.1's `projections`.
    Projections(&'a ProjectedResults),
}

impl SectionView<'_> {
    /// Which section this view is.
    #[must_use]
    pub const fn section(&self) -> UiSection {
        match self {
            Self::DeterministicResults(_) => UiSection::DeterministicResults,
            Self::Projections(_) => UiSection::Projections,
        }
    }
}
