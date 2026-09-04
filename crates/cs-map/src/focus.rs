//! Section 26.4's five focus modes.
//!
//! Each mode names a node set and an edge rule, and [`focus`] returns exactly
//! that set. `five_focus_modes_return_exact_subgraphs` compares each against a
//! set written out by hand in both directions, so a mode that returned a
//! superset would fail as loudly as one that returned nothing.
//!
//! # The five edge rules, written down rather than assumed
//!
//! | Mode | Nodes | Edges kept |
//! |---|---|---|
//! | goal | the goal, every node reachable from it along `REQUIRES`, and the one-hop `REQUIRES` predecessors | `REQUIRES` between two selected nodes |
//! | local neighbourhood | every node within `hops` of the centre, moving only along the caller's edge-type filter | a filtered predicate between two selected nodes |
//! | evidence | the node and every node its state assertion is `EVIDENCED_BY` | `EVIDENCED_BY` between two selected nodes |
//! | uncertainty | every node whose reading is disputed, AI-inferred, or below the confidence threshold | every edge between two selected nodes |
//! | course | the revision's `DESIGNED_TO_TEACH` targets and the offering lectures' `TAUGHT_IN` sources | `DESIGNED_TO_TEACH` or `TAUGHT_IN` between two selected nodes |
//!
//! # `BUILDS_ON` is deliberately not a prerequisite here
//!
//! Section 7.2 separates the two in so many words: `REQUIRES` is a hard or
//! near-hard dependency and `BUILDS_ON` `반드시 선행해야 하는 것은 아닐 수 있음`.
//! A goal focus that walked both would show optional foundations as blocking
//! prerequisites, which is the reading section 15's gap engine refuses one layer
//! down.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use academic_domain::{ConfidencePermille, EntityId, EpistemicStatus, predicates::PredicateName};
use serde::Serialize;

use crate::{
    CsMapError,
    encoding::NodeReading,
    graph::{MapEdge, MapGraph},
};

/// The fewest hops a local-neighbourhood focus admits.
pub const MIN_HOPS: u8 = 1;

/// The most hops a local-neighbourhood focus admits.
pub const MAX_HOPS: u8 = 3;

/// A hop count section 26.4 admits: one, two or three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct HopCount(u8);

impl HopCount {
    /// Admits a hop count in `1..=3`.
    ///
    /// # Errors
    ///
    /// [`CsMapError::HopsOutOfRange`]. Zero is refused as well as four: a
    /// zero-hop neighbourhood is the node by itself, which is not a
    /// neighbourhood, and section 26.4 writes the range as `1–3 hop`.
    pub fn new(hops: u8) -> Result<Self, CsMapError> {
        if !(MIN_HOPS..=MAX_HOPS).contains(&hops) {
            return Err(CsMapError::HopsOutOfRange { hops });
        }
        Ok(Self(hops))
    }

    /// The hop count.
    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Which of section 26.4's five modes a focus is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FocusKind {
    /// `goal focus`.
    Goal,
    /// `local neighborhood`.
    LocalNeighbourhood,
    /// `evidence focus`.
    Evidence,
    /// `uncertainty focus`.
    Uncertainty,
    /// `course focus`.
    Course,
}

/// The five modes, in section 26.4's bullet order.
pub const FOCUS_KINDS: [FocusKind; 5] = [
    FocusKind::Goal,
    FocusKind::LocalNeighbourhood,
    FocusKind::Evidence,
    FocusKind::Uncertainty,
    FocusKind::Course,
];

impl FocusKind {
    /// The mode's own words at the head of its section 26.4 bullet.
    #[must_use]
    pub const fn spec_bullet_head(self) -> &'static str {
        match self {
            Self::Goal => "goal focus",
            Self::LocalNeighbourhood => "local neighborhood",
            Self::Evidence => "evidence focus",
            Self::Uncertainty => "uncertainty focus",
            Self::Course => "course focus",
        }
    }
}

/// One focus request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusMode {
    /// The goal's ancestor prerequisites and its immediate downstream.
    Goal {
        /// The goal focused on.
        goal: EntityId,
    },
    /// One to three hops around a centre, along chosen edge types only.
    LocalNeighbourhood {
        /// The node focused around.
        centre: EntityId,
        /// How far to walk.
        hops: HopCount,
        /// Which of section 7.2's twenty edges to walk. Never empty.
        edge_types: BTreeSet<PredicateName>,
    },
    /// Only the evidence that produced a node's state.
    Evidence {
        /// The node whose state is being explained.
        node: EntityId,
    },
    /// Only what is disputed, AI-inferred or below a confidence threshold.
    Uncertainty {
        /// The threshold section 25.3's left rail supplies.
        below: ConfidencePermille,
    },
    /// A course revision's designed coverage beside one offering's actual coverage.
    Course {
        /// The `COURSE_REVISION` whose `DESIGNED_TO_TEACH` edges are the design.
        revision: EntityId,
        /// The offering's lectures.
        ///
        /// An offering's containment of its lectures is a section 9 aggregate
        /// and **not** one of section 7.2's twenty edges, so it arrives as an
        /// argument rather than as an edge this crate invented.
        offering_lectures: BTreeSet<EntityId>,
    },
}

impl FocusMode {
    /// Which of the five this is.
    #[must_use]
    pub const fn kind(&self) -> FocusKind {
        match self {
            Self::Goal { .. } => FocusKind::Goal,
            Self::LocalNeighbourhood { .. } => FocusKind::LocalNeighbourhood,
            Self::Evidence { .. } => FocusKind::Evidence,
            Self::Uncertainty { .. } => FocusKind::Uncertainty,
            Self::Course { .. } => FocusKind::Course,
        }
    }
}

/// Which side of a course comparison a concept sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageSide {
    /// The revision designs it and the offering did not reach it.
    DesignedOnly,
    /// The offering taught it and the revision does not design it.
    ActualOnly,
    /// Both.
    Both,
}

/// The exact result of one focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Subgraph {
    /// Which mode produced this.
    pub kind: FocusKind,
    /// The nodes, in identity order.
    pub nodes: BTreeSet<EntityId>,
    /// The edges, in `(from, to, predicate)` order.
    pub edges: BTreeSet<MapEdge>,
    /// Which side each node sits on. Empty for every mode but
    /// [`FocusKind::Course`], because only that comparison has sides.
    pub coverage: BTreeMap<EntityId, CoverageSide>,
}

/// Computes one focus.
///
/// `readings` is consulted only by [`FocusMode::Uncertainty`]; the other four
/// modes are functions of the graph alone. A node with no reading is **not**
/// uncertain: it has no state to be uncertain about, and admitting it would make
/// the uncertainty focus a list of everything the graph has not been told about.
///
/// # Errors
///
/// * [`CsMapError::NodeNotOnTheMap`] — the goal, centre, node or revision is
///   not in the graph, or an offering lecture is not.
/// * [`CsMapError::EmptyEdgeTypeFilter`] — a local neighbourhood with no edge
///   type to walk, which would always return the centre by itself.
pub fn focus(
    graph: &MapGraph,
    readings: &BTreeMap<EntityId, NodeReading>,
    mode: &FocusMode,
) -> Result<Subgraph, CsMapError> {
    let (nodes, admitted) = match mode {
        FocusMode::Goal { goal } => {
            require_node(graph, *goal)?;
            let mut selected: BTreeSet<EntityId> = [*goal].into_iter().collect();
            let mut frontier = VecDeque::from([*goal]);
            while let Some(current) = frontier.pop_front() {
                for edge in graph.edges() {
                    if edge.from == current
                        && edge.predicate == PredicateName::Requires
                        && selected.insert(edge.to)
                    {
                        frontier.push_back(edge.to);
                    }
                }
            }
            for edge in graph.edges() {
                if edge.to == *goal && edge.predicate == PredicateName::Requires {
                    selected.insert(edge.from);
                }
            }
            (selected, Admitted::Only([PredicateName::Requires].into()))
        }
        FocusMode::LocalNeighbourhood {
            centre,
            hops,
            edge_types,
        } => {
            require_node(graph, *centre)?;
            if edge_types.is_empty() {
                return Err(CsMapError::EmptyEdgeTypeFilter);
            }
            let mut selected: BTreeSet<EntityId> = [*centre].into_iter().collect();
            let mut frontier = VecDeque::from([(*centre, 0_u8)]);
            while let Some((current, depth)) = frontier.pop_front() {
                if depth == hops.value() {
                    continue;
                }
                for edge in graph.edges() {
                    if !edge_types.contains(&edge.predicate) {
                        continue;
                    }
                    for (near, far) in [(edge.from, edge.to), (edge.to, edge.from)] {
                        if near == current && selected.insert(far) {
                            frontier.push_back((far, depth + 1));
                        }
                    }
                }
            }
            (selected, Admitted::Only(edge_types.clone()))
        }
        FocusMode::Evidence { node } => {
            require_node(graph, *node)?;
            let mut selected: BTreeSet<EntityId> = [*node].into_iter().collect();
            for edge in graph.edges() {
                if edge.from == *node && edge.predicate == PredicateName::EvidencedBy {
                    selected.insert(edge.to);
                }
            }
            (
                selected,
                Admitted::Only([PredicateName::EvidencedBy].into()),
            )
        }
        FocusMode::Uncertainty { below } => {
            let selected = graph
                .nodes()
                .filter(|node| {
                    readings.get(&node.id()).is_some_and(|reading| {
                        matches!(
                            reading.status,
                            EpistemicStatus::Disputed | EpistemicStatus::AiInferred
                        ) || reading
                            .confidence
                            .is_some_and(|value| value.value() < below.value())
                    })
                })
                .map(|node| node.id())
                .collect();
            (selected, Admitted::Any)
        }
        FocusMode::Course {
            revision,
            offering_lectures,
        } => {
            require_node(graph, *revision)?;
            for lecture in offering_lectures {
                require_node(graph, *lecture)?;
            }
            let mut designed: BTreeSet<EntityId> = BTreeSet::new();
            let mut actual: BTreeSet<EntityId> = BTreeSet::new();
            for edge in graph.edges() {
                if edge.from == *revision && edge.predicate == PredicateName::DesignedToTeach {
                    designed.insert(edge.to);
                }
                if offering_lectures.contains(&edge.to) && edge.predicate == PredicateName::TaughtIn
                {
                    actual.insert(edge.from);
                }
            }
            let mut selected: BTreeSet<EntityId> = designed.union(&actual).copied().collect();
            selected.insert(*revision);
            selected.extend(offering_lectures.iter().copied());
            let mut coverage = BTreeMap::new();
            for concept in designed.union(&actual) {
                let side = match (designed.contains(concept), actual.contains(concept)) {
                    (true, true) => CoverageSide::Both,
                    (true, false) => CoverageSide::DesignedOnly,
                    (false, true) => CoverageSide::ActualOnly,
                    (false, false) => continue,
                };
                coverage.insert(*concept, side);
            }
            return Ok(Subgraph {
                kind: mode.kind(),
                edges: induced(
                    graph,
                    &selected,
                    &Admitted::Only(
                        [PredicateName::DesignedToTeach, PredicateName::TaughtIn].into(),
                    ),
                ),
                nodes: selected,
                coverage,
            });
        }
    };

    Ok(Subgraph {
        kind: mode.kind(),
        edges: induced(graph, &nodes, &admitted),
        nodes,
        coverage: BTreeMap::new(),
    })
}

enum Admitted {
    Any,
    Only(BTreeSet<PredicateName>),
}

fn induced(graph: &MapGraph, nodes: &BTreeSet<EntityId>, admitted: &Admitted) -> BTreeSet<MapEdge> {
    graph
        .edges()
        .iter()
        .filter(|edge| nodes.contains(&edge.from) && nodes.contains(&edge.to))
        .filter(|edge| match admitted {
            Admitted::Any => true,
            Admitted::Only(kinds) => kinds.contains(&edge.predicate),
        })
        .copied()
        .collect()
}

fn require_node(graph: &MapGraph, node: EntityId) -> Result<(), CsMapError> {
    if graph.node(node).is_none() {
        return Err(CsMapError::NodeNotOnTheMap { node });
    }
    Ok(())
}
