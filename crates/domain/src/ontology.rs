//! Versioned ontology import, concept-promotion, and user-curator workflow.
//!
//! This module deliberately builds on [`crate::entity_registry`]. It stages
//! fields, concepts, and operations for import, but aliases, concept senses,
//! merges, splits, mention resolution, redirects, and reclassification remain
//! owned by the canonical entity registry.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    Actor, Claim, ClaimObject, ContentDigest, DomainError, EntityId, EpistemicStatus, EvidenceId,
    EvidenceItem, ScopeId,
    entity_registry::{
        EntityKind, ImpactPreview, OntologyChangeProposal, OntologyImpactSnapshot, RegistryError,
    },
};

/// Predicate carried by the user-confirmed claim that authorizes curation.
pub const CURATOR_APPROVAL_PREDICATE: &str = "ontology.curator.approved";
/// The sole accepted object value for a curator-approval claim.
pub const CURATOR_APPROVAL_OBJECT: &str = "APPROVE";

/// Failures at the ontology import and curator boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OntologyError {
    /// A required label or version identifier was empty.
    #[error("{0} must not be empty")]
    EmptyValue(&'static str),
    /// A versioned configuration used version zero.
    #[error("{0} version must be greater than zero")]
    InvalidVersion(&'static str),
    /// Two imported nodes reused one stable identity.
    #[error("taxonomy import repeats entity identity {0}")]
    DuplicateEntity(EntityId),
    /// A concept or operation named a parent in the wrong ontology tier.
    #[error("{child_kind} {child} requires parent {parent_kind} {parent}")]
    InvalidParent {
        /// The imported child identity.
        child: EntityId,
        /// The child's ontology tier.
        child_kind: &'static str,
        /// The required parent identity.
        parent: EntityId,
        /// The required parent tier.
        parent_kind: &'static str,
    },
    /// The declared taxonomy-version digest did not match the imported nodes.
    #[error("taxonomy version digest mismatch: declared {declared}, computed {computed}")]
    TaxonomyDigestMismatch {
        /// Digest carried by the version identity.
        declared: ContentDigest,
        /// Digest of the canonical imported nodes.
        computed: ContentDigest,
    },
    /// A selected source mix was empty or repeated a version identity.
    #[error("taxonomy mix must name one or more distinct version identities")]
    InvalidTaxonomyMix,
    /// The existing entity-registry preview rejected the change.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// The ADR-003 actor/authority/status matrix rejected an approval claim.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A curator approval did not carry the exact fixed predicate and object.
    #[error("curator approval claim has the wrong predicate or object")]
    InvalidApprovalAction,
    /// A curator approval claim named a different subject.
    #[error("curator approval claim names the wrong subject")]
    ApprovalSubjectMismatch,
    /// A curator approval claim belonged to another resolution scope.
    #[error("curator approval claim names the wrong scope")]
    ApprovalScopeMismatch,
    /// The cited evidence item was not attached to the approval claim.
    #[error("curator approval does not cite its preview evidence")]
    ApprovalEvidenceMissing,
    /// The approval did not bind the exact review bytes it was applied to.
    #[error("curator approval is bound to a different review")]
    ApprovalReviewMismatch,
    /// A required ontology metric was absent.
    #[error("ontology metric {0} is missing")]
    MissingMetric(&'static str),
    /// One ontology metric was supplied more than once.
    #[error("ontology metric {0} is duplicated")]
    DuplicateMetric(&'static str),
    /// A metrics producer attempted to expose a label, term, or pair.
    ///
    /// The content is intentionally absent from this error so rejection cannot
    /// become a second disclosure channel.
    #[error("ontology metric {0} attempted to expose content")]
    MetricContentForbidden(&'static str),
}

fn require_text(value: impl Into<String>, name: &'static str) -> Result<String, OntologyError> {
    let value = value.into();
    if value.trim().is_empty() {
        Err(OntologyError::EmptyValue(name))
    } else {
        Ok(value)
    }
}

/// A broad ontology cluster, distinct from a directly learnable concept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    id: EntityId,
    label: String,
}

impl Field {
    /// Constructs a field with a stable identity and non-empty label.
    pub fn new(id: EntityId, label: impl Into<String>) -> Result<Self, OntologyError> {
        Ok(Self {
            id,
            label: require_text(label, "field label")?,
        })
    }

    /// Returns the stable entity identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// A knowledge unit that may carry explanations, questions, evidence, and prerequisites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Concept {
    id: EntityId,
    label: String,
    field_id: EntityId,
}

impl Concept {
    /// Constructs a concept under one field.
    pub fn new(
        id: EntityId,
        label: impl Into<String>,
        field_id: EntityId,
    ) -> Result<Self, OntologyError> {
        Ok(Self {
            id,
            label: require_text(label, "concept label")?,
            field_id,
        })
    }

    /// Returns the stable entity identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the field that contains this concept.
    #[must_use]
    pub const fn field_id(&self) -> EntityId {
        self.field_id
    }
}

/// A named procedure below a concept, distinct from the concept itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Operation {
    id: EntityId,
    label: String,
    concept_id: EntityId,
}

impl Operation {
    /// Constructs an operation under one concept.
    pub fn new(
        id: EntityId,
        label: impl Into<String>,
        concept_id: EntityId,
    ) -> Result<Self, OntologyError> {
        Ok(Self {
            id,
            label: require_text(label, "operation label")?,
            concept_id,
        })
    }

    /// Returns the stable entity identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        self.id
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the concept this operation specializes.
    #[must_use]
    pub const fn concept_id(&self) -> EntityId {
        self.concept_id
    }
}

/// One primary node in a taxonomy import.
///
/// `ConceptSense` and aliases do not appear here because the entity registry
/// already owns their richer identity contracts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaxonomyNode {
    /// A broad cluster.
    Field(Field),
    /// A directly learnable knowledge unit.
    Concept(Concept),
    /// A procedure beneath a concept.
    Operation(Operation),
}

impl TaxonomyNode {
    /// Returns the stable node identity.
    #[must_use]
    pub const fn id(&self) -> EntityId {
        match self {
            Self::Field(value) => value.id(),
            Self::Concept(value) => value.id(),
            Self::Operation(value) => value.id(),
        }
    }

    /// Maps the import tier onto the existing entity-registry vocabulary.
    #[must_use]
    pub const fn entity_kind(&self) -> EntityKind {
        match self {
            Self::Field(_) => EntityKind::Field,
            Self::Concept(_) => EntityKind::Concept,
            Self::Operation(_) => EntityKind::Operation,
        }
    }

    /// Returns the display label.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Field(value) => value.label(),
            Self::Concept(value) => value.label(),
            Self::Operation(value) => value.label(),
        }
    }
}

/// Provenance class for one imported taxonomy version.
///
/// The enum is descriptive only. No variant is selected as the system default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaxonomySource {
    /// An ACM taxonomy release.
    Acm,
    /// A versioned curriculum-derived taxonomy.
    Curriculum,
    /// A versioned user-derived taxonomy.
    UserDerived,
}

impl TaxonomySource {
    const fn tag(self) -> u8 {
        match self {
            Self::Acm => 1,
            Self::Curriculum => 2,
            Self::UserDerived => 3,
        }
    }
}

/// Identity of one exact taxonomy version.
///
/// Both the release label and canonical content digest participate, so two
/// editions with the same family identity do not collapse.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct TaxonomyVersionIdentity {
    taxonomy_id: EntityId,
    source: TaxonomySource,
    release: String,
    content_digest: ContentDigest,
}

impl TaxonomyVersionIdentity {
    /// Constructs an explicit version identity.
    pub fn new(
        taxonomy_id: EntityId,
        source: TaxonomySource,
        release: impl Into<String>,
        content_digest: ContentDigest,
    ) -> Result<Self, OntologyError> {
        Ok(Self {
            taxonomy_id,
            source,
            release: require_text(release, "taxonomy release")?,
            content_digest,
        })
    }

    /// Returns the stable taxonomy-family identity.
    #[must_use]
    pub const fn taxonomy_id(&self) -> EntityId {
        self.taxonomy_id
    }

    /// Returns the declared source class.
    #[must_use]
    pub const fn source(&self) -> TaxonomySource {
        self.source
    }

    /// Returns the source's release identifier.
    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }

    /// Returns the digest of the canonical imported node set.
    #[must_use]
    pub const fn content_digest(&self) -> ContentDigest {
        self.content_digest
    }

    fn append_canonical_bytes(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(self.taxonomy_id.as_bytes());
        bytes.push(self.source.tag());
        append_text(bytes, &self.release);
        bytes.extend_from_slice(self.content_digest.as_bytes());
    }
}

/// An explicit, versioned source-mix selection.
///
/// There is intentionally no `Default` implementation. The product may remain
/// [`BaseTaxonomyMix::Unselected`] until the user chooses exact version identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyMixSelection {
    configuration_version: u32,
    versions: Vec<TaxonomyVersionIdentity>,
}

impl TaxonomyMixSelection {
    /// Constructs a non-empty selection of distinct version identities.
    pub fn new(
        configuration_version: u32,
        versions: Vec<TaxonomyVersionIdentity>,
    ) -> Result<Self, OntologyError> {
        if configuration_version == 0 {
            return Err(OntologyError::InvalidVersion("taxonomy mix configuration"));
        }
        let distinct = versions.iter().collect::<BTreeSet<_>>().len();
        if versions.is_empty() || distinct != versions.len() {
            return Err(OntologyError::InvalidTaxonomyMix);
        }
        Ok(Self {
            configuration_version,
            versions,
        })
    }

    /// Returns the selection schema version.
    #[must_use]
    pub const fn configuration_version(&self) -> u32 {
        self.configuration_version
    }

    /// Returns the exact taxonomy versions selected by the user.
    #[must_use]
    pub fn versions(&self) -> &[TaxonomyVersionIdentity] {
        &self.versions
    }
}

/// Current state of the still-open base-taxonomy configuration point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseTaxonomyMix {
    /// No ACM/curriculum/user-derived combination has been chosen.
    Unselected,
    /// The user explicitly chose exact taxonomy versions.
    Selected(TaxonomyMixSelection),
}

/// One validated, version-bound taxonomy import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedTaxonomyImport {
    identity: TaxonomyVersionIdentity,
    nodes: Vec<TaxonomyNode>,
}

impl VersionedTaxonomyImport {
    /// Builds an import and computes its content-bound version identity.
    pub fn from_nodes(
        taxonomy_id: EntityId,
        source: TaxonomySource,
        release: impl Into<String>,
        nodes: Vec<TaxonomyNode>,
    ) -> Result<Self, OntologyError> {
        validate_nodes(&nodes)?;
        let content_digest = taxonomy_nodes_digest(&nodes);
        let identity = TaxonomyVersionIdentity::new(taxonomy_id, source, release, content_digest)?;
        Ok(Self { identity, nodes })
    }

    /// Admits nodes only when they match the supplied version identity.
    pub fn with_identity(
        identity: TaxonomyVersionIdentity,
        nodes: Vec<TaxonomyNode>,
    ) -> Result<Self, OntologyError> {
        validate_nodes(&nodes)?;
        let computed = taxonomy_nodes_digest(&nodes);
        if computed != identity.content_digest {
            return Err(OntologyError::TaxonomyDigestMismatch {
                declared: identity.content_digest,
                computed,
            });
        }
        Ok(Self { identity, nodes })
    }

    /// Returns the exact taxonomy version this import carries.
    #[must_use]
    pub const fn identity(&self) -> &TaxonomyVersionIdentity {
        &self.identity
    }

    /// Returns the validated primary nodes.
    #[must_use]
    pub fn nodes(&self) -> &[TaxonomyNode] {
        &self.nodes
    }
}

fn validate_nodes(nodes: &[TaxonomyNode]) -> Result<(), OntologyError> {
    if nodes.is_empty() {
        return Err(OntologyError::EmptyValue("taxonomy nodes"));
    }
    let mut identities = BTreeSet::new();
    let mut fields = BTreeSet::new();
    let mut concepts = BTreeSet::new();
    for node in nodes {
        if !identities.insert(node.id()) {
            return Err(OntologyError::DuplicateEntity(node.id()));
        }
        match node {
            TaxonomyNode::Field(value) => {
                fields.insert(value.id());
            }
            TaxonomyNode::Concept(value) => {
                concepts.insert(value.id());
            }
            TaxonomyNode::Operation(_) => {}
        }
    }
    for node in nodes {
        match node {
            TaxonomyNode::Field(_) => {}
            TaxonomyNode::Concept(value) if !fields.contains(&value.field_id()) => {
                return Err(OntologyError::InvalidParent {
                    child: value.id(),
                    child_kind: EntityKind::Concept.as_str(),
                    parent: value.field_id(),
                    parent_kind: EntityKind::Field.as_str(),
                });
            }
            TaxonomyNode::Operation(value) if !concepts.contains(&value.concept_id()) => {
                return Err(OntologyError::InvalidParent {
                    child: value.id(),
                    child_kind: EntityKind::Operation.as_str(),
                    parent: value.concept_id(),
                    parent_kind: EntityKind::Concept.as_str(),
                });
            }
            TaxonomyNode::Concept(_) | TaxonomyNode::Operation(_) => {}
        }
    }
    Ok(())
}

fn append_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn taxonomy_nodes_digest(nodes: &[TaxonomyNode]) -> ContentDigest {
    let mut nodes = nodes.iter().collect::<Vec<_>>();
    nodes.sort_by_key(|node| node.id());
    let mut bytes = b"taxonomy-import/v1\0".to_vec();
    for node in nodes {
        match node {
            TaxonomyNode::Field(value) => {
                bytes.push(1);
                bytes.extend_from_slice(value.id().as_bytes());
            }
            TaxonomyNode::Concept(value) => {
                bytes.push(2);
                bytes.extend_from_slice(value.id().as_bytes());
                bytes.extend_from_slice(value.field_id().as_bytes());
            }
            TaxonomyNode::Operation(value) => {
                bytes.push(3);
                bytes.extend_from_slice(value.id().as_bytes());
                bytes.extend_from_slice(value.concept_id().as_bytes());
            }
        }
        append_text(&mut bytes, node.label());
    }
    ContentDigest::sha256(&bytes)
}

/// One independent attachment that can justify a concept candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConceptPromotionCriterion {
    /// Evidence containing an independent explanation.
    IndependentExplanation(EvidenceId),
    /// A question that directly addresses the candidate.
    Question(EntityId),
    /// Evidence directly supporting the candidate.
    Evidence(EvidenceId),
    /// A prerequisite link incident to the candidate.
    Prerequisite(EntityId),
}

impl ConceptPromotionCriterion {
    const fn tag(self) -> u8 {
        match self {
            Self::IndependentExplanation(_) => 1,
            Self::Question(_) => 2,
            Self::Evidence(_) => 3,
            Self::Prerequisite(_) => 4,
        }
    }

    fn id_bytes(self) -> [u8; 16] {
        match self {
            Self::IndependentExplanation(id) | Self::Evidence(id) => *id.as_bytes(),
            Self::Question(id) | Self::Prerequisite(id) => *id.as_bytes(),
        }
    }
}

/// An unresolved surface term and the distinct source occurrences that named it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    surface: String,
    occurrences: BTreeSet<EvidenceId>,
    criteria: BTreeSet<ConceptPromotionCriterion>,
}

impl Mention {
    /// Constructs a mention candidate from one or more distinct occurrences.
    pub fn new(
        surface: impl Into<String>,
        occurrences: impl IntoIterator<Item = EvidenceId>,
        criteria: impl IntoIterator<Item = ConceptPromotionCriterion>,
    ) -> Result<Self, OntologyError> {
        let occurrences = occurrences.into_iter().collect::<BTreeSet<_>>();
        if occurrences.is_empty() {
            return Err(OntologyError::EmptyValue("mention occurrences"));
        }
        Ok(Self {
            surface: require_text(surface, "mention surface")?,
            occurrences,
            criteria: criteria.into_iter().collect(),
        })
    }

    /// Returns the source surface form without resolving it to an identity.
    #[must_use]
    pub fn surface(&self) -> &str {
        &self.surface
    }

    /// Returns the number of distinct occurrences.
    #[must_use]
    pub fn occurrence_count(&self) -> usize {
        self.occurrences.len()
    }

    /// Returns the independent promotion attachments.
    #[must_use]
    pub const fn criteria(&self) -> &BTreeSet<ConceptPromotionCriterion> {
        &self.criteria
    }
}

/// Visible granularity state for a mention, candidate, or approved node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GranularityStatus {
    /// The term has not earned a canonical concept identity.
    Mention,
    /// The candidate passed the mechanical gate and awaits the user curator.
    GranularityUnderReview,
    /// The single user curator approved the exact review bytes.
    Curated,
}

/// One normative example from the §7.4 granularity policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GranularityExample {
    /// The surface label used by the specification.
    pub label: &'static str,
    /// The ontology tier the example occupies.
    pub kind: EntityKind,
    /// The parent label when the example is nested.
    pub parent_label: Option<&'static str>,
}

/// Normative examples that keep broad fields, concepts, and operations apart.
pub const GRANULARITY_EXAMPLES: [GranularityExample; 4] = [
    GranularityExample {
        label: "Database Systems",
        kind: EntityKind::Field,
        parent_label: None,
    },
    GranularityExample {
        label: "Serializability",
        kind: EntityKind::Concept,
        parent_label: Some("Database Systems"),
    },
    GranularityExample {
        label: "B+ Tree",
        kind: EntityKind::Concept,
        parent_label: Some("Database Systems"),
    },
    GranularityExample {
        label: "B+ Tree node split",
        kind: EntityKind::Operation,
        parent_label: Some("B+ Tree"),
    },
];

impl GranularityStatus {
    /// Returns the stable wire vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mention => "MENTION",
            Self::GranularityUnderReview => "GRANULARITY_UNDER_REVIEW",
            Self::Curated => "CURATED",
        }
    }
}

/// Why the promotion gate abstained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionAbstention {
    /// One source occurrence cannot create a concept, even with an attachment.
    SingleOccurrence,
    /// Repeated text lacks an explanation, question, evidence, or prerequisite.
    MissingIndependentAttachment,
}

/// Result of applying the concept-granularity gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConceptPromotion {
    /// The term remains a non-canonical mention.
    Mention {
        /// Why no concept candidate was created.
        reason: PromotionAbstention,
    },
    /// The term may be proposed to the user curator, but is not yet curated.
    GranularityUnderReview(ConceptCandidate),
}

impl ConceptPromotion {
    /// Returns the externally visible granularity state.
    #[must_use]
    pub const fn status(&self) -> GranularityStatus {
        match self {
            Self::Mention { .. } => GranularityStatus::Mention,
            Self::GranularityUnderReview(_) => GranularityStatus::GranularityUnderReview,
        }
    }
}

/// A mechanically eligible concept that still requires user-curator approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptCandidate {
    concept: Concept,
    scope_id: ScopeId,
    occurrences: BTreeSet<EvidenceId>,
    criteria: BTreeSet<ConceptPromotionCriterion>,
}

impl ConceptCandidate {
    /// Returns the proposed concept.
    #[must_use]
    pub const fn concept(&self) -> &Concept {
        &self.concept
    }

    /// Returns the resolution scope the candidate belongs to.
    #[must_use]
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Returns the criteria that caused the gate to admit review.
    #[must_use]
    pub const fn criteria(&self) -> &BTreeSet<ConceptPromotionCriterion> {
        &self.criteria
    }

    /// Returns the exact digest a user approval must cite.
    #[must_use]
    pub fn review_digest(&self) -> ContentDigest {
        let mut bytes = b"concept-granularity-review/v1\0".to_vec();
        bytes.extend_from_slice(self.concept.id().as_bytes());
        bytes.extend_from_slice(self.concept.field_id().as_bytes());
        bytes.extend_from_slice(self.scope_id.as_bytes());
        append_text(&mut bytes, self.concept.label());
        bytes.extend_from_slice(&(self.occurrences.len() as u64).to_be_bytes());
        for occurrence in &self.occurrences {
            bytes.extend_from_slice(occurrence.as_bytes());
        }
        bytes.extend_from_slice(&(self.criteria.len() as u64).to_be_bytes());
        for criterion in &self.criteria {
            bytes.push(criterion.tag());
            bytes.extend_from_slice(&criterion.id_bytes());
        }
        ContentDigest::sha256(&bytes)
    }

    /// Curates the candidate only with a token verified for these exact bytes.
    ///
    /// An actor value is not an approval capability, so direct delegation to an
    /// automated actor is rejected by the Rust type system:
    ///
    /// ```compile_fail
    /// use academic_domain::{Actor, ontology::ConceptCandidate};
    ///
    /// fn automated_actor_cannot_approve(candidate: ConceptCandidate, actor: Actor) {
    ///     let _ = candidate.approve(actor);
    /// }
    /// ```
    pub fn approve(
        self,
        approval: VerifiedCuratorApproval,
    ) -> Result<ApprovedConcept, OntologyError> {
        if approval.subject != self.concept.id() || approval.review_digest != self.review_digest() {
            return Err(OntologyError::ApprovalReviewMismatch);
        }
        Ok(ApprovedConcept {
            concept: self.concept,
            approved_by: approval.user_id,
        })
    }
}

/// Applies the mechanical half of concept promotion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConceptPromotionGate;

impl ConceptPromotionGate {
    /// Keeps single occurrences and unsupported repeated terms as mentions.
    pub fn evaluate(
        mention: &Mention,
        concept_id: EntityId,
        field_id: EntityId,
        scope_id: ScopeId,
    ) -> Result<ConceptPromotion, OntologyError> {
        if mention.occurrences.len() == 1 {
            return Ok(ConceptPromotion::Mention {
                reason: PromotionAbstention::SingleOccurrence,
            });
        }
        if mention.criteria.is_empty() {
            return Ok(ConceptPromotion::Mention {
                reason: PromotionAbstention::MissingIndependentAttachment,
            });
        }
        Ok(ConceptPromotion::GranularityUnderReview(ConceptCandidate {
            concept: Concept::new(concept_id, mention.surface.clone(), field_id)?,
            scope_id,
            occurrences: mention.occurrences.clone(),
            criteria: mention.criteria.clone(),
        }))
    }
}

/// A concept approved by the user curator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedConcept {
    concept: Concept,
    approved_by: EntityId,
}

impl ApprovedConcept {
    /// Returns the curated concept.
    #[must_use]
    pub const fn concept(&self) -> &Concept {
        &self.concept
    }

    /// Returns the user principal that approved it.
    #[must_use]
    pub const fn approved_by(&self) -> EntityId {
        self.approved_by
    }

    /// Returns the final workflow state.
    #[must_use]
    pub const fn status(&self) -> GranularityStatus {
        GranularityStatus::Curated
    }
}

/// Existing registry impact counts bound to one exact taxonomy version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedImpactPreview {
    taxonomy_version: TaxonomyVersionIdentity,
    scope_id: ScopeId,
    impact: ImpactPreview,
}

impl VersionedImpactPreview {
    /// Returns the version whose ontology graph would change.
    #[must_use]
    pub const fn taxonomy_version(&self) -> &TaxonomyVersionIdentity {
        &self.taxonomy_version
    }

    /// Returns the resolution scope whose projections were counted.
    #[must_use]
    pub const fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    /// Returns the existing registry's state/edge/question/evidence counts.
    #[must_use]
    pub const fn impact(&self) -> &ImpactPreview {
        &self.impact
    }

    /// Digests the taxonomy version together with the registry preview.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut bytes = b"versioned-ontology-change-preview/v1\0".to_vec();
        self.taxonomy_version.append_canonical_bytes(&mut bytes);
        bytes.extend_from_slice(self.scope_id.as_bytes());
        bytes.extend_from_slice(&self.impact.canonical_bytes());
        ContentDigest::sha256(&bytes)
    }
}

/// A merge or split waiting for the sole user curator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyChangeReview {
    preview: VersionedImpactPreview,
}

impl OntologyChangeReview {
    /// Computes the registry-owned impact preview before approval is possible.
    pub fn new(
        taxonomy_version: TaxonomyVersionIdentity,
        scope_id: ScopeId,
        proposal: OntologyChangeProposal,
        snapshot: &OntologyImpactSnapshot,
    ) -> Result<Self, OntologyError> {
        Ok(Self {
            preview: VersionedImpactPreview {
                taxonomy_version,
                scope_id,
                impact: ImpactPreview::compute(&proposal, snapshot)?,
            },
        })
    }

    /// Returns the version-bound impact preview the user must see.
    #[must_use]
    pub const fn preview(&self) -> &VersionedImpactPreview {
        &self.preview
    }

    /// Returns the visible review state.
    #[must_use]
    pub const fn status(&self) -> GranularityStatus {
        GranularityStatus::GranularityUnderReview
    }

    /// Applies a token that can only be issued for these exact preview bytes.
    pub fn approve(
        self,
        approval: VerifiedCuratorApproval,
    ) -> Result<ApprovedOntologyChange, OntologyError> {
        if approval.subject != self.preview.impact.proposal.source()
            || approval.review_digest != self.preview.digest()
        {
            return Err(OntologyError::ApprovalReviewMismatch);
        }
        Ok(ApprovedOntologyChange {
            preview: self.preview,
            approved_by: approval.user_id,
        })
    }
}

/// An ontology change approved against an exact version and impact preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedOntologyChange {
    preview: VersionedImpactPreview,
    approved_by: EntityId,
}

impl ApprovedOntologyChange {
    /// Returns the versioned preview that was approved.
    #[must_use]
    pub const fn preview(&self) -> &VersionedImpactPreview {
        &self.preview
    }

    /// Returns the approving user principal.
    #[must_use]
    pub const fn approved_by(&self) -> EntityId {
        self.approved_by
    }

    /// Returns the final workflow state.
    #[must_use]
    pub const fn status(&self) -> GranularityStatus {
        GranularityStatus::Curated
    }
}

/// Proof that ADR-003 accepted a user-authored, user-confirmed approval claim.
///
/// Fields are private, the type is not `Clone` or `Serialize`, and curation
/// methods accept this type instead of [`Actor`] or [`Claim`]. Automated actor
/// variants can therefore reach the action only by first passing the same
/// fail-closed matrix as canonical signed events, which rejects them.
#[derive(Debug)]
pub struct VerifiedCuratorApproval {
    user_id: EntityId,
    subject: EntityId,
    review_digest: ContentDigest,
}

impl VerifiedCuratorApproval {
    /// Verifies an approval for a concept candidate.
    pub fn for_concept(
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
        candidate: &ConceptCandidate,
    ) -> Result<Self, OntologyError> {
        Self::verify(
            actor,
            claim,
            evidence,
            candidate.concept.id(),
            candidate.scope_id,
            candidate.review_digest(),
        )
    }

    /// Verifies an approval for a version-bound ontology change preview.
    pub fn for_change_review(
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
        review: &OntologyChangeReview,
    ) -> Result<Self, OntologyError> {
        Self::verify(
            actor,
            claim,
            evidence,
            review.preview.impact.proposal.source(),
            review.preview.scope_id,
            review.preview.digest(),
        )
    }

    fn verify(
        actor: &Actor,
        claim: &Claim,
        evidence: &EvidenceItem,
        subject: EntityId,
        scope_id: ScopeId,
        review_digest: ContentDigest,
    ) -> Result<Self, OntologyError> {
        claim.validate_for_actor(actor)?;
        if claim.epistemic_status != EpistemicStatus::UserConfirmed
            || claim.predicate_id.as_str() != CURATOR_APPROVAL_PREDICATE
            || claim.object != ClaimObject::Text(CURATOR_APPROVAL_OBJECT.to_owned())
        {
            return Err(OntologyError::InvalidApprovalAction);
        }
        if claim.subject_entity_id != subject {
            return Err(OntologyError::ApprovalSubjectMismatch);
        }
        if claim.scope_id != scope_id {
            return Err(OntologyError::ApprovalScopeMismatch);
        }
        if !claim.evidence_ids.contains(&evidence.id) {
            return Err(OntologyError::ApprovalEvidenceMissing);
        }
        evidence.validate()?;
        if evidence.excerpt_digest != review_digest {
            return Err(OntologyError::ApprovalReviewMismatch);
        }
        let Actor::User { user_id } = actor else {
            // The matrix above already rejects this branch for a valid approval
            // claim; retaining the pattern match keeps the resulting authority
            // token structurally user-only if that matrix ever grows.
            return Err(OntologyError::InvalidApprovalAction);
        };
        Ok(Self {
            user_id: *user_id,
            subject,
            review_digest,
        })
    }
}

/// The two aggregate quality metrics permitted to cross a metrics boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OntologyMetricName {
    /// Count of nodes with no incident ontology relation.
    OrphanCount,
    /// Count of candidate near-duplicate pairs.
    NearDuplicatePairCount,
}

impl OntologyMetricName {
    /// Returns the stable metric key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OrphanCount => "orphan_count",
            Self::NearDuplicatePairCount => "near_duplicate_pair_count",
        }
    }
}

/// Candidate value observed at the metrics boundary.
pub enum OntologyMetricValue {
    /// An aggregate count, which may be admitted.
    Count(u64),
    /// A term, label, identity list, or duplicate pair, which must be rejected.
    Content(String),
}

impl fmt::Debug for OntologyMetricValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count(value) => formatter.debug_tuple("Count").field(value).finish(),
            Self::Content(_) => formatter.write_str("Content(<redacted>)"),
        }
    }
}

/// One producer observation submitted to the content-free metrics boundary.
#[derive(Debug)]
pub struct OntologyMetricObservation {
    /// Which aggregate metric is being reported.
    pub name: OntologyMetricName,
    /// Either the permitted count or an injected forbidden content value.
    pub value: OntologyMetricValue,
}

/// Content-free ontology quality metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OntologyQualityMetrics {
    orphan_count: u64,
    near_duplicate_pair_count: u64,
}

impl OntologyQualityMetrics {
    /// Admits exactly one count for each metric and rejects all content values.
    pub fn observe(
        observations: impl IntoIterator<Item = OntologyMetricObservation>,
    ) -> Result<Self, OntologyError> {
        let mut orphan_count = None;
        let mut near_duplicate_pair_count = None;
        for observation in observations {
            let target = match observation.name {
                OntologyMetricName::OrphanCount => &mut orphan_count,
                OntologyMetricName::NearDuplicatePairCount => &mut near_duplicate_pair_count,
            };
            let OntologyMetricValue::Count(count) = observation.value else {
                return Err(OntologyError::MetricContentForbidden(
                    observation.name.as_str(),
                ));
            };
            if target.replace(count).is_some() {
                return Err(OntologyError::DuplicateMetric(observation.name.as_str()));
            }
        }
        Ok(Self {
            orphan_count: orphan_count.ok_or(OntologyError::MissingMetric(
                OntologyMetricName::OrphanCount.as_str(),
            ))?,
            near_duplicate_pair_count: near_duplicate_pair_count.ok_or(
                OntologyError::MissingMetric(OntologyMetricName::NearDuplicatePairCount.as_str()),
            )?,
        })
    }

    /// Returns the orphan aggregate only.
    #[must_use]
    pub const fn orphan_count(self) -> u64 {
        self.orphan_count
    }

    /// Returns the near-duplicate pair aggregate only.
    #[must_use]
    pub const fn near_duplicate_pair_count(self) -> u64 {
        self.near_duplicate_pair_count
    }
}
