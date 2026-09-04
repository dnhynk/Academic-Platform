//! What "meets the budget" means here, and what it does not.
//!
//! # It is a count budget, not a stopwatch
//!
//! `five_thousand_node_fixture_meets_the_budget` measures **how much the atlas
//! materialises** and **how much work the layout does**, both of which are
//! deterministic functions of the fixture. It does not measure elapsed time.
//! Wall-clock on a shared machine measures the machine, and a suite that
//! asserted on it would either be flaky or have a ceiling so high that nothing
//! could violate it.
//!
//! What that leaves open is honest and is written down in
//! `docs/contracts/cs-map-atlas.md`: no frame is rendered here, no window opens,
//! and this crate is not evidence that a real renderer draws five thousand nodes
//! in any particular time.
//!
//! What it *is* evidence for is the thing section 25.3's first sentence is
//! about — that the first screen is `10–20개 Field cluster와 현재 선택한 goal
//! neighborhood` and not `수천 node` — plus the fact that the layout behind it
//! is linear rather than a relaxation.

use serde::Serialize;

use crate::CsMapError;

/// The five ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RenderBudget {
    /// How many identities section 25.3's first screen may materialise.
    #[serde(rename = "initialViewNodes")]
    pub initial_view_nodes: usize,
    /// How many nodes `Z2 Concept` may show around the goal.
    #[serde(rename = "goalNearNodes")]
    pub goal_near_nodes: usize,
    /// How many nodes `Z3 Evidence` may show around the goal.
    #[serde(rename = "evidenceNodes")]
    pub evidence_nodes: usize,
    /// How many layout steps are admitted per node in the graph.
    ///
    /// A ceiling *per node* rather than an absolute one, so it stays a linearity
    /// claim as the fixture grows. A relaxation pass or an all-pairs step would
    /// break it at any size.
    #[serde(rename = "layoutWorkUnitsPerNode")]
    pub layout_work_units_per_node: usize,
    /// How many hops a search reveal's route may be.
    #[serde(rename = "searchPathHops")]
    pub search_path_hops: usize,
}

/// The budget this crate ships.
pub const ATLAS_BUDGET: RenderBudget = RenderBudget {
    initial_view_nodes: 64,
    goal_near_nodes: 256,
    evidence_nodes: 64,
    layout_work_units_per_node: 3,
    search_path_hops: 12,
};

/// Which ceiling a reading broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BudgetMeasure {
    /// [`RenderBudget::initial_view_nodes`].
    InitialViewNodes,
    /// [`RenderBudget::goal_near_nodes`].
    GoalNearNodes,
    /// [`RenderBudget::evidence_nodes`].
    EvidenceNodes,
    /// [`RenderBudget::layout_work_units_per_node`].
    LayoutWorkUnits,
    /// [`RenderBudget::search_path_hops`].
    SearchPathHops,
}

/// The five measures, in the order [`BudgetReading::within`] checks them.
pub const BUDGET_MEASURES: [BudgetMeasure; 5] = [
    BudgetMeasure::InitialViewNodes,
    BudgetMeasure::GoalNearNodes,
    BudgetMeasure::EvidenceNodes,
    BudgetMeasure::LayoutWorkUnits,
    BudgetMeasure::SearchPathHops,
];

impl BudgetMeasure {
    /// The stable wire discriminant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InitialViewNodes => "INITIAL_VIEW_NODES",
            Self::GoalNearNodes => "GOAL_NEAR_NODES",
            Self::EvidenceNodes => "EVIDENCE_NODES",
            Self::LayoutWorkUnits => "LAYOUT_WORK_UNITS",
            Self::SearchPathHops => "SEARCH_PATH_HOPS",
        }
    }
}

/// One measurement of a laid-out atlas.
///
/// Every field is counted from a value the crate produced, so a reading cannot
/// disagree with the atlas it describes without somebody writing a different
/// number in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BudgetReading {
    /// How many nodes the graph holds.
    #[serde(rename = "nodeCount")]
    pub node_count: usize,
    /// How many identities the first screen materialised.
    #[serde(rename = "initialViewNodes")]
    pub initial_view_nodes: usize,
    /// How many nodes `Z2 Concept` showed.
    #[serde(rename = "goalNearNodes")]
    pub goal_near_nodes: usize,
    /// How many nodes `Z3 Evidence` showed.
    #[serde(rename = "evidenceNodes")]
    pub evidence_nodes: usize,
    /// How many steps the layout took.
    #[serde(rename = "layoutWorkUnits")]
    pub layout_work_units: usize,
    /// How many hops the measured search reveal's route was.
    #[serde(rename = "searchPathHops")]
    pub search_path_hops: usize,
}

impl BudgetReading {
    /// Checks every one of the five ceilings.
    ///
    /// All five are checked in [`BUDGET_MEASURES`] order and the first breach is
    /// returned, so a reading that broke two is not reported as breaking one and
    /// then silently passing when that one is fixed.
    ///
    /// # Errors
    ///
    /// [`CsMapError::BudgetExceeded`], naming the measure, what was measured and
    /// what the ceiling was.
    pub fn within(&self, budget: &RenderBudget) -> Result<(), CsMapError> {
        let node_count = self.node_count.max(1);
        for measure in BUDGET_MEASURES {
            let (measured, ceiling) = match measure {
                BudgetMeasure::InitialViewNodes => {
                    (self.initial_view_nodes, budget.initial_view_nodes)
                }
                BudgetMeasure::GoalNearNodes => (self.goal_near_nodes, budget.goal_near_nodes),
                BudgetMeasure::EvidenceNodes => (self.evidence_nodes, budget.evidence_nodes),
                BudgetMeasure::LayoutWorkUnits => (
                    self.layout_work_units,
                    budget.layout_work_units_per_node * node_count,
                ),
                BudgetMeasure::SearchPathHops => (self.search_path_hops, budget.search_path_hops),
            };
            if measured > ceiling {
                return Err(CsMapError::BudgetExceeded {
                    measure: measure.as_str(),
                    measured,
                    ceiling,
                });
            }
        }
        Ok(())
    }
}
