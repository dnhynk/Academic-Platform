//! Filesystem discipline for a bundle: portable paths, staging, one rename.
//!
//! This repeats the Phase 1 portability rules rather than importing them, and
//! the repetition is the point. `academic-portability`'s helpers are private to
//! that crate, and its plaintext lane links the store; a reader that had to
//! link the product's database engine to open a directory would contradict the
//! sentence this crate exists for. What is repeated is a rule set, not a
//! decision, and `the_portable_path_rules_match_the_phase_1_export` compares
//! the two rule sets rather than trusting the repetition.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::ContentDigest;
use sha2::{Digest, Sha256};

use crate::{ExportError, ExportResult, MAX_BUNDLE_RELATIVE_PATH_BYTES};

const COPY_CHUNK_BYTES: usize = 64 * 1024;
static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

/// Path components Windows refuses regardless of directory or extension.
pub const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Rejects a relative path a bundle may not contain.
///
/// Length, reserved Windows device names, trailing dots or spaces, and every
/// character Windows refuses fail closed on **every** host, so a format change
/// that would make a bundle unopenable on Windows is caught on Linux too.
pub fn check_relative_path(relative: &str) -> ExportResult<()> {
    if relative.len() > MAX_BUNDLE_RELATIVE_PATH_BYTES {
        return Err(ExportError::UnportablePath {
            reason: "longer than the portable path budget",
            path: relative.to_owned(),
        });
    }
    if relative.contains('\\') || relative.contains('\0') {
        return Err(ExportError::UnportablePath {
            reason: "a bundle path is forward-slash separated",
            path: relative.to_owned(),
        });
    }
    for component in relative.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(ExportError::UnportablePath {
                reason: "empty or traversing component",
                path: relative.to_owned(),
            });
        }
        if component.ends_with('.') || component.ends_with(' ') {
            return Err(ExportError::UnportablePath {
                reason: "component ends with a dot or a space",
                path: relative.to_owned(),
            });
        }
        if component.bytes().any(|byte| {
            matches!(byte, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*') || byte < 0x20
        }) {
            return Err(ExportError::UnportablePath {
                reason: "component holds a character Windows refuses",
                path: relative.to_owned(),
            });
        }
        let stem = component.split('.').next().unwrap_or(component);
        if WINDOWS_RESERVED_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(stem))
        {
            return Err(ExportError::UnportablePath {
                reason: "component is a reserved Windows device name",
                path: relative.to_owned(),
            });
        }
    }
    Ok(())
}

/// Resolves a manifest-relative forward-slash path inside one root.
pub fn resolve_relative(root: &Path, relative: &str) -> ExportResult<PathBuf> {
    check_relative_path(relative)?;
    let mut resolved = root.to_path_buf();
    for part in relative.split('/') {
        resolved.push(part);
    }
    Ok(resolved)
}

/// Refuses a destination that already exists in any form.
pub fn require_absent(path: &Path) -> ExportResult<()> {
    match fs::symlink_metadata(path) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(ExportError::DestinationExists(path.to_path_buf())),
        Err(source) => Err(ExportError::io("inspect bundle destination", path, source)),
    }
}

/// Reserves an absent sibling staging path beside a destination.
pub fn reserve_staging_path(destination: &Path) -> ExportResult<PathBuf> {
    let parent = destination.parent().ok_or_else(|| ExportError::Malformed {
        item: "bundle destination",
        value: destination.display().to_string(),
    })?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ExportError::Malformed {
            item: "bundle destination",
            value: destination.display().to_string(),
        })?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExportError::Malformed {
            item: "system clock",
            value: "before the Unix epoch".to_owned(),
        })?
        .as_nanos();
    for _ in 0..64 {
        let sequence = NEXT_STAGING.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            "{name}.bundle-staging-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        match fs::symlink_metadata(&candidate) {
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => {}
            Err(source) => {
                return Err(ExportError::io(
                    "inspect bundle staging directory",
                    &candidate,
                    source,
                ));
            }
        }
    }
    Err(ExportError::DestinationExists(destination.to_path_buf()))
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> ExportResult<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|source| ExportError::io("create bundle directory", path, source))
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> ExportResult<()> {
    fs::create_dir(path).map_err(|source| ExportError::io("create bundle directory", path, source))
}

/// Creates one directory that must not already exist.
pub fn create_new_directory(path: &Path) -> ExportResult<()> {
    create_private_directory(path)
}

/// Creates every missing directory below a root, refusing a non-directory.
pub fn create_directories(path: &Path) -> ExportResult<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if current.parent().is_none() {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => {
                return Err(ExportError::Malformed {
                    item: "bundle directory",
                    value: current.display().to_string(),
                });
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                create_private_directory(&current)?;
            }
            Err(source) => {
                return Err(ExportError::io(
                    "inspect bundle directory",
                    &current,
                    source,
                ));
            }
        }
    }
    Ok(())
}

/// Writes one new file below a staging root, creating its parents.
pub fn write_new_file(staging: &Path, relative: &str, bytes: &[u8]) -> ExportResult<()> {
    let path = resolve_relative(staging, relative)?;
    if let Some(parent) = path.parent() {
        create_directories(parent)?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|source| ExportError::io("create bundle file", &path, source))?;
    file.write_all(bytes)
        .map_err(|source| ExportError::io("write bundle file", &path, source))?;
    file.sync_all()
        .map_err(|source| ExportError::io("synchronize bundle file", &path, source))
}

/// Copies one file below a staging root and returns its exact digest and length.
///
/// The bytes stream through a fixed buffer and are never held in a value, which
/// is why no type in this crate has a byte-buffer field to classify.
pub fn copy_new_file(
    staging: &Path,
    relative: &str,
    source_path: &Path,
) -> ExportResult<(ContentDigest, u64)> {
    let destination = resolve_relative(staging, relative)?;
    if let Some(parent) = destination.parent() {
        create_directories(parent)?;
    }
    let mut input = File::open(source_path)
        .map_err(|error| ExportError::io("open bundle source file", source_path, error))?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&destination)
        .map_err(|error| ExportError::io("create bundle copy", &destination, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_CHUNK_BYTES];
    let mut length = 0_u64;
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| ExportError::io("read bundle source file", source_path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|error| ExportError::io("write bundle copy", &destination, error))?;
        length = length.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    output
        .sync_all()
        .map_err(|error| ExportError::io("synchronize bundle copy", &destination, error))?;
    Ok((
        ContentDigest::from_sha256_bytes(hasher.finalize().into()),
        length,
    ))
}

/// Hashes one file, returning its digest and exact length.
pub fn hash_file(path: &Path) -> ExportResult<(ContentDigest, u64)> {
    let mut file =
        File::open(path).map_err(|source| ExportError::io("open bundle file", path, source))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_CHUNK_BYTES];
    let mut length = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| ExportError::io("read bundle file", path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        length = length.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    Ok((
        ContentDigest::from_sha256_bytes(hasher.finalize().into()),
        length,
    ))
}

/// Lists one directory's immediate children in stable sorted order.
fn read_directory(root: &Path) -> ExportResult<Vec<PathBuf>> {
    let mut entries = Vec::new();
    let listing = fs::read_dir(root)
        .map_err(|source| ExportError::io("enumerate bundle directory", root, source))?;
    for entry in listing {
        let entry =
            entry.map_err(|source| ExportError::io("read bundle directory entry", root, source))?;
        entries.push(entry.path());
    }
    entries.sort();
    Ok(entries)
}

/// Lists every regular file below a root as sorted relative forward-slash paths.
///
/// The listing is what makes the label check exhaustive: it reads what is on
/// disk rather than what a caller remembered to declare.
pub fn list_files(root: &Path) -> ExportResult<Vec<String>> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<String>) -> ExportResult<()> {
    for entry in read_directory(current)? {
        let metadata = fs::symlink_metadata(&entry)
            .map_err(|source| ExportError::io("inspect bundle entry", &entry, source))?;
        if metadata.file_type().is_symlink() {
            return Err(ExportError::Malformed {
                item: "bundle entry",
                value: entry.display().to_string(),
            });
        }
        if metadata.file_type().is_dir() {
            collect_files(root, &entry, files)?;
        } else if metadata.file_type().is_file() {
            files.push(relative_path_string(root, &entry)?);
        } else {
            return Err(ExportError::Malformed {
                item: "bundle entry",
                value: entry.display().to_string(),
            });
        }
    }
    Ok(())
}

/// Renders a relative forward-slash path string for a manifest.
fn relative_path_string(root: &Path, path: &Path) -> ExportResult<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ExportError::Malformed {
            item: "bundle entry",
            value: path.display().to_string(),
        })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| ExportError::Malformed {
                    item: "bundle entry",
                    value: path.display().to_string(),
                })?;
                parts.push(part.to_owned());
            }
            _ => {
                return Err(ExportError::Malformed {
                    item: "bundle entry",
                    value: path.display().to_string(),
                });
            }
        }
    }
    if parts.is_empty() {
        return Err(ExportError::Malformed {
            item: "bundle entry",
            value: path.display().to_string(),
        });
    }
    Ok(parts.join("/"))
}

/// Synchronizes one directory entry.
fn sync_directory(path: &Path) -> ExportResult<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| ExportError::io("inspect bundle directory", path, source))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(ExportError::Malformed {
            item: "bundle directory",
            value: path.display().to_string(),
        });
    }
    #[cfg(unix)]
    {
        let directory = File::open(path)
            .map_err(|source| ExportError::io("open bundle directory", path, source))?;
        directory
            .sync_all()
            .map_err(|source| ExportError::io("synchronize bundle directory", path, source))?;
    }
    Ok(())
}

/// Synchronizes a complete tree, deepest entries first.
pub fn sync_tree(root: &Path) -> ExportResult<()> {
    for entry in read_directory(root)? {
        let metadata = fs::symlink_metadata(&entry)
            .map_err(|source| ExportError::io("inspect bundle entry", &entry, source))?;
        if metadata.file_type().is_symlink() {
            return Err(ExportError::Malformed {
                item: "bundle entry",
                value: entry.display().to_string(),
            });
        }
        if metadata.file_type().is_dir() {
            sync_tree(&entry)?;
        }
    }
    sync_directory(root)
}

/// Publishes a staged bundle with one rename.
pub fn publish(staging: &Path, destination: &Path) -> ExportResult<()> {
    require_absent(destination)?;
    fs::rename(staging, destination)
        .map_err(|source| ExportError::io("publish bundle", destination, source))
}

/// Removes a staging tree that will not be published.
pub fn remove_staging(staging: &Path) -> ExportResult<()> {
    match fs::remove_dir_all(staging) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ExportError::io("remove bundle staging", staging, source)),
    }
}
