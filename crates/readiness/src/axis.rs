//! Section 24.3's six matrix columns.
//!
//! ## The six are read out of two places, and they agree
//!
//! Section 24.3's own table writes its header row as
//!
//! ```text
//! | Competency | 학문적으로 배움 | 문제/과제 | Project 적용 | 장애/Debug | 설계 선택 | Freshness |
//! ```
//!
//! and section 36.9 writes the same view for one competency as a block of keys:
//!
//! ```text
//! academic: Database lecture + assessment
//! practice: B+ Tree assignment
//! project: Project A transaction/index investigation
//! debugging: duplicate-processing incident
//! design: idempotency ADR
//! freshness: high
//! ```
//!
//! **The count is not asserted as a number here.**
//! `six_axes_are_separate_columns` parses the table's header cells after
//! `Competency` and the code block's keys out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` at run time and
//! compares each, in both directions and position by position, against
//! [`ReadinessAxis::ALL`]. Six is therefore a measurement of two independent
//! places in the design document, and a document that renames a column, adds
//! one, drops one, or lets the two places disagree fails this crate rather than
//! drifting past it.
//!
//! ## These are not `P2-Y1`'s six evidence stages
//!
//! Section 24.3 states a second six in its own prose — `사용해봄`, `구조 이해`,
//! `문제 해결`, `장애 debugging`, `설계 선택`, `새 상황 전이` — and those are
//! `academic_competency::EvidenceStage`, which `P2-Y1` owns. The two sixes are
//! different sets that share one spelling, `설계 선택`, and partly rhyme on two
//! more, and folding them would be exactly the collision `P2-R4` measured one
//! stage over. So they are two types with no conversion in either direction,
//! and `an_axis_and_a_stage_are_two_vocabularies` requires the name sets to be
//! unequal and the shared spelling to be present in both — the reading is
//! recorded, not assumed away.
//!
//! A stage says at what depth a performance was exercised. An axis says which
//! column of the readiness view a piece of evidence is displayed in. A cell may
//! carry evidence recorded at any stage; which stage it was is [`crate::cell`]'s
//! business and never this enumeration's.

use serde::{Deserialize, Serialize};

/// One column of section 24.3's readiness matrix.
///
/// Five of the six are evidence columns and the sixth is freshness. That is the
/// document's own division and not a grouping made here:
/// [`ReadinessAxis::is_freshness`] is total with no wildcard arm, and
/// [`crate::matrix::ReadinessRow`] takes the five evidence readings and the one
/// freshness reading as six separate parameters of two different types, so a
/// freshness band has no position an evidence cell could occupy and no evidence
/// cell has a position a band could.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadinessAxis {
    /// `학문적으로 배움` / `academic`.
    AcademicLearning,
    /// `문제/과제` / `practice`.
    ProblemAndAssignment,
    /// `Project 적용` / `project`.
    ProjectApplication,
    /// `장애/Debug` / `debugging`.
    IncidentDebugging,
    /// `설계 선택` / `design`.
    DesignChoice,
    /// `Freshness` / `freshness`.
    Freshness,
}

impl ReadinessAxis {
    /// Exhaustive, in the order both of the document's own places write them.
    pub const ALL: [Self; 6] = [
        Self::AcademicLearning,
        Self::ProblemAndAssignment,
        Self::ProjectApplication,
        Self::IncidentDebugging,
        Self::DesignChoice,
        Self::Freshness,
    ];

    /// Stable spelling.
    ///
    /// Total, with no wildcard arm: a seventh axis has to answer this rather
    /// than inherit an answer.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AcademicLearning => "ACADEMIC_LEARNING",
            Self::ProblemAndAssignment => "PROBLEM_AND_ASSIGNMENT",
            Self::ProjectApplication => "PROJECT_APPLICATION",
            Self::IncidentDebugging => "INCIDENT_DEBUGGING",
            Self::DesignChoice => "DESIGN_CHOICE",
            Self::Freshness => "FRESHNESS",
        }
    }

    /// Section 24.3's own table heading for this column, verbatim.
    #[must_use]
    pub const fn table_heading(self) -> &'static str {
        match self {
            Self::AcademicLearning => "학문적으로 배움",
            Self::ProblemAndAssignment => "문제/과제",
            Self::ProjectApplication => "Project 적용",
            Self::IncidentDebugging => "장애/Debug",
            Self::DesignChoice => "설계 선택",
            Self::Freshness => "Freshness",
        }
    }

    /// Section 36.9's own key for this column, verbatim.
    #[must_use]
    pub const fn scenario_key(self) -> &'static str {
        match self {
            Self::AcademicLearning => "academic",
            Self::ProblemAndAssignment => "practice",
            Self::ProjectApplication => "project",
            Self::IncidentDebugging => "debugging",
            Self::DesignChoice => "design",
            Self::Freshness => "freshness",
        }
    }

    /// Whether this column carries a freshness band rather than evidence.
    ///
    /// Total, with no wildcard arm. Section 34.5 asks for `missing/unknown과
    /// freshness를 별도 표시`, and this is the one place the division between
    /// the five and the one is written down.
    #[must_use]
    pub const fn is_freshness(self) -> bool {
        match self {
            Self::AcademicLearning
            | Self::ProblemAndAssignment
            | Self::ProjectApplication
            | Self::IncidentDebugging
            | Self::DesignChoice => false,
            Self::Freshness => true,
        }
    }

    /// The five evidence columns, in the document's order.
    ///
    /// Derived from [`Self::ALL`] by [`Self::is_freshness`] rather than written
    /// out again, so a seventh axis joins this list by answering that function
    /// and cannot be forgotten here.
    #[must_use]
    pub fn evidence_axes() -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|axis| !axis.is_freshness())
            .collect()
    }
}
