//! Shared fixtures for the `AEAD_CHUNKED_V2` object lane.
//!
//! Everything here builds keys through `P2-K1`'s public schedule: a Vault
//! Master Key, a recovery recipient, and `KEK_d` per domain. Nothing here can
//! fabricate a key from bytes, so a test cannot accidentally prove a property
//! about a key the product could never hold.

#![allow(dead_code)]

use std::{error::Error, path::Path};

use academic_crypto::{
    IDENTIFIER_BYTES, ProfileId, RECOVERY_ARGON2ID_V1, RecipientRecord, RecoverySecret,
    UnlockThrottle, VaultMasterKey, create_recovery_recipient, unlock_with_recovery,
};
use academic_domain::DomainId as CanonicalDomainId;
use academic_vault::{EncryptedDomainKeyring, EncryptedVault};

/// Profile identity used as the HKDF salt in every disposable test profile.
pub const PROFILE_ID_BYTES: [u8; IDENTIFIER_BYTES] = [0xA1; IDENTIFIER_BYTES];
/// Recipient identity of the single recovery recipient these fixtures create.
pub const RECIPIENT_ID_BYTES: [u8; IDENTIFIER_BYTES] = [0xB2; IDENTIFIER_BYTES];
/// The fixed 256-bit recovery secret. Synthetic, disposable, and committed on
/// purpose: a crash harness has to reach the same `KEK_d` from a child process.
pub const RECOVERY_ENTROPY: [u8; 32] = [0xC3; 32];

/// Returns the profile identity every fixture derives under.
#[must_use]
pub fn profile_id() -> ProfileId {
    ProfileId::from_bytes(PROFILE_ID_BYTES)
}

/// Returns the fixed recovery secret.
#[must_use]
pub fn recovery_secret() -> RecoverySecret {
    RecoverySecret::from_entropy(RECOVERY_ENTROPY)
}

/// Generates a Vault Master Key and the recovery recipient that reopens it.
///
/// The record is the only thing that crosses a process boundary; the key never
/// does. A child process reads the record and unlocks it with the same fixed
/// secret, exactly as the product does.
pub fn create_master() -> Result<(VaultMasterKey, RecipientRecord), Box<dyn Error>> {
    let master = VaultMasterKey::generate()?;
    let record = create_recovery_recipient(
        &master,
        profile_id(),
        RECIPIENT_ID_BYTES,
        &recovery_secret(),
        RECOVERY_ARGON2ID_V1,
    )?;
    Ok((master, record))
}

/// Reopens a Vault Master Key from a persisted recipient record.
pub fn unlock_master(record: &RecipientRecord) -> Result<VaultMasterKey, Box<dyn Error>> {
    let mut throttle = UnlockThrottle::default();
    Ok(unlock_with_recovery(
        record,
        profile_id(),
        &recovery_secret(),
        &mut throttle,
        0,
    )?)
}

/// Builds a keyring holding `KEK_d` for every requested domain.
pub fn keyring_for(
    master: &VaultMasterKey,
    domains: &[&str],
) -> Result<EncryptedDomainKeyring, Box<dyn Error>> {
    let mut keyring = EncryptedDomainKeyring::new(profile_id());
    for domain in domains {
        let canonical: CanonicalDomainId = domain.parse()?;
        let kek = master.derive_domain_kek(
            profile_id(),
            academic_crypto::DomainId::from_bytes(*canonical.as_bytes()),
        )?;
        keyring.insert(canonical, kek)?;
    }
    Ok(keyring)
}

/// Opens an encrypted vault over an already-created private profile root.
pub fn open_encrypted_vault(
    profile_root: &Path,
    master: &VaultMasterKey,
    domains: &[&str],
    chunk_size: u32,
) -> Result<EncryptedVault, Box<dyn Error>> {
    Ok(EncryptedVault::open_with_chunk_size(
        profile_root,
        keyring_for(master, domains)?,
        chunk_size,
    )?)
}

/// Produces deterministic bytes of an exact length.
#[must_use]
pub fn deterministic_bytes(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| {
            let index = u64::try_from(index).unwrap_or(u64::MAX);
            index.wrapping_mul(2_654_435_761).to_le_bytes()[0]
        })
        .collect()
}
