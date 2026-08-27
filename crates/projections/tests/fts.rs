use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_domain::{ContentDigest, DomainId};
use academic_projections::{
    generation::{ProjectionAvailability, ProjectionKind},
    query::ProjectionReader,
    runner::ProjectionRunner,
};
use academic_store::{connection, migration};
use rusqlite::{Connection, params};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const KOREAN_CODE_CASES: &str = include_str!("../../../testdata/search/korean-code.jsonl");
const KOREAN_CODE_EXPECTED: &str = include_str!("../../../testdata/search/expected.json");

#[derive(Debug)]
struct Fixture {
    root: PathBuf,
    canonical: PathBuf,
    sidecar: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> TestResult<Self> {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "academic-projections-fts-{label}-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)?;
        let canonical = root.join("canonical.sqlite3");
        migration::migrate_pre_listen(&canonical, digest(250, 1))?;
        Ok(Self {
            sidecar: root.join("projection.sqlite3"),
            root,
            canonical,
        })
    }

    fn runner(&self) -> TestResult<ProjectionRunner> {
        let canonical = connection::open_reader(&self.canonical)?;
        Ok(ProjectionRunner::open(
            &canonical,
            &self.sidecar,
            ContentDigest::sha256(b"fts-test-builder"),
            ContentDigest::sha256(b"fts-test-config"),
        )?)
    }

    fn reader(&self) -> TestResult<ProjectionReader> {
        let canonical = connection::open_reader(&self.canonical)?;
        Ok(ProjectionReader::new(&canonical, &self.sidecar))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

#[test]
fn fts_drop_and_rebuild_matches() -> TestResult {
    let fixture = Fixture::new("drop-rebuild")?;
    let domain = domain(1)?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 1,
            accept_seq: 1,
            claim_seed: 11,
            text: "합성 트랜잭션 원자성 검증",
            predicate: "notes.body",
            locator_payload: b"page=7;chars=4-9",
        },
    )?;
    seed_outbox(&fixture.canonical, 1, 1, 1, 1)?;

    let first_receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Unicode61, domain)?;
    let first =
        fixture
            .reader()?
            .search_ranked(ProjectionKind::Unicode61, domain, "트랜잭션", 10)?;
    assert_eq!(first.records.len(), 1);

    fixture
        .runner()?
        .drop_projection(ProjectionKind::Unicode61, domain)?;
    let dropped =
        fixture
            .reader()?
            .search_ranked(ProjectionKind::Unicode61, domain, "트랜잭션", 10)?;
    assert!(matches!(
        dropped.availability,
        ProjectionAvailability::NoActive { .. }
    ));
    assert!(dropped.records.is_empty());

    let second_receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Unicode61, domain)?;
    let second =
        fixture
            .reader()?
            .search_ranked(ProjectionKind::Unicode61, domain, "트랜잭션", 10)?;
    assert_eq!(
        first_receipt.metadata.canonical_checksum,
        second_receipt.metadata.canonical_checksum
    );
    assert_eq!(first.records.len(), second.records.len());
    assert_search_record_equivalent(&first.records[0], &second.records[0]);
    Ok(())
}

#[test]
fn fts_domain_isolation() -> TestResult {
    let fixture = Fixture::new("domain-isolation")?;
    let first_domain = domain(1)?;
    let second_domain = domain(2)?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 1,
            accept_seq: 1,
            claim_seed: 12,
            text: "sharedtoken first domain",
            predicate: "notes.body",
            locator_payload: b"domain=1",
        },
    )?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 2,
            accept_seq: 2,
            claim_seed: 13,
            text: "sharedtoken second domain",
            predicate: "notes.body",
            locator_payload: b"domain=2",
        },
    )?;
    seed_outbox(&fixture.canonical, 1, 1, 2, 1)?;

    fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Unicode61, first_domain)?;
    fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Unicode61, second_domain)?;
    let first = fixture.reader()?.search_ranked(
        ProjectionKind::Unicode61,
        first_domain,
        "sharedtoken",
        10,
    )?;
    let second = fixture.reader()?.search_ranked(
        ProjectionKind::Unicode61,
        second_domain,
        "sharedtoken",
        10,
    )?;
    assert_eq!(first.records.len(), 1);
    assert_eq!(second.records.len(), 1);
    assert_eq!(first.records[0].domain, first_domain);
    assert_eq!(second.records[0].domain, second_domain);
    assert_eq!(first.records[0].locator.locator_payload, b"domain=1");
    assert_eq!(second.records[0].locator.locator_payload, b"domain=2");
    assert_ne!(
        first.records[0].generation_id,
        second.records[0].generation_id
    );
    Ok(())
}

#[test]
fn fts_korean_code_baseline() -> TestResult {
    assert_eq!(KOREAN_CODE_CASES.lines().count(), 3);
    assert!(KOREAN_CODE_CASES.contains("합성 트랜잭션 원자성 검증"));
    assert!(KOREAN_CODE_CASES.contains("OrderService.updateStatus handles retries"));
    assert!(KOREAN_CODE_EXPECTED.contains("\"korean-unicode61\""));
    assert!(KOREAN_CODE_EXPECTED.contains("\"code-trigram\""));

    let fixture = Fixture::new("korean-code")?;
    let domain = domain(1)?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 1,
            accept_seq: 1,
            claim_seed: 14,
            text: "합성 트랜잭션 원자성 검증",
            predicate: "notes.body",
            locator_payload: b"korean-locator",
        },
    )?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 1,
            accept_seq: 2,
            claim_seed: 15,
            text: "OrderService.updateStatus handles retries",
            predicate: "code.body",
            locator_payload: b"repository-bytes=18-30",
        },
    )?;
    seed_outbox(&fixture.canonical, 1, 1, 2, 1)?;

    let unicode_receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Unicode61, domain)?;
    let trigram_receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Trigram, domain)?;
    assert_eq!(
        unicode_receipt.metadata.canonical_checksum,
        trigram_receipt.metadata.canonical_checksum
    );
    assert_eq!(
        unicode_receipt
            .metadata
            .canonical_checksum
            .map(|checksum| checksum.to_string()),
        Some("sha256:4b74f5f96332abee5c108de54d8e434cc9698eae212909e3717c62d94d1ab77e".to_owned())
    );
    let korean =
        fixture
            .reader()?
            .search_ranked(ProjectionKind::Unicode61, domain, "트랜잭션", 10)?;
    let code =
        fixture
            .reader()?
            .search_ranked(ProjectionKind::Trigram, domain, "updateStatus", 10)?;
    assert_eq!(korean.records.len(), 1);
    assert_eq!(code.records.len(), 1);
    assert_eq!(korean.records[0].text, "합성 트랜잭션 원자성 검증");
    assert_eq!(
        code.records[0].text,
        "OrderService.updateStatus handles retries"
    );
    assert_eq!(korean.records[0].locator.locator_payload, b"korean-locator");
    assert_eq!(
        code.records[0].locator.locator_payload,
        b"repository-bytes=18-30"
    );
    assert_eq!(korean.records[0].source_watermark, 2);
    assert_eq!(code.records[0].source_watermark, 2);
    assert_eq!(
        korean.records[0].generation_id,
        unicode_receipt.metadata.generation_id
    );
    assert_eq!(
        code.records[0].generation_id,
        trigram_receipt.metadata.generation_id
    );
    Ok(())
}

#[test]
fn exact_symbol_lookup_is_not_ranked_text() -> TestResult {
    let fixture = Fixture::new("exact-symbol")?;
    let domain = domain(1)?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 1,
            accept_seq: 1,
            claim_seed: 16,
            text: "OrderService.updateStatus",
            predicate: "code.symbol",
            locator_payload: b"symbol-definition",
        },
    )?;
    seed_text_claim(
        &fixture.canonical,
        TextClaimSeed {
            domain_seed: 1,
            accept_seq: 2,
            claim_seed: 17,
            text: "OrderService updateStatus ranked documentation",
            predicate: "code.description",
            locator_payload: b"documentation",
        },
    )?;
    seed_outbox(&fixture.canonical, 1, 1, 2, 1)?;
    fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Trigram, domain)?;

    let exact = fixture.reader()?.exact_symbol_lookup(
        ProjectionKind::Trigram,
        domain,
        "OrderService.updateStatus",
    )?;
    let wrong_case = fixture.reader()?.exact_symbol_lookup(
        ProjectionKind::Trigram,
        domain,
        "orderservice.updatestatus",
    )?;
    let ranked =
        fixture
            .reader()?
            .search_ranked(ProjectionKind::Trigram, domain, "updateStatus", 10)?;
    assert_eq!(exact.records.len(), 1);
    assert_eq!(exact.records[0].symbol, "OrderService.updateStatus");
    assert_eq!(
        exact.records[0].locator.locator_payload,
        b"symbol-definition"
    );
    assert!(wrong_case.records.is_empty());
    assert_eq!(ranked.records.len(), 2);
    assert!(ranked.records.iter().all(|hit| hit.rank < 2));
    Ok(())
}

fn assert_search_record_equivalent(
    left: &academic_projections::fts::SearchHit,
    right: &academic_projections::fts::SearchHit,
) {
    assert_eq!(left.rank, right.rank);
    assert_eq!(left.text, right.text);
    assert_eq!(left.subject_entity_id, right.subject_entity_id);
    assert_eq!(left.predicate_id, right.predicate_id);
    assert_eq!(left.claim_id, right.claim_id);
    assert_eq!(left.locator, right.locator);
    assert_eq!(left.domain, right.domain);
    assert_eq!(
        left.source_record_accept_seq,
        right.source_record_accept_seq
    );
    assert_eq!(left.source_watermark, right.source_watermark);
    assert_eq!(left.stable_tiebreaker, right.stable_tiebreaker);
}

#[derive(Debug, Clone, Copy)]
struct TextClaimSeed<'a> {
    domain_seed: u8,
    accept_seq: u8,
    claim_seed: u8,
    text: &'a str,
    predicate: &'a str,
    locator_payload: &'a [u8],
}

fn seed_text_claim(database: &Path, seed: TextClaimSeed<'_>) -> TestResult {
    let TextClaimSeed {
        domain_seed,
        accept_seq,
        claim_seed,
        text,
        predicate,
        locator_payload,
    } = seed;
    let domain = domain(domain_seed)?;
    let connection = Connection::open(database)?;
    seed_domain_base(&connection, domain_seed, domain)?;
    let claim_event = pair_id(3, domain_seed, claim_seed);
    let artifact_event = pair_id(4, domain_seed, claim_seed);
    let evidence_event = pair_id(5, domain_seed, claim_seed);
    let artifact_id = pair_id(6, domain_seed, claim_seed);
    let evidence_id = pair_id(7, domain_seed, claim_seed);
    let claim_id = pair_id(8, domain_seed, claim_seed);
    let auxiliary_offset = i64::from(domain_seed) * 100 + i64::from(claim_seed);
    insert_event(
        &connection,
        domain_seed,
        artifact_event,
        1_000 + auxiliary_offset,
        "ARTIFACT_REGISTERED",
        digest(4, claim_seed),
    )?;
    connection.execute(
        concat!(
            "INSERT INTO artifact_descriptor (artifact_id, registered_event_id, content_digest, ",
            "media_type, byte_length, domain_id, confidentiality, retention_class, ",
            "permission_lineage_id, format_version, vault_locator) VALUES ",
            "(?1, ?2, ?3, 'text/plain', ?4, ?5, 'PUBLIC', 'EPHEMERAL', ?6, 1, ?7)"
        ),
        params![
            artifact_id.as_slice(),
            artifact_event.as_slice(),
            digest(6, claim_seed).as_slice(),
            i64::try_from(text.len())?,
            domain.as_bytes().as_slice(),
            pair_id(9, domain_seed, claim_seed).as_slice(),
            digest(7, claim_seed).as_slice(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT INTO artifact_representation (artifact_id, representation_index, ",
            "locator_kind, locator_payload, content_digest, byte_length) ",
            "VALUES (?1, 0, 'REPOSITORY_BYTES', ?2, ?3, ?4)"
        ),
        params![
            artifact_id.as_slice(),
            locator_payload,
            digest(8, claim_seed).as_slice(),
            i64::try_from(text.len())?,
        ],
    )?;
    insert_event(
        &connection,
        domain_seed,
        evidence_event,
        2_000 + auxiliary_offset,
        "EVIDENCE_REGISTERED",
        digest(5, claim_seed),
    )?;
    connection.execute(
        concat!(
            "INSERT INTO evidence_item (evidence_id, registered_event_id, artifact_id, ",
            "representation_index, excerpt_digest, evidence_role, evidence_strength, ",
            "extraction_method, extractor_version) VALUES ",
            "(?1, ?2, ?3, 0, ?4, 'SUPPORTS', 'DIRECT', 'fixture', '1')"
        ),
        params![
            evidence_id.as_slice(),
            evidence_event.as_slice(),
            artifact_id.as_slice(),
            digest(9, claim_seed).as_slice(),
        ],
    )?;
    insert_event(
        &connection,
        domain_seed,
        claim_event,
        i64::from(accept_seq),
        "CLAIM_ASSERTED",
        digest(3, claim_seed),
    )?;
    connection.execute(
        concat!(
            "INSERT INTO claim (claim_id, assertion_event_id, subject_entity_id, predicate_id, ",
            "scope_id, object_kind, object_text, authority_class, epistemic_status, valid_from) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, 'TEXT', ?6, ",
            "'DIRECT_OBSERVATION', 'CODE_OBSERVED', 0)"
        ),
        params![
            claim_id.as_slice(),
            claim_event.as_slice(),
            pair_id(10, domain_seed, claim_seed).as_slice(),
            predicate,
            pair_id(2, domain_seed, 1).as_slice(),
            text,
        ],
    )?;
    connection.execute(
        "INSERT INTO claim_evidence (claim_id, evidence_id, evidence_ordinal) VALUES (?1, ?2, 0)",
        params![claim_id.as_slice(), evidence_id.as_slice()],
    )?;
    Ok(())
}

fn seed_domain_base(connection: &Connection, domain_seed: u8, domain: DomainId) -> TestResult {
    connection
        .execute(
            concat!(
                "INSERT OR IGNORE INTO ledger_batch (batch_id, signed_envelope, envelope_hash, ",
                "deterministic_payload, deterministic_payload_hash, signing_public_key, signature, ",
                "device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
                "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, accepted_at) ",
                "VALUES (?1, x'01', ?2, x'01', ?3, ?4, ?5, ?6, 1, 5000, NULL, 0, 1, 1, 5000, 0)"
            ),
            params![
                pair_id(1, domain_seed, 1).as_slice(),
                digest(1, domain_seed).as_slice(),
                digest(2, domain_seed).as_slice(),
                digest(10, domain_seed).as_slice(),
                [domain_seed; 64].as_slice(),
                pair_id(1, domain_seed, 2).as_slice(),
            ],
        )?;
    insert_event(
        connection,
        domain_seed,
        pair_id(2, domain_seed, 2),
        4_000 + i64::from(domain_seed),
        "SCOPE_REGISTERED",
        digest(2, domain_seed),
    )?;
    connection.execute(
        concat!(
            "INSERT OR IGNORE INTO scope (scope_id, created_event_id, domain_id, label) ",
            "VALUES (?1, ?2, ?3, 'projection fixture')"
        ),
        params![
            pair_id(2, domain_seed, 1).as_slice(),
            pair_id(2, domain_seed, 2).as_slice(),
            domain.as_bytes().as_slice(),
        ],
    )?;
    Ok(())
}

fn insert_event(
    connection: &Connection,
    domain_seed: u8,
    event_id: [u8; 16],
    sequence: i64,
    event_kind: &str,
    payload_hash: [u8; 32],
) -> TestResult {
    connection.execute(
        concat!(
            "INSERT OR IGNORE INTO ledger_event (event_id, batch_id, origin_seq, ",
            "origin_observed_at, accept_seq, actor_kind, actor_canonical, domain_id, ",
            "event_kind, canonical_payload, payload_hash) ",
            "VALUES (?1, ?2, ?3, 0, ?3, 'USER', x'01', ?4, ?5, x'01', ?6)"
        ),
        params![
            event_id.as_slice(),
            pair_id(1, domain_seed, 1).as_slice(),
            sequence,
            domain(domain_seed)?.as_bytes().as_slice(),
            event_kind,
            payload_hash.as_slice(),
        ],
    )?;
    Ok(())
}

fn seed_outbox(
    database: &Path,
    outbox_seq: u8,
    accept_start: u8,
    accept_end: u8,
    batch_domain_seed: u8,
) -> TestResult {
    let connection = Connection::open(database)?;
    connection.execute(
        concat!(
            "INSERT INTO projection_outbox (outbox_seq, accepted_batch_id, accept_seq_start, ",
            "accept_seq_end, canonical_revision, event_kind_mask, payload_digest, created_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?1, zeroblob(8), ?5, 0)"
        ),
        params![
            i64::from(outbox_seq),
            pair_id(1, batch_domain_seed, 1).as_slice(),
            i64::from(accept_start),
            i64::from(accept_end),
            digest(20, outbox_seq).as_slice(),
        ],
    )?;
    Ok(())
}

fn domain(seed: u8) -> TestResult<DomainId> {
    Ok(uuid_text(id(seed)).parse()?)
}

fn id(seed: u8) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    bytes[5] = seed;
    bytes[6] = 0x70;
    bytes[7] = seed;
    bytes[8] = 0x80;
    bytes[15] = seed;
    bytes
}

fn pair_id(tag: u8, domain_seed: u8, item_seed: u8) -> [u8; 16] {
    let mut bytes = id(item_seed);
    bytes[0] = tag;
    bytes[1] = domain_seed;
    bytes
}

fn digest(tag: u8, seed: u8) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[0] = tag;
    bytes[31] = seed;
    bytes
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
