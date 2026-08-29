//! Backup manifest v2: the sealed inventory of one encrypted backup.
//!
//! The manifest separates a `semantic` block from a `volatile` block exactly as
//! the Phase 1 manifests do, and for the same reason: two backups taken at the
//! same committed watermark must agree on the semantic digest even though their
//! bytes differ by a generation instant and a fresh AEAD nonce.
//!
//! Every field that would tell a reader anything about the profile is inside
//! the sealed body. That includes the profile's own recovery-class recipient
//! records, which is what makes a fresh-machine restore possible from a phrase
//! and a backup directory alone.

use academic_recovery::{BackupMasterKey, BackupSetId, SealedManifest};
use serde::{Deserialize, Serialize};

use crate::{
    PortabilityError, PortabilityResult,
    manifest::{FileEntry, VolatileBlock, digest_of, require_sorted_unique},
    verify::{
        CanonicalCounts, CanonicalWatermark, DeviceHeadRow, PolicyBlock, StoreSchemaIdentity,
    },
};

/// Frozen contract name of the encrypted backup format.
pub const BACKUP_FORMAT: &str = academic_recovery::BACKUP_FORMAT_V2;
/// Exact manifest version this build writes and accepts.
pub const BACKUP_MANIFEST_VERSION: u32 = academic_recovery::BACKUP_MANIFEST_VERSION;
/// Stable identity of the writer that produced an encrypted backup.
pub const BACKUP_GENERATOR: &str = "learning-platform.phase2-encrypted-backup.v2";
/// Domain separator for the encrypted backup manifest digest.
pub const SEMANTIC_DIGEST_DOMAIN: &str = "learning-platform.phase2.encrypted-backup-manifest.v2";
/// Domain separator for the digest two backups of one watermark must share.
pub const IDENTITY_DIGEST_DOMAIN: &str = "learning-platform.phase2.encrypted-backup-identity.v2";

/// One sealed object copied beside the encrypted database.
///
/// Two digests, and they are not interchangeable. `ciphertext_sha256` is what a
/// verifier can check without a key; `plaintext_sha256` is the descriptor's own
/// content digest and only ever appears inside the sealed body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedObjectEntry {
    pub artifact_id: String,
    pub domain_id: String,
    pub retention_class: String,
    pub permission_lineage_id: String,
    pub vault_locator: String,
    pub format_version: u32,
    pub path: String,
    pub byte_length: u64,
    pub ciphertext_sha256: String,
    pub plaintext_sha256: String,
    pub plaintext_byte_length: u64,
}

/// Hashed content of one encrypted backup directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedBackupSemantic {
    pub format: String,
    pub manifest_version: u32,
    pub generator: String,
    pub policy: PolicyBlock,
    pub encrypted: bool,
    pub object_format: String,
    pub recovery_profile: String,
    pub backup_set_id: String,
    pub profile_id: String,
    /// The profile's recovery-class recipient records, canonical CBOR in hex.
    ///
    /// Device recipients are deliberately absent: they are useless on the fresh
    /// machine a restore targets, and carrying them would invite a restore that
    /// depends on the device that was lost.
    pub profile_recovery_recipients: String,
    pub store_schema: StoreSchemaIdentity,
    pub watermark: CanonicalWatermark,
    pub counts: CanonicalCounts,
    pub device_heads: Vec<DeviceHeadRow>,
    pub canonical_semantic_digest: String,
    pub database: FileEntry,
    pub objects: Vec<EncryptedObjectEntry>,
    pub files: Vec<FileEntry>,
}

/// The part of a backup that two runs at one watermark must agree on.
///
/// The whole `semantic` block cannot be that part. SQLCipher re-encrypts every
/// page it writes with a fresh initialisation vector, so two Online Backup
/// copies of one unchanged database are different byte strings with different
/// SHA-256 digests, and so is every sealed object copied beside them — the
/// ciphertext is identical only because it is copied, but the database is
/// re-encrypted. A digest over the file inventory would therefore differ for
/// two backups that describe exactly the same committed state, which is the
/// opposite of what a semantic digest is for.
///
/// This block is what is left when the physical layer is removed: the format,
/// the posture, the schema identity, the watermark, the counts, the device
/// heads, the canonical semantic digest, and each object's *logical* identity
/// and plaintext digest. Two backups of one watermark agree on all of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SemanticIdentity<'a> {
    format: &'a str,
    manifest_version: u32,
    object_format: &'a str,
    policy: &'a PolicyBlock,
    profile_id: &'a str,
    store_schema: &'a StoreSchemaIdentity,
    watermark: &'a CanonicalWatermark,
    counts: &'a CanonicalCounts,
    device_heads: &'a [DeviceHeadRow],
    canonical_semantic_digest: &'a str,
    objects: Vec<ObjectIdentity<'a>>,
}

/// One object's logical identity, with no physical path or ciphertext digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ObjectIdentity<'a> {
    artifact_id: &'a str,
    domain_id: &'a str,
    retention_class: &'a str,
    permission_lineage_id: &'a str,
    vault_locator: &'a str,
    format_version: u32,
    plaintext_sha256: &'a str,
    plaintext_byte_length: u64,
}

impl EncryptedBackupSemantic {
    /// Digests the part of this backup that is a claim about committed state.
    ///
    /// Objects are sorted by artifact identifier first, so two backups that
    /// enumerated the same closure in a different order still agree.
    pub fn identity_digest(&self) -> PortabilityResult<String> {
        let mut objects: Vec<ObjectIdentity<'_>> = self
            .objects
            .iter()
            .map(|object| ObjectIdentity {
                artifact_id: &object.artifact_id,
                domain_id: &object.domain_id,
                retention_class: &object.retention_class,
                permission_lineage_id: &object.permission_lineage_id,
                vault_locator: &object.vault_locator,
                format_version: object.format_version,
                plaintext_sha256: &object.plaintext_sha256,
                plaintext_byte_length: object.plaintext_byte_length,
            })
            .collect();
        objects.sort_by(|left, right| left.artifact_id.cmp(right.artifact_id));
        digest_of(
            IDENTITY_DIGEST_DOMAIN,
            &SemanticIdentity {
                format: &self.format,
                manifest_version: self.manifest_version,
                object_format: &self.object_format,
                policy: &self.policy,
                profile_id: &self.profile_id,
                store_schema: &self.store_schema,
                watermark: &self.watermark,
                counts: &self.counts,
                device_heads: &self.device_heads,
                canonical_semantic_digest: &self.canonical_semantic_digest,
                objects,
            },
        )
    }
}

/// Complete encrypted backup manifest, before sealing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedBackupManifest {
    pub semantic: EncryptedBackupSemantic,
    pub semantic_digest: String,
    pub semantic_identity_digest: String,
    pub volatile: VolatileBlock,
}

impl EncryptedBackupManifest {
    /// Seals a semantic block with its digest and the volatile generation time.
    pub fn new(
        semantic: EncryptedBackupSemantic,
        generated_at_unix_ms: i64,
    ) -> PortabilityResult<Self> {
        let semantic_digest = digest_of(SEMANTIC_DIGEST_DOMAIN, &semantic)?;
        let semantic_identity_digest = semantic.identity_digest()?;
        Ok(Self {
            semantic,
            semantic_digest,
            semantic_identity_digest,
            volatile: VolatileBlock {
                generated_at_unix_ms,
            },
        })
    }

    /// Recomputes both digests and rejects a mismatch in either.
    pub fn verify_semantic_digest(&self) -> PortabilityResult<()> {
        let recomputed = digest_of(SEMANTIC_DIGEST_DOMAIN, &self.semantic)?;
        if recomputed != self.semantic_digest {
            return Err(PortabilityError::mismatch(
                "encrypted backup semantic digest",
                &self.semantic_digest,
                recomputed,
            ));
        }
        let identity = self.semantic.identity_digest()?;
        if identity != self.semantic_identity_digest {
            return Err(PortabilityError::mismatch(
                "encrypted backup semantic identity digest",
                &self.semantic_identity_digest,
                identity,
            ));
        }
        Ok(())
    }

    /// Rejects any manifest that is not the frozen `P2-K4` contract.
    pub fn require_contract(&self) -> PortabilityResult<()> {
        if self.semantic.format != BACKUP_FORMAT {
            return Err(PortabilityError::ManifestRejected { field: "format" });
        }
        if self.semantic.manifest_version != BACKUP_MANIFEST_VERSION {
            return Err(PortabilityError::ManifestRejected {
                field: "manifest_version",
            });
        }
        if self.semantic.generator != BACKUP_GENERATOR {
            return Err(PortabilityError::ManifestRejected { field: "generator" });
        }
        if !self.semantic.encrypted {
            return Err(PortabilityError::ManifestRejected { field: "encrypted" });
        }
        if self.semantic.object_format != academic_vault::ENCRYPTED_OBJECT_FORMAT {
            return Err(PortabilityError::ManifestRejected {
                field: "object_format",
            });
        }
        if academic_recovery::RecoveryProfile::parse(&self.semantic.recovery_profile).is_none() {
            return Err(PortabilityError::ManifestRejected {
                field: "recovery_profile",
            });
        }
        self.semantic.policy.require_encrypted_v2()?;
        self.semantic.store_schema.policy.require_encrypted_v2()?;
        require_sorted_unique(&self.semantic.files)
    }

    /// Renders the manifest as deterministic JSON bytes for sealing.
    pub fn to_body_bytes(&self) -> PortabilityResult<Vec<u8>> {
        crate::verify::canonical_json(self)
    }

    /// Parses a manifest from the exact bytes a sealed envelope produced.
    pub fn from_body_bytes(bytes: &[u8]) -> PortabilityResult<Self> {
        let parsed: Self =
            serde_json::from_slice(bytes).map_err(|source| PortabilityError::Json {
                operation: "parse encrypted backup manifest",
                source,
            })?;
        // A manifest whose bytes are not the one canonical rendering would let
        // two byte-strings claim the same semantic digest.
        if parsed.to_body_bytes()? != bytes {
            return Err(PortabilityError::ManifestRejected {
                field: "manifest body encoding",
            });
        }
        Ok(parsed)
    }

    /// Seals and signs this manifest under a backup root.
    pub fn seal(&self, root: &BackupMasterKey, set_id: BackupSetId) -> PortabilityResult<Vec<u8>> {
        let sealed = SealedManifest::seal(
            root,
            set_id,
            BACKUP_FORMAT,
            BACKUP_MANIFEST_VERSION,
            &self.to_body_bytes()?,
        )?;
        Ok(sealed.to_canonical_cbor()?)
    }

    /// Verifies, opens, and validates a sealed manifest.
    ///
    /// The order is fixed: signature, then decryption, then the frozen
    /// contract, then the semantic digest. A caller therefore never sees a
    /// field out of a manifest whose signature did not verify.
    pub fn open(bytes: &[u8], root: &BackupMasterKey) -> PortabilityResult<Self> {
        let sealed = SealedManifest::from_canonical_cbor(bytes)?;
        if sealed.format() != BACKUP_FORMAT {
            return Err(PortabilityError::ManifestRejected { field: "format" });
        }
        if sealed.manifest_version() != BACKUP_MANIFEST_VERSION {
            return Err(PortabilityError::ManifestRejected {
                field: "manifest_version",
            });
        }
        let body = sealed.open(root)?;
        let manifest = Self::from_body_bytes(&body)?;
        manifest.require_contract()?;
        manifest.verify_semantic_digest()?;
        if manifest.semantic.backup_set_id
            != crate::checksum::encode_hex(sealed.set_id().as_bytes().as_slice())
        {
            return Err(PortabilityError::ManifestRejected {
                field: "backup_set_id",
            });
        }
        Ok(manifest)
    }

    /// Verifies only the signature, which needs no key material at all.
    pub fn verify_signature(bytes: &[u8]) -> PortabilityResult<()> {
        Ok(SealedManifest::from_canonical_cbor(bytes)?.verify_signature()?)
    }
}
