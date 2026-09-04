//! A layout is a function of the graph it was given.
//!
//! `MapGraph` has no `insert`, no `remove` and no `&mut self` method, and
//! `edges` hands out a shared reference. A node added after the layout ran would
//! be a node with no placement and a landmark comparison that silently skipped
//! it.

use academic_cs_map::{MapEdge, MapGraph};
use academic_domain::{EntityId, predicates::PredicateName};

fn sneak(graph: &MapGraph, from: EntityId, to: EntityId) {
    graph.edges().insert(MapEdge {
        from,
        to,
        predicate: PredicateName::Requires,
    });
}

fn main() {
    let _ = sneak;
}
