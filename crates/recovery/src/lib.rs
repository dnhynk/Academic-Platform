//! Recovery profiles, the independent backup key, and the restore rehearsal.
//!
//! Three things live here, and they exist together because each one is a
//! consequence of the other two.
//!
//! 1. **The recovery profile** (t068 section 3.3) says which recipients a
//!    profile has. It has no default: [`RecoveryProfile`] deliberately
//!    implements neither `Default` nor `Ord`, because `GATE-38-031` is a user
//!    decision and this crate must not make it.
//! 2. **The backup key** is a root of its own. It is generated when a backup
//!    key set is created and wrapped only by recovery-class recipients, so the
//!    operating-system device wrapper cannot produce it — not directly, and
//!    not by unwrapping the Vault Master Key and deriving from that. A profile
//!    whose recovery profile is `DEVICE_ONLY` therefore cannot seal a backup
//!    at all, which is the same fact its loss statement states in words.
//! 3. **The rehearsal receipt** records that a restore was actually carried
//!    out, against named key material. It is authenticated under the VMK, and
//!    it is stale the moment the key material changes.
//!
//! # What this crate does not do
//!
//! It takes no snapshot, opens no database, and reads no vault. It owns the
//! keys, the sealed envelope, and the admission rule; the backup *contents* —
//! the watermark, the counts, the object closure — belong to the encrypted
//! portability lane that calls into this crate.
//!
//! # Posture
//!
//! Nothing here is ADR-002, ADR-005, or ADR-012 acceptance. `adr_002_accepted`
//! stays `false`, `production_data_allowed` stays `false`, and the default
//! product lane stays `storage_encryption=NONE`.

pub mod backup_key;
pub mod envelope;
pub mod profile;
pub mod rehearsal;

pub use backup_key::{
    BACKUP_RECIPIENT_SET_VERSION, BACKUP_ROOT_INFO, BackupKeyError, BackupMasterKey,
    BackupRecipientKind, BackupRecipientRecord, BackupRecipientSet, BackupSetId,
    create_backup_key_set,
};
pub use envelope::{
    ENVELOPE_VERSION, MANIFEST_AEAD_INFO, MANIFEST_SIGNING_INFO, SealedManifest,
    SealedManifestError,
};
pub use profile::{
    DEVICE_ONLY_IRRECOVERABILITY_STATEMENT, RECOVERY_PROFILES, RecipientRequirement,
    RecoveryProfile, RecoveryProfileError,
};
pub use rehearsal::{
    IngestRefusal, KeyMaterialState, REHEARSAL_RECEIPT_RELATIVE_PATH, RehearsalError,
    RehearsalObservations, RehearsalReceipt, admit_first_ingest,
};

/// Frozen name of the `P2-K4` backup format.
pub const BACKUP_FORMAT_V2: &str = "ACADEMIC_ENCRYPTED_BACKUP_V2";

/// Exact backup manifest version this build writes and accepts.
pub const BACKUP_MANIFEST_VERSION: u32 = 2;

/// The `BK` and `RS` rows of the fault matrix this crate's callers re-run under
/// encryption. The failpoints themselves belong to the portability lane; the
/// list is repeated here so a report can name the closed set.
pub const PHASE2_BACKUP_FAULT_IDS: &[&str] = &[
    "BK01", "BK02", "BK03", "BK04", "RS01", "RS02", "RS03", "RS04",
];
