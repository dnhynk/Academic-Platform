//! Section 16.4's `사용자가 relation을 제거·추가해 다시 계산할 수 있다`, and
//! section 34.5's `비용/edge 수정 후 재계산, **old path 보존**`.
//!
//! ## The base survives every edit, including the second one
//!
//! [`EditedPlan::apply`] on an already-edited plan keeps the **original** base,
//! not the previous recomputation. So a chain of three edits still answers
//! `base()` with the plan before the first, and the edits are a list rather
//! than a moving pointer. That is `CONTRIBUTING.md` rule 2 -- canonical values
//! are append-only and a correction is a new entry, never an overwrite -- and it
//! is the discipline `P2-R4`'s `ClassificationConflict`, `P2-R3`'s
//! `ImplementationDrift` and `P2-N5`'s retained tied roots already set.
//!
//! Nothing here takes `&mut`. An edit produces a new [`EditedPlan`] and the old
//! one is still a value the caller holds.
//!
//! ## An edit is a relation, not a cost
//!
//! [`RelationEdit`] adds or removes a hyperedge member. It cannot change a
//! vector, cannot change a constraint input and cannot change a rank: those
//! arrive from the recomputation, which runs the same engine over the edited
//! graph. Section 34.5 lists `비용/edge 수정` together; the cost half is a
//! different input to the same recomputation and is supplied by the caller
//! rather than edited here, because a cost the user typed is a `P2-N6` input
//! and not a relation.

use academic_domain::EntityId;

use crate::{
    CriticalPathError,
    counterfactual::without,
    hypergraph::{EdgeMember, Hyperedge, PrerequisiteHypergraph},
    plan::CriticalPathResult,
};

/// One user change to the relation graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationEdit {
    /// `제거`: the user says this relation is wrong.
    Remove {
        /// Which relation.
        member: EdgeMember,
    },
    /// `추가`: the user says this relation is missing.
    Add {
        /// The hyperedge to add it to, or to create.
        hyperedge: Hyperedge,
    },
}

impl RelationEdit {
    /// Stable spelling of which edit this is.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Remove { .. } => "REMOVE",
            Self::Add { .. } => "ADD",
        }
    }

    /// The concept the edited relation is stated about.
    #[must_use]
    pub fn target(&self) -> EntityId {
        match self {
            Self::Remove { member } => member.dependent(),
            Self::Add { hyperedge } => hyperedge.target(),
        }
    }
}

/// Applies one edit to a hypergraph, returning a new one.
///
/// # Errors
///
/// Whatever [`without`] and the [`Hyperedge`] constructors raise.
pub fn edited(
    graph: &PrerequisiteHypergraph,
    edit: &RelationEdit,
) -> Result<PrerequisiteHypergraph, CriticalPathError> {
    match edit {
        RelationEdit::Remove { member } => without(graph, member),
        RelationEdit::Add { hyperedge } => {
            let mut next = PrerequisiteHypergraph::new();
            for held in graph.edges() {
                next = next.with(held.clone());
            }
            Ok(next.with(hyperedge.clone()))
        }
    }
}

/// A base plan, the edits made to it, and the plan those edits produce.
///
/// Three values, and the first is never overwritten. See the module note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditedPlan {
    base: CriticalPathResult,
    base_graph: PrerequisiteHypergraph,
    edits: Vec<RelationEdit>,
    recomputed: CriticalPathResult,
    recomputed_graph: PrerequisiteHypergraph,
}

impl EditedPlan {
    /// Records one edit and the plan it produced.
    ///
    /// `recompute` is the caller's own call back into [`crate::engine::plan`]
    /// over the edited graph. Taking it as an argument rather than calling the
    /// engine here is what keeps this module free of the engine's inputs: the
    /// same base can be re-planned under a different preference or different
    /// constraint inputs, and this type records what happened either way.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::EditChangesTheGoal`] when the recomputed plan is
    /// for a different goal from the base. An edit changes relations, not which
    /// goal is being planned for, and a recomputation that moved the goal is
    /// not a recomputation of this plan.
    pub fn of(
        base: CriticalPathResult,
        base_graph: PrerequisiteHypergraph,
        edit: RelationEdit,
        recomputed: CriticalPathResult,
        recomputed_graph: PrerequisiteHypergraph,
    ) -> Result<Self, CriticalPathError> {
        if base.goal() != recomputed.goal() {
            return Err(CriticalPathError::EditChangesTheGoal);
        }
        Ok(Self {
            base,
            base_graph,
            edits: vec![edit],
            recomputed,
            recomputed_graph,
        })
    }

    /// Records a further edit, keeping the **original** base.
    ///
    /// # Errors
    ///
    /// [`CriticalPathError::EditChangesTheGoal`], as above.
    pub fn apply(
        &self,
        edit: RelationEdit,
        recomputed: CriticalPathResult,
        recomputed_graph: PrerequisiteHypergraph,
    ) -> Result<Self, CriticalPathError> {
        if self.base.goal() != recomputed.goal() {
            return Err(CriticalPathError::EditChangesTheGoal);
        }
        let mut edits = self.edits.clone();
        edits.push(edit);
        Ok(Self {
            base: self.base.clone(),
            base_graph: self.base_graph.clone(),
            edits,
            recomputed,
            recomputed_graph,
        })
    }

    /// The plan before any edit. Never the previous recomputation.
    #[must_use]
    pub const fn base(&self) -> &CriticalPathResult {
        &self.base
    }

    /// The relation graph before any edit.
    #[must_use]
    pub const fn base_graph(&self) -> &PrerequisiteHypergraph {
        &self.base_graph
    }

    /// Every edit, in the order they were applied.
    #[must_use]
    pub fn edits(&self) -> &[RelationEdit] {
        &self.edits
    }

    /// The plan the edits produce.
    #[must_use]
    pub const fn recomputed(&self) -> &CriticalPathResult {
        &self.recomputed
    }

    /// The relation graph the edits produce.
    #[must_use]
    pub const fn recomputed_graph(&self) -> &PrerequisiteHypergraph {
        &self.recomputed_graph
    }

    /// Whether the recomputation reached a different answer from the base.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.base.ranked().len() != self.recomputed.ranked().len()
            || self
                .base
                .ranked()
                .iter()
                .zip(self.recomputed.ranked())
                .any(|(before, after)| {
                    before.candidate().satisfying_set().concepts()
                        != after.candidate().satisfying_set().concepts()
                })
    }
}
