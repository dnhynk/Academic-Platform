//! Verified restore into a new empty profile.
//!
//! Restore never writes into a live profile. It builds a complete candidate in
//! a sibling staging directory, proves it, and only then publishes it with one
//! rename. Every failure therefore leaves both the backup source and the
//! current profile untouched, and the destination is either absent or a
//! completely verified profile — never a partially activated one.
//!
//! The order is fixed: accept only a new empty destination, validate policy,
//! version, and file hashes, open the database, run `integrity_check` and
//! `foreign_key_check`, replay every signed batch against independent trust
//! anchors, check object closure and plaintext digests, rebuild projections from
//! empty, compare counts, heads, and checksums, then publish.

use std::path::{Path, PathBuf};

use academic_contracts::DeviceAuthorization;
use academic_domain::{ContentDigest, DomainId};
use academic_projections::{
    generation::{ProjectionCoordinates, ProjectionKind},
    resolution::PredicatePolicies,
    runner::ProjectionRunner,
};
use academic_store::{
    INCOMPLETE_PROFILE_MARKER, connection::open_reader, path_policy::PathProbe,
    profile::prepare_synthetic_profile,
};
use academic_vault::{DomainKeyring, Vault};

use crate::{
    PortabilityError, PortabilityResult, RESTORE_INCOMPLETE_MARKER,
    backup::{VerifiedBackup, verify_backup_directory},
    checksum::encode_hex,
    directory,
    fault::{self, PortabilityFaultPoint},
    manifest::BackupManifest,
    verify::{CanonicalDatabase, ReplayReport, read_artifact_descriptors, read_canonical_rows},
};

/// Disposable projection sidecar created beside a restored canonical store.
pub const PROJECTION_SIDECAR_FILE: &str = "projection.sqlite3";
/// Sealed object namespace inside a restored profile.
pub const RESTORED_VAULT_OBJECTS_DIRECTORY: &str = "vault/v1";

/// One projection generation the restore must rebuild from empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionRebuildTarget {
    pub kind: ProjectionKind,
    pub domain: DomainId,
    pub coordinates: ProjectionCoordinates,
}

/// Caller-supplied trust anchors and projection plan for one restore.
///
/// Authorizations are deliberately not read from the backup: a signing key
/// carried inside a restored envelope would authenticate nothing.
#[derive(Debug)]
pub struct RestorePlan<'a> {
    pub authorizations: &'a [DeviceAuthorization],
    pub projections: &'a [ProjectionRebuildTarget],
    pub predicate_policies: Option<&'a PredicatePolicies>,
    pub projection_builder_digest: ContentDigest,
    pub projection_config_hash: ContentDigest,
}

/// One projection generation rebuilt from the restored canonical rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredProjection {
    pub kind: String,
    pub domain: String,
    pub record_count: u64,
    pub canonical_checksum: String,
    pub activated: bool,
}

/// Receipt returned by a published restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReceipt {
    pub destination: PathBuf,
    pub manifest: BackupManifest,
    pub replay: ReplayReport,
    pub projections: Vec<RestoredProjection>,
    pub canonical_semantic_digest: String,
}

/// Restores one verified backup into a new empty profile directory.
pub fn restore_profile<P: PathProbe + ?Sized>(
    backup_root: &Path,
    destination: &Path,
    probe: &P,
    keyring: DomainKeyring,
    plan: &RestorePlan<'_>,
) -> PortabilityResult<RestoreReceipt> {
    directory::require_new_empty_directory(destination)?;
    let verified = verify_backup_directory(backup_root)?;

    let staging = directory::reserve_staging_path(destination, "restore-staging")?;
    let incomplete = prepare_synthetic_profile(&staging, probe)?;
    let restore_marker = staging.join(RESTORE_INCOMPLETE_MARKER);
    directory::write_new_file(
        &restore_marker,
        restore_marker_contents(destination).as_bytes(),
    )?;
    directory::sync_directory(&staging)?;
    drop(incomplete);

    fault::trip(PortabilityFaultPoint::Rs01);

    let outcome = build_restored_profile(&staging, &verified, keyring, plan);
    let (replay, projections, canonical_semantic_digest) = outcome?;

    remove_marker(&staging, RESTORE_INCOMPLETE_MARKER)?;
    remove_marker(&staging, INCOMPLETE_PROFILE_MARKER)?;
    directory::sync_tree(&staging)?;

    fault::trip(PortabilityFaultPoint::Rs04);

    directory::publish_over_empty(&staging, destination)?;
    Ok(RestoreReceipt {
        destination: destination.to_path_buf(),
        manifest: verified.manifest,
        replay,
        projections,
        canonical_semantic_digest,
    })
}

type RestoreOutcome = (ReplayReport, Vec<RestoredProjection>, String);

fn build_restored_profile(
    staging: &Path,
    verified: &VerifiedBackup,
    keyring: DomainKeyring,
    plan: &RestorePlan<'_>,
) -> PortabilityResult<RestoreOutcome> {
    let semantic = &verified.manifest.semantic;
    let database_source = directory::resolve_relative(&verified.root, &semantic.database.path)?;
    let database_path = staging.join(academic_store::STORE_DATABASE_FILE);
    let (digest, byte_length) = directory::copy_new_file(&database_source, &database_path)?;
    if encode_hex(digest.as_bytes().as_slice()) != semantic.database.sha256
        || byte_length != semantic.database.byte_length
    {
        return Err(PortabilityError::mismatch(
            "restored database digest",
            &semantic.database.sha256,
            encode_hex(digest.as_bytes().as_slice()),
        ));
    }

    fault::trip(PortabilityFaultPoint::Rs02);

    let database = CanonicalDatabase::open_copy(&database_path)?;
    database.integrity_check()?;
    database.foreign_key_check()?;
    let rows = read_canonical_rows(&database)?;
    rows.schema.policy.require_phase1()?;
    if rows.schema != semantic.store_schema {
        return Err(PortabilityError::mismatch(
            "restored store schema",
            semantic.store_schema.schema_semver.clone(),
            rows.schema.schema_semver.clone(),
        ));
    }
    if rows.watermark != semantic.watermark {
        return Err(PortabilityError::WatermarkMoved {
            expected: semantic.watermark.accept_seq_head,
            actual: rows.watermark.accept_seq_head,
        });
    }
    if rows.counts != semantic.counts {
        return Err(PortabilityError::mismatch(
            "restored canonical counts",
            semantic.counts.events,
            rows.counts.events,
        ));
    }
    if rows.device_heads != semantic.device_heads {
        return Err(PortabilityError::mismatch(
            "restored device heads",
            semantic.device_heads.len(),
            rows.device_heads.len(),
        ));
    }
    let canonical_semantic_digest = encode_hex(rows.semantic_digest()?.as_bytes().as_slice());
    if canonical_semantic_digest != semantic.canonical_semantic_digest {
        return Err(PortabilityError::mismatch(
            "restored canonical semantic digest",
            &semantic.canonical_semantic_digest,
            canonical_semantic_digest,
        ));
    }

    let replay = crate::verify::replay_signed_batches(&database, plan.authorizations)?;
    if replay.verified_batches != rows.counts.batches
        || replay.verified_events != rows.counts.events
        || replay.device_heads != rows.counts.device_heads
    {
        return Err(PortabilityError::replay(
            "replay coverage",
            format!(
                "{} batches and {} events verified for {} stored batches and {} stored events",
                replay.verified_batches,
                replay.verified_events,
                rows.counts.batches,
                rows.counts.events
            ),
        ));
    }

    let descriptors = read_artifact_descriptors(&database)?;
    drop(database);
    remove_journal_sidecars(&database_path)?;

    let vault = Vault::open(staging, keyring)?;
    if descriptors.len() != semantic.objects.len() {
        return Err(PortabilityError::mismatch(
            "restored object closure",
            semantic.objects.len(),
            descriptors.len(),
        ));
    }
    for descriptor in &descriptors {
        let identifier = descriptor.id.to_string();
        let declared = semantic
            .objects
            .iter()
            .find(|object| object.artifact_id == identifier)
            .ok_or_else(|| PortabilityError::MissingObject {
                artifact_id: identifier.clone(),
            })?;
        let source = directory::resolve_relative(&verified.root, &declared.path)?;
        // The canonical vault path is re-derived from the signed descriptor, so
        // a manifest can never place an object where the store would not look
        // for it.
        let target = vault.layout().object_path(descriptor)?;
        if let Some(parent) = target.parent() {
            directory::create_directories(parent)?;
        }
        let (digest, byte_length) = directory::copy_new_file(&source, &target)?;
        if encode_hex(digest.as_bytes().as_slice()) != declared.plaintext_sha256
            || byte_length != declared.byte_length
        {
            return Err(PortabilityError::mismatch(
                "restored object digest",
                &declared.plaintext_sha256,
                encode_hex(digest.as_bytes().as_slice()),
            ));
        }
        if digest != descriptor.content_digest || byte_length != descriptor.byte_length {
            return Err(PortabilityError::mismatch(
                "restored object plaintext digest",
                descriptor.content_digest,
                digest,
            ));
        }
    }

    fault::trip(PortabilityFaultPoint::Rs03);

    for descriptor in &descriptors {
        vault.verify_sealed_object(descriptor)?;
    }

    let projections = rebuild_projections(staging, &database_path, plan)?;
    Ok((replay, projections, canonical_semantic_digest))
}

fn rebuild_projections(
    staging: &Path,
    database_path: &Path,
    plan: &RestorePlan<'_>,
) -> PortabilityResult<Vec<RestoredProjection>> {
    if plan.projections.is_empty() {
        return Ok(Vec::new());
    }
    let policies = plan
        .predicate_policies
        .ok_or(PortabilityError::ManifestRejected {
            field: "restore plan predicate policies",
        })?;
    let sidecar = staging.join(PROJECTION_SIDECAR_FILE);
    directory::require_absent(&sidecar)?;
    let reader = open_reader(database_path)?;
    let runner = ProjectionRunner::open(
        &reader,
        &sidecar,
        plan.projection_builder_digest,
        plan.projection_config_hash,
    )?;
    let mut rebuilt = Vec::with_capacity(plan.projections.len());
    for target in plan.projections {
        let receipt =
            runner.rebuild_at(target.kind, target.domain, target.coordinates, policies)?;
        let record_count =
            receipt
                .metadata
                .record_count
                .ok_or(PortabilityError::DatabaseCheckFailed {
                    check: "projection rebuild",
                    detail: "a verified generation reported no record count".to_owned(),
                })?;
        let checksum =
            receipt
                .metadata
                .canonical_checksum
                .ok_or(PortabilityError::DatabaseCheckFailed {
                    check: "projection rebuild",
                    detail: "a verified generation reported no canonical checksum".to_owned(),
                })?;
        if !receipt.activated {
            return Err(PortabilityError::DatabaseCheckFailed {
                check: "projection rebuild",
                detail: format!("generation for {} was not activated", target.kind),
            });
        }
        rebuilt.push(RestoredProjection {
            kind: target.kind.as_str().to_owned(),
            domain: target.domain.to_string(),
            record_count,
            canonical_checksum: encode_hex(checksum.as_bytes().as_slice()),
            activated: receipt.activated,
        });
    }
    drop(runner);
    drop(reader);
    remove_journal_sidecars(&sidecar)?;
    Ok(rebuilt)
}

fn remove_journal_sidecars(database_path: &Path) -> PortabilityResult<()> {
    let Some(name) = database_path.file_name().and_then(|value| value.to_str()) else {
        return Err(PortabilityError::UnsafeEntry(database_path.to_path_buf()));
    };
    let Some(parent) = database_path.parent() else {
        return Err(PortabilityError::UnsafeEntry(database_path.to_path_buf()));
    };
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = parent.join(format!("{name}{suffix}"));
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(PortabilityError::io(
                    "remove restore journal sidecar",
                    path,
                    source,
                ));
            }
        }
    }
    Ok(())
}

fn remove_marker(staging: &Path, marker: &str) -> PortabilityResult<()> {
    let path = staging.join(marker);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PortabilityError::io("remove restore marker", path, source)),
    }
}

fn restore_marker_contents(destination: &Path) -> String {
    format!(
        "ACADEMIC_PLATFORM_PHASE1_RESTORE_INCOMPLETE\ndestination={}\n",
        destination.display()
    )
}

/// Lists unpublished restore staging directories left beside a destination.
pub fn find_unpublished_restores(destination: &Path) -> PortabilityResult<Vec<PathBuf>> {
    directory::find_staging_directories(destination)
}

/// Removes one unpublished restore staging directory.
pub fn remove_unpublished_restore(destination: &Path, staging: &Path) -> PortabilityResult<()> {
    directory::remove_staging_directory(destination, staging)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_marker_records_its_intended_destination() {
        let contents = restore_marker_contents(Path::new("profile"));
        assert!(contents.starts_with("ACADEMIC_PLATFORM_PHASE1_RESTORE_INCOMPLETE\n"));
        assert!(contents.contains("destination=profile"));
        assert!(contents.ends_with('\n'));
    }

    #[test]
    fn projection_sidecar_is_not_the_canonical_database() {
        assert_ne!(PROJECTION_SIDECAR_FILE, academic_store::STORE_DATABASE_FILE);
    }
}
