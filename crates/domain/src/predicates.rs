//! The versioned predicate registry for the twenty §7.2 graph edges.
//!
//! [`schemas/registry/predicate-registry-v1.json`] is the single source of
//! truth. [`generated`] is rendered from it by `tools/predicate-registry.mjs`
//! and is compared byte-for-byte against a fresh render by
//! `pnpm verify:contracts`, so a hand edit to either side fails the build.
//!
//! What this module fixes:
//!
//! - **Direction.** Every edge declares the node types admitted on each end.
//!   An assertion whose ends do not match is rejected, so the reverse of an
//!   asymmetric edge is not constructible.
//! - **Inverse is a view.** Each predicate carries a human-readable
//!   `inverse_label` and no inverse predicate exists. Reverse traversal is
//!   [`inverse_neighbours`], a read over the stored forward rows; storing a
//!   reverse row would put the same fact in the append-only ledger twice.
//! - **`RELATED_TO` is undirected.** Its endpoints are canonically ordered
//!   smaller identifier first, so a pair asserted either way is one row, and it
//!   is not a prerequisite predicate: [`prerequisite_descriptor`] refuses it.
//! - **Strength.** `HARD`/`STRONG`/`HELPFUL` is carried only by the two
//!   prerequisite predicates, and the admitted set differs between them.
//! - **Minimum evidence.** Per predicate, and per strength where strength
//!   changes it. A `REQUIRES` edge at `HARD` needs two independent sources; one
//!   source is rejected.
//! - **Qualifiers.** Each predicate's qualifier schema is closed: an unknown
//!   key, a missing required key, a duplicate key, and a value outside the
//!   declared domain are all rejected. Every qualifier value is typed, so no
//!   structured value is smuggled through free text.
//!
//! The base taxonomy mix stays open (`GATE-38-022`): this registry names
//! predicates only and seeds no concept, field, or competency.

mod generated;

pub use generated::{
    NodeType, OPEN_GATES, PREDICATE_REGISTRY, PREDICATE_REGISTRY_VERSION, PredicateName,
};

use thiserror::Error;

use crate::{ArtifactId, AuthorityClass, EntityId, EvidenceItem, EvidenceLocator, EvidenceRole};
use crate::{EvidenceStrength, MasteryLevel};

/// Whether an edge's ends are ordered by meaning or by identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeDirection {
    /// Subject and object mean different things; reversing changes the claim.
    Directed,
    /// The pair is symmetric and stored smaller identifier first.
    UndirectedCanonical,
}

/// How many objects one subject may carry, and the reverse.
///
/// Every §7.2 edge is many-to-many. §7.2 constrains no edge to a functional
/// arity, and §7.3 makes every edge a scoped, valid-timed claim, so a functional
/// constraint here would forbid re-assertion under a second scope or interval.
/// The field exists so that narrowing an edge is a registry change with a
/// version bump rather than an undeclared rule inside a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cardinality {
    /// Many subjects to many objects.
    ManyToMany,
}

/// Dependency strength on the two prerequisite predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrerequisiteStrength {
    /// Useful ordering, no blocking claim.
    Helpful,
    /// Near-hard: the goal is unreliable without it.
    Strong,
    /// The goal is reliably blocked without it.
    Hard,
}

/// Which shape of evidence locator an item uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceLocatorKind {
    /// A page of a paginated document.
    Page,
    /// A byte range inside a text source.
    TextBytes,
    /// A time range inside a transcript.
    TranscriptTime,
    /// A byte range inside a repository snapshot.
    RepositoryBytes,
}

impl From<&EvidenceLocator> for EvidenceLocatorKind {
    fn from(locator: &EvidenceLocator) -> Self {
        match locator {
            EvidenceLocator::Page { .. } => Self::Page,
            EvidenceLocator::TextBytes { .. } => Self::TextBytes,
            EvidenceLocator::TranscriptTime { .. } => Self::TranscriptTime,
            EvidenceLocator::RepositoryBytes { .. } => Self::RepositoryBytes,
        }
    }
}

/// The evidence an edge must carry before it may be asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimumEvidence {
    /// Supporting items that qualify under the rules below.
    pub supporting: u8,
    /// Distinct artifacts among those qualifying items.
    pub independent_sources: u8,
    /// Weakest item strength that still counts.
    pub min_strength: EvidenceStrength,
    /// Authority classes admitted for the assertion; empty admits any.
    pub authority: &'static [AuthorityClass],
    /// Locator kinds a qualifying item may use; empty admits any.
    pub locator_kinds: &'static [EvidenceLocatorKind],
}

/// A predicate's evidence rule, including the strengths that change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceRule {
    /// Applies when no override matches the asserted strength.
    pub base: MinimumEvidence,
    /// Overrides keyed by prerequisite strength.
    pub by_strength: &'static [(PrerequisiteStrength, MinimumEvidence)],
}

impl EvidenceRule {
    /// Returns the rule in force for an assertion at `strength`.
    #[must_use]
    pub fn effective(&self, strength: Option<PrerequisiteStrength>) -> MinimumEvidence {
        strength
            .and_then(|asserted| {
                self.by_strength
                    .iter()
                    .find(|(candidate, _)| *candidate == asserted)
                    .map(|(_, rule)| *rule)
            })
            .unwrap_or(self.base)
    }
}

/// The value domain of one qualifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualifierKind {
    /// One of the predicate's admitted prerequisite strengths.
    PrerequisiteStrength,
    /// One of a closed set of names.
    Enumeration(&'static [&'static str]),
    /// A canonical entity identifier, never free text.
    EntityReference,
    /// An integer greater than zero.
    PositiveInteger,
}

/// One entry of a predicate's closed qualifier schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifierSchema {
    /// Stable qualifier key.
    pub key: &'static str,
    /// Value domain the key admits.
    pub kind: QualifierKind,
    /// Whether an assertion without the key is rejected.
    pub required: bool,
}

/// Everything the registry fixes about one §7.2 edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredicateDescriptor {
    /// Registry name, identical to the §7.2 table's first column.
    pub name: PredicateName,
    /// Stable claim predicate identifier for this edge.
    pub predicate_id: &'static str,
    /// Registry version that introduced the predicate.
    pub since_registry_version: u16,
    /// The §7.2 direction cell, verbatim.
    pub spec_direction: &'static str,
    /// The §7.2 meaning cell, verbatim.
    pub spec_meaning: &'static str,
    /// Whether reversing the ends changes the claim.
    pub direction: EdgeDirection,
    /// Declared arity.
    pub cardinality: Cardinality,
    /// Node types admitted as the subject.
    pub subject_types: &'static [NodeType],
    /// Node types admitted as the object.
    pub object_types: &'static [NodeType],
    /// Whether a path engine may traverse this edge as a prerequisite.
    pub prerequisite: bool,
    /// Prerequisite strengths this edge admits; empty means it carries none.
    pub strengths: &'static [PrerequisiteStrength],
    /// Highest mastery this edge alone may support, or `None` when asserting it
    /// licenses no personal state claim at all.
    pub personal_state_ceiling: Option<MasteryLevel>,
    /// How the reverse reading is labelled. No reverse row is ever stored.
    pub inverse_label: &'static str,
    /// Closed qualifier schema.
    pub qualifiers: &'static [QualifierSchema],
    /// Evidence rule, including per-strength overrides.
    pub minimum_evidence: EvidenceRule,
}

/// Failures raised while admitting a graph assertion against the registry.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The subject's node type is not on the predicate's admitted subject side.
    #[error("{predicate} does not admit {subject:?} as a subject")]
    SubjectTypeNotAdmitted {
        /// Predicate whose direction was violated.
        predicate: &'static str,
        /// Node type that was offered.
        subject: NodeType,
    },
    /// The object's node type is not on the predicate's admitted object side.
    #[error("{predicate} does not admit {object:?} as an object")]
    ObjectTypeNotAdmitted {
        /// Predicate whose direction was violated.
        predicate: &'static str,
        /// Node type that was offered.
        object: NodeType,
    },
    /// An edge related an entity to itself.
    #[error("{0} must not relate an entity to itself")]
    SelfEdge(&'static str),
    /// A required qualifier was absent.
    #[error("{predicate} requires the qualifier {key}")]
    MissingQualifier {
        /// Predicate whose schema was violated.
        predicate: &'static str,
        /// Qualifier key that was absent.
        key: &'static str,
    },
    /// A qualifier key is not in the predicate's closed schema.
    #[error("{predicate} declares no qualifier {key}")]
    UnknownQualifier {
        /// Predicate whose schema was violated.
        predicate: &'static str,
        /// Qualifier key that was offered.
        key: String,
    },
    /// The same qualifier key was supplied twice.
    #[error("qualifier {0} was supplied more than once")]
    DuplicateQualifier(String),
    /// A qualifier value did not match its declared domain.
    #[error("qualifier {key} does not admit the supplied value")]
    QualifierValueNotAdmitted {
        /// Qualifier key whose domain was violated.
        key: &'static str,
    },
    /// The asserted prerequisite strength is not admitted by the predicate.
    #[error("{predicate} does not admit prerequisite strength {strength:?}")]
    StrengthNotAdmitted {
        /// Predicate whose strength split was violated.
        predicate: &'static str,
        /// Strength that was offered.
        strength: PrerequisiteStrength,
    },
    /// A path engine asked to traverse an edge that is not a prerequisite.
    #[error("{0} is not a prerequisite predicate")]
    NotAPrerequisitePredicate(&'static str),
    /// A personal state derivation asked for an edge that licenses none.
    #[error("{0} licenses no personal state claim")]
    NotPersonalStateBearing(&'static str),
    /// Too few qualifying supporting evidence items.
    #[error("{predicate} needs {required} supporting evidence items, got {actual}")]
    InsufficientEvidence {
        /// Predicate whose evidence rule was violated.
        predicate: &'static str,
        /// Items the rule demands.
        required: u8,
        /// Items that qualified.
        actual: u8,
    },
    /// Qualifying evidence came from too few distinct artifacts.
    #[error("{predicate} needs {required} independent sources, got {actual}")]
    InsufficientIndependentSources {
        /// Predicate whose evidence rule was violated.
        predicate: &'static str,
        /// Distinct artifacts the rule demands.
        required: u8,
        /// Distinct artifacts that qualified.
        actual: u8,
    },
    /// The assertion's authority class is not admitted by the predicate.
    #[error("{predicate} does not admit authority class {authority:?}")]
    AuthorityNotPermitted {
        /// Predicate whose evidence rule was violated.
        predicate: &'static str,
        /// Authority class that was offered.
        authority: AuthorityClass,
    },
}

/// One evidence item as the registry reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeEvidence {
    /// Artifact the item points into; independence is counted over this.
    pub artifact_id: ArtifactId,
    /// Whether the item supports, contradicts, or only contextualises.
    pub role: EvidenceRole,
    /// Item strength, unrelated to the assertion's authority class.
    pub strength: EvidenceStrength,
    /// Shape of the item's locator.
    pub locator_kind: EvidenceLocatorKind,
}

impl EdgeEvidence {
    /// Reads a canonical evidence item as registry evidence.
    #[must_use]
    pub fn from_item(item: &EvidenceItem) -> Self {
        Self {
            artifact_id: item.artifact_id,
            role: item.role,
            strength: item.strength,
            locator_kind: EvidenceLocatorKind::from(&item.locator),
        }
    }
}

/// A typed qualifier value. There is deliberately no free-text variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualifierValue {
    /// A prerequisite strength.
    Strength(PrerequisiteStrength),
    /// A name from a closed enumeration.
    Enumerated(String),
    /// A canonical entity identifier.
    Entity(EntityId),
    /// A positive integer.
    Integer(u32),
}

/// One supplied qualifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualifier {
    /// Qualifier key, checked against the predicate's closed schema.
    pub key: String,
    /// Typed value.
    pub value: QualifierValue,
}

/// The stored identity of one edge row.
///
/// An undirected predicate is normalised here, so a pair asserted in either
/// order produces one key and therefore one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeKey {
    predicate: PredicateName,
    subject: EntityId,
    object: EntityId,
}

impl EdgeKey {
    /// Builds the stored key, canonicalising an undirected pair.
    pub fn new(
        predicate: PredicateName,
        subject: EntityId,
        object: EntityId,
    ) -> Result<Self, RegistryError> {
        if subject == object {
            return Err(RegistryError::SelfEdge(predicate.as_str()));
        }
        let (subject, object) = match predicate.descriptor().direction {
            EdgeDirection::Directed => (subject, object),
            EdgeDirection::UndirectedCanonical if subject < object => (subject, object),
            EdgeDirection::UndirectedCanonical => (object, subject),
        };
        Ok(Self {
            predicate,
            subject,
            object,
        })
    }

    /// Returns the predicate this row asserts.
    #[must_use]
    pub const fn predicate(&self) -> PredicateName {
        self.predicate
    }

    /// Returns the stored subject end.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// Returns the stored object end.
    #[must_use]
    pub const fn object(&self) -> EntityId {
        self.object
    }
}

/// A graph assertion offered for admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeAssertion<'a> {
    /// Stored identity of the edge.
    pub key: EdgeKey,
    /// Node type of the subject end.
    pub subject_type: NodeType,
    /// Node type of the object end.
    pub object_type: NodeType,
    /// Authority class of the assertion itself, not of any single item.
    pub authority_class: AuthorityClass,
    /// Supplied qualifiers.
    pub qualifiers: &'a [Qualifier],
    /// Supplied evidence.
    pub evidence: &'a [EdgeEvidence],
}

impl EdgeAssertion<'_> {
    /// Admits the assertion against its predicate's registry entry.
    pub fn validate(&self) -> Result<(), RegistryError> {
        let descriptor = self.key.predicate.descriptor();
        let name = descriptor.name.as_str();

        if !descriptor.subject_types.contains(&self.subject_type) {
            return Err(RegistryError::SubjectTypeNotAdmitted {
                predicate: name,
                subject: self.subject_type,
            });
        }
        if !descriptor.object_types.contains(&self.object_type) {
            return Err(RegistryError::ObjectTypeNotAdmitted {
                predicate: name,
                object: self.object_type,
            });
        }

        let strength = self.check_qualifiers(descriptor)?;
        let rule = descriptor.minimum_evidence.effective(strength);

        if !rule.authority.is_empty() && !rule.authority.contains(&self.authority_class) {
            return Err(RegistryError::AuthorityNotPermitted {
                predicate: name,
                authority: self.authority_class,
            });
        }

        let qualifying: Vec<ArtifactId> = self
            .evidence
            .iter()
            .filter(|item| {
                item.role == EvidenceRole::Supports
                    && strength_rank(item.strength) >= strength_rank(rule.min_strength)
                    && (rule.locator_kinds.is_empty()
                        || rule.locator_kinds.contains(&item.locator_kind))
            })
            .map(|item| item.artifact_id)
            .collect();

        let actual = saturating_len(qualifying.len());
        if actual < rule.supporting {
            return Err(RegistryError::InsufficientEvidence {
                predicate: name,
                required: rule.supporting,
                actual,
            });
        }

        let mut distinct = qualifying;
        distinct.sort_unstable();
        distinct.dedup();
        let sources = saturating_len(distinct.len());
        if sources < rule.independent_sources {
            return Err(RegistryError::InsufficientIndependentSources {
                predicate: name,
                required: rule.independent_sources,
                actual: sources,
            });
        }

        Ok(())
    }

    /// Checks the closed qualifier schema and returns the asserted strength.
    fn check_qualifiers(
        &self,
        descriptor: &'static PredicateDescriptor,
    ) -> Result<Option<PrerequisiteStrength>, RegistryError> {
        let name = descriptor.name.as_str();
        let mut strength = None;

        for (index, supplied) in self.qualifiers.iter().enumerate() {
            if self.qualifiers[..index]
                .iter()
                .any(|earlier| earlier.key == supplied.key)
            {
                return Err(RegistryError::DuplicateQualifier(supplied.key.clone()));
            }
            let schema = descriptor
                .qualifiers
                .iter()
                .find(|schema| schema.key == supplied.key)
                .ok_or_else(|| RegistryError::UnknownQualifier {
                    predicate: name,
                    key: supplied.key.clone(),
                })?;
            match (schema.kind, &supplied.value) {
                (QualifierKind::PrerequisiteStrength, QualifierValue::Strength(asserted)) => {
                    if !descriptor.strengths.contains(asserted) {
                        return Err(RegistryError::StrengthNotAdmitted {
                            predicate: name,
                            strength: *asserted,
                        });
                    }
                    strength = Some(*asserted);
                }
                (QualifierKind::Enumeration(values), QualifierValue::Enumerated(value)) => {
                    if !values.contains(&value.as_str()) {
                        return Err(RegistryError::QualifierValueNotAdmitted { key: schema.key });
                    }
                }
                (QualifierKind::EntityReference, QualifierValue::Entity(_)) => {}
                (QualifierKind::PositiveInteger, QualifierValue::Integer(value)) if *value > 0 => {}
                _ => return Err(RegistryError::QualifierValueNotAdmitted { key: schema.key }),
            }
        }

        for schema in descriptor.qualifiers {
            if schema.required
                && !self
                    .qualifiers
                    .iter()
                    .any(|supplied| supplied.key == schema.key)
            {
                return Err(RegistryError::MissingQualifier {
                    predicate: name,
                    key: schema.key,
                });
            }
        }

        Ok(strength)
    }
}

/// Returns the descriptor of a predicate a path engine may traverse.
///
/// `RELATED_TO` is an undirected association and is refused here, which is the
/// §7.2 prohibition on using it as a prerequisite.
pub fn prerequisite_descriptor(
    predicate: PredicateName,
) -> Result<&'static PredicateDescriptor, RegistryError> {
    let descriptor = predicate.descriptor();
    if descriptor.prerequisite {
        Ok(descriptor)
    } else {
        Err(RegistryError::NotAPrerequisitePredicate(
            descriptor.name.as_str(),
        ))
    }
}

/// Returns the highest mastery an edge of this predicate alone may support.
///
/// A predicate that licenses no personal state claim is refused rather than
/// answered with a floor value, so no caller can read a personal state out of
/// an edge that only describes the world.
pub fn personal_mastery_ceiling(predicate: PredicateName) -> Result<MasteryLevel, RegistryError> {
    predicate
        .descriptor()
        .personal_state_ceiling
        .ok_or(RegistryError::NotPersonalStateBearing(predicate.as_str()))
}

/// Whether an edge of this predicate alone may support `level`.
#[must_use]
pub fn supports_mastery(predicate: PredicateName, level: MasteryLevel) -> bool {
    personal_mastery_ceiling(predicate).is_ok_and(|ceiling| level <= ceiling)
}

/// Reads the inverse direction of `predicate` as a view over stored rows.
///
/// No inverse predicate exists and no reverse row is stored, so this is the
/// only inverse read path. An undirected predicate matches either end.
#[must_use]
pub fn inverse_neighbours(
    edges: &[EdgeKey],
    predicate: PredicateName,
    object: EntityId,
) -> Vec<EntityId> {
    let undirected = predicate.descriptor().direction == EdgeDirection::UndirectedCanonical;
    edges
        .iter()
        .filter(|edge| edge.predicate == predicate)
        .filter_map(|edge| {
            if edge.object == object {
                Some(edge.subject)
            } else if undirected && edge.subject == object {
                Some(edge.object)
            } else {
                None
            }
        })
        .collect()
}

/// Ranks evidence strength so a minimum can be expressed without ordering the
/// public enum, whose variants carry no numeric meaning of their own.
const fn strength_rank(strength: EvidenceStrength) -> u8 {
    match strength {
        EvidenceStrength::Weak => 0,
        EvidenceStrength::Corroborating => 1,
        EvidenceStrength::Direct => 2,
    }
}

/// Clamps a count into the `u8` the registry rules are expressed in.
fn saturating_len(count: usize) -> u8 {
    u8::try_from(count).unwrap_or(u8::MAX)
}
