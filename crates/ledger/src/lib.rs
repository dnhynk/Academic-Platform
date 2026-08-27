//! Pure append-only ledger and bitemporal resolver.
//!
//! The ledger assigns replica-local acceptance sequence numbers. Origin order,
//! acceptance order, and domain-valid time remain independent values.

use std::collections::{BTreeMap, BTreeSet};

use academic_contracts::VerifiedBatch;
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, BatchId, Claim, ClaimId, ClaimObject, ClaimRelation,
    ClaimRelationKind, ContentDigest, DecisionAction, DeviceId, DomainError, DomainId, EntityId,
    EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem, PredicateId,
    ScopeDescriptor, ScopeId, TimestampMillis, UserDecision,
};
pub use academic_domain::{EVENT_SCHEMA_VERSION, UnsignedBatch};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod resolver;

use resolver::relation_effect_is_authorized;
pub use resolver::{
    AuthorityPolicy, KnowledgeStateView, ResolutionClaim, ResolutionDecision, ResolutionQuery,
    ResolutionRelation, ResolutionResult, ResolverActorKind,
    relation_effect_is_authorized_for_kind, resolve_snapshot,
};

#[cfg(test)]
use academic_domain::{AuthorityClass, MasteryLevel};
#[cfg(test)]
use resolver::authority_rank;

/// Ledger acceptance or replay failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// A nested domain value was invalid.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// The first batch for a device must begin its chain at sequence one.
    #[error("first device batch must start at origin sequence 1 and have no previous hash")]
    InvalidChainStart,
    /// A later batch left a gap in a device's origin sequence.
    #[error("device origin gap: expected {expected}, got {actual}")]
    OriginGap { expected: u64, actual: u64 },
    /// A batch tried to reuse origin history with different content.
    #[error("device chain fork detected")]
    DeviceFork,
    /// A batch ID was reused with different canonical bytes.
    #[error("batch id was reused with a different digest")]
    BatchIdCollision,
    /// An immutable identifier was already present.
    #[error("duplicate immutable {kind} id: {id}")]
    DuplicateId { kind: &'static str, id: String },
    /// Evidence referred to an artifact not yet accepted.
    #[error("evidence references unknown artifact {0}")]
    UnknownArtifact(ArtifactId),
    /// A claim referred to evidence not yet accepted.
    #[error("claim references unknown evidence {0}")]
    UnknownEvidence(EvidenceId),
    /// A claim relation or decision referred to a claim not yet accepted.
    #[error("event references unknown claim {0}")]
    UnknownClaim(ClaimId),
    /// A claim, relation, or decision named a scope not yet registered.
    #[error("event references unknown scope {0}")]
    UnknownScope(ScopeId),
    /// A nested payload crossed its registered logical domain boundary.
    #[error("{0} crosses its registered domain boundary")]
    CrossDomain(&'static str),
    /// An evidence locator/digest was not proven by immutable artifact metadata.
    #[error("evidence representation is not registered for evidence {0}")]
    UnprovenEvidenceRepresentation(EvidenceId),
    /// A relationship or decision crossed claim scopes.
    #[error("{0} crosses claim scope boundaries")]
    CrossScope(&'static str),
    /// A user decision's explicit semantic slot or target object did not match its claim.
    #[error("user decision semantic target does not match the referenced claim")]
    DecisionSemanticMismatch,
    /// A state-removing relation was not authorized for both endpoint authorities.
    #[error(
        "actor {actor} is not authorized to apply {kind:?} to both claim authority/status pairs"
    )]
    UnauthorizedRelationEffect {
        actor: &'static str,
        kind: ClaimRelationKind,
    },
    /// An acceptance sequence counter overflowed.
    #[error("acceptance sequence exhausted")]
    AcceptSequenceExhausted,
}

/// A replica-local receipt. It does not pretend to be global wall-clock order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceReceipt {
    pub batch_id: BatchId,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
    pub batch_hash: ContentDigest,
    pub duplicate: bool,
}

/// Event paired with the replica-local acceptance order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedEvent {
    pub accept_seq: u64,
    pub batch_id: BatchId,
    pub event: Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeviceHead {
    origin_seq_end: u64,
    batch_hash: ContentDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AcceptedClaimMeta {
    accept_seq: u64,
    domain_id: DomainId,
    scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcceptedRelationMeta {
    accept_seq: u64,
    actor: Actor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AcceptedDecisionMeta {
    accept_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcceptedArtifact {
    descriptor: ArtifactDescriptor,
}

impl AcceptedArtifact {
    fn supports_evidence(&self, evidence: &EvidenceItem) -> bool {
        if evidence.artifact_id != self.descriptor.id {
            return false;
        }
        let Some(representation) = self.descriptor.representation(&evidence.locator) else {
            return false;
        };
        self.descriptor.is_artifact_digest_bound(representation)
            && evidence.excerpt_digest == self.descriptor.content_digest
    }
}

/// In-memory Phase 0 ledger. Its public API exposes append and query, never mutation.
#[derive(Debug, Clone)]
pub struct LedgerState {
    next_accept_seq: u64,
    accepted_events: Vec<AcceptedEvent>,
    batch_receipts: BTreeMap<BatchId, AcceptanceReceipt>,
    device_heads: BTreeMap<DeviceId, DeviceHead>,
    event_ids: BTreeSet<EventId>,
    scopes: BTreeMap<ScopeId, ScopeDescriptor>,
    artifacts: BTreeMap<ArtifactId, AcceptedArtifact>,
    evidence: BTreeMap<EvidenceId, EvidenceItem>,
    claims: BTreeMap<ClaimId, (Claim, AcceptedClaimMeta)>,
    relations: Vec<(ClaimRelation, AcceptedRelationMeta)>,
    decision_ids: BTreeSet<academic_domain::DecisionId>,
    decisions: Vec<(UserDecision, AcceptedDecisionMeta)>,
}

impl Default for LedgerState {
    fn default() -> Self {
        Self::new()
    }
}

impl LedgerState {
    /// Creates an empty replica whose first accepted event receives sequence one.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_accept_seq: 1,
            accepted_events: Vec::new(),
            batch_receipts: BTreeMap::new(),
            device_heads: BTreeMap::new(),
            event_ids: BTreeSet::new(),
            scopes: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            evidence: BTreeMap::new(),
            claims: BTreeMap::new(),
            relations: Vec::new(),
            decision_ids: BTreeSet::new(),
            decisions: Vec::new(),
        }
    }

    /// Atomically validates and appends a verified batch.
    ///
    /// Replaying the same batch ID and digest is idempotent and returns the
    /// original receipt with `duplicate=true`.
    pub fn accept_verified_batch(
        &mut self,
        verified: &VerifiedBatch,
    ) -> Result<AcceptanceReceipt, LedgerError> {
        let batch = verified.batch();
        let batch_hash = verified.envelope_hash();
        if let Some(receipt) = self.batch_receipts.get(&batch.batch_id) {
            if receipt.batch_hash == batch_hash {
                return Ok(AcceptanceReceipt {
                    duplicate: true,
                    ..*receipt
                });
            }
            return Err(LedgerError::BatchIdCollision);
        }
        self.validate_device_chain(batch)?;

        let mut staged = self.clone();
        let receipt = staged.apply_batch(batch, batch_hash)?;
        *self = staged;
        Ok(receipt)
    }

    fn validate_device_chain(&self, batch: &UnsignedBatch) -> Result<(), LedgerError> {
        match self.device_heads.get(&batch.device_id) {
            None => {
                if batch.origin_seq_start != 1 || batch.previous_batch_hash.is_some() {
                    return Err(LedgerError::InvalidChainStart);
                }
            }
            Some(head) => {
                let expected = head
                    .origin_seq_end
                    .checked_add(1)
                    .ok_or(LedgerError::AcceptSequenceExhausted)?;
                if batch.origin_seq_start > expected {
                    return Err(LedgerError::OriginGap {
                        expected,
                        actual: batch.origin_seq_start,
                    });
                }
                if batch.origin_seq_start < expected
                    || batch.previous_batch_hash != Some(head.batch_hash)
                {
                    return Err(LedgerError::DeviceFork);
                }
            }
        }
        Ok(())
    }

    fn apply_batch(
        &mut self,
        batch: &UnsignedBatch,
        batch_hash: ContentDigest,
    ) -> Result<AcceptanceReceipt, LedgerError> {
        let accept_seq_start = self.next_accept_seq;
        for event in &batch.events {
            let accept_seq = self.next_accept_seq;
            self.apply_event(event, accept_seq)?;
            self.accepted_events.push(AcceptedEvent {
                accept_seq,
                batch_id: batch.batch_id,
                event: event.clone(),
            });
            self.next_accept_seq = self
                .next_accept_seq
                .checked_add(1)
                .ok_or(LedgerError::AcceptSequenceExhausted)?;
        }
        let accept_seq_end = self
            .next_accept_seq
            .checked_sub(1)
            .ok_or(LedgerError::AcceptSequenceExhausted)?;
        let receipt = AcceptanceReceipt {
            batch_id: batch.batch_id,
            accept_seq_start,
            accept_seq_end,
            batch_hash,
            duplicate: false,
        };
        self.batch_receipts.insert(batch.batch_id, receipt);
        self.device_heads.insert(
            batch.device_id,
            DeviceHead {
                origin_seq_end: batch.origin_seq_end,
                batch_hash,
            },
        );
        Ok(receipt)
    }

    fn apply_event(&mut self, event: &Event, accept_seq: u64) -> Result<(), LedgerError> {
        // Acceptance revalidates the authenticated event instead of relying on a caller's
        // earlier batch traversal. This keeps claim provenance and prediction requirements
        // fail-closed at the append boundary itself.
        event.validate()?;
        if !self.event_ids.insert(event.id) {
            return Err(LedgerError::DuplicateId {
                kind: "event",
                id: event.id.to_string(),
            });
        }
        match &event.payload {
            EventPayload::ScopeRegistered(scope) => {
                if self.scopes.contains_key(&scope.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "scope",
                        id: scope.id.to_string(),
                    });
                }
                if scope.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("scope registration"));
                }
                self.scopes.insert(scope.id, scope.clone());
            }
            EventPayload::ArtifactRegistered(descriptor) => {
                if self.artifacts.contains_key(&descriptor.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "artifact",
                        id: descriptor.id.to_string(),
                    });
                }
                self.artifacts.insert(
                    descriptor.id,
                    AcceptedArtifact {
                        descriptor: descriptor.clone(),
                    },
                );
            }
            EventPayload::EvidenceRegistered(item) => {
                let descriptor = self
                    .artifacts
                    .get(&item.artifact_id)
                    .ok_or(LedgerError::UnknownArtifact(item.artifact_id))?;
                if descriptor.descriptor.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("evidence artifact"));
                }
                if !descriptor.supports_evidence(item) {
                    return Err(LedgerError::UnprovenEvidenceRepresentation(item.id));
                }
                if self.evidence.contains_key(&item.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "evidence",
                        id: item.id.to_string(),
                    });
                }
                self.evidence.insert(item.id, item.clone());
            }
            EventPayload::ClaimAsserted(claim) => {
                let scope = self
                    .scopes
                    .get(&claim.scope_id)
                    .ok_or(LedgerError::UnknownScope(claim.scope_id))?;
                if scope.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("claim scope"));
                }
                for evidence_id in &claim.evidence_ids {
                    let evidence = self
                        .evidence
                        .get(evidence_id)
                        .ok_or(LedgerError::UnknownEvidence(*evidence_id))?;
                    let artifact = self
                        .artifacts
                        .get(&evidence.artifact_id)
                        .ok_or(LedgerError::UnknownArtifact(evidence.artifact_id))?;
                    if artifact.descriptor.domain_id != event.domain_id {
                        return Err(LedgerError::CrossDomain("claim evidence"));
                    }
                }
                if self.claims.contains_key(&claim.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "claim",
                        id: claim.id.to_string(),
                    });
                }
                self.claims.insert(
                    claim.id,
                    (
                        claim.clone(),
                        AcceptedClaimMeta {
                            accept_seq,
                            domain_id: event.domain_id,
                            scope_id: claim.scope_id,
                        },
                    ),
                );
            }
            EventPayload::ClaimRelated(relation) => {
                let (source_claim, source) = self.claim_record(relation.source_claim_id)?;
                let (target_claim, target) = self.claim_record(relation.target_claim_id)?;
                if source.scope_id != relation.scope_id || target.scope_id != relation.scope_id {
                    return Err(LedgerError::CrossScope("claim relation"));
                }
                if source.domain_id != event.domain_id || target.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("claim relation"));
                }
                if !relation_effect_is_authorized(
                    &event.actor,
                    relation.kind,
                    source_claim,
                    target_claim,
                ) {
                    return Err(LedgerError::UnauthorizedRelationEffect {
                        actor: event.actor.kind_name(),
                        kind: relation.kind,
                    });
                }
                self.relations.push((
                    relation.clone(),
                    AcceptedRelationMeta {
                        accept_seq,
                        actor: event.actor.clone(),
                    },
                ));
            }
            EventPayload::DecisionRecorded(decision) => {
                let (target_claim, target) = self.claim_record(decision.target_claim_id)?;
                if target.scope_id != decision.resolution_slot.scope_id {
                    return Err(LedgerError::CrossScope("user decision"));
                }
                if target.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("user decision"));
                }
                if target_claim.subject_entity_id != decision.resolution_slot.subject_entity_id
                    || target_claim.predicate_id != decision.resolution_slot.predicate_id
                    || target_claim.object != decision.target_object
                {
                    return Err(LedgerError::DecisionSemanticMismatch);
                }
                if !self.decision_ids.insert(decision.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "decision",
                        id: decision.id.to_string(),
                    });
                }
                for evidence_id in &decision.rationale_evidence_ids {
                    let evidence = self
                        .evidence
                        .get(evidence_id)
                        .ok_or(LedgerError::UnknownEvidence(*evidence_id))?;
                    let artifact = self
                        .artifacts
                        .get(&evidence.artifact_id)
                        .ok_or(LedgerError::UnknownArtifact(evidence.artifact_id))?;
                    if artifact.descriptor.domain_id != event.domain_id {
                        return Err(LedgerError::CrossDomain("decision evidence"));
                    }
                }
                if let DecisionAction::Replace {
                    replacement_claim_id,
                } = &decision.action
                {
                    let (replacement_claim, replacement) =
                        self.claim_record(*replacement_claim_id)?;
                    if replacement.scope_id != decision.resolution_slot.scope_id {
                        return Err(LedgerError::CrossScope("replacement decision"));
                    }
                    if replacement.domain_id != event.domain_id {
                        return Err(LedgerError::CrossDomain("replacement decision"));
                    }
                    if replacement_claim.subject_entity_id
                        != decision.resolution_slot.subject_entity_id
                        || replacement_claim.predicate_id != decision.resolution_slot.predicate_id
                        || replacement_claim.object == decision.target_object
                    {
                        return Err(LedgerError::DecisionSemanticMismatch);
                    }
                }
                self.decisions
                    .push((decision.clone(), AcceptedDecisionMeta { accept_seq }));
            }
        }
        Ok(())
    }

    fn claim_record(&self, claim_id: ClaimId) -> Result<(&Claim, AcceptedClaimMeta), LedgerError> {
        self.claims
            .get(&claim_id)
            .map(|(claim, metadata)| (claim, *metadata))
            .ok_or(LedgerError::UnknownClaim(claim_id))
    }

    /// Returns the last assigned acceptance sequence, or zero for an empty replica.
    #[must_use]
    pub fn accept_seq_head(&self) -> u64 {
        self.next_accept_seq.saturating_sub(1)
    }

    /// Returns append history in replica acceptance order.
    #[must_use]
    pub fn accepted_events(&self) -> &[AcceptedEvent] {
        &self.accepted_events
    }

    /// Returns a descriptor by immutable identity.
    #[must_use]
    pub fn artifact(&self, id: ArtifactId) -> Option<&ArtifactDescriptor> {
        self.artifacts.get(&id).map(|artifact| &artifact.descriptor)
    }

    /// Returns an evidence item by immutable identity.
    #[must_use]
    pub fn evidence(&self, id: EvidenceId) -> Option<&EvidenceItem> {
        self.evidence.get(&id)
    }

    /// Returns an accepted claim by immutable identity.
    #[must_use]
    pub fn claim(&self, id: ClaimId) -> Option<&Claim> {
        self.claims.get(&id).map(|(claim, _)| claim)
    }

    /// Resolves active and conflicting claims at independent valid/known coordinates.
    #[must_use]
    pub fn resolve(&self, query: &ResolutionQuery) -> ResolutionResult {
        let claims = self
            .claims
            .values()
            .map(|(claim, metadata)| ResolutionClaim {
                claim: claim.clone(),
                accept_seq: metadata.accept_seq,
            })
            .collect::<Vec<_>>();
        let relations = self
            .relations
            .iter()
            .map(|(relation, metadata)| ResolutionRelation {
                relation: relation.clone(),
                accept_seq: metadata.accept_seq,
                actor_kind: ResolverActorKind::from(&metadata.actor),
            })
            .collect::<Vec<_>>();
        let decisions = self
            .decisions
            .iter()
            .map(|(decision, metadata)| ResolutionDecision {
                decision: decision.clone(),
                accept_seq: metadata.accept_seq,
            })
            .collect::<Vec<_>>();
        resolve_snapshot(query, &claims, &relations, &decisions)
    }

    /// Resolves mastery and freshness as separate projections at the same coordinates.
    #[must_use]
    pub fn knowledge_state_as_of(
        &self,
        subject_entity_id: EntityId,
        scope_id: ScopeId,
        mastery_predicate_id: PredicateId,
        freshness_predicate_id: PredicateId,
        valid_at: TimestampMillis,
        known_at_accept_seq: u64,
    ) -> KnowledgeStateView {
        let mastery_resolution = self.resolve(&ResolutionQuery {
            subject_entity_id,
            scope_id,
            predicate_id: mastery_predicate_id,
            valid_at,
            known_at_accept_seq,
            policy: AuthorityPolicy::UserOwned,
        });
        let freshness_resolution = self.resolve(&ResolutionQuery {
            subject_entity_id,
            scope_id,
            predicate_id: freshness_predicate_id,
            valid_at,
            known_at_accept_seq,
            policy: AuthorityPolicy::UserOwned,
        });

        let mastery = mastery_resolution
            .active_claim_ids
            .first()
            .and_then(|id| self.claims.get(id))
            .and_then(|(claim, _)| match claim.object {
                ClaimObject::Mastery(level) => Some(level),
                _ => None,
            });
        let freshness = freshness_resolution
            .active_claim_ids
            .first()
            .and_then(|id| self.claims.get(id))
            .and_then(|(claim, _)| match claim.object {
                ClaimObject::Freshness(band) => Some(band),
                _ => None,
            });

        KnowledgeStateView {
            mastery,
            freshness,
            mastery_resolution,
            freshness_resolution,
        }
    }
}

/// Minimal predicate registry entry needed to validate resolution semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateDefinition {
    pub id: PredicateId,
    pub policy: AuthorityPolicy,
    pub minimum_evidence: u16,
    pub allowed_statuses: BTreeSet<EpistemicStatus>,
}

/// Versioned predicate registry, kept independent of mutable projection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredicateRegistry {
    pub version: u16,
    pub definitions: BTreeMap<PredicateId, PredicateDefinition>,
}

impl PredicateRegistry {
    /// Validates uniqueness, versioning, and the declared evidence floor.
    pub fn validate_claim(&self, claim: &Claim) -> Result<(), RegistryError> {
        if self.version == 0 {
            return Err(RegistryError::InvalidVersion);
        }
        let definition = self
            .definitions
            .get(&claim.predicate_id)
            .ok_or_else(|| RegistryError::UnknownPredicate(claim.predicate_id.clone()))?;
        if !definition
            .allowed_statuses
            .contains(&claim.epistemic_status)
        {
            return Err(RegistryError::StatusNotAllowed(claim.epistemic_status));
        }
        if claim.evidence_ids.len() < usize::from(definition.minimum_evidence) {
            return Err(RegistryError::InsufficientEvidence {
                required: definition.minimum_evidence,
                actual: claim.evidence_ids.len(),
            });
        }
        Ok(())
    }
}

/// Predicate registry validation failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Registry version zero is never writable.
    #[error("predicate registry version must be greater than zero")]
    InvalidVersion,
    /// A claim used an unregistered predicate.
    #[error("unknown predicate {0:?}")]
    UnknownPredicate(PredicateId),
    /// The predicate disallowed this epistemic status.
    #[error("epistemic status {0:?} is not allowed for this predicate")]
    StatusNotAllowed(EpistemicStatus),
    /// A claim did not meet the predicate evidence floor.
    #[error("predicate requires {required} evidence links, got {actual}")]
    InsufficientEvidence { required: u16, actual: usize },
}

/// Constructs an event for tests and deterministic fixture builders.
pub fn event(
    id: EventId,
    origin_seq: u64,
    observed_at: TimestampMillis,
    actor: Actor,
    domain_id: DomainId,
    payload: EventPayload,
) -> Event {
    Event {
        id,
        origin_seq,
        origin_observed_at: observed_at,
        actor,
        domain_id,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
    use academic_domain::{
        ArtifactRepresentation, ConfidencePermille, EvidenceLocator, EvidenceRole,
        EvidenceStrength, MediaType, PermissionLineageId, PredictionMetadata,
        PredictionObservationWindow, ResolutionSlot, RetentionClass, ScopeDescriptor,
        ValidInterval, VaultLocator,
    };
    use ed25519_dalek::SigningKey;

    use super::*;

    fn id<T: FromStr<Err = DomainError>>(suffix: u32) -> Result<T, DomainError> {
        format!("01900000-0000-7000-8000-{suffix:012x}").parse()
    }

    fn fixture_batch() -> Result<UnsignedBatch, DomainError> {
        let domain_id = id(1)?;
        let artifact_id = id(2)?;
        let evidence_id = id(3)?;
        let subject_id = id(4)?;
        let scope_id = id(12)?;
        let user_id = id(13)?;
        let media_type = MediaType::parse("text/plain")?;
        let digest = ContentDigest::sha256(b"synthetic");
        let locator = VaultLocator::derive(b"fixture-domain-key", 1, &media_type, digest)?;
        let evidence_locator = EvidenceLocator::TextBytes {
            source_digest: digest,
            start: 0,
            end: 9,
        };
        let artifact = ArtifactDescriptor {
            id: artifact_id,
            content_digest: digest,
            media_type,
            byte_length: 9,
            domain_id,
            confidentiality: academic_domain::Confidentiality::Personal,
            retention_class: RetentionClass::UserManaged,
            permission_lineage_id: PermissionLineageId::from_str(
                "01900000-0000-7000-8000-000000000005",
            )?,
            format_version: 1,
            vault_locator: locator,
            evidence_representations: vec![ArtifactRepresentation {
                locator: evidence_locator.clone(),
                content_digest: digest,
                byte_length: 9,
            }],
        };
        let evidence = EvidenceItem {
            id: evidence_id,
            artifact_id,
            locator: evidence_locator,
            excerpt_digest: digest,
            role: EvidenceRole::Supports,
            strength: EvidenceStrength::Direct,
            extraction_method: "fixture".to_owned(),
            extractor_version: "1".to_owned(),
        };
        let claim = Claim {
            id: id(6)?,
            subject_entity_id: subject_id,
            predicate_id: PredicateId::parse("knowledge.mastery")?,
            object: ClaimObject::Mastery(MasteryLevel::Practiced),
            scope_id,
            authority_class: AuthorityClass::UserExplicit,
            epistemic_status: EpistemicStatus::UserConfirmed,
            confidence: Some(ConfidencePermille::new(900)?),
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(10)),
            evidence_ids: vec![evidence_id],
        };
        Ok(UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: id(7)?,
            device_id: id(8)?,
            origin_seq_start: 1,
            origin_seq_end: 4,
            previous_batch_hash: None,
            origin_created_at: TimestampMillis::new(20),
            events: vec![
                event(
                    id(9)?,
                    1,
                    TimestampMillis::new(10),
                    Actor::Importer {
                        name: "fixture".to_owned(),
                        version: "1".to_owned(),
                    },
                    domain_id,
                    EventPayload::ScopeRegistered(ScopeDescriptor {
                        id: scope_id,
                        domain_id,
                        label: "fixture.scope".to_owned(),
                    }),
                ),
                event(
                    id(10)?,
                    2,
                    TimestampMillis::new(11),
                    Actor::Importer {
                        name: "fixture".to_owned(),
                        version: "1".to_owned(),
                    },
                    domain_id,
                    EventPayload::ArtifactRegistered(artifact),
                ),
                event(
                    id(11)?,
                    3,
                    TimestampMillis::new(12),
                    Actor::Importer {
                        name: "fixture".to_owned(),
                        version: "1".to_owned(),
                    },
                    domain_id,
                    EventPayload::EvidenceRegistered(evidence),
                ),
                event(
                    id(14)?,
                    4,
                    TimestampMillis::new(13),
                    Actor::User { user_id },
                    domain_id,
                    EventPayload::ClaimAsserted(claim),
                ),
            ],
        })
    }

    fn verify_batch(batch: &UnsignedBatch) -> Result<VerifiedBatch, Box<dyn std::error::Error>> {
        let signing_key = SigningKey::from_bytes(&[1; 32]);
        let authorization =
            DeviceAuthorization::new(batch.device_id, id(13)?, signing_key.verifying_key());
        let signed = sign_batch(batch, &signing_key)?;
        Ok(verify_signed_batch(&signed, &authorization)?)
    }

    fn resolution_claim(
        suffix: u32,
        object: ClaimObject,
        authority_class: AuthorityClass,
        epistemic_status: EpistemicStatus,
        valid_time: ValidInterval,
    ) -> Result<Claim, DomainError> {
        let is_prediction = epistemic_status == EpistemicStatus::Prediction;
        Ok(Claim {
            id: id(suffix)?,
            subject_entity_id: id(4)?,
            predicate_id: PredicateId::parse("test.value")?,
            object,
            scope_id: id(12)?,
            authority_class,
            epistemic_status,
            confidence: is_prediction
                .then(|| ConfidencePermille::new(500))
                .transpose()?,
            prediction_metadata: is_prediction
                .then(|| {
                    PredictionMetadata::new(
                        PredictionObservationWindow::new(
                            TimestampMillis::new(-100),
                            TimestampMillis::new(0),
                        )?,
                        1,
                    )
                })
                .transpose()?,
            valid_time,
            evidence_ids: vec![id(3)?],
        })
    }

    fn insert_claim(
        ledger: &mut LedgerState,
        claim: Claim,
        accept_seq: u64,
    ) -> Result<(), DomainError> {
        ledger.claims.insert(
            claim.id,
            (
                claim.clone(),
                AcceptedClaimMeta {
                    accept_seq,
                    domain_id: id(1)?,
                    scope_id: claim.scope_id,
                },
            ),
        );
        Ok(())
    }

    fn policy_authority(policy: AuthorityPolicy) -> (AuthorityClass, EpistemicStatus) {
        match policy {
            AuthorityPolicy::UserOwned => {
                (AuthorityClass::UserExplicit, EpistemicStatus::UserConfirmed)
            }
            AuthorityPolicy::OfficialFact => {
                (AuthorityClass::Official, EpistemicStatus::OfficialConfirmed)
            }
            AuthorityPolicy::ImplementationObservation => (
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
            ),
            AuthorityPolicy::CuratedRelation => (
                AuthorityClass::Curated,
                EpistemicStatus::DeterministicDerived,
            ),
        }
    }

    fn actor_for_resolution_claim(claim: &Claim) -> Result<Actor, DomainError> {
        Ok(match claim.authority_class {
            AuthorityClass::UserExplicit => Actor::User { user_id: id(13)? },
            AuthorityClass::DeterministicEngine => Actor::DeterministicEngine {
                name: "resolution-engine".to_owned(),
                version: "1".to_owned(),
            },
            AuthorityClass::ModelInference | AuthorityClass::Prediction => {
                Actor::ModelRun { run_id: id(901)? }
            }
            AuthorityClass::Official
            | AuthorityClass::DirectObservation
            | AuthorityClass::Curated
            | AuthorityClass::Unknown => Actor::Importer {
                name: "resolution-importer".to_owned(),
                version: "1".to_owned(),
            },
        })
    }

    fn accept_resolution_batch(
        claims: &[Claim],
        decision: UserDecision,
    ) -> Result<(LedgerState, u64), Box<dyn std::error::Error>> {
        let mut batch = fixture_batch()?;
        batch.batch_id = id(902)?;
        batch.events.truncate(3);
        for (index, claim) in claims.iter().enumerate() {
            let origin_seq = 4 + u64::try_from(index)?;
            batch.events.push(event(
                id(910 + u32::try_from(index)?)?,
                origin_seq,
                TimestampMillis::new(i64::try_from(origin_seq)?),
                actor_for_resolution_claim(claim)?,
                id(1)?,
                EventPayload::ClaimAsserted(claim.clone()),
            ));
        }
        let decision_origin_seq = 4 + u64::try_from(claims.len())?;
        batch.events.push(event(
            id(920)?,
            decision_origin_seq,
            TimestampMillis::new(i64::try_from(decision_origin_seq)?),
            Actor::User { user_id: id(13)? },
            id(1)?,
            EventPayload::DecisionRecorded(decision),
        ));
        batch.origin_seq_end = decision_origin_seq;

        let verified = verify_batch(&batch)?;
        let mut ledger = LedgerState::new();
        let first = ledger.accept_verified_batch(&verified)?;
        let replay = ledger.accept_verified_batch(&verified)?;
        assert!(!first.duplicate);
        assert!(replay.duplicate);
        assert_eq!(first.accept_seq_end, replay.accept_seq_end);
        assert_eq!(ledger.accepted_events().len(), batch.events.len());
        Ok((ledger, first.accept_seq_end))
    }

    #[test]
    fn ledger_acceptance_requires_verified_capability_and_is_idempotent()
    -> Result<(), Box<dyn std::error::Error>> {
        let batch = fixture_batch()?;
        let verified = verify_batch(&batch)?;
        let mut ledger = LedgerState::new();
        let first = ledger.accept_verified_batch(&verified)?;
        let duplicate = ledger.accept_verified_batch(&verified)?;
        assert_eq!(first.accept_seq_start, 1);
        assert_eq!(first.accept_seq_end, 4);
        assert!(!first.duplicate);
        assert!(duplicate.duplicate);
        assert_eq!(ledger.accepted_events().len(), 4);
        Ok(())
    }

    #[test]
    fn prediction_requirements_are_rechecked_at_acceptance_and_resolution_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = fixture_batch()?;
        let (prediction_event, invalid_prediction) = {
            let event = batch
                .events
                .get_mut(3)
                .ok_or("fixture batch must contain its claim event")?;
            event.actor = Actor::ModelRun { run_id: id(40)? };
            let EventPayload::ClaimAsserted(prediction) = &mut event.payload else {
                return Err("fixture batch must contain a claim event".into());
            };
            prediction.authority_class = AuthorityClass::Prediction;
            prediction.epistemic_status = EpistemicStatus::Prediction;
            prediction.confidence = None;
            prediction.prediction_metadata = Some(PredictionMetadata::new(
                PredictionObservationWindow::new(
                    TimestampMillis::new(-20),
                    TimestampMillis::new(0),
                )?,
                2,
            )?);
            let invalid_prediction = prediction.clone();
            let prediction_event = event.clone();
            (prediction_event, invalid_prediction)
        };
        let signing_key = SigningKey::from_bytes(&[1; 32]);
        assert!(matches!(
            sign_batch(&batch, &signing_key),
            Err(academic_contracts::ContractError::Domain(
                DomainError::MissingPredictionConfidence
            ))
        ));

        let mut ledger = LedgerState::new();
        for (index, prerequisite) in batch.events[..3].iter().enumerate() {
            ledger.apply_event(prerequisite, 1 + u64::try_from(index)?)?;
        }
        assert!(matches!(
            ledger.apply_event(&prediction_event, 4),
            Err(LedgerError::Domain(
                DomainError::MissingPredictionConfidence
            ))
        ));
        assert!(ledger.claim(invalid_prediction.id).is_none());

        let mut missing_metadata_event = prediction_event.clone();
        missing_metadata_event.id = id(41)?;
        let EventPayload::ClaimAsserted(missing_metadata) = &mut missing_metadata_event.payload
        else {
            return Err("fixture batch must contain a claim event".into());
        };
        missing_metadata.confidence = Some(ConfidencePermille::new(500)?);
        missing_metadata.prediction_metadata = None;
        let missing_metadata_claim_id = missing_metadata.id;
        assert!(matches!(
            ledger.apply_event(&missing_metadata_event, 4),
            Err(LedgerError::Domain(DomainError::MissingPredictionMetadata))
        ));
        assert!(ledger.claim(missing_metadata_claim_id).is_none());

        // Resolution is defensive even against an impossible in-memory insertion that bypasses
        // signed verification and append validation.
        insert_claim(&mut ledger, invalid_prediction.clone(), 4)?;
        let result = ledger.resolve(&ResolutionQuery {
            subject_entity_id: invalid_prediction.subject_entity_id,
            scope_id: invalid_prediction.scope_id,
            predicate_id: invalid_prediction.predicate_id,
            valid_at: TimestampMillis::new(10),
            known_at_accept_seq: 4,
            policy: AuthorityPolicy::UserOwned,
        });
        assert!(result.active_claim_ids.is_empty());
        assert!(result.conflicting_claim_ids.is_empty());
        assert!(result.rejected_claim_ids.is_empty());
        Ok(())
    }

    #[test]
    fn origin_gap_and_fork_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let batch = fixture_batch()?;
        let mut ledger = LedgerState::new();
        let verified = verify_batch(&batch)?;
        let first = ledger.accept_verified_batch(&verified)?;

        let mut gap = batch.clone();
        gap.batch_id = id(20)?;
        gap.origin_seq_start = 6;
        gap.origin_seq_end = 9;
        gap.previous_batch_hash = Some(first.batch_hash);
        for (index, item) in gap.events.iter_mut().enumerate() {
            item.id = id(30 + u32::try_from(index)?)?;
            item.origin_seq = 6 + u64::try_from(index)?;
        }
        let verified_gap = verify_batch(&gap)?;
        assert!(matches!(
            ledger.accept_verified_batch(&verified_gap),
            Err(LedgerError::OriginGap { .. })
        ));

        gap.origin_seq_start = 5;
        gap.origin_seq_end = 8;
        gap.previous_batch_hash = Some(ContentDigest::sha256(b"wrong-parent"));
        for (index, item) in gap.events.iter_mut().enumerate() {
            item.origin_seq = 5 + u64::try_from(index)?;
        }
        let verified_fork = verify_batch(&gap)?;
        assert!(matches!(
            ledger.accept_verified_batch(&verified_fork),
            Err(LedgerError::DeviceFork)
        ));
        Ok(())
    }

    #[test]
    fn evidence_without_exact_registered_representation_fails_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let base = fixture_batch()?;
        let digest = ContentDigest::sha256(b"synthetic");
        let unsupported = [
            EvidenceLocator::TextBytes {
                source_digest: digest,
                start: 0,
                end: 8,
            },
            EvidenceLocator::Page { page_number: 1 },
            EvidenceLocator::TranscriptTime {
                start_ms: 1,
                end_ms: 2,
            },
            EvidenceLocator::RepositoryBytes {
                snapshot_digest: digest,
                path: academic_domain::LogicalPath::parse("src/lib.rs")?,
                start: 0,
                end: 1,
            },
        ];
        for locator in unsupported {
            let mut batch = base.clone();
            let EventPayload::EvidenceRegistered(item) = &mut batch.events[2].payload else {
                return Err("fixture evidence event missing".into());
            };
            item.locator = locator;
            let verified = verify_batch(&batch)?;
            assert!(matches!(
                LedgerState::new().accept_verified_batch(&verified),
                Err(LedgerError::UnprovenEvidenceRepresentation(_))
            ));
        }
        let mut wrong_digest = base;
        let EventPayload::EvidenceRegistered(item) = &mut wrong_digest.events[2].payload else {
            return Err("fixture evidence event missing".into());
        };
        item.excerpt_digest = ContentDigest::sha256(b"different representation");
        let verified = verify_batch(&wrong_digest)?;
        assert!(matches!(
            LedgerState::new().accept_verified_batch(&verified),
            Err(LedgerError::UnprovenEvidenceRepresentation(_))
        ));
        Ok(())
    }

    #[test]
    fn evidence_and_claim_cross_domain_or_unknown_scope_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut cross_domain = fixture_batch()?;
        cross_domain.events[2].domain_id = id(90)?;
        let verified = verify_batch(&cross_domain)?;
        assert!(matches!(
            LedgerState::new().accept_verified_batch(&verified),
            Err(LedgerError::CrossDomain("evidence artifact"))
        ));

        let mut unknown_scope = fixture_batch()?;
        let EventPayload::ClaimAsserted(claim) = &mut unknown_scope.events[3].payload else {
            return Err("fixture claim event missing".into());
        };
        claim.scope_id = id(91)?;
        let verified = verify_batch(&unknown_scope)?;
        assert!(matches!(
            LedgerState::new().accept_verified_batch(&verified),
            Err(LedgerError::UnknownScope(_))
        ));
        Ok(())
    }

    #[test]
    fn page_time_and_repository_evidence_fails_closed_without_a_byte_resolver()
    -> Result<(), Box<dyn std::error::Error>> {
        let digest = ContentDigest::sha256(b"synthetic");
        let representations = [
            (EvidenceLocator::Page { page_number: 1 }, 9),
            (
                EvidenceLocator::TranscriptTime {
                    start_ms: 10,
                    end_ms: 20,
                },
                9,
            ),
            (
                EvidenceLocator::RepositoryBytes {
                    snapshot_digest: digest,
                    path: academic_domain::LogicalPath::parse("src/lib.rs")?,
                    start: 2,
                    end: 5,
                },
                3,
            ),
        ];
        for (locator, byte_length) in representations {
            let mut batch = fixture_batch()?;
            let EventPayload::ArtifactRegistered(artifact) = &mut batch.events[1].payload else {
                return Err("fixture artifact event missing".into());
            };
            artifact.evidence_representations = vec![ArtifactRepresentation {
                locator: locator.clone(),
                content_digest: digest,
                byte_length,
            }];
            let EventPayload::EvidenceRegistered(evidence) = &mut batch.events[2].payload else {
                return Err("fixture evidence event missing".into());
            };
            evidence.locator = locator;
            assert!(verify_batch(&batch).is_err());
        }
        Ok(())
    }

    #[test]
    fn artifact_text_representation_cannot_exceed_registered_byte_length()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = fixture_batch()?;
        let EventPayload::ArtifactRegistered(artifact) = &mut batch.events[1].payload else {
            return Err("fixture artifact event missing".into());
        };
        artifact.evidence_representations[0].locator = EvidenceLocator::TextBytes {
            source_digest: artifact.content_digest,
            start: 0,
            end: artifact.byte_length + 1,
        };
        artifact.evidence_representations[0].byte_length = artifact.byte_length + 1;
        assert!(verify_batch(&batch).is_err());
        Ok(())
    }

    #[test]
    fn t007_forged_full_span_representation_digest_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = fixture_batch()?;
        let forged = ContentDigest::sha256(b"forged representation");
        let EventPayload::ArtifactRegistered(artifact) = &mut batch.events[1].payload else {
            return Err("fixture artifact event missing".into());
        };
        artifact.evidence_representations[0].content_digest = forged;
        let EventPayload::EvidenceRegistered(evidence) = &mut batch.events[2].payload else {
            return Err("fixture evidence event missing".into());
        };
        evidence.excerpt_digest = forged;
        assert!(verify_batch(&batch).is_err());
        Ok(())
    }

    #[test]
    fn t007_partial_and_derived_representations_fail_closed_without_verifier_capability()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = fixture_batch()?;
        let partial_digest = ContentDigest::sha256(b"syntheti");
        let partial_locator = EvidenceLocator::TextBytes {
            source_digest: ContentDigest::sha256(b"synthetic"),
            start: 0,
            end: 8,
        };
        let EventPayload::ArtifactRegistered(artifact) = &mut batch.events[1].payload else {
            return Err("fixture artifact event missing".into());
        };
        artifact.evidence_representations[0] = ArtifactRepresentation {
            locator: partial_locator.clone(),
            content_digest: partial_digest,
            byte_length: 8,
        };
        let EventPayload::EvidenceRegistered(evidence) = &mut batch.events[2].payload else {
            return Err("fixture evidence event missing".into());
        };
        evidence.locator = partial_locator;
        evidence.excerpt_digest = partial_digest;

        assert!(verify_batch(&batch).is_err());
        batch.events[1].actor = Actor::DeterministicEngine {
            name: "fixture.byte-resolver".to_owned(),
            version: "1".to_owned(),
        };
        assert!(
            verify_batch(&batch).is_err(),
            "a caller-selected engine actor is not a byte-resolver capability"
        );
        Ok(())
    }

    #[test]
    fn t007_user_reject_decision_survives_fresh_claim_id_rerun_and_known_time()
    -> Result<(), Box<dyn std::error::Error>> {
        let decision_interval =
            ValidInterval::new(TimestampMillis::new(0), Some(TimestampMillis::new(100)))?;
        let claim_interval = ValidInterval::open_ended(TimestampMillis::new(0));
        let first = resolution_claim(
            200,
            ClaimObject::Text("dismissed".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            claim_interval,
        )?;
        let rerun = resolution_claim(
            201,
            first.object.clone(),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            claim_interval,
        )?;
        let contrary = resolution_claim(
            202,
            ClaimObject::Text("contrary".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            claim_interval,
        )?;
        let mut ledger = LedgerState::new();
        insert_claim(&mut ledger, first.clone(), 1)?;
        ledger.decisions.push((
            UserDecision {
                id: id(203)?,
                target_claim_id: first.id,
                target_object: first.object.clone(),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: first.subject_entity_id,
                    predicate_id: first.predicate_id.clone(),
                    scope_id: first.scope_id,
                },
                action: DecisionAction::Reject,
                valid_time: decision_interval,
                rationale_evidence_ids: Vec::new(),
                decided_at: TimestampMillis::new(1),
                reversible_until: None,
            },
            AcceptedDecisionMeta { accept_seq: 2 },
        ));
        insert_claim(&mut ledger, rerun.clone(), 3)?;
        insert_claim(&mut ledger, contrary.clone(), 4)?;

        let before_known = ledger.resolve(&ResolutionQuery {
            subject_entity_id: first.subject_entity_id,
            scope_id: first.scope_id,
            predicate_id: first.predicate_id.clone(),
            valid_at: TimestampMillis::new(10),
            known_at_accept_seq: 1,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(before_known.active_claim_ids, vec![first.id]);

        let after_rerun = ledger.resolve(&ResolutionQuery {
            subject_entity_id: first.subject_entity_id,
            scope_id: first.scope_id,
            predicate_id: first.predicate_id.clone(),
            valid_at: TimestampMillis::new(10),
            known_at_accept_seq: 4,
            policy: AuthorityPolicy::UserOwned,
        });
        assert!(after_rerun.active_claim_ids.is_empty());
        assert_eq!(after_rerun.rejected_claim_ids, vec![first.id, rerun.id],);
        assert_eq!(after_rerun.conflicting_claim_ids, vec![contrary.id]);

        let after_expiry = ledger.resolve(&ResolutionQuery {
            subject_entity_id: first.subject_entity_id,
            scope_id: first.scope_id,
            predicate_id: first.predicate_id.clone(),
            valid_at: TimestampMillis::new(100),
            known_at_accept_seq: 3,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(after_expiry.active_claim_ids, vec![first.id, rerun.id]);
        assert!(after_expiry.rejected_claim_ids.is_empty());
        assert!(after_expiry.conflicting_claim_ids.is_empty());
        Ok(())
    }

    #[test]
    fn t010_decisions_preserve_predicate_authority_across_actions_and_time_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        let claim_interval =
            ValidInterval::new(TimestampMillis::new(0), Some(TimestampMillis::new(30)))?;
        let decision_interval =
            ValidInterval::new(TimestampMillis::new(10), Some(TimestampMillis::new(20)))?;
        for policy in [
            AuthorityPolicy::UserOwned,
            AuthorityPolicy::OfficialFact,
            AuthorityPolicy::ImplementationObservation,
            AuthorityPolicy::CuratedRelation,
        ] {
            let (strong_authority, strong_status) = policy_authority(policy);
            for action_name in ["confirm", "reject", "replace"] {
                let target = resolution_claim(
                    260,
                    ClaimObject::Text("target".to_owned()),
                    AuthorityClass::ModelInference,
                    EpistemicStatus::AiInferred,
                    claim_interval,
                )?;
                let replacement = resolution_claim(
                    261,
                    ClaimObject::Text("replacement".to_owned()),
                    AuthorityClass::ModelInference,
                    EpistemicStatus::AiInferred,
                    claim_interval,
                )?;
                let policy_authoritative = resolution_claim(
                    262,
                    ClaimObject::Text("unrelated-policy-authority".to_owned()),
                    strong_authority,
                    strong_status,
                    claim_interval,
                )?;
                let action = match action_name {
                    "confirm" => DecisionAction::Confirm,
                    "reject" => DecisionAction::Reject,
                    "replace" => DecisionAction::Replace {
                        replacement_claim_id: replacement.id,
                    },
                    _ => return Err("unexpected decision action".into()),
                };
                let (ledger, decision_accept_seq) = accept_resolution_batch(
                    &[
                        target.clone(),
                        replacement.clone(),
                        policy_authoritative.clone(),
                    ],
                    UserDecision {
                        id: id(263)?,
                        target_claim_id: target.id,
                        target_object: target.object.clone(),
                        resolution_slot: ResolutionSlot {
                            subject_entity_id: target.subject_entity_id,
                            predicate_id: target.predicate_id.clone(),
                            scope_id: target.scope_id,
                        },
                        action,
                        valid_time: decision_interval,
                        rationale_evidence_ids: Vec::new(),
                        decided_at: TimestampMillis::new(10),
                        reversible_until: None,
                    },
                )?;

                for (label, valid_at, known_at) in [
                    (
                        "before-known",
                        TimestampMillis::new(15),
                        decision_accept_seq - 1,
                    ),
                    ("before-valid", TimestampMillis::new(9), decision_accept_seq),
                    ("after-valid", TimestampMillis::new(20), decision_accept_seq),
                ] {
                    let outside = ledger.resolve(&ResolutionQuery {
                        subject_entity_id: target.subject_entity_id,
                        scope_id: target.scope_id,
                        predicate_id: target.predicate_id.clone(),
                        valid_at,
                        known_at_accept_seq: known_at,
                        policy,
                    });
                    assert_eq!(
                        outside.active_claim_ids,
                        vec![policy_authoritative.id],
                        "{policy:?}/{action_name}/{label}",
                    );
                    assert_eq!(
                        outside.conflicting_claim_ids,
                        vec![target.id, replacement.id],
                        "{policy:?}/{action_name}/{label}",
                    );
                    assert_eq!(
                        outside.rejected_claim_ids,
                        Vec::<ClaimId>::new(),
                        "{policy:?}/{action_name}/{label}",
                    );
                }

                let applicable = ledger.resolve(&ResolutionQuery {
                    subject_entity_id: target.subject_entity_id,
                    scope_id: target.scope_id,
                    predicate_id: target.predicate_id.clone(),
                    valid_at: TimestampMillis::new(10),
                    known_at_accept_seq: decision_accept_seq,
                    policy,
                });
                let equal_user_rank = matches!(
                    policy,
                    AuthorityPolicy::UserOwned | AuthorityPolicy::CuratedRelation
                );
                let (expected_active, expected_conflicting, expected_rejected) = match action_name {
                    "confirm" if equal_user_rank => (
                        Vec::new(),
                        vec![target.id, replacement.id, policy_authoritative.id],
                        Vec::new(),
                    ),
                    "confirm" => (
                        vec![policy_authoritative.id],
                        vec![target.id, replacement.id],
                        Vec::new(),
                    ),
                    "reject" => (
                        vec![policy_authoritative.id],
                        vec![replacement.id],
                        vec![target.id],
                    ),
                    "replace" if equal_user_rank => (
                        Vec::new(),
                        vec![replacement.id, policy_authoritative.id],
                        vec![target.id],
                    ),
                    "replace" => (
                        vec![policy_authoritative.id],
                        vec![replacement.id],
                        vec![target.id],
                    ),
                    _ => return Err("unexpected decision action".into()),
                };
                assert_eq!(
                    applicable.active_claim_ids, expected_active,
                    "{policy:?}/{action_name}",
                );
                assert_eq!(
                    applicable.conflicting_claim_ids, expected_conflicting,
                    "{policy:?}/{action_name}",
                );
                assert_eq!(
                    applicable.rejected_claim_ids, expected_rejected,
                    "{policy:?}/{action_name}",
                );
            }
        }
        Ok(())
    }

    #[test]
    fn t013_confirm_and_replace_preserve_same_object_policy_authority()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        for policy in [
            AuthorityPolicy::UserOwned,
            AuthorityPolicy::OfficialFact,
            AuthorityPolicy::ImplementationObservation,
            AuthorityPolicy::CuratedRelation,
        ] {
            let (same_object_authority, same_object_status) = policy_authority(policy);
            let (unrelated_authority, unrelated_status) = match policy {
                AuthorityPolicy::UserOwned | AuthorityPolicy::OfficialFact => (
                    AuthorityClass::DirectObservation,
                    EpistemicStatus::CodeObserved,
                ),
                AuthorityPolicy::ImplementationObservation => {
                    (AuthorityClass::UserExplicit, EpistemicStatus::UserConfirmed)
                }
                AuthorityPolicy::CuratedRelation => {
                    (AuthorityClass::Official, EpistemicStatus::OfficialConfirmed)
                }
            };
            for action_name in ["confirm", "replace"] {
                let target = resolution_claim(
                    280,
                    ClaimObject::Text("target".to_owned()),
                    AuthorityClass::ModelInference,
                    EpistemicStatus::AiInferred,
                    interval,
                )?;
                let replacement = resolution_claim(
                    281,
                    ClaimObject::Text("replacement".to_owned()),
                    AuthorityClass::ModelInference,
                    EpistemicStatus::AiInferred,
                    interval,
                )?;
                let chosen = if action_name == "confirm" {
                    &target
                } else {
                    &replacement
                };
                let same_object = resolution_claim(
                    282,
                    chosen.object.clone(),
                    same_object_authority,
                    same_object_status,
                    interval,
                )?;
                let unrelated = resolution_claim(
                    283,
                    ClaimObject::Text("unrelated".to_owned()),
                    unrelated_authority,
                    unrelated_status,
                    interval,
                )?;
                let action = if action_name == "confirm" {
                    DecisionAction::Confirm
                } else {
                    DecisionAction::Replace {
                        replacement_claim_id: replacement.id,
                    }
                };
                let (ledger, decision_accept_seq) = accept_resolution_batch(
                    &[
                        target.clone(),
                        replacement.clone(),
                        same_object.clone(),
                        unrelated.clone(),
                    ],
                    UserDecision {
                        id: id(284)?,
                        target_claim_id: target.id,
                        target_object: target.object.clone(),
                        resolution_slot: ResolutionSlot {
                            subject_entity_id: target.subject_entity_id,
                            predicate_id: target.predicate_id.clone(),
                            scope_id: target.scope_id,
                        },
                        action,
                        valid_time: interval,
                        rationale_evidence_ids: Vec::new(),
                        decided_at: TimestampMillis::new(1),
                        reversible_until: None,
                    },
                )?;
                let actual = ledger.resolve(&ResolutionQuery {
                    subject_entity_id: target.subject_entity_id,
                    scope_id: target.scope_id,
                    predicate_id: target.predicate_id.clone(),
                    valid_at: TimestampMillis::new(1),
                    known_at_accept_seq: decision_accept_seq,
                    policy,
                });
                let user_rank = authority_rank(policy, AuthorityClass::UserExplicit);
                let original_rank = authority_rank(policy, same_object.authority_class);
                let expected_active = if original_rank > user_rank {
                    vec![same_object.id]
                } else {
                    vec![chosen.id, same_object.id]
                };
                assert_eq!(
                    actual.active_claim_ids, expected_active,
                    "{policy:?}/{action_name}: the decision rank must be a floor, not a downgrade"
                );
                assert_eq!(
                    actual.conflicting_claim_ids,
                    if action_name == "confirm" {
                        vec![replacement.id, unrelated.id]
                    } else {
                        vec![unrelated.id]
                    },
                    "{policy:?}/{action_name}"
                );
                assert_eq!(
                    actual.rejected_claim_ids,
                    if action_name == "replace" {
                        vec![target.id]
                    } else {
                        Vec::new()
                    },
                    "{policy:?}/{action_name}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn t010_reject_is_object_scoped_without_reactivating_weaker_alternatives()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        for policy in [
            AuthorityPolicy::UserOwned,
            AuthorityPolicy::OfficialFact,
            AuthorityPolicy::ImplementationObservation,
            AuthorityPolicy::CuratedRelation,
        ] {
            let rejected_target = resolution_claim(
                270,
                ClaimObject::Text("rejected-target".to_owned()),
                AuthorityClass::ModelInference,
                EpistemicStatus::AiInferred,
                interval,
            )?;
            let unaffected = resolution_claim(
                271,
                ClaimObject::Text("unaffected-alternative".to_owned()),
                AuthorityClass::ModelInference,
                EpistemicStatus::AiInferred,
                interval,
            )?;
            let mut ledger = LedgerState::new();
            insert_claim(&mut ledger, rejected_target.clone(), 1)?;
            insert_claim(&mut ledger, unaffected.clone(), 2)?;
            ledger.decisions.push((
                UserDecision {
                    id: id(272)?,
                    target_claim_id: rejected_target.id,
                    target_object: rejected_target.object.clone(),
                    resolution_slot: ResolutionSlot {
                        subject_entity_id: rejected_target.subject_entity_id,
                        predicate_id: rejected_target.predicate_id.clone(),
                        scope_id: rejected_target.scope_id,
                    },
                    action: DecisionAction::Reject,
                    valid_time: interval,
                    rationale_evidence_ids: Vec::new(),
                    decided_at: TimestampMillis::new(1),
                    reversible_until: None,
                },
                AcceptedDecisionMeta { accept_seq: 3 },
            ));

            let result = ledger.resolve(&ResolutionQuery {
                subject_entity_id: rejected_target.subject_entity_id,
                scope_id: rejected_target.scope_id,
                predicate_id: rejected_target.predicate_id.clone(),
                valid_at: TimestampMillis::new(1),
                known_at_accept_seq: 3,
                policy,
            });
            assert!(result.active_claim_ids.is_empty(), "{policy:?}");
            assert_eq!(
                result.rejected_claim_ids,
                vec![rejected_target.id],
                "{policy:?}",
            );
            assert_eq!(
                result.conflicting_claim_ids,
                vec![unaffected.id],
                "{policy:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn t007_distinct_object_decisions_compose_without_erasing_prior_rejection()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        let dismissed = resolution_claim(
            204,
            ClaimObject::Text("dismissed".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let selected = resolution_claim(
            205,
            ClaimObject::Text("selected".to_owned()),
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed,
            interval,
        )?;
        let rerun = resolution_claim(
            206,
            dismissed.object.clone(),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let mut ledger = LedgerState::new();
        insert_claim(&mut ledger, dismissed.clone(), 1)?;
        insert_claim(&mut ledger, selected.clone(), 2)?;
        ledger.decisions.push((
            UserDecision {
                id: id(207)?,
                target_claim_id: dismissed.id,
                target_object: dismissed.object.clone(),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: dismissed.subject_entity_id,
                    predicate_id: dismissed.predicate_id.clone(),
                    scope_id: dismissed.scope_id,
                },
                action: DecisionAction::Reject,
                valid_time: interval,
                rationale_evidence_ids: Vec::new(),
                decided_at: TimestampMillis::new(1),
                reversible_until: None,
            },
            AcceptedDecisionMeta { accept_seq: 3 },
        ));
        ledger.decisions.push((
            UserDecision {
                id: id(208)?,
                target_claim_id: selected.id,
                target_object: selected.object.clone(),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: selected.subject_entity_id,
                    predicate_id: selected.predicate_id.clone(),
                    scope_id: selected.scope_id,
                },
                action: DecisionAction::Confirm,
                valid_time: interval,
                rationale_evidence_ids: Vec::new(),
                decided_at: TimestampMillis::new(2),
                reversible_until: None,
            },
            AcceptedDecisionMeta { accept_seq: 4 },
        ));
        insert_claim(&mut ledger, rerun.clone(), 5)?;

        let result = ledger.resolve(&ResolutionQuery {
            subject_entity_id: dismissed.subject_entity_id,
            scope_id: dismissed.scope_id,
            predicate_id: dismissed.predicate_id.clone(),
            valid_at: TimestampMillis::new(10),
            known_at_accept_seq: 5,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(result.active_claim_ids, vec![selected.id]);
        assert_eq!(result.rejected_claim_ids, vec![dismissed.id, rerun.id]);
        assert!(result.conflicting_claim_ids.is_empty());
        Ok(())
    }

    #[test]
    fn t007_replacement_decision_preserves_adjacent_valid_time_handoff()
    -> Result<(), Box<dyn std::error::Error>> {
        let before = ValidInterval::new(TimestampMillis::new(0), Some(TimestampMillis::new(10)))?;
        let after = ValidInterval::new(TimestampMillis::new(10), Some(TimestampMillis::new(20)))?;
        let first = resolution_claim(
            210,
            ClaimObject::Text("A".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            before,
        )?;
        let replacement = resolution_claim(
            211,
            ClaimObject::Text("B".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            after,
        )?;
        let replacement_rerun = resolution_claim(
            212,
            replacement.object.clone(),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            after,
        )?;
        let mut ledger = LedgerState::new();
        insert_claim(&mut ledger, first.clone(), 1)?;
        insert_claim(&mut ledger, replacement.clone(), 2)?;
        ledger.decisions.push((
            UserDecision {
                id: id(213)?,
                target_claim_id: first.id,
                target_object: first.object.clone(),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: first.subject_entity_id,
                    predicate_id: first.predicate_id.clone(),
                    scope_id: first.scope_id,
                },
                action: DecisionAction::Replace {
                    replacement_claim_id: replacement.id,
                },
                valid_time: after,
                rationale_evidence_ids: Vec::new(),
                decided_at: TimestampMillis::new(2),
                reversible_until: None,
            },
            AcceptedDecisionMeta { accept_seq: 3 },
        ));
        insert_claim(&mut ledger, replacement_rerun.clone(), 4)?;

        let before_handoff = ledger.resolve(&ResolutionQuery {
            subject_entity_id: first.subject_entity_id,
            scope_id: first.scope_id,
            predicate_id: first.predicate_id.clone(),
            valid_at: TimestampMillis::new(9),
            known_at_accept_seq: 4,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(before_handoff.active_claim_ids, vec![first.id]);

        let after_handoff = ledger.resolve(&ResolutionQuery {
            subject_entity_id: first.subject_entity_id,
            scope_id: first.scope_id,
            predicate_id: first.predicate_id.clone(),
            valid_at: TimestampMillis::new(10),
            known_at_accept_seq: 4,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(
            after_handoff.active_claim_ids,
            vec![replacement.id, replacement_rerun.id],
        );
        assert!(after_handoff.conflicting_claim_ids.is_empty());
        Ok(())
    }

    #[test]
    fn t007_replacement_must_belong_to_the_same_semantic_slot()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        let first = resolution_claim(
            220,
            ClaimObject::Text("A".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let mut replacement = resolution_claim(
            221,
            ClaimObject::Text("B".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        replacement.predicate_id = PredicateId::parse("other.value")?;
        let mut ledger = LedgerState::new();
        insert_claim(&mut ledger, first.clone(), 1)?;
        insert_claim(&mut ledger, replacement.clone(), 2)?;
        let decision = UserDecision {
            id: id(222)?,
            target_claim_id: first.id,
            target_object: first.object.clone(),
            resolution_slot: ResolutionSlot {
                subject_entity_id: first.subject_entity_id,
                predicate_id: first.predicate_id.clone(),
                scope_id: first.scope_id,
            },
            action: DecisionAction::Replace {
                replacement_claim_id: replacement.id,
            },
            valid_time: interval,
            rationale_evidence_ids: Vec::new(),
            decided_at: TimestampMillis::new(1),
            reversible_until: None,
        };
        let decision_event = event(
            id(223)?,
            3,
            TimestampMillis::new(2),
            Actor::User { user_id: id(13)? },
            id(1)?,
            EventPayload::DecisionRecorded(decision),
        );
        assert!(matches!(
            ledger.apply_event(&decision_event, 3),
            Err(LedgerError::DecisionSemanticMismatch)
        ));
        Ok(())
    }

    #[test]
    fn t007_model_supersession_of_user_confirmed_claim_is_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut batch = fixture_batch()?;
        let EventPayload::ClaimAsserted(user_claim) = &batch.events[3].payload else {
            return Err("fixture claim event missing".into());
        };
        let user_claim = user_claim.clone();
        let mut model_claim = user_claim.clone();
        model_claim.id = id(230)?;
        model_claim.object = ClaimObject::Mastery(MasteryLevel::Fluent);
        model_claim.authority_class = AuthorityClass::ModelInference;
        model_claim.epistemic_status = EpistemicStatus::AiInferred;
        model_claim.confidence = Some(ConfidencePermille::new(800)?);
        let model_actor = Actor::ModelRun { run_id: id(231)? };
        batch.events.push(event(
            id(232)?,
            5,
            TimestampMillis::new(14),
            model_actor.clone(),
            id(1)?,
            EventPayload::ClaimAsserted(model_claim.clone()),
        ));
        batch.events.push(event(
            id(233)?,
            6,
            TimestampMillis::new(15),
            model_actor,
            id(1)?,
            EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: model_claim.id,
                target_claim_id: user_claim.id,
                kind: ClaimRelationKind::Supersedes,
                scope_id: user_claim.scope_id,
            }),
        ));
        batch.origin_seq_end = 6;
        let verified = verify_batch(&batch)?;
        assert!(matches!(
            LedgerState::new().accept_verified_batch(&verified),
            Err(LedgerError::UnauthorizedRelationEffect { .. })
        ));
        Ok(())
    }

    #[test]
    fn t007_state_removing_relation_matrix_checks_both_claims_fail_closed()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        let model_source = resolution_claim(
            234,
            ClaimObject::Text("new".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let model_target = resolution_claim(
            235,
            ClaimObject::Text("old".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let model_actor = Actor::ModelRun { run_id: id(236)? };
        assert!(relation_effect_is_authorized(
            &model_actor,
            ClaimRelationKind::Supersedes,
            &model_source,
            &model_target,
        ));
        assert!(relation_effect_is_authorized(
            &model_actor,
            ClaimRelationKind::Contradicts,
            &model_source,
            &model_target,
        ));

        let user_target = resolution_claim(
            237,
            ClaimObject::Text("user".to_owned()),
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed,
            interval,
        )?;
        let prediction_target = resolution_claim(
            238,
            ClaimObject::Text("prediction".to_owned()),
            AuthorityClass::Prediction,
            EpistemicStatus::Prediction,
            interval,
        )?;
        let mut terminal_source = model_source.clone();
        terminal_source.epistemic_status = EpistemicStatus::Disputed;
        let mut terminal_target = model_target.clone();
        terminal_target.epistemic_status = EpistemicStatus::Superseded;
        let engine_actor = Actor::DeterministicEngine {
            name: "test.engine".to_owned(),
            version: "1".to_owned(),
        };
        for (name, actor, source, target) in [
            (
                "model cannot remove user-confirmed state",
                &model_actor,
                &model_source,
                &user_target,
            ),
            (
                "model inference cannot remove prediction authority",
                &model_actor,
                &model_source,
                &prediction_target,
            ),
            (
                "terminal source cannot remove state",
                &model_actor,
                &terminal_source,
                &model_target,
            ),
            (
                "terminal target cannot be state-removed again",
                &model_actor,
                &model_source,
                &terminal_target,
            ),
            (
                "actor must own the exact source and target status class",
                &engine_actor,
                &model_source,
                &model_target,
            ),
        ] {
            assert!(
                !relation_effect_is_authorized(actor, ClaimRelationKind::Retracts, source, target,),
                "{name}"
            );
        }
        Ok(())
    }

    #[test]
    fn t007_relation_actor_provenance_is_preserved_and_override_protects_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        let target = resolution_claim(
            240,
            ClaimObject::Text("kept".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let source = resolution_claim(
            241,
            ClaimObject::Text("later".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::AiInferred,
            interval,
        )?;
        let actor = Actor::ModelRun { run_id: id(242)? };
        let mut ledger = LedgerState::new();
        insert_claim(&mut ledger, target.clone(), 1)?;
        insert_claim(&mut ledger, source.clone(), 2)?;
        ledger.relations.push((
            ClaimRelation {
                source_claim_id: source.id,
                target_claim_id: target.id,
                kind: ClaimRelationKind::Supersedes,
                scope_id: target.scope_id,
            },
            AcceptedRelationMeta {
                accept_seq: 3,
                actor: actor.clone(),
            },
        ));
        ledger.decisions.push((
            UserDecision {
                id: id(243)?,
                target_claim_id: target.id,
                target_object: target.object.clone(),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: target.subject_entity_id,
                    predicate_id: target.predicate_id.clone(),
                    scope_id: target.scope_id,
                },
                action: DecisionAction::Confirm,
                valid_time: interval,
                rationale_evidence_ids: Vec::new(),
                decided_at: TimestampMillis::new(2),
                reversible_until: None,
            },
            AcceptedDecisionMeta { accept_seq: 4 },
        ));
        assert_eq!(ledger.relations[0].1.actor, actor);
        let result = ledger.resolve(&ResolutionQuery {
            subject_entity_id: target.subject_entity_id,
            scope_id: target.scope_id,
            predicate_id: target.predicate_id.clone(),
            valid_at: TimestampMillis::new(1),
            known_at_accept_seq: 4,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(result.active_claim_ids, vec![target.id]);
        assert_eq!(result.conflicting_claim_ids, vec![source.id]);
        assert!(result.rejected_claim_ids.is_empty());
        Ok(())
    }

    #[test]
    fn t007_standalone_terminal_lifecycle_claims_never_activate()
    -> Result<(), Box<dyn std::error::Error>> {
        let interval = ValidInterval::open_ended(TimestampMillis::new(0));
        let superseded = resolution_claim(
            250,
            ClaimObject::Text("old".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::Superseded,
            interval,
        )?;
        let disputed = resolution_claim(
            251,
            ClaimObject::Text("uncertain".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::Disputed,
            interval,
        )?;
        for (claim, expected_rejected, expected_conflict) in [
            (superseded.clone(), vec![superseded.id], Vec::new()),
            (disputed.clone(), Vec::new(), vec![disputed.id]),
        ] {
            let mut ledger = LedgerState::new();
            insert_claim(&mut ledger, claim.clone(), 1)?;
            let result = ledger.resolve(&ResolutionQuery {
                subject_entity_id: claim.subject_entity_id,
                scope_id: claim.scope_id,
                predicate_id: claim.predicate_id.clone(),
                valid_at: TimestampMillis::new(1),
                known_at_accept_seq: 1,
                policy: AuthorityPolicy::UserOwned,
            });
            assert!(result.active_claim_ids.is_empty());
            assert_eq!(result.rejected_claim_ids, expected_rejected);
            assert_eq!(result.conflicting_claim_ids, expected_conflict);
        }

        let corroborating = resolution_claim(
            252,
            disputed.object.clone(),
            AuthorityClass::UserExplicit,
            EpistemicStatus::UserConfirmed,
            interval,
        )?;
        let mut ledger = LedgerState::new();
        insert_claim(&mut ledger, corroborating.clone(), 1)?;
        insert_claim(&mut ledger, disputed.clone(), 2)?;
        let result = ledger.resolve(&ResolutionQuery {
            subject_entity_id: disputed.subject_entity_id,
            scope_id: disputed.scope_id,
            predicate_id: disputed.predicate_id.clone(),
            valid_at: TimestampMillis::new(1),
            known_at_accept_seq: 2,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(result.active_claim_ids, vec![corroborating.id]);
        assert_eq!(result.conflicting_claim_ids, vec![disputed.id]);

        let contrary_disputed = resolution_claim(
            253,
            ClaimObject::Text("contrary".to_owned()),
            AuthorityClass::ModelInference,
            EpistemicStatus::Disputed,
            interval,
        )?;
        insert_claim(&mut ledger, contrary_disputed.clone(), 3)?;
        let conflicting = ledger.resolve(&ResolutionQuery {
            subject_entity_id: disputed.subject_entity_id,
            scope_id: disputed.scope_id,
            predicate_id: disputed.predicate_id.clone(),
            valid_at: TimestampMillis::new(1),
            known_at_accept_seq: 3,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(conflicting.active_claim_ids, vec![corroborating.id]);
        assert_eq!(
            conflicting.conflicting_claim_ids,
            vec![disputed.id, contrary_disputed.id]
        );
        Ok(())
    }

    #[test]
    fn resolution_ignores_cross_scope_claims_relations_and_decisions()
    -> Result<(), Box<dyn std::error::Error>> {
        let batch = fixture_batch()?;
        let EventPayload::ClaimAsserted(first) = &batch.events[3].payload else {
            return Err("fixture claim event missing".into());
        };
        let mut second = first.clone();
        second.id = id(92)?;
        second.scope_id = id(93)?;
        second.object = ClaimObject::Mastery(MasteryLevel::Fluent);

        let mut ledger = LedgerState::new();
        ledger.claims.insert(
            first.id,
            (
                first.clone(),
                AcceptedClaimMeta {
                    accept_seq: 1,
                    domain_id: id(1)?,
                    scope_id: first.scope_id,
                },
            ),
        );
        ledger.claims.insert(
            second.id,
            (
                second.clone(),
                AcceptedClaimMeta {
                    accept_seq: 2,
                    domain_id: id(1)?,
                    scope_id: second.scope_id,
                },
            ),
        );
        ledger.relations.push((
            ClaimRelation {
                source_claim_id: second.id,
                target_claim_id: first.id,
                kind: ClaimRelationKind::Retracts,
                scope_id: second.scope_id,
            },
            AcceptedRelationMeta {
                accept_seq: 3,
                actor: Actor::ModelRun { run_id: id(95)? },
            },
        ));
        ledger.decisions.push((
            UserDecision {
                id: id(94)?,
                target_claim_id: first.id,
                target_object: first.object.clone(),
                resolution_slot: ResolutionSlot {
                    subject_entity_id: first.subject_entity_id,
                    predicate_id: first.predicate_id.clone(),
                    scope_id: second.scope_id,
                },
                action: DecisionAction::Reject,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
                rationale_evidence_ids: Vec::new(),
                decided_at: TimestampMillis::new(20),
                reversible_until: None,
            },
            AcceptedDecisionMeta { accept_seq: 4 },
        ));

        let result = ledger.resolve(&ResolutionQuery {
            subject_entity_id: first.subject_entity_id,
            scope_id: first.scope_id,
            predicate_id: first.predicate_id.clone(),
            valid_at: TimestampMillis::new(20),
            known_at_accept_seq: 4,
            policy: AuthorityPolicy::UserOwned,
        });
        assert_eq!(result.active_claim_ids, vec![first.id]);
        assert!(result.rejected_claim_ids.is_empty());
        assert!(result.conflicting_claim_ids.is_empty());
        Ok(())
    }
}
