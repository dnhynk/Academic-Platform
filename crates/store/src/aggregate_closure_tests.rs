//! Named acceptance evidence for migration 0004's aggregate closure tables.
//!
//! # The base these tests run on
//!
//! Migration 0004 layers on store schema version 2, so these tests build that
//! version the way a profile does: `0001` for the canonical core, then the real
//! `0003` for the schema-2 identity, then `0004`. `P2-C2` wrote them against
//! the schema-1 core as a stand-in while `P2-K2` was in flight, and
//! `migration_0003_is_absent_until_p2_k2_lands` was the machine-checked link
//! that made the substitution expire. 0003 has landed, so the substitution and
//! its guard are both gone.
//!
//! The identity row and the two SQLite identifiers are written here rather than
//! through `migrate_open_connection_pre_listen`, because that runner writes the
//! identity of whichever lane it was compiled into and these tests run in both.
//! The values are the frozen ones, and 0003's own column `CHECK`s reject the row
//! if any of them ever drifts, so this base cannot quietly stop being the real
//! one.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use academic_domain::{
    Actor, ContentDigest, DomainId, Event, EventId, EventPayload, ScopeDescriptor, ScopeId,
    TimestampMillis, V3_EVENT_KINDS, ValidInterval,
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{
    SQLITE_APPLICATION_ID,
    authorizer::{CANONICAL_TABLES, install_canonical_authorizer},
    migration::{
        MIGRATION_0001_SQL, MIGRATION_0003_SQL, MIGRATION_0004_SQL,
        apply_aggregate_migration_pre_listen,
    },
    repository::ClosureWriter,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Aggregate table, primary-key column, and parent column, in Proto tag order.
///
/// Restated here rather than imported so a silent rename in the writer cannot
/// also rename what the tests check.
const CLOSURE_TABLES: [(&str, &str, Option<&str>); 18] = [
    ("curriculum_version", "curriculum_version_id", None),
    (
        "course_revision",
        "course_revision_id",
        Some("curriculum_version_id"),
    ),
    ("offering", "offering_id", Some("course_revision_id")),
    ("attempt", "attempt_id", Some("offering_id")),
    (
        "requirement_set",
        "requirement_set_id",
        Some("curriculum_version_id"),
    ),
    ("audit", "audit_id", Some("requirement_set_id")),
    (
        "capture_permission",
        "capture_permission_id",
        Some("offering_id"),
    ),
    ("lecture_session", "lecture_session_id", Some("offering_id")),
    (
        "transcript_version",
        "transcript_version_id",
        Some("lecture_session_id"),
    ),
    (
        "lecture_document",
        "lecture_document_id",
        Some("lecture_session_id"),
    ),
    ("snapshot", "snapshot_id", Some("repository_id")),
    ("finding", "finding_id", Some("snapshot_id")),
    ("model_run", "model_run_id", None),
    ("proposal_disposition", "proposal_id", Some("model_run_id")),
    ("egress_decision", "egress_decision_id", None),
    ("consent", "consent_id", None),
    (
        "entity_identity_change",
        "entity_identity_change_id",
        Some("entity_id"),
    ),
    ("retention_action", "retention_action_id", None),
];

/// A disposable on-disk database carrying the schema-1 core plus migration 0004.
struct MigratedDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl MigratedDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let database = Self::empty(label)?;
        let mut connection = database.open()?;
        apply_schema_two_canonical_core(&connection)?;
        apply_aggregate_migration_pre_listen(&mut connection)?;
        Ok(database)
    }

    fn empty(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-0004-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let path = root.join("aggregates.sqlite3");
        Ok(Self { root, path })
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

/// Installs the canonical core that migration 0004 layers on.
///
/// See the module comment: this is 0001's canonical core, not a schema-2
/// identity. It creates no `schema_meta` row and stamps no `user_version`.
/// Builds the store schema version 2 base: `0001`, then the real `0003`.
///
/// The identity row uses the values 0003 pins. Any drift in a pinned value
/// makes 0003's own `CHECK` reject this insert, so the base cannot silently
/// stop matching the migration it is supposed to reproduce.
fn apply_schema_two_canonical_core(connection: &Connection) -> Result<(), Box<dyn Error>> {
    connection.execute_batch(MIGRATION_0001_SQL)?;
    connection.execute_batch(MIGRATION_0003_SQL)?;
    connection.execute(
        concat!(
            "INSERT INTO schema_meta (",
            "singleton, format_uuid, schema_version, schema_semver, ",
            "minimum_reader_protocol_major, minimum_reader_protocol_minor, ",
            "minimum_writer_protocol_major, minimum_writer_protocol_minor, ",
            "storage_mode, storage_encryption, creating_build_digest, created_at_unix_ms",
            ") VALUES (1, ?1, 2, '2.0.0', 2, 0, 2, 0, ?2, ?3, ?4, 1)"
        ),
        params![
            schema_two_literal("format_uuid = x'")?
                .as_bytes()
                .chunks(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair)?, 16).map_err(Into::into))
                .collect::<Result<Vec<u8>, Box<dyn Error>>>()?,
            schema_two_literal("storage_mode = '")?,
            schema_two_literal("storage_encryption = '")?,
            vec![0x24_u8; 32],
        ],
    )?;
    connection.pragma_update(None, "application_id", SQLITE_APPLICATION_ID)?;
    connection.pragma_update(None, "user_version", 2_u32)?;
    Ok(())
}

/// Extracts the single-quoted value migration 0003 pins after `prefix`.
fn schema_two_literal(prefix: &str) -> Result<String, Box<dyn Error>> {
    let start = MIGRATION_0003_SQL
        .find(prefix)
        .ok_or_else(|| format!("migration 0003 does not pin {prefix}"))?
        + prefix.len();
    let rest = &MIGRATION_0003_SQL[start..];
    let end = rest
        .find('\'')
        .ok_or_else(|| format!("migration 0003 has an unterminated literal after {prefix}"))?;
    Ok(rest[..end].to_owned())
}

fn synthetic_id(suffix: u32) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    bytes[0..8].copy_from_slice(&[0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00]);
    bytes[8] = 0x80;
    bytes[12..16].copy_from_slice(&suffix.to_be_bytes());
    bytes
}

fn typed_id<T>(suffix: u32) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let text = format!("01900000-0000-7000-8000-{suffix:012x}");
    text.parse::<T>()
        .map_err(|error| format!("{text} is not a valid identifier: {error}").into())
}

/// Inserts the one `ledger_batch` row every `ledger_event` row references.
///
/// The batch material is synthetic filler that satisfies the column CHECKs. What
/// these tests exercise is the closure writer, not signature verification, so no
/// signing key is involved and none of these bytes is a real envelope.
fn seed_batch(
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

fn column_names(connection: &Connection, table: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut statement =
        connection.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}

/// The scope plus all eighteen v3 registration arms, in Proto tag order.
///
/// Aggregate identifiers are chosen so every parent reference resolves to an
/// arm that appears earlier in the same batch.
fn eighteen_arm_events() -> Result<Vec<Event>, Box<dyn Error>> {
    let domain_id: DomainId = typed_id(0x0001)?;
    let scope_id: ScopeId = typed_id(0x0002)?;
    let actor = Actor::Importer {
        name: "academic.c2.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let interval = ValidInterval::open_ended(TimestampMillis::new(100));
    let digest = ContentDigest::sha256(b"SYNTHETIC C2 PROVENANCE");

    macro_rules! arm {
        ($variant:ident, $record:ident, $id:expr $(, $parent_field:ident: $parent:expr)?) => {
            EventPayload::$variant($record {
                id: typed_id($id)?,
                $($parent_field: typed_id($parent)?,)?
                domain_id,
                scope_id,
                // Half the arms carry a provenance digest and half do not, so the
                // nullable column is exercised in both states.
                source_digest: if $id % 2 == 0 { Some(digest) } else { None },
                valid_time: interval,
            })
        };
    }

    use academic_domain::{
        AttemptRegistration, AuditRegistration, CapturePermissionRegistration, ConsentRegistration,
        CourseRevisionRegistration, CurriculumVersionRegistration, EgressDecisionRegistration,
        EntityIdentityChangeRegistration, FindingRegistration, LectureDocumentRegistration,
        LectureSessionRegistration, ModelRunRegistration, OfferingRegistration,
        ProposalDispositionRegistration, RequirementSetRegistration, RetentionActionRegistration,
        SnapshotRegistration, TranscriptVersionRegistration,
    };

    let payloads = vec![
        arm!(
            CurriculumVersionPublished,
            CurriculumVersionRegistration,
            0x0100
        ),
        arm!(CourseRevisionPublished, CourseRevisionRegistration, 0x0101, curriculum_version_id: 0x0100),
        arm!(OfferingObserved, OfferingRegistration, 0x0102, course_revision_id: 0x0101),
        arm!(AttemptRecorded, AttemptRegistration, 0x0103, offering_id: 0x0102),
        arm!(RequirementSetPublished, RequirementSetRegistration, 0x0104, curriculum_version_id: 0x0100),
        arm!(AuditComputed, AuditRegistration, 0x0105, requirement_set_id: 0x0104),
        arm!(CapturePermissionRecorded, CapturePermissionRegistration, 0x0106, offering_id: 0x0102),
        arm!(LectureSessionRecorded, LectureSessionRegistration, 0x0107, offering_id: 0x0102),
        arm!(TranscriptVersionAdded, TranscriptVersionRegistration, 0x0108, lecture_session_id: 0x0107),
        arm!(LectureDocumentPublished, LectureDocumentRegistration, 0x0109, lecture_session_id: 0x0107),
        // `repository_id` resolves to no table: section 3.8 fixes the arm list at
        // eighteen and leaves the repository aggregate to P2-R1.
        arm!(SnapshotRegistered, SnapshotRegistration, 0x010a, repository_id: 0x0200),
        arm!(FindingPublished, FindingRegistration, 0x010b, snapshot_id: 0x010a),
        arm!(ModelRunRecorded, ModelRunRegistration, 0x010c),
        arm!(ProposalDisposed, ProposalDispositionRegistration, 0x010d, model_run_id: 0x010c),
        arm!(EgressDecided, EgressDecisionRegistration, 0x010e),
        arm!(ConsentRecorded, ConsentRegistration, 0x010f),
        // The entity registry is P2-C3's aggregate, so `entity_id` has no table.
        arm!(EntityIdentityChanged, EntityIdentityChangeRegistration, 0x0110, entity_id: 0x0201),
        arm!(RetentionActionRecorded, RetentionActionRegistration, 0x0111),
    ];
    assert_eq!(payloads.len(), V3_EVENT_KINDS.len());
    for (payload, kind) in payloads.iter().zip(V3_EVENT_KINDS) {
        assert_eq!(
            payload.kind(),
            kind,
            "arms must be built in Proto tag order"
        );
    }

    let mut events = vec![Event {
        id: typed_id::<EventId>(0x0300)?,
        origin_seq: 1,
        origin_observed_at: TimestampMillis::new(100),
        actor: actor.clone(),
        domain_id,
        payload: EventPayload::ScopeRegistered(ScopeDescriptor {
            id: scope_id,
            domain_id,
            label: "synthetic C2 scope".to_owned(),
        }),
    }];
    for (offset, payload) in payloads.into_iter().enumerate() {
        let index = u32::try_from(offset)?;
        events.push(Event {
            id: typed_id::<EventId>(0x0301 + index)?,
            origin_seq: u64::from(index) + 2,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload,
        });
    }
    for event in &events {
        event.validate()?;
    }
    Ok(events)
}

/// Names the closure table an arm's aggregate identifier actually landed in.
///
/// Read back from the migrated database rather than from the writer, so the two
/// have to agree.
fn closure_table_for(payload: &EventPayload) -> Result<&'static str, Box<dyn Error>> {
    let kind = payload.kind();
    let position = V3_EVENT_KINDS
        .iter()
        .position(|candidate| *candidate == kind)
        .ok_or_else(|| format!("{kind} is not an event schema v3 arm"))?;
    Ok(CLOSURE_TABLES[position].0)
}

/// The migration's executable statements, with its comment lines removed.
///
/// The header explains at length which identifiers migration 0004 must never
/// touch, so a naive text search would match the prose that forbids them.
fn migration_0004_statements() -> String {
    MIGRATION_0004_SQL
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

/// Writes the scope plus all eighteen arms through the real acceptance closure
/// writer, and returns the transaction so the caller decides commit or rollback.
fn append_all_arms<'connection>(
    connection: &'connection mut Connection,
    stop_after: Option<usize>,
) -> Result<(Transaction<'connection>, usize), Box<dyn Error>> {
    let events = eighteen_arm_events()?;
    let batch = academic_domain::UnsignedBatch {
        schema_version: academic_domain::EVENT_SCHEMA_VERSION,
        batch_id: typed_id(0x0400)?,
        device_id: typed_id(0x0401)?,
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
    let written = {
        // No registration arm references an artifact, so this closure needs no
        // sealed receipts; the receipt type is named only to fix the parameter.
        let receipts =
            std::collections::BTreeMap::<_, academic_vault::SealedObjectCapability>::new();
        let mut closure = ClosureWriter::new(&transaction, &batch, &receipts);
        let mut written = 0_usize;
        for (index, event) in batch.events.iter().enumerate() {
            if stop_after == Some(index) {
                break;
            }
            closure.append_event(event, u64::try_from(index)? + 1)?;
            written += 1;
        }
        written
    };
    Ok((transaction, written))
}

fn count(connection: &Connection, table: &str) -> Result<i64, Box<dyn Error>> {
    Ok(
        connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })?,
    )
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// Both append-only layers hold on every table migration 0004 adds.
#[test]
fn every_new_table_denies_update_and_delete() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("append-only")?;
    let mut connection = database.open()?;
    let (transaction, written) = append_all_arms(&mut connection, None)?;
    assert_eq!(written, 19);
    transaction.commit()?;

    for (table, key, _) in CLOSURE_TABLES {
        assert_eq!(count(&connection, table)?, 1, "{table} must hold one row");
        let update = connection.execute(
            &format!("UPDATE {table} SET domain_id = ?1"),
            params![synthetic_id(0xdead).to_vec()],
        );
        assert!(
            update
                .as_ref()
                .err()
                .is_some_and(|error| error.to_string().contains("canonical table is append-only")),
            "{table} accepted an UPDATE: {update:?}"
        );
        let delete = connection.execute(&format!("DELETE FROM {table}"), []);
        assert!(
            delete
                .as_ref()
                .err()
                .is_some_and(|error| error.to_string().contains("canonical table is append-only")),
            "{table} accepted a DELETE: {delete:?}"
        );
        assert_eq!(
            count(&connection, table)?,
            1,
            "{table} row survived the refused mutations"
        );
        assert!(
            CANONICAL_TABLES.contains(&table),
            "{table} carries guard triggers but is missing from the authorizer's canonical set"
        );
        assert!(
            column_names(&connection, table)?.contains(&key.to_owned()),
            "{table} has no {key} column"
        );
    }
    Ok(())
}

/// A closure row cannot exist without the event that registered it.
#[test]
fn closure_row_requires_its_event() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("requires-event")?;
    let mut connection = database.open()?;
    let (transaction, _) = append_all_arms(&mut connection, None)?;
    transaction.commit()?;

    let unknown_event = synthetic_id(0xbeef);
    let orphan = connection.execute(
        "INSERT INTO model_run (model_run_id, registered_event_id, domain_id, scope_id, \
         source_digest, valid_from, valid_to) VALUES (?1, ?2, ?3, ?4, NULL, 100, NULL)",
        params![
            synthetic_id(0x7001).to_vec(),
            unknown_event.to_vec(),
            synthetic_id(0x0001).to_vec(),
            synthetic_id(0x0002).to_vec(),
        ],
    );
    assert!(
        orphan
            .as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("FOREIGN KEY")),
        "a closure row referencing no event was accepted: {orphan:?}"
    );

    // The other half of the binding: within a table, one event registers at most
    // one aggregate. That is the same per-table shape `scope.created_event_id`,
    // `artifact_descriptor.registered_event_id`, and `claim.assertion_event_id`
    // already carry.
    let existing_event: Vec<u8> =
        connection.query_row("SELECT registered_event_id FROM model_run", [], |row| {
            row.get(0)
        })?;
    let reused = connection.execute(
        "INSERT INTO model_run (model_run_id, registered_event_id, domain_id, scope_id,          source_digest, valid_from, valid_to) VALUES (?1, ?2, ?3, ?4, NULL, 100, NULL)",
        params![
            synthetic_id(0x7002).to_vec(),
            existing_event,
            synthetic_id(0x0001).to_vec(),
            synthetic_id(0x0002).to_vec(),
        ],
    );
    assert!(
        reused
            .as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("UNIQUE")),
        "one event registered two aggregates in one table: {reused:?}"
    );

    // Across tables the guarantee is structural rather than declarative: an event
    // carries exactly one payload arm, and each arm maps to exactly one closure
    // table, so no event can reach two of them in the first place.
    let mut destinations = BTreeSet::new();
    for event in eighteen_arm_events()?
        .iter()
        .filter(|event| event.payload.registration().is_some())
    {
        assert!(
            destinations.insert(closure_table_for(&event.payload)?),
            "two v3 arms share one closure table"
        );
    }
    assert_eq!(destinations.len(), CLOSURE_TABLES.len());
    Ok(())
}

/// Eighteen arms and their eighteen closure rows commit together or not at all.
#[test]
fn acceptance_is_still_atomic_with_18_arms() -> Result<(), Box<dyn Error>> {
    let committed = MigratedDatabase::new("atomic-commit")?;
    let mut connection = committed.open()?;
    let (transaction, written) = append_all_arms(&mut connection, None)?;
    transaction.commit()?;
    assert_eq!(written, 19, "one scope plus eighteen registration arms");
    assert_eq!(count(&connection, "ledger_event")?, 19);
    for (table, _, _) in CLOSURE_TABLES {
        assert_eq!(count(&connection, table)?, 1, "{table} committed its row");
    }

    // The same batch abandoned partway through leaves nothing behind, including
    // the arms that had already been written when it stopped.
    let abandoned = MigratedDatabase::new("atomic-rollback")?;
    let mut connection = abandoned.open()?;
    let (transaction, written) = append_all_arms(&mut connection, Some(10))?;
    assert_eq!(written, 10, "the batch stopped mid-way through the arms");
    transaction.rollback()?;
    assert_eq!(count(&connection, "ledger_event")?, 0);
    assert_eq!(count(&connection, "scope")?, 0);
    for (table, _, _) in CLOSURE_TABLES {
        assert_eq!(count(&connection, table)?, 0, "{table} kept a partial row");
    }
    Ok(())
}

/// The trigger layer and the authorizer layer guard exactly the same tables.
#[test]
fn authorizer_covers_every_canonical_table() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("authorizer")?;
    let connection = database.open()?;

    let mut statement = connection.prepare(
        "SELECT tbl_name FROM sqlite_schema \
         WHERE type = 'trigger' AND name GLOB 'guard_*_update'",
    )?;
    let guarded = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    drop(statement);

    let authorized = CANONICAL_TABLES
        .iter()
        .map(|table| (*table).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        guarded, authorized,
        "the trigger layer and the authorizer layer must guard the same tables; \
         a table in only one of them has a single point of enforcement"
    );
    for (table, _, _) in CLOSURE_TABLES {
        assert!(guarded.contains(table), "{table} has no guard trigger pair");
    }
    Ok(())
}

/// No aggregate field is smuggled into `claim` as free text.
#[test]
fn no_aggregate_smuggled_into_object_text() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("no-smuggling")?;
    let connection = database.open()?;

    // 1. Migration 0004 leaves `claim` exactly as migration 0001 wrote it, so
    //    `object_kind` is still the closed nine-value enum and no new `object_*`
    //    column appeared to widen it.
    let claim_sql: String = connection.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'claim'",
        [],
        |row| row.get(0),
    )?;
    for kind in [
        "ENTITY",
        "TEXT",
        "INTEGER",
        "BOOLEAN",
        "DECIMAL",
        "INSTANT",
        "INTERVAL",
        "MASTERY",
        "FRESHNESS",
    ] {
        assert!(
            claim_sql.contains(&format!("'{kind}'")),
            "{kind} is missing"
        );
    }
    let object_columns = column_names(&connection, "claim")?
        .into_iter()
        .filter(|column| column.starts_with("object_"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        object_columns,
        [
            "object_decimal_coefficient",
            "object_decimal_scale",
            "object_entity_id",
            "object_integer",
            "object_interval_from",
            "object_interval_to",
            "object_kind",
            "object_text",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "migration 0004 must neither add nor remove a claim object column"
    );
    assert!(
        !migration_0004_statements().contains("claim"),
        "migration 0004 must not touch the claim table at all"
    );

    // 2. Every v3 event kind has a typed table of its own, so no aggregate has a
    //    reason to be encoded as claim text in the first place.
    let tables = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table'")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    assert_eq!(CLOSURE_TABLES.len(), V3_EVENT_KINDS.len());
    for (table, _, _) in CLOSURE_TABLES {
        assert!(tables.contains(table), "{table} has no typed home");
    }

    // 3. No closure table offers a free-text or blob-of-JSON escape hatch that a
    //    later aggregate owner could fill instead of adding a typed column.
    for (table, _, _) in CLOSURE_TABLES {
        for column in column_names(&connection, table)? {
            assert!(
                !matches!(
                    column.as_str(),
                    "object_text"
                        | "payload"
                        | "json"
                        | "data"
                        | "extra"
                        | "properties"
                        | "attributes"
                        | "metadata"
                ),
                "{table}.{column} is a free-text escape hatch; add a typed column instead"
            );
        }
    }
    Ok(())
}

/// Migration 0004 only moves forward, and only on a pre-listen connection.
#[test]
fn migration_0004_is_forward_only_and_pre_listen() -> Result<(), Box<dyn Error>> {
    // Forward-only: there is no down path and no idempotent re-run. Applying the
    // migration a second time fails, so a caller cannot quietly re-enter it.
    let database = MigratedDatabase::new("forward-only")?;
    let mut connection = database.open()?;
    let reapplied = apply_aggregate_migration_pre_listen(&mut connection);
    assert!(
        reapplied.is_err(),
        "migration 0004 re-applied itself without complaint"
    );
    let statements = migration_0004_statements();
    for statement in ["DROP TABLE", "ALTER TABLE"] {
        let occurrences = statements.matches(statement).count();
        assert_eq!(
            occurrences, 1,
            "{statement} appears {occurrences} times; the only one allowed is the \
             documented ledger_event CHECK rebuild"
        );
    }

    // The migration is a delta on the canonical core, never on profile identity,
    // so it cannot convert one profile format into another.
    for identity in ["schema_meta", "application_id", "user_version"] {
        assert!(
            !statements.contains(identity),
            "migration 0004 references {identity}; it must leave the schema-2 \
             identity that migration 0003 establishes untouched"
        );
    }

    // Pre-listen: the product connection's authorizer denies the DDL this
    // migration is made of, so 0004 can only run on the maintenance connection,
    // which installs no authorizer. The flag is set to its most permissive
    // state, mid-acceptance, so the denial is not an artefact of being idle.
    let guarded = MigratedDatabase::new("pre-listen")?;
    let connection = guarded.open()?;
    let accepting = Arc::new(AtomicBool::new(true));
    install_canonical_authorizer(&connection, Arc::clone(&accepting))?;
    for ddl in [
        "CREATE TABLE probe_0004 (probe_id INTEGER)",
        "DROP TABLE model_run",
        "ALTER TABLE model_run RENAME TO model_run_probe",
        "UPDATE model_run SET domain_id = x'00'",
        "DELETE FROM model_run",
    ] {
        assert!(
            connection.execute_batch(ddl).is_err(),
            "the product authorizer admitted `{ddl}`"
        );
    }
    Ok(())
}

/// The base above really is store schema version 2, not the schema-1 core.
///
/// `P2-C2` ran every test in this module against the schema-1 core while 0003
/// was in flight. This replaces the guard that made that substitution expire:
/// where the old test asserted 0003 was still absent, this asserts the base is
/// now built from it, so the module cannot quietly fall back.
#[test]
fn the_test_base_is_the_real_schema_two() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("real-base")?;
    let connection = database.open()?;

    let user_version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 2, "the base is not store schema version 2");
    let application_id: i64 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    assert_eq!(application_id, i64::from(SQLITE_APPLICATION_ID));

    let (schema_version, semver): (i64, String) = connection.query_row(
        "SELECT schema_version, schema_semver FROM schema_meta WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(schema_version, 2);
    assert_eq!(semver, "2.0.0");

    // The schema-2 singleton is the format-fact column set. A posture column
    // here would be a claim about admission that no receipt supports.
    let columns = column_names(&connection, "schema_meta")?;
    for absent in ["data_policy", "production_data_allowed", "product_network"] {
        assert!(
            !columns.iter().any(|name| name == absent),
            "the schema-2 singleton records {absent}"
        );
    }
    Ok(())
}
