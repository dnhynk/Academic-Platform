//! Section 20.2's `concept requirements with AND/OR branches`, and why the
//! `OR` half can never be published unconditionally.
//!
//! ## This is section 16.1's hypergraph, not a second one
//!
//! `P2-N6` already owns `REQUIRES ALL [..]` and `REQUIRES ONE OF`, and owns the
//! fact that the answer over them is **satisfaction and not a shortest path**.
//! [`ArchitectureBranch::hypergraph`] builds
//! [`academic_critical_path::Hyperedge`] values and
//! [`ArchitectureBranch::satisfying_sets`] delegates to
//! [`academic_critical_path::satisfying_sets`]. There is no solver here, no
//! second edge type, and no path length anywhere in this crate.
//!
//! ## `and_or_branches_are_conditional`
//!
//! [`ConceptRequirement`] carries a [`RequirementCondition`], and the two arms
//! have two different producers:
//!
//! | Producer | Condition it stamps |
//! |---|---|
//! | [`ConceptRequirement::always`] | [`RequirementCondition::Unconditional`] |
//! | [`BranchGroup::of`] | [`RequirementCondition::Conditional`], one per member |
//!
//! `BranchGroup::of` takes the decision and the alternative and stamps them onto
//! every member it is given, so a member of an `OR` branch is conditional **by
//! construction**. There is no public constructor of a `ConceptRequirement` that
//! takes a [`RequirementCondition`] as an argument, so an unconditional
//! requirement cannot be smuggled into a branch, and the field is private, so it
//! cannot be rewritten afterwards.
//!
//! `every_public_signature_is_in_the_inventory` and
//! `every_impl_header_in_this_crate_is_in_the_inventory` are what close the
//! shapes a name list cannot see: a third producer under any name, or a
//! `From<BranchGroup> for ConceptRequirement` that flattened one, would be an
//! entry in a pinned inventory rather than an invisible addition.
//!
//! ## An `OR` decision is one the goal actually left open
//!
//! [`ArchitectureBranch::of`] refuses a branch group naming a decision or an
//! alternative the goal does not hold. So the `OR` structure is the goal's own
//! `unresolvedDecisions`, and a plan cannot invent a choice the user never
//! stated — nor resolve one by leaving a branch out, because a decision with
//! fewer than two groups is refused with the reason
//! [`academic_critical_path::Hyperedge::requires_one_of`] gives.

use std::collections::{BTreeMap, BTreeSet};

use academic_critical_path::{EdgeMember, EdgeStanding, Hyperedge, SatisfyingSet, satisfying_sets};
use academic_domain::{EntityId, entity_registry::EntityKind};
use academic_gap::{PrerequisiteEdge, gap_bearing};
use serde::{Deserialize, Serialize};

use crate::{
    BuildLearnError, goal::ProjectGoal, responsibility::ResponsibilityDecomposition, text::PartId,
};

/// Whether a concept requirement holds on every branch or on one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequirementCondition {
    /// Needed whichever way every open decision goes.
    Unconditional,
    /// Needed only if one alternative of one open decision is chosen.
    Conditional {
        /// The decision it hangs off.
        decision: PartId,
        /// The alternative of that decision it belongs to.
        alternative: PartId,
    },
}

impl RequirementCondition {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unconditional => "UNCONDITIONAL",
            Self::Conditional { .. } => "CONDITIONAL",
        }
    }

    /// The decision this hangs off, when it hangs off one.
    #[must_use]
    pub const fn decision(&self) -> Option<&PartId> {
        match self {
            Self::Unconditional => None,
            Self::Conditional { decision, .. } => Some(decision),
        }
    }

    /// The alternative this belongs to, when it belongs to one.
    #[must_use]
    pub const fn alternative(&self) -> Option<&PartId> {
        match self {
            Self::Unconditional => None,
            Self::Conditional { alternative, .. } => Some(alternative),
        }
    }
}

/// One concept the architecture requires, and the responsibility it serves.
///
/// Private fields. The condition has no public constructor taking it as an
/// argument: see the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptRequirement {
    concept: EntityId,
    kind: EntityKind,
    serves: PartId,
    condition: RequirementCondition,
}

impl ConceptRequirement {
    /// Declares a requirement that holds on every branch.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::ConceptCarriesNoPrerequisite`] when `kind` is a tier
    /// that carries no independent prerequisite of its own — `P2-C3`'s rule,
    /// read through `P2-N5`'s [`academic_gap::gap_bearing`] rather than restated,
    /// so `Database를 배우세요` has no value to be expressed as here either.
    pub fn always(
        concept: EntityId,
        kind: EntityKind,
        serves: PartId,
    ) -> Result<Self, BuildLearnError> {
        if !gap_bearing(kind) {
            return Err(BuildLearnError::ConceptCarriesNoPrerequisite { kind });
        }
        Ok(Self {
            concept,
            kind,
            serves,
            condition: RequirementCondition::Unconditional,
        })
    }

    /// The concept required.
    #[must_use]
    pub const fn concept(&self) -> EntityId {
        self.concept
    }

    /// The tier `P2-C3`'s registry holds for it.
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        self.kind
    }

    /// The responsibility this requirement serves.
    #[must_use]
    pub const fn serves(&self) -> &PartId {
        &self.serves
    }

    /// Whether it holds on every branch or on one.
    #[must_use]
    pub const fn condition(&self) -> &RequirementCondition {
        &self.condition
    }
}

/// One `OR` branch: the concepts one alternative of one decision brings with it.
///
/// A branch is taken **whole**, which is section 16.1's own rule, so the group
/// is the unit and not its cheapest member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchGroup {
    decision: PartId,
    alternative: PartId,
    members: Vec<ConceptRequirement>,
}

impl BranchGroup {
    /// Declares one branch, stamping its condition onto every member.
    ///
    /// `members` arrive as `(concept, kind, serves)` triples rather than as
    /// [`ConceptRequirement`] values, because a value that already exists
    /// carries a condition and this constructor's whole job is that the
    /// condition is not the caller's to choose.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::EmptyBranchGroup`] for no members;
    /// [`BuildLearnError::ConceptCarriesNoPrerequisite`] as
    /// [`ConceptRequirement::always`] gives it.
    pub fn of(
        decision: PartId,
        alternative: PartId,
        members: Vec<(EntityId, EntityKind, PartId)>,
    ) -> Result<Self, BuildLearnError> {
        if members.is_empty() {
            return Err(BuildLearnError::EmptyBranchGroup {
                decision: decision.as_str().to_owned(),
                alternative: alternative.as_str().to_owned(),
            });
        }
        let mut stamped = Vec::with_capacity(members.len());
        for (concept, kind, serves) in members {
            if !gap_bearing(kind) {
                return Err(BuildLearnError::ConceptCarriesNoPrerequisite { kind });
            }
            stamped.push(ConceptRequirement {
                concept,
                kind,
                serves,
                condition: RequirementCondition::Conditional {
                    decision: decision.clone(),
                    alternative: alternative.clone(),
                },
            });
        }
        Ok(Self {
            decision,
            alternative,
            members: stamped,
        })
    }

    /// The decision this branch is one answer to.
    #[must_use]
    pub const fn decision(&self) -> &PartId {
        &self.decision
    }

    /// The alternative this branch is.
    #[must_use]
    pub const fn alternative(&self) -> &PartId {
        &self.alternative
    }

    /// The concepts the branch brings with it, every one conditional.
    #[must_use]
    pub fn members(&self) -> &[ConceptRequirement] {
        &self.members
    }
}

/// Section 20.2's second stage: the AND/OR requirements of one decomposition.
///
/// Private fields, one producer, no `Default`. Holds the decomposition by value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureBranch {
    decomposition: ResponsibilityDecomposition,
    target: EntityId,
    conjunction: Vec<ConceptRequirement>,
    disjunctions: Vec<Vec<BranchGroup>>,
}

impl ArchitectureBranch {
    /// Derives the AND/OR requirements from `decomposition`.
    ///
    /// `target` is the capability node the requirements are stated about — the
    /// `Goal:` line of section 16.1's own drawing.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::RequirementServesNoResponsibility`] when a requirement
    /// names a responsibility the decomposition does not hold;
    /// [`BuildLearnError::BranchNamesNoDecision`] and
    /// [`BuildLearnError::BranchNamesNoAlternative`] when a group names a
    /// decision or an alternative the goal did not leave open;
    /// [`BuildLearnError::DecisionHasOneBranch`] when a decision is offered
    /// fewer than two groups, or two groups for one alternative; and
    /// [`BuildLearnError::DecisionHasNoBranch`] when a decision the goal left
    /// open has no group at all, because a plan that answers only some of the
    /// open decisions has silently resolved the rest.
    pub fn of(
        decomposition: ResponsibilityDecomposition,
        target: EntityId,
        conjunction: Vec<ConceptRequirement>,
        groups: Vec<BranchGroup>,
    ) -> Result<Self, BuildLearnError> {
        for requirement in &conjunction {
            require_serves(&decomposition, requirement)?;
        }
        let goal = decomposition.goal();
        let mut by_decision: BTreeMap<String, Vec<BranchGroup>> = BTreeMap::new();
        for group in groups {
            let Some(decision) = goal.unresolved_decisions().decision(group.decision()) else {
                return Err(BuildLearnError::BranchNamesNoDecision(
                    group.decision().as_str().to_owned(),
                ));
            };
            if decision.alternative(group.alternative()).is_none() {
                return Err(BuildLearnError::BranchNamesNoAlternative {
                    decision: group.decision().as_str().to_owned(),
                    alternative: group.alternative().as_str().to_owned(),
                });
            }
            for requirement in group.members() {
                require_serves(&decomposition, requirement)?;
            }
            by_decision
                .entry(group.decision().as_str().to_owned())
                .or_default()
                .push(group);
        }
        for decision in goal.unresolved_decisions().decisions() {
            let Some(groups) = by_decision.get(decision.id().as_str()) else {
                return Err(BuildLearnError::DecisionHasNoBranch(
                    decision.id().as_str().to_owned(),
                ));
            };
            let alternatives: BTreeSet<&str> = groups
                .iter()
                .map(|group| group.alternative().as_str())
                .collect();
            if alternatives.len() != groups.len() || alternatives.len() < 2 {
                return Err(BuildLearnError::DecisionHasOneBranch(
                    decision.id().as_str().to_owned(),
                ));
            }
        }
        let disjunctions = goal
            .unresolved_decisions()
            .decisions()
            .iter()
            .filter_map(|decision| by_decision.remove(decision.id().as_str()))
            .collect();
        Ok(Self {
            decomposition,
            target,
            conjunction,
            disjunctions,
        })
    }

    /// The decomposition this branches from.
    #[must_use]
    pub const fn decomposition(&self) -> &ResponsibilityDecomposition {
        &self.decomposition
    }

    /// The goal underneath, without going through the decomposition.
    #[must_use]
    pub const fn goal(&self) -> &ProjectGoal {
        self.decomposition.goal()
    }

    /// The capability node the requirements are stated about.
    #[must_use]
    pub const fn target(&self) -> EntityId {
        self.target
    }

    /// The `REQUIRES ALL` half, in declaration order.
    #[must_use]
    pub fn conjunction(&self) -> &[ConceptRequirement] {
        &self.conjunction
    }

    /// The `REQUIRES ONE OF` half: the groups of one decision per entry, in the
    /// goal's decision order.
    #[must_use]
    pub fn disjunctions(&self) -> &[Vec<BranchGroup>] {
        &self.disjunctions
    }

    /// Every requirement of both halves, conjunction first.
    #[must_use]
    pub fn requirements(&self) -> Vec<&ConceptRequirement> {
        let mut found: Vec<&ConceptRequirement> = self.conjunction.iter().collect();
        for decision in &self.disjunctions {
            for group in decision {
                found.extend(group.members());
            }
        }
        found
    }

    /// Section 16.1's hypergraph over these requirements.
    ///
    /// One [`Hyperedge::RequiresAll`] for the conjunction and one
    /// [`Hyperedge::RequiresOneOf`] per open decision. Each member is an
    /// [`academic_gap::PrerequisiteEdge`], so it passed `P2-N5`'s admission and
    /// therefore `P2-C4`'s `prerequisite_descriptor`; this crate holds no
    /// predicate allowlist of its own.
    ///
    /// # Errors
    ///
    /// [`BuildLearnError::Gap`] when an edge is refused by `P2-N5`, and
    /// [`BuildLearnError::CriticalPath`] when a hyperedge is refused by `P2-N6`.
    pub fn hypergraph(
        &self,
        edges: &BTreeMap<EntityId, PrerequisiteEdge>,
        standing: EdgeStanding,
    ) -> Result<Vec<Hyperedge>, BuildLearnError> {
        let mut built = Vec::new();
        if !self.conjunction.is_empty() {
            let members = self.members_of(self.conjunction.iter(), edges, standing)?;
            built.push(Hyperedge::requires_all(self.target, members)?);
        }
        for decision in &self.disjunctions {
            let mut branches = Vec::with_capacity(decision.len());
            for group in decision {
                branches.push(self.members_of(group.members().iter(), edges, standing)?);
            }
            built.push(Hyperedge::requires_one_of(self.target, branches)?);
        }
        Ok(built)
    }

    fn members_of<'a>(
        &self,
        requirements: impl Iterator<Item = &'a ConceptRequirement>,
        edges: &BTreeMap<EntityId, PrerequisiteEdge>,
        standing: EdgeStanding,
    ) -> Result<Vec<EdgeMember>, BuildLearnError> {
        let mut members = Vec::new();
        for requirement in requirements {
            let Some(edge) = edges.get(&requirement.concept()) else {
                return Err(BuildLearnError::RequirementHasNoAdmittedEdge(
                    requirement.concept(),
                ));
            };
            members.push(EdgeMember::of(edge.clone(), standing));
        }
        Ok(members)
    }

    /// Section 16.1's answer over this hypergraph: the satisfying **sets**.
    ///
    /// Delegates to `P2-N6`. This crate computes no path length of its own; the
    /// question section 20.2 asks is which whole set of concepts satisfies the
    /// capability, and a satisfying set is that answer.
    ///
    /// # Errors
    ///
    /// As [`ArchitectureBranch::hypergraph`], plus
    /// [`BuildLearnError::CriticalPath`] when `P2-N6` refuses the graph.
    pub fn satisfying_sets(
        &self,
        edges: &BTreeMap<EntityId, PrerequisiteEdge>,
        standing: EdgeStanding,
    ) -> Result<Vec<SatisfyingSet>, BuildLearnError> {
        let mut graph = academic_critical_path::PrerequisiteHypergraph::new();
        for edge in self.hypergraph(edges, standing)? {
            graph = graph.with(edge);
        }
        Ok(satisfying_sets(&graph, self.target)?)
    }
}

fn require_serves(
    decomposition: &ResponsibilityDecomposition,
    requirement: &ConceptRequirement,
) -> Result<(), BuildLearnError> {
    if decomposition.responsibility(requirement.serves()).is_none() {
        return Err(BuildLearnError::RequirementServesNoResponsibility {
            concept: requirement.concept().to_string(),
            responsibility: requirement.serves().as_str().to_owned(),
        });
    }
    Ok(())
}
