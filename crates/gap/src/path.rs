//! Section 15.1's `blockingPath`, and section 15.2 step 4's `그 조상 영향도`.
//!
//! ## An ancestor's impact carries no evidence of its own
//!
//! Step 4 asks for the first strong deficit **and its effect on the ancestors
//! above it**. That is a statement about a path, not a second state reading: the
//! ancestors are affected because the root is short, not because anything was
//! observed about them. So [`AncestorImpact`] holds the ancestor, the edge that
//! reaches down toward the root and the distance, and it holds **no evidence
//! identity and no state**. The evidence stays on the root candidate that
//! actually has it.
//!
//! That is deliberate and it is the same rule the rest of this crate keeps: one
//! concept's evidence never becomes another concept's reading. An
//! `AncestorImpact` that carried the root's evidence would be exactly that, with
//! the direction reversed.

use academic_domain::{
    EntityId,
    predicates::{PredicateName, PrerequisiteStrength},
};
use serde::{Deserialize, Serialize};

use crate::graph::PrerequisiteEdge;

/// One hop of a blocking path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathStep {
    advanced: EntityId,
    prerequisite: EntityId,
    #[serde(with = "crate::graph::predicate_serde")]
    predicate: PredicateName,
    #[serde(with = "crate::graph::strength_serde")]
    strength: PrerequisiteStrength,
}

impl PathStep {
    /// Records the hop an admitted edge makes.
    #[must_use]
    pub fn of(edge: &PrerequisiteEdge) -> Self {
        Self {
            advanced: edge.advanced(),
            prerequisite: edge.prerequisite(),
            predicate: edge.predicate(),
            strength: edge.strength(),
        }
    }

    /// The upper end.
    #[must_use]
    pub const fn advanced(&self) -> EntityId {
        self.advanced
    }

    /// The lower end.
    #[must_use]
    pub const fn prerequisite(&self) -> EntityId {
        self.prerequisite
    }

    /// Which section 7.2 edge.
    #[must_use]
    pub const fn predicate(&self) -> PredicateName {
        self.predicate
    }

    /// The asserted strength.
    #[must_use]
    pub const fn strength(&self) -> PrerequisiteStrength {
        self.strength
    }
}

/// Section 15.1's `blockingPath`: the surface concept, then every hop down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockingPath {
    surface: EntityId,
    steps: Vec<PathStep>,
}

impl BlockingPath {
    /// A path that has not descended yet.
    #[must_use]
    pub const fn from_surface(surface: EntityId) -> Self {
        Self {
            surface,
            steps: Vec::new(),
        }
    }

    /// Extends the path by one admitted edge.
    #[must_use]
    pub fn extended(&self, edge: &PrerequisiteEdge) -> Self {
        let mut steps = self.steps.clone();
        steps.push(PathStep::of(edge));
        Self {
            surface: self.surface,
            steps,
        }
    }

    /// The surface concept the goal named.
    #[must_use]
    pub const fn surface(&self) -> EntityId {
        self.surface
    }

    /// The hops, surface-first.
    #[must_use]
    pub fn steps(&self) -> &[PathStep] {
        &self.steps
    }

    /// How many hops below the surface the path ends.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.steps.len()
    }

    /// The node the path ends at.
    #[must_use]
    pub fn tip(&self) -> EntityId {
        self.steps
            .last()
            .map_or(self.surface, PathStep::prerequisite)
    }

    /// Every concept on the path, surface first.
    #[must_use]
    pub fn concepts(&self) -> Vec<EntityId> {
        let mut found = vec![self.surface];
        found.extend(self.steps.iter().map(PathStep::prerequisite));
        found
    }

    /// Whether `concept` lies on the path.
    #[must_use]
    pub fn holds(&self, concept: EntityId) -> bool {
        self.concepts().contains(&concept)
    }
}

/// Section 15.2 step 4's `조상 영향도`: an ancestor the root's deficit reaches.
///
/// Carries no evidence and no state. See the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AncestorImpact {
    ancestor: EntityId,
    hops_above_root: usize,
    #[serde(with = "crate::graph::strength_serde")]
    weakest_link: PrerequisiteStrength,
}

impl AncestorImpact {
    /// Records one affected ancestor.
    #[must_use]
    pub const fn of(
        ancestor: EntityId,
        hops_above_root: usize,
        weakest_link: PrerequisiteStrength,
    ) -> Self {
        Self {
            ancestor,
            hops_above_root,
            weakest_link,
        }
    }

    /// Which ancestor.
    #[must_use]
    pub const fn ancestor(&self) -> EntityId {
        self.ancestor
    }

    /// Its distance above the root along the blocking path.
    #[must_use]
    pub const fn hops_above_root(&self) -> usize {
        self.hops_above_root
    }

    /// The weakest edge between the ancestor and the root.
    ///
    /// A chain is no stronger than its weakest hop, so an ancestor reached only
    /// through a `STRONG` edge is affected `STRONG`ly even when the hop nearest
    /// the root is `HARD`.
    #[must_use]
    pub const fn weakest_link(&self) -> PrerequisiteStrength {
        self.weakest_link
    }
}
