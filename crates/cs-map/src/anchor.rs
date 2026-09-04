//! `YOU` is where the user is standing, and standing somewhere is not being a
//! place.
//!
//! Section 25.3: `가운데의 YOU는 좌표 node가 아니라 사용자 state overlay의
//! 기준점이다.`
//!
//! # How that is held
//!
//! A [`YouAnchor`] has **no identity**. It carries the identities it is
//! *reckoned from* and a position derived from theirs, and there is no function
//! anywhere in this crate that turns one into a [`crate::graph::MapNode`], no
//! `From` in either direction, and no way to hand one to
//! [`crate::graph::MapGraph::declare`] — that constructor takes `MapNode`
//! values, and a `MapNode` needs an [`EntityId`], which this type does not have
//! and cannot be given.
//!
//! `you_is_not_an_ontology_node` does not test that; it tests the *absence*
//! exhaustively. It walks every node of every surface this crate can produce —
//! the graph, the initial view, all four zoom levels, all five focus subgraphs,
//! every lens composition's legend subject, every scrubber projection and every
//! search reveal — and requires that none of them holds a node whose identity is
//! the anchor's reference set's *own* aggregate or whose label is
//! [`YOU_REFERENCE_LABEL`]. An absence claim is checked by looking everywhere,
//! not by naming the one place somebody might have put it.

use std::collections::BTreeSet;

use academic_domain::EntityId;
use serde::Serialize;

use crate::{
    CsMapError,
    atlas::{Atlas, Coordinate},
};

/// The words the shell draws at the anchor.
///
/// Held here so that the exhaustive sweep has one string to look for, and so
/// that a node created with this label fails rather than reading as the anchor.
pub const YOU_REFERENCE_LABEL: &str = "YOU";

/// The reference point section 25.3 puts the user's state overlay on.
///
/// It has no [`EntityId`], is not in any node set, and has no relations. What it
/// has is a set of nodes the user's current state is reckoned over and the
/// position that set implies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YouAnchor {
    references: BTreeSet<EntityId>,
    at: Coordinate,
}

impl YouAnchor {
    /// Places the anchor over the nodes the user's state is reckoned from.
    ///
    /// The position is the midpoint of the referenced placements' bounding box,
    /// so it moves when the user's state moves and never otherwise. It is not a
    /// node's position: no node need sit there and generally none does.
    ///
    /// # Errors
    ///
    /// * [`CsMapError::AnchorHasNoReference`] — an empty reference set. An
    ///   anchor over nothing would be a point on the map with no meaning, which
    ///   is exactly the coordinate node section 25.3 refuses.
    /// * [`CsMapError::NodeNotOnTheMap`] — a referenced node has no placement.
    pub fn over(references: BTreeSet<EntityId>, atlas: &Atlas) -> Result<Self, CsMapError> {
        if references.is_empty() {
            return Err(CsMapError::AnchorHasNoReference);
        }
        let mut min_x = i64::MAX;
        let mut max_x = i64::MIN;
        let mut min_y = i64::MAX;
        let mut max_y = i64::MIN;
        for node in &references {
            let at = atlas
                .placement(*node)
                .ok_or(CsMapError::NodeNotOnTheMap { node: *node })?;
            min_x = min_x.min(i64::from(at.x_milli));
            max_x = max_x.max(i64::from(at.x_milli));
            min_y = min_y.min(i64::from(at.y_milli));
            max_y = max_y.max(i64::from(at.y_milli));
        }
        Ok(Self {
            references,
            at: Coordinate {
                x_milli: ((min_x + max_x) / 2) as i32,
                y_milli: ((min_y + max_y) / 2) as i32,
            },
        })
    }

    /// The nodes the anchor is reckoned from.
    #[must_use]
    pub const fn references(&self) -> &BTreeSet<EntityId> {
        &self.references
    }

    /// Where the anchor is drawn.
    #[must_use]
    pub const fn at(&self) -> Coordinate {
        self.at
    }

    /// The label the shell draws beside it.
    ///
    /// A method rather than a field so that no serialized anchor carries a name
    /// that could be read back as a node's label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        YOU_REFERENCE_LABEL
    }
}
