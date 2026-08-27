//! Unix store-level sync-root, Git-root, and directory-durability policy.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::path_policy::{PathProbeFailure, PathProbeFailureCode};

use super::SupplementalPolicy;

pub(super) fn supplemental_policy(
    requested_root: &Path,
    canonical_existing_ancestor: &Path,
    configured_sync_roots: &[PathBuf],
) -> Result<SupplementalPolicy, PathProbeFailure> {
    let sync_roots = native_sync_roots(configured_sync_roots);
    if sync_roots.iter().any(|root| !root.is_absolute()) {
        return Err(PathProbeFailure::new(
            PathProbeFailureCode::Canonicalization,
            "configured synchronization root is not absolute",
        ));
    }
    let is_sync_folder = sync_roots.iter().any(|root| {
        requested_root.starts_with(root) || canonical_existing_ancestor.starts_with(root)
    });
    let has_git_ancestor = has_git_ancestor(canonical_existing_ancestor)?;
    Ok(SupplementalPolicy {
        is_sync_folder,
        has_git_ancestor,
    })
}

pub(super) fn paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), PathProbeFailure> {
    let directory = fs::File::open(path)
        .map_err(|error| os_error("open profile directory for synchronization", error))?;
    directory
        .sync_all()
        .map_err(|error| os_error("synchronize profile directory", error))
}

pub(crate) fn sync_parent_directory(path: &Path) -> Result<(), PathProbeFailure> {
    sync_directory(path)
}

fn native_sync_roots(configured: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = configured.to_vec();
    for variable in [
        "DROPBOX_PATH",
        "NEXTCLOUD_PATH",
        "OWNCLOUD_PATH",
        "SYNCTHING_ROOT",
    ] {
        if let Some(value) = env::var_os(variable) {
            roots.push(PathBuf::from(value));
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        roots.extend([
            home.join("Dropbox"),
            home.join("Nextcloud"),
            home.join("ownCloud"),
        ]);
    }
    roots
}

fn has_git_ancestor(path: &Path) -> Result<bool, PathProbeFailure> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(os_error("inspect Git ancestor marker", error)),
        }
    }
    Ok(false)
}

fn os_error(operation: &'static str, error: io::Error) -> PathProbeFailure {
    PathProbeFailure::new(
        PathProbeFailureCode::OperatingSystem,
        format!("{operation}: {error}"),
    )
}
