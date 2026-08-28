//! Unix durable publication using `renameat2(RENAME_NOREPLACE)` and directory `fsync`.

use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

use rustix::fs::{
    CWD, FlockOperation, Mode, OFlags, RenameFlags, flock, fsync, open, renameat_with,
};

#[derive(Debug)]
pub(crate) struct LockedTemp(File);

impl LockedTemp {
    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.0
    }
}

/// Stable identity of one open Unix object while its file description remains live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectIdentity {
    device: u64,
    inode: u64,
}

/// Shared cooperative namespace lease retained by a sealed capability.
#[derive(Debug)]
pub(crate) struct SharedObjectLease {
    _file: File,
}

/// Exclusive cooperative namespace lease retained by a product mutation.
#[derive(Debug)]
pub(crate) struct ExclusiveObjectLease {
    _file: File,
}

pub(crate) fn create_locked_temp(path: &Path) -> io::Result<LockedTemp> {
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    flock(&file, FlockOperation::NonBlockingLockExclusive).map_err(errno)?;
    Ok(LockedTemp(file))
}

pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    let directory = File::open(path)?;
    fsync(directory).map_err(errno)
}

pub(crate) fn open_readonly_no_follow(path: &Path) -> io::Result<File> {
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(errno)?;
    Ok(File::from(descriptor))
}

pub(crate) fn symlink_metadata_no_follow(path: &Path) -> io::Result<fs::Metadata> {
    fs::symlink_metadata(path)
}

pub(crate) fn publish_no_replace(source: &Path, destination: &Path) -> io::Result<bool> {
    match renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(true),
        Err(error) if error == rustix::io::Errno::EXIST => Ok(false),
        Err(error) => Err(errno(error)),
    }
}

pub(crate) fn publish_locked_no_replace(
    _temp: &mut LockedTemp,
    source: &Path,
    destination: &Path,
) -> io::Result<bool> {
    publish_no_replace(source, destination)
}

pub(crate) fn try_remove_unlocked(path: &Path) -> io::Result<bool> {
    let descriptor = match open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(error) if error == rustix::io::Errno::NOENT => return Ok(true),
        Err(error) => return Err(errno(error)),
    };
    match flock(&descriptor, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {}
        Err(rustix::io::Errno::WOULDBLOCK) => return Ok(false),
        Err(error) => return Err(errno(error)),
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

pub(crate) fn try_acquire_shared_object_lease(
    path: &Path,
) -> io::Result<Option<SharedObjectLease>> {
    let file = open_lease_file(path)?;
    match flock(&file, FlockOperation::NonBlockingLockShared) {
        Ok(()) => Ok(Some(SharedObjectLease { _file: file })),
        Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
        Err(error) => Err(errno(error)),
    }
}

pub(crate) fn try_acquire_exclusive_object_lease(
    path: &Path,
) -> io::Result<Option<ExclusiveObjectLease>> {
    let file = open_lease_file(path)?;
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Some(ExclusiveObjectLease { _file: file })),
        Err(rustix::io::Errno::WOULDBLOCK) => Ok(None),
        Err(error) => Err(errno(error)),
    }
}

pub(crate) fn object_identity(file: &File) -> io::Result<ObjectIdentity> {
    let metadata = file.metadata()?;
    Ok(ObjectIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn open_lease_file(path: &Path) -> io::Result<File> {
    let descriptor = open(
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(errno)?;
    let file = File::from(descriptor);
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "vault object lease is not a regular file",
        ));
    }
    Ok(file)
}

fn errno(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}
