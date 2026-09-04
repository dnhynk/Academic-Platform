//! Section 16.5's five groups: `계산 snapshot, 비용 가정, 제외된 목표, 불확실
//! edge, 대안이 항상 노출된다`.
//!
//! ## Five, and none of them optional
//!
//! [`Disclosure`] has five private fields, one per group, and one constructor
//! that takes all five. No `Option`, no `Default`, no public field, no builder
//! that can be finished early. `five_disclosure_groups_are_always_present` is
//! about a value that has no shape without all five, and
//! [`crate::plan::CriticalPathResult`] holds one by value, so a result with a
//! missing group is not a value this crate can produce.
//!
//! ## Empty is stated, never omitted
//!
//! `REQ-16-022` allows `explicit empty lists ... with reason`. Three of the
//! five groups can legitimately have nothing in them -- no goal was excluded,
//! no edge is uncertain, only one route survived -- so each of those three is an
//! enumeration whose empty case **names its reason** rather than a list that is
//! sometimes empty. That is `P2-N5`'s `AlternativePath::None { reason }` applied
//! to section 16.5, and it is what stops `제외된 목표: []` from being ambiguous
//! between *nothing was excluded* and *nobody checked*.
//!
//! The other two -- the snapshot and the cost assumptions -- cannot be empty:
//! there is always a snapshot, and a plan with no cost assumption is a plan with
//! no cost vector, which [`crate::plan::Candidate`] cannot be built without.

use academic_domain::{ContentDigest, EntityId, EvidenceId};
use serde::{Deserialize, Serialize};

use crate::{
    CriticalPathError,
    hypergraph::EdgeStanding,
    vector::{BasisFamily, CostComponent},
};

/// Group one: `계산 snapshot`.
///
/// What the run was over, so a later reader can tell whether a difference in
/// the answer came from a difference in the inputs. Never empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationSnapshot {
    /// The goal the plan is for.
    pub goal: EntityId,
    /// The digest of the frozen inputs the run was evaluated over. `P2-C5`'s
    /// own `FrozenInputs::digest`, so two runs agree exactly when this does.
    pub frozen_inputs: ContentDigest,
    /// The engine version that produced it.
    pub engine_version: u16,
    /// The rule-set hash it was pinned to.
    pub rule_set_hash: ContentDigest,
    /// How many hyperedge members the hypergraph held.
    pub hyperedge_member_count: usize,
    /// How many satisfying sets were found before elimination.
    pub candidate_count: usize,
}

/// Group two: `비용 가정`, one entry per axis whose estimate rested on
/// something other than a full measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostAssumption {
    /// Which of section 16.2's seven axes.
    pub axis: CostComponent,
    /// The interval's ends, in the axis's own unit.
    pub low: u32,
    /// The upper end.
    pub high: u32,
    /// Which of section 16.2's four input families the estimate read. Empty
    /// means it read none, which is `근거가 없으면` and is why the interval has
    /// width.
    pub families: Vec<BasisFamily>,
}

/// Group two as a whole. Never empty: a plan has seven axes and every one of
/// them is an assumption about something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostAssumptions {
    entries: Vec<CostAssumption>,
}

impl CostAssumptions {
    /// Records the assumptions.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::DisclosureGroupIsEmpty`] for an empty list.
    pub fn of(entries: Vec<CostAssumption>) -> Result<Self, CriticalPathError> {
        if entries.is_empty() {
            return Err(CriticalPathError::DisclosureGroupIsEmpty {
                group: "비용 가정"
            });
        }
        Ok(Self { entries })
    }

    /// The entries, one per axis, in [`crate::vector::COST_COMPONENTS`] order.
    #[must_use]
    pub fn entries(&self) -> &[CostAssumption] {
        &self.entries
    }
}

/// Why a route or a goal is not in the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExclusionReason {
    /// Pareto elimination removed it. Section 16.2.
    ParetoDominated,
    /// A section 16.3 constraint refused it.
    ConstraintViolated,
    /// A section 16.3 constraint's input was itself unknown, so nothing could
    /// be concluded. Never folded into either of the other two.
    ConstraintUnknown,
}

impl ExclusionReason {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParetoDominated => "PARETO_DOMINATED",
            Self::ConstraintViolated => "CONSTRAINT_VIOLATED",
            Self::ConstraintUnknown => "CONSTRAINT_UNKNOWN",
        }
    }
}

/// One excluded route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludedRoute {
    /// The concepts the route would have needed, in identifier order.
    pub concepts: Vec<EntityId>,
    /// Why it is not in the answer.
    pub reason: ExclusionReason,
    /// The constraint that refused it, when one did.
    pub constraint: Option<crate::constraint::Constraint>,
}

/// Group three: `제외된 목표`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "exclusions", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Exclusions {
    /// Routes were excluded, and here they are.
    Excluded {
        /// Every excluded route.
        routes: Vec<ExcludedRoute>,
    },
    /// Nothing was excluded, and this says so rather than showing an empty
    /// list that could also mean nobody looked.
    NoneExcluded,
}

impl Exclusions {
    /// Records the exclusions, choosing the right variant for the list.
    #[must_use]
    pub fn of(routes: Vec<ExcludedRoute>) -> Self {
        if routes.is_empty() {
            Self::NoneExcluded
        } else {
            Self::Excluded { routes }
        }
    }

    /// The excluded routes, empty for [`Exclusions::NoneExcluded`].
    #[must_use]
    pub fn routes(&self) -> &[ExcludedRoute] {
        match self {
            Self::Excluded { routes } => routes,
            Self::NoneExcluded => &[],
        }
    }
}

/// One uncertain relation the answer rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncertainEdge {
    /// The concept the relation is stated about.
    pub dependent: EntityId,
    /// The concept it requires.
    pub prerequisite: EntityId,
    /// Which section 7.2 predicate, in `P2-C4`'s own spelling.
    pub predicate: String,
    /// What the answer would become if the relation were wrong.
    pub if_removed: crate::counterfactual::EdgeOutcome,
}

/// Group four: `불확실 edge`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "uncertain_edges", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UncertainEdges {
    /// Some relations are uncertain, and here they are with their sensitivity.
    Uncertain {
        /// Every uncertain relation on a surviving route.
        edges: Vec<UncertainEdge>,
        /// The measured ratio over the ranked-first route, in permille.
        ratio_permille: u16,
    },
    /// Every relation the answer rests on is settled.
    AllSettled,
}

impl UncertainEdges {
    /// Records the uncertain relations.
    #[must_use]
    pub fn of(edges: Vec<UncertainEdge>, ratio_permille: u16) -> Self {
        if edges.is_empty() {
            Self::AllSettled
        } else {
            Self::Uncertain {
                edges,
                ratio_permille,
            }
        }
    }

    /// The uncertain relations, empty for [`UncertainEdges::AllSettled`].
    #[must_use]
    pub fn edges(&self) -> &[UncertainEdge] {
        match self {
            Self::Uncertain { edges, .. } => edges,
            Self::AllSettled => &[],
        }
    }
}

/// One route offered as an alternative to the ranked first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlternativeRoute {
    /// The concepts it needs, in identifier order.
    pub concepts: Vec<EntityId>,
    /// Its rank under the preference in force.
    pub rank: usize,
    /// The section 16.2 name it is shown under, when one fits.
    pub strategy: Option<crate::preference::NamedStrategy>,
    /// Evidence the alternative exists at all: the sources its options supply.
    pub sources: Vec<EvidenceId>,
}

/// Group five: `대안`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "alternatives", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Alternatives {
    /// Other undominated routes exist.
    Routes {
        /// Every surviving route other than the ranked first.
        routes: Vec<AlternativeRoute>,
    },
    /// Exactly one route survived elimination and the constraints.
    ///
    /// Section 16.5 requires the group to be exposed, so a single-route answer
    /// says *this is the only undominated feasible route* rather than printing
    /// an empty list.
    SoleSurvivingRoute,
    /// No route survived at all, so there is nothing to be an alternative to.
    NoFeasibleRoute,
}

impl Alternatives {
    /// Records the alternatives for a ranked list of the given length.
    #[must_use]
    pub fn of(routes: Vec<AlternativeRoute>, ranked_count: usize) -> Self {
        if ranked_count == 0 {
            Self::NoFeasibleRoute
        } else if routes.is_empty() {
            Self::SoleSurvivingRoute
        } else {
            Self::Routes { routes }
        }
    }

    /// The alternative routes, empty for both of the other variants.
    #[must_use]
    pub fn routes(&self) -> &[AlternativeRoute] {
        match self {
            Self::Routes { routes } => routes,
            Self::SoleSurvivingRoute | Self::NoFeasibleRoute => &[],
        }
    }
}

/// Which of section 16.5's five groups a value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisclosureGroup {
    /// `계산 snapshot`.
    ComputationSnapshot,
    /// `비용 가정`.
    CostAssumptions,
    /// `제외된 목표`.
    ExcludedGoals,
    /// `불확실 edge`.
    UncertainEdges,
    /// `대안`.
    Alternatives,
}

/// The five, in section 16.5's own order.
pub const DISCLOSURE_GROUPS: [DisclosureGroup; 5] = [
    DisclosureGroup::ComputationSnapshot,
    DisclosureGroup::CostAssumptions,
    DisclosureGroup::ExcludedGoals,
    DisclosureGroup::UncertainEdges,
    DisclosureGroup::Alternatives,
];

impl DisclosureGroup {
    /// The words section 16.5 uses, verbatim.
    #[must_use]
    pub const fn spec_token(self) -> &'static str {
        match self {
            Self::ComputationSnapshot => "계산 snapshot",
            Self::CostAssumptions => "비용 가정",
            Self::ExcludedGoals => "제외된 목표",
            Self::UncertainEdges => "불확실 edge",
            Self::Alternatives => "대안",
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComputationSnapshot => "COMPUTATION_SNAPSHOT",
            Self::CostAssumptions => "COST_ASSUMPTIONS",
            Self::ExcludedGoals => "EXCLUDED_GOALS",
            Self::UncertainEdges => "UNCERTAIN_EDGES",
            Self::Alternatives => "ALTERNATIVES",
        }
    }
}

/// Section 16.5's five groups, all of them.
///
/// Five private fields, one constructor, no `Default`. See the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disclosure {
    snapshot: ComputationSnapshot,
    cost_assumptions: CostAssumptions,
    exclusions: Exclusions,
    uncertain_edges: UncertainEdges,
    alternatives: Alternatives,
}

impl Disclosure {
    /// Records all five groups.
    #[must_use]
    pub const fn of(
        snapshot: ComputationSnapshot,
        cost_assumptions: CostAssumptions,
        exclusions: Exclusions,
        uncertain_edges: UncertainEdges,
        alternatives: Alternatives,
    ) -> Self {
        Self {
            snapshot,
            cost_assumptions,
            exclusions,
            uncertain_edges,
            alternatives,
        }
    }

    /// Group one.
    #[must_use]
    pub const fn snapshot(&self) -> &ComputationSnapshot {
        &self.snapshot
    }

    /// Group two.
    #[must_use]
    pub const fn cost_assumptions(&self) -> &CostAssumptions {
        &self.cost_assumptions
    }

    /// Group three.
    #[must_use]
    pub const fn exclusions(&self) -> &Exclusions {
        &self.exclusions
    }

    /// Group four.
    #[must_use]
    pub const fn uncertain_edges(&self) -> &UncertainEdges {
        &self.uncertain_edges
    }

    /// Group five.
    #[must_use]
    pub const fn alternatives(&self) -> &Alternatives {
        &self.alternatives
    }

    /// Whether the named group is populated with something rather than stating
    /// its own emptiness.
    ///
    /// Total over [`DisclosureGroup`] with no wildcard arm. A group is
    /// *present* either way; this answers the narrower question of whether it
    /// carries entries, which is what the acceptance suite drives both ways.
    #[must_use]
    pub fn group_has_entries(&self, group: DisclosureGroup) -> bool {
        match group {
            DisclosureGroup::ComputationSnapshot => true,
            DisclosureGroup::CostAssumptions => !self.cost_assumptions.entries().is_empty(),
            DisclosureGroup::ExcludedGoals => !self.exclusions.routes().is_empty(),
            DisclosureGroup::UncertainEdges => !self.uncertain_edges.edges().is_empty(),
            DisclosureGroup::Alternatives => !self.alternatives.routes().is_empty(),
        }
    }
}

/// The standings an uncertain-edge disclosure is about.
///
/// Total over [`EdgeStanding`] with no wildcard arm, so a third standing added
/// to that enumeration is a compile error here rather than an edge silently
/// left out of group four.
#[must_use]
pub const fn is_disclosed_standing(standing: EdgeStanding) -> bool {
    match standing {
        EdgeStanding::Uncertain => true,
        EdgeStanding::Settled => false,
    }
}
