//! Acceptance evidence for the `P2-K4` key, profile, and admission half.
//!
//! Four of the eight named tests live here, because what they assert is about
//! keys, recovery profiles, and the ingest gate rather than about a database
//! snapshot. They run in the default workspace lane on every CI platform. The
//! other four assert properties of an actual encrypted backup and restore and
//! live in the encrypted portability lane.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use academic_crypto::{
    DeviceKeystore, IDENTIFIER_BYTES, KEY_BYTES, KeystoreFailure, ProfileId, RECOVERY_ARGON2ID_V1,
    RecipientSet, RecoverySecret, VaultMasterKey, create_device_recipient,
    create_recovery_recipient, unlock_with_device,
};
use academic_recovery::{
    BACKUP_FORMAT_V2, BACKUP_MANIFEST_VERSION, BackupKeyError, BackupRecipientKind,
    BackupRecipientSet, BackupSetId, DEVICE_ONLY_IRRECOVERABILITY_STATEMENT, IngestRefusal,
    KeyMaterialState, RECOVERY_PROFILES, REHEARSAL_RECEIPT_RELATIVE_PATH, RecipientRequirement,
    RecoveryProfile, RecoveryProfileError, RehearsalObservations, RehearsalReceipt, SealedManifest,
    SealedManifestError, admit_first_ingest, create_backup_key_set,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const PROFILE: ProfileId = ProfileId::from_bytes([0x71; IDENTIFIER_BYTES]);
const DEVICE_LABEL: &str = "academic-os/test-device";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// A broker that holds one secret per label, standing in for DPAPI/CNG on
/// Windows and Secret Service on Linux. Which native broker is bound is
/// `P2-K1`'s concern; what matters here is that a broker exists and works.
#[derive(Debug)]
struct MemoryKeystore {
    provider: &'static str,
    stored: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryKeystore {
    fn new() -> Self {
        Self {
            provider: "TEST_MEMORY_BROKER",
            stored: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the raw wrapping key the broker holds, which is exactly what an
    /// attacker with the live device gets. Only a test may do this; the
    /// production trait never hands the raw key to a caller.
    fn wrapping_key(&self, label: &str) -> Option<[u8; KEY_BYTES]> {
        let stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored
            .get(label)
            .and_then(|bytes| <[u8; KEY_BYTES]>::try_from(bytes.as_slice()).ok())
    }
}

impl DeviceKeystore for MemoryKeystore {
    fn provider(&self) -> &str {
        self.provider
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored.insert(label.to_owned(), secret.to_vec());
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
        let stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored
            .get(label)
            .cloned()
            .map(zeroize::Zeroizing::new)
            .ok_or(KeystoreFailure::NotFound)
    }
}

/// One disposable temporary tree removed on drop.
#[derive(Debug)]
struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> TestResult<Self> {
        for _ in 0..64 {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "acad-k4-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not reserve a unique recovery test root".into())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("test cleanup failed for {}: {error}", self.path.display());
        }
    }
}

fn phrase(seed: u8) -> RecoverySecret {
    RecoverySecret::from_entropy([seed; KEY_BYTES])
}

fn identifier(seed: u8) -> [u8; IDENTIFIER_BYTES] {
    [seed; IDENTIFIER_BYTES]
}

/// A live profile with a device recipient and a recovery recipient, which is
/// exactly what `DEVICE_PLUS_PHRASE` requires.
struct LiveProfile {
    keystore: MemoryKeystore,
    master: VaultMasterKey,
    recipients: RecipientSet,
    generation: u64,
}

impl LiveProfile {
    fn new(secret: &RecoverySecret) -> TestResult<Self> {
        let keystore = MemoryKeystore::new();
        let master = VaultMasterKey::generate()?;
        let mut recipients = RecipientSet::new(PROFILE);
        recipients.push(create_device_recipient(
            &master,
            PROFILE,
            identifier(0x01),
            DEVICE_LABEL,
            &keystore,
        )?);
        recipients.push(create_recovery_recipient(
            &master,
            PROFILE,
            identifier(0x02),
            secret,
            RECOVERY_ARGON2ID_V1,
        )?);
        Ok(Self {
            keystore,
            master,
            recipients,
            generation: 1,
        })
    }

    fn key_material(&self) -> TestResult<KeyMaterialState> {
        Ok(KeyMaterialState::from_recipient_set(
            &self.recipients,
            self.generation,
            1_000,
        )?)
    }

    /// Adds a second recovery recipient, which is a key-material change.
    fn add_recovery_recipient(&mut self, secret: &RecoverySecret) -> TestResult<()> {
        self.recipients.push(create_recovery_recipient(
            &self.master,
            PROFILE,
            identifier(0x03),
            secret,
            RECOVERY_ARGON2ID_V1,
        )?);
        self.generation += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// The backup key must not be obtainable from the operating-system device
/// wrapper — not directly, and not by unwrapping the Vault Master Key and
/// deriving from it. This is the property `P2-K4` exists to keep.
#[test]
fn backup_key_is_independent_of_device_wrapper() -> TestResult {
    let secret = phrase(0xA1);
    let live = LiveProfile::new(&secret)?;

    // The device wrapper genuinely works: this is not a test that passes
    // because the keystore is broken.
    let device_record = live
        .recipients
        .records()
        .iter()
        .find(|record| record.kind() == academic_crypto::RecipientKind::DeviceKeystore)
        .ok_or("the live profile must hold a device recipient")?;
    let unlocked = unlock_with_device(device_record, PROFILE, &live.keystore)?;
    assert_eq!(
        unlocked.expose_secret(),
        live.master.expose_secret(),
        "the device wrapper must actually open the profile"
    );

    let set_id = BackupSetId::generate()?;
    let (root, recipients) = create_backup_key_set(
        RecoveryProfile::DevicePlusPhrase,
        set_id,
        &[(BackupRecipientKind::RecoveryPhrase, &secret)],
    )?;
    let sealed = SealedManifest::seal(
        &root,
        set_id,
        BACKUP_FORMAT_V2,
        BACKUP_MANIFEST_VERSION,
        b"synthetic manifest body",
    )?;

    // 1. Structural. No backup recipient record names the device keystore, and
    //    there is no requirement that could produce one.
    assert!(
        !recipients.records().is_empty(),
        "the backup key set must hold at least one recipient"
    );
    for record in recipients.records() {
        assert!(
            record.kind().requirement().survives_device_loss(),
            "backup recipient {} does not survive device loss",
            record.kind().as_str()
        );
    }
    assert_eq!(
        BackupRecipientKind::from_requirement(RecipientRequirement::DeviceKeystore),
        None,
        "a device requirement must not map to a backup recipient"
    );

    // 2. Directly. The raw wrapping key the broker holds does not open the
    //    backup root, and it is the strongest thing an attacker with the live
    //    device gets.
    let device_key = live
        .keystore
        .wrapping_key(DEVICE_LABEL)
        .ok_or("the broker must hold the device wrapping key")?;
    for record in recipients.records() {
        assert_eq!(
            record.open_with_wrapping_key_bytes(&device_key).err(),
            Some(BackupKeyError::WrongSecret),
            "the device wrapping key opened a backup recipient"
        );
    }

    // 3. Transitively. Every key the VMK can produce is a key the device
    //    wrapper can produce, so none of them may be the backup root, and none
    //    may open a backup recipient. Each derived key is named rather than
    //    sampled, so a new derivation has to be added here too.
    let vmk_reachable: Vec<[u8; KEY_BYTES]> = vec![
        *unlocked.expose_secret(),
        *unlocked.derive_store_key(PROFILE)?.expose_secret(),
        *unlocked.derive_audit_key(PROFILE)?.expose_secret(),
        *unlocked.derive_recipient_mac_key(PROFILE)?.expose_secret(),
        *unlocked.derive_rehearsal_key(PROFILE)?.expose_secret(),
        *unlocked
            .derive_domain_kek(
                PROFILE,
                academic_crypto::DomainId::from_bytes(identifier(0x09)),
            )?
            .expose_secret(),
    ];
    for candidate in &vmk_reachable {
        assert_ne!(
            candidate,
            root.expose_secret(),
            "a key reachable from the device wrapper equalled the backup root"
        );
        for record in recipients.records() {
            assert_eq!(
                record.open_with_wrapping_key_bytes(candidate).err(),
                Some(BackupKeyError::WrongSecret),
                "a key reachable from the device wrapper opened a backup recipient"
            );
        }
    }

    // 4. The device therefore cannot obtain a backup root at all: the only two
    //    constructors are `generate`, which produces a fresh unrelated root,
    //    and opening a recipient, which needs a recovery secret. A root from a
    //    different backup opens neither the recipients nor the manifest.
    let (unrelated_root, _) = create_backup_key_set(
        RecoveryProfile::DevicePlusPhrase,
        BackupSetId::generate()?,
        &[(BackupRecipientKind::RecoveryPhrase, &phrase(0xA2))],
    )?;
    assert_eq!(
        sealed.open(&unrelated_root),
        Err(SealedManifestError::WrongKey),
        "an unrelated backup root opened the sealed manifest"
    );

    // 5. The phrase, which the device does not hold, does open it.
    let recovered = recipients.open(BackupRecipientKind::RecoveryPhrase, &secret)?;
    assert_eq!(recovered.expose_secret(), root.expose_secret());
    assert_eq!(sealed.open(&recovered)?, b"synthetic manifest body");

    // 6. Source. Nothing in this crate reaches a device key source, so no
    //    future edit can add one without this failing. `DeviceKeystore` is
    //    allowed only as the `academic-crypto` enum variant the section 3.3
    //    registry has to name in order to require it of a *profile*; it is
    //    never a trait bound, an import, or a call here.
    for (path, text) in &read_crate_sources()? {
        for forbidden in [
            "DeviceWrappingKey",
            "unlock_with_device",
            "create_device_recipient",
            "PlatformKeystore",
            "keystore_blob",
            "dyn DeviceKeystore",
            "impl DeviceKeystore",
            ": DeviceKeystore",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} reaches the device key source through {forbidden}",
                path.display()
            );
        }
        for (number, line) in text.lines().enumerate() {
            if !line.contains("DeviceKeystore") {
                continue;
            }
            let trimmed = line.trim_start();
            let allowed = line.contains("RecipientKind::DeviceKeystore")
                || line.contains("RecipientRequirement::DeviceKeystore")
                || line.contains("Self::DeviceKeystore")
                // The section 3.3 registry declares its own requirement
                // variant. Declaring a requirement is not reaching a key.
                || trimmed == "DeviceKeystore,"
                || trimmed.starts_with("//");
            assert!(
                allowed,
                "{}:{} names DeviceKeystore outside the section 3.3 registry: {line}",
                path.display(),
                number + 1
            );
        }
    }
    Ok(())
}

/// t068 section 3.3 requires `DEVICE_ONLY` to state its irrecoverability "in
/// those words". The words are a constant, every surface carries it, and the
/// profile cannot hold a backup key.
#[test]
fn device_only_profile_states_irrecoverability_verbatim() -> TestResult {
    assert_eq!(
        DEVICE_ONLY_IRRECOVERABILITY_STATEMENT,
        "OS reimage or device loss is unrecoverable"
    );
    assert_eq!(
        RecoveryProfile::DeviceOnly.loss_statement(),
        "OS reimage or device loss is unrecoverable"
    );

    // The words survive into the refusal a user actually sees when a backup is
    // attempted under this profile, rather than being a constant nothing reads.
    let set_id = BackupSetId::generate()?;
    let error = create_backup_key_set(RecoveryProfile::DeviceOnly, set_id, &[])
        .err()
        .ok_or("DEVICE_ONLY must not produce a backup key set")?;
    assert_eq!(
        error,
        BackupKeyError::Profile(RecoveryProfileError::NoIndependentBackupRecipient {
            profile: "DEVICE_ONLY",
            statement: DEVICE_ONLY_IRRECOVERABILITY_STATEMENT,
        })
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains("OS reimage or device loss is unrecoverable"),
        "the refusal paraphrased the statement: {rendered}"
    );

    // No other profile claims to be unrecoverable, and none of the three is a
    // default: `GATE-38-031` stays a user decision.
    for profile in RECOVERY_PROFILES {
        let is_device_only = *profile == RecoveryProfile::DeviceOnly;
        assert_eq!(
            profile.loss_statement() == DEVICE_ONLY_IRRECOVERABILITY_STATEMENT,
            is_device_only,
            "{} states the wrong loss behaviour",
            profile.as_str()
        );
        assert_eq!(profile.supports_independent_backup(), !is_device_only);
    }

    // `GATE-38-031` stays open: nothing in this crate picks a profile, so a
    // caller has to. A `Default` impl or a `DEFAULT`/`SELECTED` constant would
    // be exactly the silent default the plan forbids.
    for (path, text) in &read_crate_sources()? {
        for forbidden in [
            "impl Default for RecoveryProfile",
            "DEFAULT_RECOVERY_PROFILE",
            "SELECTED_RECOVERY_PROFILE",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} selects a recovery profile through {forbidden}",
                path.display()
            );
        }
    }
    Ok(())
}

/// `GATE-P2-RECOVERY`: the first real ingest is refused until a rehearsal
/// receipt exists, belongs to this profile, verifies, and names the key
/// material the profile actually holds.
#[test]
fn rehearsal_receipt_is_required_before_first_ingest() -> TestResult {
    let root_dir = TestRoot::new("rehearsal-required")?;
    let profile_root = root_dir.path().join("profile");
    fs::create_dir(&profile_root)?;

    let secret = phrase(0xB2);
    let live = LiveProfile::new(&secret)?;
    let state = live.key_material()?;

    // Absent.
    let refusal = admit_first_ingest(
        &profile_root,
        &live.master,
        PROFILE,
        RecoveryProfile::DevicePlusPhrase,
        &state,
    )
    .err()
    .ok_or("an empty profile must refuse the first ingest")?;
    match refusal {
        IngestRefusal::RehearsalAbsent { ref path } => {
            assert!(
                path.replace('\\', "/")
                    .ends_with(REHEARSAL_RECEIPT_RELATIVE_PATH),
                "the refusal named {path}"
            );
        }
        other => return Err(format!("expected an absent receipt, got {other:?}").into()),
    }

    // Present and honest.
    let set_id = BackupSetId::generate()?;
    let receipt = RehearsalReceipt::record(
        &live.master,
        &RehearsalObservations {
            profile_id: PROFILE,
            recovery_profile: RecoveryProfile::DevicePlusPhrase,
            backup_set_id: set_id,
            restored_canonical_semantic_digest: [0x5A; 32],
            restored_object_count: 2,
            completed_at_unix_ms: 1_700_000_000_000,
        },
        &state,
    )?;
    let written = receipt.write_into_profile(&profile_root)?;
    assert_eq!(written, RehearsalReceipt::path_in(&profile_root));
    let admitted = admit_first_ingest(
        &profile_root,
        &live.master,
        PROFILE,
        RecoveryProfile::DevicePlusPhrase,
        &state,
    )?;
    assert_eq!(admitted, receipt);

    // A receipt for a profile that is not this one admits nothing.
    let other_profile = ProfileId::from_bytes([0x99; IDENTIFIER_BYTES]);
    assert_eq!(
        admit_first_ingest(
            &profile_root,
            &live.master,
            other_profile,
            RecoveryProfile::DevicePlusPhrase,
            &state,
        ),
        Err(IngestRefusal::ProfileMismatch)
    );

    // A receipt for a recovery profile the user did not select admits nothing.
    assert_eq!(
        admit_first_ingest(
            &profile_root,
            &live.master,
            PROFILE,
            RecoveryProfile::DevicePlusPhrasePlusOfflineFile,
            &state,
        ),
        Err(IngestRefusal::RecoveryProfileMismatch {
            rehearsed: "DEVICE_PLUS_PHRASE",
            selected: "DEVICE_PLUS_PHRASE_PLUS_OFFLINE_FILE",
        })
    );

    // A forged receipt admits nothing: the MAC is under this profile's VMK.
    let forged_key = VaultMasterKey::generate()?;
    assert_eq!(
        admit_first_ingest(
            &profile_root,
            &forged_key,
            PROFILE,
            RecoveryProfile::DevicePlusPhrase,
            &state,
        ),
        Err(IngestRefusal::ReceiptUnverified)
    );

    // A byte flipped on disk admits nothing.
    let mut bytes = fs::read(&written)?;
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(&written, &bytes)?;
    assert_eq!(
        admit_first_ingest(
            &profile_root,
            &live.master,
            PROFILE,
            RecoveryProfile::DevicePlusPhrase,
            &state,
        ),
        Err(IngestRefusal::ReceiptUnverified)
    );
    Ok(())
}

/// A rehearsal that predates the last key-material change proves nothing about
/// the key material now in force, so it stops admitting.
#[test]
fn rehearsal_is_invalidated_by_key_change() -> TestResult {
    let root_dir = TestRoot::new("rehearsal-invalidated")?;
    let profile_root = root_dir.path().join("profile");
    fs::create_dir(&profile_root)?;

    let secret = phrase(0xC3);
    let mut live = LiveProfile::new(&secret)?;
    let before = live.key_material()?;
    let set_id = BackupSetId::generate()?;
    let observations = RehearsalObservations {
        profile_id: PROFILE,
        recovery_profile: RecoveryProfile::DevicePlusPhrase,
        backup_set_id: set_id,
        restored_canonical_semantic_digest: [0x5A; 32],
        restored_object_count: 2,
        completed_at_unix_ms: 1_700_000_000_000,
    };
    RehearsalReceipt::record(&live.master, &observations, &before)?
        .write_into_profile(&profile_root)?;
    admit_first_ingest(
        &profile_root,
        &live.master,
        PROFILE,
        RecoveryProfile::DevicePlusPhrase,
        &before,
    )?;

    // Add a recovery recipient. The key material is now different.
    live.add_recovery_recipient(&phrase(0xC4))?;
    let after = live.key_material()?;
    assert_ne!(after.generation(), before.generation());
    assert_ne!(after.digest(), before.digest());

    assert_eq!(
        admit_first_ingest(
            &profile_root,
            &live.master,
            PROFILE,
            RecoveryProfile::DevicePlusPhrase,
            &after,
        ),
        Err(IngestRefusal::StaleKeyMaterial {
            receipt_generation: before.generation(),
            current_generation: after.generation(),
        }),
        "a stale rehearsal still admitted an ingest"
    );

    // A change that keeps the generation but alters the material is refused
    // too, so the generation counter alone is not the gate.
    let same_generation_other_material =
        KeyMaterialState::from_parts(before.generation(), [0xEE; 32], 2_000);
    assert_eq!(
        admit_first_ingest(
            &profile_root,
            &live.master,
            PROFILE,
            RecoveryProfile::DevicePlusPhrase,
            &same_generation_other_material,
        ),
        Err(IngestRefusal::KeyMaterialMismatch)
    );

    // Re-running the drill against the new material admits again.
    RehearsalReceipt::record(&live.master, &observations, &after)?
        .write_into_profile(&profile_root)?;
    let admitted = admit_first_ingest(
        &profile_root,
        &live.master,
        PROFILE,
        RecoveryProfile::DevicePlusPhrase,
        &after,
    )?;
    assert_eq!(admitted.key_material_generation(), after.generation());
    Ok(())
}

// ---------------------------------------------------------------------------
// Supporting evidence
// ---------------------------------------------------------------------------

/// A backup key set round-trips through canonical CBOR, and a set that is not
/// canonically encoded is refused rather than silently accepted.
#[test]
fn backup_recipient_sets_round_trip_and_reject_non_canonical_bytes() -> TestResult {
    let secret = phrase(0xD5);
    let offline = phrase(0xD6);
    let set_id = BackupSetId::generate()?;
    let (root, recipients) = create_backup_key_set(
        RecoveryProfile::DevicePlusPhrasePlusOfflineFile,
        set_id,
        &[
            (BackupRecipientKind::RecoveryPhrase, &secret),
            (BackupRecipientKind::OfflineKeyFile, &offline),
        ],
    )?;
    let bytes = recipients.to_canonical_cbor()?;
    let parsed = BackupRecipientSet::from_canonical_cbor(&bytes)?;
    assert_eq!(parsed, recipients);
    assert_eq!(parsed.to_canonical_cbor()?, bytes);

    // Either secondary recipient opens the same root.
    assert_eq!(
        parsed
            .open(BackupRecipientKind::RecoveryPhrase, &secret)?
            .expose_secret(),
        root.expose_secret()
    );
    assert_eq!(
        parsed
            .open(BackupRecipientKind::OfflineKeyFile, &offline)?
            .expose_secret(),
        root.expose_secret()
    );
    assert_eq!(
        parsed
            .open(BackupRecipientKind::RecoveryPhrase, &phrase(0x00))
            .err(),
        Some(BackupKeyError::WrongSecret)
    );

    let mut trailing = bytes.clone();
    trailing.push(0x00);
    assert!(BackupRecipientSet::from_canonical_cbor(&trailing).is_err());
    Ok(())
}

/// A tampered wrapped root is an integrity incident, not a wrong secret.
#[test]
fn a_tampered_backup_recipient_is_an_integrity_incident() -> TestResult {
    let secret = phrase(0xE7);
    let set_id = BackupSetId::generate()?;
    let (_root, recipients) = create_backup_key_set(
        RecoveryProfile::DevicePlusPhrase,
        set_id,
        &[(BackupRecipientKind::RecoveryPhrase, &secret)],
    )?;
    let mut bytes = recipients.to_canonical_cbor()?;
    // The record MAC is the last field of the last record; flipping a bit in it
    // leaves the wrapped root openable and the MAC wrong.
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = BackupRecipientSet::from_canonical_cbor(&bytes)?;
    let error = tampered
        .open(BackupRecipientKind::RecoveryPhrase, &secret)
        .err()
        .ok_or("a tampered record must not open")?;
    assert_eq!(error, BackupKeyError::RecordIntegrity);
    assert!(error.is_integrity_incident());
    Ok(())
}

/// The recovery secret is a whole 256-bit value and nothing in this crate knows
/// what a word is.
///
/// `P2-K4` ships no 24-word codec: ADR-005 records that the wordlist rides on
/// the same user decision as `GATE-38-031` and that freezing one by guess would
/// be permanent. Two things must therefore stay true so a codec can be added
/// later without reopening any cryptographic contract, and so `KY06` keeps its
/// "no oracle about which word is wrong" property structurally:
///
/// - no public function accepts or returns a word, a word list, or an index
///   into one, and
/// - the only secret this crate accepts is a whole `RecoverySecret`.
#[test]
fn recovery_secret_api_has_no_word_level_entry_point() -> TestResult {
    for (path, text) in &read_crate_sources()? {
        for forbidden in [
            "wordlist",
            "WORDLIST",
            "WordList",
            "mnemonic",
            "Mnemonic",
            "MNEMONIC",
            "bip39",
            "BIP39",
            "Bip39",
            "from_words",
            "to_words",
            "word_index",
            "WORD_COUNT",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} exposes a word-level entry point through {forbidden}",
                path.display()
            );
        }
    }

    // The secret really is opaque: two secrets differing in one byte are simply
    // two different secrets, and every refusal is the same refusal. Nothing
    // here can be probed for a partial match.
    let set_id = BackupSetId::generate()?;
    let secret = phrase(0xF1);
    let (_root, recipients) = create_backup_key_set(
        RecoveryProfile::DevicePlusPhrase,
        set_id,
        &[(BackupRecipientKind::RecoveryPhrase, &secret)],
    )?;
    let mut near_miss = [0xF1_u8; KEY_BYTES];
    near_miss[KEY_BYTES - 1] ^= 0x01;
    assert_eq!(
        recipients
            .open(
                BackupRecipientKind::RecoveryPhrase,
                &RecoverySecret::from_entropy(near_miss),
            )
            .err(),
        Some(BackupKeyError::WrongSecret),
        "a near-miss secret produced a different outcome from a wrong one"
    );
    assert_eq!(
        recipients
            .open(BackupRecipientKind::RecoveryPhrase, &phrase(0x00))
            .err(),
        Some(BackupKeyError::WrongSecret)
    );
    Ok(())
}

/// Reads every `*.rs` under this crate's `src`, at any depth.
///
/// The walk is recursive because the flat `read_dir` it replaced read only the
/// top level. `src` happens to be flat today, so nothing was missed today —
/// but a device-key reach placed in `src/platform/mod.rs`, a word-level codec
/// in `src/phrase/codec.rs`, and a `Default` for [`RecoveryProfile`] in
/// `src/lane/mod.rs` were each invisible to all three tests that call this,
/// and each is a shipped-shaped change that alters nothing observable.
fn read_crate_sources() -> TestResult<Vec<(PathBuf, String)>> {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    let mut pending = vec![source_root];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = fs::read_to_string(&path)?;
                sources.push((path, text));
            }
        }
    }
    sources.sort();
    assert!(
        sources.len() >= 5,
        "the source scan found only {} files",
        sources.len()
    );

    // Descending is the property, so it is checked rather than assumed: every
    // module the crate declares has to be a file the scan read. A walk that
    // stops descending leaves a declared module unread, and this fails then
    // rather than passing quietly the way the flat walk did.
    let read: Vec<&Path> = sources.iter().map(|(path, _)| path.as_path()).collect();
    for (path, text) in &sources {
        for name in declared_modules(text) {
            let candidates = module_files(path, &name);
            assert!(
                candidates
                    .iter()
                    .any(|candidate| read.contains(&&**candidate)),
                "{} declares `mod {name};` but the source scan read neither {} nor {}",
                path.display(),
                candidates[0].display(),
                candidates[1].display()
            );
        }
    }
    Ok(sources)
}

/// Names of the out-of-line modules one source file declares.
fn declared_modules(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_suffix(';'))
        .filter_map(|line| {
            line.strip_prefix("mod ")
                .or_else(|| line.strip_prefix("pub mod "))
        })
        .map(str::to_owned)
        .collect()
}

/// The two paths a `mod name;` in `declaring` may live at.
fn module_files(declaring: &Path, name: &str) -> [PathBuf; 2] {
    let directory = match declaring.file_stem().and_then(|stem| stem.to_str()) {
        Some("lib" | "mod") => declaring
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        _ => declaring.with_extension(""),
    };
    [
        directory.join(format!("{name}.rs")),
        directory.join(name).join("mod.rs"),
    ]
}
