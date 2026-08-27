use std::{
    env,
    error::Error,
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread,
};

use academic_domain::{ContentDigest, DomainId, EntityId};
use academic_projections::{
    generation::{GenerationState, ProjectionAvailability, ProjectionKind},
    query::ProjectionReader,
    runner::{
        ProjectionError, ProjectionFaultInjector, ProjectionFaultPoint, ProjectionResult,
        ProjectionRunner,
    },
};
use academic_store::{connection, migration};
use rusqlite::{Connection, params};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const CHILD_ENV: &str = "ACADEMIC_PROJECTION_FAULT_CHILD";
const CANONICAL_ENV: &str = "ACADEMIC_PROJECTION_FAULT_CANONICAL";
const SIDECAR_ENV: &str = "ACADEMIC_PROJECTION_FAULT_SIDECAR";
const DOMAIN_ENV: &str = "ACADEMIC_PROJECTION_FAULT_DOMAIN";

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
            "academic-projections-generation-{label}-{}-{sequence}",
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
        let reader = connection::open_reader(&self.canonical)?;
        Ok(ProjectionRunner::open(
            &reader,
            &self.sidecar,
            ContentDigest::sha256(b"generation-test-builder"),
            ContentDigest::sha256(b"generation-test-config"),
        )?)
    }

    fn projection_reader(&self) -> TestResult<ProjectionReader> {
        let reader = connection::open_reader(&self.canonical)?;
        Ok(ProjectionReader::new(&reader, &self.sidecar))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

#[derive(Debug)]
struct ErrorFault(ProjectionFaultPoint);

impl ProjectionFaultInjector for ErrorFault {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == self.0 {
            Err(ProjectionError::InjectedFault(point))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
struct ExitFault(ProjectionFaultPoint);

impl ProjectionFaultInjector for ExitFault {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == self.0 {
            std::process::exit(fault_exit_code(point));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct BlockingChecksumFault {
    reached: SyncSender<()>,
    release: Receiver<()>,
}

impl ProjectionFaultInjector for BlockingChecksumFault {
    fn hit(&self, point: ProjectionFaultPoint) -> ProjectionResult<()> {
        if point == ProjectionFaultPoint::Pr02AfterChecksum {
            self.reached.send(()).map_err(|_| {
                ProjectionError::Corrupt("stale-build test coordinator disappeared".to_owned())
            })?;
            self.release.recv().map_err(|_| {
                ProjectionError::Corrupt("stale-build test release disappeared".to_owned())
            })?;
        }
        Ok(())
    }
}

#[test]
fn outbox_rebuild_reaches_source_watermark() -> TestResult {
    let fixture = Fixture::new("watermark")?;
    let domain = domain(1)?;
    seed_graph_claim(&fixture.canonical, 1, 11, 21, 31, domain, &[41, 42])?;
    seed_graph_claim(&fixture.canonical, 2, 12, 21, 32, domain, &[43])?;
    seed_outbox(&fixture.canonical, 1, 1, 2)?;

    let receipt = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Graph, domain)?;
    assert!(receipt.activated);
    assert_eq!(receipt.metadata.source_accept_seq, 2);
    assert_eq!(receipt.metadata.source_outbox_seq, 1);
    assert_eq!(receipt.metadata.record_count, Some(2));
    assert_eq!(receipt.metadata.state, GenerationState::Verified);

    let page = fixture
        .projection_reader()?
        .graph_neighbors(domain, entity(21)?)?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { .. }
    ));
    assert_eq!(page.records.len(), 2);
    assert_eq!(page.records[0].source_watermark, 2);
    assert!(page.records.iter().any(|edge| edge.evidence_ids.len() == 2));
    Ok(())
}

#[test]
fn failed_generation_is_never_active() -> TestResult {
    let fixture = Fixture::new("failed")?;
    let domain = domain(2)?;
    seed_graph_claim(&fixture.canonical, 1, 13, 22, 33, domain, &[44])?;
    seed_graph_claim(&fixture.canonical, 2, 14, 22, 34, domain, &[45])?;
    seed_outbox(&fixture.canonical, 1, 1, 2)?;

    let result = fixture.runner()?.rebuild_latest_with_faults(
        ProjectionKind::Graph,
        domain,
        &ErrorFault(ProjectionFaultPoint::Pr01MidWrite),
    );
    let error = match result {
        Err(error) => error,
        Ok(receipt) => {
            return Err(io::Error::other(format!(
                "PR01 error injection unexpectedly built {receipt:?}"
            ))
            .into());
        }
    };
    assert!(matches!(
        error,
        ProjectionError::InjectedFault(ProjectionFaultPoint::Pr01MidWrite)
    ));

    let sidecar = Connection::open(&fixture.sidecar)?;
    let failed = sidecar.query_row(
        "SELECT count(*) FROM projection_generation WHERE state = 'FAILED'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let active = sidecar.query_row("SELECT count(*) FROM projection_active", [], |row| {
        row.get::<_, i64>(0)
    })?;
    assert_eq!(failed, 1);
    assert_eq!(active, 0);
    drop(sidecar);

    let availability = fixture
        .projection_reader()?
        .availability(ProjectionKind::Graph, domain)?;
    assert!(matches!(
        availability,
        ProjectionAvailability::NoActive { .. }
    ));
    Ok(())
}

#[test]
fn generation_switch_is_atomic() -> TestResult {
    for point in [
        ProjectionFaultPoint::Pr01MidWrite,
        ProjectionFaultPoint::Pr02AfterChecksum,
        ProjectionFaultPoint::Pr03DuringActivation,
    ] {
        assert_process_crash_preserves_atomic_authority(point)?;
    }
    assert_stale_generation_cannot_regress_authority()?;
    Ok(())
}

#[test]
fn projection_fault_child() -> TestResult {
    let Ok(point) = env::var(CHILD_ENV) else {
        return Ok(());
    };
    let point = parse_fault(&point)?;
    let canonical = PathBuf::from(required_env_os(CANONICAL_ENV)?);
    let sidecar = PathBuf::from(required_env_os(SIDECAR_ENV)?);
    let domain: DomainId = env::var(DOMAIN_ENV)?.parse()?;
    let reader = connection::open_reader(&canonical)?;
    let runner = ProjectionRunner::open(
        &reader,
        sidecar,
        ContentDigest::sha256(b"generation-test-builder"),
        ContentDigest::sha256(b"generation-test-config"),
    )?;
    let result =
        runner.rebuild_latest_with_faults(ProjectionKind::Graph, domain, &ExitFault(point));
    Err(io::Error::other(format!("child failpoint did not exit: {result:?}")).into())
}

fn assert_process_crash_preserves_atomic_authority(point: ProjectionFaultPoint) -> TestResult {
    let fixture = Fixture::new(point.as_str())?;
    let domain = domain(3)?;
    seed_graph_claim(&fixture.canonical, 1, 15, 23, 35, domain, &[46])?;
    seed_graph_claim(&fixture.canonical, 2, 16, 23, 36, domain, &[47])?;
    seed_outbox(&fixture.canonical, 1, 1, 2)?;
    let original = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Graph, domain)?;
    seed_graph_claim(&fixture.canonical, 3, 17, 23, 37, domain, &[48])?;
    seed_outbox(&fixture.canonical, 2, 3, 3)?;

    let status = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("projection_fault_child")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ENV, point.as_str())
        .env(CANONICAL_ENV, &fixture.canonical)
        .env(SIDECAR_ENV, &fixture.sidecar)
        .env(DOMAIN_ENV, domain.to_string())
        .status()?;
    assert_eq!(status.code(), Some(fault_exit_code(point)));

    let sidecar = Connection::open(&fixture.sidecar)?;
    let authority = sidecar.query_row(
        concat!(
            "SELECT a.generation_id, a.source_accept_seq, a.source_outbox_seq, ",
            "c.source_accept_seq, c.last_outbox_seq FROM projection_active a ",
            "JOIN projection_cursor c ON c.projection_kind = a.projection_kind ",
            "AND c.security_domain = a.security_domain ",
            "WHERE a.projection_kind = ?1 AND a.security_domain = ?2"
        ),
        params![ProjectionKind::Graph.as_str(), domain.as_bytes().as_slice()],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
    )?;
    assert_eq!(authority.0, original.metadata.generation_id.as_bytes());
    assert_eq!((authority.1, authority.2), (2, 1));
    assert_eq!((authority.3, authority.4), (2, 1));
    let verified_inactive = sidecar.query_row(
        concat!(
            "SELECT count(*) FROM projection_generation g ",
            "WHERE g.state = 'VERIFIED' AND g.generation_id <> ?1"
        ),
        [original.metadata.generation_id.as_bytes().as_slice()],
        |row| row.get::<_, i64>(0),
    )?;
    match point {
        ProjectionFaultPoint::Pr01MidWrite => assert_eq!(verified_inactive, 0),
        ProjectionFaultPoint::Pr02AfterChecksum | ProjectionFaultPoint::Pr03DuringActivation => {
            assert_eq!(verified_inactive, 1);
        }
    }
    drop(sidecar);

    let page = fixture
        .projection_reader()?
        .graph_neighbors(domain, entity(23)?)?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Lagging {
            latest_source_accept_seq: 3,
            latest_source_outbox_seq: 2,
            ..
        }
    ));
    assert_eq!(page.records.len(), 2);
    assert!(page.records.iter().all(|edge| edge.source_watermark == 2));
    Ok(())
}

fn assert_stale_generation_cannot_regress_authority() -> TestResult {
    let fixture = Fixture::new("stale-generation")?;
    let domain = domain(4)?;
    seed_graph_claim(&fixture.canonical, 1, 18, 24, 38, domain, &[49])?;
    seed_outbox(&fixture.canonical, 1, 1, 1)?;

    let stale_runner = fixture.runner()?;
    let (reached_sender, reached_receiver) = sync_channel(0);
    let (release_sender, release_receiver) = sync_channel(0);
    let stale_thread = thread::spawn(move || {
        stale_runner.rebuild_latest_with_faults(
            ProjectionKind::Graph,
            domain,
            &BlockingChecksumFault {
                reached: reached_sender,
                release: release_receiver,
            },
        )
    });
    reached_receiver.recv()?;

    seed_graph_claim(&fixture.canonical, 2, 19, 24, 39, domain, &[50])?;
    seed_outbox(&fixture.canonical, 2, 2, 2)?;
    let newer = fixture
        .runner()?
        .rebuild_latest(ProjectionKind::Graph, domain)?;
    assert!(newer.activated);
    release_sender.send(())?;
    let stale = stale_thread
        .join()
        .map_err(|_| io::Error::other("stale generation worker panicked"))??;
    assert!(!stale.activated);

    let page = fixture
        .projection_reader()?
        .graph_neighbors(domain, entity(24)?)?;
    assert!(matches!(
        page.availability,
        ProjectionAvailability::Current { ref active }
            if active.source_accept_seq == 2
                && active.source_outbox_seq == 2
                && active.generation_id == newer.metadata.generation_id
    ));
    assert_eq!(page.records.len(), 2);
    Ok(())
}

fn seed_graph_claim(
    database: &Path,
    accept_seq: u8,
    claim_seed: u8,
    source_seed: u8,
    target_seed: u8,
    domain: DomainId,
    evidence_seeds: &[u8],
) -> TestResult {
    let connection = Connection::open(database)?;
    seed_base(&connection, domain)?;
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
    let claim_id = id(claim_seed);
    connection
        .execute(
            concat!(
                "INSERT INTO claim (claim_id, assertion_event_id, subject_entity_id, predicate_id, ",
                "scope_id, object_kind, object_entity_id, authority_class, epistemic_status, valid_from) ",
                "VALUES (?1, ?2, ?3, 'graph.related', ?4, 'ENTITY', ?5, ",
                "'DIRECT_OBSERVATION', 'CODE_OBSERVED', 0)"
            ),
            params![
                claim_id.as_slice(),
                event_id.as_slice(),
                id(source_seed).as_slice(),
                tagged_id(2, 1).as_slice(),
                id(target_seed).as_slice(),
            ],
        )?;
    for (ordinal, evidence_seed) in evidence_seeds.iter().enumerate() {
        seed_evidence(&connection, domain, *evidence_seed)?;
        connection
            .execute(
                "INSERT INTO claim_evidence (claim_id, evidence_id, evidence_ordinal) VALUES (?1, ?2, ?3)",
                params![
                    claim_id.as_slice(),
                    id(*evidence_seed).as_slice(),
                    i64::try_from(ordinal)?,
                ],
            )?;
    }
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

fn seed_base(connection: &Connection, domain: DomainId) -> TestResult {
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
            "INSERT OR IGNORE INTO ledger_event (event_id, batch_id, origin_seq, ",
            "origin_observed_at, accept_seq, actor_kind, actor_canonical, domain_id, ",
            "event_kind, canonical_payload, payload_hash) VALUES ",
            "(?1, ?2, 4000, 0, 4000, 'USER', x'01', ?3, 'SCOPE_REGISTERED', x'01', ?4)"
        ),
        params![
            tagged_id(2, 2).as_slice(),
            tagged_id(1, 1).as_slice(),
            domain.as_bytes().as_slice(),
            digest(200).as_slice(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT OR IGNORE INTO scope (scope_id, created_event_id, domain_id, label) ",
            "VALUES (?1, ?2, ?3, 'projection fixture')"
        ),
        params![
            tagged_id(2, 1).as_slice(),
            tagged_id(2, 2).as_slice(),
            domain.as_bytes().as_slice(),
        ],
    )?;
    Ok(())
}

fn seed_evidence(connection: &Connection, domain: DomainId, evidence_seed: u8) -> TestResult {
    let artifact_event = tagged_id(4, evidence_seed);
    let evidence_event = tagged_id(5, evidence_seed);
    let artifact_id = tagged_id(6, evidence_seed);
    let artifact_origin = 1_000_i64 + i64::from(evidence_seed);
    let evidence_origin = 2_000_i64 + i64::from(evidence_seed);
    connection.execute(
        concat!(
            "INSERT INTO ledger_event (event_id, batch_id, origin_seq, origin_observed_at, ",
            "accept_seq, actor_kind, actor_canonical, domain_id, event_kind, ",
            "canonical_payload, payload_hash) VALUES ",
            "(?1, ?2, ?3, 0, ?3, 'USER', x'01', ?4, 'ARTIFACT_REGISTERED', x'01', ?5)"
        ),
        params![
            artifact_event.as_slice(),
            tagged_id(1, 1).as_slice(),
            artifact_origin,
            domain.as_bytes().as_slice(),
            digest(evidence_seed.wrapping_add(80)).as_slice(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT INTO artifact_descriptor (artifact_id, registered_event_id, content_digest, ",
            "media_type, byte_length, domain_id, confidentiality, retention_class, ",
            "permission_lineage_id, format_version, vault_locator) VALUES ",
            "(?1, ?2, ?3, 'text/plain', 1, ?4, 'PUBLIC', 'EPHEMERAL', ?5, 1, ?6)"
        ),
        params![
            artifact_id.as_slice(),
            artifact_event.as_slice(),
            digest(evidence_seed.wrapping_add(90)).as_slice(),
            domain.as_bytes().as_slice(),
            tagged_id(7, evidence_seed).as_slice(),
            digest(evidence_seed.wrapping_add(100)).as_slice(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT INTO artifact_representation (artifact_id, representation_index, ",
            "locator_kind, locator_payload, content_digest, byte_length) ",
            "VALUES (?1, 0, 'TEXT_BYTES', x'0001', ?2, 1)"
        ),
        params![
            artifact_id.as_slice(),
            digest(evidence_seed.wrapping_add(110)).as_slice(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT INTO ledger_event (event_id, batch_id, origin_seq, origin_observed_at, ",
            "accept_seq, actor_kind, actor_canonical, domain_id, event_kind, ",
            "canonical_payload, payload_hash) VALUES ",
            "(?1, ?2, ?3, 0, ?3, 'USER', x'01', ?4, 'EVIDENCE_REGISTERED', x'01', ?5)"
        ),
        params![
            evidence_event.as_slice(),
            tagged_id(1, 1).as_slice(),
            evidence_origin,
            domain.as_bytes().as_slice(),
            digest(evidence_seed.wrapping_add(120)).as_slice(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT INTO evidence_item (evidence_id, registered_event_id, artifact_id, ",
            "representation_index, excerpt_digest, evidence_role, evidence_strength, ",
            "extraction_method, extractor_version) VALUES ",
            "(?1, ?2, ?3, 0, ?4, 'SUPPORTS', 'DIRECT', 'fixture', '1')"
        ),
        params![
            id(evidence_seed).as_slice(),
            evidence_event.as_slice(),
            artifact_id.as_slice(),
            digest(evidence_seed.wrapping_add(130)).as_slice(),
        ],
    )?;
    Ok(())
}

fn parse_fault(value: &str) -> TestResult<ProjectionFaultPoint> {
    Ok(match value {
        "PR01" => ProjectionFaultPoint::Pr01MidWrite,
        "PR02" => ProjectionFaultPoint::Pr02AfterChecksum,
        "PR03" => ProjectionFaultPoint::Pr03DuringActivation,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown child fault point {value}"),
            )
            .into());
        }
    })
}

fn required_env_os(key: &str) -> TestResult<std::ffi::OsString> {
    env::var_os(key).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("child {key} missing")).into()
    })
}

const fn fault_exit_code(point: ProjectionFaultPoint) -> i32 {
    match point {
        ProjectionFaultPoint::Pr01MidWrite => 71,
        ProjectionFaultPoint::Pr02AfterChecksum => 72,
        ProjectionFaultPoint::Pr03DuringActivation => 73,
    }
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
