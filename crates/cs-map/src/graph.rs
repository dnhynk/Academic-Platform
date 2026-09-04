//! What the atlas is drawn *of*.
//!
//! A map graph is a set of field clusters, a set of nodes each belonging to one
//! cluster, and a set of edges. Every vocabulary in it is
//! `academic_domain`'s: [`NodeType`] is section 7.1's thirty-seven-arm
//! hierarchy and [`PredicateName`] is section 7.2's twenty edges. This module
//! declares neither again.
//!
//! # A cluster is a `FIELD` node, not a second kind of thing
//!
//! Section 7.1 separates `Field` from `Concept` in so many words: `Database
//! Systems` can be a cluster and `Serializability` is a concept. So a
//! [`ClusterId`] wraps the identity of a node whose [`NodeType`] is
//! [`NodeType::Field`], and [`MapGraph::declare`] refuses a cluster whose node
//! is anything else. There is no free-floating "group" here that the ontology
//! does not know about.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use academic_domain::{
    EntityId,
    predicates::{NodeType, PredicateName},
};
use serde::Serialize;

use crate::CsMapError;

/// The identity of a field cluster, which is the identity of a `FIELD` node.
///
/// A newtype rather than a bare [`EntityId`] so that a cluster and a member
/// cannot be passed to each other's parameter, and so that
/// [`MapGraph::declare`] is the only place a cluster comes into existence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ClusterId(EntityId);

impl ClusterId {
    /// Names the cluster that a `FIELD` node is.
    ///
    /// A cluster's identity **is** its field node's identity, so this is a
    /// rename rather than a mint. It is the only constructor, and it is safe to
    /// be public because it decides nothing on its own:
    /// [`MapGraph::declare`] refuses a node whose cluster is not among the
    /// `FIELD`s actually declared, and [`MapNode::declare`] refuses a non-`FIELD`
    /// naming itself. A caller that renames a concept as a cluster therefore
    /// gets a refusal at declaration time rather than a graph with a cluster
    /// nothing is in.
    #[must_use]
    pub const fn of_field(field: EntityId) -> Self {
        Self(field)
    }

    /// Returns the underlying entity identity.
    #[must_use]
    pub const fn entity(self) -> EntityId {
        self.0
    }
}

impl fmt::Display for ClusterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One node of the map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapNode {
    id: EntityId,
    node_type: NodeType,
    label: String,
    cluster: ClusterId,
}

impl MapNode {
    /// Declares a node of `node_type` inside `cluster`.
    ///
    /// # Errors
    ///
    /// Returns [`CsMapError::EmptyLabel`] when the label is blank, and
    /// [`CsMapError::ClusterIsNotAField`] when a node claims to be a cluster
    /// without being a `FIELD`.
    pub fn declare(
        id: EntityId,
        node_type: NodeType,
        label: impl Into<String>,
        cluster: ClusterId,
    ) -> Result<Self, CsMapError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(CsMapError::EmptyLabel { node: id });
        }
        if cluster.entity() == id && node_type != NodeType::Field {
            return Err(CsMapError::ClusterIsNotAField {
                node: id,
                node_type: node_type.as_str(),
            });
        }
        Ok(Self {
            id,
            node_type,
            label,
            cluster,
        })
    }

    /// Returns the node's identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Returns section 7.1's type of this node.
    #[must_use]
    pub const fn node_type(&self) -> NodeType {
        self.node_type
    }

    /// Returns the label a viewer reads.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the cluster this node is laid out inside.
    #[must_use]
    pub const fn cluster(&self) -> ClusterId {
        self.cluster
    }
}

/// One directed edge, carrying section 7.2's predicate and its claim status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct MapEdge {
    /// The subject end.
    pub from: EntityId,
    /// The object end.
    pub to: EntityId,
    /// Which of section 7.2's twenty edges this is.
    #[serde(serialize_with = "serialize_predicate")]
    pub predicate: PredicateName,
}

fn serialize_predicate<S>(value: &PredicateName, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(value.as_str())
}

/// The graph an atlas is laid out from.
///
/// Built once by [`MapGraph::declare`] and never edited: there is no `insert`,
/// no `remove` and no `&mut self` method, so a view cannot quietly grow a node
/// the layout did not see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapGraph {
    clusters: Vec<ClusterId>,
    nodes: BTreeMap<EntityId, MapNode>,
    edges: BTreeSet<MapEdge>,
}

impl MapGraph {
    /// Declares the whole graph at once.
    ///
    /// # Errors
    ///
    /// * [`CsMapError::ClusterIsNotAField`] — a declared cluster's node is not
    ///   a section 7.1 `FIELD`, or is missing from the node set entirely.
    /// * [`CsMapError::NodeOutsideEveryCluster`] — a node names a cluster that
    ///   was not declared.
    /// * [`CsMapError::EdgeEndpointIsNotANode`] — an edge names an identity the
    ///   node set does not hold.
    /// * [`CsMapError::DuplicateNode`] — two nodes share one identity.
    pub fn declare(nodes: Vec<MapNode>, edges: Vec<MapEdge>) -> Result<Self, CsMapError> {
        let mut by_id: BTreeMap<EntityId, MapNode> = BTreeMap::new();
        for node in nodes {
            if by_id.contains_key(&node.id()) {
                return Err(CsMapError::DuplicateNode { node: node.id() });
            }
            by_id.insert(node.id(), node);
        }

        let mut clusters: Vec<ClusterId> = Vec::new();
        for node in by_id.values() {
            if node.node_type() == NodeType::Field {
                clusters.push(ClusterId(node.id()));
            }
        }
        clusters.sort_unstable();
        let declared: BTreeSet<ClusterId> = clusters.iter().copied().collect();

        for node in by_id.values() {
            if !declared.contains(&node.cluster()) {
                return Err(CsMapError::NodeOutsideEveryCluster {
                    node: node.id(),
                    cluster: node.cluster(),
                });
            }
        }

        let mut kept: BTreeSet<MapEdge> = BTreeSet::new();
        for edge in edges {
            for end in [edge.from, edge.to] {
                if !by_id.contains_key(&end) {
                    return Err(CsMapError::EdgeEndpointIsNotANode { node: end });
                }
            }
            kept.insert(edge);
        }

        Ok(Self {
            clusters,
            nodes: by_id,
            edges: kept,
        })
    }

    /// Every field cluster, in identity order.
    #[must_use]
    pub fn clusters(&self) -> &[ClusterId] {
        &self.clusters
    }

    /// Every node, in identity order.
    pub fn nodes(&self) -> impl Iterator<Item = &MapNode> {
        self.nodes.values()
    }

    /// One node, by identity.
    #[must_use]
    pub fn node(&self, id: EntityId) -> Option<&MapNode> {
        self.nodes.get(&id)
    }

    /// Every edge, in `(from, to, predicate)` order.
    #[must_use]
    pub const fn edges(&self) -> &BTreeSet<MapEdge> {
        &self.edges
    }

    /// The identities belonging to `cluster`, in identity order.
    #[must_use]
    pub fn members(&self, cluster: ClusterId) -> BTreeSet<EntityId> {
        self.nodes
            .values()
            .filter(|node| node.cluster() == cluster)
            .map(MapNode::id)
            .collect()
    }

    /// How many nodes the graph holds.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
