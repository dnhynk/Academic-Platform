//! Section 24.3's evidence stages.
//!
//! The section names them in one sentence, in backticks, in this order:
//!
//! > `사용해봄`, `구조 이해`, `문제 해결`, `장애 debugging`, `설계 선택`,
//! > `새 상황 전이` evidence를 구분한다.
//!
//! **The count is not asserted as a number here.**
//! `six_evidence_stages_are_distinct` reads that sentence out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`, takes the backticked
//! spans whole, and compares them against [`EvidenceStage::ALL`] and
//! [`EvidenceStage::spec_name`] in both directions. Six is therefore a
//! measurement of the design document, and the day that sentence changes this
//! crate fails rather than drifts.
//!
//! ## They are not section 13.2's rows
//!
//! Section 13.2 has eight evidence rows and two of them license no promotion,
//! which leaves six that do. That coincidence is **not** a correspondence and
//! this crate does not build one: section 13.2's first row is
//! `transcript에서 meaningful teaching`, which is exposure rather than
//! `사용해봄`, and section 24.3's `설계 선택` has no row of its own at all. A
//! total map between the two would have to invent three of its six answers.
//!
//! What this crate reads out of section 13.2 instead is the one thing that
//! table decides on its own: whether a row licenses any promotion. See
//! [`crate::evidence::PromotingEvidence`].

use serde::{Deserialize, Serialize};

/// One of section 24.3's evidence stages.
///
/// The stage says at what depth a competency was exercised. It does not say how
/// well: section 24.3 asks for the six to be *separated*, and separating them is
/// what keeps `사용해봄` from being read as `설계 선택`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceStage {
    /// `사용해봄`.
    Used,
    /// `구조 이해`.
    UnderstoodStructure,
    /// `문제 해결`.
    SolvedProblem,
    /// `장애 debugging`.
    DebuggedIncident,
    /// `설계 선택`.
    MadeDesignChoice,
    /// `새 상황 전이`.
    TransferredToNovel,
}

impl EvidenceStage {
    /// Exhaustive, in section 24.3's own order.
    pub const ALL: [Self; 6] = [
        Self::Used,
        Self::UnderstoodStructure,
        Self::SolvedProblem,
        Self::DebuggedIncident,
        Self::MadeDesignChoice,
        Self::TransferredToNovel,
    ];

    /// Stable spelling.
    ///
    /// Total, with no wildcard arm: a seventh stage has to answer this rather
    /// than inherit an answer.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Used => "USED",
            Self::UnderstoodStructure => "UNDERSTOOD_STRUCTURE",
            Self::SolvedProblem => "SOLVED_PROBLEM",
            Self::DebuggedIncident => "DEBUGGED_INCIDENT",
            Self::MadeDesignChoice => "MADE_DESIGN_CHOICE",
            Self::TransferredToNovel => "TRANSFERRED_TO_NOVEL",
        }
    }

    /// The design document's own name for this stage, verbatim.
    ///
    /// `six_evidence_stages_are_distinct` compares these against the backticked
    /// spans of section 24.3's own sentence, so this is the string that binds
    /// the enumeration to the specification.
    #[must_use]
    pub const fn spec_name(self) -> &'static str {
        match self {
            Self::Used => "사용해봄",
            Self::UnderstoodStructure => "구조 이해",
            Self::SolvedProblem => "문제 해결",
            Self::DebuggedIncident => "장애 debugging",
            Self::MadeDesignChoice => "설계 선택",
            Self::TransferredToNovel => "새 상황 전이",
        }
    }
}
