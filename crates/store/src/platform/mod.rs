//! Store policy adapter over the reviewed native path-capability facade.

use std::path::{Path, PathBuf};

use academic_store_platform::{
    DirectoryAccess, FinalPathStatus, PathCapabilities, PathCapabilityError,
    PathCapabilityErrorCode, RootState as NativeRootState,
    StorageLocality as NativeStorageLocality,
};

use crate::path_policy::{
    PathEvidence, PathProbeFailure, PathProbeFailureCode, ProfileAccess, ProfileRootState,
    StorageLocality,
};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as host;
#[cfg(windows)]
use windows as host;

#[cfg(unix)]
pub(crate) use unix::{sync_directory, sync_parent_directory};
#[cfg(windows)]
pub(crate) use windows::{sync_directory, sync_parent_directory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupplementalPolicy {
    is_sync_folder: bool,
    has_git_ancestor: bool,
}

pub(crate) fn inspect_profile_path(
    requested_root: &Path,
    configured_sync_roots: &[PathBuf],
) -> Result<PathEvidence, PathProbeFailure> {
    let capabilities =
        academic_store_platform::inspect_path(requested_root).map_err(map_native_error)?;
    let supplemental = host::supplemental_policy(
        requested_root,
        &capabilities.canonical_existing_ancestor,
        configured_sync_roots,
    )?;
    map_capabilities(capabilities, supplemental)
}

pub(crate) fn create_profile_directory(path: &Path) -> Result<(), PathProbeFailure> {
    let capabilities =
        academic_store_platform::create_owner_only_directory(path).map_err(map_native_error)?;
    let root_state = map_root_state(capabilities.root_state)?;
    if root_state != ProfileRootState::EmptyDirectory {
        return Err(inconsistent_capabilities(
            "protected creation did not return an empty directory",
        ));
    }
    if !matches!(capabilities.storage_locality, NativeStorageLocality::Local) {
        return Err(inconsistent_capabilities(
            "protected creation did not return proven-local storage",
        ));
    }
    if capabilities.access != DirectoryAccess::OwnerOnly {
        return Err(inconsistent_capabilities(
            "protected creation did not return owner-only access",
        ));
    }
    match capabilities.final_path {
        FinalPathStatus::Verified(final_path)
            if final_path.is_absolute()
                && host::paths_equal(&final_path, &capabilities.canonical_existing_ancestor) =>
        {
            Ok(())
        }
        FinalPathStatus::Verified(_) | FinalPathStatus::Changed | FinalPathStatus::Unknown => Err(
            inconsistent_capabilities("protected creation final identity did not agree"),
        ),
        FinalPathStatus::Missing => Err(inconsistent_capabilities(
            "protected creation still reported a missing root",
        )),
        _ => Err(inconsistent_capabilities(
            "protected creation returned an unknown final-path status",
        )),
    }
}

fn map_capabilities(
    capabilities: PathCapabilities,
    supplemental: SupplementalPolicy,
) -> Result<PathEvidence, PathProbeFailure> {
    if !capabilities.canonical_existing_ancestor.is_absolute() {
        return Err(inconsistent_capabilities(
            "native canonical ancestor was not absolute",
        ));
    }
    let root_state = map_root_state(capabilities.root_state)?;
    let storage_locality = match capabilities.storage_locality {
        NativeStorageLocality::Local => StorageLocality::Local,
        NativeStorageLocality::Remote => StorageLocality::Remote,
        NativeStorageLocality::Unknown(_) => StorageLocality::Unknown,
        _ => StorageLocality::Unknown,
    };
    let access = match (capabilities.access, root_state) {
        (DirectoryAccess::OwnerOnly, ProfileRootState::EmptyDirectory)
        | (DirectoryAccess::OwnerOnly, ProfileRootState::NonEmptyDirectory) => {
            ProfileAccess::OwnerOnly
        }
        (DirectoryAccess::RequiresProtectedCreation, ProfileRootState::Missing) => {
            ProfileAccess::OwnerOnlyOnCreate
        }
        (
            DirectoryAccess::Broad,
            ProfileRootState::EmptyDirectory | ProfileRootState::NonEmptyDirectory,
        ) => ProfileAccess::Broad,
        (
            DirectoryAccess::Unknown,
            ProfileRootState::EmptyDirectory
            | ProfileRootState::NonEmptyDirectory
            | ProfileRootState::NotDirectory,
        ) => ProfileAccess::Unknown,
        (
            DirectoryAccess::OwnerOnly
            | DirectoryAccess::RequiresProtectedCreation
            | DirectoryAccess::Broad
            | DirectoryAccess::Unknown,
            _,
        ) => {
            return Err(inconsistent_capabilities(
                "native access and root-state facts disagreed",
            ));
        }
        _ => ProfileAccess::Unknown,
    };
    let final_identity_matches = match (&capabilities.final_path, root_state) {
        (FinalPathStatus::Missing, ProfileRootState::Missing) => true,
        (FinalPathStatus::Verified(final_path), state)
            if state != ProfileRootState::Missing && final_path.is_absolute() =>
        {
            host::paths_equal(final_path, &capabilities.canonical_existing_ancestor)
        }
        (FinalPathStatus::Changed | FinalPathStatus::Unknown, state)
            if state != ProfileRootState::Missing =>
        {
            false
        }
        (FinalPathStatus::Verified(_), _)
        | (FinalPathStatus::Missing, _)
        | (FinalPathStatus::Changed, _)
        | (FinalPathStatus::Unknown, _) => {
            return Err(inconsistent_capabilities(
                "native final-path and root-state facts disagreed",
            ));
        }
        _ => {
            return Err(inconsistent_capabilities(
                "native facade returned an unknown final-path status",
            ));
        }
    };
    Ok(PathEvidence {
        canonical_existing_ancestor: capabilities.canonical_existing_ancestor,
        root_state,
        storage_locality,
        access,
        has_symlink_or_reparse_component: false,
        is_sync_folder: supplemental.is_sync_folder,
        has_git_ancestor: supplemental.has_git_ancestor,
        final_identity_matches,
    })
}

fn map_root_state(state: NativeRootState) -> Result<ProfileRootState, PathProbeFailure> {
    match state {
        NativeRootState::Missing => Ok(ProfileRootState::Missing),
        NativeRootState::EmptyDirectory => Ok(ProfileRootState::EmptyDirectory),
        NativeRootState::NonEmptyDirectory => Ok(ProfileRootState::NonEmptyDirectory),
        NativeRootState::NotDirectory => Ok(ProfileRootState::NotDirectory),
        _ => Err(inconsistent_capabilities(
            "native facade returned an unknown root state",
        )),
    }
}

fn map_native_error(error: PathCapabilityError) -> PathProbeFailure {
    let code = match error.code {
        PathCapabilityErrorCode::RemoteStorage | PathCapabilityErrorCode::UnknownStorage => {
            PathProbeFailureCode::StorageInspection
        }
        PathCapabilityErrorCode::BroadAccess => PathProbeFailureCode::AccessInspection,
        PathCapabilityErrorCode::UnsupportedPlatform => PathProbeFailureCode::UnsupportedPlatform,
        PathCapabilityErrorCode::OperatingSystem => PathProbeFailureCode::OperatingSystem,
        PathCapabilityErrorCode::EmptyPath
        | PathCapabilityErrorCode::NonUnicodePath
        | PathCapabilityErrorCode::RelativePath
        | PathCapabilityErrorCode::TraversalComponent
        | PathCapabilityErrorCode::NetworkShare
        | PathCapabilityErrorCode::DevicePath
        | PathCapabilityErrorCode::DriveRelativePath
        | PathCapabilityErrorCode::LinkOrReparsePoint
        | PathCapabilityErrorCode::NotDirectoryAncestor
        | PathCapabilityErrorCode::ParentMissing
        | PathCapabilityErrorCode::AlreadyExists
        | PathCapabilityErrorCode::IdentityChanged => PathProbeFailureCode::Canonicalization,
        _ => PathProbeFailureCode::UnsupportedPlatform,
    };
    PathProbeFailure::new(code, error.to_string())
}

fn inconsistent_capabilities(detail: &'static str) -> PathProbeFailure {
    PathProbeFailure::new(PathProbeFailureCode::Canonicalization, detail)
}

#[cfg(not(any(unix, windows)))]
mod host {
    use std::path::{Path, PathBuf};

    use super::{PathProbeFailure, PathProbeFailureCode, SupplementalPolicy};

    pub(super) fn supplemental_policy(
        _requested_root: &Path,
        _canonical_existing_ancestor: &Path,
        _configured_sync_roots: &[PathBuf],
    ) -> Result<SupplementalPolicy, PathProbeFailure> {
        Ok(SupplementalPolicy {
            is_sync_folder: false,
            has_git_ancestor: false,
        })
    }

    pub(super) fn paths_equal(left: &Path, right: &Path) -> bool {
        left == right
    }

    pub(crate) fn sync_directory(_path: &Path) -> Result<(), PathProbeFailure> {
        Err(PathProbeFailure::new(
            PathProbeFailureCode::UnsupportedPlatform,
            "directory synchronization is unsupported on this platform",
        ))
    }

    pub(crate) fn sync_parent_directory(_path: &Path) -> Result<(), PathProbeFailure> {
        Err(PathProbeFailure::new(
            PathProbeFailureCode::UnsupportedPlatform,
            "parent directory synchronization is unsupported on this platform",
        ))
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) use host::{sync_directory, sync_parent_directory};

#[cfg(test)]
mod tests {
    use academic_store_platform::LocalityUnknown;

    use super::*;

    #[test]
    fn native_capabilities_map_without_promoting_unknown() {
        let ancestor = absolute_test_path("ancestor");
        let evidence = map_capabilities(
            PathCapabilities {
                canonical_existing_ancestor: ancestor.clone(),
                root_state: NativeRootState::Missing,
                storage_locality: NativeStorageLocality::Unknown(
                    LocalityUnknown::UnsupportedPlatform,
                ),
                access: DirectoryAccess::RequiresProtectedCreation,
                final_path: FinalPathStatus::Missing,
            },
            no_supplemental_policy(),
        );
        assert!(matches!(
            evidence,
            Ok(PathEvidence {
                root_state: ProfileRootState::Missing,
                storage_locality: StorageLocality::Unknown,
                access: ProfileAccess::OwnerOnlyOnCreate,
                final_identity_matches: true,
                ..
            })
        ));
    }

    #[test]
    fn inconsistent_native_capabilities_fail_closed() {
        let ancestor = absolute_test_path("ancestor");
        let result = map_capabilities(
            PathCapabilities {
                canonical_existing_ancestor: ancestor,
                root_state: NativeRootState::Missing,
                storage_locality: NativeStorageLocality::Local,
                access: DirectoryAccess::OwnerOnly,
                final_path: FinalPathStatus::Missing,
            },
            no_supplemental_policy(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn changed_native_final_identity_stays_failed() {
        let ancestor = absolute_test_path("profile");
        let evidence = map_capabilities(
            PathCapabilities {
                canonical_existing_ancestor: ancestor,
                root_state: NativeRootState::EmptyDirectory,
                storage_locality: NativeStorageLocality::Local,
                access: DirectoryAccess::OwnerOnly,
                final_path: FinalPathStatus::Changed,
            },
            no_supplemental_policy(),
        );
        assert!(matches!(
            evidence,
            Ok(PathEvidence {
                final_identity_matches: false,
                ..
            })
        ));
    }

    fn no_supplemental_policy() -> SupplementalPolicy {
        SupplementalPolicy {
            is_sync_folder: false,
            has_git_ancestor: false,
        }
    }

    fn absolute_test_path(component: &str) -> PathBuf {
        #[cfg(windows)]
        {
            PathBuf::from(format!(r"C:\synthetic\{component}"))
        }
        #[cfg(not(windows))]
        {
            PathBuf::from(format!("/synthetic/{component}"))
        }
    }
}
