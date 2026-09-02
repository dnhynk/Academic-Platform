//! Named acceptance evidence for migration 0007's model-run tables.
//!
//! Two of `P2-M1`'s rows are here rather than in `academic-model-run`:
//! `reanalysis_creates_new_candidate_not_mutation` and
//! `reanalysis_diff_links_both_model_runs` assert that the tables append and
//! never edit, and those tables exist only on a migrated database. The other
//! four rows are in `crates/model-run/tests/model_run.rs`, where the types are.
//!
//! The base is the one `aggregate_closure_tests` builds -- `0001`, the real
//! `0003`, then the aggregate migrations through `0007` -- so these rows run in
//! both lanes and against the real schema rather than something resembling it.

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, ContentDigest, DomainId, Event, EventPayload, ModelRunId, ModelRunRegistration,
    ScopeDescriptor, ScopeId, TimestampMillis, ValidInterval,
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::apply_aggregate_migration_pre_listen,
    repository::ClosureWriter,
};

static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The provenance digest both model runs' events carry.
///
/// `guard_model_run_provenance_authorized` compares an inserted record's
/// `record_digest` against it, so a provenance row nobody signed for cannot
/// exist.
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
            "academic-store-0007-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let database = Self {
            path: root.join("model-run.sqlite3"),
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

/// The three `MODEL_RUN_RECORDED` aggregates these tests register.
///
/// Three rather than two, because a fork -- two runs superseding one candidate
/// -- needs a third run to be reachable at all, and a rule nothing can reach is
/// a rule nothing has watched refuse anything.
const RUNS: [(u32, &str); 3] = [(0x010c, "first"), (0x010d, "reanalysis"), (0x010e, "third")];

/// Registers the `MODEL_RUN_RECORDED` aggregates in `RUNS`. Each carries a
/// provenance digest, because the provenance rows below need one to be
/// authorized by.
fn three_model_runs(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
    let domain_id: DomainId = typed_id(0x0001)?;
    let scope_id: ScopeId = typed_id(0x0002)?;
    let actor = Actor::Importer {
        name: "academic.m1.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let interval = ValidInterval::open_ended(TimestampMillis::new(100));

    let mut events = vec![Event {
        id: typed_id(0x0300)?,
        origin_seq: 1,
        origin_observed_at: TimestampMillis::new(100),
        actor: actor.clone(),
        domain_id,
        payload: EventPayload::ScopeRegistered(ScopeDescriptor {
            id: scope_id,
            domain_id,
            label: "synthetic M1 scope".to_owned(),
        }),
    }];
    for (ordinal, (id, label)) in RUNS.into_iter().enumerate() {
        let model_run_id: ModelRunId = typed_id(id)?;
        events.push(Event {
            id: typed_id(0x0301 + u32::try_from(ordinal)?)?,
            origin_seq: u64::try_from(ordinal)? + 2,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::ModelRunRecorded(ModelRunRegistration {
                id: model_run_id,
                domain_id,
                scope_id,
                source_digest: Some(ContentDigest::sha256(label.as_bytes())),
                valid_time: interval,
            }),
        });
    }

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

/// Inserts one provenance row and its children for a registered model run.
fn insert_provenance(
    connection: &Connection,
    model_run: &[u8; 16],
    label: &str,
    transmission: (&str, Option<&str>),
) -> Result<(), Box<dyn Error>> {
    let (kind, grant) = transmission;
    connection.execute(
        concat!(
            "INSERT INTO model_run_provenance (model_run_id, purpose_id, provider_id, ",
            "model_version, prompt_template_hash, transmission_kind, transmitted_grant_id, ",
            "redaction_policy_hash, output_artifact_id, started_at, cost_micros, ",
            "cost_currency, retention_declaration_id, record_digest) ",
            "VALUES (?1, 'CONCEPT_EXTRACTION', 'provider-y', 'concept-extractor-3', ?2, ",
            "?3, ?4, ?5, ?6, 205, 1250, 'KRW', 'ZERO_DAY', ?7)"
        ),
        params![
            model_run.to_vec(),
            vec![0x41_u8; 32],
            kind,
            grant,
            vec![0x42_u8; 32],
            synthetic_id(0x0500).to_vec(),
            record_digest(label).to_vec(),
        ],
    )?;
    connection.execute(
        concat!(
            "INSERT INTO model_run_input_artifact (model_run_id, input_ordinal, artifact_id, ",
            "content_digest) VALUES (?1, 0, ?2, ?3)"
        ),
        params![
            model_run.to_vec(),
            synthetic_id(0x0501).to_vec(),
            vec![0x43_u8; 32],
        ],
    )?;
    if kind == "EGRESSED" {
        connection.execute(
            concat!(
                "INSERT INTO model_run_transmitted_range (model_run_id, range_ordinal, ",
                "object_id, byte_start, byte_end, content_digest) ",
                "VALUES (?1, 0, 'synthetic-object', 10, 18, ?2)"
            ),
            params![model_run.to_vec(), vec![0x44_u8; 32]],
        )?;
    }
    Ok(())
}

fn insert_candidate(
    connection: &Connection,
    candidate: &[u8; 16],
    model_run: &[u8; 16],
    subject: &[u8; 32],
    value: &[u8; 32],
    supersedes: Option<&[u8; 16]>,
) -> rusqlite::Result<usize> {
    connection.execute(
        concat!(
            "INSERT INTO model_run_candidate (candidate_id, model_run_id, subject_digest, ",
            "candidate_digest, supersedes_candidate_id) VALUES (?1, ?2, ?3, ?4, ?5)"
        ),
        params![
            candidate.to_vec(),
            model_run.to_vec(),
            subject.to_vec(),
            value.to_vec(),
            supersedes.map(|id| id.to_vec()),
        ],
    )
}

/// A prepared database plus the three model-run identifiers it registered.
type SeededRuns = (MigratedDatabase, [[u8; 16]; 3]);

/// Sets up the three runs and their provenance rows.
fn seeded(label: &str) -> Result<SeededRuns, Box<dyn Error>> {
    let database = MigratedDatabase::new(label)?;
    let mut connection = database.open()?;
    three_model_runs(&mut connection)?;
    let mut identifiers = [[0_u8; 16]; 3];
    for (slot, (id, run_label)) in identifiers.iter_mut().zip(RUNS) {
        *slot = synthetic_id(id);
        let grant = (run_label != "first").then(|| "a".repeat(64));
        let kind = if grant.is_some() {
            "EGRESSED"
        } else {
            "LOCAL_ONLY"
        };
        insert_provenance(&connection, slot, run_label, (kind, grant.as_deref()))?;
    }
    Ok((database, identifiers))
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// A second model run over the same source appends a candidate; the first
/// candidate is still there, unchanged, and cannot be edited.
#[test]
fn reanalysis_creates_new_candidate_not_mutation() -> Result<(), Box<dyn Error>> {
    let (database, [first_run, second_run, third_run]) = seeded("reanalysis-appends")?;
    let connection = database.open()?;
    let subject = record_digest("the same lecture transcript");
    let prior = synthetic_id(0x0600);
    let revision = synthetic_id(0x0601);
    insert_candidate(
        &connection,
        &prior,
        &first_run,
        &subject,
        &record_digest("candidate A"),
        None,
    )?;

    // The reanalysis appends. Both rows exist afterwards.
    insert_candidate(
        &connection,
        &revision,
        &second_run,
        &subject,
        &record_digest("candidate B"),
        Some(&prior),
    )?;
    let rows: i64 =
        connection.query_row("SELECT count(*) FROM model_run_candidate", [], |row| {
            row.get(0)
        })?;
    assert_eq!(rows, 2, "the reanalysis replaced the earlier candidate");

    // The earlier candidate is byte-identical to what it was written as.
    let (stored_run, stored_value, stored_supersedes) = connection.query_row(
        "SELECT model_run_id, candidate_digest, supersedes_candidate_id \
         FROM model_run_candidate WHERE candidate_id = ?1",
        params![prior.to_vec()],
        |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        },
    )?;
    assert_eq!(stored_run, first_run.to_vec());
    assert_eq!(stored_value, record_digest("candidate A").to_vec());
    assert_eq!(stored_supersedes, None);

    // And it cannot be edited or removed, on both enforcement layers' first one.
    let update = connection.execute(
        "UPDATE model_run_candidate SET candidate_digest = ?1 WHERE candidate_id = ?2",
        params![record_digest("candidate B").to_vec(), prior.to_vec()],
    );
    assert!(
        update
            .as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("canonical table is append-only")),
        "the earlier candidate accepted an UPDATE: {update:?}"
    );
    let delete = connection.execute(
        "DELETE FROM model_run_candidate WHERE candidate_id = ?1",
        params![prior.to_vec()],
    );
    assert!(
        delete
            .as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("canonical table is append-only")),
        "the earlier candidate accepted a DELETE: {delete:?}"
    );

    // A fork is refused: `UNIQUE (supersedes_candidate_id)` admits one revision
    // of a candidate and no second.
    let fork = insert_candidate(
        &connection,
        &synthetic_id(0x0602),
        &third_run,
        &subject,
        &record_digest("candidate C"),
        Some(&prior),
    );
    assert!(
        fork.as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("UNIQUE")),
        "two runs superseded one candidate: {fork:?}"
    );

    // And a run cannot record two candidates about one subject.
    let repeat = insert_candidate(
        &connection,
        &synthetic_id(0x0603),
        &first_run,
        &subject,
        &record_digest("candidate D"),
        None,
    );
    assert!(
        repeat
            .as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("UNIQUE")),
        "one run recorded two candidates about one subject: {repeat:?}"
    );
    Ok(())
}

/// A supersession addresses the subject the prior candidate addressed.
///
/// This is the case `guard_model_run_candidate_supersession` alone refuses.
/// Both `UNIQUE` constraints are satisfied -- `(third_run, another subject)` is
/// a new pair and the prior candidate is not yet superseded -- so removing the
/// trigger makes this insert succeed. That is `M-I14`, and it is why the
/// trigger is not redundant with the constraints beside it.
#[test]
fn a_reanalysis_addresses_the_subject_it_supersedes() -> Result<(), Box<dyn Error>> {
    let (database, [first_run, _, third_run]) = seeded("reanalysis-subject")?;
    let connection = database.open()?;
    let subject = record_digest("the same lecture transcript");
    let prior = synthetic_id(0x0600);
    insert_candidate(
        &connection,
        &prior,
        &first_run,
        &subject,
        &record_digest("candidate A"),
        None,
    )?;

    let other_subject = insert_candidate(
        &connection,
        &synthetic_id(0x0604),
        &third_run,
        &record_digest("a different transcript"),
        &record_digest("candidate E"),
        Some(&prior),
    );
    assert!(
        other_subject.as_ref().err().is_some_and(|error| {
            error
                .to_string()
                .contains("does not supersede the same subject from another run")
        }),
        "a supersession addressed another subject: {other_subject:?}"
    );

    // The control: the same insert about the right subject is accepted, so what
    // the assertion above measures is the subject and not the shape.
    insert_candidate(
        &connection,
        &synthetic_id(0x0605),
        &third_run,
        &subject,
        &record_digest("candidate E"),
        Some(&prior),
    )?;
    Ok(())
}

/// The appended candidate names the run it came from and the run whose
/// candidate it supersedes, so the diff reaches both.
#[test]
fn reanalysis_diff_links_both_model_runs() -> Result<(), Box<dyn Error>> {
    let (database, [first_run, second_run, _]) = seeded("reanalysis-diff")?;
    let connection = database.open()?;
    let subject = record_digest("the same lecture transcript");
    let prior = synthetic_id(0x0600);
    let revision = synthetic_id(0x0601);
    insert_candidate(
        &connection,
        &prior,
        &first_run,
        &subject,
        &record_digest("candidate A"),
        None,
    )?;
    insert_candidate(
        &connection,
        &revision,
        &second_run,
        &subject,
        &record_digest("candidate B"),
        Some(&prior),
    )?;

    let (prior_run, revised_run, prior_value, revised_value, prior_provider, revised_provider) =
        connection.query_row(
            concat!(
                "SELECT prior.model_run_id, revised.model_run_id, prior.candidate_digest, ",
                "revised.candidate_digest, prior_run.provider_id, revised_run.provider_id ",
                "FROM model_run_candidate AS revised ",
                "JOIN model_run_candidate AS prior ",
                "  ON prior.candidate_id = revised.supersedes_candidate_id ",
                "JOIN model_run_provenance AS prior_run ",
                "  ON prior_run.model_run_id = prior.model_run_id ",
                "JOIN model_run_provenance AS revised_run ",
                "  ON revised_run.model_run_id = revised.model_run_id ",
                "WHERE revised.candidate_id = ?1"
            ),
            params![revision.to_vec()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )?;
    assert_eq!(prior_run, first_run.to_vec());
    assert_eq!(revised_run, second_run.to_vec());
    assert_ne!(prior_run, revised_run, "the diff must link two runs");
    assert_ne!(
        prior_value, revised_value,
        "the diff must show the values differ"
    );
    assert_eq!(prior_provider, "provider-y");
    assert_eq!(revised_provider, "provider-y");
    Ok(())
}

/// Every one of the twelve stored fields is required by the row itself.
///
/// The set-equality half of `model_run_requires_every_field` is in
/// `academic-model-run`, where the type is. This is the database half: each
/// column dropped from the insert in turn, each required to be refused.
#[test]
fn model_run_row_requires_every_stored_field() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("required-fields")?;
    let mut connection = database.open()?;
    three_model_runs(&mut connection)?;
    let model_run = synthetic_id(0x010c);

    const COLUMNS: [(&str, &str); 13] = [
        ("model_run_id", "?1"),
        ("purpose_id", "'CONCEPT_EXTRACTION'"),
        ("provider_id", "'provider-y'"),
        ("model_version", "'concept-extractor-3'"),
        ("prompt_template_hash", "?2"),
        ("transmission_kind", "'LOCAL_ONLY'"),
        ("redaction_policy_hash", "?3"),
        ("output_artifact_id", "?4"),
        ("started_at", "205"),
        ("cost_micros", "1250"),
        ("cost_currency", "'KRW'"),
        ("retention_declaration_id", "'ZERO_DAY'"),
        ("record_digest", "?5"),
    ];

    for (dropped, (dropped_column, _)) in COLUMNS.iter().enumerate() {
        let kept = COLUMNS
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != dropped)
            .map(|(_, entry)| *entry)
            .collect::<Vec<_>>();
        let statement = format!(
            "INSERT INTO model_run_provenance ({}) VALUES ({})",
            kept.iter()
                .map(|(column, _)| *column)
                .collect::<Vec<_>>()
                .join(", "),
            kept.iter()
                .map(|(_, value)| *value)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let attempted = connection.execute(
            &statement,
            params![
                model_run.to_vec(),
                vec![0x41_u8; 32],
                vec![0x42_u8; 32],
                synthetic_id(0x0500).to_vec(),
                record_digest("first").to_vec(),
            ],
        );
        assert!(
            attempted.is_err(),
            "a provenance row without {dropped_column} was accepted"
        );
    }

    // The whole row is accepted, so the loop above measures the missing column
    // and not a statement that could never have worked.
    insert_provenance(&connection, &model_run, "first", ("LOCAL_ONLY", None))?;
    Ok(())
}

/// A provenance row is the record its event authorized, and no other.
#[test]
fn model_run_provenance_is_bound_to_its_event_digest() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("authorized")?;
    let mut connection = database.open()?;
    three_model_runs(&mut connection)?;
    let model_run = synthetic_id(0x010c);

    // A record whose digest is not the event's `source_digest` is refused.
    let forged = insert_provenance(
        &connection,
        &model_run,
        "a record nobody signed for",
        ("LOCAL_ONLY", None),
    );
    assert!(
        forged.as_ref().err().is_some_and(|error| {
            error
                .to_string()
                .contains("not the record its event authorized")
        }),
        "an unauthorized provenance record was accepted: {forged:?}"
    );

    insert_provenance(&connection, &model_run, "first", ("LOCAL_ONLY", None))?;

    // A local-only run cannot carry a transmitted range.
    let smuggled = connection.execute(
        concat!(
            "INSERT INTO model_run_transmitted_range (model_run_id, range_ordinal, object_id, ",
            "byte_start, byte_end, content_digest) VALUES (?1, 0, 'synthetic-object', 10, 18, ?2)"
        ),
        params![model_run.to_vec(), vec![0x44_u8; 32]],
    );
    assert!(
        smuggled
            .as_ref()
            .err()
            .is_some_and(|error| error.to_string().contains("local-only model run")),
        "a local-only run recorded a transmitted range: {smuggled:?}"
    );
    Ok(())
}
