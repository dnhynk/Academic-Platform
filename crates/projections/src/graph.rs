//! Relational adjacency projection derived from entity-valued canonical claims.

use std::{collections::BTreeMap, str::FromStr};

use academic_domain::{
    ClaimId, ContentDigest, DomainError, DomainId, EntityId, EvidenceId, ScopeId,
};
use rusqlite::{Connection, Transaction, params};

use crate::{
    checksum::append_field,
    generation::GenerationId,
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
    pub source_record_accept_seq: u64,
    pub generation_id: GenerationId,
    pub source_watermark: u64,
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
    authority_class: String,
    epistemic_status: String,
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
        append_field(&mut bytes, self.authority_class.as_bytes());
        append_field(&mut bytes, self.epistemic_status.as_bytes());
        append_field(&mut bytes, &self.source_accept_seq.to_be_bytes());
        for evidence_id in &self.evidence_ids {
            append_field(&mut bytes, evidence_id);
        }
        bytes
    }
}

#[derive(Debug)]
struct RawGraphRow {
    claim_id: Vec<u8>,
    source_entity_id: Vec<u8>,
    predicate_id: String,
    target_entity_id: Vec<u8>,
    scope_id: Vec<u8>,
    domain: Vec<u8>,
    authority_class: String,
    epistemic_status: String,
    source_accept_seq: i64,
    evidence_id: Option<Vec<u8>>,
}

pub(crate) fn load_graph_sources(
    canonical: &Connection,
    domain: DomainId,
    watermark: u64,
) -> ProjectionResult<Vec<GraphSourceRecord>> {
    let watermark = checked_i64(watermark)?;
    let mut statement = canonical.prepare(concat!(
        "SELECT c.claim_id, c.subject_entity_id, c.predicate_id, c.object_entity_id, ",
        "c.scope_id, e.domain_id, c.authority_class, c.epistemic_status, e.accept_seq, ",
        "ce.evidence_id FROM claim c ",
        "JOIN ledger_event e ON e.event_id = c.assertion_event_id ",
        "LEFT JOIN claim_evidence ce ON ce.claim_id = c.claim_id ",
        "WHERE c.object_kind = 'ENTITY' AND e.domain_id = ?1 AND e.accept_seq <= ?2 ",
        "ORDER BY c.claim_id, ce.evidence_ordinal, ce.evidence_id"
    ))?;
    let rows = statement.query_map(params![domain.as_bytes().as_slice(), watermark], |row| {
        Ok(RawGraphRow {
            claim_id: row.get(0)?,
            source_entity_id: row.get(1)?,
            predicate_id: row.get(2)?,
            target_entity_id: row.get(3)?,
            scope_id: row.get(4)?,
            domain: row.get(5)?,
            authority_class: row.get(6)?,
            epistemic_status: row.get(7)?,
            source_accept_seq: row.get(8)?,
            evidence_id: row.get(9)?,
        })
    })?;

    let mut grouped = BTreeMap::<[u8; 16], GraphSourceRecord>::new();
    for row in rows {
        let row = row?;
        let claim_id = fixed_bytes(row.claim_id, "graph claim identifier")?;
        let evidence_id = row
            .evidence_id
            .map(|value| fixed_bytes(value, "graph evidence identifier"))
            .transpose()?;
        let record = grouped.entry(claim_id).or_insert(GraphSourceRecord {
            claim_id,
            source_entity_id: fixed_bytes(row.source_entity_id, "graph source entity")?,
            predicate_id: row.predicate_id,
            target_entity_id: fixed_bytes(row.target_entity_id, "graph target entity")?,
            scope_id: fixed_bytes(row.scope_id, "graph scope identifier")?,
            domain: fixed_bytes(row.domain, "graph domain identifier")?,
            authority_class: row.authority_class,
            epistemic_status: row.epistemic_status,
            source_accept_seq: positive_u64(row.source_accept_seq, "graph source accept_seq")?,
            evidence_ids: Vec::new(),
        });
        if let Some(evidence_id) = evidence_id {
            record.evidence_ids.push(evidence_id);
        }
    }
    Ok(grouped.into_values().collect())
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
                "epistemic_status, source_accept_seq, stable_tiebreaker) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            ),
            params![
                generation_id.as_bytes().as_slice(),
                record.claim_id.as_slice(),
                record.source_entity_id.as_slice(),
                record.predicate_id,
                record.target_entity_id.as_slice(),
                record.scope_id.as_slice(),
                record.domain.as_slice(),
                record.authority_class,
                record.epistemic_status,
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
        "security_domain, authority_class, epistemic_status, source_accept_seq ",
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
            row.get::<_, i64>(8)?,
        ))
    })?;
    let mut canonical = Vec::new();
    for row in rows {
        let row = row?;
        let claim_id = fixed_bytes(row.0, "persisted graph claim identifier")?;
        let evidence_ids = read_evidence_bytes(connection, generation_id, claim_id)?;
        canonical.push(
            GraphSourceRecord {
                claim_id,
                source_entity_id: fixed_bytes(row.1, "persisted graph source entity")?,
                predicate_id: row.2,
                target_entity_id: fixed_bytes(row.3, "persisted graph target entity")?,
                scope_id: fixed_bytes(row.4, "persisted graph scope")?,
                domain: fixed_bytes(row.5, "persisted graph domain")?,
                authority_class: row.6,
                epistemic_status: row.7,
                source_accept_seq: positive_u64(row.8, "persisted graph accept_seq")?,
                evidence_ids,
            }
            .canonical_bytes(),
        );
    }
    Ok(canonical)
}

pub(crate) fn read_graph_edges(
    connection: &Connection,
    generation_id: GenerationId,
    source_watermark: u64,
    source_entity_id: EntityId,
) -> ProjectionResult<Vec<GraphEdge>> {
    let mut statement = connection.prepare(concat!(
        "SELECT claim_id, source_entity_id, predicate_id, target_entity_id, scope_id, ",
        "security_domain, source_accept_seq, stable_tiebreaker ",
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
                row.get::<_, i64>(6)?,
                row.get::<_, Vec<u8>>(7)?,
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
            source_record_accept_seq: positive_u64(row.6, "query graph accept_seq")?,
            generation_id,
            source_watermark,
            stable_tiebreaker: ContentDigest::from_sha256_bytes(fixed_bytes(
                row.7,
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
