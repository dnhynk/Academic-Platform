//! Versioned physical layout and safe directory creation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use academic_domain::{
    ArtifactDescriptor, DomainId, PermissionLineageId, RetentionClass, VaultLocator,
};

use crate::{VaultError, VaultResult, durability, encode_hex};

const VAULT_DIRECTORY: &str = "vault";
const LEASES_DIRECTORY: &str = "leases";
const TEMP_DIRECTORY: &str = "tmp";
const QUARANTINE_DIRECTORY: &str = "quarantine";

/// One physical object namespace below a profile's `vault/` directory.
///
/// A layout is bound to exactly one of these for its whole life, so a vault
/// that owns the synthetic namespace has no spelling for an encrypted object
/// path and an encrypted vault has none for a plaintext one. t068 section 3.4
/// requires exactly that separation: a reader accepts format `1` only inside a
/// synthetic profile, and a writer in an encrypted profile emits `2` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    /// `PLAINTEXT_SYNTHETIC_V1` under `vault/v1`, file extension `.obj`.
    PlaintextSyntheticV1,
    /// `AEAD_CHUNKED_V2` under `vault/v2`, file extension `.aobj`.
    AeadChunkedV2,
}

impl ObjectFormat {
    /// Returns the descriptor `format_version` this namespace admits.
    #[must_use]
    pub const fn format_version(self) -> u16 {
        match self {
            Self::PlaintextSyntheticV1 => 1,
            Self::AeadChunkedV2 => 2,
        }
    }

    /// Returns the `vault/<component>` directory holding the namespace.
    #[must_use]
    pub const fn directory_component(self) -> &'static str {
        match self {
            Self::PlaintextSyntheticV1 => "v1",
            Self::AeadChunkedV2 => "v2",
        }
    }

    /// Returns the object file extension, without its dot.
    #[must_use]
    pub const fn object_extension(self) -> &'static str {
        match self {
            Self::PlaintextSyntheticV1 => "obj",
            Self::AeadChunkedV2 => "aobj",
        }
    }
}

/// Canonical physical namespace below one synthetic profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultLayout {
    format: ObjectFormat,
    profile_root: PathBuf,
    vault_root: PathBuf,
    objects_root: PathBuf,
    leases_directory: PathBuf,
    leases_root: PathBuf,
    temp_dir: PathBuf,
    quarantine_dir: PathBuf,
}

impl VaultLayout {
    pub(crate) fn new(profile_root: &Path, format: ObjectFormat) -> Self {
        let vault_root = profile_root.join(VAULT_DIRECTORY);
        let leases_directory = vault_root.join(LEASES_DIRECTORY);
        Self {
            format,
            profile_root: profile_root.to_path_buf(),
            objects_root: vault_root.join(format.directory_component()),
            leases_root: leases_directory.join(format.directory_component()),
            leases_directory,
            temp_dir: vault_root.join(TEMP_DIRECTORY),
            quarantine_dir: vault_root.join(QUARANTINE_DIRECTORY),
            vault_root,
        }
    }

    /// Returns the object format this namespace is bound to.
    #[must_use]
    pub const fn format(&self) -> ObjectFormat {
        self.format
    }

    pub(crate) fn initialize(&self) -> VaultResult<()> {
        require_safe_directory(&self.profile_root)?;
        ensure_child_directory(&self.profile_root, &self.vault_root)?;
        ensure_child_directory(&self.vault_root, &self.objects_root)?;
        ensure_child_directory(&self.vault_root, &self.leases_directory)?;
        ensure_child_directory(&self.leases_directory, &self.leases_root)?;
        ensure_child_directory(&self.vault_root, &self.temp_dir)?;
        ensure_child_directory(&self.vault_root, &self.quarantine_dir)?;
        durability::sync_directory(&self.vault_root)?;
        Ok(())
    }

    /// Returns the profile root that owns this vault namespace.
    #[must_use]
    pub fn profile_root(&self) -> &Path {
        &self.profile_root
    }

    /// Returns the profile's vault directory.
    #[must_use]
    pub fn vault_root(&self) -> &Path {
        &self.vault_root
    }

    /// Returns this layout's object namespace.
    #[must_use]
    pub fn objects_root(&self) -> &Path {
        &self.objects_root
    }

    /// Returns the operational object-lease namespace.
    ///
    /// Lease files are not artifacts or canonical truth. They coordinate every product-controlled
    /// object publish, verification, quarantine, remove, and replacement path across processes.
    #[must_use]
    pub fn leases_root(&self) -> &Path {
        &self.leases_root
    }

    /// Returns the ingest-temp directory.
    #[must_use]
    pub fn temp_dir(&self) -> &Path {
        &self.temp_dir
    }

    /// Returns the orphan quarantine directory.
    #[must_use]
    pub fn quarantine_dir(&self) -> &Path {
        &self.quarantine_dir
    }

    /// Computes the canonical policy-namespaced path for a descriptor.
    pub fn object_path(&self, descriptor: &ArtifactDescriptor) -> VaultResult<PathBuf> {
        if descriptor.format_version != self.format.format_version() {
            return Err(VaultError::UnsafeEntry(self.objects_root.clone()));
        }
        Ok(self.object_path_parts(
            descriptor.domain_id,
            descriptor.retention_class,
            descriptor.permission_lineage_id,
            &descriptor.vault_locator,
        ))
    }

    pub(crate) fn object_path_parts(
        &self,
        domain_id: DomainId,
        retention_class: RetentionClass,
        permission_lineage_id: PermissionLineageId,
        locator: &VaultLocator,
    ) -> PathBuf {
        let encoded_locator = encode_hex(locator.as_bytes());
        let extension = self.format.object_extension();
        self.objects_root
            .join(domain_id.to_string())
            .join(retention_component(retention_class))
            .join(permission_lineage_id.to_string())
            .join(&encoded_locator[0..2])
            .join(&encoded_locator[2..4])
            .join(format!("{encoded_locator}.{extension}"))
    }

    pub(crate) fn ensure_object_parent(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<PathBuf> {
        let object_path = self.object_path(descriptor)?;
        self.ensure_namespaced_parent(&self.objects_root, &object_path)?;
        Ok(object_path)
    }

    pub(crate) fn ensure_lease_path(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<PathBuf> {
        if descriptor.format_version != self.format.format_version() {
            return Err(VaultError::UnsafeEntry(self.leases_root.clone()));
        }
        let lease_path = self.lease_path_parts(
            descriptor.domain_id,
            descriptor.retention_class,
            descriptor.permission_lineage_id,
            &descriptor.vault_locator,
        );
        self.ensure_namespaced_parent(&self.leases_root, &lease_path)?;
        Ok(lease_path)
    }

    pub(crate) fn ensure_lease_path_for_object(&self, object_path: &Path) -> VaultResult<PathBuf> {
        if !self.is_canonical_object_path(object_path) {
            return Err(VaultError::UnsafeEntry(object_path.to_path_buf()));
        }
        let relative = object_path
            .strip_prefix(&self.objects_root)
            .map_err(|_| VaultError::UnsafeEntry(object_path.to_path_buf()))?;
        let mut lease_path = self.leases_root.join(relative);
        if !lease_path.set_extension("lease") {
            return Err(VaultError::UnsafeEntry(object_path.to_path_buf()));
        }
        self.ensure_namespaced_parent(&self.leases_root, &lease_path)?;
        Ok(lease_path)
    }

    fn lease_path_parts(
        &self,
        domain_id: DomainId,
        retention_class: RetentionClass,
        permission_lineage_id: PermissionLineageId,
        locator: &VaultLocator,
    ) -> PathBuf {
        let encoded_locator = encode_hex(locator.as_bytes());
        self.leases_root
            .join(domain_id.to_string())
            .join(retention_component(retention_class))
            .join(permission_lineage_id.to_string())
            .join(&encoded_locator[0..2])
            .join(&encoded_locator[2..4])
            .join(format!("{encoded_locator}.lease"))
    }

    fn ensure_namespaced_parent(&self, root: &Path, path: &Path) -> VaultResult<()> {
        let parent = path
            .parent()
            .ok_or_else(|| VaultError::UnsafeEntry(path.to_path_buf()))?;
        let relative = parent
            .strip_prefix(root)
            .map_err(|_| VaultError::UnsafeEntry(parent.to_path_buf()))?;
        let mut current = root.to_path_buf();
        require_safe_directory(&current)?;
        for component in relative.components() {
            current.push(component.as_os_str());
            let parent = current
                .parent()
                .ok_or_else(|| VaultError::UnsafeEntry(current.clone()))?;
            ensure_child_directory(parent, &current)?;
        }
        Ok(())
    }

    pub(crate) fn is_canonical_object_path(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.objects_root) else {
            return false;
        };
        let components = relative
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>();
        let Some(components) = components else {
            return false;
        };
        if components.len() != 6 {
            return false;
        }
        let filename = components[5];
        let suffix = format!(".{}", self.format.object_extension());
        let Some(locator) = filename.strip_suffix(suffix.as_str()) else {
            return false;
        };
        is_uuid_component(components[0])
            && is_retention_component(components[1])
            && is_uuid_component(components[2])
            && is_hex_component(components[3], 2)
            && is_hex_component(components[4], 2)
            && is_hex_component(locator, 64)
            && components[3] == &locator[0..2]
            && components[4] == &locator[2..4]
    }
}

fn retention_component(value: RetentionClass) -> &'static str {
    match value {
        RetentionClass::Ephemeral => "EPHEMERAL",
        RetentionClass::CourseTerm => "COURSE_TERM",
        RetentionClass::UserManaged => "USER_MANAGED",
        RetentionClass::LegalHold => "LEGAL_HOLD",
    }
}

fn is_retention_component(value: &str) -> bool {
    matches!(
        value,
        "EPHEMERAL" | "COURSE_TERM" | "USER_MANAGED" | "LEGAL_HOLD"
    )
}

fn is_uuid_component(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

fn is_hex_component(value: &str, expected_length: usize) -> bool {
    value.len() == expected_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn ensure_child_directory(parent: &Path, child: &Path) -> VaultResult<()> {
    if child.parent() != Some(parent) {
        return Err(VaultError::UnsafeEntry(child.to_path_buf()));
    }
    match fs::symlink_metadata(child) {
        Ok(_) => require_safe_directory(child),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            create_private_directory(child)?;
            require_safe_directory(child)?;
            durability::sync_directory(parent)
        }
        Err(source) => Err(VaultError::io("inspect vault directory", child, source)),
    }
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> VaultResult<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(source) => Err(VaultError::io(
            "create private vault directory",
            path,
            source,
        )),
    }
}

#[cfg(windows)]
fn create_private_directory(path: &Path) -> VaultResult<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(source) => Err(VaultError::io(
            "create private vault directory",
            path,
            source,
        )),
    }
}

#[cfg(not(any(unix, windows)))]
fn create_private_directory(path: &Path) -> VaultResult<()> {
    let _ = path;
    Err(VaultError::UnsafeEntry(path.to_path_buf()))
}

fn require_safe_directory(path: &Path) -> VaultResult<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| VaultError::io("inspect vault directory", path, source))?;
    if !metadata.file_type().is_dir() || is_link_or_reparse(&metadata) {
        return Err(VaultError::UnsafeEntry(path.to_path_buf()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(VaultError::UnsafeEntry(path.to_path_buf()));
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
pub(crate) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes()
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_link_or_reparse(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_path_shape_rejects_bad_fanout() {
        let root = PathBuf::from("profile");
        let layout = VaultLayout::new(&root, ObjectFormat::PlaintextSyntheticV1);
        let valid = layout
            .objects_root()
            .join("01900000-0000-7000-8000-000000000001")
            .join("USER_MANAGED")
            .join("01900000-0000-7000-8000-000000000002")
            .join("ab")
            .join("cd")
            .join(format!("{}{}{}.obj", "ab", "cd", "0".repeat(60)));
        assert!(layout.is_canonical_object_path(&valid));
        assert!(
            !layout.is_canonical_object_path(&valid.with_file_name(format!(
                "{}{}{}.obj",
                "ff",
                "cd",
                "0".repeat(60)
            )))
        );
    }

    #[test]
    fn the_two_object_namespaces_never_share_a_path() {
        let root = PathBuf::from("profile");
        let plaintext = VaultLayout::new(&root, ObjectFormat::PlaintextSyntheticV1);
        let encrypted = VaultLayout::new(&root, ObjectFormat::AeadChunkedV2);
        assert_ne!(plaintext.objects_root(), encrypted.objects_root());
        assert_ne!(plaintext.leases_root(), encrypted.leases_root());
        assert_eq!(plaintext.temp_dir(), encrypted.temp_dir());

        let name = format!("{}{}{}", "ab", "cd", "0".repeat(60));
        let tail = |layout: &VaultLayout, extension: &str| {
            layout
                .objects_root()
                .join("01900000-0000-7000-8000-000000000001")
                .join("USER_MANAGED")
                .join("01900000-0000-7000-8000-000000000002")
                .join("ab")
                .join("cd")
                .join(format!("{name}.{extension}"))
        };
        let plaintext_object = tail(&plaintext, "obj");
        let encrypted_object = tail(&encrypted, "aobj");
        assert!(plaintext.is_canonical_object_path(&plaintext_object));
        assert!(encrypted.is_canonical_object_path(&encrypted_object));
        // Neither namespace can name the other's object.
        assert!(!plaintext.is_canonical_object_path(&encrypted_object));
        assert!(!encrypted.is_canonical_object_path(&plaintext_object));
        assert!(!plaintext.is_canonical_object_path(&tail(&plaintext, "aobj")));
    }
}
