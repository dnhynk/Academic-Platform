//! The dependency graph section 29.2 invalidates through.
//!
//! *When the source text changes, the requirements, scenarios and course
//! mappings it affects are invalidated through a dependency graph.* The graph
//! is directed from a dependent to what it depends on, and
//! [`DependencyGraph::invalidate`] walks it in reverse from the impacted rules.
//!
//! # Exactly the dependents, and no others
//!
//! The walk is transitive, because a scenario that cites a requirement that
//! cites a rule is moved by that rule. It is also bounded: a node reached by no
//! path from an impacted rule is not in the result.
//! `source_change_invalidates_exact_dependents` compares the whole set both
//! ways, so an over-invalidation fails as an extra entry and an
//! under-invalidation as a missing one.
//!
//! A cycle terminates. Nodes already seen are not walked again, which is what
//! makes the reverse walk finite over a graph this crate does not build.

use academic_domain::engines::RuleId;

use crate::identifier::DependentId;

/// What kind of thing depends on a rule.
///
/// Section 29.2's own three: requirements, scenarios, course mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependentKind {
    /// A graduation or programme requirement.
    Requirement,
    /// A planning scenario.
    Scenario,
    /// A course mapping: substitution, equivalence, transfer.
    CourseMapping,
}

impl DependentKind {
    /// Exhaustive listing.
    pub const ALL: [Self; 3] = [Self::Requirement, Self::Scenario, Self::CourseMapping];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requirement => "REQUIREMENT",
            Self::Scenario => "SCENARIO",
            Self::CourseMapping => "COURSE_MAPPING",
        }
    }
}

/// One node that depends on something.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DependentNode {
    kind: DependentKind,
    id: DependentId,
}

impl DependentNode {
    /// A node.
    #[must_use]
    pub const fn new(kind: DependentKind, id: DependentId) -> Self {
        Self { kind, id }
    }

    /// Which kind.
    #[must_use]
    pub const fn kind(&self) -> DependentKind {
        self.kind
    }

    /// Which node.
    #[must_use]
    pub const fn id(&self) -> &DependentId {
        &self.id
    }
}

/// What a node depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dependency {
    /// A rule in an official document.
    Rule(RuleId),
    /// Another dependent node.
    Node(DependentNode),
}

/// Which nodes depend on which rules and on each other.
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    edges: Vec<(DependentNode, Dependency)>,
}

impl DependencyGraph {
    /// An empty graph.
    #[must_use]
    pub const fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Records that `dependent` depends on `dependency`.
    pub fn record(&mut self, dependent: DependentNode, dependency: Dependency) {
        let edge = (dependent, dependency);
        if !self.edges.contains(&edge) {
            self.edges.push(edge);
        }
    }

    /// Every edge, in the order they were recorded.
    #[must_use]
    pub fn edges(&self) -> &[(DependentNode, Dependency)] {
        &self.edges
    }

    /// Exactly the nodes a change to `impacted` moves.
    ///
    /// The result is sorted and has no repeats, so a caller comparing it
    /// against an expectation compares a set.
    #[must_use]
    pub fn invalidate(&self, impacted: &[RuleId]) -> Invalidation {
        let mut reached: Vec<DependentNode> = Vec::new();
        let mut pending: Vec<DependentNode> = Vec::new();

        for (dependent, dependency) in &self.edges {
            if let Dependency::Rule(rule) = dependency
                && impacted.contains(rule)
                && !reached.contains(dependent)
            {
                reached.push(dependent.clone());
                pending.push(dependent.clone());
            }
        }

        while let Some(current) = pending.pop() {
            for (dependent, dependency) in &self.edges {
                if let Dependency::Node(target) = dependency
                    && target == &current
                    && !reached.contains(dependent)
                {
                    reached.push(dependent.clone());
                    pending.push(dependent.clone());
                }
            }
        }

        reached.sort();
        reached.dedup();
        Invalidation { nodes: reached }
    }
}

/// The nodes one source change invalidated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalidation {
    nodes: Vec<DependentNode>,
}

impl Invalidation {
    /// The invalidated nodes, sorted and without repeats.
    #[must_use]
    pub fn nodes(&self) -> &[DependentNode] {
        &self.nodes
    }

    /// The invalidated nodes of one kind.
    #[must_use]
    pub fn of_kind(&self, kind: DependentKind) -> Vec<&DependentNode> {
        self.nodes
            .iter()
            .filter(|node| node.kind() == kind)
            .collect()
    }

    /// Whether nothing was invalidated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
