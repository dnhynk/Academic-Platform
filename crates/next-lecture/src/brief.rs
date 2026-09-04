//! Section 4's `선수개념 1–3개` and section 25.2's `최대 1–3개`, assembled whole.
//!
//! # The bound is the document's, and it is written in two places
//!
//! > 오전에는 오늘 강의와 녹음 권한 상태, 다음 강의에서 막힐 가능성이 큰
//! > 선수개념 **1–3개**가 보인다.
//!
//! > 2. 수업 전 최소 prerequisite: **최대 1–3개**, `“왜 지금”`과 예상 시간.
//!
//! [`LOWEST_PREPARATION`] and [`HIGHEST_PREPARATION`] are read back out of both
//! sentences by `morning_home_contract`, which splits each on the document's own
//! en dash, requires the two readings to agree with each other, and then
//! compares them with `P2-X2`'s `LOWEST_BRIEF` and `HIGHEST_BRIEF`. Two crates
//! offering the same card with different bounds is the defect that check exists
//! for; `academic-home` is a **dev** edge, so the comparison is made in the
//! suite and no product file here depends on that crate.
//!
//! # Assembled whole, so the bound is checked where the value appears
//!
//! [`PreparationBrief::assemble`] is the only constructor. There is no `push`,
//! no `extend`, no `insert` and no `&mut` accessor, so a fourth candidate cannot
//! be added after the check. The empty brief is refused for `P2-X2`'s reason: a
//! group with nothing to offer shows no card rather than an empty one, and
//! [`crate::engine::propose`] answers `Ok(None)` for that case instead.
//!
//! # Evidence, not a count
//!
//! Section 4 says the morning shows the prerequisites; section 25.2 says each
//! carries `“왜 지금”` and `예상 시간`. Both are on the candidate by
//! construction: `왜 지금` is the [`academic_gap::BlockingPath`] from tomorrow's
//! concept down to this one, and `예상 시간` is `P2-N5`'s own bounded
//! `최소 보강`. Neither is free text and neither can be absent, because a
//! [`academic_gap::RootCandidate`] cannot exist without them.

use academic_domain::EntityId;
use academic_gap::{BlockingPath, GapKind, MinimumRemediation, PrerequisiteGraph, RootCandidate};

use crate::{
    NextLectureError, claim::ExpectedConceptClaim, minimality::minimality_defects,
    uncertainty::PrepUncertainty,
};

/// Section 4's and section 25.2's lower bound.
pub const LOWEST_PREPARATION: usize = 1;

/// Section 4's and section 25.2's upper bound.
pub const HIGHEST_PREPARATION: usize = 3;

/// Everything one candidate is assembled from, so the constructor takes one
/// argument rather than five positional ones.
#[derive(Debug, Clone, Copy)]
pub struct CandidateParts<'a> {
    /// The extracted claim about tomorrow's concept this preparation serves.
    pub claim: &'a ExpectedConceptClaim,
    /// `P2-N5`'s own root, produced by descending from that concept.
    pub root: &'a RootCandidate,
    /// The overlay the descent read at that root.
    pub state: &'a academic_gap::ConceptState,
    /// The edge's confidence. See [`crate::uncertainty`] for why this one is
    /// supplied and the other two are not.
    pub edge_confidence: academic_domain::ConfidencePermille,
}

/// One minimum blocking foundation, with its three uncertainties beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparationCandidate {
    concept: EntityId,
    kind: GapKind,
    blocks: BlockingPath,
    remediation: MinimumRemediation,
    reason: String,
    uncertainty: PrepUncertainty,
}

impl PreparationCandidate {
    /// Assembles one candidate, or refuses it.
    ///
    /// # Errors
    ///
    /// [`NextLectureError::CandidateStateIsAboutAnotherConcept`] when the
    /// overlay is not about the concept the descent ended at;
    /// [`NextLectureError::NoEdgeForTheDeepestStep`] when the graph holds no
    /// blocking edge for the path's last hop;
    /// [`NextLectureError::NotMinimal`] carrying every defect, not the first;
    /// and every error [`PrepUncertainty::factor`] raises.
    pub fn of(
        parts: CandidateParts<'_>,
        graph: &PrerequisiteGraph,
    ) -> Result<Self, NextLectureError> {
        let CandidateParts {
            claim,
            root,
            state,
            edge_confidence,
        } = parts;
        if state.concept() != root.concept() {
            return Err(NextLectureError::CandidateStateIsAboutAnotherConcept {
                candidate: root.concept(),
                state: state.concept(),
            });
        }
        let defects = minimality_defects(claim, root, graph);
        if !defects.is_empty() {
            return Err(NextLectureError::NotMinimal(defects));
        }
        // `minimality_defects` already refused an empty path, so the deepest
        // step exists here; the graph lookup can still fail, because a caller
        // may pass a graph other than the one the descent ran over.
        let step = root
            .blocking_path()
            .steps()
            .last()
            .ok_or(NextLectureError::NoEdgeForTheDeepestStep)?;
        let edge = graph
            .blocking_out_of(step.advanced())
            .into_iter()
            .find(|edge| edge.prerequisite() == step.prerequisite())
            .ok_or(NextLectureError::NoEdgeForTheDeepestStep)?;
        let uncertainty = PrepUncertainty::factor(claim, edge, edge_confidence, state)?;
        Ok(Self {
            concept: root.concept(),
            kind: root.kind(),
            blocks: root.blocking_path().clone(),
            remediation: root.explanation().remediation().clone(),
            reason: root.reason().to_owned(),
            uncertainty,
        })
    }

    /// Which foundation to prepare.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// Which of section 15.2's five kinds the descent routed it to.
    #[must_use]
    pub const fn kind(&self) -> GapKind {
        self.kind
    }

    /// Section 25.2's `“왜 지금”`: the descent from tomorrow's concept to this
    /// one, with a strength on every hop.
    #[must_use]
    pub const fn why_now(&self) -> &BlockingPath {
        &self.blocks
    }

    /// Section 25.2's `예상 시간`, and what to read, run or answer with it.
    #[must_use]
    pub const fn preparation(&self) -> &MinimumRemediation {
        &self.remediation
    }

    /// Section 15.1's `reason`, in the user's own reading.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// The three axes, side by side.
    #[must_use]
    pub const fn uncertainty(&self) -> &PrepUncertainty {
        &self.uncertainty
    }
}

/// The one to three foundations section 4 and section 25.2 allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparationBrief {
    lecture_concept: EntityId,
    candidates: Vec<PreparationCandidate>,
}

impl PreparationBrief {
    /// Assembles a brief.
    ///
    /// # Errors
    ///
    /// [`NextLectureError::PreparationCountOutOfBounds`] when the count is
    /// outside the documents' `1–3`, and
    /// [`NextLectureError::CandidateRepeatsAnother`] when two candidates name
    /// one concept — the same foundation offered twice is two of the at most
    /// three the morning has room for.
    pub fn assemble(
        lecture_concept: EntityId,
        candidates: Vec<PreparationCandidate>,
    ) -> Result<Self, NextLectureError> {
        let count = candidates.len();
        if !(LOWEST_PREPARATION..=HIGHEST_PREPARATION).contains(&count) {
            return Err(NextLectureError::PreparationCountOutOfBounds { count });
        }
        for (index, candidate) in candidates.iter().enumerate() {
            if candidates[..index]
                .iter()
                .any(|earlier| earlier.concept() == candidate.concept())
            {
                return Err(NextLectureError::CandidateRepeatsAnother {
                    concept: candidate.concept(),
                });
            }
        }
        Ok(Self {
            lecture_concept,
            candidates,
        })
    }

    /// The concept tomorrow's lecture is expected to use.
    #[must_use]
    pub const fn lecture_concept(&self) -> EntityId {
        self.lecture_concept
    }

    /// The candidates, in the order they were offered.
    #[must_use]
    pub fn candidates(&self) -> &[PreparationCandidate] {
        &self.candidates
    }
}
