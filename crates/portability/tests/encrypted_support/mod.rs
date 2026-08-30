//! Deterministic encrypted profile used by every `P2-K4` test.
//!
//! The canonical corpus is the Phase 1 one: same identifiers, same timestamps,
//! same signing key, two registered artifacts. Only the physical layer differs
//! — a SQLCipher profile and `AEAD_CHUNKED_V2` objects — so a difference in
//! behaviour between the lanes is a difference in the lane, not in the corpus.
//!
//! Two artifacts is not incidental. `BK03` fires on the second object copy, so
//! a one-artifact corpus never reaches it — which is why Phase 1's daemon exit
//! corpus records it `NOT_RUN` and points at its own crash suite. This corpus
//! reaches it directly.

#![allow(dead_code)]

use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
use academic_crypto::{
    DeviceKeystore, IDENTIFIER_BYTES, KEY_BYTES, KeystoreFailure, ProfileId, RECOVERY_ARGON2ID_V1,
    RecipientSet, RecoverySecret, VaultMasterKey, create_device_recipient,
    create_recovery_recipient, unlock_with_recovery,
};
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim,
    ClaimId, ClaimObject, ClaimRelation, ClaimRelationKind, Confidentiality, ContentDigest,
    DecisionAction, DecisionId, DeviceId, DomainError, DomainId, EVENT_SCHEMA_VERSION, EntityId,
    EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem, EvidenceLocator,
    EvidenceRole, EvidenceStrength, MediaType, PermissionLineageId, PredicateId, ResolutionSlot,
    RetentionActionId, RetentionActionRegistration, RetentionClass, ScopeDescriptor, ScopeId,
    TimestampMillis, UnsignedBatch, UserDecision, ValidInterval,
};
use academic_portability::{
    encrypted::ProfileKeys,
    verify::{CanonicalDatabase, read_artifact_descriptors},
};
use academic_recovery::{
    BackupMasterKey, BackupRecipientKind, BackupRecipientSet, BackupSetId, RecoveryProfile,
    create_backup_key_set,
};
use academic_store::{
    accept::AcceptanceStore,
    cipher::{EncryptedProfile, create_encrypted_profile},
    idempotency::AcceptanceCommand,
    path_policy::NativePathProbe,
};
use academic_vault::{ArtifactIngestRequest, EncryptedDomainKeyring, EncryptedVault};
use ed25519_dalek::SigningKey;

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Fixed profile identity every derived key is salted with.
pub const PROFILE_ID: ProfileId = ProfileId::from_bytes([0x74; IDENTIFIER_BYTES]);
/// The fixed 256-bit recovery secret every "phrase" in these tests is.
///
/// `P2-K4` ships no 24-word codec, so nothing here encodes or decodes words.
/// A test named for a phrase is exercising this secret.
pub const RECOVERY_ENTROPY: [u8; KEY_BYTES] = [0x6b; KEY_BYTES];
/// A second secret standing in for the offline key file.
pub const OFFLINE_ENTROPY: [u8; KEY_BYTES] = [0x6c; KEY_BYTES];
/// Fixed backup set identity, so a harness child addresses the same backup.
pub const BACKUP_SET_SEED: [u8; IDENTIFIER_BYTES] = [0x75; IDENTIFIER_BYTES];
/// Fixed Ed25519 seed for the single synthetic device.
pub const SIGNING_SEED: [u8; 32] = [0x51; 32];
/// Fixed build digest recorded by the encrypted profile.
pub const BUILD_DIGEST: [u8; 32] = [0xb2; 32];
/// Exact bytes of the first synthetic artifact.
pub const FIRST_ARTIFACT_BYTES: &[u8] = b"synthetic encrypted artifact one\n";
/// Exact bytes of the second synthetic artifact.
pub const SECOND_ARTIFACT_BYTES: &[u8] = b"synthetic encrypted artifact two\n";
/// Label the test device broker stores its wrapping key under.
pub const DEVICE_LABEL: &str = "academic-os/k4-test-device";
/// Filename the test device broker keeps its wrapping key in.
pub const DEVICE_KEYSTORE_FILE: &str = "device-keystore.bin";
/// Relative path of the profile's recipient set.
pub const RECIPIENTS_FILE: &str = "keys/recipients.cbor";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// A broker that keeps one wrapping key in a file beside the test tree.
///
/// It stands in for DPAPI/CNG on Windows and Secret Service on Linux, which are
/// `P2-K1`'s concern. It is a *file* rather than a process-local map only so a
/// crash-harness child can reach the same broker the parent used; nothing here
/// claims anything about a real broker's protection.
#[derive(Debug)]
pub struct FileKeystore {
    path: PathBuf,
}

impl FileKeystore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl DeviceKeystore for FileKeystore {
    fn provider(&self) -> &str {
        "TEST_FILE_BROKER"
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| KeystoreFailure::Unavailable)?;
        }
        fs::write(&self.path, secret).map_err(|_| KeystoreFailure::Unavailable)?;
        Ok(label.as_bytes().to_vec())
    }

    fn open(
        &self,
        label: &str,
        blob: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeystoreFailure> {
        if blob != label.as_bytes() {
            return Err(KeystoreFailure::InvalidBlob);
        }
        fs::read(&self.path)
            .map(zeroize::Zeroizing::new)
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => KeystoreFailure::NotFound,
                _ => KeystoreFailure::Unavailable,
            })
    }
}

/// Owner of one disposable temporary tree removed on drop.
#[derive(Debug)]
pub struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    pub fn new(label: &str) -> TestResult<Self> {
        let label = sanitize(label);
        for _ in 0..64 {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = temporary_base()?.join(format!(
                "acad-k4-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not reserve a unique encrypted portability test root".into())
    }

    /// Adopts an existing directory a parent process created.
    pub const fn adopt(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Gives up ownership so the tree survives this value's drop.
    pub fn leak(self) -> PathBuf {
        let path = self.path.clone();
        std::mem::forget(self);
        path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("test cleanup failed for {}: {error}", self.path.display());
        }
    }
}

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native path
/// facade refuses to follow a link component, so the tests address the real
/// directory.
#[cfg(unix)]
fn temporary_base() -> io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

/// The 256-bit secret a printed phrase would encode. No words are involved.
#[must_use]
pub fn recovery_secret() -> RecoverySecret {
    RecoverySecret::from_entropy(RECOVERY_ENTROPY)
}

/// The secret standing in for the offline key file.
#[must_use]
pub fn offline_secret() -> RecoverySecret {
    RecoverySecret::from_entropy(OFFLINE_ENTROPY)
}

/// The fixed backup set identity.
#[must_use]
pub fn backup_set_id() -> BackupSetId {
    BackupSetId::from_bytes(BACKUP_SET_SEED)
}

/// The independent trust anchor bound to the fixed synthetic device.
pub fn authorizations() -> TestResult<Vec<DeviceAuthorization>> {
    let signing_key = SigningKey::from_bytes(&SIGNING_SEED);
    Ok(vec![DeviceAuthorization::new(
        id::<DeviceId>(0xd001)?,
        id::<EntityId>(0xd002)?,
        signing_key.verifying_key(),
    )])
}

/// The single synthetic security domain.
pub fn domain_id() -> Result<DomainId, DomainError> {
    id(0x0101)
}

/// A complete encrypted profile with a fixed canonical corpus.
#[derive(Debug)]
pub struct EncryptedFixture {
    root: TestRoot,
    profile_root: PathBuf,
    profile: EncryptedProfile,
    master: VaultMasterKey,
    keys: ProfileKeys,
    recipients: RecipientSet,
    signing_key: SigningKey,
    authorization: DeviceAuthorization,
    next_origin_seq: u64,
    previous_batch_hash: Option<ContentDigest>,
    revision: u64,
    batch_counter: u64,
    accepted_at: i64,
}

impl EncryptedFixture {
    /// Creates an encrypted profile and accepts the fixed synthetic corpus.
    pub fn new(label: &str) -> TestResult<Self> {
        let root = TestRoot::new(label)?;
        Self::in_root(root)
    }

    fn in_root(root: TestRoot) -> TestResult<Self> {
        let profile_root = root.child("profile");
        let master = VaultMasterKey::generate()?;
        let keys = ProfileKeys::derive(&master, PROFILE_ID, &[domain_id()?])?;
        let profile = create_encrypted_profile(
            &profile_root,
            &NativePathProbe::default(),
            keys.store_key(),
            BUILD_DIGEST,
        )?;

        let keystore = FileKeystore::new(root.child(DEVICE_KEYSTORE_FILE));
        let mut recipients = RecipientSet::new(PROFILE_ID);
        recipients.push(create_device_recipient(
            &master,
            PROFILE_ID,
            [0x01; IDENTIFIER_BYTES],
            DEVICE_LABEL,
            &keystore,
        )?);
        recipients.push(create_recovery_recipient(
            &master,
            PROFILE_ID,
            [0x02; IDENTIFIER_BYTES],
            &recovery_secret(),
            RECOVERY_ARGON2ID_V1,
        )?);
        write_recipients(&profile_root, &recipients)?;

        let signing_key = SigningKey::from_bytes(&SIGNING_SEED);
        let authorization = DeviceAuthorization::new(
            id::<DeviceId>(0xd001)?,
            id::<EntityId>(0xd002)?,
            signing_key.verifying_key(),
        );
        let mut fixture = Self {
            root,
            profile_root,
            profile,
            master,
            keys,
            recipients,
            signing_key,
            authorization,
            next_origin_seq: 1,
            previous_batch_hash: None,
            revision: 0,
            batch_counter: 0,
            accepted_at: 10_000,
        };
        fixture.seed_corpus()?;
        Ok(fixture)
    }

    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn work_path(&self, name: &str) -> PathBuf {
        self.root.child(name)
    }

    pub const fn master(&self) -> &VaultMasterKey {
        &self.master
    }

    pub const fn keys(&self) -> &ProfileKeys {
        &self.keys
    }

    pub const fn recipients(&self) -> &RecipientSet {
        &self.recipients
    }

    pub fn authorizations(&self) -> Vec<DeviceAuthorization> {
        vec![self.authorization.clone()]
    }

    /// Returns an authorization bound to a different device identity.
    pub fn foreign_authorization(&self) -> TestResult<DeviceAuthorization> {
        Ok(DeviceAuthorization::new(
            id::<DeviceId>(0xdfff)?,
            id::<EntityId>(0xd002)?,
            self.signing_key.verifying_key(),
        ))
    }

    /// Returns only the recovery-class recipient records, canonical CBOR.
    pub fn recovery_recipients_cbor(&self) -> TestResult<Vec<u8>> {
        recovery_only_recipients(&self.recipients)
    }

    /// Gives up ownership of the test tree so a fault child can inherit it.
    pub fn leak_root(self) -> PathBuf {
        let Self { root, .. } = self;
        root.leak()
    }

    /// Accepts one extra claim so the canonical watermark advances.
    pub fn accept_additional_claim(&mut self, seed: u64) -> TestResult<()> {
        let domain = domain_id()?;
        let scope_id: ScopeId = id(0x0102)?;
        let evidence_id: EvidenceId = id(0x0402)?;
        let claim = text_claim(
            id(0x0f00 + seed)?,
            id(0x0fa0 + seed)?,
            "note.body",
            "additional synthetic note",
            scope_id,
            evidence_id,
        )?;
        self.accept(
            importer_actor(),
            domain,
            vec![EventPayload::ClaimAsserted(claim)],
        )
    }

    fn seed_corpus(&mut self) -> TestResult<()> {
        let domain = domain_id()?;
        let scope_id: ScopeId = id(0x0102)?;
        let vault = EncryptedVault::open(&self.profile_root, self.keyring()?)?;

        let first = self.register_artifact(&vault, 0x0201, 0x0301, FIRST_ARTIFACT_BYTES)?;
        let second = self.register_artifact(&vault, 0x0202, 0x0302, SECOND_ARTIFACT_BYTES)?;
        let first_evidence: EvidenceId = id(0x0401)?;
        let second_evidence: EvidenceId = id(0x0402)?;

        self.accept(
            importer_actor(),
            domain,
            vec![
                EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: scope_id,
                    domain_id: domain,
                    label: "synthetic encrypted scope".to_owned(),
                }),
                EventPayload::ArtifactRegistered(first.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    first_evidence,
                    &first.descriptor,
                    first.locator.clone(),
                )),
                EventPayload::ArtifactRegistered(second.descriptor.clone()),
                EventPayload::EvidenceRegistered(evidence_item(
                    second_evidence,
                    &second.descriptor,
                    second.locator.clone(),
                )),
            ],
        )?;
        drop(vault);

        let symbol_claim = text_claim(
            id(0x0501)?,
            id(0x0601)?,
            "code.symbol",
            "encrypted_backup_symbol",
            scope_id,
            first_evidence,
        )?;
        let note_claim = text_claim(
            id(0x0502)?,
            id(0x0602)?,
            "note.body",
            "an encrypted backup keeps canonical bytes",
            scope_id,
            second_evidence,
        )?;
        let confirmed_claim = user_claim(
            id(0x0504)?,
            id(0x0605)?,
            "note.body",
            "user confirmed synthetic note",
            scope_id,
            second_evidence,
        )?;
        let confirmed_object = confirmed_claim.object.clone();
        self.accept(
            importer_actor(),
            domain,
            vec![
                EventPayload::ClaimAsserted(symbol_claim),
                EventPayload::ClaimAsserted(note_claim),
            ],
        )?;
        self.accept(
            user_actor(self.authorization.user_id()),
            domain,
            vec![EventPayload::ClaimAsserted(confirmed_claim)],
        )?;
        self.accept(
            importer_actor(),
            domain,
            vec![EventPayload::ClaimRelated(ClaimRelation {
                source_claim_id: id(0x0502)?,
                target_claim_id: id(0x0501)?,
                kind: ClaimRelationKind::Supports,
                scope_id,
            })],
        )?;
        self.accept(
            user_actor(self.authorization.user_id()),
            domain,
            vec![EventPayload::DecisionRecorded(UserDecision {
                id: id::<DecisionId>(0x0701)?,
                target_claim_id: id(0x0504)?,
                target_object: confirmed_object,
                resolution_slot: ResolutionSlot {
                    subject_entity_id: id(0x0605)?,
                    predicate_id: PredicateId::parse("note.body")?,
                    scope_id,
                },
                action: DecisionAction::Confirm,
                valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
                rationale_evidence_ids: vec![second_evidence],
                decided_at: TimestampMillis::new(400),
                reversible_until: Some(TimestampMillis::new(900)),
            })],
        )
    }

    fn keyring(&self) -> TestResult<EncryptedDomainKeyring> {
        Ok(self.keys.keyring(&self.master)?)
    }

    /// Opens the profile's object vault under an arbitrary generation's key.
    ///
    /// A rotation moves objects to a second Vault Master Key one unit at a
    /// time, so between the first `UnitMigrated` and the last one this profile
    /// has objects under both generations and both keyrings are needed. The
    /// store key is the one the fixture currently holds, which is correct at
    /// every point: the `STORE_DATABASE` unit runs last, and `adopt_generation`
    /// is what moves this fixture onto the new one afterwards.
    pub fn open_vault_under(&self, master: &VaultMasterKey) -> TestResult<EncryptedVault> {
        Ok(EncryptedVault::open(
            &self.profile_root,
            self.keys.keyring(master)?,
        )?)
    }

    /// Opens the profile's object vault under the generation that created it.
    pub fn open_vault(&self) -> TestResult<EncryptedVault> {
        Ok(EncryptedVault::open(&self.profile_root, self.keyring()?)?)
    }

    /// Adopts the generation a completed rotation moved this profile onto.
    ///
    /// The `STORE_DATABASE` unit rekeys the database to the target generation's
    /// `SKEY_p`, so from that point a key set derived from the superseded
    /// master opens neither half of the profile. This is the caller-side half
    /// of a rotation, and calling it is how a test states that the rotation
    /// finished rather than merely started.
    pub fn adopt_generation(&mut self, master: VaultMasterKey) -> TestResult<()> {
        let keys = ProfileKeys::derive(&master, PROFILE_ID, &[domain_id()?])?;
        let profile = academic_store::cipher::open_encrypted_profile(
            &self.profile_root,
            &NativePathProbe::default(),
            keys.store_key(),
        )?;
        self.profile = profile;
        self.master = master;
        self.keys = keys;
        Ok(())
    }

    /// Moves the profile's recipient records onto the generation it now holds.
    ///
    /// This is the recipient half of a rotation, through the product path:
    /// `rewrap_for_generation` re-wraps each stored record for the generation
    /// the rotation left — same identity, new wrapped key — and
    /// `retire_generation` then writes the set holding only that generation.
    ///
    /// A backup carrying the superseded generation's recovery records would
    /// recover a master that opens neither half of the restored profile, so a
    /// rotation that stops before this one has not finished.
    pub fn rewrap_recovery_recipients(&mut self) -> TestResult<()> {
        let keystore = FileKeystore::new(self.root.child(DEVICE_KEYSTORE_FILE));
        let generation = academic_retention::rotation::KeyGeneration::of(&self.master, PROFILE_ID)?;
        let mut journal = academic_retention::AppendOnlyJournal::open(
            &self
                .profile_root
                .join(academic_retention::journal::ROTATION_JOURNAL_RELATIVE_PATH),
        )?;
        let rewrapped = academic_retention::recipients::rewrap_for_generation(
            &self.profile_root,
            PROFILE_ID,
            &mut journal,
            generation,
            |record| {
                if record.kind() == academic_crypto::RecipientKind::RecoverySecret {
                    create_recovery_recipient(
                        &self.master,
                        PROFILE_ID,
                        *record.recipient_id(),
                        &recovery_secret(),
                        RECOVERY_ARGON2ID_V1,
                    )
                    .map_err(academic_retention::recipients::RecipientError::from)
                } else {
                    create_device_recipient(
                        &self.master,
                        PROFILE_ID,
                        *record.recipient_id(),
                        DEVICE_LABEL,
                        &keystore,
                    )
                    .map_err(academic_retention::recipients::RecipientError::from)
                }
            },
        )?;
        academic_retention::recipients::retire_generation(
            &self.profile_root,
            PROFILE_ID,
            &journal,
            generation,
            |record| rewrapped.iter().any(|kept| kept == record),
        )?;
        let mut set = RecipientSet::new(PROFILE_ID);
        for record in &rewrapped {
            set.push(record.clone());
        }
        self.recipients = set;
        Ok(())
    }

    /// Opens the owned acceptance writer over the profile's canonical store.
    pub fn open_store(&self) -> TestResult<AcceptanceStore> {
        Ok(self.profile.open_acceptance_store(self.keys.store_key())?)
    }

    /// Reads every registered descriptor, with every recorded migration applied.
    pub fn descriptors(&self) -> TestResult<Vec<ArtifactDescriptor>> {
        let database = CanonicalDatabase::open_source(
            &self.profile_root.join(academic_store::STORE_DATABASE_FILE),
            self.keys.store_key(),
        )?;
        Ok(read_artifact_descriptors(&database)?)
    }

    /// Accepts one `RETENTION_ACTION_RECORDED` event authorizing `source_digest`.
    ///
    /// This is the canonical half of a descriptor migration: the typed row that
    /// moves the reference is refused unless this event exists and carries this
    /// exact digest.
    pub fn accept_retention_action(
        &mut self,
        action: RetentionActionId,
        source_digest: ContentDigest,
    ) -> TestResult<()> {
        let domain = domain_id()?;
        self.accept(
            importer_actor(),
            domain,
            vec![EventPayload::RetentionActionRecorded(
                RetentionActionRegistration {
                    id: action,
                    domain_id: domain,
                    scope_id: id(0x0102)?,
                    source_digest: Some(source_digest),
                    valid_time: ValidInterval::open_ended(TimestampMillis::new(700)),
                },
            )],
        )
    }

    fn register_artifact(
        &self,
        vault: &EncryptedVault,
        artifact_seed: u64,
        lineage_seed: u64,
        bytes: &[u8],
    ) -> TestResult<RegisteredArtifact> {
        let request = ArtifactIngestRequest::new(
            id::<ArtifactId>(artifact_seed)?,
            MediaType::parse("text/plain")?,
            domain_id()?,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            id::<PermissionLineageId>(lineage_seed)?,
        );
        let receipt = vault.ingest(&request, bytes)?;
        let mut descriptor = receipt.descriptor().clone();
        let locator = EvidenceLocator::TextBytes {
            source_digest: descriptor.content_digest,
            start: 0,
            end: descriptor.byte_length,
        };
        descriptor.evidence_representations = vec![ArtifactRepresentation {
            locator: locator.clone(),
            content_digest: descriptor.content_digest,
            byte_length: descriptor.byte_length,
        }];
        Ok(RegisteredArtifact {
            descriptor,
            locator,
        })
    }

    /// Signs and accepts one batch of payloads through the product writer.
    pub fn accept(
        &mut self,
        actor: Actor,
        domain: DomainId,
        payloads: Vec<EventPayload>,
    ) -> TestResult<()> {
        self.batch_counter = self
            .batch_counter
            .checked_add(1)
            .ok_or("synthetic batch counter overflow")?;
        let origin_start = self.next_origin_seq;
        let mut events = Vec::with_capacity(payloads.len());
        for (offset, payload) in payloads.into_iter().enumerate() {
            let offset = u64::try_from(offset)?;
            let origin_seq = origin_start
                .checked_add(offset)
                .ok_or("synthetic origin sequence overflow")?;
            let event = Event {
                id: id::<EventId>(
                    0x0e00_0000_u64
                        .checked_add(self.batch_counter << 8)
                        .and_then(|value| value.checked_add(offset))
                        .ok_or("synthetic event identifier overflow")?,
                )?,
                origin_seq,
                origin_observed_at: TimestampMillis::new(
                    1_000_i64
                        .checked_add(i64::try_from(origin_seq)?)
                        .ok_or("synthetic observation overflow")?,
                ),
                actor: actor.clone(),
                domain_id: domain,
                payload,
            };
            event.validate()?;
            events.push(event);
        }
        let event_count = u64::try_from(events.len())?;
        let origin_end = origin_start
            .checked_add(event_count - 1)
            .ok_or("synthetic origin end overflow")?;
        let batch = UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: id::<BatchId>(
                0x0b00_0000_u64
                    .checked_add(self.batch_counter)
                    .ok_or("synthetic batch identifier overflow")?,
            )?,
            device_id: self.authorization.device_id(),
            origin_seq_start: origin_start,
            origin_seq_end: origin_end,
            previous_batch_hash: self.previous_batch_hash,
            origin_created_at: TimestampMillis::new(
                2_000_i64
                    .checked_add(i64::try_from(self.batch_counter)?)
                    .ok_or("synthetic creation overflow")?,
            ),
            events,
        };
        let envelope = sign_batch(&batch, &self.signing_key)?;
        let verified = verify_signed_batch(&envelope, &self.authorization)?;
        let vault = EncryptedVault::open(&self.profile_root, self.keyring()?)?;
        let mut store = self.profile.open_acceptance_store(self.keys.store_key())?;
        let outcome = store.accept_verified_batch(
            &verified,
            AcceptanceCommand {
                request_id: [u8::try_from(self.batch_counter % 251)?; 16],
                client_instance_id: [0x62; 16],
                idempotency_key: *ContentDigest::sha256(
                    &[
                        b"encrypted-portability-request".as_slice(),
                        &self.batch_counter.to_be_bytes(),
                    ]
                    .concat(),
                )
                .as_bytes(),
                expected_revision: Some(self.revision),
                envelope_bytes: &envelope,
            },
            TimestampMillis::new(self.accepted_at),
            &vault,
        )?;
        drop(store);
        drop(vault);
        self.next_origin_seq = origin_end
            .checked_add(1)
            .ok_or("synthetic next origin overflow")?;
        self.previous_batch_hash = Some(outcome.receipt.envelope_hash);
        self.revision = outcome.receipt.committed_revision;
        self.accepted_at = self
            .accepted_at
            .checked_add(1)
            .ok_or("synthetic acceptance clock overflow")?;
        Ok(())
    }
}

/// Writes the profile's recipient set, which a fresh machine unlocks from.
pub fn write_recipients(profile_root: &Path, set: &RecipientSet) -> TestResult<()> {
    let directory = profile_root.join("keys");
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("recipients.cbor"), set.to_canonical_cbor()?)?;
    Ok(())
}

/// Reads the profile's recipient set back.
pub fn read_recipients(profile_root: &Path) -> TestResult<RecipientSet> {
    let bytes = fs::read(profile_root.join(RECIPIENTS_FILE))?;
    Ok(RecipientSet::from_canonical_cbor(&bytes)?)
}

/// Strips device recipients, leaving only what a fresh machine can use.
pub fn recovery_only_recipients(set: &RecipientSet) -> TestResult<Vec<u8>> {
    let mut recovery = RecipientSet::new(PROFILE_ID);
    for record in set.records() {
        if record.kind() == academic_crypto::RecipientKind::RecoverySecret {
            recovery.push(record.clone());
        }
    }
    if recovery.records().is_empty() {
        return Err("the profile holds no recovery recipient".into());
    }
    Ok(recovery.to_canonical_cbor()?)
}

/// Builds the backup key set for `DEVICE_PLUS_PHRASE` from the fixed phrase.
pub fn backup_key_set() -> TestResult<(BackupMasterKey, BackupRecipientSet)> {
    Ok(create_backup_key_set(
        RecoveryProfile::DevicePlusPhrase,
        backup_set_id(),
        &[(BackupRecipientKind::RecoveryPhrase, &recovery_secret())],
    )?)
}

/// Recovers the Vault Master Key the way a fresh machine does: phrase only.
pub fn unlock_master_with_phrase(set: &RecipientSet) -> TestResult<VaultMasterKey> {
    let mut throttle = academic_crypto::UnlockThrottle::new();
    for record in set.records() {
        if record.kind() != academic_crypto::RecipientKind::RecoverySecret {
            continue;
        }
        if let Ok(master) =
            unlock_with_recovery(record, PROFILE_ID, &recovery_secret(), &mut throttle, 0)
        {
            return Ok(master);
        }
    }
    Err("no recovery recipient opened with the fixed phrase".into())
}

#[derive(Debug)]
struct RegisteredArtifact {
    descriptor: ArtifactDescriptor,
    locator: EvidenceLocator,
}

/// The importer actor used by every synthetic ingest event.
#[must_use]
pub fn importer_actor() -> Actor {
    Actor::Importer {
        name: "academic.portability.encrypted".to_owned(),
        version: "1.0.0".to_owned(),
    }
}

/// The user actor bound to the fixture's authorized identity.
#[must_use]
pub const fn user_actor(user_id: EntityId) -> Actor {
    Actor::User { user_id }
}

fn evidence_item(
    id: EvidenceId,
    descriptor: &ArtifactDescriptor,
    locator: EvidenceLocator,
) -> EvidenceItem {
    EvidenceItem {
        id,
        artifact_id: descriptor.id,
        locator,
        excerpt_digest: descriptor.content_digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "academic.portability.encrypted".to_owned(),
        extractor_version: "1.0.0".to_owned(),
    }
}

pub fn text_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    text: &str,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
) -> TestResult<Claim> {
    build_claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Text(text.to_owned()),
        scope_id,
        evidence_id,
        AuthorityClass::DirectObservation,
        EpistemicStatus::CodeObserved,
    )
}

fn user_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    text: &str,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
) -> TestResult<Claim> {
    build_claim(
        claim_id,
        subject,
        predicate,
        ClaimObject::Text(text.to_owned()),
        scope_id,
        evidence_id,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_claim(
    claim_id: ClaimId,
    subject: EntityId,
    predicate: &str,
    object: ClaimObject,
    scope_id: ScopeId,
    evidence_id: EvidenceId,
    authority_class: AuthorityClass,
    epistemic_status: EpistemicStatus,
) -> TestResult<Claim> {
    let claim = Claim {
        id: claim_id,
        subject_entity_id: subject,
        predicate_id: PredicateId::parse(predicate)?,
        object,
        scope_id,
        authority_class,
        epistemic_status,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
        evidence_ids: vec![evidence_id],
    };
    claim.validate()?;
    Ok(claim)
}

/// Constructs one deterministic UUIDv7-shaped identifier from a numeric seed.
pub fn id<T>(suffix: u64) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}

/// Encodes bytes as lowercase hexadecimal.
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn sanitize(label: &str) -> String {
    let label = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if label.is_empty() {
        "case".to_owned()
    } else {
        label
    }
}
