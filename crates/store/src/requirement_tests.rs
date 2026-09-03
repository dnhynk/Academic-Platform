//! Named acceptance evidence for migration 0015's requirement-rule tables.
//!
//! The type half of `P2-U2` is in `crates/requirement`, where the review gate
//! and the immutable published set are. What that crate cannot observe is a
//! writer that skips it: it has no `academic-store` edge at all, so every row
//! in these tables was written by something outside its boundary. The two
//! properties the gate exists for -- that one person cannot be both reviewers,
//! and that a published version is replaced only by supersession -- therefore
//! each need a second enforcement layer that does not depend on the Rust
//! boundary having been used.
//!
//! The base is the one `aggregate_closure_tests` builds -- `0001`, the real
//! `0003`, then the aggregate migrations through `0015` -- and the parent rows
//! come from the real closure writer over real registration events, so these
//! rows sit under the same foreign keys a product write would.

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, ContentDigest, CurriculumVersionId, DomainId, Event, EventPayload, RequirementSetId,
    ScopeDescriptor, ScopeId, TimestampMillis, ValidInterval,
};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::{MIGRATION_0015_SQL, apply_aggregate_migration_pre_listen},
    repository::ClosureWriter,
};

type TestResult = Result<(), Box<dyn Error>>;

static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Migration 0015's three tables.
const REQUIREMENT_TABLES: [&str; 3] = [
    "requirement_set_version",
    "requirement_rule",
    "requirement_rule_review",
];

struct MigratedDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl MigratedDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-0015-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let database = Self {
            path: root.join("requirement.sqlite3"),
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
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The identifiers the registration produced.
struct Registered {
    curriculum: CurriculumVersionId,
    requirement_set: RequirementSetId,
}

fn record_digest(label: &str) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (slot, byte) in bytes.iter_mut().zip(label.bytes().cycle()) {
        *slot = byte;
    }
    bytes
}

/// Registers a scope, a curriculum version and a requirement set through the
/// real closure writer, so migration 0015's foreign keys have real parents.
fn register(connection: &mut Connection) -> Result<Registered, Box<dyn Error>> {
    let domain_id: DomainId = typed_id(0x0001)?;
    let scope_id: ScopeId = typed_id(0x0002)?;
    let curriculum: CurriculumVersionId = typed_id(0x0100)?;
    let requirement_set: RequirementSetId = typed_id(0x0110)?;
    let actor = Actor::Importer {
        name: "academic.u2.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let interval = ValidInterval::open_ended(TimestampMillis::new(100));

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
                label: "synthetic U2 scope".to_owned(),
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
                    id: curriculum,
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
            actor,
            domain_id,
            payload: EventPayload::RequirementSetPublished(
                academic_domain::RequirementSetRegistration {
                    id: requirement_set,
                    curriculum_version_id: curriculum,
                    domain_id,
                    scope_id,
                    source_digest: Some(ContentDigest::from_sha256_bytes(record_digest("rules"))),
                    valid_time: interval,
                },
            ),
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
    crate::curriculum_tests::seed_batch(
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
        curriculum,
        requirement_set,
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

/// The rule-type vocabulary the `CHECK` admits, read out of the migration text.
fn migration_rule_types() -> Result<Vec<String>, Box<dyn Error>> {
    let at = MIGRATION_0015_SQL
        .find("rule_type TEXT NOT NULL CHECK (rule_type IN (")
        .ok_or("the rule_type CHECK is not in migration 0015")?;
    let body = &MIGRATION_0015_SQL[at..];
    let end = body
        .find("))")
        .ok_or("the rule_type CHECK does not close")?;
    Ok(body[..end]
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect())
}

fn publish_version(
    connection: &Connection,
    registered: &Registered,
    version: i64,
    supersedes: Option<i64>,
    hash_byte: u8,
) -> Result<(), Box<dyn Error>> {
    connection.execute(
        "INSERT INTO requirement_set_version (
            requirement_set_id, version, supersedes_version, rule_set_hash,
            curriculum_version_id, effective_from
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            registered.requirement_set.as_bytes().to_vec(),
            version,
            supersedes,
            vec![hash_byte; 32],
            registered.curriculum.as_bytes().to_vec(),
            1_800_000_000_000_i64,
        ],
    )?;
    Ok(())
}

fn insert_rule(
    connection: &Connection,
    registered: &Registered,
    rule_uuid: u32,
    rule_id: &str,
    rule_type: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = [0_u8; 16];
    bytes[0..8].copy_from_slice(&[0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00]);
    bytes[8] = 0x80;
    bytes[12..16].copy_from_slice(&rule_uuid.to_be_bytes());
    let identity = bytes.to_vec();
    connection.execute(
        "INSERT INTO requirement_rule (
            requirement_rule_id, requirement_set_id, version, rule_id, rule_type, source_digest
         ) VALUES (?1, ?2, 1, ?3, ?4, ?5)",
        params![
            identity,
            registered.requirement_set.as_bytes().to_vec(),
            rule_id,
            rule_type,
            vec![0x22_u8; 32],
        ],
    )?;
    Ok(identity)
}

/// The `CHECK` vocabulary is section 11.2's fourteen, in section 11.2's order.
#[test]
fn the_rule_type_check_is_the_specifications_fourteen() -> TestResult {
    let admitted = migration_rule_types()?;
    assert_eq!(
        admitted,
        vec![
            "CREDIT_MINIMUM",
            "ALL_OF",
            "AT_LEAST_N_OF",
            "COUNT_WITH_CONSTRAINTS",
            "GPA_MINIMUM",
            "AREA_DISTRIBUTION",
            "CO_REQUISITE",
            "MUTUALLY_EXCLUSIVE",
            "EQUIVALENCY",
            "MAXIMUM_RECOGNITION",
            "NON_CREDIT_TRAINING",
            "LANGUAGE_OF_INSTRUCTION",
            "THESIS_RESEARCH",
            "EXCEPTION_APPROVAL",
        ],
        "the migration's rule-type vocabulary is not section 11.2's, in order"
    );
    // No `UNKNOWN`. A rule whose kind nobody decided is not a row.
    assert!(
        !admitted.iter().any(|kind| kind == "UNKNOWN"),
        "the rule-type CHECK admits UNKNOWN as a published kind"
    );

    // And the database refuses a fifteenth.
    let (_database, connection, registered) = migrated("rule-type")?;
    publish_version(&connection, &registered, 1, None, 0x11)?;
    let refused = insert_rule(
        &connection,
        &registered,
        0x0200,
        "invented",
        "FREE_TEXT_JUDGEMENT",
    );
    assert!(
        refused.is_err(),
        "the database admitted a rule type section 11.2 does not name"
    );
    // Each of the fourteen is admitted, so the CHECK is not refusing everything.
    for (index, kind) in admitted.iter().enumerate() {
        insert_rule(
            &connection,
            &registered,
            0x0300 + u32::try_from(index)?,
            &format!("rule_{index}"),
            kind,
        )?;
    }
    Ok(())
}

/// The row shape carries no sentence.
///
/// `production_audit_no_llm`'s database half. The Rust half is that
/// `ExecutableRule` has no free-text field; this is that the column it would be
/// persisted through does not exist either, so a writer that skipped the crate
/// has nowhere to put one.
#[test]
fn no_requirement_table_carries_a_free_text_column() -> TestResult {
    let (_database, connection, _registered) = migrated("columns")?;
    for table in REQUIREMENT_TABLES {
        let columns = columns_of(&connection, table)?;
        assert!(
            !columns.is_empty(),
            "{table} has no columns, so the comparison below is vacuous"
        );
    }
    // Each table's whole column set, so a column nobody predicted appears as a
    // difference rather than having to be guessed at in a forbidden list.
    assert_eq!(
        columns_of(&connection, "requirement_rule")?,
        BTreeSet::from([
            "requirement_rule_id".to_owned(),
            "requirement_set_id".to_owned(),
            "version".to_owned(),
            "rule_id".to_owned(),
            "rule_type".to_owned(),
            "source_digest".to_owned(),
        ]),
        "requirement_rule's columns changed"
    );
    assert_eq!(
        columns_of(&connection, "requirement_rule_review")?,
        BTreeSet::from([
            "requirement_rule_id".to_owned(),
            "reviewer_entity_id".to_owned(),
            "attested_at".to_owned(),
        ]),
        "requirement_rule_review's columns changed"
    );
    assert_eq!(
        columns_of(&connection, "requirement_set_version")?,
        BTreeSet::from([
            "requirement_set_id".to_owned(),
            "version".to_owned(),
            "supersedes_version".to_owned(),
            "rule_set_hash".to_owned(),
            "curriculum_version_id".to_owned(),
            "effective_from".to_owned(),
        ]),
        "requirement_set_version's columns changed"
    );
    Ok(())
}

/// One person cannot be both reviewers, at the database layer.
///
/// `rule_candidate_review_gate`'s second half. `ReviewGate::admit` refuses the
/// same shape in Rust; this is the layer a writer that never came through the
/// gate still meets.
#[test]
fn one_reviewer_cannot_attest_twice_to_one_rule() -> TestResult {
    let (_database, connection, registered) = migrated("reviewers")?;
    publish_version(&connection, &registered, 1, None, 0x31)?;
    let rule = insert_rule(
        &connection,
        &registered,
        0x0201,
        "total_credits",
        "CREDIT_MINIMUM",
    )?;

    // The instant is a parameter, and the second attestation by the same person
    // uses a *different* one. Injection `U2-I17` is what made that load-bearing:
    // it widened the key to `(rule, reviewer, attested_at)`, and a test that
    // re-attested at the same instant still saw a refusal -- of a duplicate row,
    // not of a duplicate reviewer. The guard passed and proved nothing.
    let attest = |reviewer: u8, at: i64| {
        connection.execute(
            "INSERT INTO requirement_rule_review (
                requirement_rule_id, reviewer_entity_id, attested_at
             ) VALUES (?1, ?2, ?3)",
            params![rule.clone(), vec![reviewer; 16], at],
        )
    };

    attest(0x71, 1_800_000_000_000)?;
    assert!(
        attest(0x71, 1_800_000_000_001).is_err(),
        "one reviewer attested twice to one rule at two instants, which is one          review recorded twice"
    );
    // Two different people is the shape that is admitted.
    attest(0x72, 1_800_000_000_002)?;
    let reviewers: i64 = connection.query_row(
        "SELECT count(DISTINCT reviewer_entity_id) FROM requirement_rule_review
         WHERE requirement_rule_id = ?1",
        params![rule],
        |row| row.get(0),
    )?;
    assert_eq!(
        reviewers, 2,
        "the two admitted attestations are not two people"
    );
    Ok(())
}

/// A published version is replaced only by supersession, and the chain does not
/// fork.
///
/// `ruleset_immutable_publish`'s second half.
#[test]
fn a_published_version_is_append_only_and_the_chain_does_not_fork() -> TestResult {
    let (_database, connection, registered) = migrated("versions")?;
    publish_version(&connection, &registered, 1, None, 0x41)?;
    publish_version(&connection, &registered, 2, Some(1), 0x42)?;

    // Republishing a version number is refused by the primary key, and by the
    // primary key alone: the row below carries a fresh hash and supersedes
    // nothing, so neither UNIQUE can be what refuses it. `U2-I17` is the reason
    // that matters -- a case that more than one constraint refuses cannot say
    // which one it measured, and a weakened key would keep passing it.
    assert!(
        publish_version(&connection, &registered, 2, None, 0x43).is_err(),
        "the database admitted a second row for one version number"
    );

    // A second version superseding the same predecessor forks the chain a
    // historical replay walks, and the UNIQUE refuses it.
    assert!(
        publish_version(&connection, &registered, 3, Some(1), 0x44).is_err(),
        "two versions superseded the same predecessor"
    );
    // Superseding the head is what is admitted.
    publish_version(&connection, &registered, 3, Some(2), 0x45)?;

    // Two versions cannot share a rule-set hash: two versions with the same
    // content are the same version.
    assert!(
        publish_version(&connection, &registered, 4, Some(3), 0x41).is_err(),
        "two versions were published under one rule-set hash"
    );

    // And an UPDATE is refused by the trigger, whatever it would have changed.
    let updated = connection.execute(
        "UPDATE requirement_set_version SET effective_from = 1 WHERE version = 1",
        [],
    );
    let message = match updated {
        Ok(_) => return Err("the append-only trigger admitted an UPDATE".into()),
        Err(error) => error.to_string(),
    };
    assert!(
        message.contains("canonical table is append-only"),
        "unexpected trigger message: {message}"
    );
    Ok(())
}

/// Every table is append-only and the authorizer knows each of them.
#[test]
fn every_requirement_table_is_guarded_and_canonical() -> TestResult {
    let (_database, connection, _registered) = migrated("guards")?;
    let mut statement =
        connection.prepare("SELECT name FROM sqlite_schema WHERE type = 'trigger'")?;
    let triggers: BTreeSet<String> = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<_, _>>()?;
    for table in REQUIREMENT_TABLES {
        for action in ["update", "delete"] {
            let name = format!("guard_{table}_{action}");
            assert!(
                triggers.contains(&name),
                "{table} has no {action} guard; migration 0004's terms apply to every \
                 canonical table"
            );
        }
        assert!(
            crate::authorizer::CANONICAL_TABLES.contains(&table),
            "{table} is not in CANONICAL_TABLES, so the authorizer would admit a DROP"
        );
    }
    Ok(())
}
