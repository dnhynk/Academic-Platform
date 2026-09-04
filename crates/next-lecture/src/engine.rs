//! The one entry point, and the two things it refuses to do.
//!
//! [`propose`] takes a [`GapCase`] `P2-N5` produced and turns its strong-deficit
//! roots into a brief. It computes no deficit of its own: there is no function
//! in this crate that produces a [`crate::PreparationCandidate`] without a
//! `RootCandidate`, and no function that produces a `RootCandidate` at all.
//!
//! # It does not rank, and it does not hide
//!
//! Section 25.2's own instruction for a crowded morning is
//! `알림 수가 많으면 자동 중요도 순으로 숨기지 않고`. So when the descent finds
//! more strong deficits than the morning has room for, [`propose`] answers
//! [`crate::NextLectureError::TooManyBlockingFoundations`] with the count rather
//! than choosing three. Ranking blocking foundations against each other is
//! `P2-N6`'s AND/OR question and nothing here decides it.
//!
//! # It answers `Ok(None)` rather than an empty card
//!
//! A descent that found no strong deficit produces no brief at all, which is
//! `P2-X2`'s rule that a group with nothing to offer shows no card. It is a
//! different answer from a refusal: nothing is wrong, there is simply nothing to
//! prepare.
//!
//! # One claim, one lecture concept
//!
//! Section 12.7 extracts claims from seven places and compares them against the
//! graph. Several claims may name one concept — the syllabus and the next slide
//! agreeing is the ordinary case — and [`propose`] takes the one whose material
//! the caller is proposing under, so the brief's `예상 concept` axis cites that
//! material's spans and that material's date. A caller holding several claims
//! calls [`propose`] once per claim and the answers stay attributable.

use academic_domain::ConfidencePermille;
use academic_gap::{ConceptState, GapCase, PrerequisiteGraph};

use crate::{
    NextLectureError,
    brief::{CandidateParts, HIGHEST_PREPARATION, PreparationBrief, PreparationCandidate},
    claim::ExpectedConceptClaim,
};

/// Section 12.7's proposal step.
///
/// Returns `Ok(None)` when the case holds no strong deficit at all.
///
/// # Errors
///
/// [`NextLectureError::CaseIsAboutAnotherConcept`] when the descent did not
/// start at the claim's concept; [`NextLectureError::NoStateForConcept`] when
/// the caller supplied no overlay for a root the case holds;
/// [`NextLectureError::TooManyBlockingFoundations`] when more strong deficits
/// were found than the morning has room for; and every error
/// [`PreparationCandidate::of`] and [`PreparationBrief::assemble`] raise.
pub fn propose(
    claim: &ExpectedConceptClaim,
    case: &GapCase,
    graph: &PrerequisiteGraph,
    states: &[ConceptState],
    edge_confidence: ConfidencePermille,
) -> Result<Option<PreparationBrief>, NextLectureError> {
    if case.surface_concept() != claim.concept() {
        return Err(NextLectureError::CaseIsAboutAnotherConcept {
            case: case.surface_concept(),
            claim: claim.concept(),
        });
    }
    let blocking: Vec<_> = case
        .candidates()
        .iter()
        .filter(|candidate| candidate.is_strong_deficit())
        .collect();
    if blocking.is_empty() {
        return Ok(None);
    }
    if blocking.len() > HIGHEST_PREPARATION {
        return Err(NextLectureError::TooManyBlockingFoundations {
            count: blocking.len(),
        });
    }
    let mut candidates = Vec::new();
    for root in blocking {
        let state = states
            .iter()
            .find(|state| state.concept() == root.concept())
            .ok_or(NextLectureError::NoStateForConcept(root.concept()))?;
        candidates.push(PreparationCandidate::of(
            CandidateParts {
                claim,
                root,
                state,
                edge_confidence,
            },
            graph,
        )?);
    }
    PreparationBrief::assemble(claim.concept(), candidates).map(Some)
}
