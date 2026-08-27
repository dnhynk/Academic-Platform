use std::{
    env,
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim,
    ClaimObject, Confidentiality, ContentDigest, DeviceId, DomainError, DomainId, EpistemicStatus,
    Event, EventId, EventPayload, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole,
    EvidenceStrength, MediaType, PermissionLineageId, PredicateId, RetentionClass, ScopeDescriptor,
    ScopeId, TimestampMillis, UnsignedBatch, ValidInterval, VaultLocator,
};
use academic_ledger::{EVENT_SCHEMA_VERSION, LedgerError, LedgerState};
use academic_store::{
    SealedObjectReceipt, SealedObjectVerifier,
    accept::{AcceptError, accept_verified_batch, accept_verified_batch_with_faults},
    connection::{open_reader, open_writer},
    fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault},
    idempotency::{AcceptanceCommand, IdempotencyError},
    migration::migrate_pre_listen,
    queries::{batch_material, canonical_snapshot},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TestDatabase {
    root: PathBuf,
    path: PathBuf,
}

impl TestDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-s2-acceptance-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let path = root.join("store.sqlite3");
        migrate_pre_listen(&path, [0x82; 32])?;
        Ok(Self { root, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TestReceipt {
    artifact_id: ArtifactId,
    content_digest: ContentDigest,
}

impl SealedObjectReceipt for TestReceipt {
    fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    fn content_digest(&self) -> ContentDigest {
        self.content_digest
    }
}

#[derive(Debug, Clone, Copy)]
enum GateMode {
    Accept,
    Reject,
    Mismatch,
}

#[derive(Debug, Clone, Copy)]
struct TestGate(GateMode);

#[derive(Debug)]
struct GateError;

impl fmt::Display for GateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic object is not sealed")
    }
}

impl Error for GateError {}

impl SealedObjectVerifier for TestGate {
    type Receipt = TestReceipt;
    type Error = GateError;

    fn verify_sealed_object(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<Self::Receipt, Self::Error> {
        match self.0 {
            GateMode::Accept => Ok(TestReceipt {
                artifact_id: descriptor.id,
                content_digest: descriptor.content_digest,
            }),
            GateMode::Reject => Err(GateError),
            GateMode::Mismatch => Ok(TestReceipt {
                artifact_id: descriptor.id,
                content_digest: ContentDigest::sha256(b"different object"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FailAt(AcceptanceFaultPoint);

impl AcceptanceFaultInjector for FailAt {
    fn hit(&self, point: AcceptanceFaultPoint) -> Result<(), InjectedFault> {
        if point == self.0 {
            Err(InjectedFault { point })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ProcessExitAt(AcceptanceFaultPoint);

impl AcceptanceFaultInjector for ProcessExitAt {
    fn hit(&self, point: AcceptanceFaultPoint) -> Result<(), InjectedFault> {
        if point == self.0 {
            std::process::exit(90 + fault_ordinal(point));
        }
        Ok(())
    }
}

struct SignedBatch {
    envelope: Vec<u8>,
    authorization: DeviceAuthorization,
}

#[test]
fn sql_acceptance_matches_pure_ledger() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("oracle")?;
    let batch = artifact_batch(0x100, 0x900)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
    let mut oracle = LedgerState::new();
    let pure = oracle.accept_verified_batch(&verified)?;

    let mut writer = open_writer(database.path())?;
    let outcome = accept_verified_batch(
        &mut writer,
        &verified,
        command(&signed.envelope, 1, Some(0)),
        TimestampMillis::new(1_000),
        &TestGate(GateMode::Accept),
    )?;
    assert_eq!(outcome.receipt.batch_id, pure.batch_id);
    assert_eq!(outcome.receipt.envelope_hash, pure.batch_hash);
    assert_eq!(outcome.receipt.accept_seq_start, pure.accept_seq_start);
    assert_eq!(outcome.receipt.accept_seq_end, pure.accept_seq_end);
    assert_eq!(outcome.receipt.committed_revision, 1);

    let reader = open_reader(database.path())?;
    let snapshot = canonical_snapshot(&reader)?;
    assert_eq!(snapshot.batch_count, 1);
    assert_eq!(snapshot.event_count, 4);
    assert_eq!(snapshot.next_accept_seq, 5);
    assert_eq!(snapshot.profile_revision, 1);
    let material = batch_material(&reader, batch.batch_id)?;
    assert_eq!(material.signed_envelope, signed.envelope);
    assert_eq!(material.deterministic_payload, verified.source_payload());
    assert_eq!(material.signature, verified.signature_bytes());
    Ok(())
}

#[test]
fn sql_batch_acceptance_is_atomic() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("atomic")?;
    let batch = artifact_batch(0x200, 0x901)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let result = accept_verified_batch_with_faults(
        &mut writer,
        &verified,
        command(&signed.envelope, 2, Some(0)),
        TimestampMillis::new(2_000),
        &TestGate(GateMode::Accept),
        &FailAt(AcceptanceFaultPoint::Db03),
    );
    assert!(matches!(result, Err(AcceptError::Injected(_))));
    drop(writer);

    let snapshot = canonical_snapshot(&open_reader(database.path())?)?;
    assert_empty(snapshot);
    Ok(())
}

#[test]
fn duplicate_batch_returns_original_receipt() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("duplicate")?;
    let batch = artifact_batch(0x300, 0x902)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let first = accept_verified_batch(
        &mut writer,
        &verified,
        command(&signed.envelope, 3, Some(0)),
        TimestampMillis::new(3_000),
        &TestGate(GateMode::Accept),
    )?;
    let duplicate = accept_verified_batch(
        &mut writer,
        &verified,
        command(&signed.envelope, 4, Some(1)),
        TimestampMillis::new(3_001),
        &TestGate(GateMode::Accept),
    )?;
    assert!(duplicate.duplicate_batch);
    assert_eq!(duplicate.receipt, first.receipt);
    assert_eq!(
        duplicate.receipt.response_bytes(),
        first.receipt.response_bytes()
    );
    drop(writer);

    let snapshot = canonical_snapshot(&open_reader(database.path())?)?;
    assert_eq!(snapshot.batch_count, 1);
    assert_eq!(snapshot.event_count, 4);
    assert_eq!(snapshot.profile_revision, 1);
    assert_eq!(snapshot.outbox_count, 1);
    assert_eq!(snapshot.receipt_count, 2);
    Ok(())
}

#[test]
fn idempotency_key_hash_collision_fails() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("key-collision")?;
    let batch = artifact_batch(0x400, 0x903)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let original = command(&signed.envelope, 5, Some(0));
    accept_verified_batch(
        &mut writer,
        &verified,
        original,
        TimestampMillis::new(4_000),
        &TestGate(GateMode::Accept),
    )?;
    let collision = AcceptanceCommand {
        request_id: [0x99; 16],
        client_instance_id: original.client_instance_id,
        idempotency_key: original.idempotency_key,
        expected_revision: original.expected_revision,
        envelope_bytes: original.envelope_bytes,
    };
    let result = accept_verified_batch(
        &mut writer,
        &verified,
        collision,
        TimestampMillis::new(4_001),
        &TestGate(GateMode::Accept),
    );
    assert!(matches!(
        result,
        Err(AcceptError::Idempotency(IdempotencyError::KeyCollision))
    ));
    drop(writer);
    let snapshot = canonical_snapshot(&open_reader(database.path())?)?;
    assert_eq!(snapshot.batch_count, 1);
    assert_eq!(snapshot.receipt_count, 1);
    assert_eq!(snapshot.profile_revision, 1);
    Ok(())
}

#[test]
fn expected_revision_conflict_has_no_effect() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("revision")?;
    let batch = scope_batch(0x500, 0x904, 1, None)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let result = accept_verified_batch(
        &mut writer,
        &verified,
        command(&signed.envelope, 6, Some(7)),
        TimestampMillis::new(5_000),
        &TestGate(GateMode::Accept),
    );
    assert!(matches!(
        result,
        Err(AcceptError::ExpectedRevisionConflict {
            expected: 7,
            actual: 0
        })
    ));
    drop(writer);
    assert_empty(canonical_snapshot(&open_reader(database.path())?)?);
    Ok(())
}

#[test]
fn accept_seq_is_contiguous_after_failed_batch() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("contiguous")?;
    let failed_batch = artifact_batch(0x600, 0x905)?;
    let failed_signed = signed(&failed_batch)?;
    let failed_verified =
        verify_signed_batch(&failed_signed.envelope, &failed_signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let result = accept_verified_batch_with_faults(
        &mut writer,
        &failed_verified,
        command(&failed_signed.envelope, 7, Some(0)),
        TimestampMillis::new(6_000),
        &TestGate(GateMode::Accept),
        &FailAt(AcceptanceFaultPoint::Db04),
    );
    assert!(result.is_err());

    let good_batch = scope_batch(0x700, 0x905, 1, None)?;
    let good_signed = signed(&good_batch)?;
    let good_verified = verify_signed_batch(&good_signed.envelope, &good_signed.authorization)?;
    let outcome = accept_verified_batch(
        &mut writer,
        &good_verified,
        command(&good_signed.envelope, 8, Some(0)),
        TimestampMillis::new(6_001),
        &TestGate(GateMode::Accept),
    )?;
    assert_eq!(outcome.receipt.accept_seq_start, 1);
    assert_eq!(outcome.receipt.accept_seq_end, 1);
    drop(writer);
    let snapshot = canonical_snapshot(&open_reader(database.path())?)?;
    assert_eq!(snapshot.next_accept_seq, 2);
    assert_eq!(snapshot.accept_seq_head, 1);
    Ok(())
}

#[test]
fn sql_device_gap_and_fork_consume_nothing() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("device-chain")?;
    let first_batch = scope_batch(0x710, 0x918, 1, None)?;
    let first_signed = signed(&first_batch)?;
    let first_verified = verify_signed_batch(&first_signed.envelope, &first_signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let first = accept_verified_batch(
        &mut writer,
        &first_verified,
        command(&first_signed.envelope, 50, Some(0)),
        TimestampMillis::new(6_100),
        &TestGate(GateMode::Accept),
    )?;

    let gap_batch = scope_batch(0x720, 0x918, 3, Some(first.receipt.envelope_hash))?;
    let gap_signed = signed(&gap_batch)?;
    let gap_verified = verify_signed_batch(&gap_signed.envelope, &gap_signed.authorization)?;
    assert!(matches!(
        accept_verified_batch(
            &mut writer,
            &gap_verified,
            command(&gap_signed.envelope, 51, Some(1)),
            TimestampMillis::new(6_101),
            &TestGate(GateMode::Accept),
        ),
        Err(AcceptError::Ledger(LedgerError::OriginGap {
            expected: 2,
            actual: 3
        }))
    ));

    let fork_batch = scope_batch(
        0x730,
        0x918,
        2,
        Some(ContentDigest::sha256(b"wrong parent")),
    )?;
    let fork_signed = signed(&fork_batch)?;
    let fork_verified = verify_signed_batch(&fork_signed.envelope, &fork_signed.authorization)?;
    assert!(matches!(
        accept_verified_batch(
            &mut writer,
            &fork_verified,
            command(&fork_signed.envelope, 52, Some(1)),
            TimestampMillis::new(6_102),
            &TestGate(GateMode::Accept),
        ),
        Err(AcceptError::Ledger(LedgerError::DeviceFork))
    ));

    drop(writer);
    let unchanged = canonical_snapshot(&open_reader(database.path())?)?;
    assert_eq!(unchanged.batch_count, 1);
    assert_eq!(unchanged.event_count, 1);
    assert_eq!(unchanged.receipt_count, 1);
    assert_eq!(unchanged.profile_revision, 1);
    assert_eq!(unchanged.next_accept_seq, 2);

    let good_batch = scope_batch(0x740, 0x918, 2, Some(first.receipt.envelope_hash))?;
    let good_signed = signed(&good_batch)?;
    let good_verified = verify_signed_batch(&good_signed.envelope, &good_signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    let good = accept_verified_batch(
        &mut writer,
        &good_verified,
        command(&good_signed.envelope, 53, Some(1)),
        TimestampMillis::new(6_103),
        &TestGate(GateMode::Accept),
    )?;
    assert_eq!(good.receipt.accept_seq_start, 2);
    assert_eq!(good.receipt.accept_seq_end, 2);
    Ok(())
}

#[test]
fn batch_id_collision_and_signed_i64_overflow_have_no_effect() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("collision-overflow")?;
    let batch = scope_batch(0x750, 0x919, 1, None)?;
    let initial_signed = signed(&batch)?;
    let verified = verify_signed_batch(&initial_signed.envelope, &initial_signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    accept_verified_batch(
        &mut writer,
        &verified,
        command(&initial_signed.envelope, 54, Some(0)),
        TimestampMillis::new(6_200),
        &TestGate(GateMode::Accept),
    )?;

    let mut collision_batch = batch.clone();
    let EventPayload::ScopeRegistered(scope) = &mut collision_batch.events[0].payload else {
        unreachable!();
    };
    scope.label.push_str(" changed");
    let collision_signed = signed(&collision_batch)?;
    let collision_verified =
        verify_signed_batch(&collision_signed.envelope, &collision_signed.authorization)?;
    assert!(matches!(
        accept_verified_batch(
            &mut writer,
            &collision_verified,
            command(&collision_signed.envelope, 55, Some(1)),
            TimestampMillis::new(6_201),
            &TestGate(GateMode::Accept),
        ),
        Err(AcceptError::Ledger(LedgerError::BatchIdCollision))
    ));
    drop(writer);
    let unchanged = canonical_snapshot(&open_reader(database.path())?)?;
    assert_eq!(unchanged.batch_count, 1);
    assert_eq!(unchanged.profile_revision, 1);

    let overflow_database = TestDatabase::new("signed-i64")?;
    let mut overflow_writer = open_writer(overflow_database.path())?;
    overflow_writer.execute(
        "UPDATE replica_state SET next_accept_seq = ?1 WHERE singleton = 1",
        [i64::MAX],
    )?;
    let overflow_batch = scope_batch(0x760, 0x920, 1, None)?;
    let overflow_signed = signed(&overflow_batch)?;
    let overflow_verified =
        verify_signed_batch(&overflow_signed.envelope, &overflow_signed.authorization)?;
    assert!(matches!(
        accept_verified_batch(
            &mut overflow_writer,
            &overflow_verified,
            command(&overflow_signed.envelope, 56, Some(0)),
            TimestampMillis::new(6_202),
            &TestGate(GateMode::Accept),
        ),
        Err(AcceptError::IntegerOverflow(_))
    ));
    drop(overflow_writer);
    let overflow_snapshot = canonical_snapshot(&open_reader(overflow_database.path())?)?;
    assert_eq!(overflow_snapshot.next_accept_seq, i64::MAX as u64);
    assert_eq!(overflow_snapshot.batch_count, 0);
    assert_eq!(overflow_snapshot.profile_revision, 0);
    Ok(())
}

#[test]
fn sealed_receipt_is_required_for_artifact_reference() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("sealed")?;
    let batch = artifact_batch(0x800, 0x906)?;
    let initial_signed = signed(&batch)?;
    let verified = verify_signed_batch(&initial_signed.envelope, &initial_signed.authorization)?;
    let mut writer = open_writer(database.path())?;
    for mode in [GateMode::Reject, GateMode::Mismatch] {
        let result = accept_verified_batch(
            &mut writer,
            &verified,
            command(&initial_signed.envelope, 9, Some(0)),
            TimestampMillis::new(7_000),
            &TestGate(mode),
        );
        assert!(result.is_err());
    }
    drop(writer);
    assert_empty(canonical_snapshot(&open_reader(database.path())?)?);

    let mut writer = open_writer(database.path())?;
    let accepted = accept_verified_batch(
        &mut writer,
        &verified,
        command(&initial_signed.envelope, 10, Some(0)),
        TimestampMillis::new(7_001),
        &TestGate(GateMode::Accept),
    )?;
    let reference = existing_evidence_claim_batch(
        0x900,
        0x907,
        batch.events[0].domain_id,
        match &batch.events[0].payload {
            EventPayload::ScopeRegistered(scope) => scope.id,
            _ => unreachable!(),
        },
        match &batch.events[2].payload {
            EventPayload::EvidenceRegistered(evidence) => evidence.id,
            _ => unreachable!(),
        },
    )?;
    let reference_signed = signed(&reference)?;
    let reference_verified =
        verify_signed_batch(&reference_signed.envelope, &reference_signed.authorization)?;
    let result = accept_verified_batch(
        &mut writer,
        &reference_verified,
        command(
            &reference_signed.envelope,
            11,
            Some(accepted.receipt.committed_revision),
        ),
        TimestampMillis::new(7_002),
        &TestGate(GateMode::Reject),
    );
    assert!(matches!(result, Err(AcceptError::SealingFailed { .. })));
    drop(writer);
    let snapshot = canonical_snapshot(&open_reader(database.path())?)?;
    assert_eq!(snapshot.batch_count, 1);
    assert_eq!(snapshot.event_count, 4);
    assert_eq!(snapshot.profile_revision, 1);
    assert_eq!(snapshot.next_accept_seq, 5);
    Ok(())
}

#[test]
fn db01_db07_process_exit_restart_matrix() -> Result<(), Box<dyn Error>> {
    for (index, point) in [
        AcceptanceFaultPoint::Db01,
        AcceptanceFaultPoint::Db02,
        AcceptanceFaultPoint::Db03,
        AcceptanceFaultPoint::Db04,
        AcceptanceFaultPoint::Db05,
        AcceptanceFaultPoint::Db06,
        AcceptanceFaultPoint::Db07,
    ]
    .into_iter()
    .enumerate()
    {
        let database = TestDatabase::new(point.as_str())?;
        let status = ProcessCommand::new(env::current_exe()?)
            .arg("--exact")
            .arg("process_fault_child")
            .arg("--nocapture")
            .env("ACADEMIC_S2_FAULT_CHILD", point.as_str())
            .env("ACADEMIC_S2_FAULT_DATABASE", database.path())
            .status()?;
        assert_eq!(
            status.code(),
            Some(90 + fault_ordinal(point)),
            "{} child must terminate at the requested checkpoint",
            point.as_str()
        );

        let restart = canonical_snapshot(&open_reader(database.path())?)?;
        if point == AcceptanceFaultPoint::Db07 {
            assert_eq!(restart.batch_count, 1);
            assert_eq!(restart.event_count, 4);
            assert_eq!(restart.outbox_count, 1);
            assert_eq!(restart.receipt_count, 1);
            assert_eq!(restart.profile_revision, 1);
            assert_eq!(restart.next_accept_seq, 5);
        } else {
            assert_empty(restart);
        }

        let batch = artifact_batch(0xa00 + u32::try_from(index)?, 0xb00 + u32::try_from(index)?)?;
        let signed = signed(&batch)?;
        let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
        let mut writer = open_writer(database.path())?;
        let retry = accept_verified_batch(
            &mut writer,
            &verified,
            command(&signed.envelope, 42, Some(0)),
            TimestampMillis::new(8_000),
            &TestGate(GateMode::Accept),
        )?;
        assert_eq!(
            retry.replayed_request,
            point == AcceptanceFaultPoint::Db07,
            "{} restart replay classification",
            point.as_str()
        );
        if point == AcceptanceFaultPoint::Db07 {
            let replay = accept_verified_batch(
                &mut writer,
                &verified,
                command(&signed.envelope, 42, Some(0)),
                TimestampMillis::new(8_001),
                &TestGate(GateMode::Accept),
            )?;
            assert!(replay.replayed_request);
            assert_eq!(
                replay.receipt.response_bytes(),
                retry.receipt.response_bytes()
            );
        }
        drop(writer);
        let final_snapshot = canonical_snapshot(&open_reader(database.path())?)?;
        assert_eq!(final_snapshot.batch_count, 1);
        assert_eq!(final_snapshot.event_count, 4);
        assert_eq!(final_snapshot.outbox_count, 1);
        assert_eq!(final_snapshot.receipt_count, 1);
        assert_eq!(final_snapshot.profile_revision, 1);
        assert_eq!(final_snapshot.next_accept_seq, 5);
    }
    Ok(())
}

#[test]
fn process_fault_child() -> Result<(), Box<dyn Error>> {
    let Ok(point_name) = env::var("ACADEMIC_S2_FAULT_CHILD") else {
        return Ok(());
    };
    let database_path = PathBuf::from(
        env::var_os("ACADEMIC_S2_FAULT_DATABASE")
            .ok_or("ACADEMIC_S2_FAULT_DATABASE must accompany ACADEMIC_S2_FAULT_CHILD")?,
    );
    let point = parse_db_fault(&point_name).ok_or("unknown DB fault point")?;
    let index = u32::try_from(fault_ordinal(point) - 1)?;
    let batch = artifact_batch(0xa00 + index, 0xb00 + index)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.envelope, &signed.authorization)?;
    let mut writer = open_writer(&database_path)?;
    let _ = accept_verified_batch_with_faults(
        &mut writer,
        &verified,
        command(&signed.envelope, 42, Some(0)),
        TimestampMillis::new(8_000),
        &TestGate(GateMode::Accept),
        &ProcessExitAt(point),
    )?;
    Err(format!("{} checkpoint was not reached", point.as_str()).into())
}

fn assert_empty(snapshot: academic_store::queries::CanonicalSnapshot) {
    assert_eq!(snapshot.next_accept_seq, 1);
    assert_eq!(snapshot.profile_revision, 0);
    assert_eq!(snapshot.batch_count, 0);
    assert_eq!(snapshot.event_count, 0);
    assert_eq!(snapshot.outbox_count, 0);
    assert_eq!(snapshot.receipt_count, 0);
    assert_eq!(snapshot.device_count, 0);
}

const fn fault_ordinal(point: AcceptanceFaultPoint) -> i32 {
    match point {
        AcceptanceFaultPoint::Db01 => 1,
        AcceptanceFaultPoint::Db02 => 2,
        AcceptanceFaultPoint::Db03 => 3,
        AcceptanceFaultPoint::Db04 => 4,
        AcceptanceFaultPoint::Db05 => 5,
        AcceptanceFaultPoint::Db06 => 6,
        AcceptanceFaultPoint::Db07 => 7,
        AcceptanceFaultPoint::Ipc02 => 8,
    }
}

fn parse_db_fault(value: &str) -> Option<AcceptanceFaultPoint> {
    match value {
        "DB01" => Some(AcceptanceFaultPoint::Db01),
        "DB02" => Some(AcceptanceFaultPoint::Db02),
        "DB03" => Some(AcceptanceFaultPoint::Db03),
        "DB04" => Some(AcceptanceFaultPoint::Db04),
        "DB05" => Some(AcceptanceFaultPoint::Db05),
        "DB06" => Some(AcceptanceFaultPoint::Db06),
        "DB07" => Some(AcceptanceFaultPoint::Db07),
        _ => None,
    }
}

fn signed(batch: &UnsignedBatch) -> Result<SignedBatch, Box<dyn Error>> {
    let seed = [0x37_u8; 32];
    let signing_key = seed.as_slice().try_into()?;
    let envelope = sign_batch(batch, &signing_key)?;
    let authorization =
        DeviceAuthorization::new(batch.device_id, id(0xff0)?, signing_key.verifying_key());
    Ok(SignedBatch {
        envelope,
        authorization,
    })
}

fn command(envelope: &[u8], seed: u8, expected_revision: Option<u64>) -> AcceptanceCommand<'_> {
    AcceptanceCommand {
        request_id: [seed; 16],
        client_instance_id: [seed.wrapping_add(1); 16],
        idempotency_key: [seed.wrapping_add(2); 32],
        expected_revision,
        envelope_bytes: envelope,
    }
}

fn artifact_batch(namespace: u32, device: u32) -> Result<UnsignedBatch, DomainError> {
    let domain_id: DomainId = id(namespace + 1)?;
    let scope_id: ScopeId = id(namespace + 2)?;
    let artifact_id: ArtifactId = id(namespace + 3)?;
    let evidence_id: EvidenceId = id(namespace + 4)?;
    let media_type = MediaType::parse("text/plain")?;
    let bytes = format!("SYNTHETIC S2 {namespace}").into_bytes();
    let digest = ContentDigest::sha256(&bytes);
    let length = u64::try_from(bytes.len()).map_err(|_| DomainError::InvalidRange)?;
    let locator = EvidenceLocator::TextBytes {
        source_digest: digest,
        start: 0,
        end: length,
    };
    let actor = Actor::Importer {
        name: "academic.s2.test".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let events = vec![
        event(
            namespace + 10,
            1,
            actor.clone(),
            domain_id,
            EventPayload::ScopeRegistered(ScopeDescriptor {
                id: scope_id,
                domain_id,
                label: format!("synthetic scope {namespace}"),
            }),
        )?,
        event(
            namespace + 11,
            2,
            actor.clone(),
            domain_id,
            EventPayload::ArtifactRegistered(ArtifactDescriptor {
                id: artifact_id,
                content_digest: digest,
                media_type,
                byte_length: length,
                domain_id,
                confidentiality: Confidentiality::Personal,
                retention_class: RetentionClass::UserManaged,
                permission_lineage_id: id::<PermissionLineageId>(namespace + 5)?,
                format_version: 1,
                vault_locator: VaultLocator::derive(
                    b"academic-s2-test-locator-key",
                    1,
                    &MediaType::parse("text/plain")?,
                    digest,
                )?,
                evidence_representations: vec![ArtifactRepresentation {
                    locator: locator.clone(),
                    content_digest: digest,
                    byte_length: length,
                }],
            }),
        )?,
        event(
            namespace + 12,
            3,
            actor.clone(),
            domain_id,
            EventPayload::EvidenceRegistered(EvidenceItem {
                id: evidence_id,
                artifact_id,
                locator,
                excerpt_digest: digest,
                role: EvidenceRole::Supports,
                strength: EvidenceStrength::Direct,
                extraction_method: "academic.s2.synthetic".to_owned(),
                extractor_version: "1.0.0".to_owned(),
            }),
        )?,
        event(
            namespace + 13,
            4,
            actor,
            domain_id,
            EventPayload::ClaimAsserted(Claim {
                id: id(namespace + 6)?,
                subject_entity_id: id(namespace + 7)?,
                predicate_id: PredicateId::parse("academic.synthetic")?,
                object: ClaimObject::Text(format!("value {namespace}")),
                scope_id,
                authority_class: AuthorityClass::Official,
                epistemic_status: EpistemicStatus::OfficialConfirmed,
                confidence: None,
                prediction_metadata: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
                evidence_ids: vec![evidence_id],
            }),
        )?,
    ];
    Ok(UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: id::<BatchId>(namespace + 20)?,
        device_id: id::<DeviceId>(device)?,
        origin_seq_start: 1,
        origin_seq_end: 4,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(100),
        events,
    })
}

fn scope_batch(
    namespace: u32,
    device: u32,
    origin_seq: u64,
    previous_batch_hash: Option<ContentDigest>,
) -> Result<UnsignedBatch, DomainError> {
    let domain_id = id::<DomainId>(namespace + 1)?;
    Ok(UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: id::<BatchId>(namespace + 20)?,
        device_id: id::<DeviceId>(device)?,
        origin_seq_start: origin_seq,
        origin_seq_end: origin_seq,
        previous_batch_hash,
        origin_created_at: TimestampMillis::new(100),
        events: vec![event(
            namespace + 10,
            origin_seq,
            Actor::Importer {
                name: "academic.s2.test".to_owned(),
                version: "1.0.0".to_owned(),
            },
            domain_id,
            EventPayload::ScopeRegistered(ScopeDescriptor {
                id: id::<ScopeId>(namespace + 2)?,
                domain_id,
                label: format!("scope {namespace}"),
            }),
        )?],
    })
}

fn existing_evidence_claim_batch(
    namespace: u32,
    device: u32,
    domain_id: DomainId,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
) -> Result<UnsignedBatch, DomainError> {
    Ok(UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: id::<BatchId>(namespace + 20)?,
        device_id: id::<DeviceId>(device)?,
        origin_seq_start: 1,
        origin_seq_end: 1,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(200),
        events: vec![event(
            namespace + 10,
            1,
            Actor::Importer {
                name: "academic.s2.test".to_owned(),
                version: "1.0.0".to_owned(),
            },
            domain_id,
            EventPayload::ClaimAsserted(Claim {
                id: id(namespace + 6)?,
                subject_entity_id: id(namespace + 7)?,
                predicate_id: PredicateId::parse("academic.synthetic")?,
                object: ClaimObject::Text("existing artifact closure".to_owned()),
                scope_id,
                authority_class: AuthorityClass::Official,
                epistemic_status: EpistemicStatus::OfficialConfirmed,
                confidence: None,
                prediction_metadata: None,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
                evidence_ids: vec![evidence_id],
            }),
        )?],
    })
}

fn event(
    suffix: u32,
    origin_seq: u64,
    actor: Actor,
    domain_id: DomainId,
    payload: EventPayload,
) -> Result<Event, DomainError> {
    let event = Event {
        id: id::<EventId>(suffix)?,
        origin_seq,
        origin_observed_at: TimestampMillis::new(100),
        actor,
        domain_id,
        payload,
    };
    event.validate()?;
    Ok(event)
}

fn id<T>(suffix: u32) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}

#[allow(dead_code)]
fn _ledger_error_anchor(_: LedgerError) {}
