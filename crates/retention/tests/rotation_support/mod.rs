//! Shared fixtures for the `P2-K5` rotation and shred suites.
//!
//! Two Vault Master Keys, because a rotation is a change of generation and a
//! suite that used one key could not tell "exactly one key opens this" from
//! "the only key opens this". Both are reachable from a recipient record and a
//! fixed synthetic secret, so a child process reopens the same keys the parent
//! wrote without ever receiving one.
//!
//! Both records live in the profile's own `keys/recipients.cbor`, written
//! through `add_recipient` and `rewrap_for_generation`. There is no test-only
//! side file holding a generation: a suite that kept one could not observe a
//! rotation leaving the profile with no key for an object that had not moved.

#![allow(dead_code)]

use std::{
    error::Error,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use academic_crypto::{
    DomainKek, IDENTIFIER_BYTES, ProfileId, RECOVERY_ARGON2ID_V1, RecipientRecord, RecoverySecret,
    UnlockThrottle, VaultMasterKey, create_recovery_recipient, unlock_with_recovery,
};
use academic_domain::{
    ArtifactDescriptor, Confidentiality, DomainId as CanonicalDomainId, MediaType, RetentionClass,
};
use academic_retention::{
    AppendOnlyJournal, journal::ROTATION_JOURNAL_RELATIVE_PATH, recipients, rotation::KeyGeneration,
};
use academic_vault::{ArtifactIngestRequest, EncryptedDomainKeyring, EncryptedVault};

/// Profile identity every fixture derives under.
pub const PROFILE_ID_BYTES: [u8; IDENTIFIER_BYTES] = [0xA1; IDENTIFIER_BYTES];
/// Recipient identity of the source generation's recovery recipient.
pub const SOURCE_RECIPIENT: [u8; IDENTIFIER_BYTES] = [0xB1; IDENTIFIER_BYTES];
/// Recipient identity of the target generation's recovery recipient.
pub const TARGET_RECIPIENT: [u8; IDENTIFIER_BYTES] = [0xB2; IDENTIFIER_BYTES];
/// Fixed synthetic secret opening the source generation.
pub const SOURCE_ENTROPY: [u8; 32] = [0xC1; 32];
/// Fixed synthetic secret opening the target generation.
pub const TARGET_ENTROPY: [u8; 32] = [0xC2; 32];

/// The one security domain these fixtures use.
pub const DOMAIN_ID: &str = "01900000-0000-7000-8000-000000000201";
/// Permission lineage every fixture artifact carries.
pub const PERMISSION_LINEAGE_ID: &str = "01900000-0000-7000-8000-000000000301";
/// Artifact identities, one per fixture object.
pub const ARTIFACT_IDS: [&str; 3] = [
    "01900000-0000-7000-8000-000000000101",
    "01900000-0000-7000-8000-000000000102",
    "01900000-0000-7000-8000-000000000103",
];

/// Small enough that a few kilobytes span many chunks.
pub const CHUNK_SIZE: u32 = 256;

/// Returns the profile identity every fixture derives under.
#[must_use]
pub fn profile_id() -> ProfileId {
    ProfileId::from_bytes(PROFILE_ID_BYTES)
}

/// One disposable profile root, removed on drop.
#[derive(Debug)]
pub struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    /// Creates a unique, owner-only directory below the host temp directory.
    ///
    /// The owner-only mode is not decoration: the vault's path policy refuses a
    /// profile root any group or other bit can reach, so a root created with the
    /// default mode fails on Unix while passing on Windows.
    pub fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "academic-retention-{label}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { path })
    }

    /// Returns the root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos}-{sequence}")
}

/// Generates one generation's Vault Master Key and the record that reopens it.
pub fn create_generation(
    recipient_id: [u8; IDENTIFIER_BYTES],
    entropy: [u8; 32],
) -> Result<(VaultMasterKey, RecipientRecord), Box<dyn Error>> {
    let master = VaultMasterKey::generate()?;
    let record = create_recovery_recipient(
        &master,
        profile_id(),
        recipient_id,
        &RecoverySecret::from_entropy(entropy),
        RECOVERY_ARGON2ID_V1,
    )?;
    Ok((master, record))
}

/// Reopens a Vault Master Key from a persisted record and its fixed secret.
pub fn unlock_generation(
    record: &RecipientRecord,
    entropy: [u8; 32],
) -> Result<VaultMasterKey, Box<dyn Error>> {
    let mut throttle = UnlockThrottle::default();
    Ok(unlock_with_recovery(
        record,
        profile_id(),
        &RecoverySecret::from_entropy(entropy),
        &mut throttle,
        0,
    )?)
}

/// Publishes both generations into the profile's own `keys/recipients.cbor`.
///
/// This is the product path, not a test file: `add_recipient` writes each
/// record into the document the profile really holds, so a child process
/// reopens both generations from that document and a rotation that left the
/// profile with no key for an unmigrated object would be observable here.
///
/// Two *different* recipient identities stand for the two generations, which is
/// what lets a test tell them apart without a key. That is why this is two
/// `add_recipient` calls and not `rewrap_for_generation`: a rewrap re-wraps one
/// recipient's own copy of the key and keeps its identity, so using it to
/// introduce a second identity is what `a_rewrap_that_changes_identity_is_refused`
/// now refuses.
pub fn publish_generations(
    root: &Path,
    source_master: &VaultMasterKey,
    source_record: &RecipientRecord,
    target_master: &VaultMasterKey,
    target_record: &RecipientRecord,
) -> Result<(), Box<dyn Error>> {
    let mut journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    recipients::add_recipient(
        root,
        profile_id(),
        &mut journal,
        source_record.clone(),
        generation_of(source_master)?,
    )?;
    recipients::add_recipient(
        root,
        profile_id(),
        &mut journal,
        target_record.clone(),
        generation_of(target_master)?,
    )?;
    Ok(())
}

/// Reads both generations back out of the profile's stored recipient set.
pub fn load_generations(root: &Path) -> Result<(VaultMasterKey, VaultMasterKey), Box<dyn Error>> {
    let set = recipients::read_set(root, profile_id())?;
    let find = |wanted: [u8; IDENTIFIER_BYTES]| {
        set.records()
            .iter()
            .find(|record| record.recipient_id() == &wanted)
            .cloned()
    };
    let source = find(SOURCE_RECIPIENT).ok_or("the recipient set holds no source generation")?;
    let target = find(TARGET_RECIPIENT).ok_or("the recipient set holds no target generation")?;
    Ok((
        unlock_generation(&source, SOURCE_ENTROPY)?,
        unlock_generation(&target, TARGET_ENTROPY)?,
    ))
}

/// Returns the domain identity every fixture artifact belongs to.
pub fn domain() -> Result<CanonicalDomainId, Box<dyn Error>> {
    Ok(DOMAIN_ID.parse()?)
}

/// Derives one generation's domain KEK.
pub fn domain_kek(master: &VaultMasterKey) -> Result<DomainKek, Box<dyn Error>> {
    let canonical = domain()?;
    Ok(master.derive_domain_kek(
        profile_id(),
        academic_crypto::DomainId::from_bytes(*canonical.as_bytes()),
    )?)
}

/// Opens an encrypted vault over `root` under one generation's key.
pub fn open_vault(root: &Path, master: &VaultMasterKey) -> Result<EncryptedVault, Box<dyn Error>> {
    let mut keyring = EncryptedDomainKeyring::new(profile_id());
    keyring.insert(domain()?, domain_kek(master)?)?;
    Ok(EncryptedVault::open_with_chunk_size(
        root, keyring, CHUNK_SIZE,
    )?)
}

/// Names one generation.
pub fn generation_of(master: &VaultMasterKey) -> Result<KeyGeneration, Box<dyn Error>> {
    Ok(KeyGeneration::of(master, profile_id())?)
}

/// Builds an ingest request for fixture artifact `index`.
pub fn request(index: usize) -> Result<ArtifactIngestRequest, Box<dyn Error>> {
    Ok(ArtifactIngestRequest::new(
        ARTIFACT_IDS[index].parse()?,
        MediaType::parse("application/pdf")?,
        domain()?,
        Confidentiality::Restricted,
        RetentionClass::UserManaged,
        PERMISSION_LINEAGE_ID.parse()?,
    ))
}

/// Produces deterministic bytes of an exact length.
#[must_use]
pub fn deterministic_bytes(length: usize, salt: u8) -> Vec<u8> {
    (0..length)
        .map(|index| {
            let index = u64::try_from(index).unwrap_or(u64::MAX);
            index
                .wrapping_mul(2_654_435_761)
                .wrapping_add(u64::from(salt))
                .to_le_bytes()[0]
        })
        .collect()
}

/// Seals `count` fixture artifacts into `vault` and returns their descriptors.
pub fn seal_corpus(
    vault: &EncryptedVault,
    count: usize,
) -> Result<Vec<ArtifactDescriptor>, Box<dyn Error>> {
    let mut descriptors = Vec::with_capacity(count);
    for index in 0..count {
        let bytes = deterministic_bytes(1024 + index * 37, u8::try_from(index).unwrap_or(0));
        let sealed = vault.ingest(&request(index)?, Cursor::new(bytes))?;
        descriptors.push(sealed.descriptor().clone());
    }
    Ok(descriptors)
}
