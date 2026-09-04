//! Section 25.3's `YOU는 좌표 node가 아니라 ... 기준점이다`, as a type error.
//!
//! A `YouAnchor` has no identity, so it cannot be handed to `MapNode::declare`
//! and cannot reach `MapGraph::declare`. The absence is checked exhaustively by
//! `you_is_not_an_ontology_node`; this case is the half that cannot be written
//! at all.

use academic_cs_map::{ClusterId, MapNode, YouAnchor};
use academic_domain::predicates::NodeType;

fn place(anchor: YouAnchor, cluster: ClusterId) -> MapNode {
    MapNode::declare(anchor, NodeType::Concept, "YOU", cluster)
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn main() {
    let _ = place;
}
