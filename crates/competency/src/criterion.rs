//! Section 24.1's `context`, `performanceCriteria` and `enabledByConcepts`.
//!
//! ## A criterion names its own concepts, and that is the whole of §24.3's
//! first sentence
//!
//! `dependency를 사용했다는 이유만으로 competency를 채우지 않는다.` The shape
//! that sentence refuses is a join that succeeds for the wrong reason, and the
//! wrong reason available here is the competency's `enabledByConcepts` list: a
//! competency six concepts enable would have every cell filled by evidence
//! about any one of them, whether or not that evidence has anything to do with
//! the criterion being judged.
//!
//! So [`PerformanceCriterion::about`] is required and non-empty, and
//! [`crate::sheet::fill`] joins on it and on nothing else. There is no arm that
//! falls back to the competency's enabling set when a criterion names no
//! concept, because a criterion that names no concept is a value that cannot be
//! built.
//!
//! `P2-R5` shipped exactly this defect one layer down and measured it:
//! `AuthoredWork::touches` compared a changed site to an observed locator by
//! path when either side carried no symbol, so an unrelated edit inside a file
//! that imported a library was credited with that library's use. The repair
//! there was to compare the pair and refuse the mixed case. The repair here is
//! for the weaker key never to exist.

use serde::{Deserialize, Serialize};

use crate::{
    CompetencyError,
    identity::{ConceptRef, CriterionId, non_empty},
};

/// Section 24.1's `context`: the situation the performance is judged in.
///
/// Required. A statement with no situation is the shape section 7.1 refuses —
/// `Competency는 "개념을 안다"가 아니라 관찰 가능한 상황에서 수행할 수 있다는
/// 문장으로 모델링한다` — because there is no occasion on which anybody could
/// watch it happen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Situation(String);

impl Situation {
    /// Takes one situation.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::EmptyText`] when it carries nothing.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self(non_empty(value.into(), "context")?))
    }

    /// The situation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Situation {
    type Error = CompetencyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Situation> for String {
    fn from(value: Situation) -> Self {
        value.0
    }
}

/// One row of section 24.1's `performanceCriteria`.
///
/// Three parts, all required: which criterion this is, what has to be true of
/// the performance, and which concepts it is about. The third is what evidence
/// is joined on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CriterionWire", into = "CriterionWire")]
pub struct PerformanceCriterion {
    id: CriterionId,
    requirement: String,
    about: Vec<ConceptRef>,
}

impl PerformanceCriterion {
    /// Records one criterion.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::EmptyText`] when the requirement carries nothing, and
    /// [`CompetencyError::CriterionNamesNoConcept`] when `about` is empty.
    pub fn of(
        id: CriterionId,
        requirement: impl Into<String>,
        about: Vec<ConceptRef>,
    ) -> Result<Self, CompetencyError> {
        let requirement = non_empty(requirement.into(), "performance criterion")?;
        if about.is_empty() {
            return Err(CompetencyError::CriterionNamesNoConcept(
                id.as_str().to_owned(),
            ));
        }
        let mut about = about;
        about.sort();
        about.dedup();
        Ok(Self {
            id,
            requirement,
            about,
        })
    }

    /// Which criterion.
    #[must_use]
    pub const fn id(&self) -> &CriterionId {
        &self.id
    }

    /// What has to be true of the performance.
    #[must_use]
    pub fn requirement(&self) -> &str {
        &self.requirement
    }

    /// The concepts this criterion is about, deduplicated and in order.
    #[must_use]
    pub fn about(&self) -> &[ConceptRef] {
        &self.about
    }

    /// Whether this criterion is about `concept`.
    ///
    /// Whole-pair membership over [`ConceptRef`], which carries its namespace.
    /// There is no second comparison and no weaker one: a concept this
    /// criterion does not name is not this criterion's concept, however the
    /// competency around it is enabled.
    #[must_use]
    pub fn is_about(&self, concept: &ConceptRef) -> bool {
        self.about.contains(concept)
    }
}

/// The serialized shape of a [`PerformanceCriterion`].
#[derive(Debug, Serialize, Deserialize)]
struct CriterionWire {
    id: CriterionId,
    requirement: String,
    about: Vec<ConceptRef>,
}

impl TryFrom<CriterionWire> for PerformanceCriterion {
    type Error = CompetencyError;

    fn try_from(wire: CriterionWire) -> Result<Self, Self::Error> {
        Self::of(wire.id, wire.requirement, wire.about)
    }
}

impl From<PerformanceCriterion> for CriterionWire {
    fn from(value: PerformanceCriterion) -> Self {
        Self {
            id: value.id,
            requirement: value.requirement,
            about: value.about,
        }
    }
}

/// Section 7.2's `contribution_importance` qualifier on `ENABLES_COMPETENCY`.
///
/// The three values are `academic_domain`'s, not this crate's:
/// `enabling_qualifiers_are_the_registry_s` compares
/// [`ContributionImportance::ALL`] against
/// `PredicateName::EnablesCompetency`'s descriptor in both directions, so a
/// fourth value is a change to the predicate registry rather than one this
/// crate may make on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContributionImportance {
    /// `CRITICAL`.
    Critical,
    /// `SUBSTANTIAL`.
    Substantial,
    /// `MINOR`.
    Minor,
}

impl ContributionImportance {
    /// Exhaustive, in the registry's own order.
    pub const ALL: [Self; 3] = [Self::Critical, Self::Substantial, Self::Minor];

    /// The registry's spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::Substantial => "SUBSTANTIAL",
            Self::Minor => "MINOR",
        }
    }
}

/// Section 7.2's `necessity` qualifier on `ENABLES_COMPETENCY`.
///
/// The two values are `academic_domain`'s. See [`ContributionImportance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Necessity {
    /// `NECESSARY`.
    Necessary,
    /// `OPTIONAL`.
    Optional,
}

impl Necessity {
    /// Exhaustive, in the registry's own order.
    pub const ALL: [Self; 2] = [Self::Necessary, Self::Optional];

    /// The registry's spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Necessary => "NECESSARY",
            Self::Optional => "OPTIONAL",
        }
    }
}

/// One entry of section 24.1's `enabledByConcepts`.
///
/// Section 7.2's row for `ENABLES_COMPETENCY` reads
/// `수행에 기여. 중요도와 필요/선택 구분 필요`, and its closed qualifier schema
/// requires both keys, so both are fields here and neither has a default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnablingConcept {
    concept: ConceptRef,
    importance: ContributionImportance,
    necessity: Necessity,
}

impl EnablingConcept {
    /// Records one enabling concept with both required qualifiers.
    #[must_use]
    pub const fn of(
        concept: ConceptRef,
        importance: ContributionImportance,
        necessity: Necessity,
    ) -> Self {
        Self {
            concept,
            importance,
            necessity,
        }
    }

    /// Which concept.
    #[must_use]
    pub const fn concept(&self) -> &ConceptRef {
        &self.concept
    }

    /// How much it contributes.
    #[must_use]
    pub const fn importance(&self) -> ContributionImportance {
        self.importance
    }

    /// Whether it is necessary or optional.
    #[must_use]
    pub const fn necessity(&self) -> Necessity {
        self.necessity
    }
}
