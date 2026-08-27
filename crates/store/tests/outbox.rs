use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
use academic_domain::{
    ArtifactDescriptor, ArtifactId, BatchId, ContentDigest, DeviceId, DomainError, DomainId, Event,
    EventId, EventPayload, ScopeDescriptor, ScopeId, TimestampMillis, UnsignedBatch,
};
use academic_ledger::EVENT_SCHEMA_VERSION;
use academic_store::{
    SealedObjectReceipt, SealedObjectVerifier,
    accept::{accept_verified_batch, accept_verified_batch_with_faults},
    connection::{open_reader, open_writer},
    fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault},
    idempotency::AcceptanceCommand,
    migration::migrate_pre_listen,
    outbox::read_outbox,
    queries::canonical_snapshot,
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
            "academic-s2-outbox-{label}-{}-{sequence}",
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
struct DummyReceipt {
    artifact_id: ArtifactId,
    content_digest: ContentDigest,
}

impl SealedObjectReceipt for DummyReceipt {
    fn artifact_id(&self) -> ArtifactId {
        self.artifact_id
    }

    fn content_digest(&self) -> ContentDigest {
        self.content_digest
    }
}

#[derive(Debug)]
struct EmptyGate;

#[derive(Debug)]
struct UnexpectedArtifact;

impl fmt::Display for UnexpectedArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scope-only batch unexpectedly referenced an artifact")
    }
}

impl Error for UnexpectedArtifact {}

impl SealedObjectVerifier for EmptyGate {
    type Receipt = DummyReceipt;
    type Error = UnexpectedArtifact;

    fn verify_sealed_object(
        &self,
        _descriptor: &ArtifactDescriptor,
    ) -> Result<Self::Receipt, Self::Error> {
        Err(UnexpectedArtifact)
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
struct PauseAtDb05 {
    reached: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl AcceptanceFaultInjector for PauseAtDb05 {
    fn hit(&self, point: AcceptanceFaultPoint) -> Result<(), InjectedFault> {
        if point == AcceptanceFaultPoint::Db05 {
            if self.reached.send(()).is_err() {
                return Err(InjectedFault { point });
            }
            let Ok(release) = self.release.lock() else {
                return Err(InjectedFault { point });
            };
            if release.recv().is_err() {
                return Err(InjectedFault { point });
            }
        }
        Ok(())
    }
}

#[test]
fn outbox_commits_with_batch() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("commit")?;
    let batch = scope_batch(0x100, 0x800)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.0, &signed.1)?;
    let mut writer = open_writer(database.path())?;
    let outcome = accept_verified_batch(
        &mut writer,
        &verified,
        command(&signed.0, 1),
        TimestampMillis::new(1_000),
        &EmptyGate,
    )?;
    drop(writer);

    let reader = open_reader(database.path())?;
    let entries = read_outbox(&reader)?;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].accepted_batch_id, batch.batch_id);
    assert_eq!(entries[0].accept_seq_start, 1);
    assert_eq!(entries[0].accept_seq_end, 1);
    assert_eq!(
        entries[0].canonical_revision,
        outcome.receipt.committed_revision
    );
    assert_eq!(entries[0].outbox_seq, outcome.receipt.committed_revision);
    assert_eq!(entries[0].event_kind_mask, 1_u64.to_be_bytes());
    assert_eq!(entries[0].payload_digest, verified.payload_hash());
    Ok(())
}

#[test]
fn outbox_never_leads_canonical_commit() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("rollback")?;
    let batch = scope_batch(0x200, 0x801)?;
    let signed = signed(&batch)?;
    let verified = verify_signed_batch(&signed.0, &signed.1)?;
    let mut writer = open_writer(database.path())?;
    assert!(
        accept_verified_batch_with_faults(
            &mut writer,
            &verified,
            command(&signed.0, 2),
            TimestampMillis::new(2_000),
            &EmptyGate,
            &FailAt(AcceptanceFaultPoint::Db05),
        )
        .is_err()
    );
    drop(writer);
    let reader = open_reader(database.path())?;
    let snapshot = canonical_snapshot(&reader)?;
    assert_eq!(snapshot.batch_count, 0);
    assert_eq!(snapshot.event_count, 0);
    assert_eq!(snapshot.outbox_count, 0);
    assert_eq!(snapshot.profile_revision, 0);
    assert!(read_outbox(&reader)?.is_empty());
    Ok(())
}

#[test]
fn readers_observe_before_or_after_not_partial() -> Result<(), Box<dyn Error>> {
    let database = TestDatabase::new("concurrency")?;
    let path = Arc::new(database.path().to_path_buf());
    let batch = scope_batch(0x300, 0x802)?;
    let signed = signed(&batch)?;
    let (reached_sender, reached_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();

    let writer_path = Arc::clone(&path);
    let writer = thread::spawn(move || -> Result<(), String> {
        let verified =
            verify_signed_batch(&signed.0, &signed.1).map_err(|error| error.to_string())?;
        let mut writer = open_writer(&writer_path).map_err(|error| error.to_string())?;
        accept_verified_batch_with_faults(
            &mut writer,
            &verified,
            command(&signed.0, 3),
            TimestampMillis::new(3_000),
            &EmptyGate,
            &PauseAtDb05 {
                reached: reached_sender,
                release: Mutex::new(release_receiver),
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    });
    reached_receiver.recv()?;

    let before_done = Arc::new(Barrier::new(3));
    let commit_done = Arc::new(Barrier::new(3));
    let mut readers = Vec::new();
    for _ in 0..2 {
        let reader_path = Arc::clone(&path);
        let before_done = Arc::clone(&before_done);
        let commit_done = Arc::clone(&commit_done);
        readers.push(thread::spawn(move || -> Result<(), String> {
            let reader = open_reader(&reader_path).map_err(|error| error.to_string())?;
            let before = canonical_snapshot(&reader).map_err(|error| error.to_string())?;
            if before.batch_count != 0
                || before.event_count != 0
                || before.outbox_count != 0
                || before.profile_revision != 0
            {
                return Err(format!("partial pre-commit snapshot: {before:?}"));
            }
            before_done.wait();
            commit_done.wait();
            let after = canonical_snapshot(&reader).map_err(|error| error.to_string())?;
            if after.batch_count != 1
                || after.event_count != 1
                || after.outbox_count != 1
                || after.profile_revision != 1
            {
                return Err(format!("partial post-commit snapshot: {after:?}"));
            }
            Ok(())
        }));
    }
    before_done.wait();
    release_sender.send(())?;
    writer.join().map_err(|_| "writer thread panicked")??;
    commit_done.wait();
    for reader in readers {
        reader.join().map_err(|_| "reader thread panicked")??;
    }
    Ok(())
}

fn scope_batch(namespace: u32, device: u32) -> Result<UnsignedBatch, DomainError> {
    let domain_id = id::<DomainId>(namespace + 1)?;
    let event = Event {
        id: id::<EventId>(namespace + 2)?,
        origin_seq: 1,
        origin_observed_at: TimestampMillis::new(100),
        actor: academic_domain::Actor::Importer {
            name: "academic.s2.outbox".to_owned(),
            version: "1.0.0".to_owned(),
        },
        domain_id,
        payload: EventPayload::ScopeRegistered(ScopeDescriptor {
            id: id::<ScopeId>(namespace + 3)?,
            domain_id,
            label: format!("outbox scope {namespace}"),
        }),
    };
    event.validate()?;
    Ok(UnsignedBatch {
        schema_version: EVENT_SCHEMA_VERSION,
        batch_id: id::<BatchId>(namespace + 4)?,
        device_id: id::<DeviceId>(device)?,
        origin_seq_start: 1,
        origin_seq_end: 1,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(100),
        events: vec![event],
    })
}

fn signed(batch: &UnsignedBatch) -> Result<(Vec<u8>, DeviceAuthorization), Box<dyn Error>> {
    let seed = [0x48_u8; 32];
    let signing_key = seed.as_slice().try_into()?;
    let envelope = sign_batch(batch, &signing_key)?;
    Ok((
        envelope,
        DeviceAuthorization::new(batch.device_id, id(0xff1)?, signing_key.verifying_key()),
    ))
}

fn command(envelope: &[u8], seed: u8) -> AcceptanceCommand<'_> {
    AcceptanceCommand {
        request_id: [seed; 16],
        client_instance_id: [seed.wrapping_add(1); 16],
        idempotency_key: [seed.wrapping_add(2); 32],
        expected_revision: Some(0),
        envelope_bytes: envelope,
    }
}

fn id<T>(suffix: u32) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}
