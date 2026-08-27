//! Startup reconciliation for partials, orphans, quarantine, and authoritative references.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use academic_domain::{ArtifactDescriptor, ArtifactId};

use crate::{Vault, VaultError, VaultResult, durability, ingest::verify_object};

const DEFAULT_TEMP_EXPIRY: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_ORPHAN_GRACE: Duration = Duration::from_secs(24 * 60 * 60);
const DIRECTORY_BARRIER_FILE: &str = ".academic-vault-directory-barrier";

/// Configuration and authoritative descriptor sets consumed by one reconciliation pass.
#[derive(Debug, Clone)]
pub struct ReconcileOptions<'a> {
    referenced: &'a [ArtifactDescriptor],
    retry_candidates: &'a [ArtifactDescriptor],
    now: SystemTime,
    temp_expiry: Duration,
    orphan_grace: Duration,
}

impl<'a> ReconcileOptions<'a> {
    /// Creates an empty-reference reconciliation pass at an explicit clock value.
    #[must_use]
    pub const fn new(now: SystemTime) -> Self {
        Self {
            referenced: &[],
            retry_candidates: &[],
            now,
            temp_expiry: DEFAULT_TEMP_EXPIRY,
            orphan_grace: DEFAULT_ORPHAN_GRACE,
        }
    }

    /// Supplies canonical descriptors actually referenced by the store.
    #[must_use]
    pub const fn with_referenced(mut self, referenced: &'a [ArtifactDescriptor]) -> Self {
        self.referenced = referenced;
        self
    }

    /// Supplies descriptors from a trusted idempotent retry context.
    ///
    /// These candidates let reconciliation call an object a valid orphan without inventing a
    /// media type, digest, or permission policy from an opaque HMAC filename.
    #[must_use]
    pub const fn with_retry_candidates(
        mut self,
        retry_candidates: &'a [ArtifactDescriptor],
    ) -> Self {
        self.retry_candidates = retry_candidates;
        self
    }

    /// Overrides the expired-partial threshold.
    #[must_use]
    pub const fn with_temp_expiry(mut self, temp_expiry: Duration) -> Self {
        self.temp_expiry = temp_expiry;
        self
    }

    /// Overrides the unreferenced-object quarantine grace window.
    #[must_use]
    pub const fn with_orphan_grace(mut self, orphan_grace: Duration) -> Self {
        self.orphan_grace = orphan_grace;
        self
    }
}

/// Explicit state assigned to one physical or referenced object during reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileState {
    TempLive,
    TempExpiredRemoved,
    OrphanPendingGrace,
    ValidOrphan,
    QuarantinedOrphan,
    ReferencedValid,
    ReferencedMissingRepairRequired,
    ReferencedCorruptRepairRequired,
    UnsafeEntry,
}

/// One evidence row from a reconciliation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileRecord {
    state: ReconcileState,
    path: PathBuf,
    artifact_id: Option<ArtifactId>,
}

impl ReconcileRecord {
    /// Returns the explicit reconciliation state.
    #[must_use]
    pub const fn state(&self) -> ReconcileState {
        self.state
    }

    /// Returns the physical path inspected or mutated.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the canonical artifact ID when an authoritative descriptor supplied one.
    #[must_use]
    pub const fn artifact_id(&self) -> Option<ArtifactId> {
        self.artifact_id
    }
}

/// Complete deterministic outcome from one reconciliation pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReconcileReport {
    records: Vec<ReconcileRecord>,
}

impl ReconcileReport {
    /// Returns every temp/object/quarantine/reference decision.
    #[must_use]
    pub fn records(&self) -> &[ReconcileRecord] {
        &self.records
    }

    /// Returns true when any canonical reference is missing or corrupt.
    #[must_use]
    pub fn repair_required(&self) -> bool {
        self.records.iter().any(|record| {
            matches!(
                record.state,
                ReconcileState::ReferencedMissingRepairRequired
                    | ReconcileState::ReferencedCorruptRepairRequired
            )
        })
    }
}

pub(crate) fn reconcile(
    vault: &Vault,
    options: &ReconcileOptions<'_>,
) -> VaultResult<ReconcileReport> {
    let mut report = ReconcileReport::default();
    reconcile_temps(vault, options, &mut report)?;

    let mut referenced_paths = BTreeSet::new();
    for descriptor in options.referenced {
        let path = vault.validate_descriptor_locator(descriptor)?;
        referenced_paths.insert(path.clone());
        match fs::symlink_metadata(&path) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                report.records.push(record(
                    ReconcileState::ReferencedMissingRepairRequired,
                    path,
                    Some(descriptor.id),
                ));
            }
            Err(source) => {
                return Err(VaultError::io(
                    "inspect referenced vault object",
                    path,
                    source,
                ));
            }
            Ok(_) => {
                match verify_object(&path, descriptor.content_digest, descriptor.byte_length) {
                    Ok(()) => report.records.push(record(
                        ReconcileState::ReferencedValid,
                        path,
                        Some(descriptor.id),
                    )),
                    Err(VaultError::IntegrityMismatch(_)) => report.records.push(record(
                        ReconcileState::ReferencedCorruptRepairRequired,
                        path,
                        Some(descriptor.id),
                    )),
                    Err(error) => return Err(error),
                }
            }
        }
    }

    let mut retry_paths = BTreeMap::new();
    for descriptor in options.retry_candidates {
        let path = vault.validate_descriptor_locator(descriptor)?;
        retry_paths.entry(path).or_insert(descriptor);
    }

    let mut object_files = Vec::new();
    walk_objects(
        vault.layout.objects_root(),
        vault,
        &mut object_files,
        &mut report,
    )?;
    for path in object_files {
        if referenced_paths.contains(&path) {
            continue;
        }
        let age = file_age(&path, options.now)?;
        let candidate = retry_paths.get(&path).copied();
        let candidate_is_valid = if let Some(descriptor) = candidate {
            match verify_object(&path, descriptor.content_digest, descriptor.byte_length) {
                Ok(()) => true,
                Err(VaultError::IntegrityMismatch(_)) => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };
        if candidate_is_valid {
            report.records.push(record(
                ReconcileState::ValidOrphan,
                path,
                candidate.map(|descriptor| descriptor.id),
            ));
        } else if age >= options.orphan_grace {
            let quarantined = quarantine(vault, &path, options.now)?;
            report.records.push(record(
                ReconcileState::QuarantinedOrphan,
                quarantined,
                candidate.map(|descriptor| descriptor.id),
            ));
        } else {
            report
                .records
                .push(record(ReconcileState::OrphanPendingGrace, path, None));
        }
    }

    reconcile_existing_quarantine(vault, &mut report)?;
    Ok(report)
}

fn reconcile_temps(
    vault: &Vault,
    options: &ReconcileOptions<'_>,
    report: &mut ReconcileReport,
) -> VaultResult<()> {
    for entry in fs::read_dir(vault.layout.temp_dir()).map_err(|source| {
        VaultError::io(
            "enumerate vault temp directory",
            vault.layout.temp_dir(),
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            VaultError::io(
                "read vault temp directory entry",
                vault.layout.temp_dir(),
                source,
            )
        })?;
        if entry.file_name() == DIRECTORY_BARRIER_FILE {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| VaultError::io("inspect vault temp", &path, source))?;
        if !metadata.file_type().is_file()
            || crate::layout::is_link_or_reparse(&metadata)
            || path.extension().and_then(|value| value.to_str()) != Some("partial")
        {
            report
                .records
                .push(record(ReconcileState::UnsafeEntry, path, None));
            continue;
        }
        if file_age_from_metadata(&metadata, options.now)? < options.temp_expiry {
            report
                .records
                .push(record(ReconcileState::TempLive, path, None));
            continue;
        }
        if durability::try_remove_unlocked(&path)? {
            durability::sync_directory(vault.layout.temp_dir())?;
            report
                .records
                .push(record(ReconcileState::TempExpiredRemoved, path, None));
        } else {
            report
                .records
                .push(record(ReconcileState::TempLive, path, None));
        }
    }
    Ok(())
}

fn walk_objects(
    directory: &Path,
    vault: &Vault,
    files: &mut Vec<PathBuf>,
    report: &mut ReconcileReport,
) -> VaultResult<()> {
    for entry in fs::read_dir(directory)
        .map_err(|source| VaultError::io("enumerate vault object namespace", directory, source))?
    {
        let entry = entry.map_err(|source| {
            VaultError::io("read vault object namespace entry", directory, source)
        })?;
        if entry.file_name() == DIRECTORY_BARRIER_FILE {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| VaultError::io("inspect vault object entry", &path, source))?;
        if crate::layout::is_link_or_reparse(&metadata) {
            report
                .records
                .push(record(ReconcileState::UnsafeEntry, path, None));
        } else if metadata.file_type().is_dir() {
            walk_objects(&path, vault, files, report)?;
        } else if metadata.file_type().is_file() && vault.layout.is_canonical_object_path(&path) {
            files.push(path);
        } else {
            report
                .records
                .push(record(ReconcileState::UnsafeEntry, path, None));
        }
    }
    Ok(())
}

fn reconcile_existing_quarantine(vault: &Vault, report: &mut ReconcileReport) -> VaultResult<()> {
    let already_reported = report
        .records
        .iter()
        .filter(|record| record.state == ReconcileState::QuarantinedOrphan)
        .map(|record| record.path.clone())
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(vault.layout.quarantine_dir()).map_err(|source| {
        VaultError::io(
            "enumerate vault quarantine directory",
            vault.layout.quarantine_dir(),
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            VaultError::io(
                "read vault quarantine entry",
                vault.layout.quarantine_dir(),
                source,
            )
        })?;
        if entry.file_name() == DIRECTORY_BARRIER_FILE {
            continue;
        }
        let path = entry.path();
        if already_reported.contains(&path) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| VaultError::io("inspect quarantined orphan", &path, source))?;
        let state = if metadata.file_type().is_file()
            && !crate::layout::is_link_or_reparse(&metadata)
            && path.extension().and_then(|value| value.to_str()) == Some("orphan")
        {
            ReconcileState::QuarantinedOrphan
        } else {
            ReconcileState::UnsafeEntry
        };
        report.records.push(record(state, path, None));
    }
    Ok(())
}

fn quarantine(vault: &Vault, source: &Path, now: SystemTime) -> VaultResult<PathBuf> {
    let locator = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| VaultError::UnsafeEntry(source.to_path_buf()))?;
    let timestamp = system_time_millis(now)?;
    let destination = vault
        .layout
        .quarantine_dir()
        .join(format!("{timestamp:013}-{locator}.orphan"));
    if !durability::publish_no_replace(source, &destination)? {
        return Err(VaultError::PathCollision(destination));
    }
    let source_parent = source
        .parent()
        .ok_or_else(|| VaultError::UnsafeEntry(source.to_path_buf()))?;
    durability::sync_directory(source_parent)?;
    durability::sync_directory(vault.layout.quarantine_dir())?;
    Ok(destination)
}

fn file_age(path: &Path, now: SystemTime) -> VaultResult<Duration> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| VaultError::io("inspect vault entry age", path, source))?;
    file_age_from_metadata(&metadata, now)
}

fn file_age_from_metadata(metadata: &fs::Metadata, now: SystemTime) -> VaultResult<Duration> {
    let modified = metadata.modified().map_err(|source| {
        VaultError::io(
            "read vault entry modification time",
            "<vault-entry>",
            source,
        )
    })?;
    Ok(now.duration_since(modified).unwrap_or(Duration::ZERO))
}

fn system_time_millis(value: SystemTime) -> VaultResult<u64> {
    let elapsed = value
        .duration_since(UNIX_EPOCH)
        .map_err(|_| VaultError::ClockUnavailable)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| VaultError::ClockUnavailable)
}

fn record(
    state: ReconcileState,
    path: PathBuf,
    artifact_id: Option<ArtifactId>,
) -> ReconcileRecord {
    ReconcileRecord {
        state,
        path,
        artifact_id,
    }
}
