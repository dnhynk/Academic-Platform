// The plaintext synthetic lane only. The encrypted lane cannot link this lane's
// profile API at all (t068 section 2.3-13), so under `sqlcipher-store` this file
// compiles to nothing and `tests/encrypted_profile.rs` carries the equivalent
// coverage against the schema-2 profile.
#![cfg(not(feature = "sqlcipher-store"))]

use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_store::{
    INCOMPLETE_PROFILE_MARKER, PHASE1_POLICY_BANNER, SYNTHETIC_PROFILE_MARKER,
    path_policy::{
        PathEvidence, PathPolicyViolation, PathProbe, PathProbeFailure, ProfileAccess,
        ProfileRootState, StorageLocality, validate_new_profile_path,
    },
    profile::{
        SYNTHETIC_PROFILE_MARKER_CONTENTS, SyntheticIngestManifest, create_synthetic_profile,
        open_synthetic_profile, prepare_synthetic_profile, remove_incomplete_profile,
        validate_synthetic_manifest, write_policy_banner,
    },
};

// Only the native Linux and Windows tests below reach the real probe.
#[cfg(any(target_os = "linux", windows))]
use academic_store::path_policy::{NativePathProbe, validate_existing_profile_path};

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> std::io::Result<Self> {
        let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = temporary_base()?.join(format!(
            "academic-store-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!("test cleanup failed for {}: {error}", self.path.display());
        }
    }
}

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native facade
/// refuses to follow a link component, so the tests address the real directory.
#[cfg(unix)]
fn temporary_base() -> std::io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> std::io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

#[derive(Debug, Clone)]
struct FixedProbe {
    evidence: PathEvidence,
}

impl PathProbe for FixedProbe {
    fn inspect(&self, _requested_root: &Path) -> Result<PathEvidence, PathProbeFailure> {
        Ok(self.evidence.clone())
    }
}

#[derive(Debug, Clone, Copy)]
struct FailingProbe;

impl PathProbe for FailingProbe {
    fn inspect(&self, _requested_root: &Path) -> Result<PathEvidence, PathProbeFailure> {
        Err(PathProbeFailure::new(
            academic_store::path_policy::PathProbeFailureCode::StorageInspection,
            "injected failure",
        ))
    }
}

#[derive(Debug, Clone, Copy)]
struct LocalFilesystemProbe;

impl PathProbe for LocalFilesystemProbe {
    fn inspect(&self, requested_root: &Path) -> Result<PathEvidence, PathProbeFailure> {
        let (root_state, canonical_existing_ancestor, access) =
            match fs::symlink_metadata(requested_root) {
                Ok(metadata) if !metadata.is_dir() => (
                    ProfileRootState::NotDirectory,
                    requested_root.to_path_buf(),
                    ProfileAccess::Unknown,
                ),
                Ok(_) => {
                    let mut entries = fs::read_dir(requested_root).map_err(probe_io)?;
                    let state = if entries.next().is_some() {
                        ProfileRootState::NonEmptyDirectory
                    } else {
                        ProfileRootState::EmptyDirectory
                    };
                    (
                        state,
                        fs::canonicalize(requested_root).map_err(probe_io)?,
                        ProfileAccess::OwnerOnly,
                    )
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let parent = requested_root.parent().ok_or_else(|| {
                        PathProbeFailure::new(
                            academic_store::path_policy::PathProbeFailureCode::Canonicalization,
                            "test path has no parent",
                        )
                    })?;
                    (
                        ProfileRootState::Missing,
                        fs::canonicalize(parent).map_err(probe_io)?,
                        ProfileAccess::OwnerOnlyOnCreate,
                    )
                }
                Err(error) => return Err(probe_io(error)),
            };
        Ok(PathEvidence {
            canonical_existing_ancestor,
            root_state,
            storage_locality: StorageLocality::Local,
            access,
            has_symlink_or_reparse_component: false,
            is_sync_folder: false,
            has_git_ancestor: false,
            final_identity_matches: root_state != ProfileRootState::NotDirectory,
        })
    }
}

#[test]
fn plaintext_profile_banner_is_unavoidable() -> Result<(), Box<dyn Error>> {
    let mut output = Vec::new();
    write_policy_banner(&mut output)?;
    assert_eq!(output, format!("{PHASE1_POLICY_BANNER}\n").as_bytes());
    assert!(
        SYNTHETIC_PROFILE_MARKER_CONTENTS
            .as_bytes()
            .starts_with(&output)
    );

    let temporary = TemporaryDirectory::new("banner")?;
    let root = temporary.child("profile");
    let incomplete = prepare_synthetic_profile(&root, &LocalFilesystemProbe)?;
    assert_eq!(incomplete.root(), root);
    assert_eq!(
        fs::read(root.join(SYNTHETIC_PROFILE_MARKER))?,
        SYNTHETIC_PROFILE_MARKER_CONTENTS.as_bytes()
    );
    assert!(root.join(INCOMPLETE_PROFILE_MARKER).is_file());
    remove_incomplete_profile(&root, &LocalFilesystemProbe)?;
    assert!(!root.exists());
    Ok(())
}

#[test]
fn real_data_manifest_is_rejected() {
    let allowlisted = SyntheticIngestManifest::allowlisted();
    assert!(validate_synthetic_manifest(&allowlisted).is_ok());

    let real_data = SyntheticIngestManifest {
        data_class: "PERSONAL",
        ..allowlisted
    };
    assert!(matches!(
        validate_synthetic_manifest(&real_data),
        Err(academic_store::error::StoreError::ManifestRejected {
            field: "data_class"
        })
    ));

    let networked = SyntheticIngestManifest {
        network_egress: "HTTPS",
        ..allowlisted
    };
    assert!(validate_synthetic_manifest(&networked).is_err());
    let production = SyntheticIngestManifest {
        production_data_allowed: true,
        ..allowlisted
    };
    assert!(validate_synthetic_manifest(&production).is_err());
    let encrypted = SyntheticIngestManifest {
        storage_encryption: "SQLCIPHER",
        ..allowlisted
    };
    assert!(validate_synthetic_manifest(&encrypted).is_err());
    let product_network = SyntheticIngestManifest {
        product_network: "TCP",
        ..allowlisted
    };
    assert!(validate_synthetic_manifest(&product_network).is_err());
    let unsupported_schema = SyntheticIngestManifest {
        fixture_schema_version: 3,
        ..allowlisted
    };
    assert!(validate_synthetic_manifest(&unsupported_schema).is_err());
    let substituted_fixture = SyntheticIngestManifest {
        fixture_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        ..allowlisted
    };
    assert!(validate_synthetic_manifest(&substituted_fixture).is_err());
}

#[test]
fn unsafe_profile_path_matrix() -> Result<(), Box<dyn Error>> {
    let temporary = TemporaryDirectory::new("matrix")?;
    let candidate = temporary.child("profile");
    let safe = PathEvidence::safe_missing(&temporary.path);
    let cases = [
        (
            PathEvidence {
                storage_locality: StorageLocality::Unknown,
                ..safe.clone()
            },
            PathPolicyViolation::UnknownStorage,
        ),
        (
            PathEvidence {
                access: ProfileAccess::Broad,
                ..safe.clone()
            },
            PathPolicyViolation::BroadAccess,
        ),
        (
            PathEvidence {
                access: ProfileAccess::Unknown,
                ..safe.clone()
            },
            PathPolicyViolation::UnknownAccess,
        ),
        (
            PathEvidence {
                has_symlink_or_reparse_component: true,
                ..safe.clone()
            },
            PathPolicyViolation::SymlinkOrReparsePoint,
        ),
        (
            PathEvidence {
                root_state: ProfileRootState::EmptyDirectory,
                access: ProfileAccess::OwnerOnly,
                final_identity_matches: false,
                ..safe.clone()
            },
            PathPolicyViolation::FinalIdentityChanged,
        ),
        (
            PathEvidence {
                root_state: ProfileRootState::NonEmptyDirectory,
                access: ProfileAccess::OwnerOnly,
                ..safe.clone()
            },
            PathPolicyViolation::ProfileNotEmpty,
        ),
        (
            PathEvidence {
                root_state: ProfileRootState::NotDirectory,
                access: ProfileAccess::Unknown,
                final_identity_matches: false,
                ..PathEvidence::safe_missing(&temporary.path)
            },
            PathPolicyViolation::ProfileNotDirectory,
        ),
    ];
    for (evidence, expected) in cases {
        let result = validate_new_profile_path(&candidate, &FixedProbe { evidence });
        assert_eq!(result, Err(expected));
    }

    assert_eq!(
        validate_new_profile_path(
            Path::new("relative/profile"),
            &FixedProbe {
                evidence: PathEvidence::safe_missing(&temporary.path),
            }
        ),
        Err(PathPolicyViolation::RelativePath)
    );
    assert_eq!(
        validate_new_profile_path(
            Path::new("file:///tmp/profile"),
            &FixedProbe {
                evidence: PathEvidence::safe_missing(&temporary.path),
            }
        ),
        Err(PathPolicyViolation::UriLikePath)
    );
    assert_eq!(
        validate_new_profile_path(
            &temporary.path.join(".").join("profile"),
            &FixedProbe {
                evidence: PathEvidence::safe_missing(&temporary.path),
            }
        ),
        Err(PathPolicyViolation::TraversalComponent)
    );
    assert_eq!(
        validate_new_profile_path(Path::new(""), &FailingProbe),
        Err(PathPolicyViolation::EmptyPath)
    );
    assert_eq!(
        validate_new_profile_path(Path::new(r"\\?\C:\profile"), &FailingProbe),
        Err(PathPolicyViolation::DevicePath)
    );
    assert!(matches!(
        validate_new_profile_path(&candidate, &FailingProbe),
        Err(PathPolicyViolation::ProbeFailed(_))
    ));
    let empty = FixedProbe {
        evidence: PathEvidence::safe_empty(&candidate),
    };
    assert!(validate_new_profile_path(&candidate, &empty).is_ok());
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn foreign_windows_drive_path_is_rejected() {
    assert_eq!(
        validate_new_profile_path(Path::new(r"C:\profile"), &FailingProbe),
        Err(PathPolicyViolation::ForeignDrivePath)
    );
}

#[cfg(unix)]
#[test]
fn non_unicode_unix_path_is_rejected() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let path = PathBuf::from(OsString::from_vec(vec![b'/', 0xff]));
    assert_eq!(
        validate_new_profile_path(&path, &FailingProbe),
        Err(PathPolicyViolation::NonUnicodePath)
    );
}

#[cfg(windows)]
#[test]
fn non_unicode_windows_path_is_rejected() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    let path = PathBuf::from(OsString::from_wide(&[0xd800]));
    assert_eq!(
        validate_new_profile_path(&path, &FailingProbe),
        Err(PathPolicyViolation::NonUnicodePath)
    );
}

#[cfg(windows)]
#[test]
fn windows_alternate_stream_path_is_rejected() {
    for path in [
        r"C:\profile:stream",
        r"C:\synthetic\NUL.txt",
        r"\??\C:\synthetic\profile",
        r"\Device\HarddiskVolume1\synthetic",
    ] {
        assert_eq!(
            validate_new_profile_path(Path::new(path), &FailingProbe),
            Err(PathPolicyViolation::DevicePath),
            "unsafe Windows namespace was accepted: {path}"
        );
    }
    assert_eq!(
        validate_new_profile_path(Path::new(r"C:profile"), &FailingProbe),
        Err(PathPolicyViolation::RelativePath)
    );
}

#[test]
fn network_share_is_rejected() -> Result<(), Box<dyn Error>> {
    let temporary = TemporaryDirectory::new("network")?;
    let candidate = temporary.child("profile");
    let remote = FixedProbe {
        evidence: PathEvidence {
            storage_locality: StorageLocality::Remote,
            ..PathEvidence::safe_missing(&temporary.path)
        },
    };
    assert_eq!(
        validate_new_profile_path(&candidate, &remote),
        Err(PathPolicyViolation::RemoteStorage)
    );
    assert_eq!(
        validate_new_profile_path(Path::new(r"\\server\share\profile"), &remote),
        Err(PathPolicyViolation::NetworkShare)
    );
    Ok(())
}

#[test]
fn sync_folder_is_rejected() -> Result<(), Box<dyn Error>> {
    let temporary = TemporaryDirectory::new("sync")?;
    let candidate = temporary.child("profile");
    let probe = FixedProbe {
        evidence: PathEvidence {
            is_sync_folder: true,
            ..PathEvidence::safe_missing(&temporary.path)
        },
    };
    assert_eq!(
        validate_new_profile_path(&candidate, &probe),
        Err(PathPolicyViolation::SyncFolder)
    );
    Ok(())
}

#[test]
fn git_worktree_profile_is_rejected() -> Result<(), Box<dyn Error>> {
    let temporary = TemporaryDirectory::new("git")?;
    let candidate = temporary.child("profile");
    let probe = FixedProbe {
        evidence: PathEvidence {
            has_git_ancestor: true,
            ..PathEvidence::safe_missing(&temporary.path)
        },
    };
    assert_eq!(
        validate_new_profile_path(&candidate, &probe),
        Err(PathPolicyViolation::GitWorktree)
    );
    Ok(())
}

#[test]
fn interrupted_bootstrap_is_refused_and_cleanup_fails_closed() -> Result<(), Box<dyn Error>> {
    let temporary = TemporaryDirectory::new("interrupted")?;
    let root = temporary.child("profile");
    let incomplete = prepare_synthetic_profile(&root, &LocalFilesystemProbe)?;
    assert!(matches!(
        open_synthetic_profile(&root, &LocalFilesystemProbe),
        Err(academic_store::error::StoreError::IncompleteProfile(path)) if path == root
    ));

    let unknown = root.join("user-file.txt");
    fs::write(&unknown, b"must not delete")?;
    assert!(remove_incomplete_profile(&root, &LocalFilesystemProbe).is_err());
    assert_eq!(fs::read(&unknown)?, b"must not delete");
    fs::remove_file(unknown)?;
    fs::write(root.join(INCOMPLETE_PROFILE_MARKER), b"")?;
    drop(incomplete);
    remove_incomplete_profile(&root, &LocalFilesystemProbe)?;
    assert!(!root.exists());
    Ok(())
}

#[test]
fn completed_profile_reopens_only_after_marker_removal() -> Result<(), Box<dyn Error>> {
    let temporary = TemporaryDirectory::new("complete")?;
    let root = temporary.child("profile");
    let created = create_synthetic_profile(&root, &LocalFilesystemProbe, [0x73; 32])?;
    assert!(!root.join(INCOMPLETE_PROFILE_MARKER).exists());
    assert_eq!(created.root(), root);
    let reopened = open_synthetic_profile(&root, &LocalFilesystemProbe)?;
    assert_eq!(reopened.database_path(), created.database_path());
    assert!(reopened.open_reader()?.pragma_snapshot()?.query_only);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn native_linux_mode_mount_symlink_sync_and_git_checks() -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temporary = TemporaryDirectory::new("native-linux")?;
    let root = temporary.child("profile");
    let native = NativePathProbe::default();
    let incomplete = prepare_synthetic_profile(&root, &native)?;
    assert_eq!(fs::metadata(&root)?.permissions().mode() & 0o777, 0o700);
    assert!(validate_existing_profile_path(&root, &native).is_ok());
    drop(incomplete);
    remove_incomplete_profile(&root, &native)?;

    let broad = temporary.child("broad");
    fs::create_dir(&broad)?;
    fs::set_permissions(&broad, fs::Permissions::from_mode(0o755))?;
    assert_eq!(
        validate_existing_profile_path(&broad, &native),
        Err(PathPolicyViolation::BroadAccess)
    );

    let target = temporary.child("target");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
    let link = temporary.child("link");
    symlink(&target, &link)?;
    assert!(matches!(
        validate_new_profile_path(&link, &native),
        Err(PathPolicyViolation::ProbeFailed(failure))
            if failure.code
                == academic_store::path_policy::PathProbeFailureCode::Canonicalization
    ));

    let sync_candidate = temporary.child("sync-profile");
    let sync_probe = NativePathProbe::with_sync_roots(vec![temporary.path.clone()]);
    assert_eq!(
        validate_new_profile_path(&sync_candidate, &sync_probe),
        Err(PathPolicyViolation::SyncFolder)
    );

    fs::create_dir(temporary.child(".git"))?;
    assert_eq!(
        validate_new_profile_path(&temporary.child("git-profile"), &native),
        Err(PathPolicyViolation::GitWorktree)
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn native_windows_acl_reparse_sync_and_git_checks() -> Result<(), Box<dyn Error>> {
    use std::os::windows::fs::symlink_dir;

    use academic_store_platform::DirectoryAccess;

    let temporary = TemporaryDirectory::new("native-windows")?;
    assert_eq!(
        academic_store_platform::inspect_path(&temporary.path)?.access,
        DirectoryAccess::Broad
    );
    let root = temporary.child("profile");
    let native = NativePathProbe::default();
    let incomplete = prepare_synthetic_profile(&root, &native)?;
    assert_eq!(
        academic_store_platform::inspect_path(&root)?.access,
        DirectoryAccess::OwnerOnly
    );
    assert!(validate_existing_profile_path(&root, &native).is_ok());
    drop(incomplete);
    remove_incomplete_profile(&root, &native)?;
    assert!(!root.exists());

    let sync_candidate = temporary.child("sync-profile");
    let sync_probe = NativePathProbe::with_sync_roots(vec![temporary.path.clone()]);
    assert_eq!(
        validate_new_profile_path(&sync_candidate, &sync_probe),
        Err(PathPolicyViolation::SyncFolder)
    );

    fs::create_dir(temporary.child(".git"))?;
    assert_eq!(
        validate_new_profile_path(&temporary.child("git-profile"), &native),
        Err(PathPolicyViolation::GitWorktree)
    );
    fs::remove_dir(temporary.child(".git"))?;

    let broad = temporary.child("broad");
    fs::create_dir(&broad)?;
    assert_eq!(
        validate_existing_profile_path(&broad, &native),
        Err(PathPolicyViolation::BroadAccess)
    );
    fs::remove_dir(broad)?;

    let target = temporary.child("target");
    fs::create_dir(&target)?;
    let link = temporary.child("link");
    match symlink_dir(&target, &link) {
        Ok(()) => assert!(matches!(
            validate_new_profile_path(&link, &native),
            Err(PathPolicyViolation::ProbeFailed(failure))
                if failure.code
                    == academic_store::path_policy::PathProbeFailureCode::Canonicalization
        )),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn probe_io(error: std::io::Error) -> PathProbeFailure {
    PathProbeFailure::new(
        academic_store::path_policy::PathProbeFailureCode::OperatingSystem,
        error.to_string(),
    )
}
