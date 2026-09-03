//! The three section 38 cells this task leaves open, stated where they bite.
//!
//! None has a default and none is given one here, exactly as
//! `academic_ingestion::gate` and `academic_consent::gate` do it. What this
//! module supplies is the shape of each cell and the value that stands while it
//! is empty.
//!
//! There is deliberately no recognition table, no substitution table, and no
//! function comparing an official prerequisite with a recommended one. Each of
//! the three is an official fact the user has to confirm, and inventing a
//! default is how a graduation audit becomes confidently wrong.

use crate::{
    offering::GradingMode,
    relation::CourseCodeReuse,
    revision::CurriculumCategory,
    version::{CohortTransition, PublicationStatus},
};

/// A section 38 cell this task leaves for the user to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OpenGate {
    /// `GATE-38-013`: the current engineering-common recognition list and how
    /// it distributes between required and elective major credit.
    RecognitionList,
    /// `GATE-38-014`: which courses substitute for which, for whom, and from
    /// when.
    SubstitutionRules,
    /// `GATE-38-018`: how a course's official prerequisite differs from the
    /// instructor's recommended prior knowledge.
    OfficialVersusRecommendedPrerequisite,
}

impl OpenGate {
    /// All three cells.
    pub const ALL: [Self; 3] = [
        Self::RecognitionList,
        Self::SubstitutionRules,
        Self::OfficialVersusRecommendedPrerequisite,
    ];

    /// The section 38 identifier.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::RecognitionList => "GATE-38-013",
            Self::SubstitutionRules => "GATE-38-014",
            Self::OfficialVersusRecommendedPrerequisite => "GATE-38-018",
        }
    }

    /// What the cell leaves open, and what stands while it is empty.
    #[must_use]
    pub const fn statement(self) -> &'static str {
        match self {
            Self::RecognitionList => {
                "the current engineering-common recognition list and its \
                 required/elective distribution is an official fact the user \
                 must confirm (GATE-38-013); an unconfirmed revision holds \
                 CurriculumCategory::Unknown, and nothing infers a category \
                 from a course code, a credit count, or a sibling revision"
            }
            Self::SubstitutionRules => {
                "which courses substitute for which, for whom, and from when is \
                 an official fact the user must confirm (GATE-38-014); an \
                 equivalence exists only where one was recorded, it holds in \
                 the asserted direction only, and no rule derives one from a \
                 replacement, a retirement, or a shared course code"
            }
            Self::OfficialVersusRecommendedPrerequisite => {
                "how a course's official prerequisite differs from the \
                 instructor's recommended prior knowledge needs a reviewed \
                 source (GATE-38-018); the two are captured as separate typed \
                 lists on a revision and this crate contains no function that \
                 compares them or derives one from the other"
            }
        }
    }
}

/// Every value in this crate that means "no official record exists".
///
/// Enumerated rather than counted, and each entry is the type's own spelling,
/// so a variant renamed on one side fails against the other.
/// `an_absent_official_fact_reads_unknown` walks this list and requires each
/// spelling to be the value the corresponding constructor starts at.
#[must_use]
pub fn unknown_readings() -> [(&'static str, &'static str); 5] {
    [
        ("CurriculumCategory", CurriculumCategory::Unknown.as_str()),
        ("GradingMode", GradingMode::Unknown.as_str()),
        ("CourseCodeReuse", CourseCodeReuse::Unknown.as_str()),
        ("CohortTransition", CohortTransition::Unknown.as_str()),
        ("PublicationStatus", PublicationStatus::Unknown.as_str()),
    ]
}
