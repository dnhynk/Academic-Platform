//! Pure append-only ledger and bitemporal resolver.
//!
//! The ledger assigns replica-local acceptance sequence numbers. Origin order,
//! acceptance order, and domain-valid time remain independent values.

use std::collections::{BTreeMap, BTreeSet};

use academic_contracts::VerifiedBatch;
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, AuthorityClass, BatchId, Claim, ClaimId, ClaimObject,
    ClaimRelation, ClaimRelationKind, ContentDigest, DecisionAction, DeviceId, DomainError,
    DomainId, EntityId, EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem,
    FreshnessBand, MasteryLevel, PredicateId, ScopeDescriptor, ScopeId, TimestampMillis,
    UserDecision,
};
pub use academic_domain::{EVENT_SCHEMA_VERSION, UnsignedBatch};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AcceptedRelationMeta {
    accept_seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AcceptedDecisionMeta {
    accept_seq: u64,
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
    artifacts: BTreeMap<ArtifactId, ArtifactDescriptor>,
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
                self.artifacts.insert(descriptor.id, descriptor.clone());
            }
            EventPayload::EvidenceRegistered(item) => {
                let descriptor = self
                    .artifacts
                    .get(&item.artifact_id)
                    .ok_or(LedgerError::UnknownArtifact(item.artifact_id))?;
                if descriptor.domain_id != event.domain_id {
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
                    if artifact.domain_id != event.domain_id {
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
                let source = self.claim_metadata(relation.source_claim_id)?;
                let target = self.claim_metadata(relation.target_claim_id)?;
                if source.scope_id != relation.scope_id || target.scope_id != relation.scope_id {
                    return Err(LedgerError::CrossScope("claim relation"));
                }
                if source.domain_id != event.domain_id || target.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("claim relation"));
                }
                self.relations
                    .push((relation.clone(), AcceptedRelationMeta { accept_seq }));
            }
            EventPayload::DecisionRecorded(decision) => {
                let target = self.claim_metadata(decision.target_claim_id)?;
                if target.scope_id != decision.scope_id {
                    return Err(LedgerError::CrossScope("user decision"));
                }
                if target.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("user decision"));
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
                    if artifact.domain_id != event.domain_id {
                        return Err(LedgerError::CrossDomain("decision evidence"));
                    }
                }
                if let DecisionAction::Replace {
                    replacement_claim_id,
                } = &decision.action
                {
                    let replacement = self.claim_metadata(*replacement_claim_id)?;
                    if replacement.scope_id != decision.scope_id {
                        return Err(LedgerError::CrossScope("replacement decision"));
                    }
                    if replacement.domain_id != event.domain_id {
                        return Err(LedgerError::CrossDomain("replacement decision"));
                    }
                }
                self.decisions
                    .push((decision.clone(), AcceptedDecisionMeta { accept_seq }));
            }
        }
        Ok(())
    }

    fn claim_metadata(&self, claim_id: ClaimId) -> Result<AcceptedClaimMeta, LedgerError> {
        self.claims
            .get(&claim_id)
            .map(|(_, metadata)| *metadata)
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
        self.artifacts.get(&id)
    }

    /// Returns an evidence item by immutable identity.
    #[must_use]
    pub fn evidence(&self, id: EvidenceId) -> Option<&EvidenceItem> {
        self.evidence.get(&id)
    }

    /// Resolves active and conflicting claims at independent valid/known coordinates.
    #[must_use]
    pub fn resolve(&self, query: &ResolutionQuery) -> ResolutionResult {
        let mut candidates: Vec<(&Claim, u64)> = self
            .claims
            .values()
            .filter_map(|(claim, metadata)| {
                (metadata.accept_seq <= query.known_at_accept_seq
                    && claim.subject_entity_id == query.subject_entity_id
                    && claim.predicate_id == query.predicate_id
                    && claim.scope_id == query.scope_id
                    && claim.valid_time.contains(query.valid_at))
                .then_some((claim, metadata.accept_seq))
            })
            .collect();
        candidates.sort_by_key(|(claim, accepted)| (*accepted, claim.id));

        let candidate_ids: BTreeSet<ClaimId> =
            candidates.iter().map(|(claim, _)| claim.id).collect();
        let superseded: BTreeSet<ClaimId> = self
            .relations
            .iter()
            .filter(|(_, metadata)| metadata.accept_seq <= query.known_at_accept_seq)
            .filter(|(relation, _)| {
                matches!(
                    relation.kind,
                    ClaimRelationKind::Supersedes | ClaimRelationKind::Retracts
                ) && relation.scope_id == query.scope_id
                    && candidate_ids.contains(&relation.source_claim_id)
                    && candidate_ids.contains(&relation.target_claim_id)
            })
            .map(|(relation, _)| relation.target_claim_id)
            .collect();

        let mut latest_decisions: BTreeMap<ClaimId, (u64, &DecisionAction)> = BTreeMap::new();
        for (decision, metadata) in self.decisions.iter().filter(|(decision, metadata)| {
            metadata.accept_seq <= query.known_at_accept_seq
                && decision.scope_id == query.scope_id
                && candidate_ids.contains(&decision.target_claim_id)
        }) {
            let entry = latest_decisions
                .entry(decision.target_claim_id)
                .or_insert((metadata.accept_seq, &decision.action));
            if metadata.accept_seq > entry.0 {
                *entry = (metadata.accept_seq, &decision.action);
            }
        }

        let mut rejected = superseded;
        let mut confirmed = BTreeSet::new();
        for (target, (_, action)) in latest_decisions {
            match action {
                DecisionAction::Reject => {
                    rejected.insert(target);
                }
                DecisionAction::Confirm => {
                    confirmed.insert(target);
                }
                DecisionAction::Replace {
                    replacement_claim_id,
                } => {
                    rejected.insert(target);
                    confirmed.insert(*replacement_claim_id);
                }
            }
        }

        let eligible: Vec<(&Claim, u64, u16)> = candidates
            .into_iter()
            .filter(|(claim, _)| !rejected.contains(&claim.id))
            .map(|(claim, accepted)| {
                let decision_bonus = u16::from(confirmed.contains(&claim.id)) * 1000;
                (
                    claim,
                    accepted,
                    decision_bonus + authority_rank(query.policy, claim.authority_class),
                )
            })
            .collect();

        let Some(max_rank) = eligible.iter().map(|(_, _, rank)| *rank).max() else {
            return ResolutionResult {
                active_claim_ids: Vec::new(),
                conflicting_claim_ids: Vec::new(),
                rejected_claim_ids: rejected.into_iter().collect(),
            };
        };
        let top_ranked: Vec<&Claim> = eligible
            .iter()
            .filter(|(_, _, rank)| *rank == max_rank)
            .map(|(claim, _, _)| *claim)
            .collect();
        let mut top_objects: Vec<&ClaimObject> = Vec::new();
        for claim in &top_ranked {
            if !top_objects.contains(&&claim.object) {
                top_objects.push(&claim.object);
            }
        }
        let equal_rank_conflict = top_objects.len() > 1;
        let active_claim_ids = if equal_rank_conflict {
            Vec::new()
        } else {
            top_ranked.iter().map(|claim| claim.id).collect()
        };
        let conflicting_claim_ids = if equal_rank_conflict {
            eligible.iter().map(|(claim, _, _)| claim.id).collect()
        } else {
            eligible
                .iter()
                .filter(|(claim, _, _)| !top_ranked.contains(claim))
                .filter(|(claim, _, _)| !top_objects.contains(&&claim.object))
                .map(|(claim, _, _)| claim.id)
                .collect()
        };

        ResolutionResult {
            active_claim_ids,
            conflicting_claim_ids,
            rejected_claim_ids: rejected.into_iter().collect(),
        }
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

/// Predicate-specific authority policy used instead of arrival-time LWW.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorityPolicy {
    UserOwned,
    OfficialFact,
    ImplementationObservation,
    CuratedRelation,
}

fn authority_rank(policy: AuthorityPolicy, authority: AuthorityClass) -> u16 {
    match policy {
        AuthorityPolicy::UserOwned => match authority {
            AuthorityClass::UserExplicit => 800,
            AuthorityClass::DirectObservation => 600,
            AuthorityClass::DeterministicEngine => 500,
            AuthorityClass::Curated => 400,
            AuthorityClass::Official => 350,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
        AuthorityPolicy::OfficialFact => match authority {
            AuthorityClass::Official => 800,
            AuthorityClass::DirectObservation => 600,
            AuthorityClass::Curated => 500,
            AuthorityClass::UserExplicit => 400,
            AuthorityClass::DeterministicEngine => 350,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
        AuthorityPolicy::ImplementationObservation => match authority {
            AuthorityClass::DirectObservation => 800,
            AuthorityClass::UserExplicit => 600,
            AuthorityClass::Official | AuthorityClass::Curated => 400,
            AuthorityClass::DeterministicEngine => 350,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
        AuthorityPolicy::CuratedRelation => match authority {
            AuthorityClass::Curated | AuthorityClass::UserExplicit => 800,
            AuthorityClass::Official => 700,
            AuthorityClass::DirectObservation => 600,
            AuthorityClass::DeterministicEngine => 500,
            AuthorityClass::ModelInference => 200,
            AuthorityClass::Prediction => 100,
            AuthorityClass::Unknown => 0,
        },
    }
}

/// Coordinates for a bitemporal claim query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionQuery {
    pub subject_entity_id: EntityId,
    pub scope_id: ScopeId,
    pub predicate_id: PredicateId,
    pub valid_at: TimestampMillis,
    pub known_at_accept_seq: u64,
    pub policy: AuthorityPolicy,
}

/// Stable resolution result with lower-authority conflicts still visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionResult {
    pub active_claim_ids: Vec<ClaimId>,
    pub conflicting_claim_ids: Vec<ClaimId>,
    pub rejected_claim_ids: Vec<ClaimId>,
}

/// Knowledge projection that cannot decay mastery when freshness changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeStateView {
    pub mastery: Option<MasteryLevel>,
    pub freshness: Option<FreshnessBand>,
    pub mastery_resolution: ResolutionResult,
    pub freshness_resolution: ResolutionResult,
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
        EvidenceStrength, MediaType, PermissionLineageId, RetentionClass, ScopeDescriptor,
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
    fn explicit_page_time_and_repository_representation_metadata_closes_evidence()
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
            let verified = verify_batch(&batch)?;
            let receipt = LedgerState::new().accept_verified_batch(&verified)?;
            assert_eq!(receipt.accept_seq_end, 4);
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
            AcceptedRelationMeta { accept_seq: 3 },
        ));
        ledger.decisions.push((
            UserDecision {
                id: id(94)?,
                target_claim_id: first.id,
                action: DecisionAction::Reject,
                scope_id: second.scope_id,
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
