//! Section 16.4's `경로마다 “이 edge가 틀리면 무엇이 바뀌는가”를 보여주고`.
//!
//! ## The answer is recomputed, not described
//!
//! [`sensitivity`] removes one hyperedge member and runs the *same* solver
//! again. It does not estimate what would change; it computes what does. That
//! is the only reading under which `counterfactual_shows_edge_sensitivity` can
//! fail when the solver is wrong, and section 34.5's detection column names
//! `sensitivity analysis` beside `expert/user counter-path` for exactly that
//! reason.
//!
//! ## Removing a member is not editing the graph
//!
//! The preview builds a new hypergraph without the member and leaves the
//! original untouched -- it takes `&PrerequisiteHypergraph` and returns owned
//! values. A user who wants the change to *stick* uses [`crate::edit`], which
//! keeps the base as well. Neither path mutates.
//!
//! Removing the only member of a `REQUIRES ALL` leaves the conjunction with
//! nothing, which is not a hyperedge this crate admits, so the hyperedge is
//! dropped whole; removing a member of a `REQUIRES ONE OF` branch empties that
//! branch, so the branch is dropped, and a disjunction left with one branch is
//! rebuilt as the conjunction it has become. Both are recorded on the outcome
//! rather than being silent.

use academic_domain::EntityId;
use serde::{Deserialize, Serialize};

use crate::{
    CriticalPathError,
    hypergraph::{EdgeMember, Hyperedge, PrerequisiteHypergraph, satisfying_sets},
};

/// What removing one relation does to the set of satisfying routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeOutcome {
    /// The goal can no longer be satisfied at all.
    GoalBecomesUnsatisfiable,
    /// Fewer routes satisfy the goal; the ones that remain are unchanged.
    FewerRoutes,
    /// The routes themselves change: at least one concept stops being needed.
    RoutesLoseAConcept,
    /// Nothing changes. The relation was not load-bearing.
    NoChange,
}

impl EdgeOutcome {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GoalBecomesUnsatisfiable => "GOAL_BECOMES_UNSATISFIABLE",
            Self::FewerRoutes => "FEWER_ROUTES",
            Self::RoutesLoseAConcept => "ROUTES_LOSE_A_CONCEPT",
            Self::NoChange => "NO_CHANGE",
        }
    }
}

/// One relation's counterfactual: what the answer becomes without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeSensitivity {
    dependent: EntityId,
    prerequisite: EntityId,
    predicate: &'static str,
    outcome: EdgeOutcome,
    routes_before: usize,
    routes_after: usize,
    concepts_no_longer_needed: Vec<EntityId>,
}

impl EdgeSensitivity {
    /// The concept the relation is stated about.
    #[must_use]
    pub const fn dependent(&self) -> EntityId {
        self.dependent
    }

    /// The concept it requires.
    #[must_use]
    pub const fn prerequisite(&self) -> EntityId {
        self.prerequisite
    }

    /// Which section 7.2 predicate, in `P2-C4`'s own spelling.
    #[must_use]
    pub const fn predicate(&self) -> &'static str {
        self.predicate
    }

    /// What removing it does.
    #[must_use]
    pub const fn outcome(&self) -> EdgeOutcome {
        self.outcome
    }

    /// How many routes satisfied the goal with the relation in place.
    #[must_use]
    pub const fn routes_before(&self) -> usize {
        self.routes_before
    }

    /// How many satisfy it without.
    #[must_use]
    pub const fn routes_after(&self) -> usize {
        self.routes_after
    }

    /// Concepts that were needed by every route before and by none after, in
    /// identifier order.
    #[must_use]
    pub fn concepts_no_longer_needed(&self) -> &[EntityId] {
        &self.concepts_no_longer_needed
    }
}

/// The counterfactual for every member of `graph`, in the order the members
/// appear.
///
/// # Errors
///
/// Whatever [`satisfying_sets`] raises, on either the original graph or a
/// counterfactual one.
pub fn sensitivity(
    graph: &PrerequisiteHypergraph,
    goal_concept: EntityId,
) -> Result<Vec<EdgeSensitivity>, CriticalPathError> {
    let before = satisfying_sets(graph, goal_concept)?;
    let mut found = Vec::new();
    for member in graph.all_members() {
        found.push(sensitivity_of(graph, goal_concept, member, &before)?);
    }
    Ok(found)
}

/// The counterfactual for one member.
///
/// # Errors
///
/// Whatever [`satisfying_sets`] raises.
pub fn sensitivity_of(
    graph: &PrerequisiteHypergraph,
    goal_concept: EntityId,
    member: &EdgeMember,
    before: &[crate::hypergraph::SatisfyingSet],
) -> Result<EdgeSensitivity, CriticalPathError> {
    let reduced = without(graph, member)?;
    let after = satisfying_sets(&reduced, goal_concept)?;

    let needed_before = concepts_needed_by_every(before);
    let needed_after = concepts_needed_by_every(&after);
    let mut lost: Vec<EntityId> = needed_before
        .iter()
        .copied()
        .filter(|concept| !needed_after.contains(concept))
        .collect();
    lost.sort_by_key(|id| id.as_uuid());
    lost.dedup();

    let outcome = if after.is_empty() {
        EdgeOutcome::GoalBecomesUnsatisfiable
    } else if !lost.is_empty() {
        EdgeOutcome::RoutesLoseAConcept
    } else if after.len() < before.len() {
        EdgeOutcome::FewerRoutes
    } else {
        EdgeOutcome::NoChange
    };

    Ok(EdgeSensitivity {
        dependent: member.dependent(),
        prerequisite: member.concept(),
        predicate: member.edge().predicate().as_str(),
        outcome,
        routes_before: before.len(),
        routes_after: after.len(),
        concepts_no_longer_needed: lost,
    })
}

/// A copy of `graph` with `member` removed.
///
/// # Errors
///
/// Whatever [`Hyperedge::requires_all`] and [`Hyperedge::requires_one_of`]
/// raise while the remaining shape is rebuilt.
pub fn without(
    graph: &PrerequisiteHypergraph,
    member: &EdgeMember,
) -> Result<PrerequisiteHypergraph, CriticalPathError> {
    let mut reduced = PrerequisiteHypergraph::new();
    for hyperedge in graph.edges() {
        match hyperedge {
            Hyperedge::RequiresAll { target, members } => {
                let kept: Vec<EdgeMember> = members
                    .iter()
                    .filter(|held| !same_relation(held, member))
                    .cloned()
                    .collect();
                if kept.is_empty() {
                    continue;
                }
                reduced = reduced.with(Hyperedge::requires_all(*target, kept)?);
            }
            Hyperedge::RequiresOneOf { target, branches } => {
                let kept: Vec<Vec<EdgeMember>> = branches
                    .iter()
                    .map(|branch| {
                        branch
                            .iter()
                            .filter(|held| !same_relation(held, member))
                            .cloned()
                            .collect::<Vec<EdgeMember>>()
                    })
                    .filter(|branch| !branch.is_empty())
                    .collect();
                match kept.len() {
                    0 => continue,
                    // A disjunction with one branch left is the conjunction it
                    // has become, and saying so is what stops the counterfactual
                    // from reporting a choice that no longer exists.
                    1 => {
                        let Some(only) = kept.into_iter().next() else {
                            continue;
                        };
                        reduced = reduced.with(Hyperedge::requires_all(*target, only)?);
                    }
                    _ => reduced = reduced.with(Hyperedge::requires_one_of(*target, kept)?),
                }
            }
        }
    }
    Ok(reduced)
}

/// Two members are the same relation when they join the same two concepts
/// through the same predicate.
///
/// The evidence list is deliberately not compared: two assertions of one
/// relation with different citations are one relation, and removing it removes
/// both.
fn same_relation(left: &EdgeMember, right: &EdgeMember) -> bool {
    left.dependent() == right.dependent()
        && left.concept() == right.concept()
        && left.edge().predicate() == right.edge().predicate()
}

/// Concepts every satisfying set holds.
///
/// Empty when there are no sets, which is what makes an unsatisfiable
/// counterfactual report `GoalBecomesUnsatisfiable` rather than a long list of
/// concepts that stopped being needed.
fn concepts_needed_by_every(sets: &[crate::hypergraph::SatisfyingSet]) -> Vec<EntityId> {
    let Some(first) = sets.first() else {
        return Vec::new();
    };
    first
        .concepts()
        .iter()
        .copied()
        .filter(|concept| sets.iter().all(|set| set.holds(*concept)))
        .collect()
}
