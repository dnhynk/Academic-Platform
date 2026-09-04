//! Section 26.1's stable multiscale atlas: four semantic zoom levels, integer
//! coordinates that are a pure function of identity, and a bounded drift when
//! the graph grows.
//!
//! # Why the coordinates are integers
//!
//! No `f32` or `f64` is declared anywhere in this crate. A coordinate is
//! thousandths of a layout unit in an `i32`, so two layouts of the same graph
//! compare byte for byte and a golden coordinate file is a comparison rather
//! than an approximation. That is what makes
//! `landmark_coordinates_stay_within_tolerance` a measurement.
//!
//! # Why the layout is a lattice and not a force simulation
//!
//! Section 26.1's first sentence is that the map is *not* an unconstrained
//! force-directed hairball. A relaxation would move every landmark whenever any
//! node was added, which is exactly the spatial-memory loss section 19 forbids.
//! Here a cluster's anchor is
//!
//! ```text
//! cell   = digest(cluster identity) mod (GRID * GRID)
//! column = cell mod GRID,  row = cell / GRID
//! pitch  = PITCH_BASE + growth_band(node count) * PITCH_STEP
//! anchor = ((column - GRID/2) * pitch, (row - GRID/2) * pitch)
//! ```
//!
//! and a member sits at its anchor plus an offset that is a pure function of
//! its own identity. So:
//!
//! * changing the lens, the overlays, the focus mode or the zoom level moves
//!   **nothing**, because none of them is an input to any line above;
//! * adding nodes inside one growth band moves **nothing**;
//! * any two sizes at all put a landmark at most [`LAYOUT_TOLERANCE_MILLI`]
//!   apart, because the pitch has only [`MAX_GROWTH_BAND`] steps to range over.
//!
//! A layout that scaled the pitch by the raw node count instead of the band
//! would have no bound at all, which is how the tolerance is kept from being a
//! number nothing can violate.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use academic_domain::{EntityId, predicates::NodeType};
use serde::Serialize;

use crate::{
    CsMapError,
    graph::{ClusterId, MapGraph},
};

/// Section 26.1's four zoom levels, in the table's own order.
///
/// A zoom level is a **semantic level**, not a scale factor: each arm names a
/// different node set and a different type set, which is why
/// [`Atlas::level`] returns a [`LevelView`] and there is no `scale` anywhere in
/// this crate. `zoom_changes_semantic_level_not_only_scale` is the measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SemanticZoom {
    /// `Z0 Ecosystem`: fields; individual concepts hidden.
    Ecosystem,
    /// `Z1 Domain`: domain clusters and major bridges; detailed operations hidden.
    Domain,
    /// `Z2 Concept`: goal-near concepts, prerequisites and overlays; distant nodes hidden.
    Concept,
    /// `Z3 Evidence`: relations, evidence cards and locators; the global graph hidden.
    Evidence,
}

/// The four zoom levels, in section 26.1's table order.
pub const SEMANTIC_ZOOMS: [SemanticZoom; 4] = [
    SemanticZoom::Ecosystem,
    SemanticZoom::Domain,
    SemanticZoom::Concept,
    SemanticZoom::Evidence,
];

impl SemanticZoom {
    /// The table's own row label, `Z0 Ecosystem` through `Z3 Evidence`.
    #[must_use]
    pub const fn spec_label(self) -> &'static str {
        match self {
            Self::Ecosystem => "Z0 Ecosystem",
            Self::Domain => "Z1 Domain",
            Self::Concept => "Z2 Concept",
            Self::Evidence => "Z3 Evidence",
        }
    }

    /// The stable wire discriminant.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ecosystem => "ECOSYSTEM",
            Self::Domain => "DOMAIN",
            Self::Concept => "CONCEPT",
            Self::Evidence => "EVIDENCE",
        }
    }

    /// The section 7.1 node types this level shows.
    ///
    /// The sets are **nested in neither direction**: `Z1` adds `CONCEPT` to
    /// `Z0`'s `FIELD`, `Z2` drops `FIELD` and adds `CONCEPT_SENSE`, and `Z3`
    /// drops that again for the evidence tier. A level that only scaled would
    /// show the same types at every step.
    #[must_use]
    pub fn shown_types(self) -> BTreeSet<NodeType> {
        match self {
            Self::Ecosystem => [NodeType::Field].into_iter().collect(),
            Self::Domain => [NodeType::Field, NodeType::Concept].into_iter().collect(),
            Self::Concept => [NodeType::Concept, NodeType::ConceptSense]
                .into_iter()
                .collect(),
            Self::Evidence => [
                NodeType::Concept,
                NodeType::Claim,
                NodeType::EvidenceItem,
                NodeType::Lecture,
                NodeType::CodeComponent,
            ]
            .into_iter()
            .collect(),
        }
    }

    /// How far from the selected goal this level reaches, in hops.
    ///
    /// `None` at the two coarse levels, which the section 26.1 table scopes by
    /// type rather than by distance: `Z0` hides `개별 concept` and `Z1` hides
    /// `세부 operation`. `Z2` hides `먼 주변 node` and `Z3` hides the
    /// `전역 graph`, so those two are the ones with a horizon.
    #[must_use]
    pub const fn horizon(self) -> Option<u8> {
        match self {
            Self::Ecosystem | Self::Domain => None,
            Self::Concept => Some(2),
            Self::Evidence => Some(1),
        }
    }
}

/// A position on the atlas, in thousandths of a layout unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Coordinate {
    /// Horizontal position, in thousandths of a layout unit.
    pub x_milli: i32,
    /// Vertical position, in thousandths of a layout unit.
    pub y_milli: i32,
}

impl Coordinate {
    /// The larger of the two axis displacements between `self` and `other`.
    ///
    /// Chebyshev rather than Euclidean, so no square root and therefore no
    /// float is needed to state a tolerance.
    #[must_use]
    pub const fn displacement(self, other: Self) -> i64 {
        let dx = (self.x_milli as i64 - other.x_milli as i64).abs();
        let dy = (self.y_milli as i64 - other.y_milli as i64).abs();
        if dx > dy { dx } else { dy }
    }
}

/// Cells per side of the anchor lattice.
pub const GRID: i64 = 8;

/// The lattice pitch at growth band zero, in thousandths of a layout unit.
pub const PITCH_BASE: i64 = 100_000;

/// How much the lattice pitch grows per growth band.
pub const PITCH_STEP: i64 = 2_000;

/// How many nodes fit in one growth band.
pub const GROWTH_BAND_SIZE: usize = 1_024;

/// The highest growth band; beyond it the pitch stops growing.
pub const MAX_GROWTH_BAND: usize = 7;

/// The furthest a landmark can move between **any** two layouts of one cluster
/// set, at any two graph sizes.
///
/// The pitch ranges over [`MAX_GROWTH_BAND`] steps of [`PITCH_STEP`] and the
/// outermost lattice column is `GRID / 2` cells from the origin, so the product
/// of the three is a bound no amount of growth can pass. It is attained: a
/// layout in band zero and one in band seven put an outer landmark exactly this
/// far apart, and `landmark_coordinates_stay_within_tolerance` measures that
/// pair so the bound is known to be tight rather than merely large.
///
/// A layout whose pitch followed the raw node count instead of the band would
/// have no bound at all, which is the failure this constant exists to name.
pub const LAYOUT_TOLERANCE_MILLI: i64 = (GRID / 2) * PITCH_STEP * MAX_GROWTH_BAND as i64;

/// Half the width of the square a member is scattered into around its anchor.
const MEMBER_SPAN: i64 = 20_000;

/// FNV-1a over the identity's sixteen opaque bytes.
///
/// Not a cryptographic digest and not used as one: it is a spreading function
/// whose only requirement is that it is the same on every platform, which is
/// why it is written out here rather than taken from `DefaultHasher` (whose
/// output Rust does not promise to keep stable).
fn spread(id: EntityId) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Which growth band a graph of `node_count` nodes sits in.
#[must_use]
pub fn growth_band(node_count: usize) -> usize {
    (node_count / GROWTH_BAND_SIZE).min(MAX_GROWTH_BAND)
}

/// One node's placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Placement {
    /// The node placed.
    pub node: EntityId,
    /// Where it sits.
    pub at: Coordinate,
}

/// A cluster and the landmark a viewer navigates by.
///
/// The landmark **is** the cluster's own `FIELD` node, and it sits exactly on
/// the anchor: no scatter offset is applied to it. That is a decision about
/// stability rather than a shortcut. A landmark chosen from the cluster's
/// members — the highest-degree one, the lowest identity, the most recently
/// used — moves to a different node whenever the membership changes, and a
/// landmark that moves to a different node has not "stayed within tolerance";
/// it has been replaced by another landmark that happens to be nearby. Section
/// 26.1 asks for `cluster 경계와 주요 landmark의 위치` to be kept, and the
/// cluster's own node is the one thing whose identity growth cannot change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Landmark {
    /// The cluster this landmark anchors.
    pub cluster: ClusterId,
    /// The node a viewer recognises the cluster by.
    pub node: EntityId,
    /// Where that node sits.
    pub at: Coordinate,
}

/// What one zoom level shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LevelView {
    /// The level this is.
    pub zoom: SemanticZoom,
    /// The node types this level admits, which differ level by level.
    #[serde(serialize_with = "serialize_node_types")]
    pub types: BTreeSet<NodeType>,
    /// The nodes actually shown, in identity order.
    pub nodes: BTreeSet<EntityId>,
}

fn serialize_node_types<S>(value: &BTreeSet<NodeType>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.collect_seq(value.iter().map(|kind| kind.as_str()))
}

/// Section 25.3's first screen: the field clusters plus the goal neighbourhood.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitialView {
    /// Every field cluster, in identity order.
    pub clusters: Vec<ClusterId>,
    /// The selected goal.
    pub goal: EntityId,
    /// The goal and its one-hop neighbourhood, in identity order.
    pub goal_neighbourhood: BTreeSet<EntityId>,
}

impl InitialView {
    /// Every identity this view materialises, clusters and neighbourhood alike.
    ///
    /// The whole point of section 25.3's first sentence is that this is *not*
    /// the graph, so the set is exposed for comparison rather than implied by a
    /// count.
    #[must_use]
    pub fn materialised(&self) -> BTreeSet<EntityId> {
        self.clusters
            .iter()
            .map(|cluster| cluster.entity())
            .chain(self.goal_neighbourhood.iter().copied())
            .collect()
    }
}

/// The fewest field clusters section 25.3 admits on the first screen.
pub const MIN_INITIAL_CLUSTERS: usize = 10;

/// The most field clusters section 25.3 admits on the first screen.
pub const MAX_INITIAL_CLUSTERS: usize = 20;

/// A laid-out map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atlas {
    band: usize,
    anchors: BTreeMap<ClusterId, Coordinate>,
    placements: BTreeMap<EntityId, Coordinate>,
    landmarks: Vec<Landmark>,
    work_units: usize,
}

/// Lays the graph out.
///
/// The result is a pure function of the identities in `graph` and of its node
/// count. No lens, overlay, focus mode, zoom level or instant is a parameter,
/// which is section 19's stability rule stated as a signature rather than as a
/// promise.
///
/// # Errors
///
/// Returns [`CsMapError::EmptyAtlas`] when the graph declares no cluster: an
/// atlas of nothing would satisfy every comparison drawn over it.
pub fn lay_out(graph: &MapGraph) -> Result<Atlas, CsMapError> {
    if graph.clusters().is_empty() {
        return Err(CsMapError::EmptyAtlas);
    }
    if graph.clusters().len() > (GRID * GRID) as usize {
        return Err(CsMapError::ClusterCountOutOfRange {
            count: graph.clusters().len(),
        });
    }

    let band = growth_band(graph.node_count());
    let pitch = PITCH_BASE + (band as i64) * PITCH_STEP;
    let mut work_units = 0_usize;

    let mut anchors = BTreeMap::new();
    let mut taken: BTreeSet<i64> = BTreeSet::new();
    for cluster in graph.clusters() {
        work_units += 1;
        let preferred = (spread(cluster.entity()) % (GRID * GRID) as u64) as i64;
        // Two clusters whose identities prefer one cell would be drawn on top of
        // each other, and a map with two fields in one place is not an atlas.
        // The probe walks forward in identity order, which makes the assignment
        // a function of the **cluster set** rather than of one identity: adding
        // or removing a field can move an anchor. That is the boundary of the
        // stability claim and it is stated in
        // `docs/contracts/cs-map-atlas.md` rather than hidden — a field appearing
        // or disappearing is an ontology change, which section 26.5 already
        // draws with its own transition.
        let mut cell = preferred;
        while !taken.insert(cell) {
            cell = (cell + 1) % (GRID * GRID);
        }
        let column = cell % GRID;
        let row = cell / GRID;
        anchors.insert(
            *cluster,
            Coordinate {
                x_milli: ((column - GRID / 2) * pitch) as i32,
                y_milli: ((row - GRID / 2) * pitch) as i32,
            },
        );
    }

    let mut placements = BTreeMap::new();
    for node in graph.nodes() {
        work_units += 1;
        let anchor =
            anchors
                .get(&node.cluster())
                .copied()
                .ok_or(CsMapError::NodeOutsideEveryCluster {
                    node: node.id(),
                    cluster: node.cluster(),
                })?;
        if node.id() == node.cluster().entity() {
            placements.insert(node.id(), anchor);
            continue;
        }
        let scatter = spread(node.id());
        let dx = (scatter % (MEMBER_SPAN as u64 * 2)) as i64 - MEMBER_SPAN;
        let dy =
            ((scatter / (MEMBER_SPAN as u64 * 2)) % (MEMBER_SPAN as u64 * 2)) as i64 - MEMBER_SPAN;
        placements.insert(
            node.id(),
            Coordinate {
                x_milli: (i64::from(anchor.x_milli) + dx) as i32,
                y_milli: (i64::from(anchor.y_milli) + dy) as i32,
            },
        );
    }

    let mut landmarks = Vec::new();
    for cluster in graph.clusters() {
        work_units += 1;
        let at = placements
            .get(&cluster.entity())
            .copied()
            .ok_or(CsMapError::EmptyAtlas)?;
        landmarks.push(Landmark {
            cluster: *cluster,
            node: cluster.entity(),
            at,
        });
    }

    Ok(Atlas {
        band,
        anchors,
        placements,
        landmarks,
        work_units,
    })
}

/// Every node within `horizon` undirected hops of `from`, including `from`.
fn reachable(graph: &MapGraph, from: EntityId, horizon: u8) -> BTreeSet<EntityId> {
    let mut seen: BTreeSet<EntityId> = [from].into_iter().collect();
    let mut frontier = VecDeque::from([(from, 0_u8)]);
    while let Some((current, depth)) = frontier.pop_front() {
        if depth == horizon {
            continue;
        }
        for edge in graph.edges() {
            for (near, far) in [(edge.from, edge.to), (edge.to, edge.from)] {
                if near == current && seen.insert(far) {
                    frontier.push_back((far, depth + 1));
                }
            }
        }
    }
    seen
}

impl Atlas {
    /// Which growth band this layout was computed in.
    #[must_use]
    pub const fn growth_band(&self) -> usize {
        self.band
    }

    /// How many placement steps the layout took.
    ///
    /// One per cluster anchor, one per node and one per landmark: linear in the
    /// graph, with no relaxation pass and no iteration count. It is exposed so
    /// that `five_thousand_node_fixture_meets_the_budget` measures work rather
    /// than wall-clock, which on a shared machine measures the machine.
    #[must_use]
    pub const fn work_units(&self) -> usize {
        self.work_units
    }

    /// Where a cluster's anchor sits.
    #[must_use]
    pub fn anchor(&self, cluster: ClusterId) -> Option<Coordinate> {
        self.anchors.get(&cluster).copied()
    }

    /// Where a node sits.
    #[must_use]
    pub fn placement(&self, node: EntityId) -> Option<Coordinate> {
        self.placements.get(&node).copied()
    }

    /// Every placement, in identity order.
    pub fn placements(&self) -> impl Iterator<Item = Placement> + '_ {
        self.placements.iter().map(|(node, at)| Placement {
            node: *node,
            at: *at,
        })
    }

    /// The landmarks, in cluster order.
    #[must_use]
    pub fn landmarks(&self) -> &[Landmark] {
        &self.landmarks
    }

    /// The furthest any landmark this atlas and `other` share has moved.
    ///
    /// A landmark present in only one of the two is **not** silently skipped:
    /// it is reported as [`LandmarkDrift::vanished`], because a landmark that
    /// disappeared is a worse spatial-memory failure than one that moved and a
    /// comparison over the intersection alone would call it zero.
    #[must_use]
    pub fn landmark_drift(&self, other: &Self) -> LandmarkDrift {
        let mine: BTreeMap<ClusterId, Landmark> = self
            .landmarks
            .iter()
            .map(|landmark| (landmark.cluster, *landmark))
            .collect();
        let theirs: BTreeMap<ClusterId, Landmark> = other
            .landmarks
            .iter()
            .map(|landmark| (landmark.cluster, *landmark))
            .collect();
        let mut furthest = 0_i64;
        let mut vanished = BTreeSet::new();
        for (cluster, landmark) in &mine {
            match theirs.get(cluster) {
                Some(counterpart) => {
                    furthest = furthest.max(landmark.at.displacement(counterpart.at));
                }
                None => {
                    vanished.insert(*cluster);
                }
            }
        }
        for cluster in theirs.keys() {
            if !mine.contains_key(cluster) {
                vanished.insert(*cluster);
            }
        }
        LandmarkDrift { furthest, vanished }
    }

    /// What one zoom level shows over `graph`, with `goal` selected.
    ///
    /// The node **set** differs level by level and so does the **type** set,
    /// which is the difference between a semantic level and a scale factor. The
    /// two fine levels also narrow by distance from the goal, because the
    /// section 26.1 table's `감추는 것` column hides `먼 주변 node` at `Z2` and
    /// the `전역 graph` at `Z3`.
    ///
    /// Coordinates are **not** a parameter and are not changed: a node present
    /// at two levels sits at the same place in both, which is the half of
    /// `zoom_changes_semantic_level_not_only_scale` a transform would fail.
    ///
    /// # Errors
    ///
    /// [`CsMapError::GoalIsNotANode`] when the selected goal is not in the graph.
    pub fn level(
        &self,
        graph: &MapGraph,
        zoom: SemanticZoom,
        goal: EntityId,
    ) -> Result<LevelView, CsMapError> {
        if graph.node(goal).is_none() {
            return Err(CsMapError::GoalIsNotANode { node: goal });
        }
        let types = zoom.shown_types();
        let within = zoom
            .horizon()
            .map(|horizon| reachable(graph, goal, horizon));
        let nodes = graph
            .nodes()
            .filter(|node| types.contains(&node.node_type()))
            .filter(|node| within.as_ref().is_none_or(|near| near.contains(&node.id())))
            .map(|node| node.id())
            .collect();
        Ok(LevelView { zoom, types, nodes })
    }

    /// Section 25.3's first screen.
    ///
    /// # Errors
    ///
    /// * [`CsMapError::ClusterCountOutOfRange`] — the graph declares fewer than
    ///   [`MIN_INITIAL_CLUSTERS`] or more than [`MAX_INITIAL_CLUSTERS`] field
    ///   clusters, so the first screen cannot be what section 25.3 describes.
    /// * [`CsMapError::GoalIsNotANode`] — the selected goal is not in the graph.
    pub fn initial_view(
        &self,
        graph: &MapGraph,
        goal: EntityId,
    ) -> Result<InitialView, CsMapError> {
        let clusters = graph.clusters().to_vec();
        if clusters.len() < MIN_INITIAL_CLUSTERS || clusters.len() > MAX_INITIAL_CLUSTERS {
            return Err(CsMapError::ClusterCountOutOfRange {
                count: clusters.len(),
            });
        }
        if graph.node(goal).is_none() {
            return Err(CsMapError::GoalIsNotANode { node: goal });
        }
        let mut goal_neighbourhood: BTreeSet<EntityId> = [goal].into_iter().collect();
        for edge in graph.edges() {
            if edge.from == goal {
                goal_neighbourhood.insert(edge.to);
            }
            if edge.to == goal {
                goal_neighbourhood.insert(edge.from);
            }
        }
        Ok(InitialView {
            clusters,
            goal,
            goal_neighbourhood,
        })
    }
}

/// How far the landmarks moved between two layouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandmarkDrift {
    furthest: i64,
    vanished: BTreeSet<ClusterId>,
}

impl LandmarkDrift {
    /// The furthest a shared landmark moved, in thousandths of a layout unit.
    #[must_use]
    pub const fn furthest(&self) -> i64 {
        self.furthest
    }

    /// Clusters whose landmark is present in only one of the two layouts.
    #[must_use]
    pub const fn vanished(&self) -> &BTreeSet<ClusterId> {
        &self.vanished
    }

    /// Whether every landmark stayed put to within [`LAYOUT_TOLERANCE_MILLI`]
    /// and none vanished.
    ///
    /// # Errors
    ///
    /// [`CsMapError::LandmarkMoved`] or [`CsMapError::LandmarkVanished`].
    pub fn within_tolerance(&self) -> Result<(), CsMapError> {
        if let Some(cluster) = self.vanished.iter().next() {
            return Err(CsMapError::LandmarkVanished { cluster: *cluster });
        }
        if self.furthest > LAYOUT_TOLERANCE_MILLI {
            return Err(CsMapError::LandmarkMoved {
                moved: self.furthest,
                tolerance: LAYOUT_TOLERANCE_MILLI,
            });
        }
        Ok(())
    }
}
