//! FTS5 Korean/code baselines and separate exact-symbol retrieval.

use std::str::FromStr;

use academic_domain::{
    ArtifactId, ClaimId, ContentDigest, DomainError, DomainId, EntityId, EvidenceId,
};
use rusqlite::{Connection, Transaction, params};

use crate::{
    checksum::{append_field, append_optional_field},
    generation::{GenerationId, ProjectionKind},
    runner::{ProjectionError, ProjectionResult},
};

/// Lossless locator bytes copied from canonical normalized evidence rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactLocator {
    pub evidence_id: EvidenceId,
    pub artifact_id: ArtifactId,
    pub representation_index: u64,
    pub locator_kind: String,
    pub locator_payload: Vec<u8>,
}

/// One ranked FTS result. `rank` is the deterministic result ordinal after
/// SQLite BM25 ordering and the stable SHA-256 tie-breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub rank: u64,
    pub text: String,
    pub subject_entity_id: EntityId,
    pub predicate_id: String,
    pub claim_id: ClaimId,
    pub locator: ExactLocator,
    pub domain: DomainId,
    pub generation_id: GenerationId,
    pub source_watermark: u64,
    pub source_record_accept_seq: u64,
    pub stable_tiebreaker: ContentDigest,
}

/// One case-sensitive exact-symbol result from the non-FTS lookup table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactSymbolHit {
    pub symbol: String,
    pub text: String,
    pub subject_entity_id: EntityId,
    pub predicate_id: String,
    pub claim_id: ClaimId,
    pub locator: ExactLocator,
    pub domain: DomainId,
    pub generation_id: GenerationId,
    pub source_watermark: u64,
    pub source_record_accept_seq: u64,
    pub stable_tiebreaker: ContentDigest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchSourceRecord {
    record_key: [u8; 32],
    claim_id: [u8; 16],
    evidence_id: [u8; 16],
    subject_entity_id: [u8; 16],
    predicate_id: String,
    body: String,
    symbol: Option<String>,
    artifact_id: [u8; 16],
    representation_index: u64,
    locator_kind: String,
    locator_payload: Vec<u8>,
    domain: [u8; 16],
    source_accept_seq: u64,
}

impl SearchSourceRecord {
    fn without_key_canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_field(&mut bytes, b"SEARCH_CONTENT_V1");
        append_field(&mut bytes, &self.claim_id);
        append_field(&mut bytes, &self.evidence_id);
        append_field(&mut bytes, &self.subject_entity_id);
        append_field(&mut bytes, self.predicate_id.as_bytes());
        append_field(&mut bytes, self.body.as_bytes());
        append_optional_field(&mut bytes, self.symbol.as_deref().map(str::as_bytes));
        append_field(&mut bytes, &self.artifact_id);
        append_field(&mut bytes, &self.representation_index.to_be_bytes());
        append_field(&mut bytes, self.locator_kind.as_bytes());
        append_field(&mut bytes, &self.locator_payload);
        append_field(&mut bytes, &self.domain);
        append_field(&mut bytes, &self.source_accept_seq.to_be_bytes());
        bytes
    }

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = self.without_key_canonical_bytes();
        append_field(&mut bytes, &self.record_key);
        bytes
    }

    fn stable_tiebreaker(&self) -> ContentDigest {
        ContentDigest::sha256(&self.canonical_bytes())
    }
}

#[derive(Debug)]
struct RawSearchRow {
    claim_id: Vec<u8>,
    evidence_id: Vec<u8>,
    subject_entity_id: Vec<u8>,
    predicate_id: String,
    body: String,
    artifact_id: Vec<u8>,
    representation_index: i64,
    locator_kind: String,
    locator_payload: Vec<u8>,
    domain: Vec<u8>,
    source_accept_seq: i64,
}

pub(crate) fn load_search_sources(
    canonical: &Connection,
    domain: DomainId,
    watermark: u64,
) -> ProjectionResult<Vec<SearchSourceRecord>> {
    let mut statement = canonical.prepare(concat!(
        "SELECT c.claim_id, ce.evidence_id, c.subject_entity_id, c.predicate_id, ",
        "c.object_text, ei.artifact_id, ei.representation_index, ar.locator_kind, ",
        "ar.locator_payload, e.domain_id, e.accept_seq FROM claim c ",
        "JOIN ledger_event e ON e.event_id = c.assertion_event_id ",
        "JOIN claim_evidence ce ON ce.claim_id = c.claim_id ",
        "JOIN evidence_item ei ON ei.evidence_id = ce.evidence_id ",
        "JOIN artifact_representation ar ON ar.artifact_id = ei.artifact_id ",
        "AND ar.representation_index = ei.representation_index ",
        "WHERE c.object_kind = 'TEXT' AND length(c.object_text) > 0 ",
        "AND e.domain_id = ?1 AND e.accept_seq <= ?2 ",
        "ORDER BY c.claim_id, ce.evidence_ordinal, ce.evidence_id"
    ))?;
    let rows = statement.query_map(
        params![domain.as_bytes().as_slice(), checked_i64(watermark)?],
        |row| {
            Ok(RawSearchRow {
                claim_id: row.get(0)?,
                evidence_id: row.get(1)?,
                subject_entity_id: row.get(2)?,
                predicate_id: row.get(3)?,
                body: row.get(4)?,
                artifact_id: row.get(5)?,
                representation_index: row.get(6)?,
                locator_kind: row.get(7)?,
                locator_payload: row.get(8)?,
                domain: row.get(9)?,
                source_accept_seq: row.get(10)?,
            })
        },
    )?;
    let mut records = Vec::new();
    for row in rows {
        let row = row?;
        let symbol = row
            .predicate_id
            .ends_with(".symbol")
            .then(|| row.body.clone());
        let mut record = SearchSourceRecord {
            record_key: [0_u8; 32],
            claim_id: fixed_bytes(row.claim_id, "search claim identifier")?,
            evidence_id: fixed_bytes(row.evidence_id, "search evidence identifier")?,
            subject_entity_id: fixed_bytes(row.subject_entity_id, "search subject entity")?,
            predicate_id: row.predicate_id,
            body: row.body,
            symbol,
            artifact_id: fixed_bytes(row.artifact_id, "search artifact identifier")?,
            representation_index: nonnegative_u64(
                row.representation_index,
                "search representation index",
            )?,
            locator_kind: row.locator_kind,
            locator_payload: row.locator_payload,
            domain: fixed_bytes(row.domain, "search domain identifier")?,
            source_accept_seq: positive_u64(row.source_accept_seq, "search source accept_seq")?,
        };
        record.record_key =
            *ContentDigest::sha256(&record.without_key_canonical_bytes()).as_bytes();
        records.push(record);
    }
    records.sort_by_key(|record| record.record_key);
    Ok(records)
}

pub(crate) fn write_search_records<F>(
    transaction: &Transaction<'_>,
    kind: ProjectionKind,
    generation_id: GenerationId,
    records: &[SearchSourceRecord],
    mut after_record: F,
) -> ProjectionResult<()>
where
    F: FnMut(usize, usize) -> ProjectionResult<()>,
{
    let fts_table = lexical_table(kind)?;
    for (record_index, record) in records.iter().enumerate() {
        let stable_tiebreaker = record.stable_tiebreaker();
        transaction.execute(
            concat!(
                "INSERT INTO projection_search_content (generation_id, record_key, claim_id, ",
                "evidence_id, subject_entity_id, predicate_id, body, artifact_id, ",
                "representation_index, locator_kind, locator_payload, security_domain, ",
                "source_accept_seq, stable_tiebreaker) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                generation_id.as_bytes().as_slice(),
                record.record_key.as_slice(),
                record.claim_id.as_slice(),
                record.evidence_id.as_slice(),
                record.subject_entity_id.as_slice(),
                record.predicate_id,
                record.body,
                record.artifact_id.as_slice(),
                checked_i64(record.representation_index)?,
                record.locator_kind,
                record.locator_payload,
                record.domain.as_slice(),
                checked_i64(record.source_accept_seq)?,
                stable_tiebreaker.as_bytes().as_slice(),
            ],
        )?;
        let content_id = transaction.last_insert_rowid();
        let insert_fts =
            format!("INSERT INTO {fts_table} (rowid, body, content_id) VALUES (?1, ?2, ?3)");
        transaction.execute(&insert_fts, params![content_id, record.body, content_id])?;
        if let Some(symbol) = &record.symbol {
            transaction.execute(
                concat!(
                    "INSERT INTO projection_exact_symbol ",
                    "(generation_id, symbol, content_id, stable_tiebreaker) ",
                    "VALUES (?1, ?2, ?3, ?4)"
                ),
                params![
                    generation_id.as_bytes().as_slice(),
                    symbol,
                    content_id,
                    stable_tiebreaker.as_bytes().as_slice(),
                ],
            )?;
        }
        after_record(record_index + 1, records.len())?;
    }
    Ok(())
}

pub(crate) fn persisted_search_canonical_records(
    connection: &Connection,
    generation_id: GenerationId,
) -> ProjectionResult<Vec<Vec<u8>>> {
    let mut statement = connection.prepare(concat!(
        "SELECT c.record_key, c.claim_id, c.evidence_id, c.subject_entity_id, c.predicate_id, ",
        "c.body, s.symbol, c.artifact_id, c.representation_index, c.locator_kind, ",
        "c.locator_payload, c.security_domain, c.source_accept_seq ",
        "FROM projection_search_content c LEFT JOIN projection_exact_symbol s ",
        "ON s.generation_id = c.generation_id AND s.content_id = c.content_id ",
        "WHERE c.generation_id = ?1 ORDER BY c.record_key"
    ))?;
    let rows = statement.query_map([generation_id.as_bytes().as_slice()], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Vec<u8>>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, Vec<u8>>(10)?,
            row.get::<_, Vec<u8>>(11)?,
            row.get::<_, i64>(12)?,
        ))
    })?;
    let mut canonical = Vec::new();
    for row in rows {
        let row = row?;
        canonical.push(
            SearchSourceRecord {
                record_key: fixed_bytes(row.0, "persisted search record key")?,
                claim_id: fixed_bytes(row.1, "persisted search claim identifier")?,
                evidence_id: fixed_bytes(row.2, "persisted search evidence identifier")?,
                subject_entity_id: fixed_bytes(row.3, "persisted search subject entity")?,
                predicate_id: row.4,
                body: row.5,
                symbol: row.6,
                artifact_id: fixed_bytes(row.7, "persisted search artifact identifier")?,
                representation_index: nonnegative_u64(
                    row.8,
                    "persisted search representation index",
                )?,
                locator_kind: row.9,
                locator_payload: row.10,
                domain: fixed_bytes(row.11, "persisted search domain")?,
                source_accept_seq: positive_u64(row.12, "persisted search accept_seq")?,
            }
            .canonical_bytes(),
        );
    }
    Ok(canonical)
}

pub(crate) fn read_ranked_hits(
    connection: &Connection,
    kind: ProjectionKind,
    generation_id: GenerationId,
    source_watermark: u64,
    query: &str,
    limit: usize,
) -> ProjectionResult<Vec<SearchHit>> {
    let table = lexical_table(kind)?;
    let query = fts_literal(query)?;
    let limit = i64::try_from(limit.min(100))
        .map_err(|_| ProjectionError::Corrupt("search result limit overflow".to_owned()))?;
    let sql = format!(
        "SELECT c.body, c.subject_entity_id, c.predicate_id, c.claim_id, c.evidence_id, \
         c.artifact_id, c.representation_index, c.locator_kind, c.locator_payload, \
         c.security_domain, c.source_accept_seq, c.stable_tiebreaker FROM {table} f \
         JOIN projection_search_content c ON c.content_id = f.content_id \
         WHERE {table} MATCH ?1 AND c.generation_id = ?2 \
         ORDER BY bm25({table}), c.stable_tiebreaker, c.record_key LIMIT ?3"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![query, generation_id.as_bytes().as_slice(), limit],
        read_hit_row,
    )?;
    rows.enumerate()
        .map(|(rank, row)| {
            let row = row?;
            hit_from_raw(
                row,
                u64::try_from(rank).map_err(|_| {
                    ProjectionError::Corrupt("search rank ordinal overflow".to_owned())
                })?,
                generation_id,
                source_watermark,
            )
        })
        .collect()
}

pub(crate) fn read_exact_symbol_hits(
    connection: &Connection,
    generation_id: GenerationId,
    source_watermark: u64,
    symbol: &str,
) -> ProjectionResult<Vec<ExactSymbolHit>> {
    if symbol.is_empty() || symbol.contains('\0') {
        return Err(ProjectionError::InvalidQuery(
            "exact symbol must be non-empty and contain no NUL",
        ));
    }
    let mut statement = connection.prepare(concat!(
        "SELECT s.symbol, c.body, c.subject_entity_id, c.predicate_id, c.claim_id, ",
        "c.evidence_id, c.artifact_id, c.representation_index, c.locator_kind, ",
        "c.locator_payload, c.security_domain, c.source_accept_seq, c.stable_tiebreaker ",
        "FROM projection_exact_symbol s JOIN projection_search_content c ",
        "ON c.content_id = s.content_id AND c.generation_id = s.generation_id ",
        "WHERE s.generation_id = ?1 AND s.symbol = ?2 COLLATE BINARY ",
        "ORDER BY s.stable_tiebreaker, c.record_key"
    ))?;
    let rows = statement.query_map(
        params![generation_id.as_bytes().as_slice(), symbol],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Vec<u8>>(5)?,
                row.get::<_, Vec<u8>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Vec<u8>>(9)?,
                row.get::<_, Vec<u8>>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Vec<u8>>(12)?,
            ))
        },
    )?;
    rows.map(|row| {
        let row = row?;
        Ok(ExactSymbolHit {
            symbol: row.0,
            text: row.1,
            subject_entity_id: id_from_bytes(row.2, "symbol subject entity")?,
            predicate_id: row.3,
            claim_id: id_from_bytes(row.4, "symbol claim identifier")?,
            locator: ExactLocator {
                evidence_id: id_from_bytes(row.5, "symbol evidence identifier")?,
                artifact_id: id_from_bytes(row.6, "symbol artifact identifier")?,
                representation_index: nonnegative_u64(row.7, "symbol representation index")?,
                locator_kind: row.8,
                locator_payload: row.9,
            },
            domain: id_from_bytes(row.10, "symbol domain identifier")?,
            generation_id,
            source_watermark,
            source_record_accept_seq: positive_u64(row.11, "symbol source accept_seq")?,
            stable_tiebreaker: ContentDigest::from_sha256_bytes(fixed_bytes(
                row.12,
                "symbol stable tiebreaker",
            )?),
        })
    })
    .collect()
}

type RawHit = (
    String,
    Vec<u8>,
    String,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    i64,
    String,
    Vec<u8>,
    Vec<u8>,
    i64,
    Vec<u8>,
);

fn read_hit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawHit> {
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
    ))
}

fn hit_from_raw(
    row: RawHit,
    rank: u64,
    generation_id: GenerationId,
    source_watermark: u64,
) -> ProjectionResult<SearchHit> {
    Ok(SearchHit {
        rank,
        text: row.0,
        subject_entity_id: id_from_bytes(row.1, "search hit subject entity")?,
        predicate_id: row.2,
        claim_id: id_from_bytes(row.3, "search hit claim identifier")?,
        locator: ExactLocator {
            evidence_id: id_from_bytes(row.4, "search hit evidence identifier")?,
            artifact_id: id_from_bytes(row.5, "search hit artifact identifier")?,
            representation_index: nonnegative_u64(row.6, "search hit representation index")?,
            locator_kind: row.7,
            locator_payload: row.8,
        },
        domain: id_from_bytes(row.9, "search hit domain identifier")?,
        generation_id,
        source_watermark,
        source_record_accept_seq: positive_u64(row.10, "search hit source accept_seq")?,
        stable_tiebreaker: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.11,
            "search hit stable tiebreaker",
        )?),
    })
}

fn lexical_table(kind: ProjectionKind) -> ProjectionResult<&'static str> {
    match kind {
        ProjectionKind::Unicode61 => Ok("projection_search_unicode"),
        ProjectionKind::Trigram => Ok("projection_search_trigram"),
        ProjectionKind::Graph => Err(ProjectionError::InvalidQuery(
            "graph generations do not support lexical search",
        )),
    }
}

fn fts_literal(query: &str) -> ProjectionResult<String> {
    let query = query.trim();
    if query.is_empty() || query.contains('\0') {
        return Err(ProjectionError::InvalidQuery(
            "ranked query must be non-empty and contain no NUL",
        ));
    }
    Ok(format!("\"{}\"", query.replace('"', "\"\"")))
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
    let bytes = fixed_bytes::<16>(bytes, reason)?;
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

fn checked_i64(value: u64) -> ProjectionResult<i64> {
    i64::try_from(value).map_err(|_| ProjectionError::IntegerOverflow(value))
}

fn nonnegative_u64(value: i64, reason: &'static str) -> ProjectionResult<u64> {
    u64::try_from(value).map_err(|_| ProjectionError::Corrupt(reason.to_owned()))
}

fn positive_u64(value: i64, reason: &'static str) -> ProjectionResult<u64> {
    let value = nonnegative_u64(value, reason)?;
    if value == 0 {
        Err(ProjectionError::Corrupt(reason.to_owned()))
    } else {
        Ok(value)
    }
}
