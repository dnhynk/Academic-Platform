//! Section 16.1's typed prerequisite hypergraph, and why a shortest-path
//! algorithm answers the wrong question on it.
//!
//! ## What the design document draws
//!
//! ```text
//! Goal: reliable real-time collaboration
//!   REQUIRES ALL [failure model, shared-state semantics]
//!   REQUIRES ONE OF
//!     ├─ [OT fundamentals, central server ordering]
//!     └─ [CRDT fundamentals, merge semantics]
//! ```
//!
//! Two shapes and no third: a conjunction whose every member is needed, and a
//! disjunction of **groups**, where choosing a branch takes on all of that
//! branch's members. [`Hyperedge`] is those two and nothing else.
//!
//! ## `and_or_hypergraph_is_satisfied_not_shortest`
//!
//! A shortest-path algorithm minimises a length over a *sequence of arcs*. Two
//! things go wrong when it is pointed at this structure.
//!
//! *A conjunction is not a choice.* The cheapest arc out of the goal reaches
//! one member of `REQUIRES ALL`, and a path algorithm stops there because it
//! arrived. Satisfaction does not: every member of a conjunction is required,
//! so the answer is a **set** and not a walk. [`satisfying_sets`] returns sets.
//!
//! *A disjunction is not the cheapest member.* Choosing the `ONE OF` branch
//! with the single cheapest node can be strictly worse than the other branch,
//! because a branch is taken whole. `Path B` in section 16.6 is the design
//! document's own case: it is longer and it is kept.
//!
//! So the two answers differ observably, and they are compared: [`shortest_by_node_count`]
//! is the naive answer, implemented here **so the acceptance suite can show it
//! is wrong** rather than describing it. Nothing in [`crate::engine`] calls it.
//!
//! ## Members are `P2-C4`'s edges
//!
//! A hyperedge is not a second graph beside `P2-N5`'s. Each member arrives as an
//! [`academic_gap::PrerequisiteEdge`], which passed that crate's
//! `PrerequisiteEdge::admit` and therefore `P2-C4`'s `prerequisite_descriptor`.
//! Eighteen of section 7.2's twenty predicates have no value of that type at
//! all, so `RELATED_TO` cannot enter a hyperedge and this crate holds no
//! allowlist.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::EntityId;
use academic_gap::PrerequisiteEdge;
use serde::{Deserialize, Serialize};

use crate::CriticalPathError;

/// How confident the graph is that one hyperedge member is really required.
///
/// Section 16.3's eighth constraint counts the uncertain ones, and section
/// 16.5 discloses them, so this is a fact about the edge rather than a display
/// choice. Two values and no middle: an edge is either one the user or an
/// official source settled, or one nobody has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeStanding {
    /// The relation is settled: an official prerequisite, or one the user
    /// confirmed.
    Settled,
    /// The relation is asserted and nobody has confirmed it.
    Uncertain,
}

/// One member of a hyperedge: an admitted section 7.2 edge and its standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeMember {
    edge: PrerequisiteEdge,
    standing: EdgeStanding,
}

impl EdgeMember {
    /// Records one member.
    #[must_use]
    pub const fn of(edge: PrerequisiteEdge, standing: EdgeStanding) -> Self {
        Self { edge, standing }
    }

    /// The admitted `P2-N5` edge.
    #[must_use]
    pub const fn edge(&self) -> &PrerequisiteEdge {
        &self.edge
    }

    /// The concept this member requires.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.edge.prerequisite()
    }

    /// The concept the requirement is stated about.
    #[must_use]
    pub const fn dependent(&self) -> EntityId {
        self.edge.advanced()
    }

    /// Whether the relation is settled.
    #[must_use]
    pub const fn standing(&self) -> EdgeStanding {
        self.standing
    }
}

/// Section 16.1's two hyperedge shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hyperedge {
    /// `REQUIRES ALL [..]`. Every member is needed.
    RequiresAll {
        /// The concept the requirement is about.
        target: EntityId,
        /// Every member, in declaration order. Never empty.
        members: Vec<EdgeMember>,
    },
    /// `REQUIRES ONE OF` with a branch per group. One whole group is needed.
    RequiresOneOf {
        /// The concept the requirement is about.
        target: EntityId,
        /// The branches, in declaration order. At least two, each non-empty.
        branches: Vec<Vec<EdgeMember>>,
    },
}

impl Hyperedge {
    /// Declares a conjunction.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::EmptyHyperedge`] for no members, and
    /// [`CriticalPathError::HyperedgeMemberLeavesTarget`] for a member whose
    /// admitted edge is stated about a different concept.
    pub fn requires_all(
        target: EntityId,
        members: Vec<EdgeMember>,
    ) -> Result<Self, CriticalPathError> {
        if members.is_empty() {
            return Err(CriticalPathError::EmptyHyperedge);
        }
        require_members_are_about(target, &members)?;
        Ok(Self::RequiresAll { target, members })
    }

    /// Declares a disjunction of branches.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::DisjunctionHasOneBranch`] for fewer than two
    /// branches -- a `ONE OF` with a single branch is a conjunction wearing the
    /// other shape's name, and section 16.4's whole point is that the user can
    /// see there is a choice; [`CriticalPathError::EmptyHyperedge`] for an
    /// empty branch; and [`CriticalPathError::HyperedgeMemberLeavesTarget`] as
    /// above.
    pub fn requires_one_of(
        target: EntityId,
        branches: Vec<Vec<EdgeMember>>,
    ) -> Result<Self, CriticalPathError> {
        if branches.len() < 2 {
            return Err(CriticalPathError::DisjunctionHasOneBranch);
        }
        for branch in &branches {
            if branch.is_empty() {
                return Err(CriticalPathError::EmptyHyperedge);
            }
            require_members_are_about(target, branch)?;
        }
        Ok(Self::RequiresOneOf { target, branches })
    }

    /// The concept this hyperedge states a requirement about.
    #[must_use]
    pub const fn target(&self) -> EntityId {
        match self {
            Self::RequiresAll { target, .. } | Self::RequiresOneOf { target, .. } => *target,
        }
    }

    /// Every member of every branch, in declaration order.
    #[must_use]
    pub fn members(&self) -> Vec<&EdgeMember> {
        match self {
            Self::RequiresAll { members, .. } => members.iter().collect(),
            Self::RequiresOneOf { branches, .. } => branches.iter().flatten().collect(),
        }
    }

    /// The branches a satisfying set must choose between, as concept groups.
    ///
    /// A conjunction has exactly one branch, which is its whole member list;
    /// that is what makes `REQUIRES ALL` a choice with no alternative rather
    /// than a special case in the solver below.
    #[must_use]
    fn branch_groups(&self) -> Vec<Vec<&EdgeMember>> {
        match self {
            Self::RequiresAll { members, .. } => vec![members.iter().collect()],
            Self::RequiresOneOf { branches, .. } => branches
                .iter()
                .map(|branch| branch.iter().collect())
                .collect(),
        }
    }
}

fn require_members_are_about(
    target: EntityId,
    members: &[EdgeMember],
) -> Result<(), CriticalPathError> {
    for member in members {
        if member.dependent() != target {
            return Err(CriticalPathError::HyperedgeMemberLeavesTarget);
        }
    }
    Ok(())
}

/// Section 16.1's `typed prerequisite hypergraph`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrerequisiteHypergraph {
    edges: Vec<Hyperedge>,
}

impl PrerequisiteHypergraph {
    /// An empty hypergraph.
    #[must_use]
    pub const fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Adds one hyperedge.
    #[must_use]
    pub fn with(mut self, edge: Hyperedge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Every hyperedge, in insertion order.
    #[must_use]
    pub fn edges(&self) -> &[Hyperedge] {
        &self.edges
    }

    /// The hyperedges stated about `target`, in insertion order.
    #[must_use]
    pub fn about(&self, target: EntityId) -> Vec<&Hyperedge> {
        self.edges
            .iter()
            .filter(|edge| edge.target() == target)
            .collect()
    }

    /// Every member edge in the whole hypergraph, in insertion order.
    #[must_use]
    pub fn all_members(&self) -> Vec<&EdgeMember> {
        self.edges.iter().flat_map(Hyperedge::members).collect()
    }
}

/// One set of concepts that satisfies a goal, with the members it was reached
/// through.
///
/// This is the answer shape section 16.1 asks for. It is a **set**, not a walk:
/// the concepts are ordered by identifier so two runs that reached them in a
/// different order are the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SatisfyingSet {
    concepts: Vec<EntityId>,
    members: Vec<EdgeMember>,
}

impl SatisfyingSet {
    /// The concepts the set requires, in identifier order and deduplicated.
    #[must_use]
    pub fn concepts(&self) -> &[EntityId] {
        &self.concepts
    }

    /// The hyperedge members the set was reached through, in identifier order
    /// of the concept each requires.
    #[must_use]
    pub fn members(&self) -> &[EdgeMember] {
        &self.members
    }

    /// How many of the members are [`EdgeStanding::Uncertain`].
    #[must_use]
    pub fn uncertain_member_count(&self) -> usize {
        self.members
            .iter()
            .filter(|member| member.standing() == EdgeStanding::Uncertain)
            .count()
    }

    /// Whether `concept` is in the set.
    #[must_use]
    pub fn holds(&self, concept: EntityId) -> bool {
        self.concepts.contains(&concept)
    }
}

/// Every minimal set of concepts that satisfies `goal_concept` in `graph`.
///
/// The recursion is over the hypergraph's own structure: a concept with no
/// hyperedge about it is a leaf and satisfies itself; a concept with hyperedges
/// is satisfied by choosing one branch of each and satisfying every member of
/// every chosen branch. The results are deduplicated by concept set and
/// returned in a total order, so the answer is a function of the graph and not
/// of the traversal.
///
/// A cycle terminates: a concept already on the current expansion stack
/// contributes nothing further, because it is already required by the set being
/// built.
///
/// # Errors
///
/// [`CriticalPathError::HypergraphIsTooWide`] when the branch product exceeds
/// [`MAX_SATISFYING_SETS`]. That is a refusal and not a truncation: returning
/// some of the sets and calling them the answer is exactly the silent
/// narrowing section 16.5 forbids.
pub fn satisfying_sets(
    graph: &PrerequisiteHypergraph,
    goal_concept: EntityId,
) -> Result<Vec<SatisfyingSet>, CriticalPathError> {
    let mut stack = BTreeSet::new();
    let raw = expand(graph, goal_concept, &mut stack)?;
    let mut byconcepts: BTreeMap<Vec<[u8; 16]>, SatisfyingSet> = BTreeMap::new();
    for (concepts, members) in raw {
        let mut ordered: Vec<EntityId> = concepts.into_iter().collect();
        ordered.sort_by_key(|id| id.as_uuid());
        ordered.dedup();
        let mut kept: Vec<EdgeMember> = Vec::new();
        for member in members {
            if !kept.iter().any(|held: &EdgeMember| {
                held.concept() == member.concept() && held.dependent() == member.dependent()
            }) {
                kept.push(member);
            }
        }
        kept.sort_by_key(|member| (member.concept().as_uuid(), member.dependent().as_uuid()));
        let key: Vec<[u8; 16]> = ordered.iter().map(|id| *id.as_bytes()).collect();
        byconcepts.insert(
            key,
            SatisfyingSet {
                concepts: ordered,
                members: kept,
            },
        );
    }
    Ok(byconcepts.into_values().collect())
}

/// The largest number of satisfying sets [`satisfying_sets`] will assemble.
///
/// A hypergraph whose branch product exceeds it is refused rather than sampled.
pub const MAX_SATISFYING_SETS: usize = 256;

type Partial = (BTreeSet<EntityId>, Vec<EdgeMember>);

fn expand(
    graph: &PrerequisiteHypergraph,
    concept: EntityId,
    stack: &mut BTreeSet<[u8; 16]>,
) -> Result<Vec<Partial>, CriticalPathError> {
    if !stack.insert(*concept.as_bytes()) {
        // Already required by the set being built; requiring it again adds
        // nothing and would not terminate.
        return Ok(vec![(BTreeSet::new(), Vec::new())]);
    }
    let mut combined: Vec<Partial> = vec![(BTreeSet::from([concept]), Vec::new())];
    for hyperedge in graph.about(concept) {
        let mut alternatives: Vec<Partial> = Vec::new();
        for branch in hyperedge.branch_groups() {
            let mut branch_states: Vec<Partial> = vec![(BTreeSet::new(), Vec::new())];
            for member in branch {
                let below = expand(graph, member.concept(), stack)?;
                let mut next: Vec<Partial> = Vec::new();
                for (concepts, members) in &branch_states {
                    for (sub_concepts, sub_members) in &below {
                        let mut merged_concepts = concepts.clone();
                        merged_concepts.extend(sub_concepts.iter().copied());
                        let mut merged_members = members.clone();
                        merged_members.push((*member).clone());
                        merged_members.extend(sub_members.iter().cloned());
                        next.push((merged_concepts, merged_members));
                    }
                }
                branch_states = bounded(next)?;
            }
            alternatives.extend(branch_states);
        }
        let mut next: Vec<Partial> = Vec::new();
        for (concepts, members) in &combined {
            for (branch_concepts, branch_members) in &alternatives {
                let mut merged_concepts = concepts.clone();
                merged_concepts.extend(branch_concepts.iter().copied());
                let mut merged_members = members.clone();
                merged_members.extend(branch_members.iter().cloned());
                next.push((merged_concepts, merged_members));
            }
        }
        combined = bounded(next)?;
    }
    stack.remove(concept.as_bytes());
    Ok(combined)
}

fn bounded(states: Vec<Partial>) -> Result<Vec<Partial>, CriticalPathError> {
    if states.len() > MAX_SATISFYING_SETS {
        return Err(CriticalPathError::HypergraphIsTooWide);
    }
    Ok(states)
}

/// The answer a shortest-path algorithm gives: the reachable concept with the
/// fewest arcs between it and the goal, walking one member at a time.
///
/// **Nothing in [`crate::engine`] calls this.** It exists so that
/// `and_or_hypergraph_is_satisfied_not_shortest` can compare the two answers on
/// the same graph and show they differ, rather than asserting a difference the
/// suite cannot see. Reading a hyperedge as a set of independent arcs is
/// precisely the mistake section 34.5 names -- `shortest node count, AND/OR
/// 무시` -- so the mistake is committed here, once, in a function the product
/// path cannot reach.
#[must_use]
pub fn shortest_by_node_count(
    graph: &PrerequisiteHypergraph,
    goal_concept: EntityId,
) -> Vec<EntityId> {
    let mut seen: BTreeSet<[u8; 16]> = BTreeSet::from([*goal_concept.as_bytes()]);
    let mut frontier = vec![goal_concept];
    let mut walk = vec![goal_concept];
    while let Some(current) = frontier.pop() {
        let mut next: Option<EntityId> = None;
        for hyperedge in graph.about(current) {
            for member in hyperedge.members() {
                if !seen.contains(member.concept().as_bytes()) {
                    next = Some(member.concept());
                    break;
                }
            }
            if next.is_some() {
                break;
            }
        }
        if let Some(step) = next {
            seen.insert(*step.as_bytes());
            walk.push(step);
            frontier.push(step);
        }
    }
    walk
}
