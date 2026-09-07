//! Profile-derived detail projections and signed append-only relation dispositions.
//! This namespace records disposition of an imported detail relation; it never
//! confirms, replaces or changes the authority of an existing UserDecision.

use crate::{
    Core, CoreError,
    service::{AcceptanceService, ServiceError},
};
use academic_contracts::{DeviceAuthorization, VerifiedBatch, sign_batch, verify_signed_batch};
use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, DomainId, EntityId,
    EpistemicStatus, Event, EventPayload, TimestampMillis, UnsignedBatch, ValidInterval,
};
use academic_rpc::details::{
    self as dto, DetailAction, DetailCorpus, DetailDecisionRequest, DetailReply, DetailReplyState,
    DetailRequest, DetailSelector, DetailState, DispositionAction, Relation, RelationDecision,
};
use academic_store::{
    accept::AcceptError,
    idempotency::{AcceptanceCommand, IdempotencyError},
    profile::SyntheticProfile,
    queries::{QueryError, signed_history_snapshot},
};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

pub const CORPUS_PREDICATE: &str = "detail.workspace.corpus.v1";
pub const RELATION_PREDICATE: &str = "detail.workspace.relation.v1";
pub const DISPOSITION_PREDICATE: &str = "detail.workspace.disposition.v1";
pub const PROJECTOR_VERSION: &str = "academic.details.v1";

#[cfg(any(test, feature = "synthetic-detail-fixtures"))]
pub mod fixture;

#[derive(Debug, thiserror::Error)]
pub enum DetailError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Rpc(#[from] academic_rpc::RpcError),
    #[error(transparent)]
    Query(#[from] QueryError),
    #[error(transparent)]
    Service(#[from] ServiceError),
    #[error(transparent)]
    Contract(#[from] academic_contracts::ContractError),
    #[error(transparent)]
    Domain(#[from] academic_domain::DomainError),
    #[error(transparent)]
    Store(#[from] academic_store::error::StoreError),
    #[error(transparent)]
    Vault(#[from] academic_vault::VaultError),
    #[error("detail source is invalid: {0}")]
    Invalid(&'static str),
}

/// Signed import payload, bound to canonical relation claims in its exact scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusRecord {
    pub version: u16,
    pub corpus: DetailCorpus,
    pub relations: BTreeMap<String, ClaimId>,
    pub entities: BTreeMap<String, EntityId>,
    pub media: BTreeMap<String, academic_domain::ArtifactId>,
}
/// Signed user disposition, with complete request identity and the actual reject target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispositionRecord {
    version: u16,
    request: DetailDecisionRequest,
    request_digest: ContentDigest,
    relation_claim_id: ClaimId,
    undoes: Option<u64>,
}

/// Host-owned authorization and signer. Neither crosses the native/UI boundary.
pub struct DetailContext {
    profile_id: String,
    authorization: DeviceAuthorization,
    signing_key: SigningKey,
    trust: Vec<DeviceAuthorization>,
}
impl std::fmt::Debug for DetailContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetailContext")
            .field("profile_id", &self.profile_id)
            .finish_non_exhaustive()
    }
}
struct History {
    core: Core,
    batches: Vec<(VerifiedBatch, Vec<u8>)>,
    revision: u64,
    head: u64,
}
#[derive(Clone)]
struct BoundRelation {
    claim: Claim,
    domain: DomainId,
}
struct Projection {
    state: DetailState,
    relations: BTreeMap<String, BoundRelation>,
    media: BTreeMap<String, academic_domain::ArtifactDescriptor>,
}

impl DetailContext {
    pub fn new(
        profile: &SyntheticProfile,
        authorization: DeviceAuthorization,
        signing_key: SigningKey,
        mut trust: Vec<DeviceAuthorization>,
    ) -> Result<Self, DetailError> {
        if signing_key.verifying_key() != *authorization.verifying_key() {
            return Err(DetailError::Invalid("signer authorization mismatch"));
        }
        if !trust.contains(&authorization) {
            trust.push(authorization.clone());
        }
        let profile_id = profile_identity(profile)?;
        Ok(Self {
            profile_id,
            authorization,
            signing_key,
            trust,
        })
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub(crate) fn referenced_artifacts(
        &self,
        profile: &SyntheticProfile,
    ) -> Result<Vec<academic_domain::ArtifactDescriptor>, DetailError> {
        Ok(self
            .history(profile)?
            .core
            .ledger()
            .accepted_events()
            .iter()
            .filter_map(|e| match &e.event.payload {
                EventPayload::ArtifactRegistered(descriptor) => Some(descriptor.clone()),
                _ => None,
            })
            .collect())
    }

    fn history(&self, profile: &SyntheticProfile) -> Result<History, DetailError> {
        if profile_identity(profile)? != self.profile_id {
            return Err(DetailError::Invalid("host selected profile changed"));
        }
        let mut reader = profile.open_reader()?;
        let snapshot = signed_history_snapshot(&mut reader)?;
        let mut core = Core::new();
        let mut batches = Vec::new();
        for stored in snapshot.batches {
            let verified = self
                .trust
                .iter()
                .find_map(|authorization| verify_signed_batch(&stored.envelope, authorization).ok())
                .ok_or(DetailError::Invalid(
                    "no independent authorization verifies a stored batch",
                ))?;
            let authorization = self
                .trust
                .iter()
                .find(|a| {
                    a.device_id() == verified.batch().device_id
                        && a.verifying_key() == verified.public_key()
                })
                .ok_or(DetailError::Invalid("stored authorization is ambiguous"))?;
            let (_, receipt) = core.accept_signed_batch(&stored.envelope, authorization)?;
            if receipt.accept_seq_start != stored.accept_seq_start
                || receipt.accept_seq_end != stored.accept_seq_end
            {
                return Err(DetailError::Invalid(
                    "acceptance coordinates disagree with signed replay",
                ));
            }
            batches.push((verified, stored.envelope));
        }
        if u64::try_from(core.ledger().accepted_events().len()).ok()
            != Some(snapshot.accept_seq_head)
        {
            return Err(DetailError::Invalid("signed replay watermark mismatch"));
        }
        Ok(History {
            core,
            batches,
            revision: snapshot.revision,
            head: snapshot.accept_seq_head,
        })
    }

    fn project(
        &self,
        history: &History,
        selector: &DetailSelector,
        now: TimestampMillis,
    ) -> Result<Projection, DetailError> {
        let known = selector.known_at_accept_seq.unwrap_or(history.head);
        let valid = selector.valid_at_ms.unwrap_or(
            u64::try_from(now.value()).map_err(|_| DetailError::Invalid("negative valid time"))?,
        );
        if known > history.head || valid > dto::MAX_SAFE_INTEGER {
            return Err(DetailError::Invalid("invalid read coordinate"));
        }
        let valid_at = TimestampMillis::new(
            i64::try_from(valid).map_err(|_| DetailError::Invalid("valid time overflow"))?,
        );
        let mut records = BTreeMap::new();
        let mut source = Vec::new();
        let mut accepted_start = 1_u64;
        for (batch, _) in &history.batches {
            if accepted_start > known {
                break;
            }
            source.extend_from_slice(batch.envelope_hash().as_bytes());
            accepted_start += u64::try_from(batch.batch().events.len())
                .map_err(|_| DetailError::Invalid("batch size overflow"))?;
        }
        for event in history
            .core
            .ledger()
            .accepted_events()
            .iter()
            .filter(|e| e.accept_seq <= known)
        {
            if let EventPayload::ClaimAsserted(claim) = &event.event.payload
                && claim.predicate_id.as_str() == CORPUS_PREDICATE
                && claim.valid_time.contains(valid_at)
            {
                require_import_claim(&event.event.actor, claim)?;
                let record: CorpusRecord = decode_claim(claim)?;
                if record.version != 1 {
                    return Err(DetailError::Invalid("unsupported corpus version"));
                }
                record.corpus.validate()?;
                records.insert(
                    (
                        event.event.domain_id,
                        claim.scope_id,
                        claim.subject_entity_id,
                    ),
                    (event.accept_seq, claim, record),
                );
            }
        }
        let mut corpus = DetailCorpus::default();
        let mut relations = BTreeMap::new();
        let mut media = BTreeMap::new();
        for ((domain, scope, _), (corpus_sequence, corpus_claim, record)) in records {
            let aliases = record.corpus.relations()?;
            if aliases.len() != record.relations.len() {
                return Err(DetailError::Invalid("relation bindings are incomplete"));
            }
            let entities: BTreeSet<_> = record
                .corpus
                .lectures
                .iter()
                .map(|v| &v.id)
                .chain(record.corpus.concepts.iter().map(|v| &v.id))
                .chain(record.corpus.projects.iter().map(|v| &v.id))
                .chain(record.corpus.questions.iter().map(|v| &v.id))
                .collect();
            if entities != record.entities.keys().collect() {
                return Err(DetailError::Invalid("entity bindings are incomplete"));
            }
            for (lecture, artifact_id) in &record.media {
                if !record.corpus.lectures.iter().any(|l| &l.id == lecture) {
                    return Err(DetailError::Invalid("media lecture is outside corpus"));
                }
                let descriptor = history
                    .core
                    .ledger()
                    .accepted_events()
                    .iter()
                    .find_map(|event| match &event.event.payload {
                        EventPayload::ArtifactRegistered(descriptor)
                            if event.accept_seq < corpus_sequence
                                && descriptor.id == *artifact_id =>
                        {
                            Some(descriptor)
                        }
                        _ => None,
                    })
                    .ok_or(DetailError::Invalid(
                        "media artifact is not accepted before corpus",
                    ))?;
                if descriptor.domain_id != domain {
                    return Err(DetailError::Invalid("media domain is outside corpus"));
                }
                if media.insert(lecture.clone(), descriptor.clone()).is_some() {
                    return Err(DetailError::Invalid("ambiguous media lecture"));
                }
            }
            for (alias, relation) in aliases {
                let id = record
                    .relations
                    .get(alias)
                    .ok_or(DetailError::Invalid("missing relation binding"))?;
                let event = history.core.ledger().accepted_events().iter().find(|e| e.accept_seq < corpus_sequence && matches!(&e.event.payload, EventPayload::ClaimAsserted(c) if c.id == *id))
                    .ok_or(DetailError::Invalid("relation binding is not accepted"))?;
                let EventPayload::ClaimAsserted(claim) = &event.event.payload else {
                    return Err(DetailError::Invalid("not a relation claim"));
                };
                require_import_claim(&event.event.actor, claim)?;
                if event.event.domain_id != domain
                    || claim.scope_id != scope
                    || claim.predicate_id.as_str() != RELATION_PREDICATE
                    || !claim.valid_time.contains(valid_at)
                    || !record
                        .corpus
                        .relation_owners(alias)
                        .iter()
                        .filter_map(|alias| record.entities.get(*alias))
                        .any(|e| *e == claim.subject_entity_id)
                    || decode_claim::<Relation>(claim)? != *relation
                {
                    return Err(DetailError::Invalid(
                        "relation scope, entity, or signed source differs",
                    ));
                }
                // Every displayed source is backed by an exact registered text representation.
                let evidence = claim
                    .evidence_ids
                    .first()
                    .and_then(|id| history.core.ledger().evidence(*id))
                    .ok_or(DetailError::Invalid("relation has no evidence"))?;
                if evidence.excerpt_digest
                    != ContentDigest::sha256(relation.source.content.as_bytes())
                {
                    return Err(DetailError::Invalid(
                        "displayed source differs from evidence digest",
                    ));
                }
                if !corpus_claim
                    .evidence_ids
                    .iter()
                    .all(|id| history.core.ledger().evidence(*id).is_some())
                {
                    return Err(DetailError::Invalid("corpus evidence is absent"));
                }
                if relations
                    .insert(
                        alias.to_owned(),
                        BoundRelation {
                            claim: claim.clone(),
                            domain,
                        },
                    )
                    .is_some()
                {
                    return Err(DetailError::Invalid(
                        "relation alias is ambiguous across scopes",
                    ));
                }
            }
            corpus.lectures.extend(record.corpus.lectures);
            corpus.concepts.extend(record.corpus.concepts);
            corpus.projects.extend(record.corpus.projects);
            corpus.questions.extend(record.corpus.questions);
        }
        corpus.validate()?;
        let mut decisions = Vec::new();
        let mut rejected = BTreeMap::new();
        for event in history
            .core
            .ledger()
            .accepted_events()
            .iter()
            .filter(|e| e.accept_seq <= known)
        {
            let EventPayload::ClaimAsserted(claim) = &event.event.payload else {
                continue;
            };
            if claim.predicate_id.as_str() != DISPOSITION_PREDICATE
                || !claim.valid_time.contains(valid_at)
            {
                continue;
            }
            let record: DispositionRecord = decode_claim(claim)?;
            let Actor::User { user_id } = &event.event.actor else {
                return Err(DetailError::Invalid("disposition is not user-owned"));
            };
            if record.version != 1
                || claim.authority_class != AuthorityClass::UserExplicit
                || claim.epistemic_status != EpistemicStatus::UserConfirmed
                || record.request_digest != dto::decision_digest(&record.request)?
            {
                return Err(DetailError::Invalid(
                    "invalid disposition authority or digest",
                ));
            }
            let target_event = history.core.ledger().accepted_events().iter().find(|e| e.accept_seq < event.accept_seq && matches!(&e.event.payload, EventPayload::ClaimAsserted(c) if c.id == record.relation_claim_id))
                .ok_or(DetailError::Invalid("disposition target is not an earlier accepted claim"))?;
            let EventPayload::ClaimAsserted(target) = &target_event.event.payload else {
                return Err(DetailError::Invalid("disposition target is not a claim"));
            };
            if target.predicate_id.as_str() != RELATION_PREDICATE
                || event.event.domain_id != target_event.event.domain_id
                || claim.scope_id != target.scope_id
                || claim.subject_entity_id != target.subject_entity_id
                || claim.evidence_ids != target.evidence_ids
                || record.request.selector.known_at_accept_seq.is_some()
                || record.request.selector.valid_at_ms.is_some()
                || decode_claim::<Relation>(target)?.id != record.request.relation_id
            {
                return Err(DetailError::Invalid(
                    "disposition is outside relation scope",
                ));
            }
            match record.request.action {
                DetailAction::Reject
                    if record.undoes.is_none()
                        && !rejected.contains_key(&record.relation_claim_id) =>
                {
                    rejected.insert(record.relation_claim_id, event.accept_seq);
                }
                DetailAction::Undo
                    if record.undoes.is_some()
                        && rejected.get(&record.relation_claim_id).copied() == record.undoes =>
                {
                    rejected.remove(&record.relation_claim_id);
                }
                _ => return Err(DetailError::Invalid("invalid reject/undo target history")),
            }
            let Some(relation) = relations.get(&record.request.relation_id) else {
                continue;
            };
            if record.relation_claim_id != relation.claim.id {
                continue;
            }
            decisions.push(RelationDecision {
                sequence: event.accept_seq,
                relation_id: record.request.relation_id,
                action: if record.request.action == DetailAction::Reject {
                    DispositionAction::Reject
                } else {
                    DispositionAction::Undo
                },
                undoes: record.undoes,
                actor: user_id.to_string(),
            });
        }
        let state = DetailState {
            corpus,
            decisions,
            revision: history.revision,
            profile_id: self.profile_id.clone(),
            known_at_accept_seq: known,
            valid_at_ms: valid,
            projector_version: PROJECTOR_VERSION.to_owned(),
            source_digest: hex::encode(ContentDigest::sha256(&source).as_bytes()),
        };
        let _ = dto::encode(&state)?;
        Ok(Projection {
            state,
            relations,
            media,
        })
    }

    pub fn handle(
        &self,
        profile: &SyntheticProfile,
        service: &mut AcceptanceService,
        request: &DetailRequest,
        now: TimestampMillis,
    ) -> Result<DetailReply, DetailError> {
        if !service.uses_profile(profile) {
            return Err(DetailError::Invalid("writer belongs to another profile"));
        }
        let _ = dto::encode(request)?;
        let history = self.history(profile)?;
        for event in history.core.ledger().accepted_events() {
            if let EventPayload::ArtifactRegistered(descriptor) = &event.event.payload {
                let _ = service.vault().verify_sealed_object(descriptor)?;
            }
        }
        match request {
            DetailRequest::DetailsAudio { audio } => self.audio(service, &history, audio, now),
            DetailRequest::DetailsRead { selector } => Ok(reply(
                DetailReplyState::Ready,
                "READY",
                Some(self.project(&history, selector, now)?.state),
                None,
                None,
            )?),
            DetailRequest::DetailsDecide { decision } => {
                self.decide(profile, service, &history, decision, now)
            }
        }
    }

    fn audio(
        &self,
        service: &AcceptanceService,
        history: &History,
        request: &dto::DetailAudioRequest,
        now: TimestampMillis,
    ) -> Result<DetailReply, DetailError> {
        let reject = |reason| reply(DetailReplyState::Rejected, reason, None, None, None);
        if request.expected_profile_id != self.profile_id {
            return reject("PROFILE_MISMATCH");
        }
        if request.expected_revision != history.revision {
            return reject("REVISION_CONFLICT");
        }
        if request.length == 0 || request.length > 4096 {
            return reject("AUDIO_RANGE_INVALID");
        }
        let projection = self.project(history, &request.selector, now)?;
        let Some(descriptor) = projection.media.get(&request.lecture_id) else {
            return reject("AUDIO_NOT_FOUND");
        };
        if descriptor.media_type.as_str() != "audio/wav" || descriptor.byte_length > 4_194_304 {
            return reject("AUDIO_FORMAT_OR_SIZE_UNSUPPORTED");
        }
        if request.offset >= descriptor.byte_length {
            return reject("AUDIO_RANGE_INVALID");
        }
        let length = request.length.min(descriptor.byte_length - request.offset);
        let mut capability = service.vault().verify_sealed_object(descriptor)?;
        let bytes = capability.read_verified_range(
            request.offset,
            usize::try_from(length).map_err(|_| DetailError::Invalid("audio range overflow"))?,
        )?;
        let mut response = reply(DetailReplyState::Ready, "READY", None, None, None)?;
        response.audio = Some(dto::DetailAudio {
            lecture_id: request.lecture_id.clone(),
            media_type: "audio/wav".to_owned(),
            content_digest: hex::encode(descriptor.content_digest.as_bytes()),
            total_bytes: descriptor.byte_length,
            offset: request.offset,
            bytes,
        });
        Ok(response)
    }

    fn decide(
        &self,
        profile: &SyntheticProfile,
        service: &mut AcceptanceService,
        history: &History,
        request: &DetailDecisionRequest,
        now: TimestampMillis,
    ) -> Result<DetailReply, DetailError> {
        let reject = |reason| {
            reply(
                DetailReplyState::Rejected,
                reason,
                None,
                Some(request),
                None,
            )
        };
        if request.expected_profile_id != self.profile_id {
            return reject("PROFILE_MISMATCH");
        }
        if request.selector.known_at_accept_seq.is_some() || request.selector.valid_at_ms.is_some()
        {
            return reject("HISTORICAL_VIEW");
        }
        dto::reference(&request.relation_id)?;
        // Locate persisted identity BEFORE looking at revision/current disposition. Reuse
        // the original envelope so a lost acknowledgement survives both restart and later writes.
        for (batch, envelope) in &history.batches {
            for event in &batch.batch().events {
                let EventPayload::ClaimAsserted(claim) = &event.payload else {
                    continue;
                };
                if claim.predicate_id.as_str() != DISPOSITION_PREDICATE {
                    continue;
                }
                let old: DispositionRecord = decode_claim(claim)?;
                if old.request.client_instance_id == request.client_instance_id
                    && (old.request.idempotency_key == request.idempotency_key
                        || old.request.request_id == request.request_id)
                {
                    if old.request != *request {
                        return reject("IDEMPOTENCY_KEY_COLLISION");
                    }
                    return self.commit(profile, service, request, envelope, now);
                }
            }
        }
        if request.expected_revision != history.revision {
            return reject("REVISION_CONFLICT");
        }
        if history.core.ledger().accepted_events().iter().any(|event| matches!(&event.event.payload,
            EventPayload::ClaimAsserted(claim) if claim.predicate_id.as_str() == DISPOSITION_PREDICATE && claim.valid_time.from() > now)) {
            return reject("CLOCK_BEFORE_HISTORY");
        }
        let projection = self.project(history, &DetailSelector::default(), now)?;
        let Some(relation) = projection.relations.get(&request.relation_id) else {
            return reject("RELATION_NOT_FOUND");
        };
        let active = projection
            .state
            .decisions
            .iter()
            .rev()
            .find(|d| d.relation_id == request.relation_id)
            .filter(|d| d.action == DispositionAction::Reject)
            .map(|d| d.sequence);
        let undoes = match (request.action, active) {
            (DetailAction::Reject, None) => None,
            (DetailAction::Reject, Some(_)) => return reject("ALREADY_REJECTED"),
            (DetailAction::Undo, None) => return reject("NOTHING_TO_UNDO"),
            (DetailAction::Undo, Some(sequence)) => Some(sequence),
        };
        let digest = dto::decision_digest(request)?;
        let record = DispositionRecord {
            version: 1,
            request: request.clone(),
            request_digest: digest,
            relation_claim_id: relation.claim.id,
            undoes,
        };
        let object = String::from_utf8(dto::encode(&record)?)
            .map_err(|_| DetailError::Invalid("record UTF-8"))?;
        let claim = Claim {
            id: derived_id(digest, "claim")?,
            subject_entity_id: relation.claim.subject_entity_id,
            predicate_id: academic_domain::PredicateId::parse(DISPOSITION_PREDICATE)?,
            object: ClaimObject::Text(object),
            scope_id: relation.claim.scope_id,
            authority_class: AuthorityClass::UserExplicit,
            epistemic_status: EpistemicStatus::UserConfirmed,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(now),
            evidence_ids: relation.claim.evidence_ids.clone(),
        };
        let previous = history
            .batches
            .iter()
            .rev()
            .find(|(b, _)| b.batch().device_id == self.authorization.device_id())
            .map(|(b, _)| b);
        let sequence = previous.map_or(Ok(1), |b| {
            b.batch()
                .origin_seq_end
                .checked_add(1)
                .ok_or(DetailError::Invalid("origin exhausted"))
        })?;
        let batch = UnsignedBatch {
            schema_version: academic_domain::EVENT_SCHEMA_VERSION_V4,
            batch_id: derived_id(digest, "batch")?,
            device_id: self.authorization.device_id(),
            origin_seq_start: sequence,
            origin_seq_end: sequence,
            previous_batch_hash: previous.map(VerifiedBatch::envelope_hash),
            origin_created_at: now,
            events: vec![Event {
                id: derived_id(digest, "event")?,
                origin_seq: sequence,
                origin_observed_at: now,
                actor: Actor::User {
                    user_id: self.authorization.user_id(),
                },
                domain_id: relation.domain,
                payload: EventPayload::ClaimAsserted(claim),
            }],
        };
        let envelope = sign_batch(&batch, &self.signing_key)?;
        // Validate the resulting projection and its bounded receipt before durable acceptance.
        // This is a dry replay through the same canonical core, never an optimistic ACK.
        let mut preview = self.history(profile)?;
        let (verified, _) = preview
            .core
            .accept_signed_batch(&envelope, &self.authorization)?;
        let original_decision = decision_receipt(&verified, preview.head + 1, request)?;
        preview.batches.push((verified, envelope.clone()));
        preview.head += 1;
        preview.revision += 1;
        let mut response = reply(
            DetailReplyState::Accepted,
            "ACCEPTED",
            Some(
                self.project(&preview, &DetailSelector::default(), now)?
                    .state,
            ),
            Some(request),
            Some(*batch.batch_id.as_bytes()),
        )?;
        response.decision_sequence = Some(preview.head);
        response.receipt_decision = Some(original_decision);
        response.validate()?;
        let _ = dto::encode(&response)?;
        self.commit(profile, service, request, &envelope, now)
    }
    fn commit(
        &self,
        profile: &SyntheticProfile,
        service: &mut AcceptanceService,
        request: &DetailDecisionRequest,
        envelope: &[u8],
        now: TimestampMillis,
    ) -> Result<DetailReply, DetailError> {
        let outcome = match service.accept_signed_command(
            AcceptanceCommand {
                request_id: request.request_id,
                client_instance_id: request.client_instance_id,
                idempotency_key: request.idempotency_key,
                expected_revision: Some(request.expected_revision),
                envelope_bytes: envelope,
            },
            &self.authorization,
            now,
        ) {
            Ok(value) => value,
            Err(ServiceError::Acceptance(AcceptError::ExpectedRevisionConflict { .. })) => {
                return reply(
                    DetailReplyState::Rejected,
                    "REVISION_CONFLICT",
                    None,
                    Some(request),
                    None,
                );
            }
            Err(ServiceError::Acceptance(AcceptError::Idempotency(
                IdempotencyError::KeyCollision,
            ))) => {
                return reply(
                    DetailReplyState::Rejected,
                    "IDEMPOTENCY_KEY_COLLISION",
                    None,
                    Some(request),
                    None,
                );
            }
            Err(error) => return Err(error.into()),
        };
        let state = self
            .project(&self.history(profile)?, &DetailSelector::default(), now)?
            .state;
        let mut response = reply(
            DetailReplyState::Accepted,
            "ACCEPTED",
            Some(state),
            Some(request),
            Some(*outcome.receipt.batch_id.as_bytes()),
        )?;
        response.decision_sequence = Some(outcome.receipt.accept_seq_end);
        response.receipt_decision = Some(decision_receipt(
            &verify_signed_batch(envelope, &self.authorization)?,
            outcome.receipt.accept_seq_end,
            request,
        )?);
        response.validate()?;
        let _ = dto::encode(&response)?;
        Ok(response)
    }
}
fn decision_receipt(
    batch: &VerifiedBatch,
    sequence: u64,
    request: &DetailDecisionRequest,
) -> Result<dto::DetailDecisionReceipt, DetailError> {
    let [event] = batch.batch().events.as_slice() else {
        return Err(DetailError::Invalid(
            "decision receipt must bind one original event",
        ));
    };
    let EventPayload::ClaimAsserted(claim) = &event.payload else {
        return Err(DetailError::Invalid("receipt event is not a claim"));
    };
    let Actor::User { user_id } = &event.actor else {
        return Err(DetailError::Invalid("receipt actor is not a user"));
    };
    let record: DispositionRecord = decode_claim(claim)?;
    if claim.predicate_id.as_str() != DISPOSITION_PREDICATE
        || claim.authority_class != AuthorityClass::UserExplicit
        || claim.epistemic_status != EpistemicStatus::UserConfirmed
        || record.version != 1
        || record.request != *request
        || record.request_digest != dto::decision_digest(request)?
    {
        return Err(DetailError::Invalid(
            "original decision does not bind receipt request",
        ));
    }
    Ok(dto::DetailDecisionReceipt {
        sequence,
        relation_id: record.request.relation_id,
        relation_claim_id: record.relation_claim_id,
        action: record.request.action,
        undoes: record.undoes,
        actor: *user_id,
    })
}
fn profile_identity(profile: &SyntheticProfile) -> Result<String, DetailError> {
    // Host location and a synced incarnation guard reset/restore and relocation.
    // They are correlation metadata, never an authorization token or signing key.
    let schema: (Vec<u8>, i64) = profile.open_reader()?.query_row(
        "SELECT creating_build_digest, created_at_unix_ms FROM schema_meta WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(hex::encode(
        ContentDigest::sha256(
            &[
                b"academic.detail-profile-incarnation.v1\0".as_slice(),
                &profile.detail_incarnation()?,
                profile.root().as_os_str().as_encoded_bytes(),
                &schema.0,
                &schema.1.to_be_bytes(),
            ]
            .concat(),
        )
        .as_bytes(),
    ))
}
fn reply(
    state: DetailReplyState,
    reason: &str,
    details: Option<DetailState>,
    request: Option<&DetailDecisionRequest>,
    receipt_id: Option<[u8; 16]>,
) -> Result<DetailReply, DetailError> {
    let reply = DetailReply {
        version: 1,
        state,
        message: reason.to_owned(),
        receipt_id,
        decision_sequence: None,
        receipt_decision: None,
        reason: Some(reason.to_owned()),
        details,
        audio: None,
        request_id: request.map(|r| r.request_id),
        client_instance_id: request.map(|r| r.client_instance_id),
        idempotency_key: request.map(|r| r.idempotency_key),
        request_digest: request
            .map(dto::decision_digest)
            .transpose()?
            .map(|d| *d.as_bytes()),
    };
    let _ = dto::encode(&reply)?;
    Ok(reply)
}
fn require_import_claim(actor: &Actor, claim: &Claim) -> Result<(), DetailError> {
    if !matches!(actor, Actor::Importer { .. })
        || claim.authority_class != AuthorityClass::DirectObservation
        || claim.epistemic_status != EpistemicStatus::CodeObserved
    {
        return Err(DetailError::Invalid(
            "detail import actor/authority mismatch",
        ));
    }
    Ok(())
}
fn decode_claim<T: serde::de::DeserializeOwned + Serialize>(
    claim: &Claim,
) -> Result<T, DetailError> {
    let ClaimObject::Text(json) = &claim.object else {
        return Err(DetailError::Invalid("detail claim must carry typed JSON"));
    };
    Ok(dto::decode(json.as_bytes())?)
}
pub(crate) fn derived_id<T: FromStr<Err = academic_domain::DomainError>>(
    digest: ContentDigest,
    label: &str,
) -> Result<T, DetailError> {
    let digest = ContentDigest::sha256(&[digest.as_bytes().as_slice(), label.as_bytes()].concat());
    let mut bytes = *digest.as_bytes();
    bytes[6] = (bytes[6] & 15) | 0x70;
    bytes[8] = (bytes[8] & 63) | 0x80;
    let hex = hex::encode(&bytes[..16]);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
    .parse()?)
}

#[cfg(test)]
mod tests {
    // Included only by the detail module's cfg(test) unit-test wrapper.
    use super::*;
    use academic_rpc::details::*;
    use academic_store::{
        path_policy::NativePathProbe,
        profile::{create_synthetic_profile, open_synthetic_profile},
        queries::signed_history_snapshot,
    };
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn distinct_relations_can_share_exact_source_bytes() -> TestResult {
        let path = detail_test_root();
        let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        let mut source = corpus("alpha");
        let relations = source.concepts[0]
            .relations
            .get_mut("Evidence")
            .ok_or("missing relations")?;
        let mut second = relations[0].clone();
        second.id = "relation-alpha-second".to_owned();
        relations.push(second);
        fixture::import_synthetic_corpus(&profile, source.clone())?;
        let context = context(&profile)?;
        let mut service = open_service(&profile)?;
        assert_eq!(read(&context, &profile, &mut service, 100)?.corpus, source);
        assert_eq!(
            academic_store::queries::canonical_snapshot(&profile.open_reader()?)?.artifact_count,
            2
        );
        drop(service);
        drop(profile);
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    #[test]
    fn lost_ack_receipt_survives_source_removal_and_alias_reuse_without_leaking_overlay()
    -> TestResult {
        let path = detail_test_root();
        let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        fixture::import_synthetic_corpus(&profile, corpus("alpha"))?;
        let context = context(&profile)?;
        let mut service = open_service(&profile)?;
        let original_history = context.history(&profile)?;
        let original_corpus = original_history
            .core
            .ledger()
            .accepted_events()
            .iter()
            .find_map(|event| match &event.event.payload {
                EventPayload::ClaimAsserted(claim)
                    if claim.predicate_id.as_str() == CORPUS_PREDICATE =>
                {
                    Some(claim.clone())
                }
                _ => None,
            })
            .ok_or("missing corpus")?;
        let record: CorpusRecord = decode_claim(&original_corpus)?;
        let original_relation = original_history
            .core
            .ledger()
            .accepted_events()
            .iter()
            .find_map(|event| match &event.event.payload {
                EventPayload::ClaimAsserted(claim)
                    if claim.predicate_id.as_str() == RELATION_PREDICATE =>
                {
                    Some(claim.clone())
                }
                _ => None,
            })
            .ok_or("missing relation")?;
        let request = request(
            context.profile_id(),
            "relation-alpha",
            1,
            40,
            DetailAction::Reject,
        );
        let accepted =
            context.handle(&profile, &mut service, &request, TimestampMillis::new(100))?;
        let removed = CorpusRecord {
            version: 1,
            corpus: DetailCorpus::default(),
            relations: BTreeMap::new(),
            entities: BTreeMap::new(),
            media: BTreeMap::new(),
        };
        append_source_revision(
            &profile,
            &mut service,
            original_corpus.clone(),
            removed,
            None,
            41,
        )?;
        let retry = context.handle(&profile, &mut service, &request, TimestampMillis::new(200))?;
        assert_eq!(retry.receipt_id, accepted.receipt_id);
        assert_eq!(retry.receipt_decision, accepted.receipt_decision);
        assert!(
            retry
                .details
                .as_ref()
                .ok_or("missing snapshot")?
                .corpus
                .concepts
                .is_empty()
        );
        assert!(
            retry
                .details
                .as_ref()
                .ok_or("missing snapshot")?
                .decisions
                .is_empty()
        );
        let mut replacement = original_relation.clone();
        replacement.id = derived_id(ContentDigest::sha256(b"replacement relation"), "claim")?;
        let mut replacement_record = record;
        replacement_record
            .relations
            .insert("relation-alpha".to_owned(), replacement.id);
        append_source_revision(
            &profile,
            &mut service,
            original_corpus,
            replacement_record,
            Some(replacement.clone()),
            42,
        )?;
        let retry = context.handle(&profile, &mut service, &request, TimestampMillis::new(300))?;
        assert_eq!(
            retry
                .receipt_decision
                .as_ref()
                .map(|receipt| receipt.relation_claim_id),
            Some(original_relation.id)
        );
        assert_eq!(retry.receipt_id, accepted.receipt_id);
        let state = retry.details.as_ref().ok_or("missing snapshot")?;
        assert!(!state.corpus.concepts.is_empty());
        assert!(state.decisions.is_empty());
        let new_request = super::tests::request(
            context.profile_id(),
            "relation-alpha",
            state.revision,
            43,
            DetailAction::Reject,
        );
        let new = context.handle(
            &profile,
            &mut service,
            &new_request,
            TimestampMillis::new(310),
        )?;
        assert_eq!(
            new.receipt_decision
                .as_ref()
                .map(|receipt| receipt.relation_claim_id),
            Some(replacement.id)
        );
        assert_ne!(new.receipt_id, accepted.receipt_id);
        assert_eq!(
            new.details.as_ref().map(|state| state.decisions.len()),
            Some(1)
        );
        drop(service);
        drop(profile);
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    /// Ordinary signed source updates exercise the consumer independently of the empty-profile importer.
    fn append_source_revision(
        profile: &SyntheticProfile,
        service: &mut AcceptanceService,
        mut corpus_claim: Claim,
        record: CorpusRecord,
        relation: Option<Claim>,
        seed: u8,
    ) -> TestResult {
        use academic_domain::{
            ArtifactRepresentation, Confidentiality, EvidenceItem, EvidenceLocator, EvidenceRole,
            EvidenceStrength, MediaType, RetentionClass,
        };
        let history = context(profile)?.history(profile)?;
        let previous = history
            .batches
            .last()
            .ok_or("missing source history")?
            .0
            .clone();
        let domain = previous.batch().events[0].domain_id;
        let digest = ContentDigest::sha256(&[seed]);
        let bytes = dto::encode(&record)?;
        let request = academic_vault::ArtifactIngestRequest::new(
            derived_id(digest, "artifact")?,
            MediaType::parse("text/plain")?,
            domain,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            derived_id(digest, "permission")?,
        );
        let sealed = service
            .vault()
            .ingest(&request, std::io::Cursor::new(&bytes))?;
        let mut descriptor = sealed.descriptor().clone();
        let locator = EvidenceLocator::TextBytes {
            source_digest: descriptor.content_digest,
            start: 0,
            end: descriptor.byte_length,
        };
        descriptor
            .evidence_representations
            .push(ArtifactRepresentation {
                locator: locator.clone(),
                content_digest: descriptor.content_digest,
                byte_length: descriptor.byte_length,
            });
        let evidence = EvidenceItem {
            id: derived_id(digest, "evidence")?,
            artifact_id: descriptor.id,
            locator,
            excerpt_digest: descriptor.content_digest,
            role: EvidenceRole::Supports,
            strength: EvidenceStrength::Direct,
            extraction_method: "synthetic-source-update".to_owned(),
            extractor_version: "1".to_owned(),
        };
        corpus_claim.id = derived_id(digest, "corpus")?;
        corpus_claim.object = ClaimObject::Text(String::from_utf8(bytes)?);
        corpus_claim.evidence_ids = vec![evidence.id];
        let mut payloads = vec![
            EventPayload::ArtifactRegistered(descriptor),
            EventPayload::EvidenceRegistered(evidence),
        ];
        if let Some(relation) = relation {
            payloads.push(EventPayload::ClaimAsserted(relation));
        }
        payloads.push(EventPayload::ClaimAsserted(corpus_claim));
        let start = previous.batch().origin_seq_end + 1;
        let mut events = Vec::new();
        for (index, payload) in payloads.into_iter().enumerate() {
            let sequence = start + u64::try_from(index)?;
            events.push(Event {
                id: derived_id(digest, &format!("event-{sequence}"))?,
                origin_seq: sequence,
                origin_observed_at: TimestampMillis::new(150),
                actor: Actor::Importer {
                    name: "synthetic-source-update".to_owned(),
                    version: "1".to_owned(),
                },
                domain_id: domain,
                payload,
            });
        }
        let batch = UnsignedBatch {
            schema_version: academic_domain::EVENT_SCHEMA_VERSION_V4,
            batch_id: derived_id(digest, "batch")?,
            device_id: previous.batch().device_id,
            origin_seq_start: start,
            origin_seq_end: start + u64::try_from(events.len())? - 1,
            previous_batch_hash: Some(previous.envelope_hash()),
            origin_created_at: TimestampMillis::new(150),
            events,
        };
        let envelope = sign_batch(&batch, &crate::fixture_signing_key())?;
        service.accept_signed_command(
            academic_store::idempotency::AcceptanceCommand {
                request_id: [seed; 16],
                client_instance_id: [seed; 16],
                idempotency_key: [seed; 32],
                expected_revision: Some(history.revision),
                envelope_bytes: &envelope,
            },
            &crate::fixture_device_authorization()?,
            TimestampMillis::new(150),
        )?;
        Ok(())
    }

    #[test]
    fn ordinary_restore_renews_incarnation_and_invalid_marker_is_not_regenerated() -> TestResult {
        let path = detail_test_root();
        let backup = detail_test_root();
        let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        let (mut local, _) = crate::local_service::LocalService::open(
            profile.clone(),
            std::time::SystemTime::now(),
        )?;
        let ingest = crate::operations::synthetic_ingest_request(
            crate::local_service::PHASE1_SYNTHETIC_FIXTURE_ID,
            Some(0),
        )?;
        local.handle_mutable_request(&ingest, TimestampMillis::new(1000))?;
        drop(local);
        let original = context(&profile)?;
        crate::operations::backup_synthetic_profile(&path, &backup)?;
        drop(profile);
        std::fs::remove_dir_all(&path)?;
        crate::operations::restore_synthetic_profile(&backup, &path)?;
        let restored = open_synthetic_profile(&path, &NativePathProbe::default())?;
        let reopened = context(&restored)?;
        assert_ne!(original.profile_id(), reopened.profile_id());
        assert_eq!(context(&restored)?.profile_id(), reopened.profile_id());
        let marker = path.join("detail-incarnation.v1");
        std::fs::write(&marker, b"invalid")?;
        assert!(context(&restored).is_err());
        assert_eq!(std::fs::read(&marker)?, b"invalid");
        drop(restored);
        std::fs::remove_dir_all(path)?;
        std::fs::remove_dir_all(backup)?;
        Ok(())
    }
    fn corpus(label: &str) -> DetailCorpus {
        DetailCorpus {
            concepts: vec![Concept {
                id: format!("concept-{label}"),
                title: label.to_owned(),
                state: "UNKNOWN".to_owned(),
                confidence: "Unavailable".to_owned(),
                freshness: "UNKNOWN".to_owned(),
                last_strong_evidence: "None".to_owned(),
                explanation: String::new(),
                relations: BTreeMap::from([(
                    "Evidence".to_owned(),
                    vec![Relation {
                        id: format!("relation-{label}"),
                        label: label.to_owned(),
                        source: Source {
                            id: format!("source-{label}"),
                            title: label.to_owned(),
                            locator: "text bytes [0, 18)".to_owned(),
                            content: format!("SYNTHETIC {label} source"),
                            href: None,
                        },
                        status: RelationStatus::Proposed,
                        confidence: "Unavailable".to_owned(),
                        target: None,
                    }],
                )]),
            }],
            ..DetailCorpus::default()
        }
    }
    fn detail_test_root() -> std::path::PathBuf {
        let base = std::env::temp_dir();
        #[cfg(unix)]
        let base = std::fs::canonicalize(&base).unwrap_or(base);
        base.join(format!(
            "academic-detail-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn request(
        profile_id: &str,
        relation: &str,
        revision: u64,
        number: u8,
        action: DetailAction,
    ) -> DetailRequest {
        DetailRequest::DetailsDecide {
            decision: DetailDecisionRequest {
                relation_id: relation.to_owned(),
                action,
                expected_revision: revision,
                expected_profile_id: profile_id.to_owned(),
                selector: DetailSelector::default(),
                request_id: [number; 16],
                client_instance_id: [99; 16],
                idempotency_key: [number; 32],
            },
        }
    }
    fn context(profile: &SyntheticProfile) -> Result<DetailContext, DetailError> {
        DetailContext::new(
            profile,
            crate::fixture_device_authorization()?,
            crate::fixture_signing_key(),
            Vec::new(),
        )
    }
    fn open_service(profile: &SyntheticProfile) -> Result<AcceptanceService, DetailError> {
        let mut keys = academic_vault::DomainKeyring::new();
        for artifact in context(profile)?.referenced_artifacts(profile)? {
            keys.insert(
                artifact.domain_id,
                crate::local_service::FIXTURE_LOCATOR_KEY,
            )?;
        }
        Ok(AcceptanceService::open(profile, keys)?)
    }
    fn read(
        context: &DetailContext,
        profile: &SyntheticProfile,
        service: &mut AcceptanceService,
        now: i64,
    ) -> Result<DetailState, DetailError> {
        context
            .handle(
                profile,
                service,
                &DetailRequest::DetailsRead {
                    selector: DetailSelector::default(),
                },
                TimestampMillis::new(now),
            )?
            .details
            .ok_or(DetailError::Invalid("missing test snapshot"))
    }
    #[test]
    fn selected_empty_and_two_seeded_profiles_are_distinct() -> TestResult {
        for label in [None, Some("alpha"), Some("beta")] {
            let path = detail_test_root();
            let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
            if let Some(label) = label {
                fixture::import_synthetic_corpus(&profile, corpus(label))?;
            }
            let context = context(&profile)?;
            let mut service = open_service(&profile)?;
            let state = read(&context, &profile, &mut service, 100)?;
            assert_eq!(
                state.corpus,
                label.map_or_else(DetailCorpus::default, corpus)
            );
            assert_eq!(state.revision, u64::from(label.is_some()));
            drop(service);
            drop(profile);
            std::fs::remove_dir_all(path)?;
        }
        Ok(())
    }
    #[test]
    fn context_and_writer_cannot_cross_selected_profiles() -> TestResult {
        let first_path = detail_test_root();
        let second_path = detail_test_root();
        let first = create_synthetic_profile(&first_path, &NativePathProbe::default(), [32; 32])?;
        let second = create_synthetic_profile(&second_path, &NativePathProbe::default(), [32; 32])?;
        fixture::import_synthetic_corpus(&first, corpus("alpha"))?;
        fixture::import_synthetic_corpus(&second, corpus("beta"))?;
        let context = context(&first)?;
        let mut first_service = open_service(&first)?;
        let mut second_service = open_service(&second)?;
        assert!(read(&context, &second, &mut second_service, 100).is_err());
        assert!(read(&context, &first, &mut second_service, 100).is_err());
        assert_eq!(
            read(&context, &first, &mut first_service, 100)?.corpus,
            corpus("alpha")
        );
        drop(first_service);
        drop(second_service);
        drop(first);
        drop(second);
        std::fs::remove_dir_all(first_path)?;
        std::fs::remove_dir_all(second_path)?;
        Ok(())
    }
    #[test]
    fn durable_reject_undo_retry_conflicts_and_restart_preserve_history() -> TestResult {
        let path = detail_test_root();
        let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        fixture::import_synthetic_corpus(&profile, corpus("alpha"))?;
        let context = context(&profile)?;
        let mut service = open_service(&profile)?;
        let before = signed_history_snapshot(&mut profile.open_reader()?)?;
        let original_projection = read(&context, &profile, &mut service, 160)?;
        let reject = request(
            context.profile_id(),
            "relation-alpha",
            1,
            1,
            DetailAction::Reject,
        );
        let accepted =
            context.handle(&profile, &mut service, &reject, TimestampMillis::new(100))?;
        assert_eq!(accepted.state, DetailReplyState::Accepted);
        let rejected = accepted.details.as_ref().ok_or("no accepted snapshot")?;
        assert_eq!(rejected.decisions.len(), 1);
        assert_eq!(
            rejected.corpus.concepts[0].relations["Evidence"][0].status,
            RelationStatus::Proposed
        );
        let duplicate =
            context.handle(&profile, &mut service, &reject, TimestampMillis::new(110))?;
        assert_eq!(duplicate.receipt_id, accepted.receipt_id);
        assert_eq!(duplicate.details.as_ref().map(|s| s.revision), Some(2));
        let backwards = request(
            context.profile_id(),
            "relation-alpha",
            2,
            9,
            DetailAction::Undo,
        );
        assert_eq!(
            context
                .handle(&profile, &mut service, &backwards, TimestampMillis::new(90))?
                .reason
                .as_deref(),
            Some("CLOCK_BEFORE_HISTORY")
        );
        let stale = request(
            context.profile_id(),
            "relation-alpha",
            1,
            2,
            DetailAction::Undo,
        );
        assert_eq!(
            context
                .handle(&profile, &mut service, &stale, TimestampMillis::new(120))?
                .reason
                .as_deref(),
            Some("REVISION_CONFLICT")
        );
        let mut collision = reject.clone();
        if let DetailRequest::DetailsDecide { decision } = &mut collision {
            decision.action = DetailAction::Undo;
        }
        assert_eq!(
            context
                .handle(
                    &profile,
                    &mut service,
                    &collision,
                    TimestampMillis::new(120)
                )?
                .reason
                .as_deref(),
            Some("IDEMPOTENCY_KEY_COLLISION")
        );
        let missing = request(
            context.profile_id(),
            "relation-beta",
            2,
            3,
            DetailAction::Reject,
        );
        assert_eq!(
            context
                .handle(&profile, &mut service, &missing, TimestampMillis::new(120))?
                .reason
                .as_deref(),
            Some("RELATION_NOT_FOUND")
        );
        let undo = request(
            context.profile_id(),
            "relation-alpha",
            2,
            4,
            DetailAction::Undo,
        );
        let undone = context.handle(&profile, &mut service, &undo, TimestampMillis::new(130))?;
        let undo_receipt = undone
            .receipt_decision
            .as_ref()
            .ok_or("missing original undo receipt")?;
        assert_eq!(undo_receipt.action, DetailAction::Undo);
        assert_eq!(undo_receipt.undoes, accepted.decision_sequence);
        assert_eq!(
            Some(undo_receipt.relation_claim_id),
            accepted
                .receipt_decision
                .as_ref()
                .map(|receipt| receipt.relation_claim_id)
        );
        let snapshot = undone.details.ok_or("missing undo snapshot")?;
        assert_eq!(
            snapshot.decisions[1].undoes,
            Some(snapshot.decisions[0].sequence)
        );
        assert_eq!(snapshot.decisions[1].action, DispositionAction::Undo);
        let nothing = request(
            context.profile_id(),
            "relation-alpha",
            3,
            5,
            DetailAction::Undo,
        );
        assert_eq!(
            context
                .handle(&profile, &mut service, &nothing, TimestampMillis::new(140))?
                .reason
                .as_deref(),
            Some("NOTHING_TO_UNDO")
        );
        let after = signed_history_snapshot(&mut profile.open_reader()?)?;
        assert_eq!(before.batches[0].envelope, after.batches[0].envelope);
        assert_eq!(after.batches.len(), 3);
        drop(service);
        drop(profile);
        let profile = open_synthetic_profile(&path, &NativePathProbe::default())?;
        let mut service = open_service(&profile)?;
        let reopened = read(&context, &profile, &mut service, 130)?;
        assert_eq!(reopened, snapshot);
        let retry_after_lost_ack =
            context.handle(&profile, &mut service, &reject, TimestampMillis::new(150))?;
        assert_eq!(retry_after_lost_ack.receipt_id, accepted.receipt_id);
        assert_eq!(
            retry_after_lost_ack
                .details
                .as_ref()
                .map(|s| s.decisions.len()),
            Some(2)
        );
        let mut wrong_profile = request(
            "different-profile",
            "relation-alpha",
            3,
            6,
            DetailAction::Reject,
        );
        assert_eq!(
            context
                .handle(
                    &profile,
                    &mut service,
                    &wrong_profile,
                    TimestampMillis::new(160)
                )?
                .reason
                .as_deref(),
            Some("PROFILE_MISMATCH")
        );
        if let DetailRequest::DetailsDecide { decision } = &mut wrong_profile {
            decision.expected_profile_id = context.profile_id().to_owned();
            decision.selector.known_at_accept_seq = Some(after.accept_seq_head);
        }
        assert_eq!(
            context
                .handle(
                    &profile,
                    &mut service,
                    &wrong_profile,
                    TimestampMillis::new(160)
                )?
                .reason
                .as_deref(),
            Some("HISTORICAL_VIEW")
        );
        let history = context
            .handle(
                &profile,
                &mut service,
                &DetailRequest::DetailsRead {
                    selector: DetailSelector {
                        known_at_accept_seq: Some(before.accept_seq_head),
                        ..DetailSelector::default()
                    },
                },
                TimestampMillis::new(160),
            )?
            .details
            .ok_or("historical snapshot")?;
        assert!(history.decisions.is_empty());
        assert_eq!(history.corpus, original_projection.corpus);
        assert_eq!(history.source_digest, original_projection.source_digest);
        assert_eq!(
            retry_after_lost_ack.decision_sequence,
            accepted.decision_sequence
        );
        drop(service);
        drop(profile);
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    #[test]
    fn same_path_replacement_has_a_new_incarnation_even_with_same_source_and_revision() -> TestResult
    {
        let path = detail_test_root();
        let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        fixture::import_synthetic_corpus(&profile, corpus("alpha"))?;
        let original = context(&profile)?;
        let old_request = request(
            original.profile_id(),
            "relation-alpha",
            1,
            10,
            DetailAction::Reject,
        );
        assert_eq!(context(&profile)?.profile_id(), original.profile_id());
        drop(profile);
        std::fs::remove_dir_all(&path)?;
        let replacement = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        fixture::import_synthetic_corpus(&replacement, corpus("alpha"))?;
        let replacement_context = context(&replacement)?;
        assert_ne!(original.profile_id(), replacement_context.profile_id());
        let mut service = open_service(&replacement)?;
        assert_eq!(
            replacement_context
                .handle(
                    &replacement,
                    &mut service,
                    &old_request,
                    TimestampMillis::new(100)
                )?
                .reason
                .as_deref(),
            Some("PROFILE_MISMATCH")
        );
        assert_eq!(
            read(&replacement_context, &replacement, &mut service, 100)?.revision,
            1
        );
        drop(service);
        drop(replacement);
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    #[test]
    fn profile_audio_ranges_are_exact_and_bound_to_lecture_revision_and_profile() -> TestResult {
        let path = detail_test_root();
        let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [32; 32])?;
        let mut source = corpus("alpha");
        source.lectures.push(Lecture {
            id: "lecture-alpha".to_owned(),
            title: "Synthetic".to_owned(),
            segments: vec![],
            paragraphs: vec![],
            captures: vec![],
            review: vec![],
            links: BTreeMap::new(),
            explanation: String::new(),
        });
        let mut wav = vec![0_u8; 8200];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        for (index, byte) in wav[44..].iter_mut().enumerate() {
            *byte = u8::try_from(index % 251)?;
        }
        fixture::import_synthetic_corpus_with_audio(
            &profile,
            source,
            BTreeMap::from([("lecture-alpha".to_owned(), wav.clone())]),
        )?;
        let context = context(&profile)?;
        let mut service = open_service(&profile)?;
        let mut request = DetailAudioRequest {
            lecture_id: "lecture-alpha".to_owned(),
            expected_profile_id: context.profile_id().to_owned(),
            expected_revision: 1,
            selector: DetailSelector::default(),
            offset: 0,
            length: 4096,
        };
        let mut received = Vec::new();
        while received.len() < wav.len() {
            request.offset = u64::try_from(received.len())?;
            let audio = context
                .handle(
                    &profile,
                    &mut service,
                    &DetailRequest::DetailsAudio {
                        audio: request.clone(),
                    },
                    TimestampMillis::new(100),
                )?
                .audio
                .ok_or("missing audio")?;
            assert_eq!(audio.offset, request.offset);
            assert_eq!(
                audio.content_digest,
                hex::encode(ContentDigest::sha256(&wav).as_bytes())
            );
            received.extend(audio.bytes);
        }
        assert_eq!(received, wav);
        for (field, reason) in [
            (0, "PROFILE_MISMATCH"),
            (1, "REVISION_CONFLICT"),
            (2, "AUDIO_NOT_FOUND"),
            (3, "AUDIO_RANGE_INVALID"),
        ] {
            let mut bad = request.clone();
            match field {
                0 => bad.expected_profile_id = "wrong".to_owned(),
                1 => bad.expected_revision = 0,
                2 => bad.lecture_id = "missing".to_owned(),
                _ => bad.length = 4097,
            }
            assert_eq!(
                context
                    .handle(
                        &profile,
                        &mut service,
                        &DetailRequest::DetailsAudio { audio: bad },
                        TimestampMillis::new(100)
                    )?
                    .reason
                    .as_deref(),
                Some(reason)
            );
        }
        drop(service);
        drop(profile);
        std::fs::remove_dir_all(path)?;
        Ok(())
    }
}
