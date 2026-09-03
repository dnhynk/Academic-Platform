//! Named acceptance evidence for migration 0012's repository-snapshot tables.
//!
//! The type half of `P2-R1` is in `crates/repository`, where the gate and the
//! frozen snapshot are. What that crate cannot observe is a writer that skips
//! it: these tables exist on a migrated database and are reachable by any
//! process holding the file, so the dirty-worktree rule, the tracked/untracked
//! manifest and the secret-hash default-deny each need a second enforcement
//! layer that does not depend on the Rust boundary having been used.
//!
//! The base is the one `aggregate_closure_tests` builds — `0001`, the real
//! `0003`, then the aggregate migrations through `0012` — so these rows run in
//! both lanes and against the real schema rather than something resembling it.

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, ContentDigest, DomainId, Event, EventPayload, RepositoryId, ScopeDescriptor, ScopeId,
    SnapshotId, SnapshotRegistration, TimestampMillis, ValidInterval,
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::apply_aggregate_migration_pre_listen,
    repository::ClosureWriter,
};

static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The two snapshots every case registers: one clean commit, one dirty tree.
///
/// Enumerated rather than counted, and the `source_type` token is the one the
/// migration admits, so a value renamed on one side fails against the other.
const SNAPSHOTS: [(u32, &str, &str); 2] = [
    (0x0301, "COMMIT", "GIT_COMMIT"),
    (0x0302, "DIRTY_WORKTREE", "DIRTY_WORKTREE"),
];

/// The digest the `SNAPSHOT_REGISTERED` event carries, which
/// `guard_repository_snapshot_authorized` compares a typed row against.
fn record_digest(label: &str) -> [u8; 32] {
    ContentDigest::sha256(label.as_bytes())
        .as_bytes()
        .to_owned()
}

struct MigratedDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl MigratedDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-store-0012-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let database = Self {
            path: root.join("repository.sqlite3"),
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

/// Registers the two snapshots under one repository.
fn two_snapshots(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
    let domain_id: DomainId = typed_id(0x0001)?;
    let scope_id: ScopeId = typed_id(0x0002)?;
    let repository_id: RepositoryId = typed_id(0x0300)?;
    let actor = Actor::Importer {
        name: "academic.r1.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let interval = ValidInterval::open_ended(TimestampMillis::new(100));

    let mut events = vec![Event {
        id: typed_id(0x0400)?,
        origin_seq: 1,
        origin_observed_at: TimestampMillis::new(100),
        actor: actor.clone(),
        domain_id,
        payload: EventPayload::ScopeRegistered(ScopeDescriptor {
            id: scope_id,
            domain_id,
            label: "synthetic R1 scope".to_owned(),
        }),
    }];
    for (ordinal, (id, label, _)) in SNAPSHOTS.into_iter().enumerate() {
        let snapshot_id: SnapshotId = typed_id(id)?;
        events.push(Event {
            id: typed_id(0x0401 + u32::try_from(ordinal)?)?,
            origin_seq: u64::try_from(ordinal)? + 2,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::SnapshotRegistered(SnapshotRegistration {
                id: snapshot_id,
                repository_id,
                domain_id,
                scope_id,
                source_digest: Some(ContentDigest::sha256(label.as_bytes())),
                valid_time: interval,
            }),
        });
    }

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
        let receipts = BTreeMap::<_, academic_vault::SealedObjectCapability>::new();
        let mut closure = ClosureWriter::new(&transaction, &batch, &receipts);
        for (index, event) in batch.events.iter().enumerate() {
            closure.append_event(event, u64::try_from(index)? + 1)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn migrated(label: &str) -> Result<(MigratedDatabase, Connection), Box<dyn Error>> {
    let database = MigratedDatabase::new(label)?;
    let mut connection = database.open()?;
    two_snapshots(&mut connection)?;
    Ok((database, connection))
}

/// Inserts one typed snapshot row, returning whatever SQLite said.
#[allow(clippy::too_many_arguments)]
fn insert_snapshot(
    connection: &Connection,
    suffix: u32,
    digest_label: &str,
    source: &str,
    source_type: &str,
    commit_id: Option<&str>,
    dirty_patch: Option<Vec<u8>>,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO repository_snapshot (snapshot_id, source, source_type, branch, commit_id, \
         captured_at, manifest_digest, dirty_patch_digest, analysis_policy_hash, \
         secret_scan_result, record_digest) \
         VALUES (?1, ?2, ?3, 'main', ?4, 100, ?5, ?6, ?7, 'PASS', ?8)",
        params![
            synthetic_id(suffix).to_vec(),
            source,
            source_type,
            commit_id,
            vec![0x41_u8; 32],
            dirty_patch,
            vec![0x42_u8; 32],
            record_digest(digest_label).to_vec(),
        ],
    )
}

fn must_fail(result: rusqlite::Result<usize>, claim: &str) -> Result<String, Box<dyn Error>> {
    match result {
        Ok(_) => Err(claim.to_string().into()),
        Err(error) => Ok(error.to_string()),
    }
}

#[test]
fn a_typed_snapshot_row_needs_the_event_that_authorized_it() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("authorized")?;

    // The digest the event carries is what the trigger compares.
    let refused = must_fail(
        insert_snapshot(
            &connection,
            0x0301,
            "not the registered digest",
            "COMMIT",
            "GIT_COMMIT",
            Some("abc1234"),
            None,
        ),
        "a typed row with the wrong digest was accepted",
    )?;
    assert!(
        refused.contains("not the record its event authorized"),
        "unexpected trigger message: {refused}"
    );

    // A row for a snapshot nobody registered is refused too. SQLite runs a
    // `BEFORE INSERT` trigger before it checks a foreign key, so the message is
    // the trigger's: the reference to `snapshot` is the second layer here
    // rather than the one that speaks first.
    let unregistered = must_fail(
        insert_snapshot(
            &connection,
            0x03ff,
            "COMMIT",
            "COMMIT",
            "GIT_COMMIT",
            Some("abc1234"),
            None,
        ),
        "a typed row for an unregistered snapshot was accepted",
    )?;
    assert!(
        unregistered.contains("not the record its event authorized"),
        "unexpected refusal for an unregistered snapshot: {unregistered}"
    );

    // The reference itself is observable one table down, where no trigger
    // speaks first: a manifest row under a snapshot that has no typed row is
    // refused by the foreign key.
    let orphan = must_fail(
        connection.execute(
            "INSERT INTO repository_snapshot_manifest_entry (snapshot_id, path_ordinal, path, \
             blob_digest, language, byte_len, dirty_kind) VALUES (?1, 0, 'README.md', ?2, \
             'MARKDOWN', 12, NULL)",
            params![synthetic_id(0x03ff).to_vec(), vec![0x44_u8; 32]],
        ),
        "a manifest row under no snapshot was accepted",
    )?;
    assert!(
        orphan.contains("FOREIGN KEY"),
        "unexpected refusal for an orphan manifest row: {orphan}"
    );

    insert_snapshot(
        &connection,
        0x0301,
        "COMMIT",
        "COMMIT",
        "GIT_COMMIT",
        Some("abc1234"),
        None,
    )?;
    Ok(())
}

#[test]
fn a_dirty_working_tree_row_is_not_a_commit_row() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("dirty")?;

    // A dirty tree with no patch digest is refused: it would be recorded as
    // indistinguishable from its HEAD.
    let missing = must_fail(
        insert_snapshot(
            &connection,
            0x0302,
            "DIRTY_WORKTREE",
            "DIRTY_WORKTREE",
            "DIRTY_WORKTREE",
            Some("abc1234"),
            None,
        ),
        "a dirty working tree with no patch digest was accepted",
    )?;
    assert!(
        missing.contains("a dirty working tree is not its HEAD commit"),
        "unexpected trigger message: {missing}"
    );

    // And a commit row carrying one is refused from the other side, so the
    // guard is not satisfied by simply always supplying a digest.
    let spurious = must_fail(
        insert_snapshot(
            &connection,
            0x0301,
            "COMMIT",
            "COMMIT",
            "GIT_COMMIT",
            Some("abc1234"),
            Some(vec![0x43_u8; 32]),
        ),
        "a git-commit row carrying a dirty patch digest was accepted",
    )?;
    assert!(
        spurious.contains("a dirty working tree is not its HEAD commit"),
        "unexpected trigger message: {spurious}"
    );

    // A git-commit row naming no commit is refused too.
    let headless = must_fail(
        insert_snapshot(
            &connection,
            0x0301,
            "COMMIT",
            "COMMIT",
            "GIT_COMMIT",
            None,
            None,
        ),
        "a git-commit row naming no commit was accepted",
    )?;
    assert!(
        headless.contains("names no commit"),
        "unexpected trigger message: {headless}"
    );

    insert_snapshot(
        &connection,
        0x0302,
        "DIRTY_WORKTREE",
        "DIRTY_WORKTREE",
        "DIRTY_WORKTREE",
        Some("abc1234"),
        Some(vec![0x43_u8; 32]),
    )?;
    Ok(())
}

#[test]
fn both_halves_of_the_dirty_manifest_are_rows() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("manifest")?;
    insert_snapshot(
        &connection,
        0x0301,
        "COMMIT",
        "COMMIT",
        "GIT_COMMIT",
        Some("abc1234"),
        None,
    )?;
    insert_snapshot(
        &connection,
        0x0302,
        "DIRTY_WORKTREE",
        "DIRTY_WORKTREE",
        "DIRTY_WORKTREE",
        Some("abc1234"),
        Some(vec![0x43_u8; 32]),
    )?;

    let insert = |snapshot: u32, ordinal: i64, path: &str, kind: Option<&str>| {
        connection.execute(
            "INSERT INTO repository_snapshot_manifest_entry (snapshot_id, path_ordinal, path, \
             blob_digest, language, byte_len, dirty_kind) VALUES (?1, ?2, ?3, ?4, ?5, 12, ?6)",
            params![
                synthetic_id(snapshot).to_vec(),
                ordinal,
                path,
                vec![0x44_u8; 32],
                "RUST",
                kind,
            ],
        )
    };

    // Both labels are storable under the dirty snapshot, and the enumeration is
    // the claim: a manifest recording only one half fails here.
    insert(0x0302, 0, "src/orders/service.rs", Some("TRACKED"))?;
    insert(0x0302, 1, "notes/scratch.md", Some("UNTRACKED"))?;
    insert(0x0302, 2, "README.md", None)?;
    let stored: Vec<String> = connection
        .prepare(
            "SELECT dirty_kind FROM repository_snapshot_manifest_entry \
             WHERE snapshot_id = ?1 AND dirty_kind IS NOT NULL ORDER BY dirty_kind",
        )?
        .query_map(params![synthetic_id(0x0302).to_vec()], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    assert_eq!(stored, vec!["TRACKED".to_owned(), "UNTRACKED".to_owned()]);

    // Neither label is storable under a snapshot that is not a dirty tree.
    for kind in ["TRACKED", "UNTRACKED"] {
        let refused = must_fail(
            insert(0x0301, 0, "src/orders/service.rs", Some(kind)),
            "a dirty manifest row was accepted under a clean snapshot",
        )?;
        assert!(
            refused.contains("belongs to a snapshot that is not one"),
            "unexpected trigger message: {refused}"
        );
    }
    // A third label does not exist.
    let unknown = must_fail(
        insert(0x0302, 3, "docs/plan.md", Some("STAGED")),
        "a third dirty label was accepted",
    )?;
    assert!(
        unknown.contains("CHECK constraint failed"),
        "unexpected refusal for a third dirty label: {unknown}"
    );
    Ok(())
}

#[test]
fn a_secret_digest_row_needs_a_recorded_decision() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("disclosure")?;
    let request = vec![0x45_u8; 32];

    let insert = |path: &str, decision: Option<&str>, digest: Option<Vec<u8>>| {
        connection.execute(
            "INSERT INTO repository_secret_finding (request_digest, path, reason_code, \
             disclosure_decision_id, blob_digest) VALUES (?1, ?2, 'SECRET_PATTERN', ?3, ?4)",
            params![request, path, decision, digest],
        )
    };

    // Default deny. A finding with no decision and no digest is the ordinary
    // row, and it is accepted.
    insert("deploy/key.txt", None, None)?;

    // A digest with no decision is refused.
    let undisclosed = must_fail(
        insert("src/config.rs", None, Some(vec![0x46_u8; 32])),
        "a secret digest was stored with no recorded decision",
    )?;
    assert!(
        undisclosed.contains("needs a recorded disclosure decision"),
        "unexpected trigger message: {undisclosed}"
    );

    // A decision with no digest is refused from the other side, so the guard
    // is not satisfied by naming a decision that discloses nothing.
    connection.execute(
        "INSERT INTO repository_hash_disclosure_decision (decision_id, actor_id, reason, \
         recorded_at) VALUES ('decision-1', 'actor-user', 'rotation verification', 100)",
        [],
    )?;
    let empty = must_fail(
        insert("src/config.rs", Some("decision-1"), None),
        "a disclosure naming no digest was accepted",
    )?;
    assert!(
        empty.contains("needs a recorded disclosure decision"),
        "unexpected trigger message: {empty}"
    );

    // A digest naming a decision nobody recorded is refused by the reference,
    // which is what makes the decision a record rather than a flag.
    let unrecorded = must_fail(
        insert("src/config.rs", Some("decision-2"), Some(vec![0x46_u8; 32])),
        "a digest naming an unrecorded decision was accepted",
    )?;
    assert!(
        unrecorded.contains("FOREIGN KEY"),
        "unexpected refusal for an unrecorded decision: {unrecorded}"
    );

    // Both together, with the decision recorded, is the one accepted shape.
    insert("src/config.rs", Some("decision-1"), Some(vec![0x46_u8; 32]))?;
    Ok(())
}

#[test]
fn the_repository_tables_are_append_only() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("append-only")?;
    insert_snapshot(
        &connection,
        0x0301,
        "COMMIT",
        "COMMIT",
        "GIT_COMMIT",
        Some("abc1234"),
        None,
    )?;
    connection.execute(
        "INSERT INTO repository_hash_disclosure_decision (decision_id, actor_id, reason, \
         recorded_at) VALUES ('decision-1', 'actor-user', 'rotation verification', 100)",
        [],
    )?;
    connection.execute(
        "INSERT INTO repository_snapshot_tool_version (snapshot_id, tool, version) \
         VALUES (?1, 'academic-repository', '0.1.0')",
        params![synthetic_id(0x0301).to_vec()],
    )?;
    connection.execute(
        "INSERT INTO repository_snapshot_excluded_path (snapshot_id, path, exclusion_reason) \
         VALUES (?1, '.env', 'SECRET_FILE_POLICY')",
        params![synthetic_id(0x0301).to_vec()],
    )?;
    connection.execute(
        "INSERT INTO repository_snapshot_manifest_entry (snapshot_id, path_ordinal, path, \
         blob_digest, language, byte_len, dirty_kind) VALUES (?1, 0, 'README.md', ?2, \
         'MARKDOWN', 12, NULL)",
        params![synthetic_id(0x0301).to_vec(), vec![0x44_u8; 32]],
    )?;
    connection.execute(
        "INSERT INTO repository_secret_finding (request_digest, path, reason_code, \
         disclosure_decision_id, blob_digest) VALUES (?1, 'deploy/key.txt', 'SECRET_PATTERN', \
         NULL, NULL)",
        params![vec![0x45_u8; 32]],
    )?;

    // Every table this migration creates, enumerated with a statement that
    // would edit it. A table added later without a trigger pair fails as a
    // missing key in the first comparison rather than being forgotten.
    let tables = [
        (
            "repository_snapshot",
            "UPDATE repository_snapshot SET branch = 'other'",
        ),
        (
            "repository_snapshot_manifest_entry",
            "UPDATE repository_snapshot_manifest_entry SET byte_len = 13",
        ),
        (
            "repository_snapshot_tool_version",
            "UPDATE repository_snapshot_tool_version SET version = '0.2.0'",
        ),
        (
            "repository_snapshot_excluded_path",
            "UPDATE repository_snapshot_excluded_path SET exclusion_reason = 'DENY_RULE'",
        ),
        (
            "repository_hash_disclosure_decision",
            "UPDATE repository_hash_disclosure_decision SET reason = 'other'",
        ),
        (
            "repository_secret_finding",
            "UPDATE repository_secret_finding SET reason_code = 'SECRET_ENTROPY'",
        ),
    ];
    let created: Vec<String> = connection
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name LIKE 'repository%' \
             ORDER BY name",
        )?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut named: Vec<String> = tables.iter().map(|(name, _)| (*name).to_owned()).collect();
    named.sort();
    assert_eq!(
        created, named,
        "migration 0012 creates a table this test does not exercise"
    );

    for (name, statement) in tables {
        let refused = must_fail(
            connection.execute(statement, []),
            "a repository table accepted an update",
        )?;
        assert!(
            refused.contains("canonical table is append-only"),
            "{name} accepted an update: {refused}"
        );
        let deleted = must_fail(
            connection.execute(&format!("DELETE FROM {name}"), []),
            "a repository table accepted a delete",
        )?;
        assert!(
            deleted.contains("canonical table is append-only"),
            "{name} accepted a delete: {deleted}"
        );
    }
    Ok(())
}
