//! Versioned export and backup manifests with an explicit volatile boundary.
//!
//! Every manifest separates a `semantic` block from a `volatile` block. The
//! semantic block is hashed into `semantic_digest`; the volatile block carries
//! the generation instant and is deliberately outside that digest, so two runs
//! at the same committed watermark agree on the digest even though their
//! manifest bytes differ by one integer.

use serde::{Deserialize, Serialize};

use crate::{
    PHASE1_BACKUP_FORMAT, PHASE1_BACKUP_MANIFEST_VERSION, PHASE1_BACKUP_PLAINTEXT_WARNING,
    PHASE1_EXPORT_FORMAT, PHASE1_EXPORT_MANIFEST_VERSION, PHASE1_PORTABILITY_GENERATOR,
    PortabilityError, PortabilityResult,
    checksum::{CanonicalDigest, encode_hex},
    verify::{
        BatchRow, CanonicalCounts, CanonicalWatermark, DeviceHeadRow, PolicyBlock,
        StoreSchemaIdentity, canonical_json,
    },
};

/// Domain separator for the export manifest digest.
pub const EXPORT_SEMANTIC_DIGEST_DOMAIN: &str = "learning-platform.phase1.export-manifest.v1";
/// Domain separator for the backup manifest digest.
pub const BACKUP_SEMANTIC_DIGEST_DOMAIN: &str = "learning-platform.phase1.backup-manifest.v1";

/// One produced file with its exact content digest.
///
/// `path` is always relative to the directory root and always uses forward
/// slashes, so a Windows-produced manifest and a Linux-produced manifest are
/// byte-identical. Filesystem metadata is never recorded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub byte_length: u64,
    pub sha256: String,
}

/// One reachable sealed object copied beside the canonical rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectEntry {
    pub artifact_id: String,
    pub domain_id: String,
    pub retention_class: String,
    pub permission_lineage_id: String,
    pub vault_locator: String,
    pub path: String,
    pub byte_length: u64,
    pub plaintext_sha256: String,
}

/// Non-semantic bookkeeping deliberately excluded from every digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolatileBlock {
    pub generated_at_unix_ms: i64,
}

/// Hashed content of one deterministic export directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportSemantic {
    pub format: String,
    pub manifest_version: u32,
    pub generator: String,
    pub policy: PolicyBlock,
    pub encrypted: bool,
    pub projections_included: bool,
    pub store_schema: StoreSchemaIdentity,
    pub watermark: CanonicalWatermark,
    pub counts: CanonicalCounts,
    pub device_heads: Vec<DeviceHeadRow>,
    pub batches: Vec<BatchRow>,
    pub objects: Vec<ObjectEntry>,
    pub canonical_semantic_digest: String,
    pub files: Vec<FileEntry>,
}

/// Complete export manifest as written to `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportManifest {
    pub semantic: ExportSemantic,
    pub semantic_digest: String,
    pub volatile: VolatileBlock,
}

impl ExportManifest {
    /// Seals a semantic block with its digest and the volatile generation time.
    pub fn seal(semantic: ExportSemantic, generated_at_unix_ms: i64) -> PortabilityResult<Self> {
        let semantic_digest = digest_of(EXPORT_SEMANTIC_DIGEST_DOMAIN, &semantic)?;
        Ok(Self {
            semantic,
            semantic_digest,
            volatile: VolatileBlock {
                generated_at_unix_ms,
            },
        })
    }

    /// Recomputes the semantic digest and rejects a mismatch.
    pub fn verify_semantic_digest(&self) -> PortabilityResult<()> {
        let recomputed = digest_of(EXPORT_SEMANTIC_DIGEST_DOMAIN, &self.semantic)?;
        if recomputed == self.semantic_digest {
            Ok(())
        } else {
            Err(PortabilityError::mismatch(
                "export semantic digest",
                &self.semantic_digest,
                recomputed,
            ))
        }
    }

    /// Rejects any manifest that is not the frozen Phase 1 export contract.
    pub fn require_phase1_contract(&self) -> PortabilityResult<()> {
        if self.semantic.format != PHASE1_EXPORT_FORMAT {
            return Err(PortabilityError::ManifestRejected { field: "format" });
        }
        if self.semantic.manifest_version != PHASE1_EXPORT_MANIFEST_VERSION {
            return Err(PortabilityError::ManifestRejected {
                field: "manifest_version",
            });
        }
        if self.semantic.generator != PHASE1_PORTABILITY_GENERATOR {
            return Err(PortabilityError::ManifestRejected { field: "generator" });
        }
        if self.semantic.encrypted {
            return Err(PortabilityError::ManifestRejected { field: "encrypted" });
        }
        if self.semantic.projections_included {
            return Err(PortabilityError::ManifestRejected {
                field: "projections_included",
            });
        }
        self.semantic.policy.require_phase1()?;
        self.semantic.store_schema.policy.require_phase1()?;
        require_sorted_unique(&self.semantic.files)
    }

    /// Renders the manifest as deterministic UTF-8 JSON with a final newline.
    pub fn to_json_bytes(&self) -> PortabilityResult<Vec<u8>> {
        render_json(self)
    }

    /// Parses a manifest from exact JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> PortabilityResult<Self> {
        parse_json(bytes, "parse export manifest")
    }
}

/// Hashed content of one plaintext synthetic backup directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupSemantic {
    pub format: String,
    pub manifest_version: u32,
    pub generator: String,
    pub policy: PolicyBlock,
    pub encrypted: bool,
    pub plaintext_warning: String,
    pub store_schema: StoreSchemaIdentity,
    pub watermark: CanonicalWatermark,
    pub counts: CanonicalCounts,
    pub device_heads: Vec<DeviceHeadRow>,
    pub canonical_semantic_digest: String,
    pub database: FileEntry,
    pub objects: Vec<ObjectEntry>,
    pub files: Vec<FileEntry>,
}

/// Complete backup manifest as written to `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupManifest {
    pub semantic: BackupSemantic,
    pub semantic_digest: String,
    pub volatile: VolatileBlock,
}

impl BackupManifest {
    /// Seals a semantic block with its digest and the volatile generation time.
    pub fn seal(semantic: BackupSemantic, generated_at_unix_ms: i64) -> PortabilityResult<Self> {
        let semantic_digest = digest_of(BACKUP_SEMANTIC_DIGEST_DOMAIN, &semantic)?;
        Ok(Self {
            semantic,
            semantic_digest,
            volatile: VolatileBlock {
                generated_at_unix_ms,
            },
        })
    }

    /// Recomputes the semantic digest and rejects a mismatch.
    pub fn verify_semantic_digest(&self) -> PortabilityResult<()> {
        let recomputed = digest_of(BACKUP_SEMANTIC_DIGEST_DOMAIN, &self.semantic)?;
        if recomputed == self.semantic_digest {
            Ok(())
        } else {
            Err(PortabilityError::mismatch(
                "backup semantic digest",
                &self.semantic_digest,
                recomputed,
            ))
        }
    }

    /// Rejects any manifest that is not the frozen Phase 1 backup contract.
    pub fn require_phase1_contract(&self) -> PortabilityResult<()> {
        if self.semantic.format != PHASE1_BACKUP_FORMAT {
            return Err(PortabilityError::ManifestRejected { field: "format" });
        }
        if self.semantic.manifest_version != PHASE1_BACKUP_MANIFEST_VERSION {
            return Err(PortabilityError::ManifestRejected {
                field: "manifest_version",
            });
        }
        if self.semantic.generator != PHASE1_PORTABILITY_GENERATOR {
            return Err(PortabilityError::ManifestRejected { field: "generator" });
        }
        if self.semantic.encrypted {
            return Err(PortabilityError::ManifestRejected { field: "encrypted" });
        }
        if self.semantic.plaintext_warning != PHASE1_BACKUP_PLAINTEXT_WARNING {
            return Err(PortabilityError::ManifestRejected {
                field: "plaintext_warning",
            });
        }
        self.semantic.policy.require_phase1()?;
        self.semantic.store_schema.policy.require_phase1()?;
        require_sorted_unique(&self.semantic.files)
    }

    /// Renders the manifest as deterministic UTF-8 JSON with a final newline.
    pub fn to_json_bytes(&self) -> PortabilityResult<Vec<u8>> {
        render_json(self)
    }

    /// Parses a manifest from exact JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> PortabilityResult<Self> {
        parse_json(bytes, "parse backup manifest")
    }
}

fn digest_of<T: Serialize>(domain: &str, value: &T) -> PortabilityResult<String> {
    let mut digest = CanonicalDigest::new(domain);
    digest.field(&canonical_json(value)?);
    Ok(encode_hex(digest.finish().as_bytes().as_slice()))
}

fn render_json<T: Serialize>(value: &T) -> PortabilityResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PortabilityError::Json {
        operation: "render manifest",
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn parse_json<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    operation: &'static str,
) -> PortabilityResult<T> {
    serde_json::from_slice(bytes).map_err(|source| PortabilityError::Json { operation, source })
}

fn require_sorted_unique(files: &[FileEntry]) -> PortabilityResult<()> {
    for window in files.windows(2) {
        let (left, right) = (&window[0], &window[1]);
        if left.path >= right.path {
            return Err(PortabilityError::ManifestRejected {
                field: "files (must be strictly sorted by path)",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<FileEntry> {
        vec![
            FileEntry {
                path: "a.jsonl".to_owned(),
                byte_length: 1,
                sha256: "00".repeat(32),
            },
            FileEntry {
                path: "b.jsonl".to_owned(),
                byte_length: 2,
                sha256: "11".repeat(32),
            },
        ]
    }

    #[test]
    fn unsorted_file_lists_are_rejected() {
        let mut files = sample_files();
        files.reverse();
        assert!(require_sorted_unique(&files).is_err());
        files.reverse();
        assert!(require_sorted_unique(&files).is_ok());
    }

    #[test]
    fn duplicate_file_paths_are_rejected() {
        let entry = FileEntry {
            path: "a.jsonl".to_owned(),
            byte_length: 1,
            sha256: "00".repeat(32),
        };
        assert!(require_sorted_unique(&[entry.clone(), entry]).is_err());
    }

    #[test]
    fn volatile_generation_time_is_outside_the_digest() -> PortabilityResult<()> {
        let semantic = ExportSemantic {
            format: PHASE1_EXPORT_FORMAT.to_owned(),
            manifest_version: PHASE1_EXPORT_MANIFEST_VERSION,
            generator: PHASE1_PORTABILITY_GENERATOR.to_owned(),
            policy: PolicyBlock::phase1(),
            encrypted: false,
            projections_included: false,
            store_schema: StoreSchemaIdentity {
                format_uuid: "00".repeat(16),
                schema_version: 1,
                schema_semver: "1.0.0".to_owned(),
                minimum_reader_protocol_major: 1,
                minimum_reader_protocol_minor: 0,
                minimum_writer_protocol_major: 1,
                minimum_writer_protocol_minor: 0,
                policy: PolicyBlock::phase1(),
            },
            watermark: CanonicalWatermark {
                next_accept_seq: 1,
                profile_revision: 0,
                accept_seq_head: 0,
                outbox_head: 0,
            },
            counts: CanonicalCounts {
                batches: 0,
                events: 0,
                scopes: 0,
                artifacts: 0,
                artifact_representations: 0,
                evidence: 0,
                claims: 0,
                claim_evidence_links: 0,
                relations: 0,
                decisions: 0,
                outbox: 0,
                command_receipts: 0,
                device_heads: 0,
            },
            device_heads: Vec::new(),
            batches: Vec::new(),
            objects: Vec::new(),
            canonical_semantic_digest: "22".repeat(32),
            files: sample_files(),
        };
        let first = ExportManifest::seal(semantic.clone(), 1)?;
        let second = ExportManifest::seal(semantic, 999_999)?;
        assert_eq!(first.semantic_digest, second.semantic_digest);
        assert_ne!(first.to_json_bytes()?, second.to_json_bytes()?);
        first.verify_semantic_digest()?;
        first.require_phase1_contract()?;

        let bytes = first.to_json_bytes()?;
        assert_eq!(bytes.last().copied(), Some(b'\n'));
        let parsed = ExportManifest::from_json_bytes(&bytes)?;
        assert_eq!(parsed, first);
        assert_eq!(parsed.to_json_bytes()?, bytes);
        Ok(())
    }
}
