//! Ordinary persistent/add-only broker model, with a separate durable caller
//! journal. Discarding all creator state models interruption; these are not
//! native process-crash, filesystem power-loss, or macOS acceptance evidence.

use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::Write as _,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

use academic_crypto::{
    DeviceKeystore, IDENTIFIER_BYTES, KeystoreFailure, PreparedDeviceSeal, ProfileId,
    PublicationJournalFailure, RecipientRecord, RecoverableDeviceKeystore, UnlockError,
    VaultMasterKey, create_device_recipient, create_recoverable_device_recipient,
    purge_incomplete_device_recipient, unlock_with_device,
};
use zeroize::Zeroizing;

const PROFILE: ProfileId = ProfileId::from_bytes([0x26; IDENTIFIER_BYTES]);
const RECIPIENT: [u8; IDENTIFIER_BYTES] = [0x68; IDENTIFIER_BYTES];
const LABEL: &str = "academic-os:test:publication";

#[derive(Debug, Clone, Copy, Default)]
enum AddMode {
    #[default]
    Success,
    RefuseBeforeAdd,
    ErrorAfterAdd,
    InterruptAfterAdd,
}

#[derive(Debug)]
struct PersistentBroker {
    directory: PathBuf,
    mode: AddMode,
    refuse_cleanup: bool,
    provider: &'static str,
}

impl PersistentBroker {
    fn reconnect(directory: &Path) -> Self {
        Self {
            directory: directory.to_owned(),
            mode: AddMode::Success,
            refuse_cleanup: false,
            provider: "TEST_ADD_ONLY_PERSISTENT",
        }
    }

    fn item(&self, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure> {
        let bytes = fs::read(self.directory.join("item")).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                KeystoreFailure::NotFound
            } else {
                KeystoreFailure::Unavailable
            }
        })?;
        let bytes = Zeroizing::new(bytes);
        if bytes.len() != 64 || bytes.get(..32) != Some(blob) {
            return Err(KeystoreFailure::NotFound);
        }
        Ok(bytes)
    }
}

impl DeviceKeystore for PersistentBroker {
    fn provider(&self) -> &str {
        self.provider
    }

    fn requires_publication_journal(&self) -> bool {
        true
    }

    fn seal(&self, _: &str, _: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        unreachable!("the legacy helper must refuse before calling seal")
    }

    fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure> {
        if label != LABEL {
            return Err(KeystoreFailure::InvalidBlob);
        }
        Ok(Zeroizing::new(self.item(blob)?[32..].to_vec()))
    }
}

struct PreparedItem {
    directory: PathBuf,
    generation: [u8; 32],
    mode: AddMode,
}

impl std::fmt::Debug for PreparedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedItem(<redacted>)")
    }
}

impl PreparedDeviceSeal for PreparedItem {
    fn blob(&self) -> &[u8] {
        &self.generation
    }

    fn seal(self, secret: &[u8]) -> Result<(), KeystoreFailure> {
        assert_eq!(secret.len(), 32);
        // The broker observes a complete, recoverable recipient BEFORE add.
        let staged = read_incomplete(&self.directory).map_err(|_| KeystoreFailure::Unavailable)?;
        assert_eq!(staged.keystore_blob(), &self.generation);
        if matches!(self.mode, AddMode::RefuseBeforeAdd) {
            return Err(KeystoreFailure::AccessDenied);
        }
        let mut item = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.directory.join("item"))
            .map_err(|_| KeystoreFailure::Unavailable)?;
        item.write_all(&self.generation)
            .map_err(|_| KeystoreFailure::Unavailable)?;
        item.write_all(secret)
            .map_err(|_| KeystoreFailure::Unavailable)?;
        item.sync_all().map_err(|_| KeystoreFailure::Unavailable)?;
        assert!(
            !matches!(self.mode, AddMode::InterruptAfterAdd),
            "modeled interruption after persistent add"
        );
        match self.mode {
            AddMode::ErrorAfterAdd => Err(KeystoreFailure::Unavailable),
            AddMode::Success | AddMode::RefuseBeforeAdd | AddMode::InterruptAfterAdd => Ok(()),
        }
    }
}

impl RecoverableDeviceKeystore for PersistentBroker {
    type PreparedSeal = PreparedItem;

    fn prepare_seal(&self, label: &str) -> Result<Self::PreparedSeal, KeystoreFailure> {
        if label != LABEL {
            return Err(KeystoreFailure::InvalidBlob);
        }
        let mut generation = [0; 32];
        getrandom::fill(&mut generation).map_err(|_| KeystoreFailure::Unavailable)?;
        Ok(PreparedItem {
            directory: self.directory.clone(),
            generation,
            mode: self.mode,
        })
    }

    fn purge_incomplete(&self, label: &str, blob: &[u8]) -> Result<bool, KeystoreFailure> {
        if label != LABEL || blob.len() != 32 {
            return Err(KeystoreFailure::InvalidBlob);
        }
        if self.refuse_cleanup {
            return Err(KeystoreFailure::AccessDenied);
        }
        match self.item(blob) {
            Ok(_) => {
                fs::remove_file(self.directory.join("item"))
                    .map_err(|_| KeystoreFailure::Unavailable)?;
                Ok(true)
            }
            Err(KeystoreFailure::NotFound) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn persist(directory: &Path, record: &RecipientRecord) -> Result<(), PublicationJournalFailure> {
    let bytes = record
        .to_canonical_cbor()
        .map_err(|_| PublicationJournalFailure)?;
    let mut journal = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("incomplete.cbor"))
        .map_err(|_| PublicationJournalFailure)?;
    journal
        .write_all(&bytes)
        .map_err(|_| PublicationJournalFailure)?;
    journal.sync_all().map_err(|_| PublicationJournalFailure)
}

fn read_incomplete(directory: &Path) -> Result<RecipientRecord, Box<dyn Error>> {
    Ok(RecipientRecord::from_canonical_cbor(&fs::read(
        directory.join("incomplete.cbor"),
    )?)?)
}

fn create(
    broker: &PersistentBroker,
    master: &VaultMasterKey,
) -> Result<RecipientRecord, UnlockError> {
    create_recoverable_device_recipient(master, PROFILE, RECIPIENT, LABEL, broker, |record| {
        persist(&broker.directory, record)
    })
}

#[test]
fn journal_refusal_and_legacy_call_never_reach_persistent_add() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let broker = PersistentBroker::reconnect(directory.path());
    let master = VaultMasterKey::generate()?;
    assert_eq!(
        create_device_recipient(&master, PROFILE, RECIPIENT, LABEL, &broker),
        Err(UnlockError::PublicationJournalRequired)
    );
    assert_eq!(
        create_recoverable_device_recipient(&master, PROFILE, RECIPIENT, LABEL, &broker, |_| {
            Err(PublicationJournalFailure)
        }),
        Err(UnlockError::PublicationJournalUnavailable)
    );
    assert!(!directory.path().join("item").exists());
    assert!(!directory.path().join("incomplete.cbor").exists());
    let record = create(&broker, &master)?;
    assert_eq!(
        unlock_with_device(&record, PROFILE, &broker)?.expose_secret(),
        master.expose_secret()
    );
    Ok(())
}

#[test]
fn pre_add_refusal_leaves_exact_cleanup_identity_and_allows_fresh_retry()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let master = VaultMasterKey::generate()?;
    {
        let mut broker = PersistentBroker::reconnect(directory.path());
        broker.mode = AddMode::RefuseBeforeAdd;
        assert!(matches!(
            create(&broker, &master),
            Err(UnlockError::KeystoreAccessDenied { .. })
        ));
    }
    let broker = PersistentBroker::reconnect(directory.path());
    let incomplete = read_incomplete(directory.path())?;
    assert!(!purge_incomplete_device_recipient(
        &incomplete,
        PROFILE,
        &broker
    )?);
    fs::remove_file(directory.path().join("incomplete.cbor"))?;
    let next = create(&broker, &master)?;
    assert_ne!(incomplete.keystore_blob(), next.keystore_blob());
    assert_eq!(
        unlock_with_device(&next, PROFILE, &broker)?.expose_secret(),
        master.expose_secret()
    );
    Ok(())
}

#[test]
fn ambiguous_add_error_recovers_from_durable_record_after_reconnect() -> Result<(), Box<dyn Error>>
{
    let directory = tempfile::tempdir()?;
    let master = VaultMasterKey::generate()?;
    {
        let mut broker = PersistentBroker::reconnect(directory.path());
        broker.mode = AddMode::ErrorAfterAdd;
        assert!(matches!(
            create(&broker, &master),
            Err(UnlockError::KeystoreUnavailable { .. })
        ));
    }
    let broker = PersistentBroker::reconnect(directory.path());
    let recovered = read_incomplete(directory.path())?;
    assert_eq!(
        unlock_with_device(&recovered, PROFILE, &broker)?.expose_secret(),
        master.expose_secret()
    );
    assert!(purge_incomplete_device_recipient(
        &recovered, PROFILE, &broker
    )?);
    assert!(!purge_incomplete_device_recipient(
        &recovered, PROFILE, &broker
    )?);
    Ok(())
}

#[test]
fn interruption_before_or_after_add_recovers_without_creator_state() -> Result<(), Box<dyn Error>> {
    for after_add in [false, true] {
        let directory = tempfile::tempdir()?;
        let master = VaultMasterKey::generate()?;
        let interrupted = catch_unwind(AssertUnwindSafe(|| {
            let mut broker = PersistentBroker::reconnect(directory.path());
            broker.mode = AddMode::InterruptAfterAdd;
            let _ = create_recoverable_device_recipient(
                &master,
                PROFILE,
                RECIPIENT,
                LABEL,
                &broker,
                |record| {
                    persist(directory.path(), record)?;
                    assert!(after_add, "modeled interruption after journal, before add");
                    Ok(())
                },
            );
        }));
        assert!(interrupted.is_err());
        // All broker/prepared/caller state from the interrupted call is gone.
        let restarted = PersistentBroker::reconnect(directory.path());
        let recovered = read_incomplete(directory.path())?;
        assert_eq!(
            purge_incomplete_device_recipient(&recovered, PROFILE, &restarted)?,
            after_add
        );
        fs::remove_file(directory.path().join("incomplete.cbor"))?;
        let fresh = create(&restarted, &master)?;
        assert_ne!(fresh.keystore_blob(), recovered.keystore_blob());
        assert_eq!(
            unlock_with_device(&fresh, PROFILE, &restarted)?.expose_secret(),
            master.expose_secret()
        );
    }
    Ok(())
}

#[test]
fn failed_final_publication_and_cleanup_refusal_retain_recovery_across_restart()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let master = VaultMasterKey::generate()?;
    {
        let broker = PersistentBroker::reconnect(directory.path());
        let record = create(&broker, &master)?;
        fs::create_dir(directory.path().join("published.cbor"))?;
        assert!(
            fs::write(
                directory.path().join("published.cbor"),
                record.to_canonical_cbor()?
            )
            .is_err()
        );
        // The returned record is dropped without publication or rollback.
    }
    let saved_bytes = fs::read(directory.path().join("incomplete.cbor"))?;
    {
        let mut restarted = PersistentBroker::reconnect(directory.path());
        restarted.refuse_cleanup = true;
        let incomplete = read_incomplete(directory.path())?;
        assert!(matches!(
            purge_incomplete_device_recipient(&incomplete, PROFILE, &restarted),
            Err(UnlockError::KeystoreAccessDenied { .. })
        ));
        assert_eq!(
            fs::read(directory.path().join("incomplete.cbor"))?,
            saved_bytes
        );
        assert_eq!(
            create(&restarted, &master),
            Err(UnlockError::PublicationJournalUnavailable)
        );
        assert_eq!(
            fs::read(directory.path().join("incomplete.cbor"))?,
            saved_bytes
        );
    }
    let restarted = PersistentBroker::reconnect(directory.path());
    let incomplete = read_incomplete(directory.path())?;
    assert_eq!(
        unlock_with_device(&incomplete, PROFILE, &restarted)?.expose_secret(),
        master.expose_secret()
    );
    assert!(purge_incomplete_device_recipient(
        &incomplete,
        PROFILE,
        &restarted
    )?);
    fs::remove_file(directory.path().join("incomplete.cbor"))?;
    let fresh = create(&restarted, &master)?;
    assert_ne!(fresh.keystore_blob(), incomplete.keystore_blob());
    Ok(())
}

#[test]
fn duplicate_and_stale_cleanup_cannot_replace_or_remove_another_generation()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let broker = PersistentBroker::reconnect(directory.path());
    let master = VaultMasterKey::generate()?;
    let first = create(&broker, &master)?;
    // Model final publication, releasing the incomplete slot for a new attempt.
    fs::rename(
        directory.path().join("incomplete.cbor"),
        directory.path().join("published.cbor"),
    )?;
    assert!(matches!(
        create(&broker, &master),
        Err(UnlockError::KeystoreUnavailable { .. })
    ));
    let refused = read_incomplete(directory.path())?;
    assert_ne!(first.keystore_blob(), refused.keystore_blob());
    assert!(!purge_incomplete_device_recipient(
        &refused, PROFILE, &broker
    )?);
    assert_eq!(
        unlock_with_device(&first, PROFILE, &broker)?.expose_secret(),
        master.expose_secret()
    );
    // Explicitly retire the first generation, then permit label reuse.
    assert!(broker.purge_incomplete(LABEL, first.keystore_blob())?);
    fs::remove_file(directory.path().join("incomplete.cbor"))?;
    let replacement = create(&broker, &master)?;
    assert!(!broker.purge_incomplete(LABEL, first.keystore_blob())?);
    assert!(!purge_incomplete_device_recipient(
        &refused, PROFILE, &broker
    )?);
    assert!(unlock_with_device(&first, PROFILE, &broker).is_err());
    assert_eq!(
        unlock_with_device(&replacement, PROFILE, &broker)?.expose_secret(),
        master.expose_secret()
    );
    Ok(())
}

#[test]
fn recovery_checks_profile_and_provider_before_exact_cleanup() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let mut broker = PersistentBroker::reconnect(directory.path());
    let master = VaultMasterKey::generate()?;
    let record = create(&broker, &master)?;
    let wrong_profile = ProfileId::from_bytes([0xFF; IDENTIFIER_BYTES]);
    assert_eq!(
        purge_incomplete_device_recipient(&record, wrong_profile, &broker),
        Err(UnlockError::ProfileMismatch)
    );
    broker.provider = "TEST_FOREIGN_PROVIDER";
    assert!(matches!(
        purge_incomplete_device_recipient(&record, PROFILE, &broker),
        Err(UnlockError::KeystoreProviderMismatch { .. })
    ));
    assert!(matches!(
        unlock_with_device(&record, PROFILE, &broker),
        Err(UnlockError::KeystoreProviderMismatch { .. })
    ));
    broker.provider = "TEST_ADD_ONLY_PERSISTENT";
    assert_eq!(
        unlock_with_device(&record, PROFILE, &broker)?.expose_secret(),
        master.expose_secret()
    );
    assert!(purge_incomplete_device_recipient(
        &record, PROFILE, &broker
    )?);
    Ok(())
}
