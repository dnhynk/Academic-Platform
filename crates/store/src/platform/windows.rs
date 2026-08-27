//! Windows store-level sync-root, Git-root, and directory-barrier policy.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use academic_store_platform::{
    DirectoryAccess, FinalPathStatus, PathCapabilities, RootState, StorageLocality,
};

use crate::path_policy::{PathProbeFailure, PathProbeFailureCode};

use super::{SupplementalPolicy, inconsistent_capabilities, map_native_error};

pub(super) fn supplemental_policy(
    requested_root: &Path,
    canonical_existing_ancestor: &Path,
    configured_sync_roots: &[PathBuf],
) -> Result<SupplementalPolicy, PathProbeFailure> {
    let sync_roots = native_sync_roots(configured_sync_roots);
    if sync_roots
        .iter()
        .any(|root| !root.is_absolute() || root.to_str().is_none())
    {
        return Err(PathProbeFailure::new(
            PathProbeFailureCode::Canonicalization,
            "configured synchronization root is not an absolute Unicode path",
        ));
    }
    let is_sync_folder = sync_roots.iter().any(|root| {
        path_starts_with_case_insensitive(requested_root, root)
            || path_starts_with_case_insensitive(canonical_existing_ancestor, root)
    });
    let has_git_ancestor = has_git_ancestor(canonical_existing_ancestor)?;
    Ok(SupplementalPolicy {
        is_sync_folder,
        has_git_ancestor,
    })
}

pub(super) fn paths_equal(left: &Path, right: &Path) -> bool {
    normalize_windows_path(left).eq_ignore_ascii_case(&normalize_windows_path(right))
}

/// Revalidates the directory identity at each durability barrier.
///
/// Rust's safe Windows filesystem API cannot flush a directory handle. Every
/// profile file is flushed directly; this barrier reuses the reviewed native
/// facade so an identity swap can never be mistaken for successful durability.
pub(crate) fn sync_directory(path: &Path) -> Result<(), PathProbeFailure> {
    revalidate_directory_barrier(path, true)
}

/// Revalidates the parent after deleting a secure child profile.
///
/// The parent may legitimately have broad access (for example, the OS temporary
/// directory), so this barrier requires only its directory, locality, and final
/// identity facts.
pub(crate) fn sync_parent_directory(path: &Path) -> Result<(), PathProbeFailure> {
    revalidate_directory_barrier(path, false)
}

fn revalidate_directory_barrier(
    path: &Path,
    require_owner_only: bool,
) -> Result<(), PathProbeFailure> {
    let capabilities = academic_store_platform::inspect_path(path).map_err(map_native_error)?;
    verify_directory_barrier_capabilities(&capabilities, require_owner_only)
}

fn verify_directory_barrier_capabilities(
    capabilities: &PathCapabilities,
    require_owner_only: bool,
) -> Result<(), PathProbeFailure> {
    if !matches!(
        capabilities.root_state,
        RootState::EmptyDirectory | RootState::NonEmptyDirectory
    ) || !matches!(capabilities.storage_locality, StorageLocality::Local)
    {
        return Err(inconsistent_capabilities(
            "directory barrier did not return an existing local directory",
        ));
    }
    if require_owner_only && capabilities.access != DirectoryAccess::OwnerOnly {
        return Err(inconsistent_capabilities(
            "profile-root directory barrier did not return owner-only access",
        ));
    }
    match &capabilities.final_path {
        FinalPathStatus::Verified(final_path)
            if paths_equal(final_path, &capabilities.canonical_existing_ancestor) =>
        {
            Ok(())
        }
        _ => Err(inconsistent_capabilities(
            "directory barrier final identity did not agree",
        )),
    }
}

fn native_sync_roots(configured: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = configured.to_vec();
    for variable in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(value) = env::var_os(variable) {
            roots.push(PathBuf::from(value));
        }
    }
    roots
}

fn has_git_ancestor(path: &Path) -> Result<bool, PathProbeFailure> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(PathProbeFailure::new(
                    PathProbeFailureCode::OperatingSystem,
                    format!("inspect Git ancestor marker: {error}"),
                ));
            }
        }
    }
    Ok(false)
}

fn path_starts_with_case_insensitive(path: &Path, root: &Path) -> bool {
    let path = normalize_windows_path(path).to_ascii_lowercase();
    let mut root = normalize_windows_path(root).to_ascii_lowercase();
    while root.ends_with('\\') {
        root.pop();
    }
    path == root
        || path
            .strip_prefix(&root)
            .is_some_and(|tail| tail.starts_with('\\'))
}

fn normalize_windows_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('/', "\\");
    let lowercase = value.to_ascii_lowercase();
    if lowercase.starts_with("\\\\?\\unc\\") {
        format!("\\\\{}", &value[8..])
    } else if lowercase.starts_with("\\\\?\\") {
        value[4..].to_owned()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_root_barrier_rejects_broad_access_while_parent_barrier_allows_it() {
        let path = PathBuf::from(r"C:\synthetic\profile-parent");
        let capabilities = PathCapabilities {
            canonical_existing_ancestor: path.clone(),
            root_state: RootState::EmptyDirectory,
            storage_locality: StorageLocality::Local,
            access: DirectoryAccess::Broad,
            final_path: FinalPathStatus::Verified(path),
        };

        assert!(verify_directory_barrier_capabilities(&capabilities, true).is_err());
        assert!(verify_directory_barrier_capabilities(&capabilities, false).is_ok());
    }
}
