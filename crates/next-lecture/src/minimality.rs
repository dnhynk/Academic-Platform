//! Section 12.7's `최소 기초만`, and its `not included` line, as graph facts.
//!
//! > 이것을 Knowledge State와 prerequisite graph에 비교해 **이해를 막을 가능성이
//! > 큰 최소 기초**만 제안한다.
//! >
//! > ```text
//! > not included
//! > full lecture preview, advanced replacement-policy survey
//! > ```
//!
//! ## No list of broad phrases, here or anywhere in this crate
//!
//! `P2-N5` established the reading and this crate keeps it: a validator that
//! refused `full lecture preview` by matching its words would pass the next
//! paraphrase. Each variant of [`MinimalityDefect`] is instead a fact about the
//! prerequisite graph and the descent, and `minimum_blocking_preparation` drives
//! three fluent, entirely reasonable-sounding proposals that spell none of
//! section 12.7's words and observes each refused, with the defect list saying
//! *which* breadth it was.
//!
//! | Section 12.7 phrase | The fact that refuses it |
//! |---|---|
//! | `이해를 막을 가능성이 큰` | the routed kind is `P2-N5`'s `강한 부족` |
//! | `full lecture preview` | every candidate is reached by descending from the expected concept, so a concept the descent never reached is not one |
//! | `advanced replacement-policy survey` | the graph holds the expected concept as a prerequisite **of** the proposal, so it is above the lecture rather than beneath it |
//!
//! ## Three defects, and not one more, because the fourth was unreachable
//!
//! An earlier draft also refused an unbounded and an uncited preparation. Both
//! are already impossible: a [`RootCandidate`] carries a `GapExplanation`, and
//! `GapExplanation::of` refuses `RemediationUnbounded` and `RemediationUncited`
//! before the candidate exists. A branch that no input reaches is what
//! `P2-R5` measured as a suite that cannot see a real defect, so the two are
//! *cited* from `P2-N5` in
//! [the next-lecture contract](../../../docs/contracts/next-lecture-preparation.md)
//! rather than re-checked here.
//!
//! ## `강한 부족` is section 15.2's word and not a threshold this file picked
//!
//! Section 12.7 asks for what is `막을 가능성이 큰` — likely to block. Section
//! 15.2's table gives exactly one of its five kinds a `뜻` that is a claim the
//! person is missing something: `MASTERY_GAP`, `prerequisite 수행 evidence가
//! 부족`. The other four say the person may know it, that immediate use is
//! *uncertain*, that the graph is wrong, or that the goal has not chosen. So
//! `GapKind::is_strong_deficit` is the likelihood section 12.7 asks about,
//! reused rather than restated, and a `FRESHNESS_GAP` root is preview.

use academic_gap::{PrerequisiteGraph, RootCandidate};
use serde::{Deserialize, Serialize};

use crate::claim::ExpectedConceptClaim;

/// One structural reason a proposal is more than the minimum blocking
/// preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MinimalityDefect {
    /// The descent routed this concept to a kind that is not `강한 부족`.
    NotALikelyBlock,
    /// No descent from the expected concept reaches this concept.
    NotReachedFromTheExpectedConcept,
    /// The graph holds the expected concept as a prerequisite of this concept,
    /// so the proposal is above tomorrow's lecture rather than beneath it.
    BeyondTheExpectedConcept,
}

impl MinimalityDefect {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [
        Self::NotALikelyBlock,
        Self::NotReachedFromTheExpectedConcept,
        Self::BeyondTheExpectedConcept,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotALikelyBlock => "NOT_A_LIKELY_BLOCK",
            Self::NotReachedFromTheExpectedConcept => "NOT_REACHED_FROM_THE_EXPECTED_CONCEPT",
            Self::BeyondTheExpectedConcept => "BEYOND_THE_EXPECTED_CONCEPT",
        }
    }

    /// The half of section 12.7 this defect holds: the `최소 기초만` clause, or
    /// one of the two shapes its `not included` block names.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::NotALikelyBlock => "이해를 막을 가능성이 큰",
            Self::NotReachedFromTheExpectedConcept => "full lecture preview",
            Self::BeyondTheExpectedConcept => "advanced replacement-policy survey",
        }
    }
}

/// Every structural reason this proposal is broader than the minimum, in
/// [`MinimalityDefect::ALL`] order.
///
/// All three rules are evaluated and the whole list is returned rather than the
/// first entry, which is `P2-N5`'s `GapExplanation::defects` shape and
/// `P2-N2`'s `blocking_reasons` shape before it. The list is what makes a
/// refusal attributable: an unrelated topic answers with one defect and an
/// advanced survey answers with two, so the two are distinguishable without this
/// file holding either phrase.
#[must_use]
pub fn minimality_defects(
    claim: &ExpectedConceptClaim,
    root: &RootCandidate,
    graph: &PrerequisiteGraph,
) -> Vec<MinimalityDefect> {
    let mut found = Vec::new();
    if !root.is_strong_deficit() {
        found.push(MinimalityDefect::NotALikelyBlock);
    }
    let path = root.blocking_path();
    if path.surface() != claim.concept() || path.steps().is_empty() {
        found.push(MinimalityDefect::NotReachedFromTheExpectedConcept);
    }
    if reaches(graph, root.concept(), claim.concept()) {
        found.push(MinimalityDefect::BeyondTheExpectedConcept);
    }
    found
}

/// Whether `target` lies on some blocking descent out of `from`.
///
/// Breadth-first over `PrerequisiteGraph::blocking_out_of`, which is the same
/// traversal `P2-N5`'s `expand` runs and the same edge admission behind it, so
/// `BEYOND_THE_EXPECTED_CONCEPT` means the graph says so under `P2-C4`'s own
/// predicate rule rather than under a second one here. Each concept is visited
/// once, so a graph that holds a cycle terminates.
fn reaches(
    graph: &PrerequisiteGraph,
    from: academic_domain::EntityId,
    target: academic_domain::EntityId,
) -> bool {
    let mut seen: Vec<[u8; 16]> = vec![*from.as_bytes()];
    let mut queue: Vec<academic_domain::EntityId> = vec![from];
    while let Some(node) = queue.pop() {
        for edge in graph.blocking_out_of(node) {
            let next = edge.prerequisite();
            if next == target {
                return true;
            }
            if seen.contains(next.as_bytes()) {
                continue;
            }
            seen.push(*next.as_bytes());
            queue.push(next);
        }
    }
    false
}
