#![cfg(any(unix, windows))]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use academic_store_platform::{
    DirectoryAccess, FinalPathStatus, PathCapabilityErrorCode, RootState, StorageLocality,
    create_owner_only_directory, inspect_path,
};

static TEST_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create(label: &str) -> Result<Self, Box<dyn Error>> {
        let counter = TEST_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = temporary_base()?.join(format!(
            "academic-store-platform-{label}-{}-{counter}",
            process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_local_temp_directory_has_verified_final_path_and_owner_boundary()
-> Result<(), Box<dyn Error>> {
    let parent = TestDirectory::create("local")?;
    let profile = parent.path().join("profile");

    let missing = inspect_path(&profile)?;
    assert_eq!(missing.root_state, RootState::Missing);
    assert_eq!(missing.storage_locality, StorageLocality::Local);
    assert_eq!(missing.access, DirectoryAccess::RequiresProtectedCreation);
    assert_eq!(missing.final_path, FinalPathStatus::Missing);

    let created = create_owner_only_directory(&profile)?;
    assert_eq!(created.root_state, RootState::EmptyDirectory);
    assert_eq!(created.storage_locality, StorageLocality::Local);
    assert_eq!(created.access, DirectoryAccess::OwnerOnly);
    let FinalPathStatus::Verified(final_path) = created.final_path else {
        return Err("native final path was not verified".into());
    };
    assert!(final_path.is_absolute());
    assert!(paths_equal(&final_path, &fs::canonicalize(&profile)?));

    let marker = profile.join("marker");
    fs::write(&marker, b"synthetic")?;
    assert_eq!(
        inspect_path(&profile)?.root_state,
        RootState::NonEmptyDirectory
    );
    fs::remove_file(marker)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn native_unix_symlink_and_broad_mode_are_rejected() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let parent = TestDirectory::create("unix-policy")?;
    let target = parent.path().join("target");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
    let link = parent.path().join("link");
    symlink(&target, &link)?;
    let link_error = inspect_path(&link)
        .err()
        .ok_or("symlink path was accepted")?;
    assert_eq!(link_error.code, PathCapabilityErrorCode::LinkOrReparsePoint);

    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    assert_eq!(inspect_path(&target)?.access, DirectoryAccess::Broad);
    Ok(())
}

#[cfg(windows)]
#[test]
fn native_windows_reparse_and_inherited_broad_dacl_are_rejected() -> Result<(), Box<dyn Error>> {
    use std::os::windows::fs::symlink_dir;

    let parent = TestDirectory::create("windows-policy")?;
    let target = parent.path().join("target");
    fs::create_dir(&target)?;
    let link = parent.path().join("link");
    symlink_dir(&target, &link)?;
    let link_error = inspect_path(&link)
        .err()
        .ok_or("reparse path was accepted")?;
    assert_eq!(link_error.code, PathCapabilityErrorCode::LinkOrReparsePoint);

    let broad = parent.path().join("broad");
    fs::create_dir(&broad)?;
    assert_eq!(inspect_path(&broad)?.access, DirectoryAccess::Broad);
    Ok(())
}

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the facade refuses to
/// follow a link component, so the tests address the real directory.
#[cfg(unix)]
fn temporary_base() -> Result<PathBuf, Box<dyn Error>> {
    Ok(fs::canonicalize(std::env::temp_dir())?)
}

#[cfg(windows)]
fn temporary_base() -> Result<PathBuf, Box<dyn Error>> {
    Ok(std::env::temp_dir())
}

#[cfg(windows)]
fn paths_equal(left: &Path, right: &Path) -> bool {
    normalize_windows_path(left).eq_ignore_ascii_case(&normalize_windows_path(right))
}

#[cfg(windows)]
fn normalize_windows_path(path: &Path) -> String {
    path.as_os_str()
        .to_string_lossy()
        .strip_prefix(r"\\?\")
        .unwrap_or(&path.as_os_str().to_string_lossy())
        .to_owned()
}

#[cfg(unix)]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}
