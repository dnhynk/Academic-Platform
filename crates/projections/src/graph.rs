//! Relational adjacency projection derived from entity-valued canonical claims.

use std::str::FromStr;

use academic_domain::{
    AuthorityClass, ClaimId, ClaimObject, ContentDigest, DomainError, DomainId, EntityId,
    EpistemicStatus, EvidenceId, ScopeId, TimestampMillis, ValidInterval,
};
use academic_store::queries::ProjectionResolvedClaim;
use rusqlite::{Connection, Transaction, params};

use crate::{
    checksum::append_field,
    generation::{GenerationId, ProjectionCoordinates, ResolutionProvenance},
    resolution::{
        AuthorityPolicy, authority_name, authority_policy_name, epistemic_name, parse_authority,
        parse_authority_policy, parse_epistemic,
    },
    runner::{ProjectionError, ProjectionResult},
};

/// One active relational edge with exact claim and evidence provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub source_entity_id: EntityId,
    pub predicate_id: String,
    pub target_entity_id: EntityId,
    pub claim_id: ClaimId,
    pub evidence_ids: Vec<EvidenceId>,
    pub scope_id: ScopeId,
    pub domain: DomainId,
    pub authority_class: AuthorityClass,
    pub epistemic_status: EpistemicStatus,
    pub valid_time: ValidInterval,
    pub resolution: ResolutionProvenance,
    pub source_record_accept_seq: u64,
    pub generation_id: GenerationId,
    pub stable_tiebreaker: ContentDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphSourceRecord {
    claim_id: [u8; 16],
    source_entity_id: [u8; 16],
    predicate_id: String,
    target_entity_id: [u8; 16],
    scope_id: [u8; 16],
    domain: [u8; 16],
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
    authority_policy: AuthorityPolicy,
    valid_from_unix_ms: i64,
    valid_to_unix_ms: Option<i64>,
    source_accept_seq: u64,
    evidence_ids: Vec<[u8; 16]>,
}

impl GraphSourceRecord {
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_field(&mut bytes, b"GRAPH_EDGE_V1");
        append_field(&mut bytes, &self.claim_id);
        append_field(&mut bytes, &self.source_entity_id);
        append_field(&mut bytes, self.predicate_id.as_bytes());
        append_field(&mut bytes, &self.target_entity_id);
        append_field(&mut bytes, &self.scope_id);
        append_field(&mut bytes, &self.domain);
        append_field(&mut bytes, authority_name(self.authority_class).as_bytes());
        append_field(&mut bytes, epistemic_name(self.epistemic_status).as_bytes());
        append_field(
            &mut bytes,
            authority_policy_name(self.authority_policy).as_bytes(),
        );
        append_field(&mut bytes, &self.valid_from_unix_ms.to_be_bytes());
        match self.valid_to_unix_ms {
            Some(valid_to) => {
                bytes.push(1);
                append_field(&mut bytes, &valid_to.to_be_bytes());
            }
            None => bytes.push(0),
        }
        append_field(&mut bytes, &self.source_accept_seq.to_be_bytes());
        for evidence_id in &self.evidence_ids {
            append_field(&mut bytes, evidence_id);
        }
        bytes
    }

    pub(crate) fn verification_bytes(&self) -> Vec<u8> {
        let mut bytes = self.canonical_bytes();
        let stable_tiebreaker = ContentDigest::sha256(&bytes);
        append_field(&mut bytes, stable_tiebreaker.as_bytes().as_slice());
        bytes
    }
}

pub(crate) fn load_graph_sources(
    resolved: &[ProjectionResolvedClaim],
    domain: DomainId,
) -> ProjectionResult<Vec<GraphSourceRecord>> {
    let mut records = resolved
        .iter()
        .filter_map(|record| {
            let ClaimObject::Entity(target_entity_id) = record.claim.object else {
                return None;
            };
            Some((record, target_entity_id))
        })
        .map(|(record, target_entity_id)| {
            Ok(GraphSourceRecord {
                claim_id: *record.claim.id.as_bytes(),
                source_entity_id: *record.claim.subject_entity_id.as_bytes(),
                predicate_id: record.claim.predicate_id.as_str().to_owned(),
                target_entity_id: *target_entity_id.as_bytes(),
                scope_id: *record.claim.scope_id.as_bytes(),
                domain: *domain.as_bytes(),
                authority_class: record.claim.authority_class,
                epistemic_status: record.claim.epistemic_status,
                authority_policy: record.applied_policy,
                valid_from_unix_ms: record.claim.valid_time.from().value(),
                valid_to_unix_ms: record.claim.valid_time.to().map(TimestampMillis::value),
                source_accept_seq: record.accept_seq,
                evidence_ids: record
                    .claim
                    .evidence_ids
                    .iter()
                    .map(|evidence_id| *evidence_id.as_bytes())
                    .collect(),
            })
        })
        .collect::<ProjectionResult<Vec<_>>>()?;
    records.sort_by_key(|record| record.claim_id);
    Ok(records)
}

pub(crate) fn write_graph_records<F>(
    transaction: &Transaction<'_>,
    generation_id: GenerationId,
    records: &[GraphSourceRecord],
    mut after_record: F,
) -> ProjectionResult<()>
where
    F: FnMut(usize, usize) -> ProjectionResult<()>,
{
    for (record_index, record) in records.iter().enumerate() {
        let stable_tiebreaker = ContentDigest::sha256(&record.canonical_bytes());
        transaction.execute(
            concat!(
                "INSERT INTO projection_graph_edge (generation_id, claim_id, source_entity_id, ",
                "predicate_id, target_entity_id, scope_id, security_domain, authority_class, ",
                "epistemic_status, authority_policy, valid_from_unix_ms, valid_to_unix_ms, ",
                "source_accept_seq, stable_tiebreaker) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                generation_id.as_bytes().as_slice(),
                record.claim_id.as_slice(),
                record.source_entity_id.as_slice(),
                record.predicate_id,
                record.target_entity_id.as_slice(),
                record.scope_id.as_slice(),
                record.domain.as_slice(),
                authority_name(record.authority_class),
                epistemic_name(record.epistemic_status),
                authority_policy_name(record.authority_policy),
                record.valid_from_unix_ms,
                record.valid_to_unix_ms,
                checked_i64(record.source_accept_seq)?,
                stable_tiebreaker.as_bytes().as_slice(),
            ],
        )?;
        for (ordinal, evidence_id) in record.evidence_ids.iter().enumerate() {
            transaction.execute(
                concat!(
                    "INSERT INTO projection_graph_edge_evidence ",
                    "(generation_id, claim_id, evidence_ordinal, evidence_id) ",
                    "VALUES (?1, ?2, ?3, ?4)"
                ),
                params![
                    generation_id.as_bytes().as_slice(),
                    record.claim_id.as_slice(),
                    checked_usize(ordinal)?,
                    evidence_id.as_slice(),
                ],
            )?;
        }
        after_record(record_index + 1, records.len())?;
    }
    Ok(())
}

pub(crate) fn persisted_graph_canonical_records(
    connection: &Connection,
    generation_id: GenerationId,
) -> ProjectionResult<Vec<Vec<u8>>> {
    let mut statement = connection.prepare(concat!(
        "SELECT claim_id, source_entity_id, predicate_id, target_entity_id, scope_id, ",
        "security_domain, authority_class, epistemic_status, authority_policy, ",
        "valid_from_unix_ms, valid_to_unix_ms, source_accept_seq, stable_tiebreaker ",
        "FROM projection_graph_edge WHERE generation_id = ?1 ORDER BY claim_id"
    ))?;
    let rows = statement.query_map([generation_id.as_bytes().as_slice()], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
            row.get::<_, Vec<u8>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, Option<i64>>(10)?,
            row.get::<_, i64>(11)?,
            row.get::<_, Vec<u8>>(12)?,
        ))
    })?;
    let mut canonical = Vec::new();
    for row in rows {
        let row = row?;
        let claim_id = fixed_bytes(row.0, "persisted graph claim identifier")?;
        let evidence_ids = read_evidence_bytes(connection, generation_id, claim_id)?;
        let record = GraphSourceRecord {
            claim_id,
            source_entity_id: fixed_bytes(row.1, "persisted graph source entity")?,
            predicate_id: row.2,
            target_entity_id: fixed_bytes(row.3, "persisted graph target entity")?,
            scope_id: fixed_bytes(row.4, "persisted graph scope")?,
            domain: fixed_bytes(row.5, "persisted graph domain")?,
            authority_class: parse_authority(&row.6)?,
            epistemic_status: parse_epistemic(&row.7)?,
            authority_policy: parse_authority_policy(&row.8)?,
            valid_from_unix_ms: row.9,
            valid_to_unix_ms: row.10,
            source_accept_seq: positive_u64(row.11, "persisted graph accept_seq")?,
            evidence_ids,
        };
        let stored_tiebreaker = ContentDigest::from_sha256_bytes(fixed_bytes(
            row.12,
            "persisted graph stable tiebreaker",
        )?);
        let expected_tiebreaker = ContentDigest::sha256(&record.canonical_bytes());
        if stored_tiebreaker != expected_tiebreaker {
            return Err(ProjectionError::Corrupt(
                "persisted graph stable tiebreaker does not match canonical record".to_owned(),
            ));
        }
        canonical.push(record.verification_bytes());
    }
    Ok(canonical)
}

pub(crate) fn read_graph_edges(
    connection: &Connection,
    generation_id: GenerationId,
    coordinates: ProjectionCoordinates,
    source_entity_id: EntityId,
) -> ProjectionResult<Vec<GraphEdge>> {
    let mut statement = connection.prepare(concat!(
        "SELECT claim_id, source_entity_id, predicate_id, target_entity_id, scope_id, ",
        "security_domain, authority_class, epistemic_status, authority_policy, ",
        "valid_from_unix_ms, valid_to_unix_ms, source_accept_seq, stable_tiebreaker ",
        "FROM projection_graph_edge WHERE generation_id = ?1 AND source_entity_id = ?2 ",
        "ORDER BY stable_tiebreaker, claim_id"
    ))?;
    let rows = statement.query_map(
        params![
            generation_id.as_bytes().as_slice(),
            source_entity_id.as_bytes().as_slice()
        ],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Vec<u8>>(12)?,
            ))
        },
    )?;
    let mut edges = Vec::new();
    for row in rows {
        let row = row?;
        let claim_bytes = fixed_bytes(row.0, "query graph claim identifier")?;
        edges.push(GraphEdge {
            source_entity_id: id_from_bytes(row.1, "query graph source entity")?,
            predicate_id: row.2,
            target_entity_id: id_from_bytes(row.3, "query graph target entity")?,
            claim_id: id_from_array(claim_bytes)?,
            evidence_ids: read_evidence_bytes(connection, generation_id, claim_bytes)?
                .into_iter()
                .map(id_from_array)
                .collect::<Result<Vec<_>, _>>()?,
            scope_id: id_from_bytes(row.4, "query graph scope")?,
            domain: id_from_bytes(row.5, "query graph domain")?,
            authority_class: parse_authority(&row.6)?,
            epistemic_status: parse_epistemic(&row.7)?,
            valid_time: ValidInterval::new(
                TimestampMillis::new(row.9),
                row.10.map(TimestampMillis::new),
            )
            .map_err(ProjectionError::Domain)?,
            resolution: ResolutionProvenance {
                authority_policy: parse_authority_policy(&row.8)?,
                coordinates,
            },
            source_record_accept_seq: positive_u64(row.11, "query graph accept_seq")?,
            generation_id,
            stable_tiebreaker: ContentDigest::from_sha256_bytes(fixed_bytes(
                row.12,
                "query graph stable tiebreaker",
            )?),
        });
    }
    Ok(edges)
}

fn read_evidence_bytes(
    connection: &Connection,
    generation_id: GenerationId,
    claim_id: [u8; 16],
) -> ProjectionResult<Vec<[u8; 16]>> {
    let mut statement = connection.prepare(concat!(
        "SELECT evidence_id FROM projection_graph_edge_evidence ",
        "WHERE generation_id = ?1 AND claim_id = ?2 ",
        "ORDER BY evidence_ordinal, evidence_id"
    ))?;
    let rows = statement.query_map(
        params![generation_id.as_bytes().as_slice(), claim_id.as_slice()],
        |row| row.get::<_, Vec<u8>>(0),
    )?;
    rows.map(|row| {
        fixed_bytes(
            row.map_err(ProjectionError::from)?,
            "graph edge evidence identifier",
        )
    })
    .collect()
}

fn fixed_bytes<const N: usize>(bytes: Vec<u8>, reason: &'static str) -> ProjectionResult<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| ProjectionError::Corrupt(reason.to_owned()))
}

fn id_from_bytes<T>(bytes: Vec<u8>, reason: &'static str) -> ProjectionResult<T>
where
    T: FromStr<Err = DomainError>,
{
    id_from_array(fixed_bytes(bytes, reason)?)
}

fn id_from_array<T>(bytes: [u8; 16]) -> ProjectionResult<T>
where
    T: FromStr<Err = DomainError>,
{
    uuid_text(bytes).parse().map_err(ProjectionError::Domain)
}

fn uuid_text(bytes: [u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn positive_u64(value: i64, reason: &'static str) -> ProjectionResult<u64> {
    let value = u64::try_from(value).map_err(|_| ProjectionError::Corrupt(reason.to_owned()))?;
    if value == 0 {
        Err(ProjectionError::Corrupt(reason.to_owned()))
    } else {
        Ok(value)
    }
}

fn checked_i64(value: u64) -> ProjectionResult<i64> {
    i64::try_from(value).map_err(|_| ProjectionError::IntegerOverflow(value))
}

fn checked_usize(value: usize) -> ProjectionResult<i64> {
    i64::try_from(value)
        .map_err(|_| ProjectionError::Corrupt("projection ordinal overflow".to_owned()))
}
