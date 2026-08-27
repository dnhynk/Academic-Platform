//! Composition root for authenticated durable acceptance.

use std::{error::Error, fmt};

use academic_contracts::{ContractError, DeviceAuthorization, verify_signed_batch};
use academic_domain::TimestampMillis;
use academic_store::{
    SealedObjectVerifier,
    accept::{
        AcceptError, AcceptanceOutcome, accept_verified_batch, accept_verified_batch_with_faults,
    },
    connection::WriterConnection,
    fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault},
    idempotency::AcceptanceCommand,
};

/// Authentication or durable-store failure at the local core boundary.
#[derive(Debug)]
pub enum ServiceError {
    Contract(ContractError),
    Acceptance(AcceptError),
    Injected(InjectedFault),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => write!(formatter, "signed acceptance rejected: {error}"),
            Self::Acceptance(error) => write!(formatter, "durable acceptance rejected: {error}"),
            Self::Injected(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Acceptance(error) => Some(error),
            Self::Injected(error) => Some(error),
        }
    }
}

impl From<ContractError> for ServiceError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<AcceptError> for ServiceError {
    fn from(error: AcceptError) -> Self {
        Self::Acceptance(error)
    }
}

impl From<InjectedFault> for ServiceError {
    fn from(error: InjectedFault) -> Self {
        Self::Injected(error)
    }
}

/// Verifies canonical bytes/signature, seals every referenced artifact, and
/// commits the resulting batch through the one synchronous writer.
pub fn accept_signed_command<V: SealedObjectVerifier>(
    writer: &mut WriterConnection,
    command: AcceptanceCommand<'_>,
    authorization: &DeviceAuthorization,
    accepted_at: TimestampMillis,
    sealed_objects: &V,
) -> Result<AcceptanceOutcome, ServiceError> {
    let verified = verify_signed_batch(command.envelope_bytes, authorization)?;
    Ok(accept_verified_batch(
        writer,
        &verified,
        command,
        accepted_at,
        sealed_objects,
    )?)
}

/// Test-harness composition with DB01-DB07 and IPC02 checkpoints.
pub fn accept_signed_command_with_faults<V, F>(
    writer: &mut WriterConnection,
    command: AcceptanceCommand<'_>,
    authorization: &DeviceAuthorization,
    accepted_at: TimestampMillis,
    sealed_objects: &V,
    faults: &F,
) -> Result<AcceptanceOutcome, ServiceError>
where
    V: SealedObjectVerifier,
    F: AcceptanceFaultInjector,
{
    let verified = verify_signed_batch(command.envelope_bytes, authorization)?;
    let outcome = accept_verified_batch_with_faults(
        writer,
        &verified,
        command,
        accepted_at,
        sealed_objects,
        faults,
    )?;
    faults.hit(AcceptanceFaultPoint::Ipc02)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        error::Error,
        fmt, fs,
        path::PathBuf,
        process::Command as ProcessCommand,
        str::FromStr,
        sync::atomic::{AtomicU64, Ordering},
    };

    use academic_domain::{
        Actor, ArtifactDescriptor, ArtifactId, BatchId, ContentDigest, DeviceId, DomainError,
        DomainId, EntityId, Event, EventId, EventPayload, ScopeDescriptor, ScopeId, UnsignedBatch,
    };
    use academic_ledger::EVENT_SCHEMA_VERSION;
    use academic_store::{
        SealedObjectReceipt,
        connection::{open_reader, open_writer},
        fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault},
        migration::migrate_pre_listen,
        queries::canonical_snapshot,
    };
    use ed25519_dalek::SigningKey;

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug)]
    struct TemporaryDatabase {
        root: PathBuf,
        path: PathBuf,
    }

    impl TemporaryDatabase {
        fn new() -> Result<Self, Box<dyn Error>> {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "academic-s2-ipc02-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root)?;
            let path = root.join("store.sqlite3");
            migrate_pre_listen(&path, [0x82; 32])?;
            Ok(Self { root, path })
        }
    }

    impl Drop for TemporaryDatabase {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_dir_all(&self.root) {
                eprintln!("test cleanup failed for {}: {error}", self.root.display());
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct DummyReceipt {
        artifact_id: ArtifactId,
        digest: ContentDigest,
    }

    impl SealedObjectReceipt for DummyReceipt {
        fn artifact_id(&self) -> ArtifactId {
            self.artifact_id
        }

        fn content_digest(&self) -> ContentDigest {
            self.digest
        }
    }

    #[derive(Debug)]
    struct EmptyGate;

    #[derive(Debug)]
    struct UnexpectedArtifact;

    impl fmt::Display for UnexpectedArtifact {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("scope-only IPC fixture has no artifact")
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
    struct ExitAtIpc02;

    impl AcceptanceFaultInjector for ExitAtIpc02 {
        fn hit(&self, point: AcceptanceFaultPoint) -> Result<(), InjectedFault> {
            if point == AcceptanceFaultPoint::Ipc02 {
                std::process::exit(98);
            }
            Ok(())
        }
    }

    #[test]
    fn ipc02_process_exit_restart_replays_exact_receipt() -> Result<(), Box<dyn Error>> {
        let database = TemporaryDatabase::new()?;
        let status = ProcessCommand::new(env::current_exe()?)
            .arg("--exact")
            .arg("service::tests::ipc02_fault_child")
            .arg("--nocapture")
            .env("ACADEMIC_S2_IPC02_CHILD", "1")
            .env("ACADEMIC_S2_IPC02_DATABASE", &database.path)
            .status()?;
        assert_eq!(status.code(), Some(98));

        let restart = canonical_snapshot(&open_reader(&database.path)?)?;
        assert_eq!(restart.batch_count, 1);
        assert_eq!(restart.event_count, 1);
        assert_eq!(restart.outbox_count, 1);
        assert_eq!(restart.receipt_count, 1);
        assert_eq!(restart.profile_revision, 1);
        assert_eq!(restart.next_accept_seq, 2);

        let (batch, envelope, authorization) = signed_scope_batch()?;
        let mut writer = open_writer(&database.path)?;
        let replay = accept_signed_command(
            &mut writer,
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(1_001),
            &EmptyGate,
        )?;
        assert!(replay.replayed_request);
        assert_eq!(replay.receipt.batch_id, batch.batch_id);
        let exact = replay.receipt.response_bytes().to_vec();
        let second = accept_signed_command(
            &mut writer,
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(1_002),
            &EmptyGate,
        )?;
        assert_eq!(second.receipt.response_bytes(), exact);
        Ok(())
    }

    #[test]
    fn ipc02_fault_child() -> Result<(), Box<dyn Error>> {
        if env::var_os("ACADEMIC_S2_IPC02_CHILD").is_none() {
            return Ok(());
        }
        let path = PathBuf::from(
            env::var_os("ACADEMIC_S2_IPC02_DATABASE")
                .ok_or("ACADEMIC_S2_IPC02_DATABASE is required")?,
        );
        let (_batch, envelope, authorization) = signed_scope_batch()?;
        let mut writer = open_writer(&path)?;
        let _ = accept_signed_command_with_faults(
            &mut writer,
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(1_000),
            &EmptyGate,
            &ExitAtIpc02,
        )?;
        Err("IPC02 checkpoint was not reached".into())
    }

    fn signed_scope_batch() -> Result<(UnsignedBatch, Vec<u8>, DeviceAuthorization), Box<dyn Error>>
    {
        let domain_id = id::<DomainId>(1)?;
        let event = Event {
            id: id::<EventId>(2)?,
            origin_seq: 1,
            origin_observed_at: TimestampMillis::new(100),
            actor: Actor::Importer {
                name: "academic.s2.ipc".to_owned(),
                version: "1.0.0".to_owned(),
            },
            domain_id,
            payload: EventPayload::ScopeRegistered(ScopeDescriptor {
                id: id::<ScopeId>(3)?,
                domain_id,
                label: "IPC02 synthetic scope".to_owned(),
            }),
        };
        event.validate()?;
        let batch = UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: id::<BatchId>(4)?,
            device_id: id::<DeviceId>(5)?,
            origin_seq_start: 1,
            origin_seq_end: 1,
            previous_batch_hash: None,
            origin_created_at: TimestampMillis::new(100),
            events: vec![event],
        };
        let signing_key = SigningKey::from_bytes(&[0x6a; 32]);
        let envelope = academic_contracts::sign_batch(&batch, &signing_key)?;
        let authorization = DeviceAuthorization::new(
            batch.device_id,
            id::<EntityId>(6)?,
            signing_key.verifying_key(),
        );
        Ok((batch, envelope, authorization))
    }

    fn acceptance_command(envelope: &[u8]) -> AcceptanceCommand<'_> {
        AcceptanceCommand {
            request_id: [1; 16],
            client_instance_id: [2; 16],
            idempotency_key: [3; 32],
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
}
