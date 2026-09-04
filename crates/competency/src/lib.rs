//! `P2-Y1`: section 24.1's `Competency`, and section 24.3's evidence stages.
//!
//! Section 7.1 states the whole of this task in one sentence:
//!
//! > `Competency는 "개념을 안다"가 아니라 관찰 가능한 상황에서 수행할 수 있다는
//! > 문장으로 모델링한다.`
//!
//! `P2-N1` fixed what a concept is. `P2-N2` fixed what the evidence about a
//! person supports saying, and which promotions are forbidden. `P2-R5` fixed
//! that using a repository is not a personal claim. This crate is the step that
//! asks a different question about the same graph: **not what is known, but
//! what can be done, in what situation, judged how, and shown by what.**
//!
//! ## `knows X` has no constructor
//!
//! There is no `statement` argument anywhere in this crate. [`Competency`] has
//! no statement field, [`declare`] takes none, and
//! [`Competency::statement`] renders one from parts that a knowledge claim
//! cannot supply:
//!
//! | Section 24.1 part | What holds it |
//! |---|---|
//! | `context` | [`Situation`] is a required argument of [`declare`] and refuses the empty one |
//! | `performanceCriteria` | at least one [`PerformanceCriterion`], each with prose and at least one concept |
//! | `evidenceRubric` | every criterion is named by at least one [`RubricRow`], in both directions |
//! | `enabledByConcepts` | at least one [`EnablingConcept`], and every criterion's concepts come from it |
//!
//! A statement with no situation, no criterion, or a criterion nothing in the
//! rubric witnesses, is a value that cannot be built. That is what makes the
//! statement observable: every part of it names either an occasion somebody
//! could watch or an artifact somebody could open.
//!
//! Deserialization does not open a second door. `Deserialize` is `try_from`,
//! and it re-renders the statement from the parts and refuses a document whose
//! `statement` field disagrees, so a hand-written sentence cannot ride in
//! through JSON either.
//!
//! ## A concept and a competency are two types
//!
//! [`CompetencyId`] and [`ConceptRef`] have no conversion in either direction,
//! and [`ConceptRef`] carries the namespace that named it. See
//! [`crate::identity`] for why the namespace is part of the value, and
//! `crates/competency/tests/compile_fail/` for the compiled half.
//!
//! **This crate resolves no namespace into another.** `P2-N2` names concepts by
//! `P2-N1`'s `EntityId` and `P2-R5` names them by `P2-R4`'s classification
//! token, and turning one into the other is the entity registry's job. Doing it
//! here would be exactly the silent conversion section 24.1 refuses, one type
//! over.
//!
//! ## Using a dependency settles nothing
//!
//! Section 24.3's `dependency를 사용했다는 이유만으로 competency를 채우지
//! 않는다` is held in two places, and neither of them is a rule written here:
//!
//! * [`evidence::PromotingEvidence`] refuses `P2-N2` evidence whose section
//!   13.2 row carries `EvidenceCeiling::NoPromotion`, which is that table's own
//!   answer for `dependency/install/import만 존재`; and
//! * [`evidence::StageEvidence`] can be founded on `P2-R5`'s
//!   `PersonalApplicationClaim` and has no arm at all for its
//!   `ProjectObservationClaim` sibling — and a snapshot that only declares a
//!   dependency produces the second and never the first.
//!
//! The join itself is the third place it could have gone wrong, and
//! [`sheet::fill`] is written against `P2-R5`'s own measured defect: see that
//! module and [`crate::criterion`].
//!
//! ## It opens nothing and persists nothing
//!
//! No file, no socket, no clock, no `academic-store` edge and no migration.
//! Every input arrives as an argument, and a sheet is a derivation over
//! evidence two crates below already froze.
//!
//! ## What this task does not decide
//!
//! * **Role bundles.** `P2-Y2` owns `RoleProfile`, versioning, importance
//!   inside a bundle, and fork lineage. Nothing here bundles competencies.
//! * **The readiness view.** `P2-Y3` owns the matrix as a view, the separation
//!   of missing from unknown from freshness, auxiliary scores and their
//!   disclosure, and the non-guarantee notice. [`sheet::CellState`] separates
//!   three readings and displays none of them.
//! * **Freshness.** `P2-N3` owns the bands. There is no time input to any
//!   function here.
//! * **§38.** This task leaves no gate open and closes none.

pub mod criterion;
pub mod enabling;
pub mod evidence;
pub mod identity;
pub mod rubric;
pub mod sheet;
pub mod stage;

use std::{collections::BTreeSet, fmt};

use academic_domain::predicates::{NodeType, PredicateName};
use serde::{Deserialize, Serialize};

pub use criterion::{
    ContributionImportance, EnablingConcept, Necessity, PerformanceCriterion, Situation,
};
pub use enabling::{EnablingEdge, EnablingGraph};
pub use evidence::{EvidenceOrigin, EvidenceSource, PromotingEvidence, StageEvidence};
pub use identity::{CompetencyId, ConceptNamespace, ConceptRef, CriterionId, RecordId};
pub use rubric::{EvidenceRubric, RubricRow};
pub use sheet::{CellState, RubricCell, RubricSheet, fill};
pub use stage::EvidenceStage;

/// Why a competency operation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompetencyError {
    /// An identifier was empty, too long, or held a forbidden byte.
    #[error("the {0} identifier {1:?} is not [A-Za-z0-9._-] within 64 bytes")]
    InvalidIdentifier(&'static str, String),
    /// A required piece of prose carried nothing.
    #[error("the {0} carries no text")]
    EmptyText(&'static str),
    /// Section 24.1: a competency with no performance criterion states no
    /// performance.
    #[error("competency {0} states no performance criterion")]
    NoPerformanceCriterion(String),
    /// Section 24.1: a competency nothing enables is not enabled by concepts.
    #[error("competency {0} names no enabling concept")]
    NoEnablingConcept(String),
    /// Two criteria were written under one identifier.
    #[error("competency {competency} states criterion {criterion} twice")]
    DuplicateCriterion {
        /// Which competency.
        competency: String,
        /// Which criterion identifier.
        criterion: String,
    },
    /// One concept was listed twice in `enabledByConcepts`.
    #[error("competency {0} lists one enabling concept twice")]
    DuplicateEnablingConcept(String),
    /// A criterion named no concept at all, so no evidence could be joined to
    /// it without falling back to the competency's whole enabling set.
    #[error("criterion {0} names no concept")]
    CriterionNamesNoConcept(String),
    /// A criterion named a concept the competency is not enabled by.
    #[error("criterion {criterion} names a concept competency {competency} is not enabled by")]
    CriterionNamesUnenablingConcept {
        /// Which competency.
        competency: String,
        /// Which criterion.
        criterion: String,
    },
    /// A rubric row named a criterion the competency does not state.
    #[error(
        "competency {competency}'s rubric names criterion {criterion}, which it does not state"
    )]
    RubricRowNamesUnknownCriterion {
        /// Which competency.
        competency: String,
        /// Which criterion identifier the row named.
        criterion: String,
    },
    /// A criterion no rubric row witnesses is a criterion nobody can check.
    #[error("competency {competency}'s rubric witnesses criterion {criterion} at no stage")]
    CriterionHasNoRubricRow {
        /// Which competency.
        competency: String,
        /// Which criterion.
        criterion: String,
    },
    /// Section 13.2 gives this evidence row no promotion at all.
    #[error("evidence row {0} licenses no promotion, so it settles no cell")]
    EvidenceLicensesNoPromotion(&'static str),
    /// A `P2-R5` claim that had been taken back was offered as evidence.
    #[error("claim {0} has been rejected")]
    ClaimIsRejected(String),
    /// A deserialized competency's statement was not the one its parts render.
    #[error("competency {0}'s statement is not the one its context, criteria and rubric render")]
    StatementDoesNotMatch(String),
}

/// Section 24.1's `statement`, rendered from the parts.
///
/// Not a caller's sentence. It has no constructor of its own:
/// [`Competency::statement`] is the one producer, and every part of what it
/// renders is a value [`declare`] already required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompetencyStatement {
    situation: String,
    performances: Vec<String>,
    witnesses: Vec<String>,
}

impl CompetencyStatement {
    /// The situation the performance is judged in.
    #[must_use]
    pub fn situation(&self) -> &str {
        &self.situation
    }

    /// What has to be true of the performance, in criterion order.
    #[must_use]
    pub fn performances(&self) -> &[String] {
        &self.performances
    }

    /// What a reader has to be able to open, in rubric order.
    #[must_use]
    pub fn witnesses(&self) -> &[String] {
        &self.witnesses
    }
}

impl fmt::Display for CompetencyStatement {
    /// Renders the sentence section 7.1 asks for: a situation, a performance in
    /// it, and what somebody would look at.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Given {}, the user ", self.situation)?;
        for (index, performance) in self.performances.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; and ")?;
            }
            formatter.write_str(performance)?;
        }
        formatter.write_str(", witnessed by ")?;
        for (index, witness) in self.witnesses.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; ")?;
            }
            formatter.write_str(witness)?;
        }
        formatter.write_str(".")
    }
}

/// Section 24.1's `Competency`.
///
/// No public field, no setter and no `&mut self` method. An edit is a new
/// [`declare`] over new parts, the way `P2-R5`'s claims and `P2-N2`'s
/// assertions are replaced rather than mutated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CompetencyWire", into = "CompetencyWire")]
pub struct Competency {
    id: CompetencyId,
    context: Situation,
    criteria: Vec<PerformanceCriterion>,
    enabled_by: Vec<EnablingConcept>,
    rubric: EvidenceRubric,
}

impl Competency {
    /// Its identity.
    #[must_use]
    pub const fn id(&self) -> &CompetencyId {
        &self.id
    }

    /// Section 24.1's `context`.
    #[must_use]
    pub const fn context(&self) -> &Situation {
        &self.context
    }

    /// Section 24.1's `performanceCriteria`, in the order they were written.
    #[must_use]
    pub fn criteria(&self) -> &[PerformanceCriterion] {
        &self.criteria
    }

    /// One criterion.
    #[must_use]
    pub fn criterion(&self, id: &CriterionId) -> Option<&PerformanceCriterion> {
        self.criteria.iter().find(|item| item.id() == id)
    }

    /// Section 24.1's `enabledByConcepts`, with both section 7.2 qualifiers.
    #[must_use]
    pub fn enabled_by(&self) -> &[EnablingConcept] {
        &self.enabled_by
    }

    /// Section 24.1's `evidenceRubric`.
    #[must_use]
    pub const fn rubric(&self) -> &EvidenceRubric {
        &self.rubric
    }

    /// Section 24.1's `statement`, rendered from the parts above.
    ///
    /// The one producer of a [`CompetencyStatement`]. There is no argument here
    /// and no field behind it: the sentence is what the situation, the criteria
    /// and the rubric already say.
    #[must_use]
    pub fn statement(&self) -> CompetencyStatement {
        CompetencyStatement {
            situation: self.context.as_str().to_owned(),
            performances: self
                .criteria
                .iter()
                .map(|criterion| criterion.requirement().to_owned())
                .collect(),
            witnesses: self
                .rubric
                .rows()
                .iter()
                .map(|row| format!("{}: {}", row.stage().as_str(), row.admits()))
                .collect(),
        }
    }

    /// Section 7.1's node type for this entity, from the shared vocabulary.
    ///
    /// `academic_domain` already places `Competency` in the node hierarchy, so
    /// this reads that enumeration rather than declaring a second one.
    #[must_use]
    pub const fn node_type() -> NodeType {
        NodeType::Competency
    }

    /// The section 7.2 predicate an `enabledByConcepts` entry asserts.
    #[must_use]
    pub const fn enabling_predicate() -> PredicateName {
        PredicateName::EnablesCompetency
    }
}

/// Declares one competency from its section 24.1 parts.
///
/// # Errors
///
/// [`CompetencyError::NoPerformanceCriterion`] and
/// [`CompetencyError::NoEnablingConcept`] when either list is empty;
/// [`CompetencyError::DuplicateCriterion`] and
/// [`CompetencyError::DuplicateEnablingConcept`] when either lists one subject
/// twice; [`CompetencyError::CriterionNamesUnenablingConcept`] when a criterion
/// is about a concept the competency is not enabled by;
/// [`CompetencyError::RubricRowNamesUnknownCriterion`] when a rubric row names a
/// criterion that is not here; and
/// [`CompetencyError::CriterionHasNoRubricRow`] when a criterion is witnessed at
/// no stage, which is the statement nobody can check.
pub fn declare(
    id: CompetencyId,
    context: Situation,
    criteria: Vec<PerformanceCriterion>,
    enabled_by: Vec<EnablingConcept>,
    rubric: EvidenceRubric,
) -> Result<Competency, CompetencyError> {
    if criteria.is_empty() {
        return Err(CompetencyError::NoPerformanceCriterion(
            id.as_str().to_owned(),
        ));
    }
    if enabled_by.is_empty() {
        return Err(CompetencyError::NoEnablingConcept(id.as_str().to_owned()));
    }

    let mut seen_criteria = BTreeSet::new();
    for criterion in &criteria {
        if !seen_criteria.insert(criterion.id().clone()) {
            return Err(CompetencyError::DuplicateCriterion {
                competency: id.as_str().to_owned(),
                criterion: criterion.id().as_str().to_owned(),
            });
        }
    }

    let mut enabling = BTreeSet::new();
    for entry in &enabled_by {
        if !enabling.insert(entry.concept().clone()) {
            return Err(CompetencyError::DuplicateEnablingConcept(
                id.as_str().to_owned(),
            ));
        }
    }

    for criterion in &criteria {
        for concept in criterion.about() {
            if !enabling.contains(concept) {
                return Err(CompetencyError::CriterionNamesUnenablingConcept {
                    competency: id.as_str().to_owned(),
                    criterion: criterion.id().as_str().to_owned(),
                });
            }
        }
    }

    for row in rubric.rows() {
        if !seen_criteria.contains(row.criterion()) {
            return Err(CompetencyError::RubricRowNamesUnknownCriterion {
                competency: id.as_str().to_owned(),
                criterion: row.criterion().as_str().to_owned(),
            });
        }
    }

    for criterion in &criteria {
        if !rubric.witnesses(criterion.id()) {
            return Err(CompetencyError::CriterionHasNoRubricRow {
                competency: id.as_str().to_owned(),
                criterion: criterion.id().as_str().to_owned(),
            });
        }
    }

    Ok(Competency {
        id,
        context,
        criteria,
        enabled_by,
        rubric,
    })
}

/// The serialized shape, with section 24.1's own key names.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompetencyWire {
    id: CompetencyId,
    statement: String,
    context: Situation,
    performance_criteria: Vec<PerformanceCriterion>,
    enabled_by_concepts: Vec<EnablingConcept>,
    evidence_rubric: EvidenceRubric,
}

impl TryFrom<CompetencyWire> for Competency {
    type Error = CompetencyError;

    /// Runs [`declare`] over the parts and then re-renders the statement.
    ///
    /// A document whose `statement` is not the one its own parts render is
    /// refused, so deserialization is not a second way to write a sentence
    /// nobody can check.
    fn try_from(wire: CompetencyWire) -> Result<Self, Self::Error> {
        let competency = declare(
            wire.id,
            wire.context,
            wire.performance_criteria,
            wire.enabled_by_concepts,
            wire.evidence_rubric,
        )?;
        if competency.statement().to_string() == wire.statement {
            Ok(competency)
        } else {
            Err(CompetencyError::StatementDoesNotMatch(
                competency.id.as_str().to_owned(),
            ))
        }
    }
}

impl From<Competency> for CompetencyWire {
    fn from(value: Competency) -> Self {
        let statement = value.statement().to_string();
        Self {
            id: value.id,
            statement,
            context: value.context,
            performance_criteria: value.criteria,
            enabled_by_concepts: value.enabled_by,
            evidence_rubric: value.rubric,
        }
    }
}
