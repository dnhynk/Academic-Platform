use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_domain::{ContentDigest, DomainId, EntityId};
use academic_projections::{
    generation::{ProjectionAvailability, ProjectionKind},
    query::ProjectionReader,
    runner::ProjectionRunner,
};
use academic_store::{connection, migration};
use rusqlite::{Connection, params};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct Fixture {
    root: PathBuf,
    canonical: PathBuf,
    sidecar: PathBuf,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "academic-projections-graph-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)?;
        let canonical = root.join("canonical.sqlite3");
        migration::migrate_pre_listen(&canonical, digest(250))?;
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
            ContentDigest::sha256(b"graph-test-builder"),
            ContentDigest::sha256(b"graph-test-config"),
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
fn graph_drop_and_rebuild_matches() -> TestResult {
    let fixture = Fixture::new()?;
    let domain = domain(1)?;
    seed_base(&fixture.canonical, domain)?;
    seed_claim(&fixture.canonical, 1, 11, 21, 31, domain)?;
    seed_claim(&fixture.canonical, 2, 12, 21, 32, domain)?;
    seed_outbox(&fixture.canonical, 1, 1, 2)?;

    let first_receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Graph, domain)?;
    let first = fixture.reader()?.graph_neighbors(domain, entity(21)?)?;
    assert_eq!(first.records.len(), 2);

    fixture
        .runner()?
        .drop_projection(ProjectionKind::Graph, domain)?;
    let dropped = fixture.reader()?.graph_neighbors(domain, entity(21)?)?;
    assert!(matches!(
        dropped.availability,
        ProjectionAvailability::NoActive { .. }
    ));
    assert!(dropped.records.is_empty());

    let second_receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Graph, domain)?;
    let second = fixture.reader()?.graph_neighbors(domain, entity(21)?)?;
    assert_eq!(
        first_receipt.metadata.canonical_checksum,
        second_receipt.metadata.canonical_checksum
    );
    assert_eq!(first.records.len(), second.records.len());
    for (left, right) in first.records.iter().zip(&second.records) {
        assert_eq!(left.source_entity_id, right.source_entity_id);
        assert_eq!(left.predicate_id, right.predicate_id);
        assert_eq!(left.target_entity_id, right.target_entity_id);
        assert_eq!(left.claim_id, right.claim_id);
        assert_eq!(left.evidence_ids, right.evidence_ids);
        assert_eq!(left.scope_id, right.scope_id);
        assert_eq!(left.domain, right.domain);
        assert_eq!(
            left.source_record_accept_seq,
            right.source_record_accept_seq
        );
        assert_eq!(left.source_watermark, right.source_watermark);
        assert_eq!(left.stable_tiebreaker, right.stable_tiebreaker);
    }
    Ok(())
}

fn seed_base(database: &Path, domain: DomainId) -> TestResult {
    let connection = Connection::open(database)?;
    connection
        .execute(
            concat!(
                "INSERT INTO ledger_batch (batch_id, signed_envelope, envelope_hash, ",
                "deterministic_payload, deterministic_payload_hash, signing_public_key, signature, ",
                "device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
                "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, accepted_at) ",
                "VALUES (?1, x'01', ?2, x'01', ?3, ?4, ?5, ?6, 1, 5000, NULL, 0, 1, 1, 5000, 0)"
            ),
            params![
                tagged_id(1, 1).as_slice(),
                digest(240).as_slice(),
                digest(241).as_slice(),
                digest(242).as_slice(),
                [243_u8; 64].as_slice(),
                tagged_id(1, 2).as_slice(),
            ],
        )?;
    connection.execute(
        concat!(
            "INSERT INTO ledger_event (event_id, batch_id, origin_seq, origin_observed_at, ",
            "accept_seq, actor_kind, actor_canonical, domain_id, event_kind, ",
            "canonical_payload, payload_hash) VALUES ",
            "(?1, ?2, 4000, 0, 4000, 'USER', x'01', ?3, 'SCOPE_REGISTERED', x'01', ?4)"
        ),
        params![
            tagged_id(2, 2).as_slice(),
            tagged_id(1, 1).as_slice(),
            domain.as_bytes().as_slice(),
            digest(200).as_slice(),
        ],
    )?;
    connection
        .execute(
            "INSERT INTO scope (scope_id, created_event_id, domain_id, label) VALUES (?1, ?2, ?3, 'projection fixture')",
            params![
                tagged_id(2, 1).as_slice(),
                tagged_id(2, 2).as_slice(),
                domain.as_bytes().as_slice(),
            ],
        )?;
    Ok(())
}

fn seed_claim(
    database: &Path,
    accept_seq: u8,
    claim_seed: u8,
    source_seed: u8,
    target_seed: u8,
    domain: DomainId,
) -> TestResult {
    let connection = Connection::open(database)?;
    let event_id = tagged_id(3, claim_seed);
    connection.execute(
        concat!(
            "INSERT INTO ledger_event (event_id, batch_id, origin_seq, origin_observed_at, ",
            "accept_seq, actor_kind, actor_canonical, domain_id, event_kind, ",
            "canonical_payload, payload_hash) ",
            "VALUES (?1, ?2, ?3, 0, ?3, 'USER', x'01', ?4, 'CLAIM_ASSERTED', x'01', ?5)"
        ),
        params![
            event_id.as_slice(),
            tagged_id(1, 1).as_slice(),
            i64::from(accept_seq),
            domain.as_bytes().as_slice(),
            digest(accept_seq).as_slice(),
        ],
    )?;
    connection
        .execute(
            concat!(
                "INSERT INTO claim (claim_id, assertion_event_id, subject_entity_id, predicate_id, ",
                "scope_id, object_kind, object_entity_id, authority_class, epistemic_status, valid_from) ",
                "VALUES (?1, ?2, ?3, 'graph.related', ?4, 'ENTITY', ?5, ",
                "'DIRECT_OBSERVATION', 'CODE_OBSERVED', 0)"
            ),
            params![
                id(claim_seed).as_slice(),
                event_id.as_slice(),
                id(source_seed).as_slice(),
                tagged_id(2, 1).as_slice(),
                id(target_seed).as_slice(),
            ],
        )?;
    Ok(())
}

fn seed_outbox(database: &Path, outbox_seq: u8, accept_start: u8, accept_end: u8) -> TestResult {
    let connection = Connection::open(database)?;
    connection.execute(
        concat!(
            "INSERT INTO projection_outbox (outbox_seq, accepted_batch_id, accept_seq_start, ",
            "accept_seq_end, canonical_revision, event_kind_mask, payload_digest, created_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?1, zeroblob(8), ?5, 0)"
        ),
        params![
            i64::from(outbox_seq),
            tagged_id(1, 1).as_slice(),
            i64::from(accept_start),
            i64::from(accept_end),
            digest(outbox_seq).as_slice(),
        ],
    )?;
    Ok(())
}

fn domain(seed: u8) -> TestResult<DomainId> {
    Ok(uuid_text(id(seed)).parse()?)
}

fn entity(seed: u8) -> TestResult<EntityId> {
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

fn tagged_id(tag: u8, seed: u8) -> [u8; 16] {
    let mut bytes = id(seed);
    bytes[0] = tag;
    bytes
}

fn digest(seed: u8) -> [u8; 32] {
    [seed; 32]
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
