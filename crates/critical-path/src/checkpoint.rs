//! Section 16.3's eighth constraint: `불확실 edge가 일정 비율을 넘을 때
//! diagnostic checkpoint 삽입`.
//!
//! ## The ratio and its denominator are both stated
//!
//! `REQ-16-017` leaves `비율 임계치·계산 분모` open. This file closes both, in
//! one place, and pins them:
//!
//! * The **denominator** is every hyperedge member the satisfying set was
//!   reached through, not every edge in the graph. An edge on a branch the set
//!   did not take is not an assumption the set rests on, so counting it would
//!   make a plan's own uncertainty depend on how many alternatives the graph
//!   happens to hold.
//! * The **threshold** is [`UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE`], and it
//!   is `넘을 때` -- strictly above. A set exactly at the threshold does not get
//!   a checkpoint, which is what `또는 같을 때` would have said and does not.
//!
//! A set with no members has ratio zero and no checkpoint: there is no
//! assumption to be uncertain about. That is deliberate rather than an
//! arithmetic accident, and it is why [`uncertain_edge_ratio_permille`] answers
//! zero rather than dividing by nothing.
//!
//! ## Where it goes
//!
//! `REQ-16-017` says `before branch commitment`. The checkpoint is a
//! [`crate::constraint::RequiredInsertion`] on the eighth constraint's finding
//! and the plan's first step, so it is reached before any concept on the set is
//! studied. [`crate::plan::PlanStep`] holds it in position zero.

use serde::{Deserialize, Serialize};

use crate::hypergraph::{EdgeStanding, SatisfyingSet};

/// The `일정 비율` of section 16.3's eighth bullet, in permille.
///
/// Three hundred permille: a plan resting on more than three uncertain
/// relations in ten is one whose shape is a guess, and section 34.5's recovery
/// for an over-simplified critical path is `비용/edge 수정 후 재계산`, which the
/// user cannot do without being told which edges to look at.
///
/// Pinned by `the_critical_path_decisions_are_pinned`.
pub const UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE: u16 = 300;

/// The share of a satisfying set's members that are
/// [`EdgeStanding::Uncertain`], in permille.
///
/// Integer arithmetic throughout: the numerator is multiplied by a thousand
/// before the division, so the result is a function of the two counts and not
/// of a floating-point rounding mode.
#[must_use]
pub fn uncertain_edge_ratio_permille(set: &SatisfyingSet) -> u16 {
    let member_count = set.members().len();
    if member_count == 0 {
        return 0;
    }
    let uncertain = set
        .members()
        .iter()
        .filter(|member| member.standing() == EdgeStanding::Uncertain)
        .count();
    let permille = uncertain.saturating_mul(1000) / member_count;
    u16::try_from(permille).unwrap_or(1000)
}

/// Whether the eighth constraint inserts a checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckpointDecision {
    /// At or below the threshold. `넘을 때` is strict.
    BelowThreshold,
    /// Above the threshold: a diagnostic checkpoint precedes the plan.
    Insert,
}

impl CheckpointDecision {
    /// The decision for one measured ratio.
    ///
    /// Pinned by `the_critical_path_decisions_are_pinned` so the comparison
    /// cannot become `>=` without the pin moving.
    #[must_use]
    pub const fn for_ratio(ratio_permille: u16) -> Self {
        if ratio_permille > UNCERTAIN_EDGE_RATIO_THRESHOLD_PERMILLE {
            Self::Insert
        } else {
            Self::BelowThreshold
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BelowThreshold => "BELOW_THRESHOLD",
            Self::Insert => "INSERT",
        }
    }
}
