//! The identifiers, and the two that never become one another.
//!
//! Section 24.1's whole first sentence is that a concept and a competency are
//! different objects: `Concept는 설명·관계·원리를 가진 지식 단위다. Competency는
//! 조건, 과제, 품질 기준이 있는 수행 능력이다.` So [`CompetencyId`] and
//! [`ConceptRef`] are two types with no conversion between them in either
//! direction — no `From`, no `Into`, no shared `AsRef<str>`, and no constructor
//! of one that takes the other. `crates/competency/tests/compile_fail/` holds
//! the compiled half.
//!
//! ## A concept is named in a namespace, and the namespace is part of the value
//!
//! Two boundaries below this one name concepts, and they name them
//! differently. `P2-N1`'s ontology issues an [`EntityId`], which is what
//! `P2-N2`'s admitted evidence carries. `P2-R4`'s classification key carries a
//! validated token, which is what `P2-R5`'s personal claim carries. A criterion
//! that matched a token against an identifier — or either against a label —
//! would join evidence to a criterion that is about something else.
//!
//! [`ConceptRef`] therefore carries the namespace with the value and compares
//! the pair, the way `P2-R5`'s `ExternalAuthorId` carries its `IdentitySource`.
//! A classification token spelled like an ontology identifier resolves to
//! nothing on the ontology side, because there is no arm that reads one as the
//! other.

use academic_domain::EntityId;
use serde::{Deserialize, Serialize};

use crate::CompetencyError;

/// Longest identifier this crate admits, in bytes.
const MAX_IDENTIFIER: usize = 64;

/// Checks one identifier against `[A-Za-z0-9._-]` within [`MAX_IDENTIFIER`].
///
/// The same shape `P2-R5` admits, so a `P2-R4` classification token stays a
/// legal value on the way through this crate rather than being reshaped here.
pub(crate) fn validated(value: String, what: &'static str) -> Result<String, CompetencyError> {
    let legal = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if legal {
        Ok(value)
    } else {
        Err(CompetencyError::InvalidIdentifier(what, value))
    }
}

/// Checks one prose field for emptiness.
///
/// Section 24.1's `context`, `performanceCriteria` rows and `evidenceRubric`
/// rows are prose the user writes, so they are not identifiers. What is refused
/// is the empty one, because a criterion nobody wrote is a criterion nobody can
/// check.
pub(crate) fn non_empty(value: String, what: &'static str) -> Result<String, CompetencyError> {
    if value.trim().is_empty() {
        Err(CompetencyError::EmptyText(what))
    } else {
        Ok(value)
    }
}

/// One competency's identity.
///
/// Section 24.1's `id`. A separate type from [`ConceptRef`], with no conversion
/// either way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CompetencyId(String);

impl CompetencyId {
    /// Checks and takes one identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is not `[A-Za-z0-9._-]`
    /// within 64 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self(validated(value.into(), "competency")?))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CompetencyId {
    type Error = CompetencyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CompetencyId> for String {
    fn from(value: CompetencyId) -> Self {
        value.0
    }
}

/// One performance criterion's identity, inside its competency.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CriterionId(String);

impl CriterionId {
    /// Checks and takes one identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is not `[A-Za-z0-9._-]`
    /// within 64 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self(validated(value.into(), "criterion")?))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CriterionId {
    type Error = CompetencyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CriterionId> for String {
    fn from(value: CriterionId) -> Self {
        value.0
    }
}

/// One piece of stage evidence, by identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RecordId(String);

impl RecordId {
    /// Checks and takes one identifier.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when it is not `[A-Za-z0-9._-]`
    /// within 64 bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self(validated(value.into(), "record")?))
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Which namespace named a concept.
///
/// Two, because two boundaries below this one name concepts and they do not
/// share a spelling. A third would be a new arm here rather than a value that
/// quietly reads as one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConceptNamespace {
    /// `P2-N1`'s entity registry, whose identity `P2-N2`'s admitted evidence
    /// carries.
    Ontology,
    /// `P2-R4`'s classification key, whose concept token `P2-R5`'s personal
    /// claim carries.
    Classification,
}

impl ConceptNamespace {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Ontology, Self::Classification];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ontology => "ONTOLOGY",
            Self::Classification => "CLASSIFICATION",
        }
    }
}

/// A concept, named in the namespace that named it.
///
/// **Not a [`CompetencyId`], and there is no route between them.** Section
/// 24.1: a concept is a knowledge unit and a competency is a performance under
/// conditions, so the identity of one is never the identity of the other.
///
/// Equality is over the pair. `Ontology(id)` never equals
/// `Classification(token)`, whatever the token spells, because the namespace is
/// a field of the value rather than context a caller is trusted to carry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "namespace", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConceptRef {
    /// The ontology's own identity.
    Ontology(EntityId),
    /// `P2-R4`'s classification concept token.
    Classification(String),
}

impl ConceptRef {
    /// Names a concept by `P2-N1`'s identity.
    #[must_use]
    pub const fn ontology(id: EntityId) -> Self {
        Self::Ontology(id)
    }

    /// Names a concept by `P2-R4`'s classification token.
    ///
    /// # Errors
    ///
    /// [`CompetencyError::InvalidIdentifier`] when the token is not
    /// `[A-Za-z0-9._-]` within 64 bytes, which is the shape `P2-R4` issues.
    pub fn classification(token: impl Into<String>) -> Result<Self, CompetencyError> {
        Ok(Self::Classification(validated(token.into(), "concept")?))
    }

    /// Which namespace named it.
    #[must_use]
    pub const fn namespace(&self) -> ConceptNamespace {
        match self {
            Self::Ontology(_) => ConceptNamespace::Ontology,
            Self::Classification(_) => ConceptNamespace::Classification,
        }
    }
}
