//! Section 26.4's last sentence: a search result is guided, not teleported.
//!
//! > `graph search result는 node를 화면 밖에서 순간이동시키지 않고
//! > cluster → path → node 순으로 안내한다.`
//!
//! # The three stages are the return type, not a sequence of calls
//!
//! [`SearchReveal`] holds a cluster, a path and a node, all three by value, and
//! there is no constructor that takes fewer. [`reveal`] is the only producer,
//! and it refuses when there is no edge path from where the viewer is standing
//! to the target: a match with no route is [`CsMapError::NoPathToTarget`] rather
//! than a node that appears from nowhere.
//!
//! No function in this crate returns a bare target identity from a query.
//! `search_reveals_in_three_stages` compares the whole set of public signatures
//! that take a query for one that does, in both directions, which is what would
//! catch a convenience wrapper added later.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use academic_domain::EntityId;
use serde::Serialize;

use crate::{
    CsMapError,
    graph::{ClusterId, MapGraph},
};

/// One stage of the reveal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "stage", content = "value", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RevealStage {
    /// First: which cluster the match is in.
    Cluster(ClusterId),
    /// Second: the route from where the viewer is standing to it.
    Path(Vec<EntityId>),
    /// Third, and only third: the node.
    Node(EntityId),
}

/// The number of stages section 26.4 names.
pub const REVEAL_STAGES: usize = 3;

/// A guided reveal of one search match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchReveal {
    cluster: ClusterId,
    path: Vec<EntityId>,
    node: EntityId,
}

impl SearchReveal {
    /// The three stages, in section 26.4's order.
    ///
    /// A fixed-size array, so a reveal that dropped a stage would not compile
    /// rather than returning a shorter list nobody counted.
    #[must_use]
    pub fn stages(&self) -> [RevealStage; REVEAL_STAGES] {
        [
            RevealStage::Cluster(self.cluster),
            RevealStage::Path(self.path.clone()),
            RevealStage::Node(self.node),
        ]
    }

    /// The cluster the match sits in.
    #[must_use]
    pub const fn cluster(&self) -> ClusterId {
        self.cluster
    }

    /// The route, beginning where the viewer stood and ending at the match.
    #[must_use]
    pub fn path(&self) -> &[EntityId] {
        &self.path
    }

    /// The match.
    #[must_use]
    pub const fn node(&self) -> EntityId {
        self.node
    }
}

/// Finds one match for `query` and reveals it from `standing_at`.
///
/// Matching is a case-insensitive substring of a node's label. Exactly one match
/// is required: two matches are an ambiguity the viewer has to resolve, and
/// picking one would be this surface guessing.
///
/// # Errors
///
/// * [`CsMapError::EmptyQuery`] — a blank query, which would match every label.
/// * [`CsMapError::NodeNotOnTheMap`] — the viewer is standing on a node the
///   graph does not hold.
/// * [`CsMapError::NoMatchForQuery`] / [`CsMapError::AmbiguousQuery`].
/// * [`CsMapError::NoPathToTarget`] — the match exists and cannot be walked to.
pub fn reveal(
    graph: &MapGraph,
    standing_at: EntityId,
    query: &str,
) -> Result<SearchReveal, CsMapError> {
    if query.trim().is_empty() {
        return Err(CsMapError::EmptyQuery);
    }
    if graph.node(standing_at).is_none() {
        return Err(CsMapError::NodeNotOnTheMap { node: standing_at });
    }
    let needle = query.to_lowercase();
    let matches: Vec<EntityId> = graph
        .nodes()
        .filter(|node| node.label().to_lowercase().contains(&needle))
        .map(|node| node.id())
        .collect();
    let target = match matches.as_slice() {
        [] => {
            return Err(CsMapError::NoMatchForQuery {
                query: query.to_owned(),
            });
        }
        [only] => *only,
        many => {
            return Err(CsMapError::AmbiguousQuery {
                query: query.to_owned(),
                matches: many.len(),
            });
        }
    };
    let cluster = graph
        .node(target)
        .ok_or(CsMapError::NodeNotOnTheMap { node: target })?
        .cluster();
    let path = shortest_path(graph, standing_at, target)
        .ok_or(CsMapError::NoPathToTarget { node: target })?;
    Ok(SearchReveal {
        cluster,
        path,
        node: target,
    })
}

/// The fewest-hop undirected route from `from` to `to`, or `None`.
///
/// Ties are broken by identity order so the route is the same on every run,
/// which is what lets a reveal be compared rather than merely inspected.
fn shortest_path(graph: &MapGraph, from: EntityId, to: EntityId) -> Option<Vec<EntityId>> {
    if from == to {
        return Some(vec![from]);
    }
    let mut came_from: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    let mut seen: BTreeSet<EntityId> = [from].into_iter().collect();
    let mut frontier = VecDeque::from([from]);
    while let Some(current) = frontier.pop_front() {
        let mut neighbours: BTreeSet<EntityId> = BTreeSet::new();
        for edge in graph.edges() {
            if edge.from == current {
                neighbours.insert(edge.to);
            }
            if edge.to == current {
                neighbours.insert(edge.from);
            }
        }
        for next in neighbours {
            if !seen.insert(next) {
                continue;
            }
            came_from.insert(next, current);
            if next == to {
                let mut route = vec![to];
                let mut step = to;
                while let Some(previous) = came_from.get(&step) {
                    route.push(*previous);
                    step = *previous;
                }
                route.reverse();
                return Some(route);
            }
            frontier.push_back(next);
        }
    }
    None
}
