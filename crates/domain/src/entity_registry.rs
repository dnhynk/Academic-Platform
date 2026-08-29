//! Canonical entity registry: stable identity, aliases, senses, non-destructive
//! merge, queue-only split, and the migration-equivalence contract.
//!
//! # Where the registry's bytes live
//!
//! The registry invents no storage. Every fact it holds is read back from two
//! canonical sources that already exist:
//!
//! - the migration 0004 `entity_identity_change` closure row, which anchors one
//!   `ENTITY_IDENTITY_CHANGED` event to the entity whose identity it changes,
//!   its domain, its scope, and the interval the change is effective over; and
//! - `CLAIM_ASSERTED` claims carrying the typed detail, because the eighteen
//!   event schema v3 arms are registration depth and their payload is exactly
//!   `id / parent / domain_id / scope_id / source_digest / valid_time`.
//!
//! An identity change therefore has no unsigned half: the anchor is a signed
//! registration arm and the detail is a signed typed claim. [`RegistryFact`] is
//! the total, round-trippable mapping between the two representations, and
//! [`EntityRegistry::build`] refuses any fact whose subject carries no anchor,
//! which is what makes the 0004 row load-bearing rather than decorative.
//!
//! # What the registry refuses to do
//!
//! Merge leaves a redirect and rewrites nothing: the merged-away identifier
//! still resolves, and every evidence link that named it still names it.
//! Split moves no evidence at all; it enqueues each affected evidence item for
//! reclassification and leaves the item attached where it was. Both are
//! consequences of CONTRIBUTING rule 2, so neither needs a policy switch.
//!
//! # Why comparison needs an equivalence class
//!
//! After an ontology change, "the same concept" is a claim that has to be
//! earned. [`EquivalenceClass`] names the four ways a pre-change node can
//! relate to a post-change node, and [`StateComparison`] can only carry a delta
//! when its class permits one, so a caller cannot obtain a number across a
//! change without also holding the class that licenses it.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    Claim, ClaimObject, ContentDigest, DomainId, EntityId, EntityIdentityChangeId, EpistemicStatus,
    EvidenceId, MasteryLevel, PredicateId, ScopeId, TimestampMillis, ValidInterval,
};

/// Predicate naming the entity kind a registered identity belongs to.
pub const PREDICATE_ENTITY_KIND: &str = "identity.entity.kind";
/// Predicate naming an entity's canonical label text.
pub const PREDICATE_ENTITY_LABEL: &str = "identity.entity.label";
/// Predicate naming the language of an entity's canonical label.
pub const PREDICATE_ENTITY_LABEL_LANGUAGE: &str = "identity.entity.label.language";
/// Predicate binding a `CONCEPT_SENSE` to the ambiguous concept it disambiguates.
pub const PREDICATE_SENSE_OF: &str = "identity.sense.of";
/// Predicate binding an alias entity to the entity it names.
pub const PREDICATE_ALIAS_OF: &str = "identity.alias.of";
/// Predicate carrying an alias entity's surface text.
pub const PREDICATE_ALIAS_TEXT: &str = "identity.alias.text";
/// Predicate carrying an alias entity's language tag.
pub const PREDICATE_ALIAS_LANGUAGE: &str = "identity.alias.language";
/// Predicate carrying an alias entity's kind discriminant.
pub const PREDICATE_ALIAS_KIND: &str = "identity.alias.kind";
/// Predicate carrying the release an alias is specific to, when it is.
pub const PREDICATE_ALIAS_VERSION: &str = "identity.alias.version";
/// Predicate redirecting a merged-away identity to its surviving identity.
pub const PREDICATE_MERGED_INTO: &str = "identity.merged.into";
/// Predicate naming one successor a split produced.
pub const PREDICATE_SPLIT_INTO: &str = "identity.split.into";
/// Predicate enqueuing one evidence item for post-split reclassification.
pub const PREDICATE_RECLASSIFICATION_PENDING: &str = "identity.reclassification.pending";

/// Every predicate the registry reads, in the order it is documented above.
///
/// A claim whose predicate is absent from this list is not a registry fact and
/// [`RegistryFact::decode`] leaves it alone, so the registry never captures a
/// claim that another aggregate owns.
pub const REGISTRY_PREDICATES: [&str; 12] = [
    PREDICATE_ENTITY_KIND,
    PREDICATE_ENTITY_LABEL,
    PREDICATE_ENTITY_LABEL_LANGUAGE,
    PREDICATE_SENSE_OF,
    PREDICATE_ALIAS_OF,
    PREDICATE_ALIAS_TEXT,
    PREDICATE_ALIAS_LANGUAGE,
    PREDICATE_ALIAS_KIND,
    PREDICATE_ALIAS_VERSION,
    PREDICATE_MERGED_INTO,
    PREDICATE_SPLIT_INTO,
    PREDICATE_RECLASSIFICATION_PENDING,
];

/// Failure to read a well-formed registry out of canonical rows.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// A registry claim named a subject with no `entity_identity_change` anchor.
    #[error("registry fact for {subject} has no ENTITY_IDENTITY_CHANGED anchor")]
    UnanchoredSubject {
        /// The subject entity the rejected claim named.
        subject: EntityId,
    },
    /// A registry predicate carried a claim object of the wrong typed kind.
    #[error("predicate {predicate} does not accept a {found} object")]
    ObjectKindMismatch {
        /// The registry predicate that was read.
        predicate: &'static str,
        /// The `object_kind` discriminant that was found instead.
        found: &'static str,
    },
    /// A closed registry vocabulary received a value outside it.
    #[error("{vocabulary} has no member named {value}")]
    UnknownVocabularyMember {
        /// Which closed vocabulary rejected the value.
        vocabulary: &'static str,
        /// The rejected value, retained so the caller can report it.
        value: String,
    },
    /// A merge or split was asserted at an authority the registry cannot accept.
    #[error("{action} requires a USER_CONFIRMED claim, found {found:?}")]
    UnapprovedIdentityChange {
        /// The identity action that was refused.
        action: &'static str,
        /// The epistemic status the rejected claim carried.
        found: EpistemicStatus,
    },
    /// An entity was merged into itself, or split into itself.
    #[error("{action} names {entity} as its own successor")]
    SelfSuccessor {
        /// The identity action that was refused.
        action: &'static str,
        /// The entity that appeared on both sides.
        entity: EntityId,
    },
    /// A split named fewer than two successors, which is a merge or a no-op.
    ///
    /// The field is `source_entity` rather than `source` because `thiserror`
    /// reads a field named `source` as a nested error rather than as data.
    #[error("split of {source_entity} named {found} successors; a split needs at least two")]
    DegenerateSplit {
        /// The entity whose split was refused.
        source_entity: EntityId,
        /// How many successors the rejected split named.
        found: usize,
    },
    /// An approval cited a preview digest that the recomputed preview does not match.
    #[error("impact preview digest mismatch: approval cited {cited}, preview computes {computed}")]
    PreviewDigestMismatch {
        /// The digest the approval cited as evidence.
        cited: ContentDigest,
        /// The digest the recomputed preview produces.
        computed: ContentDigest,
    },
}

/// Ontology tier an identity occupies.
///
/// The tiers are the §7.4 granularity policy's own vocabulary. A `CONCEPT_SENSE`
/// exists only to disambiguate a homonym and always names the ambiguous
/// `CONCEPT` it was separated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityKind {
    /// A broad area that carries no independent prerequisite of its own.
    Field,
    /// An independently explainable, questionable, evidence-bearing unit.
    Concept,
    /// One disambiguated reading of an otherwise ambiguous concept label.
    ConceptSense,
    /// A named procedure beneath a concept.
    Operation,
    /// A surface form that names another entity; never carries evidence itself.
    Alias,
}

impl EntityKind {
    /// Returns the wire discriminant this kind is written as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Field => "FIELD",
            Self::Concept => "CONCEPT",
            Self::ConceptSense => "CONCEPT_SENSE",
            Self::Operation => "OPERATION",
            Self::Alias => "ALIAS",
        }
    }

    /// Parses the wire discriminant, refusing anything outside the closed set.
    pub fn parse(value: &str) -> Result<Self, RegistryError> {
        match value {
            "FIELD" => Ok(Self::Field),
            "CONCEPT" => Ok(Self::Concept),
            "CONCEPT_SENSE" => Ok(Self::ConceptSense),
            "OPERATION" => Ok(Self::Operation),
            "ALIAS" => Ok(Self::Alias),
            other => Err(RegistryError::UnknownVocabularyMember {
                vocabulary: "entity kind",
                value: other.to_owned(),
            }),
        }
    }
}

/// Why a surface form exists, kept separate from the language it is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AliasKind {
    /// The form a reader is shown by default in that language.
    Preferred,
    /// An initialism or shortened form.
    Abbreviation,
    /// The same term rendered in another language.
    Translation,
    /// A name that only applies to a named release or edition.
    Versioned,
}

impl AliasKind {
    /// Returns the wire discriminant this kind is written as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preferred => "PREFERRED",
            Self::Abbreviation => "ABBREVIATION",
            Self::Translation => "TRANSLATION",
            Self::Versioned => "VERSIONED",
        }
    }

    /// Parses the wire discriminant, refusing anything outside the closed set.
    pub fn parse(value: &str) -> Result<Self, RegistryError> {
        match value {
            "PREFERRED" => Ok(Self::Preferred),
            "ABBREVIATION" => Ok(Self::Abbreviation),
            "TRANSLATION" => Ok(Self::Translation),
            "VERSIONED" => Ok(Self::Versioned),
            other => Err(RegistryError::UnknownVocabularyMember {
                vocabulary: "alias kind",
                value: other.to_owned(),
            }),
        }
    }
}

/// One migration 0004 `entity_identity_change` row, read verbatim.
///
/// The registry consumes anchors rather than re-deriving them so the columns it
/// depends on are exactly the columns that migration owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityAnchor {
    /// Primary key of the closure row.
    pub change_id: EntityIdentityChangeId,
    /// Entity whose identity the registering event changed.
    pub entity_id: EntityId,
    /// Security domain the change belongs to.
    pub domain_id: DomainId,
    /// Scope the change is registered under.
    pub scope_id: ScopeId,
    /// Half-open interval the change is effective over.
    pub valid_time: ValidInterval,
}

/// One typed registry fact, decoded from a claim or encoded back into one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryFact {
    /// The subject occupies this ontology tier.
    EntityKindDeclared {
        /// Entity the tier applies to.
        entity_id: EntityId,
        /// The tier itself.
        kind: EntityKind,
    },
    /// The subject's canonical label text.
    LabelDeclared {
        /// Entity the label names.
        entity_id: EntityId,
        /// The label text.
        text: String,
    },
    /// The language of the subject's canonical label.
    LabelLanguageDeclared {
        /// Entity the label belongs to.
        entity_id: EntityId,
        /// BCP-47-shaped language tag as written.
        language: String,
    },
    /// The subject is one reading of an ambiguous concept.
    SenseOf {
        /// The disambiguated sense.
        sense_id: EntityId,
        /// The ambiguous concept the sense was separated from.
        concept_id: EntityId,
    },
    /// The subject alias entity names another entity.
    AliasOf {
        /// The alias entity.
        alias_id: EntityId,
        /// The entity the alias names.
        entity_id: EntityId,
    },
    /// The subject alias entity's surface text.
    AliasText {
        /// The alias entity.
        alias_id: EntityId,
        /// The surface form.
        text: String,
    },
    /// The subject alias entity's language tag.
    AliasLanguage {
        /// The alias entity.
        alias_id: EntityId,
        /// BCP-47-shaped language tag as written.
        language: String,
    },
    /// The subject alias entity's kind.
    AliasKindDeclared {
        /// The alias entity.
        alias_id: EntityId,
        /// Why the surface form exists.
        kind: AliasKind,
    },
    /// The release the subject alias entity is specific to.
    AliasVersion {
        /// The alias entity.
        alias_id: EntityId,
        /// The release name as written.
        version: String,
    },
    /// The subject was merged into a surviving identity, non-destructively.
    MergedInto {
        /// The identity that stops being canonical but keeps resolving.
        source: EntityId,
        /// The surviving identity.
        target: EntityId,
    },
    /// The subject was split, and this names one successor.
    SplitInto {
        /// The identity that was split.
        source: EntityId,
        /// One successor the split produced.
        target: EntityId,
    },
    /// One evidence item awaits reclassification after a split; it has not moved.
    ReclassificationPending {
        /// The split identity the evidence is still attached to.
        source: EntityId,
        /// The evidence item that awaits a decision.
        evidence_id: EvidenceId,
    },
}

impl RegistryFact {
    /// Returns the entity the fact is asserted about.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        match self {
            Self::EntityKindDeclared { entity_id, .. }
            | Self::LabelDeclared { entity_id, .. }
            | Self::LabelLanguageDeclared { entity_id, .. } => *entity_id,
            Self::SenseOf { sense_id, .. } => *sense_id,
            Self::AliasOf { alias_id, .. }
            | Self::AliasText { alias_id, .. }
            | Self::AliasLanguage { alias_id, .. }
            | Self::AliasKindDeclared { alias_id, .. }
            | Self::AliasVersion { alias_id, .. } => *alias_id,
            Self::MergedInto { source, .. }
            | Self::SplitInto { source, .. }
            | Self::ReclassificationPending { source, .. } => *source,
        }
    }

    /// Returns the registry predicate the fact is written under.
    #[must_use]
    pub const fn predicate(&self) -> &'static str {
        match self {
            Self::EntityKindDeclared { .. } => PREDICATE_ENTITY_KIND,
            Self::LabelDeclared { .. } => PREDICATE_ENTITY_LABEL,
            Self::LabelLanguageDeclared { .. } => PREDICATE_ENTITY_LABEL_LANGUAGE,
            Self::SenseOf { .. } => PREDICATE_SENSE_OF,
            Self::AliasOf { .. } => PREDICATE_ALIAS_OF,
            Self::AliasText { .. } => PREDICATE_ALIAS_TEXT,
            Self::AliasLanguage { .. } => PREDICATE_ALIAS_LANGUAGE,
            Self::AliasKindDeclared { .. } => PREDICATE_ALIAS_KIND,
            Self::AliasVersion { .. } => PREDICATE_ALIAS_VERSION,
            Self::MergedInto { .. } => PREDICATE_MERGED_INTO,
            Self::SplitInto { .. } => PREDICATE_SPLIT_INTO,
            Self::ReclassificationPending { .. } => PREDICATE_RECLASSIFICATION_PENDING,
        }
    }

    /// Returns the typed claim object the fact is written as.
    #[must_use]
    pub fn object(&self) -> ClaimObject {
        match self {
            Self::EntityKindDeclared { kind, .. } => ClaimObject::Text(kind.as_str().to_owned()),
            Self::LabelDeclared { text, .. } | Self::AliasText { text, .. } => {
                ClaimObject::Text(text.clone())
            }
            Self::LabelLanguageDeclared { language, .. } | Self::AliasLanguage { language, .. } => {
                ClaimObject::Text(language.clone())
            }
            Self::SenseOf {
                concept_id: entity_id,
                ..
            }
            | Self::AliasOf { entity_id, .. } => ClaimObject::Entity(*entity_id),
            Self::AliasKindDeclared { kind, .. } => ClaimObject::Text(kind.as_str().to_owned()),
            Self::AliasVersion { version, .. } => ClaimObject::Text(version.clone()),
            Self::MergedInto { target, .. } | Self::SplitInto { target, .. } => {
                ClaimObject::Entity(*target)
            }
            Self::ReclassificationPending { evidence_id, .. } => {
                // An evidence identifier is an opaque UUIDv7 exactly as an entity
                // identifier is, and `ClaimObject` has no evidence arm. Writing
                // it as text keeps the closed nine-value `object_kind` enum
                // untouched; the registry parses it back through `EvidenceId`,
                // so a malformed identifier is rejected rather than stored.
                ClaimObject::Text(evidence_id.to_string())
            }
        }
    }

    /// Decodes one claim into a registry fact, or `None` for a foreign predicate.
    ///
    /// Merge and split are refused unless the claim is `USER_CONFIRMED`. The
    /// canonical writer already refuses a `USER_CONFIRMED` claim from any actor
    /// other than the user, so an importer, an engine, or a model run cannot
    /// reach this path at all; the check here is what stops a user-signed batch
    /// from carrying a merge at a weaker status.
    pub fn decode(claim: &Claim) -> Result<Option<Self>, RegistryError> {
        let subject = claim.subject_entity_id;
        let predicate = claim.predicate_id.as_str();
        let fact = match predicate {
            PREDICATE_ENTITY_KIND => Self::EntityKindDeclared {
                entity_id: subject,
                kind: EntityKind::parse(text_object(claim, PREDICATE_ENTITY_KIND)?)?,
            },
            PREDICATE_ENTITY_LABEL => Self::LabelDeclared {
                entity_id: subject,
                text: text_object(claim, PREDICATE_ENTITY_LABEL)?.to_owned(),
            },
            PREDICATE_ENTITY_LABEL_LANGUAGE => Self::LabelLanguageDeclared {
                entity_id: subject,
                language: text_object(claim, PREDICATE_ENTITY_LABEL_LANGUAGE)?.to_owned(),
            },
            PREDICATE_SENSE_OF => Self::SenseOf {
                sense_id: subject,
                concept_id: entity_object(claim, PREDICATE_SENSE_OF)?,
            },
            PREDICATE_ALIAS_OF => Self::AliasOf {
                alias_id: subject,
                entity_id: entity_object(claim, PREDICATE_ALIAS_OF)?,
            },
            PREDICATE_ALIAS_TEXT => Self::AliasText {
                alias_id: subject,
                text: text_object(claim, PREDICATE_ALIAS_TEXT)?.to_owned(),
            },
            PREDICATE_ALIAS_LANGUAGE => Self::AliasLanguage {
                alias_id: subject,
                language: text_object(claim, PREDICATE_ALIAS_LANGUAGE)?.to_owned(),
            },
            PREDICATE_ALIAS_KIND => Self::AliasKindDeclared {
                alias_id: subject,
                kind: AliasKind::parse(text_object(claim, PREDICATE_ALIAS_KIND)?)?,
            },
            PREDICATE_ALIAS_VERSION => Self::AliasVersion {
                alias_id: subject,
                version: text_object(claim, PREDICATE_ALIAS_VERSION)?.to_owned(),
            },
            PREDICATE_MERGED_INTO => {
                require_user_confirmed(claim, "merge")?;
                let target = entity_object(claim, PREDICATE_MERGED_INTO)?;
                if target == subject {
                    return Err(RegistryError::SelfSuccessor {
                        action: "merge",
                        entity: subject,
                    });
                }
                Self::MergedInto {
                    source: subject,
                    target,
                }
            }
            PREDICATE_SPLIT_INTO => {
                require_user_confirmed(claim, "split")?;
                let target = entity_object(claim, PREDICATE_SPLIT_INTO)?;
                if target == subject {
                    return Err(RegistryError::SelfSuccessor {
                        action: "split",
                        entity: subject,
                    });
                }
                Self::SplitInto {
                    source: subject,
                    target,
                }
            }
            PREDICATE_RECLASSIFICATION_PENDING => {
                let written = text_object(claim, PREDICATE_RECLASSIFICATION_PENDING)?;
                Self::ReclassificationPending {
                    source: subject,
                    evidence_id: written.parse::<EvidenceId>().map_err(|_| {
                        RegistryError::UnknownVocabularyMember {
                            vocabulary: "reclassification evidence identifier",
                            value: written.to_owned(),
                        }
                    })?,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(fact))
    }

    /// Returns the parsed predicate identifier this fact is written under.
    ///
    /// The twelve registry predicates are compile-time constants that satisfy
    /// [`PredicateId::parse`]; the fallible signature exists so a future
    /// vocabulary addition cannot silently ship an unparseable predicate.
    pub fn predicate_id(&self) -> Result<PredicateId, crate::DomainError> {
        PredicateId::parse(self.predicate())
    }
}

fn text_object<'claim>(
    claim: &'claim Claim,
    predicate: &'static str,
) -> Result<&'claim str, RegistryError> {
    match &claim.object {
        ClaimObject::Text(value) => Ok(value.as_str()),
        other => Err(RegistryError::ObjectKindMismatch {
            predicate,
            found: object_kind_name(other),
        }),
    }
}

fn entity_object(claim: &Claim, predicate: &'static str) -> Result<EntityId, RegistryError> {
    match &claim.object {
        ClaimObject::Entity(value) => Ok(*value),
        other => Err(RegistryError::ObjectKindMismatch {
            predicate,
            found: object_kind_name(other),
        }),
    }
}

fn require_user_confirmed(claim: &Claim, action: &'static str) -> Result<(), RegistryError> {
    if claim.epistemic_status == EpistemicStatus::UserConfirmed {
        Ok(())
    } else {
        Err(RegistryError::UnapprovedIdentityChange {
            action,
            found: claim.epistemic_status,
        })
    }
}

const fn object_kind_name(object: &ClaimObject) -> &'static str {
    match object {
        ClaimObject::Entity(_) => "ENTITY",
        ClaimObject::Text(_) => "TEXT",
        ClaimObject::Integer(_) => "INTEGER",
        ClaimObject::Boolean(_) => "BOOLEAN",
        ClaimObject::Decimal(_) => "DECIMAL",
        ClaimObject::Instant(_) => "INSTANT",
        ClaimObject::Interval(_) => "INTERVAL",
        ClaimObject::Mastery(_) => "MASTERY",
        ClaimObject::Freshness(_) => "FRESHNESS",
    }
}

/// A surface form that names an entity, with the metadata that makes it findable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Alias {
    /// Identity of the alias itself, so its metadata stays bound together.
    pub alias_id: EntityId,
    /// Entity the alias names.
    pub entity_id: EntityId,
    /// Surface form as written.
    pub text: String,
    /// Language tag the surface form is written in.
    pub language: String,
    /// Why the surface form exists.
    pub kind: AliasKind,
    /// Release the surface form is specific to, when it is specific to one.
    pub version: Option<String>,
}

/// One registered identity and everything the registry knows about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredEntity {
    /// Stable identifier. It outlives every label and every merge.
    pub entity_id: EntityId,
    /// Ontology tier.
    pub kind: EntityKind,
    /// Canonical label text, when one was declared.
    pub label: Option<String>,
    /// Language of the canonical label, when one was declared.
    pub label_language: Option<String>,
    /// The ambiguous concept this identity disambiguates, for a `CONCEPT_SENSE`.
    pub sense_of: Option<EntityId>,
    /// Scope the identity was registered under.
    pub scope_id: ScopeId,
    /// Security domain the identity belongs to.
    pub domain_id: DomainId,
}

/// One evidence item a split left in place and enqueued for review.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReclassificationItem {
    /// The split identity the evidence is still attached to.
    pub source: EntityId,
    /// The evidence item awaiting a decision. It has not been moved.
    pub evidence_id: EvidenceId,
    /// Successors the evidence may be reclassified onto, in identifier order.
    pub candidates: Vec<EntityId>,
}

/// How a pre-change identity relates to a post-change identity.
///
/// The four classes are the whole vocabulary. There is no fifth, and no
/// "probably the same": a comparison that cannot be justified is
/// [`EquivalenceClass::Incomparable`] and reports itself as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EquivalenceClass {
    /// The same identity on both sides, untouched by any identity change.
    Identical,
    /// The post-change identity covers everything the pre-change one covered,
    /// and possibly more, because a merge redirected the earlier identity into it.
    Refined,
    /// The pre-change identity was split, so its state cannot be attributed to
    /// any one successor until the reclassification queue is decided.
    SplitAmbiguous,
    /// No justified correspondence exists in either direction.
    Incomparable,
}

impl EquivalenceClass {
    /// Whether a state delta may be computed across this correspondence.
    ///
    /// Only [`Self::Identical`] and [`Self::Refined`] permit one.
    /// [`Self::SplitAmbiguous`] withholds until reclassification is decided and
    /// [`Self::Incomparable`] never permits one.
    #[must_use]
    pub const fn permits_comparison(self) -> bool {
        matches!(self, Self::Identical | Self::Refined)
    }

    /// Returns the wire discriminant this class is reported as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identical => "IDENTICAL",
            Self::Refined => "REFINED",
            Self::SplitAmbiguous => "SPLIT_AMBIGUOUS",
            Self::Incomparable => "INCOMPARABLE",
        }
    }
}

/// One observation of an entity's mastery at a point in valid time.
///
/// This is the comparison frame the equivalence contract binds, not the full
/// knowledge-state model: facets, freshness, and evidence ceilings belong to the
/// knowledge-state task. What matters here is that a value is attached to an
/// identity, so an ontology change can be shown not to move it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservedState {
    /// Identity the observation is about.
    pub entity_id: EntityId,
    /// Observed mastery depth.
    pub mastery: MasteryLevel,
    /// Valid-time instant the observation applies to.
    pub observed_at: TimestampMillis,
}

/// The result of comparing one identity's state across an ontology change.
///
/// `delta` is `Some` exactly when `equivalence.permits_comparison()`. That
/// invariant is what makes "never silently compares `INCOMPARABLE` nodes"
/// structural: a caller cannot obtain a number without the class beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateComparison {
    /// Pre-change identity, absent when the post-change node has no predecessor.
    pub before: Option<EntityId>,
    /// Post-change identity, absent when the pre-change node has no successor.
    pub after: Option<EntityId>,
    /// The class that licenses, or refuses, the comparison.
    pub equivalence: EquivalenceClass,
    /// Mastery levels on both sides, present only for a permitted comparison.
    pub delta: Option<MasteryDelta>,
}

/// Mastery on both sides of a permitted cross-change comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MasteryDelta {
    /// Mastery observed before the ontology change.
    pub before: MasteryLevel,
    /// Mastery observed after the ontology change.
    pub after: MasteryLevel,
}

impl MasteryDelta {
    /// Whether the post-change observation is strictly deeper than the earlier one.
    #[must_use]
    pub fn is_growth(self) -> bool {
        self.after > self.before
    }
}

/// A growth statement and the nodes it refused to count.
///
/// The excluded lists are part of the output rather than a log line, because a
/// narrative that silently dropped nodes would read as complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrowthNarrative {
    /// Comparisons that carried a delta and were counted.
    pub counted: Vec<StateComparison>,
    /// Nodes with no justified correspondence; never counted, always reported.
    pub excluded_incomparable: Vec<EquivalenceExclusion>,
    /// Nodes whose split has not been reclassified; withheld, not counted.
    pub withheld_split_ambiguous: Vec<EquivalenceExclusion>,
}

impl GrowthNarrative {
    /// Number of counted comparisons whose mastery deepened.
    #[must_use]
    pub fn growth_count(&self) -> usize {
        self.counted
            .iter()
            .filter(|comparison| comparison.delta.is_some_and(MasteryDelta::is_growth))
            .count()
    }
}

/// One node a growth narrative refused to count, and which side it sat on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EquivalenceExclusion {
    /// Pre-change identity, when the excluded node had one.
    pub before: Option<EntityId>,
    /// Post-change identity, when the excluded node had one.
    pub after: Option<EntityId>,
    /// The class that caused the exclusion.
    pub equivalence: EquivalenceClass,
}

/// A proposed ontology change, before anyone has approved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OntologyChangeProposal {
    /// Redirect one identity into another without destroying either.
    Merge {
        /// The identity that will stop being canonical but keep resolving.
        source: EntityId,
        /// The surviving identity.
        target: EntityId,
    },
    /// Separate one identity into several, moving no evidence.
    Split {
        /// The identity being split.
        source: EntityId,
        /// Successors, in the order the proposal names them.
        targets: Vec<EntityId>,
    },
}

impl OntologyChangeProposal {
    /// Returns the identity the change is applied to.
    #[must_use]
    pub const fn source(&self) -> EntityId {
        match self {
            Self::Merge { source, .. } | Self::Split { source, .. } => *source,
        }
    }

    /// Rejects a self-merge and a split with fewer than two distinct successors.
    pub fn validate(&self) -> Result<(), RegistryError> {
        match self {
            Self::Merge { source, target } => {
                if source == target {
                    return Err(RegistryError::SelfSuccessor {
                        action: "merge",
                        entity: *source,
                    });
                }
                Ok(())
            }
            Self::Split { source, targets } => {
                if targets.contains(source) {
                    return Err(RegistryError::SelfSuccessor {
                        action: "split",
                        entity: *source,
                    });
                }
                let distinct = targets.iter().collect::<BTreeSet<_>>().len();
                if distinct < 2 {
                    return Err(RegistryError::DegenerateSplit {
                        source_entity: *source,
                        found: distinct,
                    });
                }
                Ok(())
            }
        }
    }

    /// Returns the wire discriminant used inside the preview's canonical bytes.
    #[must_use]
    pub const fn action(&self) -> &'static str {
        match self {
            Self::Merge { .. } => "MERGE",
            Self::Split { .. } => "SPLIT",
        }
    }
}

/// What the graph currently holds, supplied by whoever owns each projection.
///
/// The registry counts impact; it does not own states, edges, or questions, so
/// it reads their per-entity counts instead of inventing storage for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OntologyImpactSnapshot {
    /// Knowledge states attached to each entity.
    pub states: BTreeMap<EntityId, u64>,
    /// Graph edges incident to each entity.
    pub edges: BTreeMap<EntityId, u64>,
    /// Open questions attached to each entity.
    pub questions: BTreeMap<EntityId, u64>,
    /// Evidence items attached to each entity.
    pub evidence: BTreeMap<EntityId, BTreeSet<EvidenceId>>,
}

impl OntologyImpactSnapshot {
    /// Evidence attached to one entity, or an empty set.
    #[must_use]
    pub fn evidence_for(&self, entity_id: EntityId) -> BTreeSet<EvidenceId> {
        self.evidence.get(&entity_id).cloned().unwrap_or_default()
    }
}

/// The counts a user is shown before approving an ontology change.
///
/// [`ImpactPreview::digest`] is what binds the shown counts to the approval: the
/// approving claim cites an evidence item whose excerpt digest is this value, so
/// an approval recorded against different counts fails to verify rather than
/// passing quietly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactPreview {
    /// The proposal the counts were computed for.
    pub proposal: OntologyChangeProposal,
    /// Knowledge states attached to the identities the change touches.
    pub state_count: u64,
    /// Graph edges incident to those identities.
    pub edge_count: u64,
    /// Open questions attached to those identities.
    pub question_count: u64,
    /// Evidence items attached to those identities.
    pub evidence_count: u64,
}

impl ImpactPreview {
    /// Counts everything the proposal touches, on both sides of the change.
    ///
    /// A merge touches the disappearing identity and the surviving one; a split
    /// touches the source and every successor. Counting only the source would
    /// understate a merge into a heavily-populated target.
    pub fn compute(
        proposal: &OntologyChangeProposal,
        snapshot: &OntologyImpactSnapshot,
    ) -> Result<Self, RegistryError> {
        proposal.validate()?;
        let touched: BTreeSet<EntityId> = match proposal {
            OntologyChangeProposal::Merge { source, target } => [*source, *target].into(),
            OntologyChangeProposal::Split { source, targets } => {
                let mut set: BTreeSet<EntityId> = targets.iter().copied().collect();
                set.insert(*source);
                set
            }
        };
        let total = |table: &BTreeMap<EntityId, u64>| -> u64 {
            touched
                .iter()
                .map(|entity| table.get(entity).copied().unwrap_or(0))
                .sum()
        };
        let evidence: BTreeSet<EvidenceId> = touched
            .iter()
            .flat_map(|entity| snapshot.evidence_for(*entity))
            .collect();
        Ok(Self {
            proposal: proposal.clone(),
            state_count: total(&snapshot.states),
            edge_count: total(&snapshot.edges),
            question_count: total(&snapshot.questions),
            evidence_count: evidence.len() as u64,
        })
    }

    /// Deterministic bytes the preview digest is taken over.
    ///
    /// Field order is fixed, identifiers are written in their canonical hyphenated
    /// form, and split successors are sorted, so the same preview always produces
    /// the same bytes on every platform.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut rendered = format!(
            "ontology-change-preview/v1\naction={}\nsource={}\n",
            self.proposal.action(),
            self.proposal.source()
        );
        match &self.proposal {
            OntologyChangeProposal::Merge { target, .. } => {
                rendered.push_str(&format!("target={target}\n"));
            }
            OntologyChangeProposal::Split { targets, .. } => {
                for target in targets.iter().collect::<BTreeSet<_>>() {
                    rendered.push_str(&format!("target={target}\n"));
                }
            }
        }
        rendered.push_str(&format!(
            "states={}\nedges={}\nquestions={}\nevidence={}\n",
            self.state_count, self.edge_count, self.question_count, self.evidence_count
        ));
        rendered.into_bytes()
    }

    /// Digest an approval must cite as its evidence excerpt.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(&self.canonical_bytes())
    }

    /// Verifies that an approval's cited digest names exactly these counts.
    pub fn verify_cited(&self, cited: ContentDigest) -> Result<(), RegistryError> {
        let computed = self.digest();
        if cited == computed {
            Ok(())
        } else {
            Err(RegistryError::PreviewDigestMismatch { cited, computed })
        }
    }
}

/// A mention's outcome. Absence of context produces abstention, never a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentionResolution {
    /// Exactly one identity carries the surface form, or context selected one.
    Resolved {
        /// The identity the mention resolves to.
        entity_id: EntityId,
    },
    /// The surface form is ambiguous and nothing narrowed it.
    ///
    /// The mention stays a mention. No sense is assigned, and the candidates are
    /// returned so a reviewer can decide.
    Unresolved {
        /// Candidate identities, in identifier order.
        candidates: Vec<EntityId>,
    },
    /// No registered identity carries the surface form at all.
    Unknown,
}

/// Context a caller may supply to narrow an ambiguous mention.
///
/// An empty context narrows nothing, which is the point: the resolver has no
/// fallback that picks a sense on its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MentionContext {
    /// Identities already established in the surrounding material.
    pub established_entities: BTreeSet<EntityId>,
}

/// The canonical entity registry, folded from anchors and typed claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRegistry {
    entities: BTreeMap<EntityId, RegisteredEntity>,
    aliases: BTreeMap<EntityId, Alias>,
    merges: BTreeMap<EntityId, EntityId>,
    splits: BTreeMap<EntityId, BTreeSet<EntityId>>,
    reclassification: Vec<ReclassificationItem>,
}

impl EntityRegistry {
    /// Folds migration 0004 anchors and typed registry claims into a registry.
    ///
    /// `claims` are consumed in the order given, which is the store's local
    /// acceptance order. Every registry claim must name a subject that some
    /// anchor registers; a claim that does not is rejected rather than ignored,
    /// so the 0004 closure row is the registry's admission control.
    pub fn build(anchors: &[IdentityAnchor], claims: &[Claim]) -> Result<Self, RegistryError> {
        let anchored: BTreeMap<EntityId, &IdentityAnchor> = anchors
            .iter()
            .map(|anchor| (anchor.entity_id, anchor))
            .collect();
        let mut registry = Self {
            entities: BTreeMap::new(),
            aliases: BTreeMap::new(),
            merges: BTreeMap::new(),
            splits: BTreeMap::new(),
            reclassification: Vec::new(),
        };
        let mut pending_alias: BTreeMap<EntityId, PartialAlias> = BTreeMap::new();

        for claim in claims {
            let Some(fact) = RegistryFact::decode(claim)? else {
                continue;
            };
            let subject = fact.subject();
            let Some(anchor) = anchored.get(&subject) else {
                return Err(RegistryError::UnanchoredSubject { subject });
            };
            registry.apply(&fact, anchor, &mut pending_alias);
        }

        for (alias_id, partial) in pending_alias {
            if let Some(alias) = partial.into_alias(alias_id) {
                registry.aliases.insert(alias_id, alias);
            }
        }
        registry.rebuild_reclassification_candidates();
        Ok(registry)
    }

    fn apply(
        &mut self,
        fact: &RegistryFact,
        anchor: &IdentityAnchor,
        pending_alias: &mut BTreeMap<EntityId, PartialAlias>,
    ) {
        match fact {
            RegistryFact::EntityKindDeclared { entity_id, kind } => {
                self.entry(*entity_id, anchor).kind = *kind;
            }
            RegistryFact::LabelDeclared { entity_id, text } => {
                self.entry(*entity_id, anchor).label = Some(text.clone());
            }
            RegistryFact::LabelLanguageDeclared {
                entity_id,
                language,
            } => {
                self.entry(*entity_id, anchor).label_language = Some(language.clone());
            }
            RegistryFact::SenseOf {
                sense_id,
                concept_id,
            } => {
                let entry = self.entry(*sense_id, anchor);
                entry.kind = EntityKind::ConceptSense;
                entry.sense_of = Some(*concept_id);
            }
            RegistryFact::AliasOf {
                alias_id,
                entity_id,
            } => {
                self.entry(*alias_id, anchor).kind = EntityKind::Alias;
                pending_alias.entry(*alias_id).or_default().entity_id = Some(*entity_id);
            }
            RegistryFact::AliasText { alias_id, text } => {
                pending_alias.entry(*alias_id).or_default().text = Some(text.clone());
            }
            RegistryFact::AliasLanguage { alias_id, language } => {
                pending_alias.entry(*alias_id).or_default().language = Some(language.clone());
            }
            RegistryFact::AliasKindDeclared { alias_id, kind } => {
                pending_alias.entry(*alias_id).or_default().kind = Some(*kind);
            }
            RegistryFact::AliasVersion { alias_id, version } => {
                pending_alias.entry(*alias_id).or_default().version = Some(version.clone());
            }
            RegistryFact::MergedInto { source, target } => {
                self.merges.insert(*source, *target);
            }
            RegistryFact::SplitInto { source, target } => {
                self.splits.entry(*source).or_default().insert(*target);
            }
            RegistryFact::ReclassificationPending {
                source,
                evidence_id,
            } => {
                self.reclassification.push(ReclassificationItem {
                    source: *source,
                    evidence_id: *evidence_id,
                    candidates: Vec::new(),
                });
            }
        }
    }

    fn entry(&mut self, entity_id: EntityId, anchor: &IdentityAnchor) -> &mut RegisteredEntity {
        self.entities
            .entry(entity_id)
            .or_insert_with(|| RegisteredEntity {
                entity_id,
                kind: EntityKind::Concept,
                label: None,
                label_language: None,
                sense_of: None,
                scope_id: anchor.scope_id,
                domain_id: anchor.domain_id,
            })
    }

    /// Fills each queued item's candidate list from the split that produced it.
    fn rebuild_reclassification_candidates(&mut self) {
        for item in &mut self.reclassification {
            item.candidates = self
                .splits
                .get(&item.source)
                .map(|targets| targets.iter().copied().collect())
                .unwrap_or_default();
        }
        self.reclassification.sort();
        self.reclassification.dedup();
    }

    /// Every registered identity, in identifier order.
    pub fn entities(&self) -> impl Iterator<Item = &RegisteredEntity> {
        self.entities.values()
    }

    /// Looks up one registered identity.
    #[must_use]
    pub fn entity(&self, entity_id: EntityId) -> Option<&RegisteredEntity> {
        self.entities.get(&entity_id)
    }

    /// Every alias, in alias-identifier order.
    pub fn aliases(&self) -> impl Iterator<Item = &Alias> {
        self.aliases.values()
    }

    /// Aliases naming one entity, in alias-identifier order.
    #[must_use]
    pub fn aliases_of(&self, entity_id: EntityId) -> Vec<&Alias> {
        self.aliases
            .values()
            .filter(|alias| alias.entity_id == entity_id)
            .collect()
    }

    /// The reclassification queue a split produced. Nothing in it has moved.
    #[must_use]
    pub fn reclassification_queue(&self) -> &[ReclassificationItem] {
        &self.reclassification
    }

    /// Successors of a split identity, in identifier order.
    #[must_use]
    pub fn split_targets(&self, entity_id: EntityId) -> Vec<EntityId> {
        self.splits
            .get(&entity_id)
            .map(|targets| targets.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Follows the `MERGED_INTO` redirect chain to the surviving identity.
    ///
    /// A merged-away identifier still resolves; that is the whole point of the
    /// redirect. A cycle cannot extend the walk past the number of recorded
    /// merges, so the loop terminates without a visited set.
    #[must_use]
    pub fn resolve_identity(&self, entity_id: EntityId) -> EntityId {
        let mut current = entity_id;
        for _ in 0..self.merges.len() {
            match self.merges.get(&current) {
                Some(next) if *next != current => current = *next,
                _ => break,
            }
        }
        current
    }

    /// Whether the identifier was merged away and now redirects elsewhere.
    #[must_use]
    pub fn is_redirected(&self, entity_id: EntityId) -> bool {
        self.merges.contains_key(&entity_id)
    }

    /// Identities that redirect into the given one, in identifier order.
    #[must_use]
    pub fn redirects_into(&self, entity_id: EntityId) -> Vec<EntityId> {
        self.merges
            .iter()
            .filter(|(source, _)| self.resolve_identity(**source) == entity_id)
            .map(|(source, _)| *source)
            .collect()
    }

    /// Resolves a surface form, abstaining whenever context does not decide.
    ///
    /// Matching is exact on text and language: a registry that guessed at
    /// normalisation would be inventing identity. When more than one entity
    /// carries the form, `context` may narrow it, but only by naming a candidate
    /// outright. Nothing else narrows, and nothing picks a winner by ranking.
    #[must_use]
    pub fn resolve_mention(
        &self,
        text: &str,
        language: &str,
        context: &MentionContext,
    ) -> MentionResolution {
        let mut candidates: BTreeSet<EntityId> = self
            .aliases
            .values()
            .filter(|alias| alias.text == text && alias.language == language)
            .map(|alias| self.resolve_identity(alias.entity_id))
            .collect();
        for entity in self.entities.values() {
            if entity.label.as_deref() == Some(text)
                && entity.label_language.as_deref() == Some(language)
            {
                candidates.insert(self.resolve_identity(entity.entity_id));
            }
        }
        if candidates.is_empty() {
            return MentionResolution::Unknown;
        }
        if candidates.len() == 1 {
            return candidates.iter().next().map_or(
                MentionResolution::Unresolved {
                    candidates: Vec::new(),
                },
                |entity_id| MentionResolution::Resolved {
                    entity_id: *entity_id,
                },
            );
        }
        let narrowed: Vec<EntityId> = candidates
            .iter()
            .filter(|candidate| context.established_entities.contains(candidate))
            .copied()
            .collect();
        match narrowed.as_slice() {
            [only] => MentionResolution::Resolved { entity_id: *only },
            _ => MentionResolution::Unresolved {
                candidates: candidates.into_iter().collect(),
            },
        }
    }

    /// Classifies how a pre-change identity relates to a post-change identity.
    ///
    /// A split dominates a merge: if the earlier identity was split, no single
    /// successor inherits its state, and saying otherwise is the distortion this
    /// contract exists to prevent.
    #[must_use]
    pub fn equivalence(&self, before: EntityId, after: EntityId) -> EquivalenceClass {
        if self.splits.contains_key(&before) {
            return if self.split_targets(before).contains(&after) || before == after {
                EquivalenceClass::SplitAmbiguous
            } else {
                EquivalenceClass::Incomparable
            };
        }
        if before == after {
            return EquivalenceClass::Identical;
        }
        if self.resolve_identity(before) == after {
            return EquivalenceClass::Refined;
        }
        EquivalenceClass::Incomparable
    }

    /// Compares two multi-year state sets across an ontology change.
    ///
    /// Every returned row carries its equivalence class, including the rows that
    /// refuse to compare. A pre-change identity with no successor and a
    /// post-change identity with no predecessor both appear as
    /// [`EquivalenceClass::Incomparable`] rows rather than being dropped.
    #[must_use]
    pub fn compare_across_change(
        &self,
        before: &[ObservedState],
        after: &[ObservedState],
    ) -> Vec<StateComparison> {
        let after_by_entity: BTreeMap<EntityId, MasteryLevel> = after
            .iter()
            .map(|state| (state.entity_id, state.mastery))
            .collect();
        let mut comparisons = Vec::new();
        let mut matched_after: BTreeSet<EntityId> = BTreeSet::new();

        for state in before {
            let successor = self.resolve_identity(state.entity_id);
            let class = self.equivalence(state.entity_id, successor);
            let observed_after = after_by_entity.get(&successor).copied();
            if observed_after.is_some() {
                matched_after.insert(successor);
            }
            let delta = match (class.permits_comparison(), observed_after) {
                (true, Some(after_level)) => Some(MasteryDelta {
                    before: state.mastery,
                    after: after_level,
                }),
                _ => None,
            };
            let equivalence = if delta.is_none() && class.permits_comparison() {
                // The correspondence is sound but the post-change side observed
                // nothing, so there is no second value to compare against.
                EquivalenceClass::Incomparable
            } else {
                class
            };
            comparisons.push(StateComparison {
                before: Some(state.entity_id),
                after: Some(successor),
                equivalence,
                delta,
            });
        }

        for state in after {
            if matched_after.contains(&state.entity_id) {
                continue;
            }
            comparisons.push(StateComparison {
                before: None,
                after: Some(state.entity_id),
                equivalence: EquivalenceClass::Incomparable,
                delta: None,
            });
        }
        comparisons
    }

    /// Builds a growth narrative that counts only what its class licenses.
    ///
    /// Both refusals are returned, not logged: a reader is told how many nodes
    /// were left out and why, so the narrative cannot read as complete when it
    /// is not.
    #[must_use]
    pub fn growth_narrative(comparisons: &[StateComparison]) -> GrowthNarrative {
        let mut narrative = GrowthNarrative {
            counted: Vec::new(),
            excluded_incomparable: Vec::new(),
            withheld_split_ambiguous: Vec::new(),
        };
        for comparison in comparisons {
            let exclusion = EquivalenceExclusion {
                before: comparison.before,
                after: comparison.after,
                equivalence: comparison.equivalence,
            };
            match comparison.equivalence {
                EquivalenceClass::Identical | EquivalenceClass::Refined => {
                    narrative.counted.push(*comparison);
                }
                EquivalenceClass::SplitAmbiguous => {
                    narrative.withheld_split_ambiguous.push(exclusion);
                }
                EquivalenceClass::Incomparable => {
                    narrative.excluded_incomparable.push(exclusion);
                }
            }
        }
        narrative
    }
}

/// Alias fields accumulated across the claims that describe one alias entity.
#[derive(Debug, Default, Clone)]
struct PartialAlias {
    entity_id: Option<EntityId>,
    text: Option<String>,
    language: Option<String>,
    kind: Option<AliasKind>,
    version: Option<String>,
}

impl PartialAlias {
    /// Produces an alias only when identity, text, language, and kind are all present.
    ///
    /// A half-described alias is dropped rather than completed with defaults,
    /// because a guessed language is exactly the failure the alias registry is
    /// supposed to prevent.
    fn into_alias(self, alias_id: EntityId) -> Option<Alias> {
        Some(Alias {
            alias_id,
            entity_id: self.entity_id?,
            text: self.text?,
            language: self.language?,
            kind: self.kind?,
            version: self.version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthorityClass, ClaimId, DomainError, EpistemicStatus};

    fn id<T: std::str::FromStr<Err = DomainError>>(suffix: u32) -> Result<T, DomainError> {
        format!("01900000-0000-7000-8000-{suffix:012x}").parse()
    }

    fn anchor(entity: u32) -> Result<IdentityAnchor, DomainError> {
        Ok(IdentityAnchor {
            change_id: id(0x9000 + entity)?,
            entity_id: id(entity)?,
            domain_id: id(0x0001)?,
            scope_id: id(0x0002)?,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
        })
    }

    fn user_claim(
        sequence: u32,
        subject: u32,
        predicate: &str,
        object: ClaimObject,
    ) -> Result<Claim, DomainError> {
        Ok(Claim {
            id: id::<ClaimId>(0xa000 + sequence)?,
            subject_entity_id: id(subject)?,
            predicate_id: PredicateId::parse(predicate)?,
            object,
            scope_id: id(0x0002)?,
            authority_class: AuthorityClass::UserExplicit,
            epistemic_status: EpistemicStatus::UserConfirmed,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
            evidence_ids: vec![id(0x5001)?],
        })
    }

    fn built(anchors: &[IdentityAnchor], claims: &[Claim]) -> Result<EntityRegistry, DomainError> {
        EntityRegistry::build(anchors, claims).map_err(|_| DomainError::EmptyValue("registry"))
    }

    /// A redirect chain resolves to its end, and every hop stays resolvable.
    #[test]
    fn merge_chains_resolve_to_the_surviving_identity() -> Result<(), DomainError> {
        let anchors = [anchor(0x1001)?, anchor(0x1002)?, anchor(0x1003)?];
        let claims = [
            user_claim(
                1,
                0x1001,
                PREDICATE_MERGED_INTO,
                ClaimObject::Entity(id(0x1002)?),
            )?,
            user_claim(
                2,
                0x1002,
                PREDICATE_MERGED_INTO,
                ClaimObject::Entity(id(0x1003)?),
            )?,
        ];
        let registry = built(&anchors, &claims)?;
        let first: EntityId = id(0x1001)?;
        let middle: EntityId = id(0x1002)?;
        let last: EntityId = id(0x1003)?;
        assert_eq!(registry.resolve_identity(first), last);
        assert_eq!(registry.resolve_identity(middle), last);
        assert_eq!(registry.equivalence(first, last), EquivalenceClass::Refined);
        assert_eq!(
            registry.equivalence(last, last),
            EquivalenceClass::Identical
        );
        // The middle hop is not the survivor, so comparing against it is refused.
        assert_eq!(
            registry.equivalence(first, middle),
            EquivalenceClass::Incomparable
        );
        Ok(())
    }

    /// A redirect chain that loops back terminates instead of spinning.
    #[test]
    fn a_cyclic_redirect_chain_terminates() -> Result<(), DomainError> {
        let anchors = [anchor(0x1001)?, anchor(0x1002)?];
        let claims = [
            user_claim(
                1,
                0x1001,
                PREDICATE_MERGED_INTO,
                ClaimObject::Entity(id(0x1002)?),
            )?,
            user_claim(
                2,
                0x1002,
                PREDICATE_MERGED_INTO,
                ClaimObject::Entity(id(0x1001)?),
            )?,
        ];
        let registry = built(&anchors, &claims)?;
        let resolved = registry.resolve_identity(id(0x1001)?);
        assert!(resolved == id(0x1001)? || resolved == id(0x1002)?);
        Ok(())
    }

    /// A split dominates a merge: no successor inherits the source's state.
    #[test]
    fn a_split_source_is_never_comparable_to_one_successor() -> Result<(), DomainError> {
        let anchors = [anchor(0x1003)?];
        let claims = [
            user_claim(
                1,
                0x1003,
                PREDICATE_SPLIT_INTO,
                ClaimObject::Entity(id(0x1004)?),
            )?,
            user_claim(
                2,
                0x1003,
                PREDICATE_SPLIT_INTO,
                ClaimObject::Entity(id(0x1005)?),
            )?,
        ];
        let registry = built(&anchors, &claims)?;
        let source: EntityId = id(0x1003)?;
        for successor in [id(0x1004)?, id(0x1005)?, source] {
            assert_eq!(
                registry.equivalence(source, successor),
                EquivalenceClass::SplitAmbiguous
            );
            assert!(!EquivalenceClass::SplitAmbiguous.permits_comparison());
        }
        assert_eq!(
            registry.equivalence(source, id(0x1006)?),
            EquivalenceClass::Incomparable
        );
        Ok(())
    }

    /// A degenerate or self-referential proposal is refused before any preview.
    #[test]
    fn degenerate_proposals_are_refused() -> Result<(), DomainError> {
        let source: EntityId = id(0x1001)?;
        let other: EntityId = id(0x1002)?;
        assert!(matches!(
            OntologyChangeProposal::Merge {
                source,
                target: source
            }
            .validate(),
            Err(RegistryError::SelfSuccessor { .. })
        ));
        assert!(matches!(
            OntologyChangeProposal::Split {
                source,
                targets: vec![other, source]
            }
            .validate(),
            Err(RegistryError::SelfSuccessor { .. })
        ));
        assert!(matches!(
            OntologyChangeProposal::Split {
                source,
                targets: vec![other, other]
            }
            .validate(),
            Err(RegistryError::DegenerateSplit { found: 1, .. })
        ));
        Ok(())
    }

    /// Preview bytes do not depend on the order successors were proposed in.
    #[test]
    fn preview_bytes_are_order_independent() -> Result<(), DomainError> {
        let source: EntityId = id(0x1003)?;
        let targets = [id(0x1004)?, id(0x1005)?, id(0x1006)?];
        let snapshot = OntologyImpactSnapshot::default();
        let forward = OntologyChangeProposal::Split {
            source,
            targets: targets.to_vec(),
        };
        let reversed = OntologyChangeProposal::Split {
            source,
            targets: targets.iter().rev().copied().collect(),
        };
        let (Ok(first), Ok(second)) = (
            ImpactPreview::compute(&forward, &snapshot),
            ImpactPreview::compute(&reversed, &snapshot),
        ) else {
            return Err(DomainError::EmptyValue("impact preview"));
        };
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.digest(), second.digest());
        assert!(first.verify_cited(second.digest()).is_ok());
        Ok(())
    }

    /// A registry predicate carrying the wrong typed object is refused.
    #[test]
    fn a_registry_predicate_refuses_a_foreign_object_kind() -> Result<(), DomainError> {
        let claim = user_claim(
            1,
            0x1001,
            PREDICATE_MERGED_INTO,
            ClaimObject::Text("con-mvcc".to_owned()),
        )?;
        assert!(matches!(
            RegistryFact::decode(&claim),
            Err(RegistryError::ObjectKindMismatch { found: "TEXT", .. })
        ));
        Ok(())
    }

    /// Every registry fact writes and reads back through its own predicate.
    #[test]
    fn every_registry_fact_round_trips_through_a_claim() -> Result<(), DomainError> {
        let entity: EntityId = id(0x1001)?;
        let other: EntityId = id(0x1002)?;
        let facts = [
            RegistryFact::EntityKindDeclared {
                entity_id: entity,
                kind: EntityKind::ConceptSense,
            },
            RegistryFact::LabelDeclared {
                entity_id: entity,
                text: "CPU cache".to_owned(),
            },
            RegistryFact::LabelLanguageDeclared {
                entity_id: entity,
                language: "en".to_owned(),
            },
            RegistryFact::SenseOf {
                sense_id: entity,
                concept_id: other,
            },
            RegistryFact::AliasOf {
                alias_id: entity,
                entity_id: other,
            },
            RegistryFact::AliasText {
                alias_id: entity,
                text: "MVCC".to_owned(),
            },
            RegistryFact::AliasLanguage {
                alias_id: entity,
                language: "ko".to_owned(),
            },
            RegistryFact::AliasKindDeclared {
                alias_id: entity,
                kind: AliasKind::Abbreviation,
            },
            RegistryFact::AliasVersion {
                alias_id: entity,
                version: "catalog-v2".to_owned(),
            },
            RegistryFact::MergedInto {
                source: entity,
                target: other,
            },
            RegistryFact::SplitInto {
                source: entity,
                target: other,
            },
            RegistryFact::ReclassificationPending {
                source: entity,
                evidence_id: id(0x5001)?,
            },
        ];
        assert_eq!(facts.len(), REGISTRY_PREDICATES.len());
        let mut seen = BTreeSet::new();
        for (index, fact) in facts.iter().enumerate() {
            let sequence = u32::try_from(index).unwrap_or_default();
            let mut claim = user_claim(sequence, 0x1001, fact.predicate(), fact.object())?;
            claim.subject_entity_id = fact.subject();
            let Ok(Some(decoded)) = RegistryFact::decode(&claim) else {
                return Err(DomainError::EmptyValue("registry fact"));
            };
            assert_eq!(&decoded, fact);
            assert!(REGISTRY_PREDICATES.contains(&fact.predicate()));
            seen.insert(fact.predicate());
        }
        assert_eq!(seen.len(), REGISTRY_PREDICATES.len());
        Ok(())
    }

    /// The closed vocabularies refuse a member they do not declare.
    #[test]
    fn closed_vocabularies_refuse_unknown_members() {
        assert!(matches!(
            EntityKind::parse("PROCESS"),
            Err(RegistryError::UnknownVocabularyMember { .. })
        ));
        assert!(matches!(
            AliasKind::parse("NICKNAME"),
            Err(RegistryError::UnknownVocabularyMember { .. })
        ));
        for kind in [
            EntityKind::Field,
            EntityKind::Concept,
            EntityKind::ConceptSense,
            EntityKind::Operation,
            EntityKind::Alias,
        ] {
            assert_eq!(EntityKind::parse(kind.as_str()), Ok(kind));
        }
        for kind in [
            AliasKind::Preferred,
            AliasKind::Abbreviation,
            AliasKind::Translation,
            AliasKind::Versioned,
        ] {
            assert_eq!(AliasKind::parse(kind.as_str()), Ok(kind));
        }
    }
}
