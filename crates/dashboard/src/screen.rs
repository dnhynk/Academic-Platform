//! Section 25.4's academic dashboard, as the sequence of lines it lists.
//!
//! Section 25.4 is six bullets and a closing sentence:
//!
//! 1. 누적·학기·전공 GPA와 각 계산 proof.
//! 2. 총 취득학점과 category별 학점.
//! 3. 적용 중인 admission year, selected graduation standard, degree mode.
//! 4. 졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.
//! 5. 수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.
//! 6. official source freshness와 마지막 sync.
//!
//! [`DashboardLine::ALL`] is that list and
//! `dashboard_shows_three_gpas_with_proof` parses it out of the design document
//! and compares the two in both directions and in order.
//!
//! # No composite, and the claim is about places
//!
//! Section 10's last paragraph: *Academic Dashboard에서 GPA chart와 Knowledge
//! Map을 같은 카드의 한 score로 합치지 않는다.* Section 35's anti-goal table
//! forbids the same thing from the other end — *생산성 gamification*, *단순
//! GPA/졸업 계산기* — and section 36.9 closes with *한 학기의 결과는 "Database
//! 83%"가 아니라*.
//!
//! Three things hold that here, and `dashboard_no_composite` measures all
//! three:
//!
//! * **There is no second half to add.** This crate has no
//!   `academic-knowledge-state` edge and no `academic-freshness` edge, so no
//!   mastery level, no knowledge state and no concept reading is nameable from
//!   a product file. `P2-X2` holds the same line by the same means.
//! * **No section holds two figures.** [`DashboardSection`] carries the line it
//!   answers for and the values of that line only, so a card that showed a
//!   grade average beside a knowledge score would have to be a seventh line.
//! * **Nothing folds the three averages.** There is no accessor on
//!   [`AcademicDashboard`] returning one number over more than one
//!   [`crate::GpaFigure`], and the whole-set half of that is
//!   `every_item_that_reaches_a_closed_type_is_pinned` keyed on `GpaFigure`.
//!
//! # The blocking cells are shown, not filled
//!
//! [`AcademicDashboard::assemble`] takes the section 38 cells that are still
//! empty and refuses to publish a figure for a line they block.
//! [`DashboardSection::Blocked`] carries the exact cell, which is
//! `t068`'s *surfaces `GATE-38-001`–`GATE-38-006` as blocking dashboard
//! inputs*.

use crate::{
    AttemptTimeline, AuditStateReading, DashboardError, GpaFigure, GpaScope, OpenGate,
    SecondaryPercentage,
};

/// One of section 25.4's six lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DashboardLine {
    /// 누적·학기·전공 GPA와 각 계산 proof.
    Averages,
    /// 총 취득학점과 category별 학점.
    CreditsByCategory,
    /// 적용 중인 admission year, selected graduation standard, degree mode.
    AppliedProfile,
    /// 졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.
    AuditStates,
    /// 수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.
    AttemptTimeline,
    /// official source freshness와 마지막 sync.
    SourceFreshness,
}

impl DashboardLine {
    /// Every line, in section 25.4's own order.
    pub const ALL: [Self; 6] = [
        Self::Averages,
        Self::CreditsByCategory,
        Self::AppliedProfile,
        Self::AuditStates,
        Self::AttemptTimeline,
        Self::SourceFreshness,
    ];

    /// Section 25.4's own text for this line, verbatim.
    #[must_use]
    pub const fn spec_line(self) -> &'static str {
        match self {
            Self::Averages => "누적·학기·전공 GPA와 각 계산 proof.",
            Self::CreditsByCategory => "총 취득학점과 category별 학점.",
            Self::AppliedProfile => {
                "적용 중인 admission year, selected graduation standard, degree mode."
            }
            Self::AuditStates => "졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.",
            Self::AttemptTimeline => "수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.",
            Self::SourceFreshness => "official source freshness와 마지막 sync.",
        }
    }

    /// The identifier `packages/ui`'s shell half shows this line under.
    ///
    /// Written out rather than derived from the arm name, for the reason
    /// `P2-X1`'s view registry is written out rather than derived from its
    /// route manifest: a derived identifier would agree with the enumeration
    /// because it *was* the enumeration.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Averages => "AVERAGES",
            Self::CreditsByCategory => "CREDITS_BY_CATEGORY",
            Self::AppliedProfile => "APPLIED_PROFILE",
            Self::AuditStates => "AUDIT_STATES",
            Self::AttemptTimeline => "ATTEMPT_TIMELINE",
            Self::SourceFreshness => "SOURCE_FRESHNESS",
        }
    }

    /// Section 25.4's own number for this line, counting from one.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::Averages => 1,
            Self::CreditsByCategory => 2,
            Self::AppliedProfile => 3,
            Self::AuditStates => 4,
            Self::AttemptTimeline => 5,
            Self::SourceFreshness => 6,
        }
    }

    /// The section 38 cells that stop this line being filled.
    ///
    /// Every line except the freshness line rests on the imported record or on
    /// the selected requirement set, which is why five of the six cells reach
    /// four of the six lines.
    #[must_use]
    pub const fn blocked_by(self) -> &'static [OpenGate] {
        match self {
            Self::Averages => &[
                OpenGate::ProfileOfficialTranscript,
                OpenGate::ProfileDegreeMode,
                OpenGate::ProfileExchangeOrTransfer,
            ],
            Self::CreditsByCategory => &[
                OpenGate::ProfileOfficialTranscript,
                OpenGate::ProfileAdmissionYear,
                OpenGate::ProfileGraduationStandard,
            ],
            Self::AppliedProfile => &[
                OpenGate::ProfileAdmissionYear,
                OpenGate::ProfileGraduationStandard,
                OpenGate::ProfileDegreeMode,
                OpenGate::ProfileAdditionalMajor,
            ],
            Self::AuditStates => &[
                OpenGate::ProfileAdmissionYear,
                OpenGate::ProfileGraduationStandard,
                OpenGate::ProfileDegreeMode,
            ],
            Self::AttemptTimeline => &[OpenGate::ProfileOfficialTranscript],
            // Whether the last sync is stale is answerable with no user input
            // at all: it is a property of the reading, not of the profile.
            Self::SourceFreshness => &[],
        }
    }
}

/// What one line of the dashboard shows.
///
/// A section is either filled with its line's own values or blocked by a named
/// section 38 cell. There is no third arm and no empty-but-unblocked state: a
/// blank card that says nothing is the failure this enumeration exists to make
/// unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashboardSection {
    /// Section 25.4's first line, one figure per scope.
    Averages(Vec<GpaFigure>),
    /// Section 25.4's second line: the earned total, then one entry per
    /// category, each with its own count.
    CreditsByCategory(Vec<(String, u32)>),
    /// Section 25.4's third line, as the profile values in force.
    AppliedProfile(Vec<(String, String)>),
    /// Section 25.4's fourth line, one reading per evaluated rule.
    AuditStates(Vec<(String, AuditStateReading)>),
    /// Section 25.4's fifth line.
    AttemptTimeline(AttemptTimeline),
    /// Section 25.4's sixth line: the source and how old its reading is.
    SourceFreshness(Vec<(String, i64)>),
    /// The line is not shown, and this is the exact cell that stops it.
    Blocked(OpenGate),
}

impl DashboardSection {
    /// Which line this section answers for.
    #[must_use]
    pub const fn line(&self) -> Option<DashboardLine> {
        match self {
            Self::Averages(_) => Some(DashboardLine::Averages),
            Self::CreditsByCategory(_) => Some(DashboardLine::CreditsByCategory),
            Self::AppliedProfile(_) => Some(DashboardLine::AppliedProfile),
            Self::AuditStates(_) => Some(DashboardLine::AuditStates),
            Self::AttemptTimeline(_) => Some(DashboardLine::AttemptTimeline),
            Self::SourceFreshness(_) => Some(DashboardLine::SourceFreshness),
            Self::Blocked(_) => None,
        }
    }
}

/// The `/academic/dashboard` screen.
///
/// Six sections in section 25.4's own order, and the graduation percentage —
/// which section 25.4 calls a 보조 시각화 — kept out of that sequence entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcademicDashboard {
    sections: [DashboardSection; DashboardLine::ALL.len()],
    secondary: Option<SecondaryPercentage>,
}

impl AcademicDashboard {
    /// Assembles the screen, blocking each line its open cells reach.
    ///
    /// `open` is the set of section 38 cells still empty. A line reached by any
    /// of them becomes [`DashboardSection::Blocked`] naming the **first** such
    /// cell in section 38's own order, so the message is one exact missing
    /// check rather than a list the reader has to prioritise.
    pub fn assemble(
        filled: [DashboardSection; DashboardLine::ALL.len()],
        open: &[OpenGate],
        secondary: Option<SecondaryPercentage>,
    ) -> Result<Self, DashboardError> {
        let mut sections = filled;
        for (index, line) in DashboardLine::ALL.into_iter().enumerate() {
            if let Some(gate) = OpenGate::ALL
                .into_iter()
                .find(|gate| open.contains(gate) && line.blocked_by().contains(gate))
            {
                sections[index] = DashboardSection::Blocked(gate);
            }
        }
        Ok(Self {
            sections,
            secondary,
        })
    }

    /// The six sections, in section 25.4's own order.
    #[must_use]
    pub const fn sections(&self) -> &[DashboardSection; DashboardLine::ALL.len()] {
        &self.sections
    }

    /// One line's section.
    #[must_use]
    pub const fn section(&self, line: DashboardLine) -> &DashboardSection {
        &self.sections[line as usize]
    }

    /// The three averages, when the line that holds them is not blocked.
    ///
    /// Returns the figures themselves. There is no accessor returning a number
    /// over more than one of them, which is the *합치지 않는다* half of section
    /// 10's last paragraph.
    #[must_use]
    pub fn averages(&self) -> Option<&[GpaFigure]> {
        match self.section(DashboardLine::Averages) {
            DashboardSection::Averages(figures) => Some(figures),
            _ => None,
        }
    }

    /// One scope's figure, when the averages line is filled.
    #[must_use]
    pub fn average(&self, scope: GpaScope) -> Option<&GpaFigure> {
        self.averages()?
            .iter()
            .find(|figure| figure.scope() == scope)
    }

    /// The 보조 시각화, when there is one.
    ///
    /// Not one of the six sections and never the first thing on the screen:
    /// the percentage is reached through this accessor and the breakdown it
    /// carries is reached through the percentage, so there is no path to the
    /// number that does not pass the parts.
    #[must_use]
    pub const fn secondary_percentage(&self) -> Option<&SecondaryPercentage> {
        self.secondary.as_ref()
    }
}
