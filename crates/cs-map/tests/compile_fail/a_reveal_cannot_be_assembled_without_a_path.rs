//! Section 26.4's `cluster → path → node`, held by privacy.
//!
//! `SearchReveal`'s three fields are private and `reveal` is the only producer.
//! A caller cannot assemble a reveal that names a node with no route to it,
//! which is what `순간이동` would be.

use academic_cs_map::{ClusterId, SearchReveal};
use academic_domain::EntityId;

fn teleport(cluster: ClusterId, node: EntityId) -> SearchReveal {
    SearchReveal {
        cluster,
        path: Vec::new(),
        node,
    }
}

fn main() {
    let _ = teleport;
}
