//! Composition root for authenticated durable acceptance.

use std::{error::Error, fmt};

use academic_contracts::{ContractError, DeviceAuthorization, verify_signed_batch};
use academic_domain::TimestampMillis;
use academic_store::{
    accept::{AcceptError, AcceptanceOutcome, AcceptanceStore},
    error::StoreError,
    fault::InjectedFault,
    idempotency::AcceptanceCommand,
    profile::SyntheticProfile,
};
use academic_vault::{DomainKeyring, Vault, VaultError};

#[cfg(test)]
use academic_store::fault::{AcceptanceFaultInjector, AcceptanceFaultPoint};

/// Authentication or durable-store failure at the local core boundary.
#[derive(Debug)]
pub enum ServiceError {
    Contract(ContractError),
    Store(StoreError),
    Vault(VaultError),
    Acceptance(AcceptError),
    Injected(InjectedFault),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => write!(formatter, "signed acceptance rejected: {error}"),
            Self::Store(error) => write!(formatter, "acceptance store could not open: {error}"),
            Self::Vault(error) => write!(formatter, "acceptance vault could not open: {error}"),
            Self::Acceptance(error) => write!(formatter, "durable acceptance rejected: {error}"),
            Self::Injected(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Vault(error) => Some(error),
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

impl From<StoreError> for ServiceError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<VaultError> for ServiceError {
    fn from(error: VaultError) -> Self {
        Self::Vault(error)
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

/// Single-owner local composition of the concrete vault and the only canonical writer.
///
/// This service exposes artifact ingest/read-back and signed acceptance, but no raw SQLite
/// connection, arbitrary SQL method, verifier trait, receipt constructor, or second writer alias.
pub struct AcceptanceService {
    store: AcceptanceStore,
    vault: Vault,
}

impl fmt::Debug for AcceptanceService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptanceService")
            .field("store", &self.store)
            .field("vault", &self.vault)
            .finish_non_exhaustive()
    }
}

impl AcceptanceService {
    /// Opens one concrete vault and one owned acceptance writer for the same validated profile.
    pub fn open(profile: &SyntheticProfile, keyring: DomainKeyring) -> Result<Self, ServiceError> {
        let vault = Vault::open(profile.root(), keyring)?;
        let store = profile.open_acceptance_store()?;
        Ok(Self { store, vault })
    }

    /// Returns the concrete vault used to ingest exact bytes before signed acceptance.
    #[must_use]
    pub const fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Verifies canonical source bytes/signature and commits one atomic acceptance unit.
    pub fn accept_signed_command(
        &mut self,
        command: AcceptanceCommand<'_>,
        authorization: &DeviceAuthorization,
        accepted_at: TimestampMillis,
    ) -> Result<AcceptanceOutcome, ServiceError> {
        let verified = verify_signed_batch(command.envelope_bytes, authorization)?;
        Ok(self
            .store
            .accept_verified_batch(&verified, command, accepted_at, &self.vault)?)
    }

    #[cfg(test)]
    fn accept_signed_command_with_faults<F>(
        &mut self,
        command: AcceptanceCommand<'_>,
        authorization: &DeviceAuthorization,
        accepted_at: TimestampMillis,
        faults: &F,
    ) -> Result<AcceptanceOutcome, ServiceError>
    where
        F: AcceptanceFaultInjector,
    {
        let verified = verify_signed_batch(command.envelope_bytes, authorization)?;
        let outcome = self.store.accept_verified_batch_with_faults(
            &verified,
            command,
            accepted_at,
            &self.vault,
            faults,
        )?;
        faults.hit(AcceptanceFaultPoint::Ipc02)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        error::Error,
        fs,
        path::PathBuf,
        process::Command as ProcessCommand,
        str::FromStr,
        sync::atomic::{AtomicU64, Ordering},
    };

    use academic_domain::{
        Actor, BatchId, DeviceId, DomainError, DomainId, EntityId, Event, EventId, EventPayload,
        ScopeDescriptor, ScopeId, UnsignedBatch,
    };
    use academic_ledger::EVENT_SCHEMA_VERSION;
    use academic_store::{
        connection::open_reader,
        fault::{AcceptanceFaultInjector, AcceptanceFaultPoint, InjectedFault},
        path_policy::NativePathProbe,
        profile::{SyntheticProfile, create_synthetic_profile, open_synthetic_profile},
        queries::canonical_snapshot,
    };
    use academic_vault::ArtifactIngestRequest;
    use ed25519_dalek::SigningKey;

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug)]
    struct TemporaryDatabase {
        root: PathBuf,
        profile: SyntheticProfile,
    }

    impl TemporaryDatabase {
        fn new() -> Result<Self, Box<dyn Error>> {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "academic-s2-ipc02-{}-{sequence}",
                std::process::id()
            ));
            let profile = create_synthetic_profile(&root, &NativePathProbe::default(), [0x82; 32])?;
            Ok(Self { root, profile })
        }
    }

    impl Drop for TemporaryDatabase {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_dir_all(&self.root) {
                eprintln!("test cleanup failed for {}: {error}", self.root.display());
            }
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
            .env("ACADEMIC_S2_IPC02_PROFILE", database.profile.root())
            .status()?;
        assert_eq!(status.code(), Some(98));

        let restart = canonical_snapshot(&open_reader(database.profile.database_path())?)?;
        assert_eq!(restart.batch_count, 1);
        assert_eq!(restart.event_count, 1);
        assert_eq!(restart.outbox_count, 1);
        assert_eq!(restart.receipt_count, 1);
        assert_eq!(restart.profile_revision, 1);
        assert_eq!(restart.next_accept_seq, 2);

        let (batch, envelope, authorization) = signed_scope_batch()?;
        let mut service = AcceptanceService::open(&database.profile, DomainKeyring::new())?;
        let replay = service.accept_signed_command(
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(1_001),
        )?;
        assert!(replay.replayed_request);
        assert_eq!(replay.receipt.batch_id, batch.batch_id);
        let exact = replay.receipt.response_bytes().to_vec();
        let second = service.accept_signed_command(
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(1_002),
        )?;
        assert_eq!(second.receipt.response_bytes(), exact);
        Ok(())
    }

    #[test]
    fn ipc02_fault_child() -> Result<(), Box<dyn Error>> {
        if env::var_os("ACADEMIC_S2_IPC02_CHILD").is_none() {
            return Ok(());
        }
        let root = PathBuf::from(
            env::var_os("ACADEMIC_S2_IPC02_PROFILE")
                .ok_or("ACADEMIC_S2_IPC02_PROFILE is required")?,
        );
        let (_batch, envelope, authorization) = signed_scope_batch()?;
        let profile = open_synthetic_profile(&root, &NativePathProbe::default())?;
        let mut service = AcceptanceService::open(&profile, DomainKeyring::new())?;
        let _ = service.accept_signed_command_with_faults(
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(1_000),
            &ExitAtIpc02,
        )?;
        Err("IPC02 checkpoint was not reached".into())
    }

    #[test]
    fn core_acceptance_with_real_vault_bytes_rejects_missing_object() -> Result<(), Box<dyn Error>>
    {
        let database = TemporaryDatabase::new()?;
        let document = crate::build_fixture_document()?;
        let envelope = hex::decode(&document.signed_batch_cbor_hex)?;
        let authorization = crate::fixture_device_authorization()?;
        let verified = verify_signed_batch(&envelope, &authorization)?;
        let descriptor = verified
            .batch()
            .events
            .iter()
            .find_map(|event| match &event.payload {
                EventPayload::ArtifactRegistered(descriptor) => Some(descriptor.clone()),
                _ => None,
            })
            .ok_or("fixture must register an artifact")?;
        let mut keyring = DomainKeyring::new();
        keyring.insert(descriptor.domain_id, b"phase0-synthetic-domain-locator-key")?;
        let mut service = AcceptanceService::open(&database.profile, keyring)?;

        let rejected = service.accept_signed_command(
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(2_000),
        );
        assert!(matches!(
            rejected,
            Err(ServiceError::Acceptance(AcceptError::SealingFailed { .. }))
        ));
        let empty = canonical_snapshot(&open_reader(database.profile.database_path())?)?;
        assert_eq!(empty.batch_count, 0);
        assert_eq!(empty.event_count, 0);
        assert_eq!(empty.outbox_count, 0);
        assert_eq!(empty.profile_revision, 0);

        let request = ArtifactIngestRequest::new(
            descriptor.id,
            descriptor.media_type.clone(),
            descriptor.domain_id,
            descriptor.confidentiality,
            descriptor.retention_class,
            descriptor.permission_lineage_id,
        );
        let receipt = service
            .vault()
            .ingest(&request, crate::SYNTHETIC_ARTIFACT_BYTES)?;
        assert_eq!(
            receipt.descriptor().content_digest,
            descriptor.content_digest
        );
        let accepted = service.accept_signed_command(
            acceptance_command(&envelope),
            &authorization,
            TimestampMillis::new(2_001),
        )?;
        assert!(!accepted.replayed_request);
        let committed = canonical_snapshot(&open_reader(database.profile.database_path())?)?;
        assert_eq!(committed.batch_count, 1);
        assert_eq!(committed.event_count, 14);
        assert_eq!(committed.outbox_count, 1);
        assert_eq!(committed.profile_revision, 1);
        Ok(())
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
