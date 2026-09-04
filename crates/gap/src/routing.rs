//! Which of section 15.2's five kinds one overlaid node is, and which one of
//! them is a `강한 부족`.
//!
//! ## Every dimension decides something
//!
//! Step 3 overlays four dimensions and [`route`] reads all four. Each one is the
//! sole difference between two outcomes somewhere below, which is what
//! `four_state_dimensions_are_overlaid` observes: it holds three fixed, moves
//! the fourth, and watches the routed kind change. A dimension nothing branches
//! on would be a field in a struct, not an overlay.
//!
//! | Dimension | Where it decides |
//! |---|---|
//! | contradicting evidence | a recorded failure is a deficit at any rung |
//! | mastery | at or below the edge's floor |
//! | confidence | below the floor *because the records are thin* vs *because the performance is* |
//! | freshness | at the floor, but not retrievable now |
//!
//! ## Exactly one of the five kinds is a strong deficit
//!
//! Section 15.2 step 4 looks for `최초의 강한 부족`, so some deficits are not
//! strong. Which ones is not a threshold this file picked — it is the table's own
//! `뜻` column read back:
//!
//! * `EVIDENCE_GAP` is `실제로 알 수 있으나 시스템에 근거가 없음` — the design
//!   document says in its own words that the person may know it;
//! * `FRESHNESS_GAP` is `과거 mastery는 있으나 즉시 사용 불확실` — *uncertain*,
//!   and `P2-N3`'s `stale_does_not_demote` says the mastery stands;
//! * `ONTOLOGY_GAP` is `synonym/granularity 오류로 잘못 분리됨` — a statement
//!   about the graph, not about the person, answered by `merge/sense correction`;
//! * `CONTEXT_GAP` is `목표나 구현 선택이 불명확해 prerequisite가 갈림` — the
//!   goal has not chosen, answered by `선택지와 조건 명확화`;
//! * `MASTERY_GAP` is `prerequisite 수행 evidence가 부족` — evidence exists and
//!   it is short.
//!
//! Only the last is a claim that the person is missing something. So
//! [`GapKind::is_strong_deficit`] is true for it and false for the other four,
//! and `first_strong_deficit_is_root_with_ancestor_impact` descends looking for
//! that one. The other four are reported and never become a root, which is this
//! task's whole subject: **not saying `부족하다` without an evidence-backed
//! reason to.**

use academic_domain::{EntityId, FreshnessBand, MasteryLevel};
use academic_freshness::rank;
use academic_knowledge_state::{SufficiencyGap, UnseenBasis, rung};
use serde::{Deserialize, Serialize};

use crate::{kind::GapKind, state::ConceptState};

/// The lowest band that still means `즉시 사용` is not in doubt.
///
/// Section 15.2's `FRESHNESS_GAP` is `즉시 사용 불확실`, and `P2-N3` already
/// named the band at which a use stops counting as `최근 사용`: its
/// `SPILLOVER_SOURCE_FLOOR` is `MODERATE`, with `a neighbour at LOW was not
/// recently used`. This is the same reading of the same six-band scale, so the
/// three bands below it — `LOW`, `STALE` and `UNKNOWN` — are the ones section
/// 15.2 calls uncertain. Pinned by `the_gap_decisions_are_pinned`.
pub const RETRIEVAL_FLOOR: FreshnessBand = FreshnessBand::Moderate;

impl GapKind {
    /// Whether this kind is section 15.2 step 4's `강한 부족`.
    ///
    /// Total, with no wildcard arm. See the module note for why exactly one
    /// kind answers `true`.
    #[must_use]
    pub const fn is_strong_deficit(self) -> bool {
        match self {
            Self::MasteryGap => true,
            Self::FreshnessGap | Self::EvidenceGap | Self::OntologyGap | Self::ContextGap => false,
        }
    }
}

/// Whether the goal has settled which prerequisite branch applies.
///
/// The *conditional* half of `weak_builds_on_is_excluded_or_conditional`. A
/// helpful `BUILDS_ON` never blocks, so it never appears on a path; when two or
/// more of them leave one node and no success criterion names any of them, the
/// prerequisite set branches and the goal has not chosen, which is section
/// 15.2's `CONTEXT_GAP`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "branch", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BranchStanding {
    /// One route, or a route the criteria name.
    Settled,
    /// Several helpful routes and no criterion naming any of them.
    Unchosen {
        /// The concepts the unchosen helpful edges reach, in identifier order.
        options: Vec<EntityId>,
    },
}

impl BranchStanding {
    /// Whether the goal has chosen.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        match self {
            Self::Settled => true,
            Self::Unchosen { .. } => false,
        }
    }
}

/// Routes one overlaid node to one of section 15.2's five kinds, or to none.
///
/// `floor` is the rung the edge into this node needs, from
/// [`crate::graph::blocking_floor`].
#[must_use]
pub fn route(
    state: &ConceptState,
    floor: MasteryLevel,
    branch: &BranchStanding,
) -> Option<GapKind> {
    // The identity first: a mastery reading on a node whose identity is not
    // settled is a reading of a state that `P2-C3` says cannot be attributed to
    // it, so no later rule may act on one.
    if !state.identity().is_settled() {
        return Some(GapKind::OntologyGap);
    }
    if !branch.is_settled() {
        return Some(GapKind::ContextGap);
    }
    // Dimension four. A recorded failure is a deficit at any rung; section 13.1
    // keeps it as contradicting evidence rather than as a verdict, and this is
    // where it is read.
    if !state.contradicting().is_empty() {
        return Some(GapKind::MasteryGap);
    }
    if rung(state.mastery()) >= rung(floor) {
        // Dimension two.
        if rank(state.freshness()) < rank(RETRIEVAL_FLOOR) {
            return Some(GapKind::FreshnessGap);
        }
        return None;
    }
    // Below the floor. Dimension three decides whether that is a statement about
    // the person or about the records.
    if state.unseen_basis() == Some(UnseenBasis::NoEvidenceRecorded)
        || state.sufficiency_gaps().iter().any(is_admission_gap)
    {
        return Some(GapKind::EvidenceGap);
    }
    Some(GapKind::MasteryGap)
}

/// Whether a sufficiency gap says an item could not be admitted, as opposed to
/// saying the admitted evidence is thin.
///
/// `P2-N2`'s first four gaps are the four section 13.4 checks; the last two are
/// about the admitted set itself. Total, with no wildcard arm.
const fn is_admission_gap(gap: &SufficiencyGap) -> bool {
    match gap {
        SufficiencyGap::ConceptLinkUnresolved
        | SufficiencyGap::AuthorshipUnresolved
        | SufficiencyGap::OutcomeUnresolved
        | SufficiencyGap::SourceIntegrityUnresolved => true,
        SufficiencyGap::SingleSupportingItem | SufficiencyGap::Contradicted => false,
    }
}
