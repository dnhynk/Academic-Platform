//! FTS5 Korean/code baselines and separate exact-symbol retrieval.

use std::{collections::BTreeMap, str::FromStr};

use academic_domain::{
    ArtifactId, AuthorityClass, ClaimId, ClaimObject, ContentDigest, DomainError, DomainId,
    EntityId, EpistemicStatus, EvidenceId, TimestampMillis, ValidInterval,
};
use academic_store::queries::{ProjectionEvidenceLocator, ProjectionResolvedClaim};
use rusqlite::{Connection, Transaction, params};

use crate::{
    checksum::{append_field, append_optional_field},
    generation::{GenerationId, ProjectionCoordinates, ProjectionKind, ResolutionProvenance},
    resolution::{
        AuthorityPolicy, authority_name, authority_policy_name, epistemic_name, parse_authority,
        parse_authority_policy, parse_epistemic,
    },
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
    pub authority_class: AuthorityClass,
    pub epistemic_status: EpistemicStatus,
    pub valid_time: ValidInterval,
    pub resolution: ResolutionProvenance,
    pub generation_id: GenerationId,
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
    pub authority_class: AuthorityClass,
    pub epistemic_status: EpistemicStatus,
    pub valid_time: ValidInterval,
    pub resolution: ResolutionProvenance,
    pub generation_id: GenerationId,
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
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
    authority_policy: AuthorityPolicy,
    valid_from_unix_ms: i64,
    valid_to_unix_ms: Option<i64>,
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

    pub(crate) fn verification_bytes(&self) -> Vec<u8> {
        let mut bytes = self.canonical_bytes();
        let stable_tiebreaker = self.stable_tiebreaker();
        append_field(&mut bytes, stable_tiebreaker.as_bytes().as_slice());
        bytes
    }
}

pub(crate) fn load_search_sources(
    resolved: &[ProjectionResolvedClaim],
    evidence_locators: &[ProjectionEvidenceLocator],
    domain: DomainId,
) -> ProjectionResult<Vec<SearchSourceRecord>> {
    let claims = resolved
        .iter()
        .map(|record| (record.claim.id, record))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    for locator in evidence_locators {
        let claim = claims.get(&locator.claim_id).ok_or_else(|| {
            ProjectionError::Corrupt(
                "search locator references a claim absent from the resolved snapshot".to_owned(),
            )
        })?;
        let ClaimObject::Text(canonical_body) = &claim.claim.object else {
            return Err(ProjectionError::Corrupt(
                "active search claim is not text-valued".to_owned(),
            ));
        };
        if canonical_body.is_empty() || !claim.claim.evidence_ids.contains(&locator.evidence_id) {
            return Err(ProjectionError::Corrupt(
                "search locator disagrees with the resolved claim evidence".to_owned(),
            ));
        }
        let symbol = claim
            .claim
            .predicate_id
            .as_str()
            .ends_with(".symbol")
            .then(|| canonical_body.clone());
        let mut record = SearchSourceRecord {
            record_key: [0_u8; 32],
            claim_id: *claim.claim.id.as_bytes(),
            evidence_id: *locator.evidence_id.as_bytes(),
            subject_entity_id: *claim.claim.subject_entity_id.as_bytes(),
            predicate_id: claim.claim.predicate_id.as_str().to_owned(),
            body: canonical_body.clone(),
            symbol,
            artifact_id: *locator.artifact_id.as_bytes(),
            representation_index: locator.representation_index,
            locator_kind: locator.locator_kind.clone(),
            locator_payload: locator.locator_payload.clone(),
            domain: *domain.as_bytes(),
            authority_class: claim.claim.authority_class,
            epistemic_status: claim.claim.epistemic_status,
            authority_policy: claim.applied_policy,
            valid_from_unix_ms: claim.claim.valid_time.from().value(),
            valid_to_unix_ms: claim.claim.valid_time.to().map(TimestampMillis::value),
            source_accept_seq: claim.accept_seq,
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
                "authority_class, epistemic_status, authority_policy, valid_from_unix_ms, ",
                "valid_to_unix_ms, source_accept_seq, stable_tiebreaker) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ",
                "?14, ?15, ?16, ?17, ?18, ?19)"
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
                authority_name(record.authority_class),
                epistemic_name(record.epistemic_status),
                authority_policy_name(record.authority_policy),
                record.valid_from_unix_ms,
                record.valid_to_unix_ms,
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
        "c.locator_payload, c.security_domain, c.authority_class, c.epistemic_status, ",
        "c.authority_policy, c.valid_from_unix_ms, c.valid_to_unix_ms, c.source_accept_seq, ",
        "c.stable_tiebreaker ",
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
            row.get::<_, String>(12)?,
            row.get::<_, String>(13)?,
            row.get::<_, String>(14)?,
            row.get::<_, i64>(15)?,
            row.get::<_, Option<i64>>(16)?,
            row.get::<_, i64>(17)?,
            row.get::<_, Vec<u8>>(18)?,
        ))
    })?;
    let mut canonical = Vec::new();
    for row in rows {
        let row = row?;
        let record = SearchSourceRecord {
            record_key: fixed_bytes(row.0, "persisted search record key")?,
            claim_id: fixed_bytes(row.1, "persisted search claim identifier")?,
            evidence_id: fixed_bytes(row.2, "persisted search evidence identifier")?,
            subject_entity_id: fixed_bytes(row.3, "persisted search subject entity")?,
            predicate_id: row.4,
            body: row.5,
            symbol: row.6,
            artifact_id: fixed_bytes(row.7, "persisted search artifact identifier")?,
            representation_index: nonnegative_u64(row.8, "persisted search representation index")?,
            locator_kind: row.9,
            locator_payload: row.10,
            domain: fixed_bytes(row.11, "persisted search domain")?,
            authority_class: parse_authority(&row.12)?,
            epistemic_status: parse_epistemic(&row.13)?,
            authority_policy: parse_authority_policy(&row.14)?,
            valid_from_unix_ms: row.15,
            valid_to_unix_ms: row.16,
            source_accept_seq: positive_u64(row.17, "persisted search accept_seq")?,
        };
        let stored_tiebreaker = ContentDigest::from_sha256_bytes(fixed_bytes(
            row.18,
            "persisted search stable tiebreaker",
        )?);
        if stored_tiebreaker != record.stable_tiebreaker() {
            return Err(ProjectionError::Corrupt(
                "persisted search stable tiebreaker does not match canonical record".to_owned(),
            ));
        }
        canonical.push(record.verification_bytes());
    }
    Ok(canonical)
}

pub(crate) fn read_ranked_hits(
    connection: &Connection,
    kind: ProjectionKind,
    generation_id: GenerationId,
    coordinates: ProjectionCoordinates,
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
         c.security_domain, c.authority_class, c.epistemic_status, c.authority_policy, \
         c.valid_from_unix_ms, c.valid_to_unix_ms, c.source_accept_seq, \
         c.stable_tiebreaker FROM {table} f \
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
                coordinates,
            )
        })
        .collect()
}

pub(crate) fn read_exact_symbol_hits(
    connection: &Connection,
    generation_id: GenerationId,
    coordinates: ProjectionCoordinates,
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
        "c.locator_payload, c.security_domain, c.authority_class, c.epistemic_status, ",
        "c.authority_policy, c.valid_from_unix_ms, c.valid_to_unix_ms, c.source_accept_seq, ",
        "c.stable_tiebreaker ",
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
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, Option<i64>>(15)?,
                row.get::<_, i64>(16)?,
                row.get::<_, Vec<u8>>(17)?,
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
            authority_class: parse_authority(&row.11)?,
            epistemic_status: parse_epistemic(&row.12)?,
            valid_time: ValidInterval::new(
                TimestampMillis::new(row.14),
                row.15.map(TimestampMillis::new),
            )
            .map_err(ProjectionError::Domain)?,
            resolution: ResolutionProvenance {
                authority_policy: parse_authority_policy(&row.13)?,
                coordinates,
            },
            generation_id,
            source_record_accept_seq: positive_u64(row.16, "symbol source accept_seq")?,
            stable_tiebreaker: ContentDigest::from_sha256_bytes(fixed_bytes(
                row.17,
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
    String,
    String,
    String,
    i64,
    Option<i64>,
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
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
    ))
}

fn hit_from_raw(
    row: RawHit,
    rank: u64,
    generation_id: GenerationId,
    coordinates: ProjectionCoordinates,
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
        authority_class: parse_authority(&row.10)?,
        epistemic_status: parse_epistemic(&row.11)?,
        valid_time: ValidInterval::new(
            TimestampMillis::new(row.13),
            row.14.map(TimestampMillis::new),
        )
        .map_err(ProjectionError::Domain)?,
        resolution: ResolutionProvenance {
            authority_policy: parse_authority_policy(&row.12)?,
            coordinates,
        },
        generation_id,
        source_record_accept_seq: positive_u64(row.15, "search hit source accept_seq")?,
        stable_tiebreaker: ContentDigest::from_sha256_bytes(fixed_bytes(
            row.16,
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
