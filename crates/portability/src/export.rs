//! Deterministic open-directory export of one synthetic profile.
//!
//! The export is a directory, never an archive, because archive containers
//! record filesystem metadata and ordering that differ between hosts. Two
//! exports taken at the same committed watermark therefore produce identical
//! per-file hashes and an identical semantic manifest digest on Windows and
//! Linux; only the manifest's volatile generation instant differs.
//!
//! The export deliberately excludes disposable projection generations: a graph
//! or search row is never canonical truth and is always rebuilt from the ledger.

use std::path::{Path, PathBuf};

use academic_vault::{DomainKeyring, Vault};

use crate::{
    PHASE1_EXPORT_FORMAT, PHASE1_EXPORT_MANIFEST_VERSION, PHASE1_PORTABILITY_GENERATOR,
    PortabilityError, PortabilityResult,
    checksum::{encode_hex, hash_file},
    directory,
    manifest::{ExportManifest, ExportSemantic, FileEntry, ObjectEntry},
    verify::{
        ArtifactRow, CanonicalDatabase, CanonicalRows, PolicyBlock, canonical_json,
        read_artifact_descriptors, read_batch_envelope, read_canonical_rows, uuid_bytes,
    },
};

/// Exact JSON Schema shipped inside every export directory.
pub const EXPORT_MANIFEST_SCHEMA: &str =
    include_str!("../../../schemas/jsonschema/phase1-export-v1.schema.json");

/// Relative path of the manifest inside an export directory.
pub const MANIFEST_FILE: &str = "manifest.json";
/// Relative path of the human-readable inventory.
pub const INVENTORY_FILE: &str = "inventory.md";
/// Exact receipt-derived posture object carried by every export.
pub const POSTURE_FILE: &str = "posture.json";
/// Relative path of the embedded manifest schema.
pub const MANIFEST_SCHEMA_FILE: &str = "schemas/phase1-export-v1.schema.json";
/// Relative path of the exported physical store identity.
pub const STORE_SCHEMA_FILE: &str = "schemas/store-schema-v1.json";
/// Directory holding one byte-for-byte original signed envelope per batch.
pub const LEDGER_BATCH_DIRECTORY: &str = "ledger/batches";
/// Relative path of the accepted-event record stream.
pub const LEDGER_EVENTS_FILE: &str = "ledger/events.jsonl";
/// Directory holding the exported canonical record streams.
pub const CANONICAL_DIRECTORY: &str = "canonical";
/// Directory holding one exact plaintext object per registered artifact.
pub const OBJECTS_DIRECTORY: &str = "objects";

/// Ordered canonical record streams written under `canonical/`.
pub const CANONICAL_FILES: &[&str] = &[
    "canonical/artifacts.jsonl",
    "canonical/claims.jsonl",
    "canonical/decisions.jsonl",
    "canonical/evidence.jsonl",
    "canonical/relations.jsonl",
    "canonical/scopes.jsonl",
];

/// Receipt returned by a completed export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReceipt {
    pub destination: PathBuf,
    pub manifest: ExportManifest,
}

/// Writes one deterministic export directory for a synthetic profile.
///
/// The destination must not exist. Everything is built in a sibling staging
/// directory and published with one rename, so an interrupted export leaves an
/// unpublished staging root and no partial destination.
pub fn export_profile(
    profile_root: &Path,
    destination: &Path,
    keyring: DomainKeyring,
) -> PortabilityResult<ExportReceipt> {
    directory::require_absent(destination)?;
    let database_path = profile_root.join(academic_store::STORE_DATABASE_FILE);
    let database = CanonicalDatabase::open_source(&database_path)?;
    // The manifest is written from the first read set and never re-checked, so
    // that read set has to be one snapshot and it is wider than any single read
    // function: the canonical rows, the artifact descriptors, and the batch
    // envelopes `write_export` reads back all have to come from the same
    // commit. Backup does not need this outer guard — it compares its source
    // read against the Online-Backup copy and fails closed on drift — but an
    // export has nothing to compare against.
    let snapshot = database.begin_read()?;
    let rows = read_canonical_rows(&database)?;
    rows.schema.policy.require_phase1()?;
    let canonical_semantic_digest = encode_hex(rows.semantic_digest()?.as_bytes().as_slice());
    let descriptors = read_artifact_descriptors(&database)?;
    let vault = Vault::open(profile_root, keyring)?;

    let staging = directory::reserve_staging_path(destination, "export-staging")?;
    directory::create_new_directory(&staging)?;
    let posture = academic_admission::AdmissionVerifier::posture(profile_root);
    directory::write_new_file(&staging.join(POSTURE_FILE), &posture.canonical_json_bytes())?;
    let result = write_export(
        &staging,
        &database,
        &rows,
        &descriptors,
        &vault,
        &canonical_semantic_digest,
    );
    let objects = match result {
        Ok(objects) => objects,
        Err(error) => {
            drop(snapshot);
            drop(database);
            return Err(error);
        }
    };
    drop(snapshot);
    drop(database);

    let mut files = Vec::new();
    for relative in directory::list_files(&staging)? {
        directory::check_relative_path(&relative)?;
        let path = directory::resolve_relative(&staging, &relative)?;
        let (digest, byte_length) = hash_file(&path)
            .map_err(|source| PortabilityError::io("hash exported file", &path, source))?;
        files.push(FileEntry {
            path: relative,
            byte_length,
            sha256: encode_hex(digest.as_bytes().as_slice()),
        });
    }
    files.sort();

    let semantic = ExportSemantic {
        format: PHASE1_EXPORT_FORMAT.to_owned(),
        manifest_version: PHASE1_EXPORT_MANIFEST_VERSION,
        generator: PHASE1_PORTABILITY_GENERATOR.to_owned(),
        policy: PolicyBlock::phase1(),
        encrypted: false,
        projections_included: false,
        store_schema: rows.schema.clone(),
        watermark: rows.watermark,
        counts: rows.counts,
        device_heads: rows.device_heads.clone(),
        batches: rows.batches.clone(),
        objects,
        canonical_semantic_digest,
        files,
    };
    let manifest = ExportManifest::seal(semantic, directory::now_unix_millis()?)?;
    manifest.require_phase1_contract()?;
    directory::write_new_file(&staging.join(MANIFEST_FILE), &manifest.to_json_bytes()?)?;

    directory::sync_tree(&staging)?;
    directory::publish(&staging, destination)?;
    Ok(ExportReceipt {
        destination: destination.to_path_buf(),
        manifest,
    })
}

fn write_export(
    staging: &Path,
    database: &CanonicalDatabase,
    rows: &CanonicalRows,
    descriptors: &[academic_domain::ArtifactDescriptor],
    vault: &Vault,
    canonical_semantic_digest: &str,
) -> PortabilityResult<Vec<ObjectEntry>> {
    directory::create_directories(&staging.join("schemas"))?;
    directory::write_new_file(
        &staging.join(MANIFEST_SCHEMA_FILE),
        EXPORT_MANIFEST_SCHEMA.as_bytes(),
    )?;
    let mut store_schema = canonical_json(&rows.schema)?;
    store_schema.push(b'\n');
    directory::write_new_file(&staging.join(STORE_SCHEMA_FILE), &store_schema)?;

    directory::create_directories(&staging.join(LEDGER_BATCH_DIRECTORY))?;
    for batch in &rows.batches {
        let envelope = read_batch_envelope(database.connection(), &batch.batch_id)?;
        let observed = encode_hex(
            academic_domain::ContentDigest::sha256(&envelope)
                .as_bytes()
                .as_slice(),
        );
        if observed != batch.envelope_sha256 {
            return Err(PortabilityError::mismatch(
                "exported signed envelope digest",
                &batch.envelope_sha256,
                observed,
            ));
        }
        let path = staging
            .join(LEDGER_BATCH_DIRECTORY)
            .join(format!("{}.cbor", batch.batch_id));
        directory::write_new_file(&path, &envelope)?;
    }
    directory::write_new_file(&staging.join(LEDGER_EVENTS_FILE), &jsonl(&rows.events)?)?;

    directory::create_directories(&staging.join(CANONICAL_DIRECTORY))?;
    directory::write_new_file(
        &staging.join("canonical/artifacts.jsonl"),
        &jsonl(&rows.artifacts)?,
    )?;
    directory::write_new_file(
        &staging.join("canonical/claims.jsonl"),
        &jsonl(&rows.claims)?,
    )?;
    directory::write_new_file(
        &staging.join("canonical/decisions.jsonl"),
        &jsonl(&rows.decisions)?,
    )?;
    directory::write_new_file(
        &staging.join("canonical/evidence.jsonl"),
        &jsonl(&rows.evidence)?,
    )?;
    directory::write_new_file(
        &staging.join("canonical/relations.jsonl"),
        &jsonl(&rows.relations)?,
    )?;
    directory::write_new_file(
        &staging.join("canonical/scopes.jsonl"),
        &jsonl(&rows.scopes)?,
    )?;

    let mut objects = Vec::with_capacity(descriptors.len());
    if !descriptors.is_empty() {
        directory::create_directories(&staging.join(OBJECTS_DIRECTORY))?;
    }
    for descriptor in descriptors {
        let sealed = vault.verify_sealed_object(descriptor)?;
        let relative = format!(
            "{OBJECTS_DIRECTORY}/{}/{}.bin",
            descriptor.domain_id, descriptor.id
        );
        directory::check_relative_path(&relative)?;
        let path = directory::resolve_relative(staging, &relative)?;
        if let Some(parent) = path.parent() {
            directory::create_directories(parent)?;
        }
        let (digest, byte_length) = directory::copy_new_file(sealed.object_path(), &path)?;
        if digest != descriptor.content_digest || byte_length != descriptor.byte_length {
            return Err(PortabilityError::mismatch(
                "exported artifact object",
                descriptor.content_digest,
                digest,
            ));
        }
        objects.push(ObjectEntry {
            artifact_id: descriptor.id.to_string(),
            domain_id: descriptor.domain_id.to_string(),
            retention_class: retention_name(descriptor.retention_class).to_owned(),
            permission_lineage_id: descriptor.permission_lineage_id.to_string(),
            vault_locator: encode_hex(descriptor.vault_locator.as_bytes().as_slice()),
            path: relative,
            byte_length,
            plaintext_sha256: encode_hex(digest.as_bytes().as_slice()),
        });
    }

    directory::write_new_file(
        &staging.join(INVENTORY_FILE),
        inventory(rows, &objects, canonical_semantic_digest).as_bytes(),
    )?;
    Ok(objects)
}

const fn retention_name(value: academic_domain::RetentionClass) -> &'static str {
    match value {
        academic_domain::RetentionClass::Ephemeral => "EPHEMERAL",
        academic_domain::RetentionClass::CourseTerm => "COURSE_TERM",
        academic_domain::RetentionClass::UserManaged => "USER_MANAGED",
        academic_domain::RetentionClass::LegalHold => "LEGAL_HOLD",
    }
}

fn jsonl<T: serde::Serialize>(rows: &[T]) -> PortabilityResult<Vec<u8>> {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(&canonical_json(row)?);
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn inventory(
    rows: &CanonicalRows,
    objects: &[ObjectEntry],
    canonical_semantic_digest: &str,
) -> String {
    let mut text = String::new();
    text.push_str("# Phase 1 synthetic export inventory\n\n");
    text.push_str(academic_store::PHASE1_POLICY_BANNER);
    text.push_str("\n\n");
    text.push_str("This export contains synthetic fixtures only. It is not encrypted, it is not ");
    text.push_str("evidence for ADR-002 or ADR-012, and it must never receive real data.\n\n");
    text.push_str("## Contract\n\n");
    text.push_str(&format!("- format: `{PHASE1_EXPORT_FORMAT}`\n"));
    text.push_str(&format!(
        "- manifest version: {PHASE1_EXPORT_MANIFEST_VERSION}\n"
    ));
    text.push_str("- encrypted: false\n");
    text.push_str("- projections included: false\n");
    text.push_str(&format!(
        "- canonical semantic digest: `{canonical_semantic_digest}`\n"
    ));
    text.push_str(&format!(
        "- store schema: {} (`{}`)\n\n",
        rows.schema.schema_semver, rows.schema.format_uuid
    ));
    text.push_str("## Watermark\n\n");
    text.push_str("| field | value |\n|---|---:|\n");
    text.push_str(&format!(
        "| next acceptance sequence | {} |\n",
        rows.watermark.next_accept_seq
    ));
    text.push_str(&format!(
        "| profile revision | {} |\n",
        rows.watermark.profile_revision
    ));
    text.push_str(&format!(
        "| acceptance head | {} |\n",
        rows.watermark.accept_seq_head
    ));
    text.push_str(&format!(
        "| outbox head | {} |\n\n",
        rows.watermark.outbox_head
    ));
    text.push_str("## Counts\n\n");
    text.push_str("| record | count |\n|---|---:|\n");
    for (label, value) in [
        ("batches", rows.counts.batches),
        ("events", rows.counts.events),
        ("scopes", rows.counts.scopes),
        ("artifacts", rows.counts.artifacts),
        (
            "artifact representations",
            rows.counts.artifact_representations,
        ),
        ("evidence", rows.counts.evidence),
        ("claims", rows.counts.claims),
        ("claim evidence links", rows.counts.claim_evidence_links),
        ("relations", rows.counts.relations),
        ("decisions", rows.counts.decisions),
        ("outbox", rows.counts.outbox),
        ("command receipts", rows.counts.command_receipts),
        ("device heads", rows.counts.device_heads),
    ] {
        text.push_str(&format!("| {label} | {value} |\n"));
    }
    text.push_str("\n## Device heads\n\n");
    text.push_str("| device | next origin sequence | head envelope |\n|---|---:|---|\n");
    for head in &rows.device_heads {
        text.push_str(&format!(
            "| {} | {} | `{}` |\n",
            head.device_id, head.next_origin_seq, head.head_envelope_sha256
        ));
    }
    text.push_str("\n## Objects\n\n");
    text.push_str("| artifact | path | bytes |\n|---|---|---:|\n");
    for object in objects {
        text.push_str(&format!(
            "| {} | `{}` | {} |\n",
            object.artifact_id, object.path, object.byte_length
        ));
    }
    text
}

/// One export directory verified without any repository or network access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedExport {
    pub root: PathBuf,
    pub manifest: ExportManifest,
}

/// Re-reads and fully verifies a published export directory.
///
/// This is the vendor-neutral consumer path: it needs only the directory bytes.
/// Every listed file must exist with the exact recorded digest and length, no
/// unlisted file may exist beside the manifest, and the semantic digest must
/// recompute.
pub fn verify_export_directory(root: &Path) -> PortabilityResult<VerifiedExport> {
    let manifest_path = root.join(MANIFEST_FILE);
    let bytes = std::fs::read(&manifest_path)
        .map_err(|source| PortabilityError::io("read export manifest", &manifest_path, source))?;
    let manifest = ExportManifest::from_json_bytes(&bytes)?;
    manifest.require_phase1_contract()?;
    manifest.verify_semantic_digest()?;

    let mut expected: Vec<&str> = manifest
        .semantic
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    expected.push(MANIFEST_FILE);
    expected.sort_unstable();
    let observed = directory::list_files(root)?;
    if observed != expected {
        return Err(PortabilityError::mismatch(
            "export directory inventory",
            expected.join(", "),
            observed.join(", "),
        ));
    }

    for entry in &manifest.semantic.files {
        let path = directory::resolve_relative(root, &entry.path)?;
        let (digest, byte_length) = hash_file(&path)
            .map_err(|source| PortabilityError::io("hash exported file", &path, source))?;
        if encode_hex(digest.as_bytes().as_slice()) != entry.sha256
            || byte_length != entry.byte_length
        {
            return Err(PortabilityError::mismatch(
                "exported file digest",
                &entry.sha256,
                encode_hex(digest.as_bytes().as_slice()),
            ));
        }
    }

    for batch in &manifest.semantic.batches {
        uuid_bytes(&batch.batch_id)?;
        let relative = format!("{LEDGER_BATCH_DIRECTORY}/{}.cbor", batch.batch_id);
        let entry = manifest
            .semantic
            .files
            .iter()
            .find(|file| file.path == relative)
            .ok_or_else(|| {
                PortabilityError::mismatch("exported signed envelope", relative.clone(), "absent")
            })?;
        if entry.sha256 != batch.envelope_sha256 || entry.byte_length != batch.envelope_byte_length
        {
            return Err(PortabilityError::mismatch(
                "exported signed envelope binding",
                &batch.envelope_sha256,
                &entry.sha256,
            ));
        }
    }

    for object in &manifest.semantic.objects {
        let entry = manifest
            .semantic
            .files
            .iter()
            .find(|file| file.path == object.path)
            .ok_or_else(|| PortabilityError::MissingObject {
                artifact_id: object.artifact_id.clone(),
            })?;
        if entry.sha256 != object.plaintext_sha256 || entry.byte_length != object.byte_length {
            return Err(PortabilityError::mismatch(
                "exported object binding",
                &object.plaintext_sha256,
                &entry.sha256,
            ));
        }
    }

    Ok(VerifiedExport {
        root: root.to_path_buf(),
        manifest,
    })
}

/// Reads the exported canonical artifact stream without a database.
pub fn read_exported_artifacts(root: &Path) -> PortabilityResult<Vec<ArtifactRow>> {
    let path = root.join("canonical/artifacts.jsonl");
    let bytes = std::fs::read(&path)
        .map_err(|source| PortabilityError::io("read exported artifacts", &path, source))?;
    let mut rows = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        rows.push(
            serde_json::from_slice(line).map_err(|source| PortabilityError::Json {
                operation: "parse exported artifact record",
                source,
            })?,
        );
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_stream_names_are_sorted_and_complete() {
        assert!(CANONICAL_FILES.is_sorted());
        assert_eq!(CANONICAL_FILES.len(), 6);
        for name in CANONICAL_FILES {
            assert!(name.starts_with("canonical/"));
            assert!(name.ends_with(".jsonl"));
        }
    }

    #[test]
    fn embedded_schema_matches_the_frozen_export_contract() -> PortabilityResult<()> {
        let schema: serde_json::Value =
            serde_json::from_str(EXPORT_MANIFEST_SCHEMA).map_err(|source| {
                PortabilityError::Json {
                    operation: "parse embedded export schema",
                    source,
                }
            })?;
        let semantic = &schema["properties"]["semantic"]["properties"];
        assert_eq!(semantic["format"]["const"], PHASE1_EXPORT_FORMAT);
        assert_eq!(
            semantic["manifest_version"]["const"],
            PHASE1_EXPORT_MANIFEST_VERSION
        );
        assert_eq!(semantic["encrypted"]["const"], false);
        assert_eq!(semantic["projections_included"]["const"], false);
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        Ok(())
    }
}
