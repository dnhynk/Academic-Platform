//! Section 13.1's ladder and its five facets.
//!
//! ## The six are not declared here
//!
//! `academic_domain::MasteryLevel` already holds them and this crate declares
//! no second enumeration. What is here is [`LADDER`] — the same six in section
//! 13.1's own row order — and [`rung`], whose `match` has no wildcard arm. A
//! seventh level added to the domain enumeration is therefore a compile error
//! in this file rather than a value some list quietly fails to mention.
//!
//! The count is not asserted as a number in this crate.
//! `mastery_enum_is_exactly_six_ordered` reads section 13.1's table back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares its rows
//! against [`LADDER`] in both directions, so six is a measurement of the design
//! document rather than a sentence in a test.
//!
//! ## `AutomaticLevel` has five variants, and the missing one is the point
//!
//! Section 13.1's `FLUENT` row reads `AI 단독 판정 금지, 반복된 강한 evidence와
//! 사용자 확인 필요`. That is not a threshold to compare against; it is a level
//! an automatic projection may not reach at all. So the type an automatic
//! projection returns — [`AutomaticLevel`] — **has no `Fluent` variant**. Code
//! on the automatic path cannot name the value, which is stronger than
//! refusing it: there is nothing to refuse.
//!
//! `FLUENT` is reachable only through
//! [`crate::confirmation::FluentAuthorization`], whose one constructor takes
//! repeated cross-context evidence and a verified user confirmation, both by
//! value. That is `P2-U1`'s "a forbidden field has no setter", `P2-U2`'s
//! "the gate is a type", and `P2-R4`'s by-value chain, applied to the one
//! promotion section 13.1 says an AI may never make.
//!
//! ## Five facets, and every one of them is required
//!
//! Section 13.1: `단일 level이 정보를 압축하므로 내부에는 다음 facet을 둔다`.
//! A [`FacetProfile`] therefore has one slot per facet and no `Default`: a
//! profile with `transferToNovelSituation` left out is not a profile that
//! reports a gap, it is a value that cannot be built.

use academic_domain::MasteryLevel;
use serde::{Deserialize, Serialize};

/// Section 13.1's six levels, in the table's own row order.
///
/// The array is the order; [`rung`] is the ordinal. Both are compared against
/// the design document by `mastery_enum_is_exactly_six_ordered`.
pub const LADDER: [MasteryLevel; 6] = [
    MasteryLevel::Unseen,
    MasteryLevel::Exposed,
    MasteryLevel::Understood,
    MasteryLevel::Practiced,
    MasteryLevel::Applied,
    MasteryLevel::Fluent,
];

/// Section 13.1's `Level` column.
///
/// Total over `MasteryLevel` with no wildcard arm, so a level added to the
/// domain enumeration fails to compile here rather than defaulting to zero.
#[must_use]
pub const fn rung(level: MasteryLevel) -> u8 {
    match level {
        MasteryLevel::Unseen => 0,
        MasteryLevel::Exposed => 1,
        MasteryLevel::Understood => 2,
        MasteryLevel::Practiced => 3,
        MasteryLevel::Applied => 4,
        MasteryLevel::Fluent => 5,
    }
}

/// Section 13.1's second column, as the wire spelling.
///
/// Total for the same reason as [`rung`].
#[must_use]
pub const fn level_token(level: MasteryLevel) -> &'static str {
    match level {
        MasteryLevel::Unseen => "UNSEEN",
        MasteryLevel::Exposed => "EXPOSED",
        MasteryLevel::Understood => "UNDERSTOOD",
        MasteryLevel::Practiced => "PRACTICED",
        MasteryLevel::Applied => "APPLIED",
        MasteryLevel::Fluent => "FLUENT",
    }
}

/// The levels an automatic projection may reach. There is no `Fluent`.
///
/// See the module documentation: section 13.1 forbids an AI-alone `FLUENT`
/// judgement, and this type is that prohibition expressed as an absent variant
/// rather than as a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutomaticLevel {
    /// No evidence of contact. Section 13.1: `evidence 없음이지 "모른다"는 시험
    /// 결과가 아님`.
    Unseen,
    /// Met meaningfully in a lecture or a document.
    Exposed,
    /// Explained in the user's own words, distinguished, predicted.
    Understood,
    /// Used in a problem, an assignment or an experiment.
    Practiced,
    /// Used in a real project's decision, implementation or debugging.
    Applied,
}

impl AutomaticLevel {
    /// Exhaustive order, weakest first.
    pub const ALL: [Self; 5] = [
        Self::Unseen,
        Self::Exposed,
        Self::Understood,
        Self::Practiced,
        Self::Applied,
    ];

    /// The section 13.1 level this automatic level is.
    #[must_use]
    pub const fn level(self) -> MasteryLevel {
        match self {
            Self::Unseen => MasteryLevel::Unseen,
            Self::Exposed => MasteryLevel::Exposed,
            Self::Understood => MasteryLevel::Understood,
            Self::Practiced => MasteryLevel::Practiced,
            Self::Applied => MasteryLevel::Applied,
        }
    }

    /// The automatic level for a section 13.1 level, or [`None`] for `FLUENT`.
    ///
    /// The one direction that can fail, and it fails for exactly the level an
    /// automatic path may not produce.
    #[must_use]
    pub const fn of(level: MasteryLevel) -> Option<Self> {
        match level {
            MasteryLevel::Unseen => Some(Self::Unseen),
            MasteryLevel::Exposed => Some(Self::Exposed),
            MasteryLevel::Understood => Some(Self::Understood),
            MasteryLevel::Practiced => Some(Self::Practiced),
            MasteryLevel::Applied => Some(Self::Applied),
            MasteryLevel::Fluent => None,
        }
    }
}

impl From<AutomaticLevel> for MasteryLevel {
    fn from(value: AutomaticLevel) -> Self {
        value.level()
    }
}

/// Section 13.1's five performance facets, keyed as its own YAML block keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MasteryFacet {
    /// `recognize`.
    Recognize,
    /// `explain`.
    Explain,
    /// `solveStructuredProblem`.
    SolveStructuredProblem,
    /// `implementOrOperate`.
    ImplementOrOperate,
    /// `transferToNovelSituation`.
    TransferToNovelSituation,
}

impl MasteryFacet {
    /// Exhaustive order, in the YAML block's own key order.
    pub const ALL: [Self; 5] = [
        Self::Recognize,
        Self::Explain,
        Self::SolveStructuredProblem,
        Self::ImplementOrOperate,
        Self::TransferToNovelSituation,
    ];

    /// The design document's own key, verbatim.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Recognize => "recognize",
            Self::Explain => "explain",
            Self::SolveStructuredProblem => "solveStructuredProblem",
            Self::ImplementOrOperate => "implementOrOperate",
            Self::TransferToNovelSituation => "transferToNovelSituation",
        }
    }
}

/// How much evidence one facet has, in the design document's own vocabulary.
///
/// Section 13.1's example block exhibits exactly `STRONG`, `MODERATE` and
/// `LIMITED_EVIDENCE`, and this enumeration is closed at those three.
/// `ks_applied_mixed_facets` reads the block and compares. A fourth value is a
/// change to the design document, not one this crate may add on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FacetStrength {
    /// `LIMITED_EVIDENCE`: the floor. Not a statement that the user cannot.
    LimitedEvidence,
    /// `MODERATE`.
    Moderate,
    /// `STRONG`.
    Strong,
}

impl FacetStrength {
    /// Exhaustive order, weakest first.
    pub const ALL: [Self; 3] = [Self::LimitedEvidence, Self::Moderate, Self::Strong];

    /// The design document's own spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LimitedEvidence => "LIMITED_EVIDENCE",
            Self::Moderate => "MODERATE",
            Self::Strong => "STRONG",
        }
    }
}

/// One strength per facet, all five present.
///
/// No `Default` and no public field: the one constructor takes all five, so a
/// profile that forgot a facet is a program that does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacetProfile {
    recognize: FacetStrength,
    explain: FacetStrength,
    solve_structured_problem: FacetStrength,
    implement_or_operate: FacetStrength,
    transfer_to_novel_situation: FacetStrength,
}

impl FacetProfile {
    /// Builds a profile from all five facets, in the YAML block's key order.
    #[must_use]
    pub const fn of(
        recognize: FacetStrength,
        explain: FacetStrength,
        solve_structured_problem: FacetStrength,
        implement_or_operate: FacetStrength,
        transfer_to_novel_situation: FacetStrength,
    ) -> Self {
        Self {
            recognize,
            explain,
            solve_structured_problem,
            implement_or_operate,
            transfer_to_novel_situation,
        }
    }

    /// Reads one facet.
    ///
    /// Total over [`MasteryFacet`] with no wildcard arm.
    #[must_use]
    pub const fn strength(&self, facet: MasteryFacet) -> FacetStrength {
        match facet {
            MasteryFacet::Recognize => self.recognize,
            MasteryFacet::Explain => self.explain,
            MasteryFacet::SolveStructuredProblem => self.solve_structured_problem,
            MasteryFacet::ImplementOrOperate => self.implement_or_operate,
            MasteryFacet::TransferToNovelSituation => self.transfer_to_novel_situation,
        }
    }

    /// The profile with `transferToNovelSituation` raised to `strength`, as a
    /// new value.
    ///
    /// Section 13.2's fifth row is `incident debugging에서 원인 규명·수정·검증 →
    /// Applied, transfer facet 강화`. `강화` never lowers, so a strength below
    /// the standing one leaves the profile as it was, and the result is a new
    /// profile rather than a mutation of this one.
    #[must_use]
    pub fn with_transfer_at_least(&self, strength: FacetStrength) -> Self {
        let mut next = *self;
        if strength > next.transfer_to_novel_situation {
            next.transfer_to_novel_situation = strength;
        }
        next
    }
}
