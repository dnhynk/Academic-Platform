//! Read-only canonical snapshots and SQL-backed bitemporal resolution.

use std::{collections::BTreeMap, error::Error, fmt};

use academic_contracts::{
    ContractError, decode_canonical_claim_object, decode_canonical_evidence_ids,
};
use academic_domain::{
    Claim, ClaimId, ClaimRelation, ClaimRelationKind, ConfidencePermille, ContentDigest,
    DecisionAction, DecisionId, DomainError, EntityId, EvidenceId, PredicateId, ResolutionSlot,
    ScopeId, TimestampMillis, UserDecision, ValidInterval,
};
use academic_ledger::{
    ResolutionClaim, ResolutionDecision, ResolutionQuery, ResolutionRelation, ResolutionResult,
    ResolverActorKind, resolve_snapshot,
};

use crate::{
    connection::ReaderConnection,
    error::StoreError,
    repository::{
        RepositoryError, StoredClaimObject, decode_claim_object, decode_prediction_metadata,
        id_from_blob, parse_authority, parse_epistemic,
    },
};

/// One-statement view used by restart and concurrent-reader assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalSnapshot {
    pub next_accept_seq: u64,
    pub profile_revision: u64,
    pub batch_count: u64,
    pub event_count: u64,
    pub scope_count: u64,
    pub artifact_count: u64,
    pub evidence_count: u64,
    pub claim_count: u64,
    pub relation_count: u64,
    pub decision_count: u64,
    pub outbox_count: u64,
    pub receipt_count: u64,
    pub device_count: u64,
    pub accept_seq_head: u64,
    pub outbox_head: u64,
}

/// Exact authenticated batch material retained by the evidence vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBatchMaterial {
    pub signed_envelope: Vec<u8>,
    pub deterministic_payload: Vec<u8>,
    pub signing_public_key: [u8; 32],
    pub signature: [u8; 64],
    pub envelope_hash: ContentDigest,
    pub payload_hash: ContentDigest,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
}

/// Query or normalized-row integrity failure.
#[derive(Debug)]
pub enum QueryError {
    Store(StoreError),
    Repository(RepositoryError),
    Contract(ContractError),
    Domain(DomainError),
    Corrupt(&'static str),
    IntegerOverflow(u64),
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "store query error: {error}"),
            Self::Repository(error) => write!(formatter, "normalized query error: {error}"),
            Self::Contract(error) => write!(formatter, "canonical query error: {error}"),
            Self::Domain(error) => write!(formatter, "invalid normalized domain row: {error}"),
            Self::Corrupt(reason) => write!(formatter, "canonical snapshot is corrupt: {reason}"),
            Self::IntegerOverflow(value) => {
                write!(
                    formatter,
                    "query value {value} exceeds signed 64-bit storage"
                )
            }
        }
    }
}

impl Error for QueryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Repository(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Corrupt(_) | Self::IntegerOverflow(_) => None,
        }
    }
}

impl From<StoreError> for QueryError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<RepositoryError> for QueryError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<ContractError> for QueryError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<DomainError> for QueryError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

/// Reads all acceptance-relevant counts and heads through one SQLite statement.
pub fn canonical_snapshot(reader: &ReaderConnection) -> Result<CanonicalSnapshot, QueryError> {
    type Raw = (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    );
    let row: Raw = reader.query_row(
        concat!(
            "SELECT r.next_accept_seq, r.profile_revision, ",
            "(SELECT count(*) FROM ledger_batch), (SELECT count(*) FROM ledger_event), ",
            "(SELECT count(*) FROM scope), (SELECT count(*) FROM artifact_descriptor), ",
            "(SELECT count(*) FROM evidence_item), (SELECT count(*) FROM claim), ",
            "(SELECT count(*) FROM claim_relation), (SELECT count(*) FROM user_decision), ",
            "(SELECT count(*) FROM projection_outbox), (SELECT count(*) FROM command_receipt), ",
            "(SELECT count(*) FROM device_head), ",
            "coalesce((SELECT max(accept_seq) FROM ledger_event), 0), ",
            "coalesce((SELECT max(outbox_seq) FROM projection_outbox), 0) ",
            "FROM replica_state r WHERE r.singleton = 1"
        ),
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
            ))
        },
    )?;
    Ok(CanonicalSnapshot {
        next_accept_seq: positive_u64(row.0, "next acceptance sequence")?,
        profile_revision: nonnegative_u64(row.1, "profile revision")?,
        batch_count: nonnegative_u64(row.2, "batch count")?,
        event_count: nonnegative_u64(row.3, "event count")?,
        scope_count: nonnegative_u64(row.4, "scope count")?,
        artifact_count: nonnegative_u64(row.5, "artifact count")?,
        evidence_count: nonnegative_u64(row.6, "evidence count")?,
        claim_count: nonnegative_u64(row.7, "claim count")?,
        relation_count: nonnegative_u64(row.8, "relation count")?,
        decision_count: nonnegative_u64(row.9, "decision count")?,
        outbox_count: nonnegative_u64(row.10, "outbox count")?,
        receipt_count: nonnegative_u64(row.11, "receipt count")?,
        device_count: nonnegative_u64(row.12, "device count")?,
        accept_seq_head: nonnegative_u64(row.13, "acceptance head")?,
        outbox_head: nonnegative_u64(row.14, "outbox head")?,
    })
}

/// Reads the exact original authenticated bytes for one accepted batch.
pub fn batch_material(
    reader: &ReaderConnection,
    batch_id: academic_domain::BatchId,
) -> Result<StoredBatchMaterial, QueryError> {
    type Raw = (
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        i64,
        i64,
    );
    let row: Raw = reader.query_row(
        concat!(
            "SELECT signed_envelope, deterministic_payload, signing_public_key, signature, ",
            "envelope_hash, deterministic_payload_hash, accept_seq_start, accept_seq_end ",
            "FROM ledger_batch WHERE batch_id = ?1"
        ),
        [batch_id.as_bytes().as_slice()],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        },
    )?;
    Ok(StoredBatchMaterial {
        signed_envelope: row.0,
        deterministic_payload: row.1,
        signing_public_key: fixed_bytes(row.2, "signing public key")?,
        signature: fixed_bytes(row.3, "signature")?,
        envelope_hash: ContentDigest::from_sha256_bytes(fixed_bytes(row.4, "envelope hash")?),
        payload_hash: ContentDigest::from_sha256_bytes(fixed_bytes(row.5, "payload hash")?),
        accept_seq_start: positive_u64(row.6, "batch acceptance start")?,
        accept_seq_end: positive_u64(row.7, "batch acceptance end")?,
    })
}

/// Resolves bitemporal state from normalized SQL rows through the same pure
/// resolver used by the in-memory ledger.
pub fn resolve(
    reader: &ReaderConnection,
    query: &ResolutionQuery,
) -> Result<ResolutionResult, QueryError> {
    let claims = read_claims(reader, query)?;
    let relations = read_relations(reader, query)?;
    let decisions = read_decisions(reader, query)?;
    Ok(resolve_snapshot(query, &claims, &relations, &decisions))
}

type RawClaim = (
    Vec<u8>,
    Vec<u8>,
    String,
    Vec<u8>,
    String,
    Option<Vec<u8>>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    i64,
    Option<i64>,
    i64,
);

fn read_claims(
    reader: &ReaderConnection,
    query: &ResolutionQuery,
) -> Result<Vec<ResolutionClaim>, QueryError> {
    let known_at = checked_i64(query.known_at_accept_seq)?;
    let parameters = rusqlite::params![
        query.subject_entity_id.as_bytes().as_slice(),
        query.predicate_id.as_str(),
        query.scope_id.as_bytes().as_slice(),
        known_at,
    ];
    let raw: Vec<RawClaim> = reader.query_collect(
        concat!(
            "SELECT c.claim_id, c.subject_entity_id, c.predicate_id, c.scope_id, c.object_kind, ",
            "c.object_entity_id, c.object_text, c.object_integer, c.object_decimal_coefficient, ",
            "c.object_decimal_scale, c.object_interval_from, c.object_interval_to, ",
            "c.authority_class, c.epistemic_status, c.confidence_permille, ",
            "c.prediction_metadata_version, c.prediction_observation_from, ",
            "c.prediction_observation_to, c.prediction_sample_count, c.valid_from, c.valid_to, ",
            "e.accept_seq FROM claim c JOIN ledger_event e ON e.event_id = c.assertion_event_id ",
            "WHERE c.subject_entity_id = ?1 AND c.predicate_id = ?2 AND c.scope_id = ?3 ",
            "AND e.accept_seq <= ?4 ORDER BY e.accept_seq, c.claim_id"
        ),
        parameters,
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
                row.get(16)?,
                row.get(17)?,
                row.get(18)?,
                row.get(19)?,
                row.get(20)?,
                row.get(21)?,
            ))
        },
    )?;
    let evidence_rows: Vec<(Vec<u8>, Vec<u8>)> = reader.query_collect(
        concat!(
            "SELECT ce.claim_id, ce.evidence_id FROM claim_evidence ce ",
            "JOIN claim c ON c.claim_id = ce.claim_id ",
            "JOIN ledger_event e ON e.event_id = c.assertion_event_id ",
            "WHERE c.subject_entity_id = ?1 AND c.predicate_id = ?2 AND c.scope_id = ?3 ",
            "AND e.accept_seq <= ?4 ORDER BY ce.claim_id, ce.evidence_ordinal"
        ),
        rusqlite::params![
            query.subject_entity_id.as_bytes().as_slice(),
            query.predicate_id.as_str(),
            query.scope_id.as_bytes().as_slice(),
            known_at,
        ],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut evidence = BTreeMap::<ClaimId, Vec<EvidenceId>>::new();
    for (claim_id, evidence_id) in evidence_rows {
        evidence
            .entry(id_from_blob(claim_id)?)
            .or_default()
            .push(id_from_blob(evidence_id)?);
    }

    raw.into_iter()
        .map(|row| {
            let claim_id: ClaimId = id_from_blob(row.0)?;
            let confidence = row
                .14
                .map(|value| {
                    u16::try_from(value)
                        .map_err(|_| QueryError::Corrupt("claim confidence is invalid"))
                        .and_then(|value| ConfidencePermille::new(value).map_err(Into::into))
                })
                .transpose()?;
            let claim = Claim {
                id: claim_id,
                subject_entity_id: id_from_blob(row.1)?,
                predicate_id: PredicateId::parse(row.2)?,
                scope_id: id_from_blob(row.3)?,
                object: decode_claim_object(StoredClaimObject {
                    kind: row.4,
                    entity: row.5,
                    text: row.6,
                    integer: row.7,
                    coefficient: row.8,
                    scale: row.9,
                    interval_from: row.10,
                    interval_to: row.11,
                })?,
                authority_class: parse_authority(&row.12)?,
                epistemic_status: parse_epistemic(&row.13)?,
                confidence,
                prediction_metadata: decode_prediction_metadata(row.15, row.16, row.17, row.18)?,
                valid_time: ValidInterval::new(
                    TimestampMillis::new(row.19),
                    row.20.map(TimestampMillis::new),
                )?,
                evidence_ids: evidence.remove(&claim_id).unwrap_or_default(),
            };
            claim.validate()?;
            Ok(ResolutionClaim {
                claim,
                accept_seq: positive_u64(row.21, "claim acceptance sequence")?,
            })
        })
        .collect()
}

fn read_relations(
    reader: &ReaderConnection,
    query: &ResolutionQuery,
) -> Result<Vec<ResolutionRelation>, QueryError> {
    type Raw = (Vec<u8>, Vec<u8>, String, Vec<u8>, String, i64);
    let rows: Vec<Raw> = reader.query_collect(
        concat!(
            "SELECT r.source_claim_id, r.target_claim_id, r.relation_kind, r.scope_id, ",
            "r.actor_kind, e.accept_seq FROM claim_relation r ",
            "JOIN ledger_event e ON e.event_id = r.relation_event_id ",
            "WHERE r.scope_id = ?1 AND e.accept_seq <= ?2 ",
            "ORDER BY e.accept_seq, r.relation_event_id"
        ),
        rusqlite::params![
            query.scope_id.as_bytes().as_slice(),
            checked_i64(query.known_at_accept_seq)?,
        ],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    )?;
    rows.into_iter()
        .map(|row| {
            Ok(ResolutionRelation {
                relation: ClaimRelation {
                    source_claim_id: id_from_blob(row.0)?,
                    target_claim_id: id_from_blob(row.1)?,
                    kind: parse_relation_kind(&row.2)?,
                    scope_id: id_from_blob(row.3)?,
                },
                actor_kind: parse_actor_kind(&row.4)?,
                accept_seq: positive_u64(row.5, "relation acceptance sequence")?,
            })
        })
        .collect()
}

fn read_decisions(
    reader: &ReaderConnection,
    query: &ResolutionQuery,
) -> Result<Vec<ResolutionDecision>, QueryError> {
    type Raw = (
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        String,
        Vec<u8>,
        String,
        Option<Vec<u8>>,
        i64,
        Option<i64>,
        Vec<u8>,
        i64,
        Option<i64>,
        i64,
    );
    let rows: Vec<Raw> = reader.query_collect(
        concat!(
            "SELECT d.decision_id, d.target_claim_id, d.target_object_canonical, ",
            "d.resolution_subject_entity_id, d.resolution_predicate_id, d.resolution_scope_id, ",
            "d.action, d.replacement_claim_id, d.valid_from, d.valid_to, ",
            "d.rationale_evidence_ids_canonical, d.decided_at, d.reversible_until, e.accept_seq ",
            "FROM user_decision d JOIN ledger_event e ON e.event_id = d.decision_event_id ",
            "WHERE d.resolution_subject_entity_id = ?1 AND d.resolution_predicate_id = ?2 ",
            "AND d.resolution_scope_id = ?3 AND e.accept_seq <= ?4 ",
            "ORDER BY e.accept_seq, d.decision_id"
        ),
        rusqlite::params![
            query.subject_entity_id.as_bytes().as_slice(),
            query.predicate_id.as_str(),
            query.scope_id.as_bytes().as_slice(),
            checked_i64(query.known_at_accept_seq)?,
        ],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
                row.get(13)?,
            ))
        },
    )?;
    rows.into_iter()
        .map(|row| {
            let replacement = row.7.map(id_from_blob).transpose()?;
            let action = match row.6.as_str() {
                "CONFIRM" => DecisionAction::Confirm,
                "REJECT" => DecisionAction::Reject,
                "REPLACE" => DecisionAction::Replace {
                    replacement_claim_id: replacement.ok_or(QueryError::Corrupt(
                        "replace decision has no replacement claim",
                    ))?,
                },
                _ => return Err(QueryError::Corrupt("decision action is invalid")),
            };
            let decision = UserDecision {
                id: id_from_blob::<DecisionId>(row.0)?,
                target_claim_id: id_from_blob(row.1)?,
                target_object: decode_canonical_claim_object(&row.2)?,
                resolution_slot: ResolutionSlot {
                    subject_entity_id: id_from_blob::<EntityId>(row.3)?,
                    predicate_id: PredicateId::parse(row.4)?,
                    scope_id: id_from_blob::<ScopeId>(row.5)?,
                },
                action,
                valid_time: ValidInterval::new(
                    TimestampMillis::new(row.8),
                    row.9.map(TimestampMillis::new),
                )?,
                rationale_evidence_ids: decode_canonical_evidence_ids(&row.10)?,
                decided_at: TimestampMillis::new(row.11),
                reversible_until: row.12.map(TimestampMillis::new),
            };
            decision.validate()?;
            Ok(ResolutionDecision {
                decision,
                accept_seq: positive_u64(row.13, "decision acceptance sequence")?,
            })
        })
        .collect()
}

fn parse_relation_kind(value: &str) -> Result<ClaimRelationKind, QueryError> {
    match value {
        "SUPPORTS" => Ok(ClaimRelationKind::Supports),
        "CONTRADICTS" => Ok(ClaimRelationKind::Contradicts),
        "SUPERSEDES" => Ok(ClaimRelationKind::Supersedes),
        "RETRACTS" => Ok(ClaimRelationKind::Retracts),
        "DUPLICATES" => Ok(ClaimRelationKind::Duplicates),
        _ => Err(QueryError::Corrupt("relation kind is invalid")),
    }
}

fn parse_actor_kind(value: &str) -> Result<ResolverActorKind, QueryError> {
    match value {
        "USER" => Ok(ResolverActorKind::User),
        "DETERMINISTIC_ENGINE" => Ok(ResolverActorKind::DeterministicEngine),
        "MODEL_RUN" => Ok(ResolverActorKind::ModelRun),
        "IMPORTER" => Ok(ResolverActorKind::Importer),
        _ => Err(QueryError::Corrupt("relation actor kind is invalid")),
    }
}

fn checked_i64(value: u64) -> Result<i64, QueryError> {
    i64::try_from(value).map_err(|_| QueryError::IntegerOverflow(value))
}

fn nonnegative_u64(value: i64, reason: &'static str) -> Result<u64, QueryError> {
    u64::try_from(value).map_err(|_| QueryError::Corrupt(reason))
}

fn positive_u64(value: i64, reason: &'static str) -> Result<u64, QueryError> {
    let value = nonnegative_u64(value, reason)?;
    if value == 0 {
        return Err(QueryError::Corrupt(reason));
    }
    Ok(value)
}

fn fixed_bytes<const LENGTH: usize>(
    bytes: Vec<u8>,
    reason: &'static str,
) -> Result<[u8; LENGTH], QueryError> {
    bytes.try_into().map_err(|_| QueryError::Corrupt(reason))
}
