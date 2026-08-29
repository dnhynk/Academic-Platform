//! Executable Protobuf round-trip for the actor and claim-relation contract.
//!
//! The wire tags here are checked against both declared v1 and v2 schemas by
//! `tools/verify-contracts.mjs`; the domain conversion revalidates all UUIDv7
//! and event invariants after decoding.

use academic_domain::{
    Actor, ClaimId, ClaimRelation, ClaimRelationKind, DomainError, DomainId, EntityId, Event,
    EventId, EventPayload, ScopeId, TimestampMillis,
};
use prost::{Enumeration, Message};
use thiserror::Error;
use uuid::Uuid;

/// Lossless actor/relation Protobuf conversion failure.
#[derive(Debug, Error)]
pub enum ProtoContractError {
    /// A decoded domain value violated its constructor invariant.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// Protobuf bytes were malformed.
    #[error(transparent)]
    Decode(#[from] prost::DecodeError),
    /// A required semantic field was absent.
    #[error("missing required Protobuf field {0}")]
    Missing(&'static str),
    /// The event payload was not the relation profile handled by this converter.
    #[error("event payload is not ClaimRelated")]
    UnsupportedPayload,
    /// A relation enum discriminant was unknown.
    #[error("unknown ClaimRelationKind discriminant {0}")]
    InvalidRelationKind(i32),
}

#[derive(Clone, PartialEq, Message)]
struct ProtoUuidV7 {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoTimestampMillis {
    #[prost(sint64, tag = "1")]
    unix_epoch_millis: i64,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoUserActor {
    #[prost(message, optional, tag = "1")]
    user_id: Option<ProtoUuidV7>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoDeterministicEngineActor {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    version: String,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoModelRunActor {
    #[prost(message, optional, tag = "1")]
    run_id: Option<ProtoUuidV7>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoImporterActor {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(string, tag = "2")]
    version: String,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoActor {
    #[prost(oneof = "proto_actor::Kind", tags = "1, 2, 3, 4")]
    kind: Option<proto_actor::Kind>,
}

mod proto_actor {
    use super::{
        ProtoDeterministicEngineActor, ProtoImporterActor, ProtoModelRunActor, ProtoUserActor,
    };
    use prost::Oneof;

    #[derive(Clone, PartialEq, Oneof)]
    pub(super) enum Kind {
        #[prost(message, tag = "1")]
        User(ProtoUserActor),
        #[prost(message, tag = "2")]
        DeterministicEngine(ProtoDeterministicEngineActor),
        #[prost(message, tag = "3")]
        ModelRun(ProtoModelRunActor),
        #[prost(message, tag = "4")]
        Importer(ProtoImporterActor),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Enumeration)]
#[repr(i32)]
enum ProtoClaimRelationKind {
    Unspecified = 0,
    Supports = 1,
    Contradicts = 2,
    Supersedes = 3,
    Retracts = 4,
    Duplicates = 5,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoClaimRelation {
    #[prost(message, optional, tag = "1")]
    source_claim_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    target_claim_id: Option<ProtoUuidV7>,
    #[prost(enumeration = "ProtoClaimRelationKind", tag = "3")]
    kind: i32,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
}

// The relation decoder intentionally supports only ClaimRelated semantically,
// but it must still model every currently known oneof arm. Otherwise Prost
// treats tags 10-14 as unknown fields and can retain an earlier relation even
// though generated clients correctly apply protobuf's last-oneof-value rule.
#[derive(Clone, PartialEq, Message)]
struct ProtoArtifactDescriptor {}

#[derive(Clone, PartialEq, Message)]
struct ProtoEvidenceItem {}

#[derive(Clone, PartialEq, Message)]
struct ProtoClaim {}

#[derive(Clone, PartialEq, Message)]
struct ProtoUserDecision {}

#[derive(Clone, PartialEq, Message)]
struct ProtoScopeDescriptor {}

#[derive(Clone, PartialEq, Message)]
struct ProtoSha256Digest {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoValidInterval {
    #[prost(message, optional, tag = "1")]
    from: Option<ProtoTimestampMillis>,
    #[prost(message, optional, tag = "2")]
    to: Option<ProtoTimestampMillis>,
}

// Event schema v3 registration messages. Field numbering is uniform across all
// eighteen: 1 id, 2 parent where one exists, 3 domain_id, 4 scope_id,
// 5 source_digest, 6 valid_time. Tag 2 is simply absent where the aggregate has
// no parent, so no reader has to special-case one arm against another.
#[derive(Clone, PartialEq, Message)]
struct ProtoCurriculumVersionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoCourseRevisionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    curriculum_version_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoOfferingRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    course_revision_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoAttemptRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    offering_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoRequirementSetRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    curriculum_version_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoAuditRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    requirement_set_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoCapturePermissionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    offering_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoLectureSessionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    offering_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoTranscriptVersionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    lecture_session_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoLectureDocumentRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    lecture_session_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoSnapshotRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    repository_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoFindingRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    snapshot_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoModelRunRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoProposalDispositionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    model_run_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoEgressDecisionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoConsentRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoEntityIdentityChangeRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "2")]
    entity_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoRetentionActionRegistration {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "3")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "4")]
    scope_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    source_digest: Option<ProtoSha256Digest>,
    #[prost(message, optional, tag = "6")]
    valid_time: Option<ProtoValidInterval>,
}

#[derive(Clone, PartialEq, Message)]
struct ProtoOriginEvent {
    #[prost(message, optional, tag = "1")]
    id: Option<ProtoUuidV7>,
    #[prost(uint64, tag = "2")]
    origin_seq: u64,
    #[prost(message, optional, tag = "3")]
    origin_observed_at: Option<ProtoTimestampMillis>,
    #[prost(message, optional, tag = "4")]
    domain_id: Option<ProtoUuidV7>,
    #[prost(message, optional, tag = "5")]
    actor: Option<ProtoActor>,
    #[prost(
        oneof = "proto_origin_event::Payload",
        tags = "10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33"
    )]
    payload: Option<proto_origin_event::Payload>,
}

mod proto_origin_event {
    use super::{
        ProtoArtifactDescriptor, ProtoAttemptRegistration, ProtoAuditRegistration,
        ProtoCapturePermissionRegistration, ProtoClaim, ProtoClaimRelation,
        ProtoConsentRegistration, ProtoCourseRevisionRegistration,
        ProtoCurriculumVersionRegistration, ProtoEgressDecisionRegistration,
        ProtoEntityIdentityChangeRegistration, ProtoEvidenceItem, ProtoFindingRegistration,
        ProtoLectureDocumentRegistration, ProtoLectureSessionRegistration,
        ProtoModelRunRegistration, ProtoOfferingRegistration, ProtoProposalDispositionRegistration,
        ProtoRequirementSetRegistration, ProtoRetentionActionRegistration, ProtoScopeDescriptor,
        ProtoSnapshotRegistration, ProtoTranscriptVersionRegistration, ProtoUserDecision,
    };
    use prost::Oneof;

    #[derive(Clone, PartialEq, Oneof)]
    pub(super) enum Payload {
        #[prost(message, tag = "10")]
        ArtifactRegistered(ProtoArtifactDescriptor),
        #[prost(message, tag = "11")]
        EvidenceRegistered(ProtoEvidenceItem),
        #[prost(message, tag = "12")]
        ClaimAsserted(ProtoClaim),
        #[prost(message, tag = "13")]
        DecisionRecorded(ProtoUserDecision),
        #[prost(message, tag = "14")]
        ScopeRegistered(ProtoScopeDescriptor),
        #[prost(message, tag = "15")]
        ClaimRelated(ProtoClaimRelation),
        #[prost(message, tag = "16")]
        CurriculumVersionPublished(ProtoCurriculumVersionRegistration),
        #[prost(message, tag = "17")]
        CourseRevisionPublished(ProtoCourseRevisionRegistration),
        #[prost(message, tag = "18")]
        OfferingObserved(ProtoOfferingRegistration),
        #[prost(message, tag = "19")]
        AttemptRecorded(ProtoAttemptRegistration),
        #[prost(message, tag = "20")]
        RequirementSetPublished(ProtoRequirementSetRegistration),
        #[prost(message, tag = "21")]
        AuditComputed(ProtoAuditRegistration),
        #[prost(message, tag = "22")]
        CapturePermissionRecorded(ProtoCapturePermissionRegistration),
        #[prost(message, tag = "23")]
        LectureSessionRecorded(ProtoLectureSessionRegistration),
        #[prost(message, tag = "24")]
        TranscriptVersionAdded(ProtoTranscriptVersionRegistration),
        #[prost(message, tag = "25")]
        LectureDocumentPublished(ProtoLectureDocumentRegistration),
        #[prost(message, tag = "26")]
        SnapshotRegistered(ProtoSnapshotRegistration),
        #[prost(message, tag = "27")]
        FindingPublished(ProtoFindingRegistration),
        #[prost(message, tag = "28")]
        ModelRunRecorded(ProtoModelRunRegistration),
        #[prost(message, tag = "29")]
        ProposalDisposed(ProtoProposalDispositionRegistration),
        #[prost(message, tag = "30")]
        EgressDecided(ProtoEgressDecisionRegistration),
        #[prost(message, tag = "31")]
        ConsentRecorded(ProtoConsentRegistration),
        #[prost(message, tag = "32")]
        EntityIdentityChanged(ProtoEntityIdentityChangeRegistration),
        #[prost(message, tag = "33")]
        RetentionActionRecorded(ProtoRetentionActionRegistration),
    }
}

/// Encodes a relation event through the exact Protobuf tags declared in the schema.
pub fn encode_claim_relation_event_proto(event: &Event) -> Result<Vec<u8>, ProtoContractError> {
    event.validate()?;
    let EventPayload::ClaimRelated(relation) = &event.payload else {
        return Err(ProtoContractError::UnsupportedPayload);
    };
    let value = ProtoOriginEvent {
        id: Some(uuid(event.id.as_bytes())),
        origin_seq: event.origin_seq,
        origin_observed_at: Some(ProtoTimestampMillis {
            unix_epoch_millis: event.origin_observed_at.value(),
        }),
        domain_id: Some(uuid(event.domain_id.as_bytes())),
        actor: Some(encode_actor(&event.actor)),
        payload: Some(proto_origin_event::Payload::ClaimRelated(
            ProtoClaimRelation {
                source_claim_id: Some(uuid(relation.source_claim_id.as_bytes())),
                target_claim_id: Some(uuid(relation.target_claim_id.as_bytes())),
                kind: encode_relation_kind(relation.kind) as i32,
                scope_id: Some(uuid(relation.scope_id.as_bytes())),
            },
        )),
    };
    Ok(value.encode_to_vec())
}

/// Decodes Protobuf bytes and reconstructs the complete structured actor/relation event.
pub fn decode_claim_relation_event_proto(bytes: &[u8]) -> Result<Event, ProtoContractError> {
    let value = ProtoOriginEvent::decode(bytes)?;
    let Some(payload) = value.payload else {
        return Err(ProtoContractError::Missing("payload"));
    };
    let proto_origin_event::Payload::ClaimRelated(relation) = payload else {
        return Err(ProtoContractError::UnsupportedPayload);
    };
    let actor = decode_actor(value.actor.ok_or(ProtoContractError::Missing("actor"))?)?;
    let kind = ProtoClaimRelationKind::try_from(relation.kind)
        .map_err(|_| ProtoContractError::InvalidRelationKind(relation.kind))?;
    let event = Event {
        id: decode_id(value.id, "id", EventId::try_from_uuid)?,
        origin_seq: value.origin_seq,
        origin_observed_at: TimestampMillis::new(
            value
                .origin_observed_at
                .ok_or(ProtoContractError::Missing("origin_observed_at"))?
                .unix_epoch_millis,
        ),
        actor,
        domain_id: decode_id(value.domain_id, "domain_id", DomainId::try_from_uuid)?,
        payload: EventPayload::ClaimRelated(ClaimRelation {
            source_claim_id: decode_id(
                relation.source_claim_id,
                "source_claim_id",
                ClaimId::try_from_uuid,
            )?,
            target_claim_id: decode_id(
                relation.target_claim_id,
                "target_claim_id",
                ClaimId::try_from_uuid,
            )?,
            kind: decode_relation_kind(kind)
                .ok_or(ProtoContractError::InvalidRelationKind(relation.kind))?,
            scope_id: decode_id(relation.scope_id, "scope_id", ScopeId::try_from_uuid)?,
        }),
    };
    event.validate()?;
    Ok(event)
}

fn uuid(bytes: &[u8; 16]) -> ProtoUuidV7 {
    ProtoUuidV7 {
        value: bytes.to_vec(),
    }
}

fn decode_id<T>(
    value: Option<ProtoUuidV7>,
    name: &'static str,
    constructor: impl FnOnce(Uuid) -> Result<T, DomainError>,
) -> Result<T, ProtoContractError> {
    let bytes = value.ok_or(ProtoContractError::Missing(name))?.value;
    let uuid = Uuid::from_slice(&bytes).map_err(|_| DomainError::InvalidId {
        kind: name,
        value: hex::encode(bytes),
    })?;
    Ok(constructor(uuid)?)
}

fn encode_actor(actor: &Actor) -> ProtoActor {
    let kind = match actor {
        Actor::User { user_id } => proto_actor::Kind::User(ProtoUserActor {
            user_id: Some(uuid(user_id.as_bytes())),
        }),
        Actor::DeterministicEngine { name, version } => {
            proto_actor::Kind::DeterministicEngine(ProtoDeterministicEngineActor {
                name: name.clone(),
                version: version.clone(),
            })
        }
        Actor::ModelRun { run_id } => proto_actor::Kind::ModelRun(ProtoModelRunActor {
            run_id: Some(uuid(run_id.as_bytes())),
        }),
        Actor::Importer { name, version } => proto_actor::Kind::Importer(ProtoImporterActor {
            name: name.clone(),
            version: version.clone(),
        }),
    };
    ProtoActor { kind: Some(kind) }
}

fn decode_actor(value: ProtoActor) -> Result<Actor, ProtoContractError> {
    match value
        .kind
        .ok_or(ProtoContractError::Missing("actor.kind"))?
    {
        proto_actor::Kind::User(value) => Ok(Actor::User {
            user_id: decode_id(value.user_id, "user_id", EntityId::try_from_uuid)?,
        }),
        proto_actor::Kind::DeterministicEngine(value) => Ok(Actor::DeterministicEngine {
            name: value.name,
            version: value.version,
        }),
        proto_actor::Kind::ModelRun(value) => Ok(Actor::ModelRun {
            run_id: decode_id(value.run_id, "run_id", EntityId::try_from_uuid)?,
        }),
        proto_actor::Kind::Importer(value) => Ok(Actor::Importer {
            name: value.name,
            version: value.version,
        }),
    }
}

const fn encode_relation_kind(value: ClaimRelationKind) -> ProtoClaimRelationKind {
    match value {
        ClaimRelationKind::Supports => ProtoClaimRelationKind::Supports,
        ClaimRelationKind::Contradicts => ProtoClaimRelationKind::Contradicts,
        ClaimRelationKind::Supersedes => ProtoClaimRelationKind::Supersedes,
        ClaimRelationKind::Retracts => ProtoClaimRelationKind::Retracts,
        ClaimRelationKind::Duplicates => ProtoClaimRelationKind::Duplicates,
    }
}

const fn decode_relation_kind(value: ProtoClaimRelationKind) -> Option<ClaimRelationKind> {
    match value {
        ProtoClaimRelationKind::Supports => Some(ClaimRelationKind::Supports),
        ProtoClaimRelationKind::Contradicts => Some(ClaimRelationKind::Contradicts),
        ProtoClaimRelationKind::Supersedes => Some(ClaimRelationKind::Supersedes),
        ProtoClaimRelationKind::Retracts => Some(ClaimRelationKind::Retracts),
        ProtoClaimRelationKind::Duplicates => Some(ClaimRelationKind::Duplicates),
        ProtoClaimRelationKind::Unspecified => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protobuf_uuid_boundary_requires_rfc_variant_uuidv7() -> Result<(), Box<dyn std::error::Error>>
    {
        let encoded = hex::decode(
            "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e307a3e0a120a100190000000007000800000000000020612120a1001900000000070008000000000000205180322120a1001900000000070008000000000000007",
        )?;
        let valid = ProtoOriginEvent::decode(encoded.as_slice())?;
        for (octet_index, replacement) in [(6_usize, 0x40_u8), (8, 0x00), (8, 0xc0), (8, 0xe0)] {
            let mut mutated = valid.clone();
            *mutated
                .id
                .as_mut()
                .ok_or(ProtoContractError::Missing("id"))?
                .value
                .get_mut(octet_index)
                .ok_or(ProtoContractError::Missing("id.value invariant octet"))? = replacement;
            assert!(matches!(
                decode_claim_relation_event_proto(&mutated.encode_to_vec()),
                Err(ProtoContractError::Domain(DomainError::InvalidId { .. }))
            ));
        }
        Ok(())
    }

    type V3ArmGolden = (u32, &'static str, proto_origin_event::Payload, &'static str);

    fn v3_registration_event(
        payload: proto_origin_event::Payload,
    ) -> Result<ProtoOriginEvent, Box<dyn std::error::Error>> {
        Ok(ProtoOriginEvent {
            id: Some(uuid(
                "01900000-0000-7000-8000-00000000000c"
                    .parse::<EventId>()?
                    .as_bytes(),
            )),
            origin_seq: 12,
            origin_observed_at: Some(ProtoTimestampMillis {
                unix_epoch_millis: 112,
            }),
            domain_id: Some(uuid(
                "01900000-0000-7000-8000-000000000001"
                    .parse::<DomainId>()?
                    .as_bytes(),
            )),
            actor: Some(encode_actor(&Actor::Importer {
                name: "synthetic.official.fixture".to_owned(),
                version: "1.0.0".to_owned(),
            })),
            payload: Some(payload),
        })
    }

    // One explicit row per v3 arm. The hex strings were produced independently by
    // protobuf.js from `schemas/proto/academic/v3/ledger.proto`, and
    // `tools/verify-contracts.mjs` recomputes every one of them, so drift on
    // either side fails.
    fn v3_arm_goldens() -> Result<Vec<V3ArmGolden>, Box<dyn std::error::Error>> {
        let id = "01900000-0000-7000-8000-000000000410".parse::<EventId>()?;
        let parent = "01900000-0000-7000-8000-000000000411".parse::<EventId>()?;
        let domain = "01900000-0000-7000-8000-000000000001".parse::<DomainId>()?;
        let scope = "01900000-0000-7000-8000-000000000007".parse::<ScopeId>()?;
        let digest =
            hex::decode("0aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b8134")?;
        let valid_time = ProtoValidInterval {
            from: Some(ProtoTimestampMillis {
                unix_epoch_millis: 100,
            }),
            to: None,
        };
        Ok(vec![
            (
                16,
                "curriculum_version_published",
                proto_origin_event::Payload::CurriculumVersionPublished(
                    ProtoCurriculumVersionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e308201670a120a10019000000000700080000000000004101a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                17,
                "course_revision_published",
                proto_origin_event::Payload::CourseRevisionPublished(
                    ProtoCourseRevisionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        curriculum_version_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e308a017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                18,
                "offering_observed",
                proto_origin_event::Payload::OfferingObserved(ProtoOfferingRegistration {
                    id: Some(uuid(id.as_bytes())),
                    course_revision_id: Some(uuid(parent.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e3092017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                19,
                "attempt_recorded",
                proto_origin_event::Payload::AttemptRecorded(ProtoAttemptRegistration {
                    id: Some(uuid(id.as_bytes())),
                    offering_id: Some(uuid(parent.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e309a017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                20,
                "requirement_set_published",
                proto_origin_event::Payload::RequirementSetPublished(
                    ProtoRequirementSetRegistration {
                        id: Some(uuid(id.as_bytes())),
                        curriculum_version_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30a2017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                21,
                "audit_computed",
                proto_origin_event::Payload::AuditComputed(ProtoAuditRegistration {
                    id: Some(uuid(id.as_bytes())),
                    requirement_set_id: Some(uuid(parent.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30aa017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                22,
                "capture_permission_recorded",
                proto_origin_event::Payload::CapturePermissionRecorded(
                    ProtoCapturePermissionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        offering_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30b2017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                23,
                "lecture_session_recorded",
                proto_origin_event::Payload::LectureSessionRecorded(
                    ProtoLectureSessionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        offering_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30ba017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                24,
                "transcript_version_added",
                proto_origin_event::Payload::TranscriptVersionAdded(
                    ProtoTranscriptVersionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        lecture_session_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30c2017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                25,
                "lecture_document_published",
                proto_origin_event::Payload::LectureDocumentPublished(
                    ProtoLectureDocumentRegistration {
                        id: Some(uuid(id.as_bytes())),
                        lecture_session_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30ca017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                26,
                "snapshot_registered",
                proto_origin_event::Payload::SnapshotRegistered(ProtoSnapshotRegistration {
                    id: Some(uuid(id.as_bytes())),
                    repository_id: Some(uuid(parent.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30d2017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                27,
                "finding_published",
                proto_origin_event::Payload::FindingPublished(ProtoFindingRegistration {
                    id: Some(uuid(id.as_bytes())),
                    snapshot_id: Some(uuid(parent.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30da017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                28,
                "model_run_recorded",
                proto_origin_event::Payload::ModelRunRecorded(ProtoModelRunRegistration {
                    id: Some(uuid(id.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30e201670a120a10019000000000700080000000000004101a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                29,
                "proposal_disposed",
                proto_origin_event::Payload::ProposalDisposed(
                    ProtoProposalDispositionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        model_run_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30ea017b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                30,
                "egress_decided",
                proto_origin_event::Payload::EgressDecided(ProtoEgressDecisionRegistration {
                    id: Some(uuid(id.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30f201670a120a10019000000000700080000000000004101a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                31,
                "consent_recorded",
                proto_origin_event::Payload::ConsentRecorded(ProtoConsentRegistration {
                    id: Some(uuid(id.as_bytes())),
                    domain_id: Some(uuid(domain.as_bytes())),
                    scope_id: Some(uuid(scope.as_bytes())),
                    source_digest: Some(ProtoSha256Digest {
                        value: digest.clone(),
                    }),
                    valid_time: Some(valid_time.clone()),
                }),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e30fa01670a120a10019000000000700080000000000004101a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                32,
                "entity_identity_changed",
                proto_origin_event::Payload::EntityIdentityChanged(
                    ProtoEntityIdentityChangeRegistration {
                        id: Some(uuid(id.as_bytes())),
                        entity_id: Some(uuid(parent.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e3082027b0a120a100190000000007000800000000000041012120a10019000000000700080000000000004111a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
            (
                33,
                "retention_action_recorded",
                proto_origin_event::Payload::RetentionActionRecorded(
                    ProtoRetentionActionRegistration {
                        id: Some(uuid(id.as_bytes())),
                        domain_id: Some(uuid(domain.as_bytes())),
                        scope_id: Some(uuid(scope.as_bytes())),
                        source_digest: Some(ProtoSha256Digest {
                            value: digest.clone(),
                        }),
                        valid_time: Some(valid_time.clone()),
                    },
                ),
                "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e308a02670a120a10019000000000700080000000000004101a120a100190000000007000800000000000000122120a10019000000000700080000000000000072a220a200aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b813432050a0308c801",
            ),
        ])
    }

    /// Every event schema v3 arm occupies its own previously unused Proto tag.
    ///
    /// Tags 10..=15 stay bound to the v1/v2 arms and 6..=9 stay reserved, so the
    /// eighteen v3 arms occupy 16..=33 and no emitted tag is ever reused.
    #[test]
    fn v3_arms_use_unreused_tags() -> Result<(), Box<dyn std::error::Error>> {
        let mut emitted = std::collections::BTreeSet::new();
        for (tag, name, payload, _) in v3_arm_goldens()? {
            assert!((16..=33).contains(&tag), "{name} must occupy a v3 tag");
            assert!(emitted.insert(tag), "{name} reuses Proto tag {tag}");
            let encoded = v3_registration_event(payload)?.encode_to_vec();
            let decoded = ProtoOriginEvent::decode(encoded.as_slice())?;
            assert!(
                decoded.payload.is_some(),
                "{name} must decode back to its own arm"
            );
        }
        assert_eq!(emitted.len(), 18, "v3 declares exactly eighteen arms");
        assert_eq!(emitted.first().copied(), Some(16));
        assert_eq!(emitted.last().copied(), Some(33));
        for legacy in [6_u32, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
            assert!(!emitted.contains(&legacy), "v3 must not reuse tag {legacy}");
        }
        Ok(())
    }

    /// Rust encodes every v3 arm to the exact bytes protobuf.js independently produces.
    #[test]
    fn t093_every_v3_arm_matches_the_independent_protobufjs_golden()
    -> Result<(), Box<dyn std::error::Error>> {
        for (tag, name, payload, expected) in v3_arm_goldens()? {
            let encoded = v3_registration_event(payload)?.encode_to_vec();
            assert_eq!(hex::encode(&encoded), expected, "{name} (tag {tag}) bytes");
        }
        Ok(())
    }

    /// An absent `source_digest` round-trips as absence, not as an empty digest.
    #[test]
    fn t093_v3_source_digest_round_trips_present_and_absent()
    -> Result<(), Box<dyn std::error::Error>> {
        let id = "01900000-0000-7000-8000-000000000410".parse::<EventId>()?;
        let domain = "01900000-0000-7000-8000-000000000001".parse::<DomainId>()?;
        let scope = "01900000-0000-7000-8000-000000000007".parse::<ScopeId>()?;
        let bare = ProtoCurriculumVersionRegistration {
            id: Some(uuid(id.as_bytes())),
            domain_id: Some(uuid(domain.as_bytes())),
            scope_id: Some(uuid(scope.as_bytes())),
            source_digest: None,
            valid_time: Some(ProtoValidInterval {
                from: Some(ProtoTimestampMillis {
                    unix_epoch_millis: 100,
                }),
                to: None,
            }),
        };
        let encoded = v3_registration_event(
            proto_origin_event::Payload::CurriculumVersionPublished(bare.clone()),
        )?
        .encode_to_vec();
        assert_eq!(
            hex::encode(&encoded),
            "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e308201430a120a10019000000000700080000000000004101a120a100190000000007000800000000000000122120a100190000000007000800000000000000732050a0308c801"
        );
        let decoded = ProtoOriginEvent::decode(encoded.as_slice())?;
        let Some(proto_origin_event::Payload::CurriculumVersionPublished(record)) = decoded.payload
        else {
            return Err("decoded arm must stay CurriculumVersionPublished".into());
        };
        assert_eq!(record.source_digest, None);
        assert_eq!(record, bare);
        Ok(())
    }

    #[test]
    fn t017_every_actor_variant_matches_independent_v1_v2_protobufjs_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let actors = [
            (
                Actor::User {
                    user_id: "01900000-0000-7000-8000-000000000020".parse()?,
                },
                "User",
                "0a140a120a1001900000000070008000000000000020",
            ),
            (
                Actor::DeterministicEngine {
                    name: "resolver".to_owned(),
                    version: "1.2.3".to_owned(),
                },
                "DeterministicEngine",
                "12110a087265736f6c7665721205312e322e33",
            ),
            (
                Actor::ModelRun {
                    run_id: "01900000-0000-7000-8000-000000000021".parse()?,
                },
                "ModelRun",
                "1a140a120a1001900000000070008000000000000021",
            ),
            (
                Actor::Importer {
                    name: "registrar".to_owned(),
                    version: "2026.08".to_owned(),
                },
                "Importer",
                "22140a097265676973747261721207323032362e3038",
            ),
        ];
        for (actor, expected_arm, expected_hex) in actors {
            let wire = encode_actor(&actor);
            let selected_arm = match wire.kind.as_ref() {
                Some(proto_actor::Kind::User(_)) => "User",
                Some(proto_actor::Kind::DeterministicEngine(_)) => "DeterministicEngine",
                Some(proto_actor::Kind::ModelRun(_)) => "ModelRun",
                Some(proto_actor::Kind::Importer(_)) => "Importer",
                None => "Missing",
            };
            assert_eq!(selected_arm, expected_arm);
            let encoded = wire.encode_to_vec();
            assert_eq!(hex::encode(&encoded), expected_hex);
            assert_eq!(
                decode_actor(ProtoActor::decode(encoded.as_slice())?)?,
                actor
            );
        }
        Ok(())
    }
}
