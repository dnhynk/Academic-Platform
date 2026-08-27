//! Versioned physical layout and safe directory creation.

use std::{
    fs,
    path::{Path, PathBuf},
};

use academic_domain::{
    ArtifactDescriptor, DomainId, PermissionLineageId, RetentionClass, VaultLocator,
};

use crate::{VAULT_FORMAT_VERSION, VaultError, VaultResult, durability, encode_hex};

const VAULT_DIRECTORY: &str = "vault";
const OBJECTS_DIRECTORY: &str = "v1";
const TEMP_DIRECTORY: &str = "tmp";
const QUARANTINE_DIRECTORY: &str = "quarantine";

/// Canonical physical namespace below one synthetic profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultLayout {
    profile_root: PathBuf,
    vault_root: PathBuf,
    objects_root: PathBuf,
    temp_dir: PathBuf,
    quarantine_dir: PathBuf,
}

impl VaultLayout {
    pub(crate) fn new(profile_root: &Path) -> Self {
        let vault_root = profile_root.join(VAULT_DIRECTORY);
        Self {
            profile_root: profile_root.to_path_buf(),
            objects_root: vault_root.join(OBJECTS_DIRECTORY),
            temp_dir: vault_root.join(TEMP_DIRECTORY),
            quarantine_dir: vault_root.join(QUARANTINE_DIRECTORY),
            vault_root,
        }
    }

    pub(crate) fn initialize(&self) -> VaultResult<()> {
        require_safe_directory(&self.profile_root)?;
        ensure_child_directory(&self.profile_root, &self.vault_root)?;
        ensure_child_directory(&self.vault_root, &self.objects_root)?;
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

    /// Returns the version-one object namespace.
    #[must_use]
    pub fn objects_root(&self) -> &Path {
        &self.objects_root
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
        if descriptor.format_version != VAULT_FORMAT_VERSION {
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
        self.objects_root
            .join(domain_id.to_string())
            .join(retention_component(retention_class))
            .join(permission_lineage_id.to_string())
            .join(&encoded_locator[0..2])
            .join(&encoded_locator[2..4])
            .join(format!("{encoded_locator}.obj"))
    }

    pub(crate) fn ensure_object_parent(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> VaultResult<PathBuf> {
        let object_path = self.object_path(descriptor)?;
        let parent = object_path
            .parent()
            .ok_or_else(|| VaultError::UnsafeEntry(object_path.clone()))?;
        let relative = parent
            .strip_prefix(&self.objects_root)
            .map_err(|_| VaultError::UnsafeEntry(parent.to_path_buf()))?;
        let mut current = self.objects_root.clone();
        require_safe_directory(&current)?;
        for component in relative.components() {
            current.push(component.as_os_str());
            let parent = current
                .parent()
                .ok_or_else(|| VaultError::UnsafeEntry(current.clone()))?;
            ensure_child_directory(parent, &current)?;
        }
        Ok(object_path)
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
        let Some(locator) = filename.strip_suffix(".obj") else {
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
        let layout = VaultLayout::new(&root);
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
}
