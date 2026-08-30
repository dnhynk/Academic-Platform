//! Encrypted backup, independent restore, and the recovery drill (`P2-K4`).
//!
//! # What a backup is
//!
//! ```text
//! <backup>/
//!   BACKUP_FORMAT_V2                   # plaintext marker: format name + manifest version
//!   keys/backup-recipients.cbor        # the backup root, wrapped only by recovery recipients
//!   manifest.cbor                      # sealed and signed under that root
//!   store/academic-platform.sqlite3    # SQLCipher snapshot at a fixed watermark
//!   objects/<artifact-id>.aobj         # AEAD_CHUNKED_V2 objects, byte-for-byte
//! ```
//!
//! Only the marker and the wrapped key set are readable without a secret.
//! Everything a reader needs in order to *use* the backup — the watermark, the
//! counts, the object closure, the file digests, and the profile's own recovery
//! recipients — lives inside the sealed manifest, so the backup is inert
//! without the recovery phrase.
//!
//! # Key independence, and its exact boundary
//!
//! The manifest is sealed under a backup root that is generated for the backup
//! and wrapped only by recovery-class recipients. The device wrapper cannot
//! produce it: not directly, and not by unwrapping the Vault Master Key, which
//! the root has no derivation edge from. `academic-recovery` owns that property
//! and proves it.
//!
//! What that does **not** claim: the snapshot is still the profile's own
//! SQLCipher database under `SKEY_p`, and the objects are still under `KEK_d`.
//! Someone holding the live device already holds those keys and already holds
//! the live profile. The independence that matters is the one recovery depends
//! on — a backup whose manifest could only be opened by the lost device would
//! not be a backup — and that is the property implemented and tested here.
//!
//! # Posture
//!
//! Producing an encrypted backup is not ADR-002 or ADR-012 acceptance.
//! `production_data_allowed` and `adr_002_accepted` are `false` in every
//! manifest this build writes, and a manifest claiming otherwise is refused on
//! read.

pub mod backup;
pub mod manifest;
pub mod restore;
pub mod rotation;

use academic_crypto::{DomainKek, ProfileId, StoreKey, VaultMasterKey};
use academic_domain::DomainId;
use academic_vault::EncryptedDomainKeyring;

use crate::{PortabilityError, PortabilityResult};

/// Relative path of the sealed manifest inside a backup directory.
pub const MANIFEST_FILE: &str = "manifest.cbor";
/// Relative path of the plaintext format marker.
pub const FORMAT_MARKER_FILE: &str = "BACKUP_FORMAT_V2";
/// Relative path of the wrapped backup key set.
pub const RECIPIENTS_FILE: &str = "keys/backup-recipients.cbor";
/// Relative directory holding the copied encrypted database.
pub const DATABASE_DIRECTORY: &str = "store";
/// Relative directory holding one sealed object per registered artifact.
pub const OBJECTS_DIRECTORY: &str = "objects";
/// Encrypted object namespace inside a restored profile.
pub const RESTORED_VAULT_OBJECTS_DIRECTORY: &str = "vault/v2";

/// Reports whether one relative backup path is a tombstone rather than backup content.
///
/// `tombstones/` is the one part of a published backup the sealed manifest does
/// not cover, because a `P2-K5` deletion writes a tombstone into a backup that
/// was already published and sealed. See
/// [`crate::encrypted::backup::verify_encrypted_backup_directory`] for exactly
/// what that does and does not weaken. The directory name is
/// `academic-retention`'s, not a second spelling of it.
#[must_use]
pub fn is_tombstone_path(relative: &str) -> bool {
    relative
        .strip_prefix(academic_retention::TOMBSTONE_DIRECTORY)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|name| !name.is_empty() && !name.contains('/'))
}
/// Marker written into every unpublished restore staging directory.
pub const RESTORE_INCOMPLETE_MARKER: &str = crate::RESTORE_INCOMPLETE_MARKER;

/// Exact bytes of the plaintext format marker.
pub const FORMAT_MARKER_CONTENTS: &str = concat!(
    "ACADEMIC_ENCRYPTED_BACKUP_V2\n",
    "manifest_version=2\n",
    "manifest=sealed; readable only with a recovery recipient\n",
    "production_data_allowed=false\n",
    "adr_002_accepted=false\n"
);

/// The one place a profile's derived keys are assembled for backup or restore.
///
/// Holding the Vault Master Key and the profile identity is enough to derive
/// every key both directions need. Nothing here is persisted, and the derived
/// keys zeroize on drop because `academic-crypto` owns their types.
#[derive(Debug)]
pub struct ProfileKeys {
    profile_id: ProfileId,
    generation: [u8; 32],
    store_key: StoreKey,
    domains: Vec<(DomainId, DomainKek)>,
}

impl ProfileKeys {
    /// Derives the store key and one domain KEK per security domain.
    pub fn derive(
        master: &VaultMasterKey,
        profile_id: ProfileId,
        domains: &[DomainId],
    ) -> PortabilityResult<Self> {
        let generation = master.generation_id(profile_id)?;
        let store_key = master.derive_store_key(profile_id)?;
        let mut derived = Vec::with_capacity(domains.len());
        for domain in domains {
            let crypto_domain = academic_crypto::DomainId::from_bytes(*domain.as_bytes());
            derived.push((
                *domain,
                master.derive_domain_kek(profile_id, crypto_domain)?,
            ));
        }
        Ok(Self {
            profile_id,
            generation,
            store_key,
            domains: derived,
        })
    }

    /// Returns the profile identity every derivation is salted with.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    /// Returns the public name of the generation these keys were derived from.
    ///
    /// `SKEY_p` and `KEK_d` both come from the Vault Master Key, so a key set
    /// belongs to exactly one generation. Recording which one is what lets a
    /// backup refuse to write a database under one generation beside objects
    /// under another: such a backup verifies and cannot be restored, because a
    /// restore re-derives both halves from the single master it recovered.
    #[must_use]
    pub const fn generation(&self) -> [u8; 32] {
        self.generation
    }


    /// Returns the raw SQLCipher store key.
    #[must_use]
    pub const fn store_key(&self) -> &StoreKey {
        &self.store_key
    }

    /// Returns the security domains this key set covers.
    #[must_use]
    pub fn domains(&self) -> Vec<DomainId> {
        self.domains.iter().map(|(domain, _)| *domain).collect()
    }

    /// Builds a fresh vault keyring from these keys.
    ///
    /// The keyring takes ownership of one KEK per domain, so a caller needing a
    /// second vault handle derives a second keyring rather than sharing one.
    pub fn keyring(&self, master: &VaultMasterKey) -> PortabilityResult<EncryptedDomainKeyring> {
        let mut keyring = EncryptedDomainKeyring::new(self.profile_id);
        for (domain, _) in &self.domains {
            let crypto_domain = academic_crypto::DomainId::from_bytes(*domain.as_bytes());
            keyring.insert(
                *domain,
                master.derive_domain_kek(self.profile_id, crypto_domain)?,
            )?;
        }
        Ok(keyring)
    }
}

/// Rejects a relative path that is not one this format produces.
pub(crate) fn object_relative_path(artifact_id: &str) -> PortabilityResult<String> {
    let relative = format!("{OBJECTS_DIRECTORY}/{artifact_id}.aobj");
    crate::directory::check_relative_path(&relative)?;
    Ok(relative)
}

/// Reports the encrypted database's relative path inside a backup.
#[must_use]
pub fn database_relative_path() -> String {
    format!(
        "{DATABASE_DIRECTORY}/{}",
        academic_store::STORE_DATABASE_FILE
    )
}

/// Fails closed when a copied database is readable without its key.
///
/// A SQLite Online Backup into an unkeyed destination writes *plaintext* pages.
/// That would turn a backup into the exact leak this whole task exists to
/// prevent, so the copy is proved unreadable before anything else is written
/// beside it.
pub(crate) fn require_unreadable_without_key(
    database_path: &std::path::Path,
) -> PortabilityResult<()> {
    let connection = rusqlite::Connection::open_with_flags(
        database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // An unkeyed handle on a SQLCipher file cannot read the schema at all: the
    // first page decrypts to noise and SQLite reports "file is not a database".
    let readable = connection
        .query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0))
        .is_ok();
    drop(connection);
    if readable {
        return Err(PortabilityError::DatabaseCheckFailed {
            check: "backup ciphertext",
            detail: "the copied database was readable without its key".to_owned(),
        });
    }
    Ok(())
}
