//! Named acceptance evidence for the bitemporal aggregate-set read.
//!
//! # The base these tests run on
//!
//! The aggregate closure tables belong to store schema version 2, which only
//! the encrypted lane creates. These tests therefore build that base the way
//! the migration 0004 suite does — `0001`, the real `0003`, then `0004` — and
//! read it through a plain read-only connection. The profile-admission path for
//! a schema-2 profile is the encrypted lane's and is verified there; what is
//! under test here is the coordinate SQL over real closure tables.
//!
//! # The fixture is built to make one coordinate unable to stand in for the other
//!
//! Two waves of registrations are accepted:
//!
//! - wave one, acceptance `2..=19`, valid `[100, +inf)`;
//! - wave two, acceptance `20..=37`, valid `[50, 100)`.
//!
//! Wave two is accepted *later* and applies *earlier*. A reader that confused
//! acceptance order with valid time would see wave two at the early known-at
//! coordinate or miss it at the late one, so each named test below fails if the
//! two axes are ever collapsed into one.

use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use academic_domain::{
    Actor, ContentDigest, DomainId, Event, EventId, EventPayload, ScopeDescriptor, ScopeId,
    TimestampMillis, UnsignedBatch, V3_EVENT_KINDS, ValidInterval,
    temporal::{DimensionCarrier, NAMED_TIME_TRAVEL_DIMENSIONS, TimeCoordinates},
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::{MIGRATION_0001_SQL, apply_aggregate_migration_pre_listen},
    queries::QueryError,
    repository::ClosureWriter,
    timeline::{
        AGGREGATE_TABLES, AggregateTimelineRequest, AggregateTimelineSnapshot,
        aggregate_timeline_from_connection, origin_marks_from_connection,
    },
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Acceptance sequence of the last wave-one registration.
const WAVE_ONE_HEAD: u64 = 19;
/// Acceptance sequence of the last wave-two registration.
const WAVE_TWO_HEAD: u64 = 37;

struct TimelineDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl TimelineDatabase {
    fn empty(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-timeline-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let path = root.join("timeline.sqlite3");
        Ok(Self { root, path })
    }

    /// A schema-2 canonical core carrying migration 0004.
    fn migrated(label: &str) -> Result<Self, Box<dyn Error>> {
        let database = Self::empty(label)?;
        let mut connection = database.open()?;
        apply_schema_two_canonical_core(&connection)?;
        apply_aggregate_migration_pre_listen(&mut connection)?;
        Ok(database)
    }

    /// A schema-1 canonical core, which carries no aggregate table at all.
    fn schema_one(label: &str) -> Result<Self, Box<dyn Error>> {
        let database = Self::empty(label)?;
        let connection = database.open()?;
        connection.execute_batch(MIGRATION_0001_SQL)?;
        Ok(database)
    }

    fn open(&self) -> Result<Connection, Box<dyn Error>> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(connection)
    }
}

impl Drop for TimelineDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

fn domain() -> Result<DomainId, Box<dyn Error>> {
    typed_id(0x0001)
}

fn scope() -> Result<ScopeId, Box<dyn Error>> {
    typed_id(0x0002)
}

fn actor() -> Actor {
    Actor::Importer {
        name: "academic.c6.test".to_owned(),
        version: "1.0.0".to_owned(),
    }
}

fn at(known_at_accept_seq: u64, valid_at: i64) -> TimeCoordinates {
    TimeCoordinates::new(known_at_accept_seq, TimestampMillis::new(valid_at))
}

fn request(coordinates: TimeCoordinates) -> Result<AggregateTimelineRequest, Box<dyn Error>> {
    Ok(AggregateTimelineRequest {
        domain_id: domain()?,
        coordinates,
    })
}

/// Builds the eighteen registration arms for one wave.
///
/// `id_base` keeps the two waves' aggregate identifiers disjoint, and every
/// parent reference resolves to an arm earlier in the same wave.
fn wave_payloads(
    id_base: u32,
    valid_time: ValidInterval,
) -> Result<Vec<EventPayload>, Box<dyn Error>> {
    use academic_domain::{
        AttemptRegistration, AuditRegistration, CapturePermissionRegistration, ConsentRegistration,
        CourseRevisionRegistration, CurriculumVersionRegistration, EgressDecisionRegistration,
        EntityIdentityChangeRegistration, FindingRegistration, LectureDocumentRegistration,
        LectureSessionRegistration, ModelRunRegistration, OfferingRegistration,
        ProposalDispositionRegistration, RequirementSetRegistration, RetentionActionRegistration,
        SnapshotRegistration, TranscriptVersionRegistration,
    };

    let domain_id = domain()?;
    let scope_id = scope()?;
    let digest = ContentDigest::sha256(b"SYNTHETIC C6 PROVENANCE");

    macro_rules! arm {
        ($variant:ident, $record:ident, $offset:expr $(, $parent_field:ident: $parent:expr)?) => {
            EventPayload::$variant($record {
                id: typed_id(id_base + $offset)?,
                $($parent_field: typed_id(id_base + $parent)?,)?
                domain_id,
                scope_id,
                source_digest: if $offset % 2 == 0 { Some(digest) } else { None },
                valid_time,
            })
        };
    }

    let payloads = vec![
        arm!(CurriculumVersionPublished, CurriculumVersionRegistration, 0),
        arm!(CourseRevisionPublished, CourseRevisionRegistration, 1, curriculum_version_id: 0),
        arm!(OfferingObserved, OfferingRegistration, 2, course_revision_id: 1),
        arm!(AttemptRecorded, AttemptRegistration, 3, offering_id: 2),
        arm!(RequirementSetPublished, RequirementSetRegistration, 4, curriculum_version_id: 0),
        arm!(AuditComputed, AuditRegistration, 5, requirement_set_id: 4),
        arm!(CapturePermissionRecorded, CapturePermissionRegistration, 6, offering_id: 2),
        arm!(LectureSessionRecorded, LectureSessionRegistration, 7, offering_id: 2),
        arm!(TranscriptVersionAdded, TranscriptVersionRegistration, 8, lecture_session_id: 7),
        arm!(LectureDocumentPublished, LectureDocumentRegistration, 9, lecture_session_id: 7),
        arm!(SnapshotRegistered, SnapshotRegistration, 10, repository_id: 40),
        arm!(FindingPublished, FindingRegistration, 11, snapshot_id: 10),
        arm!(ModelRunRecorded, ModelRunRegistration, 12),
        arm!(ProposalDisposed, ProposalDispositionRegistration, 13, model_run_id: 12),
        arm!(EgressDecided, EgressDecisionRegistration, 14),
        arm!(ConsentRecorded, ConsentRegistration, 15),
        arm!(EntityIdentityChanged, EntityIdentityChangeRegistration, 16, entity_id: 41),
        arm!(RetentionActionRecorded, RetentionActionRegistration, 17),
    ];
    assert_eq!(payloads.len(), V3_EVENT_KINDS.len());
    for (payload, kind) in payloads.iter().zip(V3_EVENT_KINDS) {
        assert_eq!(
            payload.kind(),
            kind,
            "arms must be built in Proto tag order"
        );
    }
    Ok(payloads)
}

/// Inserts the one `ledger_batch` row every `ledger_event` row references.
///
/// The batch material is synthetic filler that satisfies the column `CHECK`s;
/// no signing key is involved and none of these bytes is a real envelope.
fn seed_batch(
    transaction: &Transaction<'_>,
    batch_id: &[u8; 16],
    device_id: &[u8; 16],
    accept_seq_start: u64,
    accept_seq_end: u64,
    filler: u8,
) -> Result<(), Box<dyn Error>> {
    transaction.execute(
        concat!(
            "INSERT INTO ledger_batch (batch_id, signed_envelope, envelope_hash, ",
            "deterministic_payload, deterministic_payload_hash, signing_public_key, ",
            "signature, device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
            "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, ",
            "accepted_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, NULL, 100, 3, ?10, ?11, 100)"
        ),
        params![
            batch_id.to_vec(),
            vec![filler; 8],
            vec![filler.wrapping_add(0x11); 32],
            vec![filler.wrapping_add(0x22); 8],
            vec![filler.wrapping_add(0x33); 32],
            vec![filler.wrapping_add(0x44); 32],
            vec![filler.wrapping_add(0x55); 64],
            device_id.to_vec(),
            i64::try_from(accept_seq_end - accept_seq_start + 1)?,
            i64::try_from(accept_seq_start)?,
            i64::try_from(accept_seq_end)?,
        ],
    )?;
    Ok(())
}

/// Appends one wave of registrations through the real acceptance closure writer.
fn append_wave(
    connection: &mut Connection,
    batch_suffix: u32,
    id_base: u32,
    event_id_base: u32,
    accept_seq_start: u64,
    valid_time: ValidInterval,
    with_scope: bool,
) -> Result<(), Box<dyn Error>> {
    let domain_id = domain()?;
    let mut events = Vec::new();
    if with_scope {
        events.push(Event {
            id: typed_id::<EventId>(event_id_base)?,
            origin_seq: 1,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor(),
            domain_id,
            payload: EventPayload::ScopeRegistered(ScopeDescriptor {
                id: scope()?,
                domain_id,
                label: "synthetic C6 scope".to_owned(),
            }),
        });
    }
    for (offset, payload) in wave_payloads(id_base, valid_time)?.into_iter().enumerate() {
        let index = u32::try_from(offset)?;
        events.push(Event {
            id: typed_id::<EventId>(event_id_base + index + 1)?,
            origin_seq: u64::from(index) + if with_scope { 2 } else { 1 },
            origin_observed_at: TimestampMillis::new(100),
            actor: actor(),
            domain_id,
            payload,
        });
    }
    let accept_seq_end = accept_seq_start + u64::try_from(events.len())? - 1;
    let batch = UnsignedBatch {
        schema_version: academic_domain::EVENT_SCHEMA_VERSION,
        batch_id: typed_id(batch_suffix)?,
        device_id: typed_id(batch_suffix + 1)?,
        origin_seq_start: 1,
        origin_seq_end: u64::try_from(events.len())?,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(100),
        events,
    };
    batch.validate()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    seed_batch(
        &transaction,
        batch.batch_id.as_bytes(),
        batch.device_id.as_bytes(),
        accept_seq_start,
        accept_seq_end,
        u8::try_from((batch_suffix >> 8) & 0xff)?,
    )?;
    {
        // No registration arm references an artifact, so this closure needs no
        // sealed receipts; the receipt type is named only to fix the parameter.
        let receipts =
            std::collections::BTreeMap::<_, academic_vault::SealedObjectCapability>::new();
        let mut closure = ClosureWriter::new(&transaction, &batch, &receipts);
        for (index, event) in batch.events.iter().enumerate() {
            closure.append_event(event, accept_seq_start + u64::try_from(index)?)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// A migrated database carrying both waves.
fn two_wave_database(label: &str) -> Result<TimelineDatabase, Box<dyn Error>> {
    let database = TimelineDatabase::migrated(label)?;
    let mut connection = database.open()?;
    append_wave(
        &mut connection,
        0x0400,
        0x1000,
        0x2000,
        1,
        ValidInterval::open_ended(TimestampMillis::new(100)),
        true,
    )?;
    append_wave(
        &mut connection,
        0x0500,
        0x3000,
        0x4000,
        WAVE_ONE_HEAD + 1,
        ValidInterval::new(TimestampMillis::new(50), Some(TimestampMillis::new(100)))?,
        false,
    )?;
    Ok(database)
}

fn read(
    connection: &Connection,
    coordinates: TimeCoordinates,
) -> Result<AggregateTimelineSnapshot, Box<dyn Error>> {
    Ok(aggregate_timeline_from_connection(
        connection,
        &request(coordinates)?,
    )?)
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// Moving only the known-at coordinate hides knowledge accepted after it.
#[test]
fn aggregate_timeline_excludes_knowledge_accepted_later() -> Result<(), Box<dyn Error>> {
    let database = two_wave_database("known-at")?;
    let connection = database.open()?;

    // Wave two applies at 75 and was accepted at 20..=37. At the wave-one head
    // it is not yet knowledge, so the same valid instant reads empty.
    let before = read(&connection, at(WAVE_ONE_HEAD, 75))?;
    assert!(
        before.rows.is_empty(),
        "wave two must be invisible at the wave-one known-at coordinate"
    );

    let after = read(&connection, at(WAVE_TWO_HEAD, 75))?;
    assert_eq!(after.rows.len(), V3_EVENT_KINDS.len());
    assert!(
        after.rows.iter().all(|row| row.accept_seq > WAVE_ONE_HEAD),
        "only wave two applies at valid instant 75"
    );
    Ok(())
}

/// Moving only the valid-at coordinate re-reads history with current knowledge.
#[test]
fn aggregate_timeline_valid_at_reinterprets_history() -> Result<(), Box<dyn Error>> {
    let database = two_wave_database("valid-at")?;
    let connection = database.open()?;

    // One known-at coordinate — everything the replica knows — read at two
    // valid instants selects two disjoint sets.
    let early = read(&connection, at(WAVE_TWO_HEAD, 75))?;
    let late = read(&connection, at(WAVE_TWO_HEAD, 150))?;
    assert_eq!(early.rows.len(), V3_EVENT_KINDS.len());
    assert_eq!(late.rows.len(), V3_EVENT_KINDS.len());
    assert!(
        early.rows.iter().all(|row| row.accept_seq > WAVE_ONE_HEAD),
        "the earlier valid instant is covered only by the later-accepted wave"
    );
    assert!(
        late.rows.iter().all(|row| row.accept_seq <= WAVE_ONE_HEAD),
        "the later valid instant is covered only by the earlier-accepted wave"
    );
    assert_ne!(early.source_row_digest, late.source_row_digest);

    // Before either interval opens, current knowledge still says nothing applied.
    assert!(read(&connection, at(WAVE_TWO_HEAD, 10))?.rows.is_empty());
    Ok(())
}

/// Every one of the eighteen registration arms is projected, not a subset.
#[test]
fn aggregate_timeline_covers_every_v3_registration_arm() -> Result<(), Box<dyn Error>> {
    let database = two_wave_database("coverage")?;
    let connection = database.open()?;
    let snapshot = read(&connection, at(WAVE_TWO_HEAD, 150))?;
    for kind in V3_EVENT_KINDS {
        assert_eq!(
            snapshot.rows_of(kind).len(),
            1,
            "{kind} must contribute exactly one row at this coordinate"
        );
    }
    assert_eq!(snapshot.rows.len(), V3_EVENT_KINDS.len());
    Ok(())
}

/// The read registry names the same eighteen tables the migrated schema holds.
#[test]
fn aggregate_timeline_registry_matches_the_migrated_schema() -> Result<(), Box<dyn Error>> {
    let database = TimelineDatabase::migrated("registry")?;
    let connection = database.open()?;
    let kinds: Vec<&str> = AGGREGATE_TABLES
        .iter()
        .map(|descriptor| descriptor.kind)
        .collect();
    assert_eq!(kinds, V3_EVENT_KINDS.to_vec());
    for descriptor in AGGREGATE_TABLES {
        let primary_key = descriptor.resolved_primary_key_column();
        let present: i64 = connection.query_row(
            "SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2",
            params![descriptor.table, primary_key],
            |row| row.get(0),
        )?;
        assert_eq!(
            present, 1,
            "{} has no column {primary_key}",
            descriptor.table
        );
    }
    Ok(())
}

/// A profile with no aggregate tables refuses instead of reading empty.
#[test]
fn aggregate_timeline_refuses_a_profile_without_aggregate_tables() -> Result<(), Box<dyn Error>> {
    let database = TimelineDatabase::schema_one("absent")?;
    let connection = database.open()?;
    let refused = aggregate_timeline_from_connection(&connection, &request(at(0, 150))?);
    assert!(
        matches!(
            refused,
            Err(QueryError::AggregatesAbsent { missing, .. }) if missing == AGGREGATE_TABLES.len()
        ),
        "schema-1 profile must refuse the aggregate read: {refused:?}"
    );
    Ok(())
}

/// A known-at coordinate ahead of the canonical head is refused, not clamped.
#[test]
fn aggregate_timeline_refuses_a_known_at_beyond_the_head() -> Result<(), Box<dyn Error>> {
    let database = two_wave_database("beyond-head")?;
    let connection = database.open()?;
    let refused =
        aggregate_timeline_from_connection(&connection, &request(at(WAVE_TWO_HEAD + 1, 150))?);
    assert!(
        matches!(
            refused,
            Err(QueryError::KnownAtBeyondHead { requested, latest })
                if requested == WAVE_TWO_HEAD + 1 && latest == WAVE_TWO_HEAD
        ),
        "a coordinate past the head must be refused: {refused:?}"
    );
    Ok(())
}

/// Every dimension that claims an aggregate carrier reads through this registry.
#[test]
fn every_carried_dimension_resolves_to_a_projected_arm() -> Result<(), Box<dyn Error>> {
    let database = two_wave_database("carriers")?;
    let connection = database.open()?;
    let snapshot = read(&connection, at(WAVE_TWO_HEAD, 150))?;
    let mut carried = 0_usize;
    for dimension in NAMED_TIME_TRAVEL_DIMENSIONS {
        if let DimensionCarrier::Aggregate(kind) = dimension.carrier() {
            carried += 1;
            assert!(
                AGGREGATE_TABLES
                    .iter()
                    .any(|descriptor| descriptor.kind == kind),
                "{} names carrier {kind}, which the read registry does not cover",
                dimension.as_str()
            );
            assert_eq!(
                snapshot.rows_of(kind).len(),
                1,
                "{} reads no row through carrier {kind}",
                dimension.as_str()
            );
        }
    }
    assert_eq!(carried, 4, "four named dimensions have a landed carrier");
    Ok(())
}

/// An identity change in the interval is read out of the ledger as an ontology
/// change, and a profile without that lane says so instead of reporting none.
#[test]
fn origin_marks_report_an_identity_change_as_an_ontology_change() -> Result<(), Box<dyn Error>> {
    let database = two_wave_database("origin-marks")?;
    let connection = database.open()?;
    let marks = origin_marks_from_connection(&connection, domain()?, 0, WAVE_TWO_HEAD)?;

    assert!(marks.identity_lane_present);
    assert_eq!(
        marks.identity_changes.len(),
        2,
        "each wave registers one ENTITY_IDENTITY_CHANGED aggregate"
    );
    assert!(
        marks.official_corrections.is_empty(),
        "this fixture records no official supersession"
    );
    for identity_change in &marks.identity_changes {
        assert!(
            !marks.other_acceptances.contains(identity_change),
            "an acceptance carries one payload arm, so the mark sets are disjoint"
        );
    }
    let ordered = marks.ordered();
    assert_eq!(
        ordered.len(),
        marks.identity_changes.len() + marks.other_acceptances.len()
    );
    assert!(
        ordered.windows(2).all(|pair| pair[0].0 <= pair[1].0),
        "marks are returned in acceptance order"
    );
    assert_eq!(
        ordered
            .iter()
            .filter(|(_, origin)| *origin == "ONTOLOGY_CHANGE")
            .count(),
        2
    );

    let plain = TimelineDatabase::schema_one("origin-marks-schema-one")?;
    let plain_connection = plain.open()?;
    let plain_marks = origin_marks_from_connection(&plain_connection, domain()?, 0, 0)?;
    assert!(
        !plain_marks.identity_lane_present,
        "a schema-1 profile cannot record an identity change"
    );
    Ok(())
}
