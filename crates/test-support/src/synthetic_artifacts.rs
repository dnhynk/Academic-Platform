//! Shared deterministic fixtures for the Phase 1 synthetic artifact vault.

#![allow(dead_code)]

use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::{
    ArtifactId, Confidentiality, DomainId, MediaType, PermissionLineageId, RetentionClass,
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring, Vault};

/// Locator key used only by disposable test profiles.
pub const DOMAIN_KEY: &[u8] = b"phase-1-synthetic-domain-locator-key";
/// A distinct key proving that physical equality does not cross domains.
pub const SECOND_DOMAIN_KEY: &[u8] = b"phase-1-second-synthetic-domain-key";
/// Stable UUIDv7-shaped test identities.
pub const ARTIFACT_ID: &str = "01900000-0000-7000-8000-000000000101";
pub const SECOND_ARTIFACT_ID: &str = "01900000-0000-7000-8000-000000000102";
pub const DOMAIN_ID: &str = "01900000-0000-7000-8000-000000000201";
pub const SECOND_DOMAIN_ID: &str = "01900000-0000-7000-8000-000000000202";
pub const PERMISSION_LINEAGE_ID: &str = "01900000-0000-7000-8000-000000000301";
pub const SECOND_PERMISSION_LINEAGE_ID: &str = "01900000-0000-7000-8000-000000000302";
/// Exact bytes reused by dedupe and crash-recovery tests.
pub const SAMPLE_BYTES: &[u8] = b"synthetic academic artifact\nwith exact bytes\n";

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native path
/// facade refuses to follow a link component, so profile roots are reserved
/// below the real directory.
#[cfg(unix)]
fn temporary_base() -> io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

/// Owner of one unique disposable profile root.
#[derive(Debug)]
pub struct SyntheticTestRoot {
    path: PathBuf,
}

impl SyntheticTestRoot {
    /// Reserves a unique, initially absent path below the host temp directory.
    pub fn new(label: &str) -> io::Result<Self> {
        let label = sanitize_label(label);
        let base = temporary_base()?;
        for _ in 0..64 {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = base.join(format!(
                "academic-vault-{label}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(Self { path });
                }
                Ok(_) => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a unique synthetic profile path",
        ))
    }

    /// Returns the absent-or-owned path for profile creation.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SyntheticTestRoot {
    fn drop(&mut self) {
        match fs::remove_dir_all(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
}

/// Produces enough deterministic data to exercise multiple streaming chunks.
#[must_use]
pub fn large_artifact_bytes() -> Vec<u8> {
    (0_u32..200_000)
        .map(|index| index.wrapping_mul(31).to_le_bytes()[0])
        .collect()
}

/// Creates and opens one complete disposable vault with both fixture domains keyed.
pub fn open_test_vault(label: &str) -> Result<(SyntheticTestRoot, Vault), Box<dyn Error>> {
    let root = SyntheticTestRoot::new(label)?;
    create_private_test_root(root.path())?;
    let mut keyring = DomainKeyring::new();
    keyring.insert(DOMAIN_ID.parse()?, DOMAIN_KEY)?;
    keyring.insert(SECOND_DOMAIN_ID.parse()?, SECOND_DOMAIN_KEY)?;
    let vault = Vault::open(root.path(), keyring)?;
    Ok((root, vault))
}

/// Creates a vault test root with the owner-only Unix mode required by the native path policy.
pub fn create_private_test_root(path: &Path) -> io::Result<()> {
    fs::create_dir(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Constructs the default exact-policy fixture request.
pub fn ingest_request() -> Result<ArtifactIngestRequest, Box<dyn Error>> {
    request_with(
        ARTIFACT_ID,
        DOMAIN_ID,
        RetentionClass::UserManaged,
        PERMISSION_LINEAGE_ID,
    )
}

/// Constructs an ingest request with explicit identity and physical policy namespace.
pub fn request_with(
    artifact_id: &str,
    domain_id: &str,
    retention_class: RetentionClass,
    permission_lineage_id: &str,
) -> Result<ArtifactIngestRequest, Box<dyn Error>> {
    let artifact_id: ArtifactId = artifact_id.parse()?;
    let domain_id: DomainId = domain_id.parse()?;
    let permission_lineage_id: PermissionLineageId = permission_lineage_id.parse()?;
    Ok(ArtifactIngestRequest::new(
        artifact_id,
        MediaType::parse("application/pdf")?,
        domain_id,
        Confidentiality::Restricted,
        retention_class,
        permission_lineage_id,
    ))
}

fn sanitize_label(label: &str) -> String {
    let value = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if value.is_empty() {
        "case".to_owned()
    } else {
        value
    }
}
