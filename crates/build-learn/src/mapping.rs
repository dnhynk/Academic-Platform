//! Section 21: `매핑은 Course title keyword matching이 아니다`.
//!
//! ```text
//! ProjectFinding
//!   → required Concept/Competency
//!   → prerequisite neighborhood
//!   → CourseRevision DESIGNED_TO_TEACH coverage
//!   → actual Offering syllabus coverage
//!   → user's previous Lecture/Assessment evidence
//! ```
//!
//! ## A course's title is not a coverage claim, and that is two types
//!
//! [`DesignedCoverage`] is what `P2-U1`'s [`academic_curriculum::CourseRevision`]
//! says a course revision is *designed* to teach; [`ActualCoverage`] is what a
//! particular [`academic_curriculum::CourseOffering`] was *observed* to cover,
//! and it carries the evidence that observed it. They are two types with no
//! conversion between them in either direction, so a designed coverage cannot be
//! read where an actual one is required. `데이터베이스` covering every isolation
//! and replication competency because of its name is not a claim this crate can
//! represent: `DesignedCoverage::of` reads
//! [`academic_curriculum::CourseRevision::designed_concept_coverage`], which is a
//! list of [`academic_domain::EntityId`], and there is no constructor taking a
//! title.
//!
//! ## The six statuses
//!
//! [`MappingStatus`] is section 21.2's six and no seventh, and
//! [`MappingStatus::for_evidence`] is total over the evidence that decides them
//! with no wildcard arm. Two of the six require an [`ActualCoverage`] and are
//! unreachable without one; that is REQ-36-038's `Mapping never confuses
//! existence of a Course with actual Offering coverage`, held as an absence of
//! values rather than as a check.
//!
//! ## A course is one acquisition channel
//!
//! Section 21.3's `학교 과목은 하나의 acquisition channel이다`. `P2-N6` already
//! owns that: [`academic_critical_path::AcquisitionOption::course`] hands out
//! [`academic_critical_path::Opportunity`] values and has no function returning a
//! mastery. This crate adds the comparison section 21.3 asks for and nothing
//! else: [`ChannelComparison`] keeps the immediate-gap effect and the breadth
//! effect as **two** values, because `양쪽 효과를 구분한다`, and there is no
//! function on it that returns one number.

use std::collections::BTreeSet;

use academic_curriculum::{CourseOffering, CourseRevision, OfferingStatus};
use academic_domain::{CourseId, EntityId, EvidenceId, OfferingId};
use serde::{Deserialize, Serialize};

use crate::{BuildLearnError, text::NonEmptyText};

/// What a `CourseRevision` is designed to teach.
///
/// `P2-U1`'s own list, read and not re-derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignedCoverage {
    course: CourseId,
    concepts: Vec<EntityId>,
    competencies: Vec<EntityId>,
}

impl DesignedCoverage {
    /// Reads the canonical coverage off a course revision.
    #[must_use]
    pub fn of(revision: &CourseRevision) -> Self {
        Self {
            course: revision.course(),
            concepts: revision.designed_concept_coverage().to_vec(),
            competencies: revision.designed_competency_coverage().to_vec(),
        }
    }

    /// The course this is about.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The concepts the revision is designed to teach.
    #[must_use]
    pub fn concepts(&self) -> &[EntityId] {
        &self.concepts
    }

    /// The competencies the revision is designed to teach.
    #[must_use]
    pub fn competencies(&self) -> &[EntityId] {
        &self.competencies
    }

    /// Whether the revision is designed to teach `subject`.
    #[must_use]
    pub fn designs(&self, subject: EntityId) -> bool {
        self.concepts.contains(&subject) || self.competencies.contains(&subject)
    }
}

/// Which of section 21.1's four evidence stages observed an actual coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageEvidenceKind {
    /// `syllabus`.
    Syllabus,
    /// `lecture`.
    Lecture,
    /// `assignment`.
    Assignment,
    /// `assessment`.
    Assessment,
}

/// The four, in section 21.1's own order.
pub const COVERAGE_EVIDENCE_KINDS: [CoverageEvidenceKind; 4] = [
    CoverageEvidenceKind::Syllabus,
    CoverageEvidenceKind::Lecture,
    CoverageEvidenceKind::Assignment,
    CoverageEvidenceKind::Assessment,
];

impl CoverageEvidenceKind {
    /// The design document's own word.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::Syllabus => "syllabus",
            Self::Lecture => "lecture",
            Self::Assignment => "assignment",
            Self::Assessment => "assessment",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syllabus => "SYLLABUS",
            Self::Lecture => "LECTURE",
            Self::Assignment => "ASSIGNMENT",
            Self::Assessment => "ASSESSMENT",
        }
    }
}

/// What a particular offering was observed to cover, and what observed it.
///
/// Private fields, one constructor, no `Default`. `sightings` is never empty:
/// an actual coverage with no evidence is exactly the claim section 21.1
/// refuses, so it has no value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActualCoverage {
    offering: OfferingId,
    subject: EntityId,
    sightings: Vec<(CoverageEvidenceKind, EvidenceId)>,
    upcoming: bool,
}

impl ActualCoverage {
    /// Records an observed coverage.
    ///
    /// `upcoming` is section 21.2's `실제 upcoming coverage 근거` — whether the
    /// sighted coverage is still ahead of the user in this offering.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::CoverageHasNoEvidence`] when nothing observed it.
    pub fn observed(
        offering: &CourseOffering,
        subject: EntityId,
        sightings: Vec<(CoverageEvidenceKind, EvidenceId)>,
        upcoming: bool,
    ) -> Result<Self, BuildLearnError> {
        if sightings.is_empty() {
            return Err(BuildLearnError::CoverageHasNoEvidence(subject.to_string()));
        }
        Ok(Self {
            offering: offering.id(),
            subject,
            sightings,
            upcoming,
        })
    }

    /// The offering this is about.
    #[must_use]
    pub const fn offering(&self) -> OfferingId {
        self.offering
    }

    /// The concept or competency covered.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// What observed it. Never empty.
    #[must_use]
    pub fn sightings(&self) -> &[(CoverageEvidenceKind, EvidenceId)] {
        &self.sightings
    }

    /// Which of the four stages observed it, deduplicated and ordered.
    #[must_use]
    pub fn stages(&self) -> Vec<CoverageEvidenceKind> {
        let found: BTreeSet<CoverageEvidenceKind> =
            self.sightings.iter().map(|(kind, _)| *kind).collect();
        found.into_iter().collect()
    }

    /// Whether the coverage is still ahead of the user in this offering.
    #[must_use]
    pub const fn is_upcoming(&self) -> bool {
        self.upcoming
    }
}

/// The user's standing with respect to one course.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EnrolmentStanding {
    /// `현재 수강 중`.
    Enrolled,
    /// `과목은 이수했`.
    Completed,
    /// Neither.
    Neither,
}

/// How strong the user's own evidence on the subject is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersonalEvidenceStanding {
    /// `해당 concept evidence가 약함`.
    Weak,
    /// Enough that the mapping is not about a shortfall.
    Sufficient,
}

/// Section 21.2's six mapping results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingStatus {
    /// `현재 수강 중이며 실제 upcoming coverage 근거가 있음`.
    CanBeSupportedByCurrentCourse,
    /// `과목은 이수했지만 해당 concept evidence가 약함`.
    PreviouslyTakenEvidenceWeak,
    /// `공식 개설된 Offering이 관련 coverage를 가짐`.
    ConfirmedNextTerm,
    /// `과거 패턴만 있음`.
    HistoricallyAvailable,
    /// `학교 강의로 직접 충족하기 어려움`.
    NoDirectCourseMatch,
    /// `짧은 project experiment가 더 직접적`.
    ExternalOrExperimentBetter,
}

/// The six, in section 21.2's own bullet order.
pub const MAPPING_STATUSES: [MappingStatus; 6] = [
    MappingStatus::CanBeSupportedByCurrentCourse,
    MappingStatus::PreviouslyTakenEvidenceWeak,
    MappingStatus::ConfirmedNextTerm,
    MappingStatus::HistoricallyAvailable,
    MappingStatus::NoDirectCourseMatch,
    MappingStatus::ExternalOrExperimentBetter,
];

impl MappingStatus {
    /// Section 21.2's own bullet text for this status.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::CanBeSupportedByCurrentCourse => {
                "현재 수강 중이며 실제 upcoming coverage 근거가 있음"
            }
            Self::PreviouslyTakenEvidenceWeak => "과목은 이수했지만 해당 concept evidence가 약함",
            Self::ConfirmedNextTerm => "공식 개설된 Offering이 관련 coverage를 가짐",
            Self::HistoricallyAvailable => "과거 패턴만 있음",
            Self::NoDirectCourseMatch => "학교 강의로 직접 충족하기 어려움",
            Self::ExternalOrExperimentBetter => "짧은 project experiment가 더 직접적",
        }
    }

    /// Stable spelling. Section 21.2's own identifiers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanBeSupportedByCurrentCourse => "CAN_BE_SUPPORTED_BY_CURRENT_COURSE",
            Self::PreviouslyTakenEvidenceWeak => "PREVIOUSLY_TAKEN_EVIDENCE_WEAK",
            Self::ConfirmedNextTerm => "CONFIRMED_NEXT_TERM",
            Self::HistoricallyAvailable => "HISTORICALLY_AVAILABLE",
            Self::NoDirectCourseMatch => "NO_DIRECT_COURSE_MATCH",
            Self::ExternalOrExperimentBetter => "EXTERNAL_OR_EXPERIMENT_BETTER",
        }
    }

    /// Whether this status can only be published with an [`ActualCoverage`].
    ///
    /// The two that assert a particular offering covers the subject. Stated as a
    /// property of the enumeration so a seventh status added later has to answer
    /// it, rather than as a list of two names at the one call site.
    #[must_use]
    pub const fn requires_actual_coverage(self) -> bool {
        match self {
            Self::CanBeSupportedByCurrentCourse | Self::ConfirmedNextTerm => true,
            Self::PreviouslyTakenEvidenceWeak
            | Self::HistoricallyAvailable
            | Self::NoDirectCourseMatch
            | Self::ExternalOrExperimentBetter => false,
        }
    }
}

/// One link of section 21.1's chain, published with the evidence for it.
///
/// Private fields, one producer, no `Default`. A mapping is bound to the
/// designed coverage it was reached through and, for the two statuses that
/// require it, to the actual coverage that observed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseProjectMapping {
    subject: EntityId,
    designed: Option<DesignedCoverage>,
    actual: Option<ActualCoverage>,
    status: MappingStatus,
    reason: NonEmptyText,
}

impl CourseProjectMapping {
    /// Publishes one mapping.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::StatusRequiresActualCoverage`] when a status that
    /// asserts a particular offering covers the subject is offered without one,
    /// and [`BuildLearnError::CoverageIsAboutAnotherSubject`] when a coverage
    /// names a different subject. The first is REQ-21-014 and REQ-36-038: the
    /// existence of a course is not a guarantee that a term's offering covers
    /// anything.
    pub fn publish(
        subject: EntityId,
        designed: Option<DesignedCoverage>,
        actual: Option<ActualCoverage>,
        status: MappingStatus,
        reason: NonEmptyText,
    ) -> Result<Self, BuildLearnError> {
        if let Some(coverage) = &actual
            && coverage.subject() != subject
        {
            return Err(BuildLearnError::CoverageIsAboutAnotherSubject {
                expected: subject.to_string(),
                found: coverage.subject().to_string(),
            });
        }
        if status.requires_actual_coverage() && actual.is_none() {
            return Err(BuildLearnError::StatusRequiresActualCoverage(
                status.as_str(),
            ));
        }
        Ok(Self {
            subject,
            designed,
            actual,
            status,
            reason,
        })
    }

    /// The concept or competency mapped.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// The course's canonical coverage, when one was reached.
    #[must_use]
    pub const fn designed(&self) -> Option<&DesignedCoverage> {
        self.designed.as_ref()
    }

    /// A particular offering's observed coverage, when one was reached.
    #[must_use]
    pub const fn actual(&self) -> Option<&ActualCoverage> {
        self.actual.as_ref()
    }

    /// Which of the six.
    #[must_use]
    pub const fn status(&self) -> MappingStatus {
        self.status
    }

    /// Why, in the user's terms.
    #[must_use]
    pub const fn reason(&self) -> &NonEmptyText {
        &self.reason
    }
}

/// What section 21.2's six statuses are decided from.
///
/// Public fields, the way `P2-R4`'s `ClassificationInput` has them: this is the
/// argument list of [`MappingStatus::for_evidence`] and every field is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingEvidence<'a> {
    /// The user's standing with respect to the course.
    pub enrolment: EnrolmentStanding,
    /// How strong the user's own evidence on the subject is.
    pub personal: PersonalEvidenceStanding,
    /// A particular offering's observed coverage, when there is one.
    pub actual: Option<&'a ActualCoverage>,
    /// That offering's `P2-U1` standing, when there is an offering at all.
    pub offering_status: Option<OfferingStatus>,
    /// Whether a short project experiment reaches the subject more directly.
    pub experiment_is_more_direct: bool,
}

impl MappingStatus {
    /// Section 21.2's classification, over one subject's evidence.
    ///
    /// Total over [`MappingEvidence`] with no wildcard arm. The order is the
    /// bullets' own: the two statuses that assert a particular offering covers
    /// the subject are answered first and only with an [`ActualCoverage`] in
    /// hand, so a course that merely exists cannot reach either.
    #[must_use]
    pub fn for_evidence(evidence: &MappingEvidence<'_>) -> Self {
        let covering = evidence.actual;
        match (evidence.enrolment, covering) {
            (EnrolmentStanding::Enrolled, Some(coverage)) if coverage.is_upcoming() => {
                Self::CanBeSupportedByCurrentCourse
            }
            (EnrolmentStanding::Completed, _)
                if evidence.personal == PersonalEvidenceStanding::Weak =>
            {
                Self::PreviouslyTakenEvidenceWeak
            }
            (_, Some(_)) if evidence.offering_status == Some(OfferingStatus::Confirmed) => {
                Self::ConfirmedNextTerm
            }
            (_, _) if evidence.offering_status == Some(OfferingStatus::HistoricallyLikely) => {
                Self::HistoricallyAvailable
            }
            (_, _) if evidence.experiment_is_more_direct => Self::ExternalOrExperimentBetter,
            (
                EnrolmentStanding::Enrolled
                | EnrolmentStanding::Completed
                | EnrolmentStanding::Neither,
                _,
            ) => Self::NoDirectCourseMatch,
        }
    }
}

/// Section 21.3's two effects, kept apart.
///
/// > Project Gap 하나를 채우기 위해 3학점 과목 전체가 최단 경로가 아닐 수 있고,
/// > 반대로 즉각적 gap을 넘어 넓은 이론적 기반을 얻는 선택일 수도 있다. 양쪽
/// > 효과를 구분한다.
///
/// Two [`academic_critical_path::CostEstimate`] values on two named axes, and
/// **no** function that returns one number. That is `P2-N6`'s
/// [`academic_critical_path::BenefitVector`] rule applied to the one comparison
/// section 21.3 asks for; this type has no `total`, no `score`, no `Ord` and no
/// numeric conversion, and the whole-set impl-header inventory in
/// `crates/build-learn/tests/build_learn_scans.rs` is what states that over every
/// type pair rather than over a list of names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelComparison {
    subject: EntityId,
    immediate_gap: academic_critical_path::CostEstimate,
    breadth: academic_critical_path::CostEstimate,
}

impl ChannelComparison {
    /// Records both effects of one acquisition channel.
    #[must_use]
    pub const fn of(
        subject: EntityId,
        immediate_gap: academic_critical_path::CostEstimate,
        breadth: academic_critical_path::CostEstimate,
    ) -> Self {
        Self {
            subject,
            immediate_gap,
            breadth,
        }
    }

    /// The concept or competency compared.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// How much of the immediate gap this channel closes.
    #[must_use]
    pub const fn immediate_gap(&self) -> &academic_critical_path::CostEstimate {
        &self.immediate_gap
    }

    /// How much theoretical breadth it adds beyond the gap.
    #[must_use]
    pub const fn breadth(&self) -> &academic_critical_path::CostEstimate {
        &self.breadth
    }
}
