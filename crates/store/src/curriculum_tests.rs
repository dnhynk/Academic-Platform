//! Named acceptance evidence for migration 0014's curriculum tables.
//!
//! The type half of `P2-U1` is in `crates/curriculum`, where the aggregates and
//! the four relations are. What that crate cannot observe is a writer that
//! skips it: it has no `academic-store` edge at all, so every row in these
//! tables was written by something outside its boundary. The section 9
//! boundaries, the shape of a retirement with no replacement, and the atomicity
//! of one publication therefore each need a second enforcement layer that does
//! not depend on the Rust boundary having been used.
//!
//! The base is the one `aggregate_closure_tests` builds — `0001`, the real
//! `0003`, then the aggregate migrations through `0014` — so these rows run in
//! both lanes and against the real schema rather than something resembling it.

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, ContentDigest, CourseRevisionId, CurriculumVersionId, DomainId, Event, EventPayload,
    OfferingId, ScopeDescriptor, ScopeId, TimestampMillis, ValidInterval,
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::{MIGRATION_0014_SQL, apply_aggregate_migration_pre_listen},
    repository::ClosureWriter,
};

static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Every table migration 0014 creates, enumerated in the order it creates them.
///
/// Enumerated rather than counted, and read back out of the migration text by
/// `the_table_list_is_the_migrations_own` so a table added there without a row
/// here fails rather than going unexamined by every test below.
const CURRICULUM_TABLES: [&str; 15] = [
    "curriculum_version_detail",
    "curriculum_transition_arrangement",
    "course",
    "course_revision_detail",
    "course_revision_official_prerequisite",
    "course_revision_recommended_prerequisite",
    "course_revision_designed_coverage",
    "offering_detail",
    "offering_instructor",
    "offering_meeting",
    "offering_reference",
    "course_identity_decision",
    "course_equivalence",
    "course_replacement",
    "course_retirement",
];

/// The four relation tables and the whole column set each is allowed.
///
/// Compared as whole sets. A column added to any of the four fails here as an
/// extra entry, which is what keeps `course_retirement` from growing a
/// replacement and `course_replacement` from growing a verdict.
const RELATION_COLUMNS: [(&str, &[&str]); 4] = [
    (
        "course_identity_decision",
        &[
            "earlier_course_id",
            "later_course_id",
            "verdict",
            "decision_id",
            "valid_from",
            "valid_to",
        ],
    ),
    (
        "course_equivalence",
        &[
            "source_course_id",
            "target_course_id",
            "valid_from",
            "valid_to",
        ],
    ),
    (
        "course_replacement",
        &[
            "retired_course_id",
            "replacement_course_id",
            "valid_from",
            "valid_to",
        ],
    ),
    (
        "course_retirement",
        &["course_id", "valid_from", "valid_to"],
    ),
];

struct MigratedDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl MigratedDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-0014-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let database = Self {
            path: root.join("curriculum.sqlite3"),
            root,
        };
        let mut connection = database.open()?;
        apply_schema_two_canonical_core(&connection)?;
        apply_aggregate_migration_pre_listen(&mut connection)?;
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

impl Drop for MigratedDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

fn synthetic_id(suffix: u32) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    bytes[0..8].copy_from_slice(&[0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00]);
    bytes[8] = 0x80;
    bytes[12..16].copy_from_slice(&suffix.to_be_bytes());
    bytes
}

fn record_digest(label: &str) -> [u8; 32] {
    ContentDigest::sha256(label.as_bytes())
        .as_bytes()
        .to_owned()
}

/// `crates/store/src/requirement_tests.rs` seeds its own registration batch
/// through this rather than copying it: one batch-row shape, one place.
pub(crate) fn seed_batch(
    transaction: &Transaction<'_>,
    batch_id: &[u8; 16],
    event_count: u64,
) -> Result<(), Box<dyn Error>> {
    transaction.execute(
        concat!(
            "INSERT INTO ledger_batch (batch_id, signed_envelope, envelope_hash, ",
            "deterministic_payload, deterministic_payload_hash, signing_public_key, ",
            "signature, device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
            "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, ",
            "accepted_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, NULL, 100, 3, 1, ?9, 100)"
        ),
        params![
            batch_id.to_vec(),
            vec![0x22_u8; 8],
            vec![0x11_u8; 32],
            vec![0x23_u8; 8],
            vec![0x12_u8; 32],
            vec![0x13_u8; 32],
            vec![0x33_u8; 64],
            synthetic_id(0x9001).to_vec(),
            i64::try_from(event_count)?,
        ],
    )?;
    Ok(())
}

/// The registered aggregates every typed row below hangs from.
struct Registered {
    version: CurriculumVersionId,
    revision: CourseRevisionId,
    offering: OfferingId,
    version_event: [u8; 16],
}

/// Registers one curriculum version, one revision under it, and one offering
/// under that, through the real acceptance closure writer.
fn register(connection: &mut Connection) -> Result<Registered, Box<dyn Error>> {
    let domain_id: DomainId = typed_id(0x0001)?;
    let scope_id: ScopeId = typed_id(0x0002)?;
    let version: CurriculumVersionId = typed_id(0x0100)?;
    let revision: CourseRevisionId = typed_id(0x0200)?;
    let offering: OfferingId = typed_id(0x0300)?;
    let actor = Actor::Importer {
        name: "academic.u1.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let interval = ValidInterval::open_ended(TimestampMillis::new(100));
    let version_event = synthetic_id(0x0401);

    let events = vec![
        Event {
            id: typed_id(0x0400)?,
            origin_seq: 1,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::ScopeRegistered(ScopeDescriptor {
                id: scope_id,
                domain_id,
                label: "synthetic U1 scope".to_owned(),
            }),
        },
        Event {
            id: typed_id(0x0401)?,
            origin_seq: 2,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::CurriculumVersionPublished(
                academic_domain::CurriculumVersionRegistration {
                    id: version,
                    domain_id,
                    scope_id,
                    source_digest: Some(ContentDigest::from_sha256_bytes(record_digest("version"))),
                    valid_time: interval,
                },
            ),
        },
        Event {
            id: typed_id(0x0402)?,
            origin_seq: 3,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::CourseRevisionPublished(
                academic_domain::CourseRevisionRegistration {
                    id: revision,
                    curriculum_version_id: version,
                    domain_id,
                    scope_id,
                    source_digest: Some(ContentDigest::from_sha256_bytes(record_digest(
                        "revision",
                    ))),
                    valid_time: interval,
                },
            ),
        },
        Event {
            id: typed_id(0x0403)?,
            origin_seq: 4,
            origin_observed_at: TimestampMillis::new(100),
            actor,
            domain_id,
            payload: EventPayload::OfferingObserved(academic_domain::OfferingRegistration {
                id: offering,
                course_revision_id: revision,
                domain_id,
                scope_id,
                source_digest: Some(ContentDigest::from_sha256_bytes(record_digest("offering"))),
                valid_time: interval,
            }),
        },
    ];

    let batch = academic_domain::UnsignedBatch {
        schema_version: academic_domain::EVENT_SCHEMA_VERSION,
        batch_id: typed_id(0x0500)?,
        device_id: typed_id(0x0501)?,
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
        u64::try_from(batch.events.len())?,
    )?;
    {
        let receipts =
            std::collections::BTreeMap::<_, academic_vault::SealedObjectCapability>::new();
        let mut closure = ClosureWriter::new(&transaction, &batch, &receipts);
        for (index, event) in batch.events.iter().enumerate() {
            closure.append_event(event, u64::try_from(index)? + 1)?;
        }
    }
    transaction.commit()?;
    Ok(Registered {
        version,
        revision,
        offering,
        version_event,
    })
}

fn migrated(label: &str) -> Result<(MigratedDatabase, Connection, Registered), Box<dyn Error>> {
    let database = MigratedDatabase::new(label)?;
    let mut connection = database.open()?;
    let registered = register(&mut connection)?;
    Ok((database, connection, registered))
}

/// Column names of one table, from `pragma_table_info`.
fn columns_of(connection: &Connection, table: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut statement = connection.prepare("SELECT name FROM pragma_table_info(?1)")?;
    let found = statement
        .query_map([table], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(found)
}

/// How many rows every curriculum table holds, keyed by table.
fn row_counts(connection: &Connection) -> Result<Vec<(&'static str, i64)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for table in CURRICULUM_TABLES {
        // The table name comes from the constant above, never from a value.
        let count: i64 =
            connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;
        found.push((table, count));
    }
    Ok(found)
}

/// Writes one whole publication into an open transaction, failing at `stop`.
///
/// `stop` is the ordinal of the insert that is refused. It is the same shape as
/// `academic_curriculum::PublishCheckpoint`: a failure that arrives after
/// writing has already started.
fn write_publication(
    transaction: &Transaction<'_>,
    registered: &Registered,
    stop: usize,
) -> Result<(), Box<dyn Error>> {
    let first_course = synthetic_id(0x0600);
    let second_course = synthetic_id(0x0601);
    let mut ordinal = 0_usize;
    macro_rules! step {
        ($sql:expr, $params:expr) => {{
            ordinal += 1;
            if ordinal == stop {
                return Err(format!("injected failure at insert {ordinal}").into());
            }
            transaction.execute($sql, $params)?;
        }};
    }

    step!(
        concat!(
            "INSERT INTO curriculum_version_detail (curriculum_version_id, institution_path, ",
            "admission_year_from, admission_year_to, publication_status, ",
            "supersedes_curriculum_version_id) VALUES (?1, ?2, ?3, ?4, ?5, NULL)"
        ),
        params![
            registered.version.as_bytes().to_vec(),
            "SNU/CollegeOfEngineering/CSE",
            "2026",
            "2026",
            "OFFICIAL_CONFIRMED"
        ]
    );
    step!(
        concat!(
            "INSERT INTO curriculum_transition_arrangement (curriculum_version_id, ",
            "admission_cohort, disposition, valid_from, valid_to) VALUES (?1, ?2, ?3, 100, NULL)"
        ),
        params![registered.version.as_bytes().to_vec(), "2025", "STAYS"]
    );
    for (course, code) in [
        (first_course, "M1522.001800"),
        (second_course, "M1522.001900"),
    ] {
        step!(
            concat!(
                "INSERT INTO course (course_id, course_code, canonical_identity, ",
                "introduced_by_version_id, registered_event_id) VALUES (?1, ?2, ?3, ?4, ?5)"
            ),
            params![
                course.to_vec(),
                code,
                synthetic_id(0x0700).to_vec(),
                registered.version.as_bytes().to_vec(),
                registered.version_event.to_vec()
            ]
        );
    }
    step!(
        concat!(
            "INSERT INTO course_revision_detail (course_revision_id, course_id, course_code, ",
            "title, credits, curriculum_category) VALUES (?1, ?2, ?3, ?4, 3, 'UNKNOWN')"
        ),
        params![
            registered.revision.as_bytes().to_vec(),
            first_course.to_vec(),
            "M1522.001800",
            "데이터베이스"
        ]
    );
    step!(
        concat!(
            "INSERT INTO course_revision_official_prerequisite (course_revision_id, ordinal, ",
            "prerequisite_course_id) VALUES (?1, 0, ?2)"
        ),
        params![
            registered.revision.as_bytes().to_vec(),
            second_course.to_vec()
        ]
    );
    step!(
        concat!(
            "INSERT INTO course_revision_recommended_prerequisite (course_revision_id, ordinal, ",
            "prerequisite_course_id) VALUES (?1, 0, ?2)"
        ),
        params![
            registered.revision.as_bytes().to_vec(),
            second_course.to_vec()
        ]
    );
    step!(
        concat!(
            "INSERT INTO course_revision_designed_coverage (course_revision_id, coverage_kind, ",
            "ordinal, entity_id) VALUES (?1, 'CONCEPT', 0, ?2)"
        ),
        params![
            registered.revision.as_bytes().to_vec(),
            synthetic_id(0x0701).to_vec()
        ]
    );
    step!(
        concat!(
            "INSERT INTO offering_detail (offering_id, term, section, capacity, grading_mode, ",
            "syllabus_artifact_id, official_status, observed_at) ",
            "VALUES (?1, '2026_FALL', '001', 40, 'LETTER', NULL, 'CONFIRMED', 100)"
        ),
        params![registered.offering.as_bytes().to_vec()]
    );
    step!(
        concat!(
            "INSERT INTO offering_instructor (offering_id, ordinal, instructor_name) ",
            "VALUES (?1, 0, 'Instructor')"
        ),
        params![registered.offering.as_bytes().to_vec()]
    );
    step!(
        concat!(
            "INSERT INTO offering_meeting (offering_id, ordinal, weekday, from_minute, to_minute) ",
            "VALUES (?1, 0, 'MONDAY', 540, 615)"
        ),
        params![registered.offering.as_bytes().to_vec()]
    );
    step!(
        concat!(
            "INSERT INTO offering_reference (offering_id, reference_kind, ordinal, entity_id) ",
            "VALUES (?1, 'LECTURE', 0, ?2)"
        ),
        params![
            registered.offering.as_bytes().to_vec(),
            synthetic_id(0x0702).to_vec()
        ]
    );
    step!(
        concat!(
            "INSERT INTO course_identity_decision (earlier_course_id, later_course_id, verdict, ",
            "decision_id, valid_from, valid_to) VALUES (?1, ?2, 'DISTINCT', ?3, 100, NULL)"
        ),
        params![
            first_course.to_vec(),
            second_course.to_vec(),
            synthetic_id(0x0703).to_vec()
        ]
    );
    step!(
        concat!(
            "INSERT INTO course_equivalence (source_course_id, target_course_id, valid_from, ",
            "valid_to) VALUES (?1, ?2, 100, NULL)"
        ),
        params![first_course.to_vec(), second_course.to_vec()]
    );
    step!(
        concat!(
            "INSERT INTO course_replacement (retired_course_id, replacement_course_id, ",
            "valid_from, valid_to) VALUES (?1, ?2, 100, NULL)"
        ),
        params![first_course.to_vec(), second_course.to_vec()]
    );
    step!(
        "INSERT INTO course_retirement (course_id, valid_from, valid_to) VALUES (?1, 100, NULL)",
        params![first_course.to_vec()]
    );
    Ok(())
}

/// The number of inserts one whole publication makes.
fn publication_insert_count(
    connection: &mut Connection,
    registered: &Registered,
) -> Result<usize, Box<dyn Error>> {
    // Driven rather than written down: the loop below stops at the first
    // ordinal that is past the end, which is the count plus one.
    let mut stop = 1_usize;
    loop {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let outcome = write_publication(&transaction, registered, stop);
        transaction.rollback()?;
        if outcome.is_ok() {
            return Ok(stop - 1);
        }
        stop += 1;
        assert!(stop < 1000, "the publication writer does not terminate");
    }
}

/// `curriculum_publish_is_atomic_under_injected_failure`, the database half.
///
/// The Rust half is in `crates/curriculum/tests/curriculum.rs` and holds that
/// the in-memory ledger is the value it was. This half holds the same of the
/// tables, against a writer that never went through that crate: for every
/// insert in one publication, the insert is refused and the transaction rolled
/// back, and every one of migration 0014's tables is empty afterwards.
#[test]
fn curriculum_publish_is_atomic_under_injected_failure() -> Result<(), Box<dyn Error>> {
    let (_database, mut connection, registered) = migrated("atomic")?;
    let inserts = publication_insert_count(&mut connection, &registered)?;
    assert!(
        inserts >= CURRICULUM_TABLES.len(),
        "the publication does not reach every table: {inserts} inserts for {} tables",
        CURRICULUM_TABLES.len()
    );

    let empty: Vec<(&str, i64)> = CURRICULUM_TABLES
        .into_iter()
        .map(|table| (table, 0))
        .collect();
    assert_eq!(row_counts(&connection)?, empty, "the base is not empty");

    for stop in 1..=inserts {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let outcome = write_publication(&transaction, &registered, stop);
        assert!(
            outcome.is_err(),
            "insert {stop} was supposed to be refused and was not"
        );
        transaction.rollback()?;
        assert_eq!(
            row_counts(&connection)?,
            empty,
            "a publication that failed at insert {stop} left rows behind"
        );
    }

    // And the same sequence with nothing injected does write, so the emptiness
    // above is not the emptiness of a publication that could never succeed.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    write_publication(&transaction, &registered, 0)?;
    transaction.commit()?;
    let written = row_counts(&connection)?;
    let unwritten: Vec<&str> = written
        .iter()
        .filter(|(_, count)| *count == 0)
        .map(|(table, _)| *table)
        .collect();
    assert!(
        unwritten.is_empty(),
        "the uninjected publication left {unwritten:?} empty"
    );
    Ok(())
}

/// A typed row cannot exist beside a registration its event did not authorize.
#[test]
fn a_curriculum_row_is_bound_to_its_registered_event() -> Result<(), Box<dyn Error>> {
    let (_database, connection, registered) = migrated("authorized")?;

    // A course naming a version that exists, but citing an event that did not
    // register it, is refused.
    let refused = connection.execute(
        concat!(
            "INSERT INTO course (course_id, course_code, canonical_identity, ",
            "introduced_by_version_id, registered_event_id) VALUES (?1, ?2, ?3, ?4, ?5)"
        ),
        params![
            synthetic_id(0x0800).to_vec(),
            "M1522.001800",
            synthetic_id(0x0801).to_vec(),
            registered.version.as_bytes().to_vec(),
            synthetic_id(0x0403).to_vec()
        ],
    );
    let Err(refusal) = refused else {
        return Err("an unauthorized course row was accepted".into());
    };
    let message = refusal.to_string();
    assert!(
        message.contains("course is not authorized by its introducing version"),
        "the refusal was for another reason: {message}"
    );

    // The same row citing the registering event is accepted, so the refusal is
    // about the binding rather than about the fixture.
    connection.execute(
        concat!(
            "INSERT INTO course (course_id, course_code, canonical_identity, ",
            "introduced_by_version_id, registered_event_id) VALUES (?1, ?2, ?3, ?4, ?5)"
        ),
        params![
            synthetic_id(0x0800).to_vec(),
            "M1522.001800",
            synthetic_id(0x0801).to_vec(),
            registered.version.as_bytes().to_vec(),
            registered.version_event.to_vec()
        ],
    )?;
    Ok(())
}

/// The four relations are four tables, and none has a column that derives
/// another.
///
/// Whole column sets, compared both ways. A `replacement_course_id` added to
/// `course_retirement` fails as an extra entry; a `verdict` added to
/// `course_replacement` fails the same way; and a column dropped from the
/// pinned list fails as a missing one.
#[test]
fn no_relation_table_carries_another_relations_column() -> Result<(), Box<dyn Error>> {
    let (_database, connection, _registered) = migrated("relations")?;
    for (table, expected) in RELATION_COLUMNS {
        let found = columns_of(&connection, table)?;
        let wanted: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(found, wanted, "{table}'s column set changed");
    }

    // The three names that would collapse the four into fewer, stated as names
    // rather than inferred from the sets above.
    let retirement = columns_of(&connection, "course_retirement")?;
    for forbidden in [
        "replacement_course_id",
        "replacement",
        "verdict",
        "target_course_id",
    ] {
        assert!(
            !retirement.contains(forbidden),
            "course_retirement carries {forbidden}, so a retirement implies a replacement"
        );
    }
    let replacement = columns_of(&connection, "course_replacement")?;
    for forbidden in ["verdict", "decision_id", "identity"] {
        assert!(
            !replacement.contains(forbidden),
            "course_replacement carries {forbidden}, so a replacement implies an identity"
        );
    }

    // `UNKNOWN` is the absence of a row on every relation table, so it is in no
    // CHECK list on any of them.
    let mut statement = connection.prepare(
        "SELECT name, sql FROM sqlite_schema WHERE type = 'table' AND name IN \
         ('course_identity_decision', 'course_equivalence', 'course_replacement', \
          'course_retirement', 'curriculum_transition_arrangement')",
    )?;
    let definitions = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(definitions.len(), 5, "a relation table is missing");
    for (name, sql) in definitions {
        assert!(
            !sql.contains("'UNKNOWN'"),
            "{name} admits UNKNOWN, which is the absence of a row rather than a value"
        );
    }
    Ok(())
}

/// Migration 0014's own table list, read out of the migration.
#[test]
fn the_table_list_is_the_migrations_own() -> Result<(), Box<dyn Error>> {
    let created: Vec<&str> = MIGRATION_0014_SQL
        .lines()
        .filter_map(|line| line.strip_prefix("CREATE TABLE "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();
    assert_eq!(
        created,
        CURRICULUM_TABLES.to_vec(),
        "the migration creates a different table list than this file walks"
    );

    // Every table it creates carries the append-only trigger pair.
    let (_database, connection, _registered) = migrated("tables")?;
    let mut statement = connection.prepare(
        "SELECT tbl_name FROM sqlite_schema WHERE type = 'trigger' AND name GLOB 'guard_*_update'",
    )?;
    let guarded = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    drop(statement);
    for table in CURRICULUM_TABLES {
        assert!(
            guarded.contains(table),
            "{table} has no append-only guard trigger pair"
        );
    }
    Ok(())
}

/// No offering table holds session text, and no revision table holds section
/// reality.
///
/// The section 9 boundaries as a fact about the schema. The Rust half is three
/// compile-fail cases in `crates/curriculum`; this half runs against a writer
/// that did not go through that crate.
#[test]
fn the_section_nine_boundaries_hold_in_the_schema() -> Result<(), Box<dyn Error>> {
    let (_database, connection, _registered) = migrated("boundaries")?;

    // A `Course` holds no offering reality.
    let course = columns_of(&connection, "course")?;
    for forbidden in [
        "instructor",
        "instructor_name",
        "term",
        "section",
        "capacity",
        "grading_mode",
        "syllabus_artifact_id",
        "observed_at",
        "official_status",
    ] {
        assert!(
            !course.contains(forbidden),
            "course carries the offering field {forbidden}"
        );
    }

    // A `CourseRevision` holds no section reality. The forbidden names are the
    // columns migration 0014 gives the offering tables, read back out of the
    // schema rather than written down twice, less the identifier the two
    // legitimately share.
    let mut revision_side = BTreeSet::new();
    for table in [
        "course_revision_detail",
        "course_revision_official_prerequisite",
        "course_revision_recommended_prerequisite",
        "course_revision_designed_coverage",
    ] {
        revision_side.extend(columns_of(&connection, table)?);
    }
    let mut offering_side = BTreeSet::new();
    for table in [
        "offering_detail",
        "offering_instructor",
        "offering_meeting",
        "offering_reference",
    ] {
        offering_side.extend(columns_of(&connection, table)?);
    }
    // The two sides share exactly two column names, and neither is a fact: an
    // `ordinal` is a position in a list and an `entity_id` is an opaque
    // reference. Every column that carries section 8.2 content is on one side
    // or the other, never both, so a section-reality column added to a revision
    // table fails here as a third shared name.
    let shared: Vec<&String> = revision_side.intersection(&offering_side).collect();
    assert_eq!(
        shared,
        vec![&"entity_id".to_owned(), &"ordinal".to_owned()],
        "the revision tables and the offering tables share a column that carries a fact"
    );

    // A `CourseOffering` holds no per-session utterance. Its four reference
    // kinds are identifiers; there is no text column on any offering table
    // besides the term, the section, the instructor name and the reference
    // kind, and each of those is section 8.2's own field.
    let mut text_columns: Vec<(String, String)> = Vec::new();
    for table in [
        "offering_detail",
        "offering_instructor",
        "offering_meeting",
        "offering_reference",
    ] {
        let mut statement = connection
            .prepare("SELECT name, type FROM pragma_table_info(?1) WHERE type = 'TEXT'")?;
        for row in statement.query_map([table], |row| {
            Ok((table.to_owned(), row.get::<_, String>(0)?))
        })? {
            let (owner, column) = row?;
            text_columns.push((owner, column));
        }
    }
    text_columns.sort();
    assert_eq!(
        text_columns,
        vec![
            ("offering_detail".to_owned(), "grading_mode".to_owned()),
            ("offering_detail".to_owned(), "official_status".to_owned()),
            ("offering_detail".to_owned(), "section".to_owned()),
            ("offering_detail".to_owned(), "term".to_owned()),
            (
                "offering_instructor".to_owned(),
                "instructor_name".to_owned()
            ),
            ("offering_meeting".to_owned(), "weekday".to_owned()),
            ("offering_reference".to_owned(), "reference_kind".to_owned()),
        ],
        "an offering table gained a text column; a transcript is text"
    );
    Ok(())
}
