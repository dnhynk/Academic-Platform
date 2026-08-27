//! Creation and opening of disposable, plaintext, synthetic-only profiles.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::{
    INCOMPLETE_PROFILE_MARKER, PHASE1_POLICY_BANNER, PHASE1_STORAGE_POLICY, STORE_DATABASE_FILE,
    SYNTHETIC_PROFILE_MARKER,
    connection::{ReaderConnection, WriterConnection, open_reader, open_writer},
    error::{StoreError, StoreResult},
    migration::{MigrationStatus, migrate_pre_listen},
    path_policy::{
        PathPolicyViolation, PathProbe, ProfileRootState, validate_created_profile_path,
        validate_existing_profile_path, validate_new_profile_path,
    },
    platform,
};

/// Exact contents of the unavoidable plaintext warning file.
pub const SYNTHETIC_PROFILE_MARKER_CONTENTS: &str = concat!(
    "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN\n",
    "data_policy=SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED\n",
    "storage_mode=PLAINTEXT_TEMPORARY_SQLITE\n",
    "storage_encryption=NONE\n",
    "production_data_allowed=false\n",
    "product_network=NONE\n",
);

const INCOMPLETE_PROFILE_MARKER_CONTENTS: &str =
    "ACADEMIC_PLATFORM_PHASE1_PROFILE_BOOTSTRAP_INCOMPLETE\n";

/// Only runtime manifest admitted by the S1 synthetic fixture boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntheticIngestManifest<'a> {
    pub manifest_version: u32,
    pub data_policy: &'a str,
    pub data_class: &'a str,
    pub network_egress: &'a str,
    pub storage_mode: &'a str,
    pub storage_encryption: &'a str,
    pub production_data_allowed: bool,
    pub product_network: &'a str,
    pub fixture_id: &'a str,
    pub fixture_schema_version: u32,
    pub fixture_relative_path: &'a str,
    pub fixture_sha256: &'a str,
    pub fixture_byte_length: u64,
    pub builder_id: &'a str,
}

impl SyntheticIngestManifest<'static> {
    /// Returns the sole manifest bound to the reviewed deterministic fixture bytes.
    #[must_use]
    pub const fn allowlisted() -> Self {
        Self {
            manifest_version: 1,
            data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
            data_class: "SYNTHETIC_ONLY",
            network_egress: "NONE",
            storage_mode: "PLAINTEXT_TEMPORARY_SQLITE",
            storage_encryption: "NONE",
            production_data_allowed: false,
            product_network: "NONE",
            fixture_id: "signed-batch-v2",
            fixture_schema_version: 2,
            fixture_relative_path: "schemas/fixtures/signed-batch-v2.json",
            fixture_sha256: "f94dfcf7e3e376e54b5514ceb3016b0b7d97d17366562f7ac4a16286d3aa367d",
            fixture_byte_length: 21_748,
            builder_id: "learning-platform.deterministic-synthetic-fixture-builder.v1",
        }
    }
}

/// Independently checks every allowlisted manifest field at the runtime boundary.
pub fn validate_synthetic_manifest(manifest: &SyntheticIngestManifest<'_>) -> StoreResult<()> {
    manifest_field(manifest.manifest_version == 1, "manifest_version")?;
    manifest_field(
        manifest.data_policy == PHASE1_STORAGE_POLICY.data_policy,
        "data_policy",
    )?;
    manifest_field(manifest.data_class == "SYNTHETIC_ONLY", "data_class")?;
    manifest_field(manifest.network_egress == "NONE", "network_egress")?;
    manifest_field(
        manifest.storage_mode == PHASE1_STORAGE_POLICY.storage_mode,
        "storage_mode",
    )?;
    manifest_field(
        manifest.storage_encryption == PHASE1_STORAGE_POLICY.storage_encryption,
        "storage_encryption",
    )?;
    manifest_field(
        manifest.production_data_allowed == PHASE1_STORAGE_POLICY.production_data_allowed,
        "production_data_allowed",
    )?;
    manifest_field(
        manifest.product_network == PHASE1_STORAGE_POLICY.product_network,
        "product_network",
    )?;
    manifest_field(manifest.fixture_id == "signed-batch-v2", "fixture_id")?;
    manifest_field(
        manifest.fixture_schema_version == 2,
        "fixture_schema_version",
    )?;
    manifest_field(
        manifest.fixture_relative_path == "schemas/fixtures/signed-batch-v2.json",
        "fixture_relative_path",
    )?;
    manifest_field(
        manifest.fixture_sha256
            == "f94dfcf7e3e376e54b5514ceb3016b0b7d97d17366562f7ac4a16286d3aa367d",
        "fixture_sha256",
    )?;
    manifest_field(
        manifest.fixture_byte_length == 21_748,
        "fixture_byte_length",
    )?;
    manifest_field(
        manifest.builder_id == "learning-platform.deterministic-synthetic-fixture-builder.v1",
        "builder_id",
    )
}

/// Writes the mandatory warning line with no quiet or bypass argument.
pub fn write_policy_banner(mut output: impl Write) -> StoreResult<()> {
    output
        .write_all(PHASE1_POLICY_BANNER.as_bytes())
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|source| StoreError::io("write synthetic-only policy banner", "<output>", source))
}

/// A root with its explicit incomplete marker durably written.
#[derive(Debug)]
pub struct IncompleteProfile {
    root: PathBuf,
}

impl IncompleteProfile {
    /// Returns the root that startup must refuse until completion.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Runs migration before any listener exists and atomically removes the incomplete marker last.
    pub fn complete<P: PathProbe + ?Sized>(
        self,
        probe: &P,
        creating_build_digest: [u8; 32],
    ) -> StoreResult<SyntheticProfile> {
        validate_existing_profile_path(&self.root, probe)?;
        verify_marker(&self.root)?;
        verify_complete_incomplete_marker(&self.root)?;
        let database_path = self.root.join(STORE_DATABASE_FILE);
        let migration_status = migrate_pre_listen(&database_path, creating_build_digest)?;
        let incomplete_path = self.root.join(INCOMPLETE_PROFILE_MARKER);
        fs::remove_file(&incomplete_path).map_err(|source| {
            StoreError::io("remove incomplete profile marker", &incomplete_path, source)
        })?;
        sync_directory(&self.root)?;
        Ok(SyntheticProfile {
            root: self.root,
            database_path,
            migration_status,
        })
    }
}

/// A complete synthetic-only profile whose marker and schema were verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticProfile {
    root: PathBuf,
    database_path: PathBuf,
    migration_status: MigrationStatus,
}

impl SyntheticProfile {
    /// Returns the profile root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the SQLite database path.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Reports whether this opening applied migration 0001 or found it current.
    #[must_use]
    pub const fn migration_status(&self) -> MigrationStatus {
        self.migration_status
    }

    /// Opens the sole product writer with its authorizer installed.
    pub fn open_writer(&self) -> StoreResult<WriterConnection> {
        open_writer(&self.database_path)
    }

    /// Opens a filesystem-read-only, SQLite-query-only reader.
    pub fn open_reader(&self) -> StoreResult<ReaderConnection> {
        open_reader(&self.database_path)
    }
}

/// Creates a secure root and writes the incomplete marker before any database work.
pub fn prepare_synthetic_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
) -> StoreResult<IncompleteProfile> {
    let validated = validate_new_profile_path(root, probe)?;
    if validated.root_state() == ProfileRootState::Missing {
        platform::create_profile_directory(root).map_err(probe_failure)?;
    }
    validate_created_profile_path(root, probe)?;

    let incomplete_path = root.join(INCOMPLETE_PROFILE_MARKER);
    write_new_synced_file(
        &incomplete_path,
        INCOMPLETE_PROFILE_MARKER_CONTENTS.as_bytes(),
    )?;
    sync_directory(root)?;

    let policy_path = root.join(SYNTHETIC_PROFILE_MARKER);
    write_new_synced_file(&policy_path, SYNTHETIC_PROFILE_MARKER_CONTENTS.as_bytes())?;
    sync_directory(root)?;
    Ok(IncompleteProfile {
        root: root.to_path_buf(),
    })
}

/// Creates and migrates a new synthetic-only profile.
pub fn create_synthetic_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
    creating_build_digest: [u8; 32],
) -> StoreResult<SyntheticProfile> {
    prepare_synthetic_profile(root, probe)?.complete(probe, creating_build_digest)
}

/// Opens a complete profile and refuses any interrupted bootstrap first.
pub fn open_synthetic_profile<P: PathProbe + ?Sized>(
    root: &Path,
    probe: &P,
) -> StoreResult<SyntheticProfile> {
    validate_existing_profile_path(root, probe)?;
    let incomplete_path = root.join(INCOMPLETE_PROFILE_MARKER);
    match fs::symlink_metadata(&incomplete_path) {
        Ok(_) => return Err(StoreError::IncompleteProfile(root.to_path_buf())),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(StoreError::io(
                "inspect incomplete profile marker",
                incomplete_path,
                source,
            ));
        }
    }
    verify_marker(root)?;
    let database_path = root.join(STORE_DATABASE_FILE);
    require_regular_file(&database_path)?;
    let writer = open_writer(&database_path)?;
    drop(writer);
    Ok(SyntheticProfile {
        root: root.to_path_buf(),
        database_path,
        migration_status: MigrationStatus::AlreadyCurrent,
    })
}

/// Removes only a provably incomplete profile containing no unrecognized entries.
///
/// This never performs recursive deletion. Any unknown entry or link makes cleanup fail closed.
pub fn remove_incomplete_profile<P: PathProbe + ?Sized>(root: &Path, probe: &P) -> StoreResult<()> {
    validate_existing_profile_path(root, probe)?;
    verify_removable_incomplete_marker(root)?;
    let mut entries = fs::read_dir(root)
        .map_err(|source| StoreError::io("enumerate incomplete profile", root, source))?;
    for result in &mut entries {
        let entry = result
            .map_err(|source| StoreError::io("read incomplete profile entry", root, source))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| StoreError::InvalidProfileState {
                path: entry.path(),
                reason: "incomplete profile contains a non-Unicode entry",
            })?;
        if !is_known_profile_file(name) {
            return Err(StoreError::InvalidProfileState {
                path: entry.path(),
                reason: "incomplete profile contains an unrecognized entry",
            });
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
            StoreError::io("inspect incomplete profile entry", entry.path(), source)
        })?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(StoreError::InvalidProfileState {
                path: entry.path(),
                reason: "incomplete profile entry is not a regular file",
            });
        }
    }
    for name in known_profile_files() {
        let path = root.join(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(StoreError::io(
                    "remove incomplete profile file",
                    path,
                    source,
                ));
            }
        }
    }
    sync_directory(root)?;
    fs::remove_dir(root)
        .map_err(|source| StoreError::io("remove empty incomplete profile", root, source))?;
    if let Some(parent) = root.parent() {
        sync_parent_directory(parent)?;
    }
    Ok(())
}

fn manifest_field(accepted: bool, field: &'static str) -> StoreResult<()> {
    if accepted {
        Ok(())
    } else {
        Err(StoreError::ManifestRejected { field })
    }
}

fn write_new_synced_file(path: &Path, contents: &[u8]) -> StoreResult<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| StoreError::io("create profile marker", path, source))?;
    file.write_all(contents)
        .map_err(|source| StoreError::io("write profile marker", path, source))?;
    file.sync_all()
        .map_err(|source| StoreError::io("synchronize profile marker", path, source))
}

fn verify_marker(root: &Path) -> StoreResult<()> {
    let path = root.join(SYNTHETIC_PROFILE_MARKER);
    let contents = read_bounded_file(&path, SYNTHETIC_PROFILE_MARKER_CONTENTS.len() + 1)?;
    if contents == SYNTHETIC_PROFILE_MARKER_CONTENTS.as_bytes() {
        Ok(())
    } else {
        Err(StoreError::InvalidPolicyMarker(path))
    }
}

fn verify_complete_incomplete_marker(root: &Path) -> StoreResult<()> {
    let path = root.join(INCOMPLETE_PROFILE_MARKER);
    let contents = read_bounded_file(&path, INCOMPLETE_PROFILE_MARKER_CONTENTS.len() + 1)
        .map_err(|_| StoreError::IncompleteProfile(root.to_path_buf()))?;
    if contents == INCOMPLETE_PROFILE_MARKER_CONTENTS.as_bytes() {
        Ok(())
    } else {
        Err(StoreError::IncompleteProfile(root.to_path_buf()))
    }
}

fn verify_removable_incomplete_marker(root: &Path) -> StoreResult<()> {
    let path = root.join(INCOMPLETE_PROFILE_MARKER);
    let contents = read_bounded_file(&path, INCOMPLETE_PROFILE_MARKER_CONTENTS.len() + 1)
        .map_err(|_| StoreError::IncompleteProfile(root.to_path_buf()))?;
    if INCOMPLETE_PROFILE_MARKER_CONTENTS
        .as_bytes()
        .starts_with(&contents)
    {
        Ok(())
    } else {
        Err(StoreError::IncompleteProfile(root.to_path_buf()))
    }
}

fn read_bounded_file(path: &Path, maximum_bytes: usize) -> StoreResult<Vec<u8>> {
    require_regular_file(path)?;
    let file =
        File::open(path).map_err(|source| StoreError::io("open profile marker", path, source))?;
    let mut contents = Vec::new();
    file.take(
        u64::try_from(maximum_bytes).map_err(|_| StoreError::InvalidProfileState {
            path: path.to_path_buf(),
            reason: "profile marker bound does not fit u64",
        })?,
    )
    .read_to_end(&mut contents)
    .map_err(|source| StoreError::io("read profile marker", path, source))?;
    Ok(contents)
}

fn require_regular_file(path: &Path) -> StoreResult<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| StoreError::io("inspect profile file", path, source))?;
    if metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(StoreError::InvalidProfileState {
            path: path.to_path_buf(),
            reason: "profile entry is not a regular file",
        })
    }
}

fn sync_directory(path: &Path) -> StoreResult<()> {
    platform::sync_directory(path).map_err(probe_failure)
}

fn sync_parent_directory(path: &Path) -> StoreResult<()> {
    platform::sync_parent_directory(path).map_err(probe_failure)
}

fn probe_failure(failure: crate::path_policy::PathProbeFailure) -> StoreError {
    StoreError::UnsafeProfilePath(PathPolicyViolation::ProbeFailed(failure))
}

fn known_profile_files() -> [&'static str; 6] {
    [
        SYNTHETIC_PROFILE_MARKER,
        STORE_DATABASE_FILE,
        concat!("academic-platform.sqlite3", "-wal"),
        concat!("academic-platform.sqlite3", "-shm"),
        concat!("academic-platform.sqlite3", "-journal"),
        INCOMPLETE_PROFILE_MARKER,
    ]
}

fn is_known_profile_file(name: &str) -> bool {
    known_profile_files().contains(&name)
}
