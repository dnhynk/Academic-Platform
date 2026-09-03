//! Named acceptance evidence for migration 0009's proposal-review tables.
//!
//! The type half of `P2-M2` is in `crates/proposal`, where the doors are. What
//! that crate cannot observe is a writer that skips it: the tables here exist
//! on a migrated database and are reachable by any process holding the file, so
//! the tier-to-workflow mapping, the user-only rule and the append-only rule
//! each need a second enforcement layer that does not depend on the Rust
//! boundary having been used.
//!
//! The base is the one `aggregate_closure_tests` builds -- `0001`, the real
//! `0003`, then the aggregate migrations through `0009` -- so these rows run in
//! both lanes and against the real schema rather than something resembling it.

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, ContentDigest, DomainId, Event, EventPayload, ModelRunId, ModelRunRegistration,
    ProposalDispositionRegistration, ProposalId, ScopeDescriptor, ScopeId, TimestampMillis,
    ValidInterval,
};
use rusqlite::{Connection, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::apply_aggregate_migration_pre_listen,
    repository::ClosureWriter,
};

static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The four proposals every case here registers, one per section 27.4 tier.
///
/// Enumerated rather than counted, and the tier token is the one the migration
/// admits, so a tier renamed on one side fails against the other.
const PROPOSALS: [(u32, &str, &str); 4] = [
    (0x0201, "LOW_AUTOSAVE", "low"),
    (0x0202, "MEDIUM_REVIEW", "medium"),
    (0x0203, "HIGH_APPROVAL", "high"),
    (0x0204, "NON_DELEGABLE", "nondelegable"),
];

/// A fifth registered proposal that gets no review row.
///
/// It exists so the two foreign keys on `proposal_review` can be told apart. A
/// row for an unregistered proposal is refused by the reference to
/// `proposal_disposition`; a row for this one, whose digest is right, can only
/// be refused by the reference to `proposal_batching_policy`.
const SPARE: (u32, &str) = (0x0205, "spare");

/// The digest the `PROPOSAL_DISPOSED` event carries, which
/// `guard_proposal_review_authorized` compares a review row against.
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
            "academic-store-0009-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        let database = Self {
            path: root.join("proposal.sqlite3"),
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

/// Registers the model run the four proposals hang off, and the proposals.
fn four_proposals(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
    let domain_id: DomainId = typed_id(0x0001)?;
    let scope_id: ScopeId = typed_id(0x0002)?;
    let model_run_id: ModelRunId = typed_id(0x0200)?;
    let actor = Actor::Importer {
        name: "academic.m2.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let interval = ValidInterval::open_ended(TimestampMillis::new(100));

    let mut events = vec![
        Event {
            id: typed_id(0x0500)?,
            origin_seq: 1,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::ScopeRegistered(ScopeDescriptor {
                id: scope_id,
                domain_id,
                label: "synthetic M2 scope".to_owned(),
            }),
        },
        Event {
            id: typed_id(0x0501)?,
            origin_seq: 2,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::ModelRunRecorded(ModelRunRegistration {
                id: model_run_id,
                domain_id,
                scope_id,
                source_digest: Some(ContentDigest::sha256(b"m2 run")),
                valid_time: interval,
            }),
        },
    ];
    for (ordinal, (id, _, label)) in PROPOSALS.into_iter().enumerate() {
        let proposal_id: ProposalId = typed_id(id)?;
        events.push(Event {
            id: typed_id(0x0502 + u32::try_from(ordinal)?)?,
            origin_seq: u64::try_from(ordinal)? + 3,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload: EventPayload::ProposalDisposed(ProposalDispositionRegistration {
                id: proposal_id,
                model_run_id,
                domain_id,
                scope_id,
                source_digest: Some(ContentDigest::sha256(label.as_bytes())),
                valid_time: interval,
            }),
        });
    }

    events.push(Event {
        id: typed_id(0x0502 + u32::try_from(PROPOSALS.len())?)?,
        origin_seq: u64::try_from(PROPOSALS.len())? + 3,
        origin_observed_at: TimestampMillis::new(100),
        actor,
        domain_id,
        payload: EventPayload::ProposalDisposed(ProposalDispositionRegistration {
            id: typed_id(SPARE.0)?,
            model_run_id,
            domain_id,
            scope_id,
            source_digest: Some(ContentDigest::sha256(SPARE.1.as_bytes())),
            valid_time: interval,
        }),
    });

    let batch = academic_domain::UnsignedBatch {
        schema_version: academic_domain::EVENT_SCHEMA_VERSION,
        batch_id: typed_id(0x0600)?,
        device_id: typed_id(0x0601)?,
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

/// Inserts the shipped batching configuration.
fn seed_policy(connection: &Connection) -> Result<(), Box<dyn Error>> {
    connection.execute(
        "INSERT INTO proposal_batching_policy (thresholds_version, policy_digest, adopted_at) \
         VALUES (1, ?1, 100)",
        params![record_digest("batching v1").to_vec()],
    )?;
    for (axis, cuts) in [("CONFIDENCE", [500, 800]), ("IMPACT", [300, 700])] {
        for (ordinal, cut) in cuts.into_iter().enumerate() {
            connection.execute(
                "INSERT INTO proposal_batching_cut \
                 (thresholds_version, axis, cut_ordinal, cut_permille) VALUES (1, ?1, ?2, ?3)",
                params![axis, i64::try_from(ordinal)?, cut],
            )?;
        }
    }
    Ok(())
}

/// Inserts the four review rows, one per section 27.4 tier.
fn seed_reviews(connection: &Connection) -> Result<(), Box<dyn Error>> {
    for (id, tier, label) in PROPOSALS {
        connection.execute(
            "INSERT INTO proposal_review (proposal_id, risk_tier, confidence_permille, \
             impact_permille, subject_digest, thresholds_version, record_digest) \
             VALUES (?1, ?2, 900, 100, ?3, 1, ?4)",
            params![
                synthetic_id(id).to_vec(),
                tier,
                record_digest("subject").to_vec(),
                record_digest(label).to_vec(),
            ],
        )?;
    }
    Ok(())
}

fn migrated(label: &str) -> Result<(MigratedDatabase, Connection), Box<dyn Error>> {
    let database = MigratedDatabase::new(label)?;
    let mut connection = database.open()?;
    four_proposals(&mut connection)?;
    seed_policy(&connection)?;
    seed_reviews(&connection)?;
    Ok((database, connection))
}

/// The identifier of the proposal in `tier`.
///
/// Fallible rather than panicking: a tier name this file does not hold is a
/// mistake in the test, and a returned error names it instead of aborting the
/// process.
fn proposal_in(tier: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let (id, _, _) = PROPOSALS
        .into_iter()
        .find(|(_, candidate, _)| *candidate == tier)
        .ok_or_else(|| format!("{tier} is not one of the four tiers this file registers"))?;
    Ok(synthetic_id(id).to_vec())
}

/// Inserts one disposition record, returning whatever SQLite said.
#[allow(clippy::too_many_arguments)]
fn insert_disposition(
    connection: &Connection,
    proposal: &[u8],
    seq: i64,
    disposition: &str,
    actor_class: &str,
    explicit_approval: i64,
    supersedes: Option<i64>,
) -> rusqlite::Result<usize> {
    let replacement = (disposition == "REPLACE").then(|| synthetic_id(0x0700).to_vec());
    connection.execute(
        "INSERT INTO proposal_disposition_record (proposal_id, disposition_seq, disposition, \
         replacement_claim_id, actor_class, user_id, explicit_approval, decided_at, \
         supersedes_seq, record_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            proposal,
            seq,
            disposition,
            replacement,
            actor_class,
            synthetic_id(0x0800).to_vec(),
            explicit_approval,
            100 + seq,
            supersedes,
            record_digest(&format!("{disposition}-{seq}")).to_vec(),
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
fn a_review_row_needs_the_event_that_authorized_it() -> Result<(), Box<dyn Error>> {
    let database = MigratedDatabase::new("authorized")?;
    let mut connection = database.open()?;
    four_proposals(&mut connection)?;
    seed_policy(&connection)?;
    // A digest the event does not carry is refused, and the one it carries is
    // accepted, so the refusal is attributable to the digest.
    let forged = connection.execute(
        "INSERT INTO proposal_review (proposal_id, risk_tier, confidence_permille, \
         impact_permille, subject_digest, thresholds_version, record_digest) \
         VALUES (?1, 'MEDIUM_REVIEW', 900, 100, ?2, 1, ?3)",
        params![
            proposal_in("MEDIUM_REVIEW")?,
            record_digest("subject").to_vec(),
            record_digest("not the event digest").to_vec(),
        ],
    );
    let message = must_fail(forged, "an unauthorized review row was accepted")?;
    assert!(
        message.contains("proposal review is not the record its event authorized"),
        "unexpected message: {message}"
    );

    // A review row for a proposal no event registered is refused too. The
    // trigger reaches it first -- there is no `proposal_disposition` row to
    // match a digest against, so the authorization guard aborts before the
    // foreign key is evaluated -- which is why the message below is the
    // trigger's. The foreign key is the layer behind it, and
    // `the_batching_configuration_is_versioned_and_immutable` is where a
    // foreign key on this table is observed refusing on its own.
    let unregistered = connection.execute(
        "INSERT INTO proposal_review (proposal_id, risk_tier, confidence_permille, \
         impact_permille, subject_digest, thresholds_version, record_digest) \
         VALUES (?1, 'MEDIUM_REVIEW', 900, 100, ?2, 1, ?3)",
        params![
            synthetic_id(0x02ff).to_vec(),
            record_digest("subject").to_vec(),
            record_digest("unregistered").to_vec(),
        ],
    );
    let message = must_fail(unregistered, "a review row named an unregistered proposal")?;
    assert!(
        message.contains("proposal review is not the record its event authorized"),
        "unexpected message: {message}"
    );

    seed_reviews(&connection)?;
    Ok(())
}

#[test]
fn low_risk_autosave_persists_only_as_ai_inferred() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("autosave")?;
    let low = proposal_in("LOW_AUTOSAVE")?;

    // The autosave outcome is accepted.
    connection.execute(
        "INSERT INTO proposal_outcome (proposal_id, epistemic_status, disposition_seq, settled_at) \
         VALUES (?1, 'AI_INFERRED', NULL, 200)",
        params![low.clone()],
    )?;

    // Every other shape for that tier is refused: a different status, and an
    // outcome that claims a user decision.
    let database = MigratedDatabase::new("autosave-forged")?;
    let mut fresh = database.open()?;
    four_proposals(&mut fresh)?;
    seed_policy(&fresh)?;
    seed_reviews(&fresh)?;
    let confirmed = fresh.execute(
        "INSERT INTO proposal_outcome (proposal_id, epistemic_status, disposition_seq, settled_at) \
         VALUES (?1, 'USER_CONFIRMED', NULL, 200)",
        params![low.clone()],
    );
    let message = must_fail(
        confirmed,
        "a low-risk autosave was stored as USER_CONFIRMED",
    )?;
    assert!(
        message.contains("a low-risk autosave settles as AI_INFERRED with no disposition"),
        "unexpected message: {message}"
    );

    // And a low-risk proposal records no user decision at all, so there is no
    // sequence an outcome could name.
    let disposed = insert_disposition(&fresh, &low, 1, "CONFIRM", "USER", 0, None);
    let message = must_fail(disposed, "a low-risk autosave recorded a user decision")?;
    assert!(
        message.contains("a low-risk autosave records no user decision"),
        "unexpected message: {message}"
    );

    // The other three tiers cannot settle as AI_INFERRED, which is the same
    // rule read the other way.
    for tier in ["MEDIUM_REVIEW", "HIGH_APPROVAL", "NON_DELEGABLE"] {
        let inferred = fresh.execute(
            "INSERT INTO proposal_outcome (proposal_id, epistemic_status, disposition_seq, \
             settled_at) VALUES (?1, 'AI_INFERRED', NULL, 200)",
            params![proposal_in(tier)?],
        );
        let message = must_fail(inferred, "a reviewed proposal settled as AI_INFERRED")?;
        assert!(
            message.contains("a reviewed proposal settles as USER_CONFIRMED on an open CONFIRM"),
            "unexpected message for {tier}: {message}"
        );
    }
    Ok(())
}

#[test]
fn a_disposition_is_a_user_action() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("actor")?;
    // The three automatic actor classes of ADR-003's matrix, by the names the
    // ledger gives them. None of them can record a disposition.
    for actor_class in ["MODEL_RUN", "IMPORTER", "DETERMINISTIC_ENGINE"] {
        let forged = insert_disposition(
            &connection,
            &proposal_in("NON_DELEGABLE")?,
            1,
            "CONFIRM",
            actor_class,
            0,
            None,
        );
        let message = must_fail(forged, "an automatic actor recorded a disposition")?;
        assert!(
            message.contains("CHECK constraint failed")
                || message.contains("a proposal disposition is a user action"),
            "unexpected message for {actor_class}: {message}"
        );
    }
    // The user can, so the refusals above are about the actor and not about the
    // row being unacceptable for some other reason.
    insert_disposition(
        &connection,
        &proposal_in("NON_DELEGABLE")?,
        1,
        "CONFIRM",
        "USER",
        0,
        None,
    )?;
    connection.execute(
        "INSERT INTO proposal_outcome (proposal_id, epistemic_status, disposition_seq, settled_at) \
         VALUES (?1, 'USER_CONFIRMED', 1, 200)",
        params![proposal_in("NON_DELEGABLE")?],
    )?;
    Ok(())
}

#[test]
fn a_high_approval_confirmation_is_explicit() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("explicit")?;

    // Without the flag, the high-approval tier refuses the confirmation.
    let implicit = insert_disposition(
        &connection,
        &proposal_in("HIGH_APPROVAL")?,
        1,
        "CONFIRM",
        "USER",
        0,
        None,
    );
    let message = must_fail(implicit, "a high-risk proposal was confirmed implicitly")?;
    assert!(
        message.contains("a high-approval confirmation needs an explicit approval"),
        "unexpected message: {message}"
    );
    insert_disposition(
        &connection,
        &proposal_in("HIGH_APPROVAL")?,
        1,
        "CONFIRM",
        "USER",
        1,
        None,
    )?;

    // And the flag is refused on the two tiers that do not have that workflow,
    // so it is a property of the tier rather than a field anyone may set.
    for tier in ["MEDIUM_REVIEW", "NON_DELEGABLE"] {
        let borrowed = insert_disposition(
            &connection,
            &proposal_in(tier)?,
            1,
            "CONFIRM",
            "USER",
            1,
            None,
        );
        let message = must_fail(borrowed, "a queued proposal carried an explicit approval")?;
        assert!(
            message.contains("a high-approval confirmation needs an explicit approval"),
            "unexpected message for {tier}: {message}"
        );
        insert_disposition(
            &connection,
            &proposal_in(tier)?,
            1,
            "CONFIRM",
            "USER",
            0,
            None,
        )?;
    }
    Ok(())
}

#[test]
fn a_rejected_proposal_row_is_retained() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("retained")?;
    let medium = proposal_in("MEDIUM_REVIEW")?;
    insert_disposition(&connection, &medium, 1, "REJECT", "USER", 0, None)?;

    // Neither the record nor the review row can be edited or removed.
    for (statement, claim) in [
        (
            "UPDATE proposal_disposition_record SET disposition = 'CONFIRM' WHERE proposal_id = ?1",
            "a rejection was edited into a confirmation",
        ),
        (
            "DELETE FROM proposal_disposition_record WHERE proposal_id = ?1",
            "a rejection was deleted",
        ),
        (
            "UPDATE proposal_review SET risk_tier = 'LOW_AUTOSAVE' WHERE proposal_id = ?1",
            "a rejected proposal changed tier",
        ),
        (
            "DELETE FROM proposal_review WHERE proposal_id = ?1",
            "a rejected proposal was deleted",
        ),
    ] {
        let message = must_fail(
            connection.execute(statement, params![medium.clone()]),
            claim,
        )?;
        assert!(
            message.contains("canonical table is append-only"),
            "unexpected message: {message}"
        );
    }

    // An undo appends beside it and the rejection stays readable.
    insert_disposition(&connection, &medium, 2, "REJECT", "USER", 0, Some(1))?;
    insert_disposition(&connection, &medium, 3, "CONFIRM", "USER", 0, None)?;
    let history: Vec<(i64, String, Option<i64>)> = connection
        .prepare(
            "SELECT disposition_seq, disposition, supersedes_seq FROM proposal_disposition_record \
             WHERE proposal_id = ?1 ORDER BY disposition_seq",
        )?
        .query_map(params![medium.clone()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(
        history,
        vec![
            (1, "REJECT".to_owned(), None),
            (2, "REJECT".to_owned(), Some(1)),
            (3, "CONFIRM".to_owned(), None),
        ],
        "the rejection did not survive the decisions that followed it"
    );
    Ok(())
}

#[test]
fn an_undo_names_an_open_record_and_cannot_fork() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("undo")?;
    let medium = proposal_in("MEDIUM_REVIEW")?;
    insert_disposition(&connection, &medium, 1, "REJECT", "USER", 0, None)?;

    // An undo of a record that does not exist is refused.
    let missing = insert_disposition(&connection, &medium, 2, "REJECT", "USER", 0, Some(9));
    let message = must_fail(missing, "an undo named a record that does not exist")?;
    assert!(
        message.contains("an undo names an open earlier record of the same proposal"),
        "unexpected message: {message}"
    );

    // An undo of a record on another proposal is refused: the sequence exists,
    // but not for this proposal, so only the per-proposal clause refuses it.
    let other = proposal_in("NON_DELEGABLE")?;
    insert_disposition(&connection, &other, 1, "CONFIRM", "USER", 0, None)?;
    let crossed = insert_disposition(&connection, &other, 2, "CONFIRM", "USER", 0, Some(1));
    assert!(crossed.is_ok(), "an undo of its own record was refused");

    // A second undo of the same record is refused twice over: the guard sees
    // the record is no longer open, and the UNIQUE refuses the fork.
    insert_disposition(&connection, &medium, 2, "REJECT", "USER", 0, Some(1))?;
    let forked = insert_disposition(&connection, &medium, 3, "REJECT", "USER", 0, Some(1));
    let message = must_fail(forked, "one record was undone twice")?;
    assert!(
        message.contains("an undo names an open earlier record of the same proposal")
            || message.contains("UNIQUE constraint failed"),
        "unexpected message: {message}"
    );
    Ok(())
}

#[test]
fn the_batching_configuration_is_versioned_and_immutable() -> Result<(), Box<dyn Error>> {
    let (_database, connection) = migrated("batching")?;

    // A review row names the version it was banded under, and that version's
    // cuts cannot move afterwards.
    let version: i64 = connection.query_row(
        "SELECT thresholds_version FROM proposal_review WHERE proposal_id = ?1",
        params![proposal_in("MEDIUM_REVIEW")?],
        |row| row.get(0),
    )?;
    assert_eq!(version, 1);
    for (statement, claim) in [
        (
            "UPDATE proposal_batching_cut SET cut_permille = 600 WHERE thresholds_version = 1",
            "a band edge moved under a proposal that had been banded by it",
        ),
        (
            "DELETE FROM proposal_batching_cut WHERE thresholds_version = 1",
            "a band edge was deleted",
        ),
        (
            "UPDATE proposal_batching_policy SET adopted_at = 0 WHERE thresholds_version = 1",
            "a batching policy was edited",
        ),
    ] {
        let message = must_fail(connection.execute(statement, []), claim)?;
        assert!(
            message.contains("canonical table is append-only"),
            "unexpected message: {message}"
        );
    }

    // A new configuration is a new version beside the old one.
    connection.execute(
        "INSERT INTO proposal_batching_policy (thresholds_version, policy_digest, adopted_at) \
         VALUES (2, ?1, 300)",
        params![record_digest("batching v2").to_vec()],
    )?;
    let versions: Vec<i64> = connection
        .prepare("SELECT thresholds_version FROM proposal_batching_policy ORDER BY 1")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(versions, vec![1, 2]);

    // A review row cannot name a version nobody adopted. The proposal is the
    // registered spare and the digest is the one its event carries, so neither
    // the aggregate foreign key nor the digest guard can be what refuses this.
    let unadopted = connection.execute(
        "INSERT INTO proposal_review (proposal_id, risk_tier, confidence_permille, \
         impact_permille, subject_digest, thresholds_version, record_digest) \
         VALUES (?1, 'MEDIUM_REVIEW', 900, 100, ?2, 7, ?3)",
        params![
            synthetic_id(SPARE.0).to_vec(),
            record_digest("subject").to_vec(),
            record_digest(SPARE.1).to_vec(),
        ],
    );
    let message = must_fail(
        unadopted,
        "a review row named an unadopted batching version",
    )?;
    assert!(
        message.contains("FOREIGN KEY constraint failed"),
        "unexpected message: {message}"
    );
    // The control: the same row under an adopted version is accepted.
    connection.execute(
        "INSERT INTO proposal_review (proposal_id, risk_tier, confidence_permille, \
         impact_permille, subject_digest, thresholds_version, record_digest) \
         VALUES (?1, 'MEDIUM_REVIEW', 900, 100, ?2, 2, ?3)",
        params![
            synthetic_id(SPARE.0).to_vec(),
            record_digest("subject").to_vec(),
            record_digest(SPARE.1).to_vec(),
        ],
    )?;
    Ok(())
}
