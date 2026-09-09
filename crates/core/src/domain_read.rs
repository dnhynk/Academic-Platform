//! Core-owned normal-domain read projection. No imported corpus or write path.
use crate::Core;
use crate::authenticated_acceptance::VaultAccess;
use academic_contracts::{VerifiedBatch, verify_signed_batch};
use academic_domain::{
    Actor, ArtifactDescriptor, Claim, ClaimObject, ContentDigest, DomainId, EntityId, EventPayload,
    EvidenceId, ScopeId, TimestampMillis, ValidInterval,
    entity_registry::{EntityKind, PREDICATE_ENTITY_KIND, PREDICATE_ENTITY_LABEL, RegistryFact},
};
use academic_ledger::{AcceptedEvent, AuthorityPolicy, ResolutionQuery, ResolutionResult};
use academic_rpc::domain_details as dto;
use academic_store::queries::{DomainHistorySnapshot, QueryError};
use std::collections::{BTreeMap, BTreeSet};

const SNAPSHOT_ADAPTER: &str = "academic.domain-details.signed-snapshot.v3";
const POLICY_VERSION: &str = "academic.domain-details.policies.v3";
const SOURCE_REF: &str = "canonical";

type ReadResult<T> = Result<T, dto::ReadFailure>;

pub(crate) struct ReadSnapshot {
    core: Core,
    batches: Vec<VerifiedBatch>,
    source: DomainHistorySnapshot,
}

pub(crate) fn authenticate(
    source: DomainHistorySnapshot,
    trust: &[academic_contracts::DeviceAuthorization],
) -> ReadResult<ReadSnapshot> {
    if source.history.revision > academic_rpc::details::MAX_SAFE_INTEGER
        || source.coordinates.known_at_accept_seq > academic_rpc::details::MAX_SAFE_INTEGER
        || source.source_authority.source_outbox_seq > academic_rpc::details::MAX_SAFE_INTEGER
    {
        return Err(dto::ReadFailure::SelectorUnavailable);
    }
    ReadSnapshot::authenticate(source, trust)
}

pub(crate) fn project(
    snapshot: &ReadSnapshot,
    vault: VaultAccess<'_>,
    profile_id: &str,
    query: &dto::Query,
) -> ReadResult<dto::DomainProjection> {
    let mut projector = Projector::new(snapshot, vault, profile_id)?;
    projector.query(query)?;
    projector.finish()
}

// Exact existing ACADEMIC_PREDICATE_POLICIES_V1 encoding, restricted to this
// projector's fixed registry. The plaintext equivalence test guards its bytes.
fn policy_hash(
    entries: &BTreeMap<academic_domain::PredicateId, AuthorityPolicy>,
) -> ReadResult<ContentDigest> {
    if POLICY_VERSION.trim().is_empty() || POLICY_VERSION.contains('\0') {
        return Err(dto::ReadFailure::ProjectionUnavailable);
    }
    let mut canonical = Vec::new();
    let mut append = |field: &[u8]| -> ReadResult<()> {
        let len = u64::try_from(field.len()).map_err(|_| dto::ReadFailure::ResultTooLarge)?;
        canonical.extend_from_slice(&len.to_be_bytes());
        canonical.extend_from_slice(field);
        Ok(())
    };
    append(b"ACADEMIC_PREDICATE_POLICIES_V1")?;
    append(POLICY_VERSION.as_bytes())?;
    for (predicate, policy) in entries {
        append(predicate.as_str().as_bytes())?;
        append(match policy {
            AuthorityPolicy::UserOwned => b"USER_OWNED",
            AuthorityPolicy::OfficialFact => b"OFFICIAL_FACT",
            AuthorityPolicy::ImplementationObservation => b"IMPLEMENTATION_OBSERVATION",
            AuthorityPolicy::CuratedRelation => b"CURATED_RELATION",
        })?;
    }
    Ok(ContentDigest::sha256(&canonical))
}

pub(crate) fn query_failure(error: QueryError) -> dto::ReadFailure {
    match error {
        QueryError::KnownAtBeyondHead { .. } | QueryError::IntegerOverflow(_) => {
            dto::ReadFailure::SelectorUnavailable
        }
        QueryError::Corrupt("signed history exceeds bounded detail replay") => {
            dto::ReadFailure::ResultTooLarge
        }
        _ => dto::ReadFailure::SourceVerificationFailed,
    }
}

impl ReadSnapshot {
    fn authenticate(
        source: DomainHistorySnapshot,
        trust: &[academic_contracts::DeviceAuthorization],
    ) -> ReadResult<Self> {
        let mut core = Core::new();
        let mut batches = Vec::new();
        for stored in &source.history.batches {
            let authorization = trust
                .iter()
                .find(|authorization| verify_signed_batch(&stored.envelope, authorization).is_ok())
                .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
            let (verified, receipt) = core
                .accept_signed_batch(&stored.envelope, authorization)
                .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?;
            if receipt.accept_seq_start != stored.accept_seq_start
                || receipt.accept_seq_end != stored.accept_seq_end
            {
                return Err(dto::ReadFailure::SourceVerificationFailed);
            }
            batches.push(verified);
        }
        source
            .verify_batch_binding(&batches)
            .map_err(query_failure)?;
        // V3 requires a complete accepted batch boundary. Authenticating an
        // envelope does not let provenance cite a later local event inside it.
        // The selector is refused explicitly, never rounded to either boundary.
        if source.coordinates.known_at_accept_seq != 0
            && !source
                .history
                .batches
                .iter()
                .any(|batch| batch.accept_seq_end == source.coordinates.known_at_accept_seq)
        {
            return Err(dto::ReadFailure::SelectorUnavailable);
        }
        let snapshot = Self {
            core,
            batches,
            source,
        };
        snapshot.verify_context()?;
        snapshot.verify_aggregate_rows()?;
        Ok(snapshot)
    }
    fn known(&self) -> u64 {
        self.source.coordinates.known_at_accept_seq
    }
    fn domain(&self) -> DomainId {
        self.source.domain_id
    }
    fn scope(&self) -> ScopeId {
        self.source.scope_id
    }
    fn valid(&self) -> TimestampMillis {
        self.source.coordinates.valid_at
    }
    fn events(&self) -> impl Iterator<Item = &AcceptedEvent> {
        self.core.ledger().accepted_events().iter().filter(|event| {
            event.accept_seq <= self.known() && event.event.domain_id == self.domain()
        })
    }
    fn verify_context(&self) -> ReadResult<()> {
        let registered = self
            .core
            .ledger()
            .accepted_events()
            .iter()
            .find_map(|event| {
                if event.accept_seq > self.known() {
                    return None;
                }
                match &event.event.payload {
                    EventPayload::ScopeRegistered(scope) if scope.id == self.scope() => Some(scope),
                    _ => None,
                }
            })
            .ok_or(dto::ReadFailure::ContextUnavailable)?;
        if registered.domain_id != self.domain() {
            return Err(dto::ReadFailure::ContextMismatch);
        }
        Ok(())
    }
    fn verify_aggregate_rows(&self) -> ReadResult<()> {
        let Some(aggregates) = &self.source.aggregates else {
            return if self
                .events()
                .any(|event| event.event.payload.registration().is_some())
            {
                Err(dto::ReadFailure::SourceVerificationFailed)
            } else {
                Ok(())
            };
        };
        let signed = self
            .events()
            .filter_map(|event| {
                let registration = event.event.payload.registration()?;
                let (valid_time, digest) = registration_interval(&event.event.payload)?;
                valid_time.contains(self.valid()).then_some((
                    event,
                    registration,
                    valid_time,
                    digest,
                ))
            })
            .collect::<Vec<_>>();
        if signed.len() != aggregates.rows.len() {
            return Err(dto::ReadFailure::SourceVerificationFailed);
        }
        for row in &aggregates.rows {
            if !signed.iter().any(|(event, registration, valid, digest)| {
                row.kind == registration.kind
                    && row.aggregate_id == *registration.id.as_bytes()
                    && row.registered_event_id == *event.event.id.as_bytes()
                    && row.accept_seq == event.accept_seq
                    && row.scope_id == registration.scope_id
                    && row.source_digest == *digest
                    && row.valid_from == valid.from()
                    && row.valid_to == valid.to()
            }) {
                return Err(dto::ReadFailure::SourceVerificationFailed);
            }
        }
        Ok(())
    }
    fn resolve(
        &self,
        subject: EntityId,
        predicate: &str,
        policy: AuthorityPolicy,
    ) -> ReadResult<ResolutionResult> {
        Ok(self.core.ledger().resolve(&ResolutionQuery {
            subject_entity_id: subject,
            predicate_id: academic_domain::PredicateId::parse(predicate)
                .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?,
            scope_id: self.scope(),
            valid_at: self.valid(),
            known_at_accept_seq: self.known(),
            policy,
        }))
    }
}

fn registration_interval(payload: &EventPayload) -> Option<(ValidInterval, Option<ContentDigest>)> {
    macro_rules! frame {
        ($record:expr) => {
            Some(($record.valid_time, $record.source_digest))
        };
    }
    match payload {
        EventPayload::CurriculumVersionPublished(r) => frame!(r),
        EventPayload::CourseRevisionPublished(r) => frame!(r),
        EventPayload::OfferingObserved(r) => frame!(r),
        EventPayload::AttemptRecorded(r) => frame!(r),
        EventPayload::RequirementSetPublished(r) => frame!(r),
        EventPayload::AuditComputed(r) => frame!(r),
        EventPayload::CapturePermissionRecorded(r) => frame!(r),
        EventPayload::LectureSessionRecorded(r) => frame!(r),
        EventPayload::TranscriptVersionAdded(r) => frame!(r),
        EventPayload::LectureDocumentPublished(r) => frame!(r),
        EventPayload::SnapshotRegistered(r) => frame!(r),
        EventPayload::FindingPublished(r) => frame!(r),
        EventPayload::ModelRunRecorded(r) => frame!(r),
        EventPayload::ProposalDisposed(r) => frame!(r),
        EventPayload::EgressDecided(r) => frame!(r),
        EventPayload::ConsentRecorded(r) => frame!(r),
        EventPayload::EntityIdentityChanged(r) => frame!(r),
        EventPayload::RetentionActionRecorded(r) => frame!(r),
        EventPayload::ScopeRegistered(_)
        | EventPayload::ArtifactRegistered(_)
        | EventPayload::EvidenceRegistered(_)
        | EventPayload::ClaimAsserted(_)
        | EventPayload::ClaimRelated(_)
        | EventPayload::DecisionRecorded(_) => None,
    }
}

struct Projector<'a> {
    snapshot: &'a ReadSnapshot,
    vault: VaultAccess<'a>,
    binding: dto::Binding,
    registry: dto::PolicyRegistry,
    provenance: BTreeMap<String, dto::ProvenanceEntry>,
    excerpt_bytes: usize,
    result: Option<dto::QueryResult>,
}

impl<'a> Projector<'a> {
    fn query(&mut self, query: &dto::Query) -> ReadResult<()> {
        // Question/project bodies and their canonical entity mappings have no
        // accepted producer contract yet. An empty index would hide that fact.
        let surface = match query {
            dto::Query::Index { surface } => *surface,
            dto::Query::Detail { subject } => subject.surface(),
        };
        if matches!(surface, dto::Surface::Question | dto::Surface::Project)
            || self.snapshot.source.aggregates.is_none()
        {
            return Err(dto::ReadFailure::ProjectionUnavailable);
        }
        let mut index = BTreeMap::new();
        let registrations = self
            .snapshot
            .events()
            .filter(|event| {
                event
                    .event
                    .payload
                    .registration()
                    .is_some_and(|r| r.scope_id == self.snapshot.scope())
                    && registration_interval(&event.event.payload)
                        .is_some_and(|(valid, _)| valid.contains(self.snapshot.valid()))
            })
            .cloned()
            .collect::<Vec<_>>();
        for accepted in registrations {
            if let dto::Query::Detail { subject } = query {
                let matches_subject = match (&accepted.event.payload, subject) {
                    (
                        EventPayload::LectureSessionRecorded(r),
                        dto::Subject::Lecture {
                            lecture_session_id, ..
                        },
                    ) => r.id == *lecture_session_id,
                    (
                        EventPayload::EntityIdentityChanged(r),
                        dto::Subject::Concept { entity_id, .. },
                    ) => r.entity_id == *entity_id,
                    _ => false,
                };
                if !matches_subject {
                    continue;
                }
            }
            let entry = match &accepted.event.payload {
                EventPayload::LectureSessionRecorded(r) if surface == dto::Surface::Lecture => {
                    self.registration(&accepted)?;
                    Some(dto::IndexEntry {
                        subject: dto::Subject::Lecture {
                            domain_id: r.domain_id,
                            scope_id: r.scope_id,
                            lecture_session_id: r.id,
                        },
                        title: dto::Field::unavailable(
                            dto::MissingReason::RegistrationOnly,
                            vec![dto::OriginalId::LectureSession { value: r.id }],
                        ),
                        source_ids: vec![
                            dto::OriginalId::LectureSession { value: r.id },
                            dto::OriginalId::Offering {
                                value: r.offering_id,
                            },
                        ],
                    })
                }
                EventPayload::EntityIdentityChanged(r) if surface == dto::Surface::Concept => {
                    match self.entity_kind(r.entity_id)? {
                        Some(EntityKind::Concept | EntityKind::ConceptSense) => {
                            self.registration(&accepted)?;
                            Some(dto::IndexEntry {
                                subject: dto::Subject::Concept {
                                    domain_id: r.domain_id,
                                    scope_id: r.scope_id,
                                    entity_id: r.entity_id,
                                },
                                title: self.title(r.entity_id)?,
                                source_ids: vec![
                                    dto::OriginalId::Entity { value: r.entity_id },
                                    dto::OriginalId::EntityIdentityChange { value: r.id },
                                ],
                            })
                        }
                        Some(_) => None,
                        None => return Err(dto::ReadFailure::ProjectionUnavailable),
                    }
                }
                _ => None,
            };
            if let Some(entry) = entry {
                index.insert(entry.subject.canonical_id(), entry);
                if index.len() > dto::MAX_INDEX_ENTRIES {
                    return Err(dto::ReadFailure::ResultTooLarge);
                }
            }
        }
        self.result = Some(match query {
            dto::Query::Index { .. } => {
                self.witness(
                    format!("index:{surface:?}"),
                    dto::QuerySelection::Index {
                        surface,
                        order: dto::IndexOrder::CanonicalIdAsc,
                        max_entries: 256,
                    },
                    index.len(),
                )?;
                dto::QueryResult::Index {
                    surface,
                    entries: index.into_values().collect(),
                }
            }
            dto::Query::Detail { subject } => {
                let entry = index
                    .remove(&subject.canonical_id())
                    .ok_or(dto::ReadFailure::SubjectNotFound)?;
                let detail = match subject {
                    dto::Subject::Concept { entity_id, .. } => dto::DomainDetail::Concept {
                        subject: subject.clone(),
                        title: entry.title,
                        state: dto::Field::unavailable(
                            dto::MissingReason::ProducerNotConnected,
                            entry.source_ids.clone(),
                        ),
                        freshness: dto::Field::unavailable(
                            dto::MissingReason::ProducerNotConnected,
                            entry.source_ids.clone(),
                        ),
                        last_strong_evidence: dto::Field::unavailable(
                            dto::MissingReason::ProducerNotConnected,
                            entry.source_ids,
                        ),
                        relation_groups: self.concept_groups(subject, *entity_id)?,
                    },
                    dto::Subject::Lecture {
                        lecture_session_id, ..
                    } => {
                        let mut original_ids = entry.source_ids;
                        let mut transcript_ids = Vec::new();
                        let mut document_ids = Vec::new();
                        let related = self
                            .snapshot
                            .events()
                            .filter(|event| {
                                event
                                    .event
                                    .payload
                                    .registration()
                                    .is_some_and(|r| r.scope_id == self.snapshot.scope())
                                    && registration_interval(&event.event.payload)
                                        .is_some_and(|(v, _)| v.contains(self.snapshot.valid()))
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        for accepted in related {
                            let id = match &accepted.event.payload {
                                EventPayload::TranscriptVersionAdded(r)
                                    if r.lecture_session_id == *lecture_session_id =>
                                {
                                    let id = dto::OriginalId::TranscriptVersion { value: r.id };
                                    transcript_ids.push(id.clone());
                                    Some(id)
                                }
                                EventPayload::LectureDocumentPublished(r)
                                    if r.lecture_session_id == *lecture_session_id =>
                                {
                                    let id = dto::OriginalId::LectureDocument { value: r.id };
                                    document_ids.push(id.clone());
                                    Some(id)
                                }
                                _ => None,
                            };
                            if let Some(id) = id {
                                self.registration(&accepted)?;
                                original_ids.push(id);
                            }
                        }
                        let transcript_reason = if transcript_ids.is_empty() {
                            dto::MissingReason::NoAcceptedSource
                        } else {
                            dto::MissingReason::RegistrationOnly
                        };
                        let document_reason = if document_ids.is_empty() {
                            dto::MissingReason::NoAcceptedSource
                        } else {
                            dto::MissingReason::RegistrationOnly
                        };
                        dto::DomainDetail::Lecture {
                            subject: subject.clone(),
                            title: entry.title,
                            entity_id: dto::Field::unavailable(
                                dto::MissingReason::ProducerNotConnected,
                                original_ids.clone(),
                            ),
                            transcript: dto::Field::unavailable(transcript_reason, transcript_ids),
                            document: dto::Field::unavailable(
                                document_reason,
                                document_ids.clone(),
                            ),
                            coverage: dto::Field::unavailable(
                                dto::MissingReason::ProducerNotConnected,
                                document_ids,
                            ),
                            captures: dto::Field::unavailable(
                                dto::MissingReason::ProducerNotConnected,
                                original_ids.clone(),
                            ),
                            review: dto::Field::unavailable(
                                dto::MissingReason::ProducerNotConnected,
                                original_ids.clone(),
                            ),
                            original_media: dto::Field::unavailable(
                                dto::MissingReason::ProducerNotConnected,
                                original_ids.clone(),
                            ),
                            original_ids,
                            relation_groups: unavailable_groups(&[
                                dto::GroupKind::ConceptCandidates,
                                dto::GroupKind::Questions,
                                dto::GroupKind::PrerequisiteGaps,
                                dto::GroupKind::NextLecturePreparation,
                                dto::GroupKind::Assessments,
                            ]),
                        }
                    }
                    _ => return Err(dto::ReadFailure::ProjectionUnavailable),
                };
                dto::QueryResult::Detail {
                    detail: Box::new(detail),
                }
            }
        });
        Ok(())
    }

    fn registration(&mut self, accepted: &AcceptedEvent) -> ReadResult<String> {
        let (id, parent) = match &accepted.event.payload {
            EventPayload::LectureSessionRecorded(r) => (
                dto::OriginalId::LectureSession { value: r.id },
                dto::OriginalId::Offering {
                    value: r.offering_id,
                },
            ),
            EventPayload::TranscriptVersionAdded(r) => (
                dto::OriginalId::TranscriptVersion { value: r.id },
                dto::OriginalId::LectureSession {
                    value: r.lecture_session_id,
                },
            ),
            EventPayload::LectureDocumentPublished(r) => (
                dto::OriginalId::LectureDocument { value: r.id },
                dto::OriginalId::LectureSession {
                    value: r.lecture_session_id,
                },
            ),
            EventPayload::EntityIdentityChanged(r) => (
                dto::OriginalId::EntityIdentityChange { value: r.id },
                dto::OriginalId::Entity { value: r.entity_id },
            ),
            _ => return Err(dto::ReadFailure::ProjectionUnavailable),
        };
        let (valid, digest) = registration_interval(&accepted.event.payload)
            .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        let (event, event_ref) = self.event(accepted, Some(self.snapshot.scope()))?;
        self.insert(
            format!("registration:{}", accepted.event.id),
            dto::ProvenanceOrigin::RegisteredAggregate {
                id,
                parent: Some(parent),
                domain_id: self.snapshot.domain(),
                scope_id: self.snapshot.scope(),
                registered_event_id: accepted.event.id,
                origin_event: available(event, vec![event_ref])?,
                accept_seq: accepted.accept_seq,
                valid_from_ms: valid.from().value().into(),
                valid_to_ms: valid.to().map(|v| v.value().into()),
                source_digest: digest.map(Into::into),
            },
        )
    }

    fn entity_kind(&mut self, entity: EntityId) -> ReadResult<Option<EntityKind>> {
        let resolution =
            self.snapshot
                .resolve(entity, PREDICATE_ENTITY_KIND, AuthorityPolicy::UserOwned)?;
        if !resolution.conflicting_claim_ids.is_empty() {
            return Err(dto::ReadFailure::ProjectionUnavailable);
        }
        let mut kinds = BTreeSet::new();
        for id in resolution.active_claim_ids {
            let claim = self
                .snapshot
                .core
                .ledger()
                .claim(id)
                .cloned()
                .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
            self.claim(&claim, AuthorityPolicy::UserOwned)?;
            if let Some(RegistryFact::EntityKindDeclared { kind, .. }) =
                RegistryFact::decode(&claim)
                    .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?
            {
                kinds.insert(kind);
            }
        }
        if kinds.len() > 1 {
            return Err(dto::ReadFailure::ProjectionUnavailable);
        }
        Ok(kinds.into_iter().next())
    }

    fn title(&mut self, entity: EntityId) -> ReadResult<dto::Field<String>> {
        let resolution =
            self.snapshot
                .resolve(entity, PREDICATE_ENTITY_LABEL, AuthorityPolicy::UserOwned)?;
        let mut refs = Vec::new();
        let mut values = BTreeSet::new();
        let ids = resolution
            .active_claim_ids
            .iter()
            .chain(&resolution.conflicting_claim_ids)
            .copied()
            .collect::<Vec<_>>();
        for id in &ids {
            let claim = self
                .snapshot
                .core
                .ledger()
                .claim(*id)
                .cloned()
                .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
            let reference = self.claim(&claim, AuthorityPolicy::UserOwned)?;
            if let Some(RegistryFact::LabelDeclared { text, .. }) = RegistryFact::decode(&claim)
                .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?
            {
                refs.push(reference);
                values.insert(text);
            }
        }
        if !resolution.conflicting_claim_ids.is_empty() || values.len() > 1 {
            return Ok(dto::Field::unavailable(
                dto::MissingReason::AmbiguousInScope,
                ids.into_iter()
                    .map(|value| dto::OriginalId::Claim { value })
                    .collect(),
            ));
        }
        match values.into_iter().next() {
            Some(text) => available(text, refs),
            None => Ok(dto::Field::unavailable(
                dto::MissingReason::NoAcceptedSource,
                vec![dto::OriginalId::Entity { value: entity }],
            )),
        }
    }

    fn witness(
        &mut self,
        reference: String,
        selection: dto::QuerySelection,
        count: usize,
    ) -> ReadResult<String> {
        self.insert(
            reference,
            dto::ProvenanceOrigin::VerifiedQuery {
                query: dto::VerifiedQuery {
                    context: dto::Context {
                        domain_id: self.snapshot.domain(),
                        scope_id: self.snapshot.scope(),
                    },
                    coordinates: dto::QueryCoordinates {
                        known_at_accept_seq: self.snapshot.known(),
                        valid_at_ms: self.binding.valid_at_ms,
                    },
                    selection,
                    completeness: dto::Completeness::Complete,
                    returned_count: u64::try_from(count)
                        .map_err(|_| dto::ReadFailure::ResultTooLarge)?,
                },
            },
        )
    }

    fn concept_groups(
        &mut self,
        subject: &dto::Subject,
        entity: EntityId,
    ) -> ReadResult<Vec<dto::RelationGroup>> {
        let mut groups = unavailable_groups(&[
            dto::GroupKind::EvidenceTimeline,
            dto::GroupKind::Contradictions,
            dto::GroupKind::Prerequisites,
            dto::GroupKind::Questions,
            dto::GroupKind::Snu,
            dto::GroupKind::Projects,
            dto::GroupKind::Competencies,
            dto::GroupKind::Roles,
        ]);
        let predicate = academic_domain::predicates::PredicateName::UsedIn;
        let descriptor = predicate.descriptor();
        let resolution = self.snapshot.resolve(
            entity,
            descriptor.predicate_id,
            AuthorityPolicy::CuratedRelation,
        )?;
        if !resolution.conflicting_claim_ids.is_empty() {
            groups.push(dto::RelationGroup {
                kind: dto::GroupKind::UsedIn,
                relations: dto::Field::unavailable(
                    dto::MissingReason::AmbiguousInScope,
                    resolution
                        .conflicting_claim_ids
                        .into_iter()
                        .map(|value| dto::OriginalId::Claim { value })
                        .collect(),
                ),
            });
            return Ok(groups);
        }
        let mut relations = Vec::new();
        let mut ids = resolution.active_claim_ids;
        ids.sort();
        for id in ids {
            let claim = self
                .snapshot
                .core
                .ledger()
                .claim(id)
                .cloned()
                .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
            let ClaimObject::Entity(target) = claim.object else {
                return Err(dto::ReadFailure::SourceVerificationFailed);
            };
            let Some(target_type) = self.anchored_node_type(target)? else {
                groups.push(dto::RelationGroup {
                    kind: dto::GroupKind::UsedIn,
                    relations: dto::Field::unavailable(
                        dto::MissingReason::UnsupportedField,
                        vec![
                            dto::OriginalId::Claim { value: id },
                            dto::OriginalId::Entity { value: target },
                        ],
                    ),
                });
                return Ok(groups);
            };
            let source_type = self
                .anchored_node_type(entity)?
                .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
            if !descriptor.subject_types.contains(&source_type)
                || !descriptor.object_types.contains(&target_type)
            {
                groups.push(dto::RelationGroup {
                    kind: dto::GroupKind::UsedIn,
                    relations: dto::Field::unavailable(
                        dto::MissingReason::UnsupportedField,
                        vec![dto::OriginalId::Claim { value: id }],
                    ),
                });
                return Ok(groups);
            }
            let evidence = claim
                .evidence_ids
                .iter()
                .map(|id| {
                    self.snapshot
                        .core
                        .ledger()
                        .evidence(*id)
                        .map(academic_domain::predicates::EdgeEvidence::from_item)
                        .ok_or(dto::ReadFailure::SourceVerificationFailed)
                })
                .collect::<ReadResult<Vec<_>>>()?;
            let key = academic_domain::predicates::EdgeKey::new(predicate, entity, target)
                .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?;
            if (academic_domain::predicates::EdgeAssertion {
                key,
                subject_type: source_type,
                object_type: target_type,
                authority_class: claim.authority_class,
                qualifiers: &[],
                evidence: &evidence,
            })
            .validate()
            .is_err()
            {
                groups.push(dto::RelationGroup {
                    kind: dto::GroupKind::UsedIn,
                    relations: dto::Field::unavailable(
                        dto::MissingReason::InsufficientEvidence,
                        vec![dto::OriginalId::Claim { value: id }],
                    ),
                });
                return Ok(groups);
            }
            let provenance_ref = self.claim(&claim, AuthorityPolicy::CuratedRelation)?;
            relations.push(dto::Relation {
                claim_id: id,
                subject: subject.clone(),
                predicate_id: descriptor.predicate_id.to_owned(),
                target: available(
                    dto::RelationTarget::Original(dto::OriginalId::Entity { value: target }),
                    vec![provenance_ref.clone()],
                )?,
                scope_id: claim.scope_id,
                label: dto::Field::unavailable(
                    dto::MissingReason::UnsupportedField,
                    vec![dto::OriginalId::Claim { value: id }],
                ),
                confidence: dto::Field::unavailable(
                    dto::MissingReason::ProducerNotConnected,
                    vec![dto::OriginalId::Claim { value: id }],
                ),
                user_decision: self.user_decision(&claim)?,
                provenance_ref,
                writes: dto::RelationWrites {
                    state: dto::UnavailableState::Unavailable,
                    reason: dto::RelationWriteReason::CanonicalRelationWriteAdapterMissing,
                },
            });
        }
        let reference = self.witness(
            format!("relations:{entity}:used-in"),
            dto::QuerySelection::Relations {
                subject: subject.clone(),
                predicate_ids: vec![descriptor.predicate_id.to_owned()],
                direction: dto::Direction::Outgoing,
                order: dto::RelationOrder::ClaimIdAsc,
                max_entries: 4096,
            },
            relations.len(),
        )?;
        groups.push(dto::RelationGroup {
            kind: dto::GroupKind::UsedIn,
            relations: available(relations, vec![reference])?,
        });
        Ok(groups)
    }

    fn anchored_node_type(
        &mut self,
        entity: EntityId,
    ) -> ReadResult<Option<academic_domain::predicates::NodeType>> {
        if !self.snapshot.events().any(|event| matches!(&event.event.payload, EventPayload::EntityIdentityChanged(r) if r.entity_id == entity && r.scope_id == self.snapshot.scope() && r.valid_time.contains(self.snapshot.valid()))) { return Ok(None); }
        use academic_domain::predicates::NodeType;
        Ok(match self.entity_kind(entity)? {
            Some(EntityKind::Concept) => Some(NodeType::Concept),
            Some(EntityKind::ConceptSense) => Some(NodeType::ConceptSense),
            Some(EntityKind::Field) => Some(NodeType::Field),
            _ => None,
        })
    }

    fn user_decision(&mut self, claim: &Claim) -> ReadResult<dto::Field<dto::UserDecisionRef>> {
        let accepted = self.snapshot.events().filter(|event| {
            let EventPayload::DecisionRecorded(decision) = &event.event.payload else { return false; };
            decision.resolution_slot.subject_entity_id == claim.subject_entity_id
                && decision.resolution_slot.predicate_id == claim.predicate_id
                && decision.resolution_slot.scope_id == claim.scope_id
                && decision.valid_time.contains(self.snapshot.valid())
                && (decision.target_object == claim.object || matches!(decision.action,
                    academic_domain::DecisionAction::Replace { replacement_claim_id }
                        if self.snapshot.core.ledger().claim(replacement_claim_id).is_some_and(|replacement| replacement.object == claim.object)))
        }).last().cloned();
        let Some(accepted) = accepted else {
            return Ok(dto::Field::unavailable(
                dto::MissingReason::NoAcceptedSource,
                vec![dto::OriginalId::Claim { value: claim.id }],
            ));
        };
        let EventPayload::DecisionRecorded(decision) = &accepted.event.payload else {
            return Err(dto::ReadFailure::SourceVerificationFailed);
        };
        let Actor::User { user_id } = accepted.event.actor else {
            return Err(dto::ReadFailure::SourceVerificationFailed);
        };
        let (action, replacement_claim_id) = match decision.action {
            academic_domain::DecisionAction::Confirm => (dto::UserAction::Confirm, None),
            academic_domain::DecisionAction::Reject => (dto::UserAction::Reject, None),
            academic_domain::DecisionAction::Replace {
                replacement_claim_id,
            } => (dto::UserAction::Replace, Some(replacement_claim_id)),
        };
        let (_, reference) = self.event(&accepted, Some(claim.scope_id))?;
        available(
            dto::UserDecisionRef {
                decision_id: decision.id,
                actor: user_id,
                action,
                decided_at_ms: decision.decided_at.value().into(),
                target_claim_id: decision.target_claim_id,
                replacement_claim_id,
            },
            vec![reference],
        )
    }

    fn new(
        snapshot: &'a ReadSnapshot,
        vault: VaultAccess<'a>,
        profile_id: &str,
    ) -> ReadResult<Self> {
        let entries = policy_entries()?;
        let policy_hash = policy_hash(&entries)?;
        Ok(Self {
            snapshot,
            vault,
            binding: dto::Binding {
                profile_id: profile_id.to_owned(),
                revision: snapshot.source.history.revision,
                domain_id: snapshot.domain(),
                scope_id: snapshot.scope(),
                known_at_accept_seq: snapshot.known(),
                valid_at_ms: u64::try_from(snapshot.valid().value())
                    .map_err(|_| dto::ReadFailure::SelectorUnavailable)?,
                source_outbox_seq: snapshot.source.source_authority.source_outbox_seq,
                source_ledger_digest: snapshot.source.source_authority.source_ledger_digest.into(),
            },
            registry: dto::PolicyRegistry {
                resolver_version: academic_store::queries::PROJECTION_RESOLVER_VERSION.to_owned(),
                policy_registry_version: POLICY_VERSION.to_owned(),
                policy_registry_hash: policy_hash.into(),
                predicate_policies: entries
                    .into_iter()
                    .map(|(predicate, policy)| dto::PredicatePolicy {
                        predicate_id: predicate.as_str().to_owned(),
                        policy: wire_policy(policy),
                    })
                    .collect(),
            },
            provenance: BTreeMap::new(),
            excerpt_bytes: 0,
            result: None,
        })
    }

    fn insert(&mut self, reference: String, origin: dto::ProvenanceOrigin) -> ReadResult<String> {
        let entry = dto::ProvenanceEntry {
            r#ref: reference.clone(),
            source_ref: SOURCE_REF.to_owned(),
            origin,
        };
        if self
            .provenance
            .get(&reference)
            .is_some_and(|old| old != &entry)
        {
            return Err(dto::ReadFailure::SourceVerificationFailed);
        }
        self.provenance.insert(reference.clone(), entry);
        if self.provenance.len() > dto::MAX_PROVENANCE_ENTRIES {
            return Err(dto::ReadFailure::ResultTooLarge);
        }
        Ok(reference)
    }

    fn event(
        &mut self,
        accepted: &AcceptedEvent,
        scope_id: Option<ScopeId>,
    ) -> ReadResult<(dto::OriginEvent, String)> {
        let batch = self
            .snapshot
            .batches
            .iter()
            .find(|batch| batch.batch().batch_id == accepted.batch_id)
            .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        let event = dto::OriginEvent {
            event_id: accepted.event.id,
            origin_seq: accepted.event.origin_seq.into(),
            origin_observed_at_ms: accepted.event.origin_observed_at.value().into(),
            actor: actor(&accepted.event.actor),
            domain_id: accepted.event.domain_id,
            accepted_batch_envelope_digest: batch.envelope_hash().into(),
            accepted_batch_payload_digest: batch.payload_hash().into(),
        };
        let reference = self.insert(
            format!("event:{}", accepted.event.id),
            dto::ProvenanceOrigin::AcceptedEvent {
                event: event.clone(),
                accept_seq: accepted.accept_seq,
                scope_id,
            },
        )?;
        Ok((event, reference))
    }

    fn artifact(&mut self, descriptor: &ArtifactDescriptor) -> ReadResult<String> {
        let accepted = self.snapshot.events().find(|event| matches!(&event.event.payload, EventPayload::ArtifactRegistered(found) if found == descriptor))
            .cloned().ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        self.vault
            .verify(descriptor)
            .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?;
        let (_, registered_event_ref) = self.event(&accepted, None)?;
        self.insert(
            format!("artifact:{}", descriptor.id),
            dto::ProvenanceOrigin::VerifiedArtifact {
                artifact_id: descriptor.id,
                domain_id: descriptor.domain_id,
                artifact_digest: descriptor.content_digest.into(),
                format_version: descriptor.format_version,
                media_type: descriptor.media_type.as_str().to_owned(),
                byte_length: descriptor.byte_length.into(),
                registered_event_ref,
            },
        )
    }

    fn evidence(&mut self, id: EvidenceId) -> ReadResult<dto::EvidenceRef> {
        let item = self
            .snapshot
            .core
            .ledger()
            .evidence(id)
            .cloned()
            .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        let descriptor = self
            .snapshot
            .core
            .ledger()
            .artifact(item.artifact_id)
            .cloned()
            .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        let (index, representation) = descriptor
            .evidence_representations
            .iter()
            .enumerate()
            .find(|(_, representation)| representation.locator == item.locator)
            .ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        if descriptor.domain_id != self.snapshot.domain()
            || item.excerpt_digest != representation.content_digest
        {
            return Err(dto::ReadFailure::SourceVerificationFailed);
        }
        // The accepted representation must cover the exact complete source. Other
        // locators remain typed provenance without an invented byte mapping.
        let excerpt = match &item.locator {
            academic_domain::EvidenceLocator::TextBytes {
                start: 0,
                end,
                source_digest,
            } if *end == descriptor.byte_length
                && *source_digest == descriptor.content_digest
                && *end <= 65_536 =>
            {
                let capability = self.vault.verify(&descriptor);
                // Vault read-back deliberately reports both a missing object and
                // corrupt bytes as IntegrityMismatch. Only a fresh no-follow
                // absence check of that already validated canonical path permits
                // the missing-body outcome; an existing object still fails closed.
                let body_missing = match &capability {
                    Err(academic_vault::VaultError::IntegrityMismatch(path)) => matches!(
                        std::fs::symlink_metadata(path),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound
                    ),
                    _ => false,
                };
                if body_missing {
                    return Ok(dto::EvidenceRef {
                        evidence_id: id,
                        artifact_id: item.artifact_id,
                        representation_index: index as u64,
                        locator: locator(&item.locator),
                        excerpt_digest: item.excerpt_digest.into(),
                        role: item.role,
                        strength: item.strength,
                        extraction_method: item.extraction_method,
                        extractor_version: item.extractor_version,
                        excerpt: dto::Field::unavailable(
                            dto::MissingReason::SourceBodyUnavailable,
                            vec![dto::OriginalId::Artifact {
                                value: descriptor.id,
                            }],
                        ),
                    });
                }
                let mut capability =
                    capability.map_err(|_| dto::ReadFailure::SourceVerificationFailed)?;
                let artifact_ref = self.artifact(&descriptor)?;
                let mut bytes = Vec::new();
                let mut offset = 0_u64;
                while offset < *end {
                    let length = usize::try_from((*end - offset).min(4096))
                        .map_err(|_| dto::ReadFailure::ResultTooLarge)?;
                    bytes.extend(
                        capability
                            .read_verified_range(offset, length)
                            .map_err(|_| dto::ReadFailure::SourceVerificationFailed)?,
                    );
                    offset += length as u64;
                }
                if ContentDigest::sha256(&bytes) != item.excerpt_digest {
                    return Err(dto::ReadFailure::SourceVerificationFailed);
                }
                match String::from_utf8(bytes) {
                    Ok(text) => available(
                        dto::Excerpt {
                            text,
                            encoding: dto::TextEncoding::Utf8,
                        },
                        vec![artifact_ref],
                    )?,
                    Err(_) => dto::Field::unavailable(
                        dto::MissingReason::UnsupportedField,
                        vec![dto::OriginalId::Evidence { value: id }],
                    ),
                }
            }
            _ => dto::Field::unavailable(
                dto::MissingReason::UnsupportedField,
                vec![dto::OriginalId::Evidence { value: id }],
            ),
        };
        Ok(dto::EvidenceRef {
            evidence_id: id,
            artifact_id: item.artifact_id,
            representation_index: index as u64,
            locator: locator(&item.locator),
            excerpt_digest: item.excerpt_digest.into(),
            role: item.role,
            strength: item.strength,
            extraction_method: item.extraction_method,
            extractor_version: item.extractor_version,
            excerpt,
        })
    }

    fn claim(&mut self, claim: &Claim, policy: AuthorityPolicy) -> ReadResult<String> {
        let reference = format!("claim:{}", claim.id);
        if let Some(entry) = self.provenance.get(&reference) {
            return if matches!(&entry.origin, dto::ProvenanceOrigin::AcceptedClaim { resolution_policy, .. } if *resolution_policy == wire_policy(policy))
            {
                Ok(reference)
            } else {
                Err(dto::ReadFailure::SourceVerificationFailed)
            };
        }
        let accepted = self.snapshot.events().find(|event| matches!(&event.event.payload, EventPayload::ClaimAsserted(found) if found == claim))
            .cloned().ok_or(dto::ReadFailure::SourceVerificationFailed)?;
        if claim.scope_id != self.snapshot.scope() {
            return Err(dto::ReadFailure::SourceVerificationFailed);
        }
        let mut evidence = Vec::new();
        for id in &claim.evidence_ids {
            let item = self.evidence(*id)?;
            if let dto::Field::Available { value, .. } = &item.excerpt {
                self.excerpt_bytes = self
                    .excerpt_bytes
                    .checked_add(value.text.len())
                    .ok_or(dto::ReadFailure::ResultTooLarge)?;
                if self.excerpt_bytes > dto::MAX_EXCERPT_BYTES {
                    return Err(dto::ReadFailure::ResultTooLarge);
                }
            }
            evidence.push(item);
        }
        let (event, event_ref) = self.event(&accepted, Some(claim.scope_id))?;
        self.insert(
            reference,
            dto::ProvenanceOrigin::AcceptedClaim {
                claim_id: claim.id,
                domain_id: self.snapshot.domain(),
                scope_id: claim.scope_id,
                authority_class: claim.authority_class,
                epistemic_status: claim.epistemic_status,
                origin_event: available(event, vec![event_ref])?,
                accept_seq: accepted.accept_seq,
                valid_from_ms: claim.valid_time.from().value().into(),
                valid_to_ms: claim.valid_time.to().map(|time| time.value().into()),
                evidence,
                resolution_policy: wire_policy(policy),
                graph_row: None,
            },
        )
    }

    fn finish(self) -> ReadResult<dto::DomainProjection> {
        let coordinates = dto::SourceCoordinates {
            domain_id: self.binding.domain_id,
            known_at_accept_seq: self.binding.known_at_accept_seq,
            valid_at_ms: self.binding.valid_at_ms,
            source_outbox_seq: self.binding.source_outbox_seq,
            source_ledger_digest: self.binding.source_ledger_digest.clone(),
        };
        Ok(dto::DomainProjection {
            source: dto::ProjectionSource {
                kind: dto::SourceKind::DomainProjection,
                projector_version: dto::PROJECTOR_VERSION.to_owned(),
            },
            binding: self.binding,
            read_sources: vec![dto::ReadSource {
                r#ref: SOURCE_REF.to_owned(),
                authority: dto::ReadAuthority::CanonicalSnapshot {
                    coordinates,
                    snapshot_adapter_version: SNAPSHOT_ADAPTER.to_owned(),
                    policy_registry: self.registry,
                    aggregate_source_row_digest: self
                        .snapshot
                        .source
                        .aggregates
                        .as_ref()
                        .map(|snapshot| snapshot.source_row_digest.into()),
                },
            }],
            provenance: self.provenance.into_values().collect(),
            result: self.result.ok_or(dto::ReadFailure::ProjectionUnavailable)?,
        })
    }
}

fn policy_entries() -> ReadResult<BTreeMap<academic_domain::PredicateId, AuthorityPolicy>> {
    let identities = academic_domain::entity_registry::REGISTRY_PREDICATES
        .iter()
        .map(|predicate| (*predicate, AuthorityPolicy::UserOwned));
    let graph = [(
        academic_domain::predicates::PredicateName::UsedIn
            .descriptor()
            .predicate_id,
        AuthorityPolicy::CuratedRelation,
    )];
    identities
        .chain(graph)
        .map(|(predicate, policy)| {
            Ok((
                academic_domain::PredicateId::parse(predicate)
                    .map_err(|_| dto::ReadFailure::ProjectionUnavailable)?,
                policy,
            ))
        })
        .try_fold(BTreeMap::new(), |mut entries, entry: ReadResult<_>| {
            let (predicate, policy) = entry?;
            if entries.insert(predicate, policy).is_some() {
                return Err(dto::ReadFailure::ProjectionUnavailable);
            }
            Ok(entries)
        })
}

fn available<T>(value: T, refs: Vec<String>) -> ReadResult<dto::Field<T>> {
    dto::Field::available(value, refs).map_err(|_| dto::ReadFailure::ResultTooLarge)
}

fn unavailable_groups(kinds: &[dto::GroupKind]) -> Vec<dto::RelationGroup> {
    kinds
        .iter()
        .map(|kind| dto::RelationGroup {
            kind: *kind,
            relations: dto::Field::unavailable(
                dto::MissingReason::ProducerNotConnected,
                Vec::new(),
            ),
        })
        .collect()
}

fn actor(actor: &Actor) -> dto::ActorRef {
    match actor {
        Actor::User { user_id } => dto::ActorRef::User { user_id: *user_id },
        Actor::ModelRun { run_id } => dto::ActorRef::ModelRun { run_id: *run_id },
        Actor::Importer { name, version } => dto::ActorRef::Importer {
            name: name.clone(),
            version: version.clone(),
        },
        Actor::DeterministicEngine { name, version } => dto::ActorRef::DeterministicEngine {
            name: name.clone(),
            version: version.clone(),
        },
        Actor::DeterministicPrediction {
            name,
            version,
            frozen_inputs_digest,
            rule_set_digest,
        } => dto::ActorRef::DeterministicPrediction {
            name: name.clone(),
            version: version.clone(),
            frozen_inputs_digest: (*frozen_inputs_digest).into(),
            rule_set_digest: (*rule_set_digest).into(),
        },
    }
}

fn locator(locator: &academic_domain::EvidenceLocator) -> dto::Locator {
    match locator {
        academic_domain::EvidenceLocator::Page { page_number } => dto::Locator::Page {
            page_number: *page_number,
        },
        academic_domain::EvidenceLocator::TextBytes {
            source_digest,
            start,
            end,
        } => dto::Locator::TextBytes {
            source_digest: (*source_digest).into(),
            start: *start,
            end: *end,
        },
        academic_domain::EvidenceLocator::TranscriptTime { start_ms, end_ms } => {
            dto::Locator::TranscriptTime {
                start_ms: *start_ms,
                end_ms: *end_ms,
            }
        }
        academic_domain::EvidenceLocator::RepositoryBytes {
            snapshot_digest,
            path,
            start,
            end,
        } => dto::Locator::RepositoryBytes {
            snapshot_digest: (*snapshot_digest).into(),
            path: path.as_str().to_owned(),
            start: *start,
            end: *end,
        },
    }
}

fn wire_policy(policy: AuthorityPolicy) -> dto::AuthorityPolicy {
    match policy {
        AuthorityPolicy::UserOwned => dto::AuthorityPolicy::UserOwned,
        AuthorityPolicy::OfficialFact => dto::AuthorityPolicy::OfficialFact,
        AuthorityPolicy::ImplementationObservation => {
            dto::AuthorityPolicy::ImplementationObservation
        }
        AuthorityPolicy::CuratedRelation => dto::AuthorityPolicy::CuratedRelation,
    }
}

#[cfg(all(test, feature = "plaintext-core"))]
mod tests {
    //! Actual schema-one acceptance exercises the authenticated claim/artifact seam.
    //! These tests deliberately do not stand in for the pending encrypted ordinary
    //! registration-command proof: that profile still refuses aggregate admission.
    use super::*;
    use crate::{details::DetailContext, service::AcceptanceService};
    use academic_contracts::{DeviceAuthorization, sign_batch};
    use academic_domain::{
        ArtifactRepresentation, AuthorityClass, BatchId, Confidentiality, DeviceId,
        EpistemicStatus, Event, EventId, EvidenceItem, EvidenceLocator, EvidenceRole,
        EvidenceStrength, MediaType, RetentionClass, ScopeDescriptor, UnsignedBatch,
    };
    use academic_store::{
        idempotency::AcceptanceCommand,
        path_policy::NativePathProbe,
        profile::{create_synthetic_profile, open_synthetic_profile},
    };
    use academic_store::{
        profile::SyntheticProfile,
        queries::{DomainHistoryRequest, domain_history_snapshot},
    };
    use academic_vault::{ArtifactIngestRequest, DomainKeyring};
    use ed25519_dalek::SigningKey;
    use std::{
        error::Error,
        path::PathBuf,
        str::FromStr,
        sync::atomic::{AtomicU64, Ordering},
    };

    type TestResult = Result<(), Box<dyn Error>>;
    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn fixed_policy_encoding_matches_the_plaintext_registry() -> TestResult {
        let entries = policy_entries().map_err(|reason| format!("{reason:?}"))?;
        let original = academic_projections::resolution::PredicatePolicies::new(
            POLICY_VERSION,
            entries.clone(),
        )?;
        let actual = policy_hash(&entries).map_err(|reason| format!("{reason:?}"))?;
        assert_eq!(actual, original.canonical_hash());
        assert_eq!(
            hex::encode(actual.as_bytes()),
            "af0c5f5caa612602de280d68edf5332f575c55310accbda9476c130f05d75060"
        );
        Ok(())
    }

    fn id<T: FromStr<Err = academic_domain::DomainError>>(
        number: u64,
    ) -> Result<T, academic_domain::DomainError> {
        format!("01900000-0000-7000-8000-{number:012x}").parse()
    }

    struct DomainTestProfile {
        path: PathBuf,
        profile: SyntheticProfile,
        service: AcceptanceService,
        context: DetailContext,
        authorization: DeviceAuthorization,
        signing: SigningKey,
        previous: Option<ContentDigest>,
        next: u64,
        revision: u64,
    }

    impl DomainTestProfile {
        fn new() -> Result<Self, Box<dyn Error>> {
            let base = std::env::temp_dir();
            #[cfg(unix)]
            let base = std::fs::canonicalize(base)?;
            let path = base.join(format!(
                "academic-domain-read-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let profile = create_synthetic_profile(&path, &NativePathProbe::default(), [0x92; 32])?;
            let signing = SigningKey::from_bytes(&[0x6e; 32]);
            let authorization = DeviceAuthorization::new(
                id::<DeviceId>(1)?,
                id::<EntityId>(2)?,
                signing.verifying_key(),
            );
            let context =
                DetailContext::new(&profile, authorization.clone(), signing.clone(), Vec::new())?;
            let service = Self::service(&profile)?;
            Ok(Self {
                path,
                profile,
                service,
                context,
                authorization,
                signing,
                previous: None,
                next: 1,
                revision: 0,
            })
        }

        fn service(profile: &SyntheticProfile) -> Result<AcceptanceService, Box<dyn Error>> {
            let mut keys = DomainKeyring::new();
            keys.insert(id(3)?, &[0x45; 32])?;
            Ok(AcceptanceService::open(profile, keys)?)
        }

        fn accept(&mut self, payloads: Vec<EventPayload>) -> Result<(), Box<dyn Error>> {
            let events = payloads
                .into_iter()
                .enumerate()
                .map(|(index, payload)| {
                    let sequence = self.next + u64::try_from(index)?;
                    Ok(Event {
                        id: id::<EventId>(1000 + sequence)?,
                        origin_seq: sequence,
                        origin_observed_at: TimestampMillis::new(-10),
                        domain_id: id(3)?,
                        actor: Actor::Importer {
                            name: "synthetic-source".into(),
                            version: "1".into(),
                        },
                        payload,
                    })
                })
                .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
            let end = self.next + u64::try_from(events.len())? - 1;
            let batch = UnsignedBatch {
                schema_version: academic_domain::EVENT_SCHEMA_VERSION_V4,
                batch_id: id::<BatchId>(2000 + self.revision)?,
                device_id: self.authorization.device_id(),
                origin_seq_start: self.next,
                origin_seq_end: end,
                previous_batch_hash: self.previous,
                origin_created_at: TimestampMillis::new(0),
                events,
            };
            let envelope = sign_batch(&batch, &self.signing)?;
            let token = u8::try_from(self.revision + 1)?;
            let outcome = self.service.accept_signed_command(
                AcceptanceCommand {
                    request_id: [token; 16],
                    client_instance_id: [0x4f; 16],
                    idempotency_key: [token; 32],
                    expected_revision: Some(self.revision),
                    envelope_bytes: &envelope,
                },
                &self.authorization,
                TimestampMillis::new(100),
            )?;
            self.previous = Some(outcome.receipt.envelope_hash);
            self.revision = outcome.receipt.committed_revision;
            self.next = end + 1;
            Ok(())
        }

        fn source(
            &mut self,
            scope: u64,
            number: u64,
            label: &str,
        ) -> Result<PathBuf, Box<dyn Error>> {
            let source = format!("Synthetic exact text:  alpha,\n beta\t→ gamma.{number}");
            let sealed = self.service.vault().ingest(
                &ArtifactIngestRequest::new(
                    id(3000 + number)?,
                    MediaType::parse("text/plain")?,
                    id(3)?,
                    Confidentiality::Restricted,
                    RetentionClass::UserManaged,
                    id(4000 + number)?,
                ),
                source.as_bytes(),
            )?;
            let path = sealed.object_path().to_owned();
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
            let claim = Claim {
                id: id(6000 + number)?,
                subject_entity_id: id(5)?,
                predicate_id: academic_domain::PredicateId::parse(PREDICATE_ENTITY_LABEL)?,
                object: ClaimObject::Text(label.into()),
                scope_id: id(scope)?,
                authority_class: AuthorityClass::DirectObservation,
                epistemic_status: EpistemicStatus::CodeObserved,
                confidence: None,
                prediction_metadata: None,
                valid_time: ValidInterval::new(TimestampMillis::new(0), None)?,
                evidence_ids: vec![id(5000 + number)?],
            };
            self.accept(vec![
                EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: id(scope)?,
                    domain_id: id(3)?,
                    label: format!("scope {scope}"),
                }),
                EventPayload::ArtifactRegistered(descriptor.clone()),
                EventPayload::EvidenceRegistered(EvidenceItem {
                    id: id(5000 + number)?,
                    artifact_id: descriptor.id,
                    locator,
                    excerpt_digest: descriptor.content_digest,
                    role: EvidenceRole::Supports,
                    strength: EvidenceStrength::Direct,
                    extraction_method: "exact synthetic source".into(),
                    extractor_version: "1".into(),
                }),
                EventPayload::ClaimAsserted(claim),
            ])?;
            Ok(path)
        }

        fn snapshot(&self, scope: u64, known: Option<u64>) -> Result<ReadSnapshot, Box<dyn Error>> {
            let source = domain_history_snapshot(
                &mut self.profile.open_reader()?,
                &DomainHistoryRequest {
                    domain_id: id(3)?,
                    scope_id: id(scope)?,
                    known_at_accept_seq: known,
                    valid_at: TimestampMillis::new(50),
                },
            )?;
            ReadSnapshot::authenticate(source, std::slice::from_ref(&self.authorization))
                .map_err(|reason| format!("{reason:?}").into())
        }

        fn request(&self, scope: u64) -> Result<dto::DomainReadRequest, Box<dyn Error>> {
            Ok(dto::DomainReadRequest::DetailsDomainReadV3 {
                context: dto::Context {
                    domain_id: id(3)?,
                    scope_id: id(scope)?,
                },
                selector: dto::DomainSelector {
                    view: dto::DomainView::DomainDetailV3,
                    known_at_accept_seq: None,
                    valid_at_ms: Some(50),
                },
                query: dto::Query::Index {
                    surface: dto::Surface::Concept,
                },
            })
        }

        fn close(self) -> TestResult {
            drop(self.service);
            drop(self.profile);
            std::fs::remove_dir_all(self.path)?;
            Ok(())
        }
    }

    #[test]
    fn accepted_claims_keep_exact_text_scope_authority_and_original_coordinates() -> TestResult {
        let mut fixture = DomainTestProfile::new()?;
        fixture.source(4, 1, "  CONFIRMED\nreported label  ")?;
        fixture.source(6, 2, "other scope")?;
        for (scope, expected) in [(4, "  CONFIRMED\nreported label  "), (6, "other scope")] {
            let snapshot = fixture.snapshot(scope, None)?;
            let mut projector = Projector::new(
                &snapshot,
                VaultAccess::Plain(fixture.service.vault()),
                fixture.context.profile_id(),
            )
            .map_err(|r| format!("{r:?}"))?;
            let title = projector.title(id(5)?).map_err(|r| format!("{r:?}"))?;
            assert!(matches!(title, dto::Field::Available { value, .. } if value == expected));
            let claims = projector
                .provenance
                .values()
                .filter_map(|p| match &p.origin {
                    dto::ProvenanceOrigin::AcceptedClaim {
                        authority_class,
                        epistemic_status,
                        origin_event,
                        evidence,
                        ..
                    } => Some((authority_class, epistemic_status, origin_event, evidence)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(claims.len(), 1);
            let (authority, status, origin, evidence) = claims[0];
            assert_eq!(*authority, AuthorityClass::DirectObservation);
            assert_eq!(*status, EpistemicStatus::CodeObserved);
            assert!(
                matches!(origin, dto::Field::Available { value, .. } if matches!(value.actor, dto::ActorRef::Importer { .. }) && value.origin_observed_at_ms == (-10_i64).into())
            );
            assert!(
                matches!(&evidence[0].excerpt, dto::Field::Available { value, .. } if value.text == format!("Synthetic exact text:  alpha,\n beta\t→ gamma.{}", if scope == 4 { 1 } else { 2 }))
            );
        }
        // A label claim never manufactures the missing identity registration lane.
        assert!(matches!(
            fixture.context.read_domain(
                &fixture.profile,
                &fixture.service,
                &fixture.request(4)?,
                TimestampMillis::new(50)
            ),
            dto::DomainReadReply::Unavailable {
                reason: dto::ReadFailure::ProjectionUnavailable,
                ..
            }
        ));
        fixture.close()
    }

    #[test]
    fn selected_profiles_and_exact_history_remain_bound_after_update_and_reopen() -> TestResult {
        let mut first = DomainTestProfile::new()?;
        let mut second = DomainTestProfile::new()?;
        first.source(4, 1, "first profile")?;
        second.source(4, 1, "second profile")?;
        let original = first.snapshot(4, Some(4))?;
        let original_binding = original.source.source_authority.source_ledger_digest;
        first.source(6, 2, "later scope")?;
        let historical = first.snapshot(4, Some(4))?;
        assert_eq!(
            historical.source.source_authority.source_ledger_digest,
            original_binding
        );
        assert_eq!(
            historical
                .resolve(id(5)?, PREDICATE_ENTITY_LABEL, AuthorityPolicy::UserOwned)
                .map_err(|r| format!("{r:?}"))?
                .active_claim_ids,
            vec![id(6001)?]
        );
        assert_ne!(first.context.profile_id(), second.context.profile_id());
        assert!(matches!(
            first.context.read_domain(
                &second.profile,
                &second.service,
                &first.request(4)?,
                TimestampMillis::new(50)
            ),
            dto::DomainReadReply::Unavailable {
                reason: dto::ReadFailure::ProfileMismatch,
                ..
            }
        ));
        let old_identity = first.context.profile_id().to_owned();
        drop(first.service);
        first.profile = open_synthetic_profile(&first.path, &NativePathProbe::default())?;
        first.service = DomainTestProfile::service(&first.profile)?;
        first.context = DetailContext::new(
            &first.profile,
            first.authorization.clone(),
            first.signing.clone(),
            Vec::new(),
        )?;
        assert_eq!(first.context.profile_id(), old_identity);
        assert_eq!(
            first
                .snapshot(4, Some(4))?
                .source
                .source_authority
                .source_ledger_digest,
            original_binding
        );
        first.close()?;
        second.close()
    }

    #[test]
    fn missing_source_body_is_unavailable_but_wrong_digest_is_refused() -> TestResult {
        let mut fixture = DomainTestProfile::new()?;
        let path = fixture.source(4, 1, "label")?;
        let snapshot = fixture.snapshot(4, None)?;
        let original = std::fs::read(&path)?;
        std::fs::remove_file(&path)?;
        let descriptor = snapshot
            .core
            .ledger()
            .artifact(id(3001)?)
            .ok_or("accepted artifact missing from replay")?;
        assert!(matches!(
            fixture.service.vault().verify_sealed_object(descriptor),
            Err(academic_vault::VaultError::IntegrityMismatch(ref canonical_path)) if canonical_path == &path
        ));
        assert!(matches!(std::fs::symlink_metadata(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound));
        let mut projector = Projector::new(
            &snapshot,
            VaultAccess::Plain(fixture.service.vault()),
            fixture.context.profile_id(),
        )
        .map_err(|r| format!("{r:?}"))?;
        let evidence = projector
            .evidence(id(5001)?)
            .map_err(|r| format!("{r:?}"))?;
        assert!(matches!(
            evidence.excerpt,
            dto::Field::Unavailable {
                reason: dto::MissingReason::SourceBodyUnavailable,
                ..
            }
        ));
        assert!(
            projector.provenance.values().all(|entry| !matches!(
                entry.origin,
                dto::ProvenanceOrigin::VerifiedArtifact { .. }
            ))
        );
        std::fs::write(&path, vec![b'!'; original.len()])?;
        assert!(matches!(
            fixture.service.vault().verify_sealed_object(descriptor),
            Err(academic_vault::VaultError::IntegrityMismatch(_))
        ));
        assert_eq!(
            projector.evidence(id(5001)?),
            Err(dto::ReadFailure::SourceVerificationFailed)
        );
        fixture.close()
    }

    #[test]
    fn source_history_needs_independent_authorization_and_exact_binding() -> TestResult {
        let mut fixture = DomainTestProfile::new()?;
        fixture.source(4, 1, "label")?;
        let request = DomainHistoryRequest {
            domain_id: id(3)?,
            scope_id: id(4)?,
            known_at_accept_seq: Some(4),
            valid_at: TimestampMillis::new(50),
        };
        let source = domain_history_snapshot(&mut fixture.profile.open_reader()?, &request)?;
        assert!(matches!(
            ReadSnapshot::authenticate(source, &[]),
            Err(dto::ReadFailure::SourceVerificationFailed)
        ));
        let mut source = domain_history_snapshot(&mut fixture.profile.open_reader()?, &request)?;
        source.source_authority.source_ledger_digest =
            ContentDigest::sha256(b"different coordinate");
        assert!(matches!(
            ReadSnapshot::authenticate(source, std::slice::from_ref(&fixture.authorization)),
            Err(dto::ReadFailure::SourceVerificationFailed)
        ));
        fixture.close()
    }

    #[test]
    fn an_interior_batch_selector_is_refused_without_rounding_or_current_fallback() -> TestResult {
        let mut fixture = DomainTestProfile::new()?;
        fixture.source(4, 1, "exact historical label")?;
        let mut request = fixture.request(4)?;
        let dto::DomainReadRequest::DetailsDomainReadV3 { selector, .. } = &mut request;
        selector.known_at_accept_seq = Some(2);
        assert_eq!(
            fixture.context.read_domain(
                &fixture.profile,
                &fixture.service,
                &request,
                TimestampMillis::new(50)
            ),
            dto::DomainReadReply::unavailable(dto::ReadFailure::SelectorUnavailable),
        );
        let dto::DomainReadRequest::DetailsDomainReadV3 { selector, .. } = &mut request;
        selector.known_at_accept_seq = Some(4);
        assert_eq!(
            fixture.context.read_domain(
                &fixture.profile,
                &fixture.service,
                &request,
                TimestampMillis::new(50)
            ),
            dto::DomainReadReply::unavailable(dto::ReadFailure::ProjectionUnavailable),
        );
        fixture.close()
    }
}
