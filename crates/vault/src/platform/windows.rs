//! Windows durable publication with live-temp exclusion and bounded sharing retry.

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{
        ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION, GENERIC_READ,
        GENERIC_WRITE, HANDLE,
    },
    Storage::FileSystem::{
        DELETE, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_DELETE_ON_CLOSE, FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_WRITE_THROUGH,
        FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileRenameInfoEx,
        FlushFileBuffers, MOVEFILE_WRITE_THROUGH, MoveFileExW, SetFileInformationByHandle,
    },
};

const DIRECTORY_BARRIER_FILE: &str = ".academic-vault-directory-barrier";
const MAX_SHARING_ATTEMPTS: usize = 6;
const RETRY_MILLIS: [u64; MAX_SHARING_ATTEMPTS - 1] = [2, 4, 8, 16, 32];

/// Live ingest handle.
///
/// The handle explicitly denies delete sharing and carries `DELETE` access itself, allowing a
/// no-replace rename by handle while preventing a concurrent scavenger from opening the temp for
/// deletion. The write-through handle remains live across the rename and final directory sync.
#[derive(Debug)]
pub(crate) struct LockedTemp(File);

impl LockedTemp {
    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.0
    }
}

pub(crate) fn create_locked_temp(path: &Path) -> io::Result<LockedTemp> {
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_WRITE_THROUGH)
        .open(path)?;
    Ok(LockedTemp(file))
}

pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    let directory = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_WRITE_THROUGH)
        .open(path)?;
    let directory_flush = flush_handle(&directory);

    // Some supported Windows filesystems reject FlushFileBuffers for directory handles. A
    // write-through file in that directory supplies a conservative ordering barrier for the
    // preceding metadata operation; it never substitutes for a successful flush when the host
    // reports a different error.
    let barrier_path = path.join(DIRECTORY_BARRIER_FILE);
    require_safe_barrier_shape(&barrier_path)?;
    let mut barrier = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_WRITE_THROUGH)
        .open(&barrier_path)?;
    barrier.write_all(b"PLAINTEXT_SYNTHETIC_V1_DIRECTORY_BARRIER\n")?;
    barrier.sync_all()?;

    match directory_flush {
        Ok(()) => Ok(()),
        Err(error) if is_directory_flush_unsupported(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn open_readonly_no_follow(path: &Path) -> io::Result<File> {
    let verbatim = verbatim_path(path)?;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(verbatim)
}

pub(crate) fn symlink_metadata_no_follow(path: &Path) -> io::Result<fs::Metadata> {
    fs::symlink_metadata(verbatim_path(path)?)
}

pub(crate) fn publish_locked_no_replace(
    temp: &mut LockedTemp,
    _source: &Path,
    destination: &Path,
) -> io::Result<bool> {
    let renamed = rename_handle_no_replace(&temp.0, destination)?;
    if renamed {
        flush_handle(&temp.0)?;
    }
    Ok(renamed)
}

pub(crate) fn publish_no_replace(source: &Path, destination: &Path) -> io::Result<bool> {
    move_file_no_replace(source, destination)
}

pub(crate) fn try_remove_unlocked(path: &Path) -> io::Result<bool> {
    let result = OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(0)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path);
    match result {
        Ok(handle) => {
            drop(handle);
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) if is_sharing_violation(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

#[allow(unsafe_code)]
fn rename_handle_no_replace(file: &File, destination: &Path) -> io::Result<bool> {
    use std::mem::{offset_of, size_of};

    let mut destination_wide = wide_nt_path(destination)?;
    destination_wide.push(0);
    let name_bytes = destination_wide
        .len()
        .checked_mul(size_of::<u16>())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "vault path is too long"))?;
    let name_bytes_u32 = u32::try_from(name_bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "vault path is too long"))?;
    let information_bytes = offset_of!(FILE_RENAME_INFO, FileName)
        .checked_add(name_bytes)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "vault path is too long"))?;
    let information_bytes_u32 = u32::try_from(information_bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "vault path is too long"))?;
    let word_count = information_bytes
        .checked_add(size_of::<usize>() - 1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "vault path is too long"))?
        / size_of::<usize>();
    let mut storage = vec![0_usize; word_count];
    let information = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();

    // SAFETY: `storage` is usize-aligned and sized through the flexible FileName payload;
    // `destination_wide` supplies exactly FileNameLength initialized bytes; the live handle has
    // DELETE access; the extended flags omit replacement; and all buffers remain live for each
    // SetFileInformationByHandle call.
    unsafe {
        (*information).Anonymous.Flags = 0;
        (*information).RootDirectory = std::ptr::null_mut();
        (*information).FileNameLength = name_bytes_u32;
        std::ptr::copy_nonoverlapping(
            destination_wide.as_ptr(),
            (*information).FileName.as_mut_ptr(),
            destination_wide.len(),
        );
    }

    retry_sharing_violation(|| {
        // SAFETY: the validated FILE_RENAME_INFO buffer and live handle described above remain
        // unchanged and live across this bounded synchronous retry.
        let result = unsafe {
            SetFileInformationByHandle(
                raw_handle(file),
                FileRenameInfoEx,
                information.cast(),
                information_bytes_u32,
            )
        };
        if result != 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if destination_exists(&error, destination) {
            Ok(false)
        } else {
            Err(error)
        }
    })
}

#[allow(unsafe_code)]
fn move_file_no_replace(source: &Path, destination: &Path) -> io::Result<bool> {
    let source_wide = wide_path(source)?;
    let destination_wide = wide_path(destination)?;
    retry_sharing_violation(|| {
        // SAFETY: both UTF-16 buffers are NUL-terminated and remain live for the call. The flags
        // omit MOVEFILE_REPLACE_EXISTING, so a concurrent destination is never overwritten, and
        // MOVEFILE_WRITE_THROUGH makes successful publication wait for the filesystem operation.
        let result = unsafe {
            MoveFileExW(
                source_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if result != 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if destination_exists(&error, destination) {
            Ok(false)
        } else {
            Err(error)
        }
    })
}

#[allow(unsafe_code)]
fn flush_handle(file: &File) -> io::Result<()> {
    // SAFETY: `file` owns a live Windows handle for the duration of the call; FlushFileBuffers
    // neither retains the handle nor accesses any caller-provided memory.
    if unsafe { FlushFileBuffers(raw_handle(file)) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn raw_handle(file: &File) -> HANDLE {
    file.as_raw_handle()
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut value = wide_path_without_terminator(path)?;
    value.push(0);
    Ok(value)
}

fn wide_path_without_terminator(path: &Path) -> io::Result<Vec<u16>> {
    const BACKSLASH: u16 = b'\\' as u16;
    const QUESTION: u16 = b'?' as u16;
    const EXTENDED_PREFIX: [u16; 4] = [BACKSLASH, BACKSLASH, QUESTION, BACKSLASH];
    const UNC_PREFIX: [u16; 8] = [
        BACKSLASH,
        BACKSLASH,
        QUESTION,
        BACKSLASH,
        b'U' as u16,
        b'N' as u16,
        b'C' as u16,
        BACKSLASH,
    ];

    let original = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let value = if original.starts_with(&EXTENDED_PREFIX) || !path.is_absolute() {
        original
    } else if original.starts_with(&[BACKSLASH, BACKSLASH]) {
        UNC_PREFIX
            .into_iter()
            .chain(original.into_iter().skip(2))
            .collect()
    } else {
        EXTENDED_PREFIX.into_iter().chain(original).collect()
    };
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows vault path contains an interior NUL",
        ));
    }
    Ok(value)
}

fn verbatim_path(path: &Path) -> io::Result<PathBuf> {
    Ok(PathBuf::from(OsString::from_wide(
        &wide_path_without_terminator(path)?,
    )))
}

fn wide_nt_path(path: &Path) -> io::Result<Vec<u16>> {
    const BACKSLASH: u16 = b'\\' as u16;
    const QUESTION: u16 = b'?' as u16;
    const NT_PREFIX: [u16; 4] = [BACKSLASH, QUESTION, QUESTION, BACKSLASH];
    const NT_UNC_PREFIX: [u16; 8] = [
        BACKSLASH,
        QUESTION,
        QUESTION,
        BACKSLASH,
        b'U' as u16,
        b'N' as u16,
        b'C' as u16,
        BACKSLASH,
    ];

    let original = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if original.contains(&0) || !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows handle rename requires an absolute path without an interior NUL",
        ));
    }
    if original.starts_with(&[BACKSLASH, BACKSLASH]) {
        Ok(NT_UNC_PREFIX
            .into_iter()
            .chain(original.into_iter().skip(2))
            .collect())
    } else {
        Ok(NT_PREFIX.into_iter().chain(original).collect())
    }
}

fn require_safe_barrier_shape(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 =>
        {
            Ok(())
        }
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows directory barrier is not a non-reparse regular file",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn retry_sharing_violation<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    for delay_millis in RETRY_MILLIS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if is_sharing_violation(&error) => {
                thread::sleep(Duration::from_millis(delay_millis));
            }
            Err(error) => return Err(error),
        }
    }
    operation()
}

fn destination_exists(error: &io::Error, destination: &Path) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists
        || matches!(error.raw_os_error(), Some(80 | 183))
        || fs::symlink_metadata(destination).is_ok()
        || verbatim_path(destination).is_ok_and(|path| fs::symlink_metadata(path).is_ok())
}

fn is_sharing_violation(error: &io::Error) -> bool {
    matches!(
        error
            .raw_os_error()
            .and_then(|value| u32::try_from(value).ok()),
        Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION | ERROR_ACCESS_DENIED)
    )
}

fn is_directory_flush_unsupported(error: &io::Error) -> bool {
    // ERROR_INVALID_FUNCTION, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE.
    matches!(error.raw_os_error(), Some(1 | 5 | 6))
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug)]
    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create() -> io::Result<Self> {
            for _ in 0..64 {
                let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "academic-vault-windows-{}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Ok(Self(path)),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
            }
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not allocate Windows vault test directory",
            ))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn sharing_retry_is_bounded_and_eventually_succeeds() -> io::Result<()> {
        let attempts = Cell::new(0_usize);
        let result = retry_sharing_violation(|| {
            let next = attempts.get().saturating_add(1);
            attempts.set(next);
            if next < 3 {
                Err(io::Error::from_raw_os_error(
                    i32::try_from(ERROR_SHARING_VIOLATION).unwrap_or(32),
                ))
            } else {
                Ok("published")
            }
        })?;
        assert_eq!(result, "published");
        assert_eq!(attempts.get(), 3);
        Ok(())
    }

    #[test]
    fn sharing_retry_stops_at_exact_limit() {
        let attempts = Cell::new(0_usize);
        let result = retry_sharing_violation(|| {
            attempts.set(attempts.get().saturating_add(1));
            Err::<(), _>(io::Error::from_raw_os_error(32))
        });
        assert!(result.is_err());
        assert_eq!(attempts.get(), MAX_SHARING_ATTEMPTS);
    }

    #[test]
    fn native_live_temp_excludes_scavenger() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let path = directory.0.join("live.partial");
        let temp = create_locked_temp(&path)?;
        assert!(!try_remove_unlocked(&path)?);
        assert!(path.is_file());
        drop(temp);
        assert!(try_remove_unlocked(&path)?);
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn native_write_through_publish_never_overwrites() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let source = directory.0.join("source.partial");
        let destination = directory.0.join("destination.obj");
        let mut temp = create_locked_temp(&source)?;
        temp.file_mut().write_all(b"new bytes")?;
        temp.file_mut().sync_all()?;
        fs::write(&destination, b"original bytes")?;

        assert!(!publish_locked_no_replace(
            &mut temp,
            &source,
            &destination
        )?);
        assert_eq!(fs::read(&destination)?, b"original bytes");
        assert_eq!(fs::read(&source)?, b"new bytes");
        Ok(())
    }

    #[test]
    fn native_write_through_publish_moves_locked_temp() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let source = directory.0.join("source.partial");
        let destination = directory.0.join("destination.obj");
        let mut temp = create_locked_temp(&source)?;
        temp.file_mut().write_all(b"sealed bytes")?;
        temp.file_mut().sync_all()?;
        assert!(!try_remove_unlocked(&source)?);

        assert!(publish_locked_no_replace(&mut temp, &source, &destination)?);
        assert!(!source.exists());
        assert_eq!(fs::read(&destination)?, b"sealed bytes");
        assert!(!try_remove_unlocked(&destination)?);
        sync_directory(&directory.0)?;
        drop(temp);
        assert!(try_remove_unlocked(&destination)?);
        assert!(!destination.exists());
        Ok(())
    }
}
