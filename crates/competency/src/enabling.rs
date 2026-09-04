//! Section 24.1's `여러 concept가 competency를 enable하고, 한 concept가 여러
//! competency에 재사용된다`, as a graph.
//!
//! ## One direction is stored and both are queried
//!
//! Section 7.2 fixes `ENABLES_COMPETENCY` as `concept → competency`,
//! `ManyToMany`, with the inverse label `is enabled by`, and it ends with
//! `역방향 탐색은 query view로 제공하며 반대 edge를 중복 저장하지 않는다`.
//!
//! So an [`EnablingGraph`] holds one list of forward edges and answers both
//! questions by reading it. There is no reverse index, no second vector and no
//! `competency → concept` row: [`EnablingGraph::edges`] is the whole of the
//! stored state, and `many_to_many_enabling_edges_query_both_ways` compares its
//! length against the number of `enabledByConcepts` entries the competencies
//! carry.
//!
//! ## The direction is in the field types
//!
//! An [`EnablingEdge`] has a [`ConceptRef`] end and a [`CompetencyId`] end, and
//! those are two types with no conversion between them, so an edge asserted
//! backwards is a program that does not compile rather than a row somebody has
//! to notice.

use serde::Serialize;

use crate::{
    Competency,
    criterion::{ContributionImportance, Necessity},
    identity::{CompetencyId, ConceptRef},
};

/// One section 7.2 `ENABLES_COMPETENCY` edge, with both required qualifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnablingEdge {
    concept: ConceptRef,
    competency: CompetencyId,
    importance: ContributionImportance,
    necessity: Necessity,
}

impl EnablingEdge {
    /// The subject end.
    #[must_use]
    pub const fn concept(&self) -> &ConceptRef {
        &self.concept
    }

    /// The object end.
    #[must_use]
    pub const fn competency(&self) -> &CompetencyId {
        &self.competency
    }

    /// The `contribution_importance` qualifier.
    #[must_use]
    pub const fn importance(&self) -> ContributionImportance {
        self.importance
    }

    /// The `necessity` qualifier.
    #[must_use]
    pub const fn necessity(&self) -> Necessity {
        self.necessity
    }
}

/// Every enabling edge of a set of competencies, stored once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct EnablingGraph {
    edges: Vec<EnablingEdge>,
}

impl EnablingGraph {
    /// Reads the edges out of the competencies themselves.
    ///
    /// The competencies are the one place `enabledByConcepts` is written, so
    /// the graph is a view over them and cannot disagree with them. A second
    /// list a caller could edit on its own would be a second answer to the same
    /// question.
    #[must_use]
    pub fn of(competencies: &[Competency]) -> Self {
        let mut edges = Vec::new();
        for competency in competencies {
            for enabling in competency.enabled_by() {
                edges.push(EnablingEdge {
                    concept: enabling.concept().clone(),
                    competency: competency.id().clone(),
                    importance: enabling.importance(),
                    necessity: enabling.necessity(),
                });
            }
        }
        Self { edges }
    }

    /// Every stored edge, in the order the competencies wrote them.
    ///
    /// This is the whole of the graph's state. There is nothing else to read.
    #[must_use]
    pub fn edges(&self) -> &[EnablingEdge] {
        &self.edges
    }

    /// Section 7.2's forward reading: what this concept enables.
    #[must_use]
    pub fn competencies_enabled_by(&self, concept: &ConceptRef) -> Vec<&EnablingEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.concept() == concept)
            .collect()
    }

    /// Section 7.2's inverse reading: what enables this competency.
    ///
    /// A query view over the same list, which is why the section's
    /// `반대 edge를 중복 저장하지 않는다` is a fact about the type rather than a
    /// rule about how to use it.
    #[must_use]
    pub fn concepts_enabling(&self, competency: &CompetencyId) -> Vec<&EnablingEdge> {
        self.edges
            .iter()
            .filter(|edge| edge.competency() == competency)
            .collect()
    }
}
