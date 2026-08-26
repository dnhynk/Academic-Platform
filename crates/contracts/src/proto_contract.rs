//! Executable Protobuf round-trip for the actor and claim-relation contract.
//!
//! The wire tags here are checked against `schemas/proto/academic/v1/ledger.proto`
//! by `tools/verify-contracts.mjs`; the domain conversion revalidates all UUIDv7
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
    #[prost(oneof = "proto_origin_event::Payload", tags = "10, 11, 12, 13, 14, 15")]
    payload: Option<proto_origin_event::Payload>,
}

mod proto_origin_event {
    use super::{
        ProtoArtifactDescriptor, ProtoClaim, ProtoClaimRelation, ProtoEvidenceItem,
        ProtoScopeDescriptor, ProtoUserDecision,
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
