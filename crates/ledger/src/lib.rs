//! Pure append-only ledger and bitemporal resolver.
//!
//! The ledger assigns replica-local acceptance sequence numbers. Origin order,
//! acceptance order, and domain-valid time remain independent values.

use std::collections::{BTreeMap, BTreeSet};

use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, AuthorityClass, BatchId, Claim, ClaimId, ClaimObject,
    ClaimRelation, ClaimRelationKind, ContentDigest, DecisionAction, DeviceId, DomainError,
    DomainId, EntityId, EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem,
    FreshnessBand, MasteryLevel, PredicateId, TimestampMillis, UserDecision,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Signed batch semantic version implemented by the Phase 0 ledger.
pub const EVENT_SCHEMA_VERSION: u16 = 1;

/// An origin-authored batch before signature framing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedBatch {
    pub schema_version: u16,
    pub batch_id: BatchId,
    pub device_id: DeviceId,
    pub origin_seq_start: u64,
    pub origin_seq_end: u64,
    pub previous_batch_hash: Option<ContentDigest>,
    pub origin_created_at: TimestampMillis,
    pub events: Vec<Event>,
}

impl UnsignedBatch {
    /// Validates the contiguous origin sequence and each nested event.
    pub fn validate(&self) -> Result<(), LedgerError> {
        if self.schema_version != EVENT_SCHEMA_VERSION {
            return Err(LedgerError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.events.is_empty() {
            return Err(LedgerError::EmptyBatch);
        }
        let expected_count = self
            .origin_seq_end
            .checked_sub(self.origin_seq_start)
            .and_then(|difference| difference.checked_add(1))
            .ok_or(LedgerError::InvalidOriginRange)?;
        if usize::try_from(expected_count).ok() != Some(self.events.len()) {
            return Err(LedgerError::InvalidOriginRange);
        }
        for (offset, event) in self.events.iter().enumerate() {
            let expected = self
                .origin_seq_start
                .checked_add(u64::try_from(offset).map_err(|_| LedgerError::InvalidOriginRange)?)
                .ok_or(LedgerError::InvalidOriginRange)?;
            if event.origin_seq != expected {
                return Err(LedgerError::NonContiguousOrigin {
                    expected,
                    actual: event.origin_seq,
                });
            }
            event.validate()?;
        }
        Ok(())
    }
}

/// Ledger acceptance or replay failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// A nested domain value was invalid.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// The event schema is unsupported and therefore fails closed.
    #[error("unsupported event schema version {0}")]
    UnsupportedSchemaVersion(u16),
    /// Empty batches are not admitted.
    #[error("event batch must not be empty")]
    EmptyBatch,
    /// The declared origin range did not match the event list.
    #[error("batch origin range does not match its event count")]
    InvalidOriginRange,
    /// Event origin sequence numbers were not contiguous.
    #[error("origin sequence must be contiguous: expected {expected}, got {actual}")]
    NonContiguousOrigin { expected: u64, actual: u64 },
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
    pub fn accept_batch(
        &mut self,
        batch: &UnsignedBatch,
        batch_hash: ContentDigest,
    ) -> Result<AcceptanceReceipt, LedgerError> {
        batch.validate()?;
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
                    .ok_or(LedgerError::InvalidOriginRange)?;
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
                if !self.artifacts.contains_key(&item.artifact_id) {
                    return Err(LedgerError::UnknownArtifact(item.artifact_id));
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
                for evidence_id in &claim.evidence_ids {
                    if !self.evidence.contains_key(evidence_id) {
                        return Err(LedgerError::UnknownEvidence(*evidence_id));
                    }
                }
                if self.claims.contains_key(&claim.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "claim",
                        id: claim.id.to_string(),
                    });
                }
                self.claims
                    .insert(claim.id, (claim.clone(), AcceptedClaimMeta { accept_seq }));
            }
            EventPayload::ClaimRelated(relation) => {
                self.ensure_claim_exists(relation.source_claim_id)?;
                self.ensure_claim_exists(relation.target_claim_id)?;
                self.relations
                    .push((relation.clone(), AcceptedRelationMeta { accept_seq }));
            }
            EventPayload::DecisionRecorded(decision) => {
                self.ensure_claim_exists(decision.target_claim_id)?;
                if !self.decision_ids.insert(decision.id) {
                    return Err(LedgerError::DuplicateId {
                        kind: "decision",
                        id: decision.id.to_string(),
                    });
                }
                for evidence_id in &decision.rationale_evidence_ids {
                    if !self.evidence.contains_key(evidence_id) {
                        return Err(LedgerError::UnknownEvidence(*evidence_id));
                    }
                }
                if let DecisionAction::Replace {
                    replacement_claim_id,
                } = &decision.action
                {
                    self.ensure_claim_exists(*replacement_claim_id)?;
                }
                self.decisions
                    .push((decision.clone(), AcceptedDecisionMeta { accept_seq }));
            }
        }
        Ok(())
    }

    fn ensure_claim_exists(&self, claim_id: ClaimId) -> Result<(), LedgerError> {
        if self.claims.contains_key(&claim_id) {
            Ok(())
        } else {
            Err(LedgerError::UnknownClaim(claim_id))
        }
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
                ) && candidate_ids.contains(&relation.source_claim_id)
            })
            .map(|(relation, _)| relation.target_claim_id)
            .collect();

        let mut latest_decisions: BTreeMap<ClaimId, (u64, &DecisionAction)> = BTreeMap::new();
        for (decision, metadata) in self
            .decisions
            .iter()
            .filter(|(_, metadata)| metadata.accept_seq <= query.known_at_accept_seq)
        {
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
        let active: Vec<&Claim> = eligible
            .iter()
            .filter(|(_, _, rank)| *rank == max_rank)
            .map(|(claim, _, _)| *claim)
            .collect();
        let active_objects: Vec<&ClaimObject> = active.iter().map(|claim| &claim.object).collect();
        let conflicts = eligible
            .iter()
            .filter(|(claim, _, _)| !active.contains(claim))
            .filter(|(claim, _, _)| !active_objects.contains(&&claim.object))
            .map(|(claim, _, _)| claim.id)
            .collect();

        ResolutionResult {
            active_claim_ids: active.into_iter().map(|claim| claim.id).collect(),
            conflicting_claim_ids: conflicts,
            rejected_claim_ids: rejected.into_iter().collect(),
        }
    }

    /// Resolves mastery and freshness as separate projections at the same coordinates.
    #[must_use]
    pub fn knowledge_state_as_of(
        &self,
        subject_entity_id: EntityId,
        mastery_predicate_id: PredicateId,
        freshness_predicate_id: PredicateId,
        valid_at: TimestampMillis,
        known_at_accept_seq: u64,
    ) -> KnowledgeStateView {
        let mastery_resolution = self.resolve(&ResolutionQuery {
            subject_entity_id,
            predicate_id: mastery_predicate_id,
            valid_at,
            known_at_accept_seq,
            policy: AuthorityPolicy::UserOwned,
        });
        let freshness_resolution = self.resolve(&ResolutionQuery {
            subject_entity_id,
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

    use academic_domain::{
        ConfidencePermille, EvidenceRole, EvidenceStrength, MediaType, PermissionLineageId,
        RetentionClass, ValidInterval, VaultLocator,
    };

    use super::*;

    fn id<T: FromStr<Err = DomainError>>(suffix: u32) -> Result<T, DomainError> {
        format!("01900000-0000-7000-8000-{suffix:012x}").parse()
    }

    fn fixture_batch() -> Result<UnsignedBatch, DomainError> {
        let domain_id = id(1)?;
        let artifact_id = id(2)?;
        let evidence_id = id(3)?;
        let subject_id = id(4)?;
        let media_type = MediaType::parse("text/plain")?;
        let digest = ContentDigest::sha256(b"synthetic");
        let locator = VaultLocator::derive(b"fixture-domain-key", 1, &media_type, digest)?;
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
        };
        let evidence = EvidenceItem {
            id: evidence_id,
            artifact_id,
            locator: academic_domain::EvidenceLocator::TextBytes {
                source_digest: digest,
                start: 0,
                end: 9,
            },
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
            scope_id: None,
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
            origin_seq_end: 3,
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
                    EventPayload::ArtifactRegistered(artifact),
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
                    EventPayload::EvidenceRegistered(evidence),
                ),
                event(
                    id(11)?,
                    3,
                    TimestampMillis::new(12),
                    Actor::User,
                    domain_id,
                    EventPayload::ClaimAsserted(claim),
                ),
            ],
        })
    }

    #[test]
    fn acceptance_is_append_only_and_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let batch = fixture_batch()?;
        let hash = ContentDigest::sha256(b"batch");
        let mut ledger = LedgerState::new();
        let first = ledger.accept_batch(&batch, hash)?;
        let duplicate = ledger.accept_batch(&batch, hash)?;
        assert_eq!(first.accept_seq_start, 1);
        assert_eq!(first.accept_seq_end, 3);
        assert!(!first.duplicate);
        assert!(duplicate.duplicate);
        assert_eq!(ledger.accepted_events().len(), 3);
        Ok(())
    }

    #[test]
    fn origin_gap_and_fork_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
        let batch = fixture_batch()?;
        let mut ledger = LedgerState::new();
        let hash = ContentDigest::sha256(b"first");
        ledger.accept_batch(&batch, hash)?;

        let mut gap = batch.clone();
        gap.batch_id = id(20)?;
        gap.origin_seq_start = 5;
        gap.origin_seq_end = 7;
        gap.previous_batch_hash = Some(hash);
        for (index, item) in gap.events.iter_mut().enumerate() {
            item.id = id(30 + u32::try_from(index)?)?;
            item.origin_seq = 5 + u64::try_from(index)?;
        }
        assert!(matches!(
            ledger.accept_batch(&gap, ContentDigest::sha256(b"gap")),
            Err(LedgerError::OriginGap { .. })
        ));

        gap.origin_seq_start = 4;
        gap.origin_seq_end = 6;
        gap.previous_batch_hash = Some(ContentDigest::sha256(b"wrong-parent"));
        for (index, item) in gap.events.iter_mut().enumerate() {
            item.origin_seq = 4 + u64::try_from(index)?;
        }
        assert!(matches!(
            ledger.accept_batch(&gap, ContentDigest::sha256(b"fork")),
            Err(LedgerError::DeviceFork)
        ));
        Ok(())
    }
}
