//! The provenance manifest of export schema v2.
//!
//! # The shape, and why the instant is outside the digest
//!
//! ```json
//! { "semantic": { ... }, "semantic_digest": "<hex>", "volatile": { ... } }
//! ```
//!
//! `semantic_digest` is SHA-256 over the domain separator
//! [`crate::SEMANTIC_DIGEST_DOMAIN`], an unsigned big-endian 64-bit length, and
//! the compact canonical JSON of `semantic`. Every field is bound because the
//! JSON structure is in those bytes; the length is what keeps the separator
//! from running into the body, and it binds nothing on its own. Saying it
//! delimited each field would have been the stronger-sounding claim and the
//! false one — `P1-I14` removed the length and no test could see it, which is
//! what a claim nothing executes looks like.
//!
//! `volatile.generated_at_unix_ms` is a **parameter of the request**, not a
//! clock this crate reads. Two bundles of one watermark are then byte-identical
//! whole-file rather than identical-except-one-integer, which is what
//! `export_is_deterministic_at_a_fixed_watermark` observes; and a caller who
//! does record two different instants still gets one `semantic_digest`, which
//! is what the field being outside the digest is for.
//!
//! # A file record cannot exist without its three attributes
//!
//! [`FileRecord::new`] takes the sensitivity label, the sharing restriction and
//! the source copyright notice as parameters, the fields are private, there is
//! no setter and no `Default`. Section 32.10's three per-file attributes are
//! therefore a property of the type rather than of a check that could be
//! skipped for one file.

use academic_domain::ContentDigest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    BUNDLE_ENCRYPTED, ExportError, ExportResult, GRADUATION_EXPORT_FORMAT,
    GRADUATION_EXPORT_GENERATOR, GRADUATION_EXPORT_MANIFEST_VERSION, PROJECTIONS_INCLUDED,
    SEMANTIC_DIGEST_DOMAIN,
    label::{CopyrightNotice, SensitivityLabel, SharingRestriction},
    source::{DeviceHead, GitRef, StoreIdentity, Watermark, WithheldReason},
};

/// The posture a bundle records and a reader refuses to disagree with.
///
/// The values come from the profile the caller read, not from a constant here:
/// this crate cannot open a store, so restating a store's posture would be an
/// assertion it has no way to check. What it does own is the refusal —
/// [`Self::require_phase2_posture`] fails on a bundle claiming production data
/// is allowed, whoever wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostureBlock {
    /// The data policy in force when the bundle was written.
    pub data_policy: String,
    /// How the source profile stores its canonical rows.
    pub storage_mode: String,
    /// The source profile's storage encryption.
    pub storage_encryption: String,
    /// Whether real data was admissible. Always `false` in this build.
    pub production_data_allowed: bool,
    /// The product's network posture.
    pub product_network: String,
}

impl PostureBlock {
    /// Refuses a posture this build may not read or write.
    pub fn require_phase2_posture(&self) -> ExportResult<()> {
        if self.production_data_allowed {
            return Err(ExportError::mismatch(
                "bundle posture production_data_allowed",
                false,
                true,
            ));
        }
        for (item, value) in [
            ("bundle posture data_policy", &self.data_policy),
            ("bundle posture storage_mode", &self.storage_mode),
            (
                "bundle posture storage_encryption",
                &self.storage_encryption,
            ),
            ("bundle posture product_network", &self.product_network),
        ] {
            if value.trim().is_empty() {
                return Err(ExportError::Malformed {
                    item,
                    value: value.clone(),
                });
            }
        }
        Ok(())
    }
}

/// One file in a bundle, with section 32.10's three attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
    path: String,
    byte_length: u64,
    sha256: String,
    sensitivity: SensitivityLabel,
    sharing_restriction: SharingRestriction,
    copyright_notice: CopyrightNotice,
}

impl FileRecord {
    /// Records one file.
    ///
    /// The restriction is derived from the label rather than taken, so the two
    /// cannot be set to disagree — a `SECRET` file marked freely
    /// redistributable would have both fields populated and a complete-looking
    /// manifest.
    pub fn new(
        path: impl Into<String>,
        byte_length: u64,
        sha256: impl Into<String>,
        sensitivity: SensitivityLabel,
        copyright_notice: CopyrightNotice,
    ) -> Self {
        Self {
            path: path.into(),
            byte_length,
            sha256: sha256.into(),
            sensitivity,
            sharing_restriction: SharingRestriction::of(sensitivity),
            copyright_notice,
        }
    }

    /// The relative forward-slash path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The exact byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// The exact SHA-256, lowercase hex.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Section 32.10's sensitivity label.
    #[must_use]
    pub const fn sensitivity(&self) -> SensitivityLabel {
        self.sensitivity
    }

    /// Section 32.10's sharing restriction.
    #[must_use]
    pub const fn sharing_restriction(&self) -> SharingRestriction {
        self.sharing_restriction
    }

    /// Section 32.10's source copyright notice.
    #[must_use]
    pub const fn copyright_notice(&self) -> &CopyrightNotice {
        &self.copyright_notice
    }

    /// Whether the restriction still follows from the label.
    ///
    /// Read by [`crate::read::read_bundle`], because a manifest is bytes
    /// somebody else wrote and the constructor's guarantee holds only on the
    /// writing side.
    #[must_use]
    pub fn restriction_follows_label(&self) -> bool {
        self.sharing_restriction == SharingRestriction::of(self.sensitivity)
    }
}

/// One registered artifact as a bundle records it.
///
/// `path` is `Some` exactly when the original bytes travel. A withheld original
/// records its identity, its exact plaintext digest and length, and **no path**,
/// so there is nothing in a published bundle pointing at a file it does not
/// carry. The artifact identifier is the address: `vault_locator` is recorded
/// and is never a key, a filename or a path segment, because two artifacts with
/// identical bytes in one domain share one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectRecord {
    /// The artifact's own identifier.
    pub artifact_id: String,
    /// The security domain whose terms cover it.
    pub domain_id: String,
    /// The registered media type.
    pub media_type: String,
    /// The exact plaintext digest.
    pub plaintext_sha256: String,
    /// The exact plaintext length.
    pub byte_length: u64,
    /// The recorded vault locator, which addresses nothing here.
    pub vault_locator: String,
    /// Section 32.10's sensitivity label for the original.
    pub sensitivity: SensitivityLabel,
    /// Where the original bytes are, when they travel.
    pub path: Option<String>,
    /// Why they do not, when they do not.
    pub withheld: Option<WithheldReason>,
}

impl ObjectRecord {
    /// Refuses a record that both carries and withholds an original.
    pub fn validate(&self) -> ExportResult<()> {
        match (&self.path, self.withheld) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            (Some(path), Some(_)) => Err(ExportError::Malformed {
                item: "object record",
                value: format!("{} both carries {path} and is withheld", self.artifact_id),
            }),
            (None, None) => Err(ExportError::Malformed {
                item: "object record",
                value: format!(
                    "{} carries no original and states no reason",
                    self.artifact_id
                ),
            }),
        }
    }
}

/// One of section 37's six parts, as a bundle records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartRecord {
    /// The contract spelling of the part.
    pub part: String,
    /// Its directory below `parts/`.
    pub directory: String,
    /// Section 37's own bullet, verbatim.
    pub specification_sentence: String,
    /// Every file this part carries, sorted by path.
    pub files: Vec<String>,
}

/// The hashed half of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSemantic {
    /// The frozen format name.
    pub format: String,
    /// The frozen manifest version.
    pub manifest_version: u32,
    /// The generator identity.
    pub generator: String,
    /// The posture the source profile was under.
    pub policy: PostureBlock,
    /// Whether the bundle itself is encrypted. Always `false`.
    pub encrypted: bool,
    /// Whether projection generations travel. Always `false`.
    pub projections_included: bool,
    /// Whether original bytes travel, which is the user's choice.
    pub originals_included: bool,
    /// The physical store identity of the source.
    pub store: StoreIdentity,
    /// The committed watermark.
    pub watermark: Watermark,
    /// Every device head.
    pub device_heads: Vec<DeviceHead>,
    /// The canonical semantic digest of the exported watermark.
    pub canonical_semantic_digest: String,
    /// Counts of each canonical record class, as the bundle wrote them.
    pub counts: BundleCounts,
    /// The six parts, in section 37's order.
    pub parts: Vec<PartRecord>,
    /// Every registered artifact.
    pub objects: Vec<ObjectRecord>,
    /// Every version-control reference.
    pub git_refs: Vec<GitRef>,
    /// The recorded graduation audit.
    pub audit: crate::audit::AuditRecord,
    /// The manifest's own three attributes, which it cannot list inside itself.
    pub manifest_attributes: ManifestAttributes,
    /// Every file except the manifest, sorted by path.
    pub files: Vec<FileRecord>,
}

/// Counts the bundle wrote, per canonical record class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleCounts {
    /// Accepted batches.
    pub batches: u64,
    /// Accepted events.
    pub events: u64,
    /// Registered scopes.
    pub scopes: u64,
    /// Registered artifacts.
    pub artifacts: u64,
    /// Registered evidence items.
    pub evidence: u64,
    /// Asserted claims.
    pub claims: u64,
    /// Claim relations.
    pub relations: u64,
    /// Recorded user decisions.
    pub decisions: u64,
}

/// Section 32.10's three attributes for `manifest.json` itself.
///
/// The manifest cannot carry its own digest, so it cannot be one of its own
/// [`FileRecord`]s. It still has terms, a label and a restriction, and they are
/// here rather than absent — a file with no attributes is exactly what the
/// exhaustive check exists to catch, and the manifest is a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestAttributes {
    /// Its sensitivity label.
    pub sensitivity: SensitivityLabel,
    /// Its sharing restriction.
    pub sharing_restriction: SharingRestriction,
    /// Its source copyright notice.
    pub copyright_notice: CopyrightNotice,
}

impl ManifestAttributes {
    /// Whether the restriction still follows from the label.
    #[must_use]
    pub fn restriction_follows_label(&self) -> bool {
        self.sharing_restriction == SharingRestriction::of(self.sensitivity)
    }
}

/// The unhashed half of a manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleVolatile {
    /// The instant the caller recorded for this generation.
    pub generated_at_unix_ms: i64,
}

/// One complete provenance manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    /// The hashed half.
    pub semantic: BundleSemantic,
    /// SHA-256 over the hashed half, lowercase hex.
    pub semantic_digest: String,
    /// The unhashed half.
    pub volatile: BundleVolatile,
}

impl BundleManifest {
    /// Seals a semantic block with its digest and one recorded instant.
    pub fn seal(semantic: BundleSemantic, generated_at_unix_ms: i64) -> ExportResult<Self> {
        let semantic_digest = encode_hex(semantic_digest(&semantic)?.as_bytes().as_slice());
        Ok(Self {
            semantic,
            semantic_digest,
            volatile: BundleVolatile {
                generated_at_unix_ms,
            },
        })
    }

    /// Renders the exact manifest bytes a bundle carries.
    ///
    /// Two-space pretty JSON with one final newline, which is what the Phase 1
    /// export writes and what a person opening the directory in an editor can
    /// read without a tool.
    pub fn to_json_bytes(&self) -> ExportResult<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|source| ExportError::Json {
            operation: "render bundle manifest",
            source,
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Parses manifest bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> ExportResult<Self> {
        serde_json::from_slice(bytes).map_err(|source| ExportError::Json {
            operation: "parse bundle manifest",
            source,
        })
    }

    /// Recomputes the semantic digest and compares it with the recorded one.
    pub fn verify_semantic_digest(&self) -> ExportResult<()> {
        let observed = encode_hex(semantic_digest(&self.semantic)?.as_bytes().as_slice());
        if observed != self.semantic_digest {
            return Err(ExportError::mismatch(
                "bundle semantic digest",
                &self.semantic_digest,
                observed,
            ));
        }
        Ok(())
    }

    /// Refuses a manifest whose frozen fields are not this format's.
    pub fn require_v2_contract(&self) -> ExportResult<()> {
        if self.semantic.format != GRADUATION_EXPORT_FORMAT {
            return Err(ExportError::mismatch(
                "bundle format",
                GRADUATION_EXPORT_FORMAT,
                &self.semantic.format,
            ));
        }
        if self.semantic.manifest_version != GRADUATION_EXPORT_MANIFEST_VERSION {
            return Err(ExportError::mismatch(
                "bundle manifest version",
                GRADUATION_EXPORT_MANIFEST_VERSION,
                self.semantic.manifest_version,
            ));
        }
        if self.semantic.generator != GRADUATION_EXPORT_GENERATOR {
            return Err(ExportError::mismatch(
                "bundle generator",
                GRADUATION_EXPORT_GENERATOR,
                &self.semantic.generator,
            ));
        }
        if self.semantic.encrypted != BUNDLE_ENCRYPTED {
            return Err(ExportError::mismatch(
                "bundle encryption",
                BUNDLE_ENCRYPTED,
                self.semantic.encrypted,
            ));
        }
        if self.semantic.projections_included != PROJECTIONS_INCLUDED {
            return Err(ExportError::mismatch(
                "bundle projections_included",
                PROJECTIONS_INCLUDED,
                self.semantic.projections_included,
            ));
        }
        self.semantic.policy.require_phase2_posture()
    }
}

/// Hashes a semantic block: the domain separator, a length, and the body.
fn semantic_digest(semantic: &BundleSemantic) -> ExportResult<ContentDigest> {
    let mut hasher = Sha256::new();
    hasher.update(SEMANTIC_DIGEST_DOMAIN.as_bytes());
    let bytes = serde_json::to_vec(semantic).map_err(|source| ExportError::Json {
        operation: "render bundle semantic block",
        source,
    })?;
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(&bytes);
    Ok(ContentDigest::from_sha256_bytes(hasher.finalize().into()))
}

/// Renders bytes as lowercase hexadecimal.
pub fn encode_hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        rendered.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::{FileRecord, ObjectRecord, PostureBlock, encode_hex};
    use crate::{
        label::{CopyrightNotice, SensitivityLabel, SharingRestriction},
        source::WithheldReason,
    };

    fn posture(production_data_allowed: bool) -> PostureBlock {
        PostureBlock {
            data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED".to_owned(),
            storage_mode: "PLAINTEXT_TEMPORARY_SQLITE".to_owned(),
            storage_encryption: "NONE".to_owned(),
            production_data_allowed,
            product_network: "NONE".to_owned(),
        }
    }

    #[test]
    fn a_posture_admitting_production_data_is_refused() {
        assert!(posture(false).require_phase2_posture().is_ok());
        assert!(posture(true).require_phase2_posture().is_err());
    }

    #[test]
    fn a_file_record_derives_its_restriction_from_its_label()
    -> Result<(), Box<dyn std::error::Error>> {
        for label in SensitivityLabel::ALL {
            let record = FileRecord::new(
                "parts/machine-readable-graph/x.jsonl",
                7,
                "00",
                label,
                CopyrightNotice::new("terms")?,
            );
            assert_eq!(record.sharing_restriction(), SharingRestriction::of(label));
            assert!(record.restriction_follows_label());
        }
        Ok(())
    }

    #[test]
    fn an_object_record_carries_a_path_or_a_reason_and_never_both() {
        let base = ObjectRecord {
            artifact_id: "a".to_owned(),
            domain_id: "d".to_owned(),
            media_type: "text/plain".to_owned(),
            plaintext_sha256: "00".to_owned(),
            byte_length: 1,
            vault_locator: "ff".to_owned(),
            sensitivity: SensitivityLabel::Restricted,
            path: None,
            withheld: None,
        };
        assert!(base.validate().is_err());

        let mut carried = base.clone();
        carried.path = Some("parts/lecture-and-question-archive/originals/d/a.bin".to_owned());
        assert!(carried.validate().is_ok());

        let mut withheld = base.clone();
        withheld.withheld = Some(WithheldReason::UserExcludedOriginals);
        assert!(withheld.validate().is_ok());

        let mut both = carried;
        both.withheld = Some(WithheldReason::UserExcludedOriginals);
        assert!(both.validate().is_err());
    }

    #[test]
    fn hexadecimal_is_lowercase_and_fixed_width() {
        assert_eq!(encode_hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    }
}
