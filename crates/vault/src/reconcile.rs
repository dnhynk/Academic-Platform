//! Startup reconciliation for partials, orphans, quarantine, and authoritative references.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use academic_domain::{ArtifactDescriptor, ArtifactId};
use sha2::{Digest as _, Sha256};

use crate::{VaultError, VaultResult, durability, encode_hex, layout::VaultLayout};

const DEFAULT_TEMP_EXPIRY: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_ORPHAN_GRACE: Duration = Duration::from_secs(24 * 60 * 60);
const DIRECTORY_BARRIER_FILE: &str = ".academic-vault-directory-barrier";
const QUARANTINE_SUFFIX: &str = ".orphan";
const MAX_QUARANTINE_FILENAME_BYTES: usize = 20 + 1 + 64 + 1 + 64 + QUARANTINE_SUFFIX.len();

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
    OrphanLeaseHeld,
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

/// The object namespace one reconciliation pass walks.
///
/// Reconciliation is identical for both object formats: it removes expired
/// partials, decides orphans against the authoritative descriptor set, and
/// quarantines what no reference reaches. Only three things differ between a
/// plaintext and an encrypted vault, and they are exactly the three methods
/// here: which physical namespace the objects live in, how a descriptor's
/// locator is validated, and what "read this object back exactly" means.
///
/// Keeping it a trait is what stops the encrypted lane from growing a second,
/// weaker reconciliation with its own orphan and quarantine rules.
pub(crate) trait ObjectNamespace {
    /// Returns the physical layout this namespace is bound to.
    fn layout(&self) -> &VaultLayout;

    /// Validates a descriptor against this namespace and returns its canonical path.
    fn validate_descriptor_locator(&self, descriptor: &ArtifactDescriptor) -> VaultResult<PathBuf>;

    /// Reads one canonical object back exactly.
    ///
    /// `Err(VaultError::IntegrityMismatch)` means an object is present at the
    /// canonical path but is not the one the descriptor names. Every other
    /// error is an inspection failure and stops the pass.
    fn verify_object(&self, descriptor: &ArtifactDescriptor) -> VaultResult<()>;
}

pub(crate) fn reconcile<N: ObjectNamespace>(
    vault: &N,
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
            Ok(_) => match vault.verify_object(descriptor) {
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
            },
        }
    }

    let mut retry_paths = BTreeMap::new();
    for descriptor in options.retry_candidates {
        let path = vault.validate_descriptor_locator(descriptor)?;
        retry_paths.entry(path).or_insert(descriptor);
    }

    let mut object_files = Vec::new();
    walk_objects(
        vault.layout().objects_root(),
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
            match vault.verify_object(descriptor) {
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
            if let Some(quarantined) = quarantine(vault, &path, options.now)? {
                report.records.push(record(
                    ReconcileState::QuarantinedOrphan,
                    quarantined,
                    candidate.map(|descriptor| descriptor.id),
                ));
            } else {
                report.records.push(record(
                    ReconcileState::OrphanLeaseHeld,
                    path,
                    candidate.map(|descriptor| descriptor.id),
                ));
            }
        } else {
            report
                .records
                .push(record(ReconcileState::OrphanPendingGrace, path, None));
        }
    }

    reconcile_existing_quarantine(vault, &mut report)?;
    Ok(report)
}

fn reconcile_temps<N: ObjectNamespace>(
    vault: &N,
    options: &ReconcileOptions<'_>,
    report: &mut ReconcileReport,
) -> VaultResult<()> {
    for entry in fs::read_dir(vault.layout().temp_dir()).map_err(|source| {
        VaultError::io(
            "enumerate vault temp directory",
            vault.layout().temp_dir(),
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            VaultError::io(
                "read vault temp directory entry",
                vault.layout().temp_dir(),
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
            durability::sync_directory(vault.layout().temp_dir())?;
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

fn walk_objects<N: ObjectNamespace>(
    directory: &Path,
    vault: &N,
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
        } else if metadata.file_type().is_file() && vault.layout().is_canonical_object_path(&path) {
            files.push(path);
        } else {
            report
                .records
                .push(record(ReconcileState::UnsafeEntry, path, None));
        }
    }
    Ok(())
}

fn reconcile_existing_quarantine<N: ObjectNamespace>(
    vault: &N,
    report: &mut ReconcileReport,
) -> VaultResult<()> {
    let already_reported = report
        .records
        .iter()
        .filter(|record| record.state == ReconcileState::QuarantinedOrphan)
        .map(|record| record.path.clone())
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(vault.layout().quarantine_dir()).map_err(|source| {
        VaultError::io(
            "enumerate vault quarantine directory",
            vault.layout().quarantine_dir(),
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            VaultError::io(
                "read vault quarantine entry",
                vault.layout().quarantine_dir(),
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

fn quarantine<N: ObjectNamespace>(
    vault: &N,
    source: &Path,
    now: SystemTime,
) -> VaultResult<Option<PathBuf>> {
    let lease_path = vault.layout().ensure_lease_path_for_object(source)?;
    let Some(_lease) = durability::try_acquire_exclusive_object_lease(&lease_path)? else {
        return Ok(None);
    };
    match fs::symlink_metadata(source) {
        Ok(metadata)
            if metadata.file_type().is_file() && !crate::layout::is_link_or_reparse(&metadata) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(VaultError::io(
                "reinspect leased vault orphan",
                source,
                error,
            ));
        }
        Ok(_) => return Err(VaultError::UnsafeEntry(source.to_path_buf())),
    }
    let locator = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| VaultError::UnsafeEntry(source.to_path_buf()))?;
    let path_identity = quarantine_path_identity(vault, source)?;
    let timestamp = system_time_millis(now)?;
    let filename = format!("{timestamp:013}-{path_identity}-{locator}{QUARANTINE_SUFFIX}");
    if filename.len() > MAX_QUARANTINE_FILENAME_BYTES || !filename.is_ascii() {
        return Err(VaultError::UnsafeEntry(source.to_path_buf()));
    }
    let destination = vault.layout().quarantine_dir().join(filename);
    if !durability::publish_no_replace(source, &destination)? {
        return Err(VaultError::PathCollision(destination));
    }
    let source_parent = source
        .parent()
        .ok_or_else(|| VaultError::UnsafeEntry(source.to_path_buf()))?;
    durability::sync_directory(source_parent)?;
    durability::sync_directory(vault.layout().quarantine_dir())?;
    Ok(Some(destination))
}

fn quarantine_path_identity<N: ObjectNamespace>(vault: &N, source: &Path) -> VaultResult<String> {
    if !vault.layout().is_canonical_object_path(source) {
        return Err(VaultError::UnsafeEntry(source.to_path_buf()));
    }
    let relative = source
        .strip_prefix(vault.layout().objects_root())
        .map_err(|_| VaultError::UnsafeEntry(source.to_path_buf()))?;
    let portable = portable_relative_object_path(relative, source)?;
    Ok(encode_hex(&Sha256::digest(portable.as_bytes())))
}

fn portable_relative_object_path(relative: &Path, source: &Path) -> VaultResult<String> {
    let mut portable = String::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(VaultError::UnsafeEntry(source.to_path_buf()));
        };
        let value = value
            .to_str()
            .filter(|value| value.is_ascii())
            .ok_or_else(|| VaultError::UnsafeEntry(source.to_path_buf()))?;
        if !portable.is_empty() {
            portable.push('/');
        }
        portable.push_str(value);
    }
    Ok(portable)
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

#[cfg(test)]
mod tests {
    use super::*;

    const DOMAIN: &str = "01900000-0000-7000-8000-000000000201";
    const LINEAGE: &str = "01900000-0000-7000-8000-000000000301";
    const LOCATOR: &str = "0c7ab0ce2ec59dd5f7987f7c15edaeee47f3d3080b7c88d794423ce8606aa004";

    #[test]
    fn quarantine_identity_serialization_is_platform_neutral() -> VaultResult<()> {
        let relative = PathBuf::new()
            .join(DOMAIN)
            .join("USER_MANAGED")
            .join(LINEAGE)
            .join("0c")
            .join("7a")
            .join(format!("{LOCATOR}.obj"));
        let portable = portable_relative_object_path(&relative, &relative)?;

        assert_eq!(
            portable,
            format!("{DOMAIN}/USER_MANAGED/{LINEAGE}/0c/7a/{LOCATOR}.obj")
        );
        let identity = encode_hex(&Sha256::digest(portable.as_bytes()));
        assert_eq!(identity.len(), 64);
        assert!(
            identity
                .bytes()
                .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) })
        );
        Ok(())
    }

    #[test]
    fn quarantine_filename_bound_covers_maximum_system_time() {
        let filename = format!(
            "{}-{}-{}{}",
            u64::MAX,
            "a".repeat(64),
            "b".repeat(64),
            QUARANTINE_SUFFIX
        );
        assert_eq!(filename.len(), MAX_QUARANTINE_FILENAME_BYTES);
        assert!(filename.is_ascii());
        assert!(!filename.ends_with([' ', '.']));
    }
}
