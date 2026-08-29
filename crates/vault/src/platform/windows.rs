//! Windows durable publication with live-temp exclusion and bounded sharing retry.

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    mem::size_of,
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
        FILE_ID_INFO, FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FileIdInfo, FileRenameInfoEx, FlushFileBuffers, GetFileInformationByHandleEx,
        MOVEFILE_WRITE_THROUGH, MoveFileExW, SetFileInformationByHandle,
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

/// Stable Windows volume/file ID captured from one live exact-object handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectIdentity {
    volume_serial: u64,
    file_id: [u8; 16],
}

/// Shared namespace lease retained by a sealed capability.
#[derive(Debug)]
pub(crate) struct SharedObjectLease {
    _file: File,
}

/// Exclusive namespace lease retained by a product mutation.
#[derive(Debug)]
pub(crate) struct ExclusiveObjectLease {
    _file: File,
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
    let directory_flush = flush_directory(path);

    // Some supported Windows filesystems reject FlushFileBuffers for directory handles. A
    // write-through file in that directory supplies a conservative ordering barrier for the
    // preceding metadata operation; it never substitutes for a successful flush when the host
    // reports a different error.
    //
    // Concurrent ingests synchronize the same shared directory - `vault/tmp`, an object fan-out
    // parent, a lease parent - so the barrier permits write sharing and its open/write/flush is
    // covered by the same bounded sharing retry as publication. Barrier content is a fixed
    // marker, never read back, so a concurrent truncate is not a loss.
    let barrier_path = path.join(DIRECTORY_BARRIER_FILE);
    require_safe_barrier_shape(&barrier_path)?;
    retry_sharing_violation(|| {
        let mut barrier = open_directory_barrier(&barrier_path)?;
        barrier.write_all(b"PLAINTEXT_SYNTHETIC_V1_DIRECTORY_BARRIER\n")?;
        barrier.sync_all()
    })?;

    match directory_flush {
        Ok(()) => Ok(()),
        Err(error) if is_directory_flush_unsupported(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Opens the explicit durable directory handle and flushes it.
///
/// `FlushFileBuffers` requires write access, so a read-only directory handle always fails with
/// `ERROR_ACCESS_DENIED` and the designed flush never executes. Requesting `GENERIC_WRITE` makes
/// it run; a host that refuses a writable directory handle still reports one of the errors
/// `is_directory_flush_unsupported` classifies, so the barrier remains the fallback there.
fn flush_directory(path: &Path) -> io::Result<()> {
    let directory = OpenOptions::new()
        .read(true)
        .access_mode(GENERIC_READ | GENERIC_WRITE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_WRITE_THROUGH)
        .open(path)?;
    flush_handle(&directory)
}

fn open_directory_barrier(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_WRITE_THROUGH)
        .open(path)
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

pub(crate) fn try_acquire_shared_object_lease(
    path: &Path,
) -> io::Result<Option<SharedObjectLease>> {
    ensure_lease_file(path)?;
    let result = OpenOptions::new()
        .read(true)
        .access_mode(GENERIC_READ)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(verbatim_path(path)?);
    match result {
        Ok(file) => {
            require_regular_non_reparse_handle(&file)?;
            Ok(Some(SharedObjectLease { _file: file }))
        }
        Err(error) if is_sharing_violation(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn try_acquire_exclusive_object_lease(
    path: &Path,
) -> io::Result<Option<ExclusiveObjectLease>> {
    ensure_lease_file(path)?;
    let result = OpenOptions::new()
        .read(true)
        .write(true)
        .access_mode(GENERIC_READ | GENERIC_WRITE)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(verbatim_path(path)?);
    match result {
        Ok(file) => {
            require_regular_non_reparse_handle(&file)?;
            Ok(Some(ExclusiveObjectLease { _file: file }))
        }
        Err(error) if is_sharing_violation(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

#[allow(unsafe_code)]
pub(crate) fn object_identity(file: &File) -> io::Result<ObjectIdentity> {
    let mut information = FILE_ID_INFO::default();
    // SAFETY: `file` owns a live handle and the output buffer has exactly FILE_ID_INFO's size.
    let result = unsafe {
        GetFileInformationByHandleEx(
            raw_handle(file),
            FileIdInfo,
            (&raw mut information).cast(),
            u32::try_from(size_of::<FILE_ID_INFO>()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "file identity buffer is too large",
                )
            })?,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ObjectIdentity {
        volume_serial: information.VolumeSerialNumber,
        file_id: information.FileId.Identifier,
    })
}

fn ensure_lease_file(path: &Path) -> io::Result<()> {
    let verbatim = verbatim_path(path)?;
    match fs::symlink_metadata(&verbatim) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 =>
        {
            return Ok(());
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "vault object lease is not a non-reparse regular file",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let file = open_lease_file_for_creation(&verbatim)?;
    require_regular_non_reparse_handle(&file)
}

/// Creates an absent lease file without requesting more access than a live shared lease permits.
///
/// The existence check above is a fast path, not a guarantee: another thread can create the file
/// and take its shared lease - which permits only `FILE_SHARE_READ` - inside that window, and a
/// `GENERIC_WRITE` request would then fail with `ERROR_SHARING_VIOLATION`. `OPEN_ALWAYS` creates
/// the file from the parent directory's permission, so read access is sufficient. `.write(true)`
/// only satisfies the standard library's creation-mode rule; `access_mode` sets the real request.
fn open_lease_file_for_creation(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .access_mode(GENERIC_READ)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

fn require_regular_non_reparse_handle(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata.file_type().is_file()
        && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
    {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "vault object lease is not a non-reparse regular file",
        ))
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

    Ok(match verbatim_body(path)? {
        VerbatimBody::Rooted(body) => EXTENDED_PREFIX.into_iter().chain(body).collect(),
        VerbatimBody::Unc(body) => UNC_PREFIX.into_iter().chain(body).collect(),
        VerbatimBody::Unprefixed(original) => original,
    })
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

    match verbatim_body(path)? {
        VerbatimBody::Rooted(body) => Ok(NT_PREFIX.into_iter().chain(body).collect()),
        VerbatimBody::Unc(body) => Ok(NT_UNC_PREFIX.into_iter().chain(body).collect()),
        VerbatimBody::Unprefixed(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows handle rename requires an absolute path without an interior NUL",
        )),
    }
}

/// One caller spelling classified for the verbatim namespace.
///
/// Each body is already spelled with single backslashes, so applying the Win32 `\\?\`/`\\?\UNC\`
/// or the NT `\??\`/`\??\UNC\` prefix to it yields a usable path.
enum VerbatimBody {
    /// A drive-rooted body, or the tail of a spelling that already carried a verbatim prefix.
    Rooted(Vec<u16>),
    /// A UNC body stripped of its two leading separators.
    Unc(Vec<u16>),
    /// A spelling no verbatim prefix applies to, returned exactly as the caller wrote it so that
    /// Win32 resolves it against the current directory as it always has.
    Unprefixed(Vec<u16>),
}

/// Classifies `path` for the verbatim namespace, normalising its separators first.
///
/// The verbatim namespace performs no separator translation: a forward slash inside `\\?\` is an
/// ordinary name character, so an unrewritten spelling names one long nonexistent file and every
/// operation below it fails with `ERROR_PATH_NOT_FOUND` far from the cause. Forward-slash and
/// mixed spellings reach the vault routinely - configuration text, a command-line argument, any
/// path built as a string - so the rewrite belongs here rather than at one caller's boundary.
fn verbatim_body(path: &Path) -> io::Result<VerbatimBody> {
    const BACKSLASH: u16 = b'\\' as u16;
    const QUESTION: u16 = b'?' as u16;
    const COLON: u16 = b':' as u16;
    const DOT: u16 = b'.' as u16;
    const EXTENDED_PREFIX: [u16; 4] = [BACKSLASH, BACKSLASH, QUESTION, BACKSLASH];

    let original = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if original.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows vault path contains an interior NUL",
        ));
    }

    let normalized = normalize_separators(&original);
    if let Some(body) = normalized.strip_prefix(&EXTENDED_PREFIX) {
        return require_resolved_components(body).map(VerbatimBody::Rooted);
    }
    if let Some(body) = normalized.strip_prefix(&[BACKSLASH, BACKSLASH]) {
        if matches!(body, [] | [DOT]) || body.starts_with(&[DOT, BACKSLASH]) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows vault path names a device namespace no verbatim prefix applies to",
            ));
        }
        return require_resolved_components(body).map(VerbatimBody::Unc);
    }
    if matches!(normalized.get(1..3), Some([COLON, BACKSLASH])) {
        return require_resolved_components(&normalized).map(VerbatimBody::Rooted);
    }
    Ok(VerbatimBody::Unprefixed(original))
}

/// Rewrites a caller spelling into the single-backslash form the verbatim namespace requires.
///
/// Every forward slash becomes a backslash, interior separator runs collapse, and a trailing
/// separator is dropped unless it belongs to a drive root. Exactly two leading separators survive
/// so that a UNC or already-verbatim spelling keeps its root.
fn normalize_separators(original: &[u16]) -> Vec<u16> {
    const BACKSLASH: u16 = b'\\' as u16;
    const SLASH: u16 = b'/' as u16;
    const COLON: u16 = b':' as u16;

    let is_separator = |unit: u16| unit == BACKSLASH || unit == SLASH;
    let leading = original
        .iter()
        .take_while(|unit| is_separator(**unit))
        .count();
    let root = leading.min(2);
    let mut value = vec![BACKSLASH; root];
    for unit in original.iter().skip(leading).copied() {
        if !is_separator(unit) {
            value.push(unit);
        } else if value.last() != Some(&BACKSLASH) {
            value.push(BACKSLASH);
        }
    }
    while value.len() > root && value.last() == Some(&BACKSLASH) {
        value.pop();
    }
    if value.last() == Some(&COLON) {
        value.push(BACKSLASH);
    }
    value
}

/// Rejects `.` and `..` components.
///
/// The verbatim namespace resolves neither, so prefixing such a spelling names a literal directory
/// that does not exist. Resolving them here would have to guess whether an intervening component
/// is a link, so the composition root that owns the profile path resolves them instead.
fn require_resolved_components(body: &[u16]) -> io::Result<Vec<u16>> {
    const BACKSLASH: u16 = b'\\' as u16;
    const DOT: u16 = b'.' as u16;

    if body
        .split(|unit| *unit == BACKSLASH)
        .any(|component| matches!(component, [DOT] | [DOT, DOT]))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows vault path has a relative component the verbatim namespace cannot resolve",
        ));
    }
    Ok(body.to_vec())
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
    // ERROR_INVALID_FUNCTION, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE. Because the durable
    // handle now requests GENERIC_WRITE, ERROR_ACCESS_DENIED means the host refused a writable
    // directory handle rather than that a readable one cannot be flushed.
    matches!(error.raw_os_error(), Some(1 | 5 | 6))
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        ffi::OsStr,
        io::Read as _,
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

    fn verbatim_text(path: &str) -> io::Result<String> {
        Ok(String::from_utf16_lossy(&wide_path_without_terminator(
            Path::new(path),
        )?))
    }

    fn nt_text(path: &str) -> io::Result<String> {
        Ok(String::from_utf16_lossy(&wide_nt_path(Path::new(path))?))
    }

    fn respell_with_forward_slashes(path: &Path) -> PathBuf {
        PathBuf::from(path.to_string_lossy().replace('\\', "/"))
    }

    /// The Win32 verbatim namespace performs no separator translation, so a caller spelling has
    /// to be rewritten before `\\?\` is applied. Every input here is an ordinary Windows spelling
    /// that configuration text, a command-line argument, or string concatenation produces.
    #[test]
    fn verbatim_prefix_normalizes_caller_separators() -> io::Result<()> {
        let cases = [
            (
                "C:/profile/.vault/objects",
                r"\\?\C:\profile\.vault\objects",
            ),
            (
                r"C:\profile/.vault\objects",
                r"\\?\C:\profile\.vault\objects",
            ),
            ("C://profile///objects", r"\\?\C:\profile\objects"),
            ("C:/profile/objects/", r"\\?\C:\profile\objects"),
            ("C:/", r"\\?\C:\"),
            (r"C:\", r"\\?\C:\"),
            ("//server/share/profile", r"\\?\UNC\server\share\profile"),
            ("//server/share/", r"\\?\UNC\server\share"),
            ("//?/C:/profile/objects", r"\\?\C:\profile\objects"),
            (
                r"\\?\UNC\server\share\profile",
                r"\\?\UNC\server\share\profile",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                verbatim_text(input)?,
                expected,
                "verbatim spelling of {input}"
            );
        }
        Ok(())
    }

    /// The rewrite must leave the canonical backslash spellings every existing caller supplies
    /// byte-for-byte unchanged, including the relative fallback Win32 resolves for itself.
    #[test]
    fn verbatim_prefix_preserves_canonical_backslash_spellings() -> io::Result<()> {
        let cases = [
            (
                r"C:\Users\profile\.vault\objects\ab\cd.obj",
                r"\\?\C:\Users\profile\.vault\objects\ab\cd.obj",
            ),
            (
                r"\\server\share\profile\.vault",
                r"\\?\UNC\server\share\profile\.vault",
            ),
            (r"\\?\C:\already\verbatim", r"\\?\C:\already\verbatim"),
            (r"\\?\UNC\server\share", r"\\?\UNC\server\share"),
            (r"relative\tail", r"relative\tail"),
            ("C:relative", "C:relative"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                verbatim_text(input)?,
                expected,
                "verbatim spelling of {input}"
            );
        }
        Ok(())
    }

    /// The NT spelling handed to `FileRenameInfoEx` shares the same namespace rule.
    #[test]
    fn handle_rename_path_normalizes_caller_separators() -> io::Result<()> {
        let cases = [
            (
                "C:/profile/objects/cd.obj",
                r"\??\C:\profile\objects\cd.obj",
            ),
            (
                r"C:\profile\objects\cd.obj",
                r"\??\C:\profile\objects\cd.obj",
            ),
            (
                r"C:\profile/objects\cd.obj",
                r"\??\C:\profile\objects\cd.obj",
            ),
            ("//server/share/profile", r"\??\UNC\server\share\profile"),
            (r"\\server\share\profile", r"\??\UNC\server\share\profile"),
            (r"\\?\C:\already\verbatim", r"\??\C:\already\verbatim"),
        ];
        for (input, expected) in cases {
            assert_eq!(nt_text(input)?, expected, "NT spelling of {input}");
        }
        assert!(
            matches!(wide_nt_path(Path::new(r"relative\tail")), Err(error)
                if error.kind() == io::ErrorKind::InvalidInput),
            "a relative rename destination has no NT spelling"
        );
        Ok(())
    }

    /// A spelling the verbatim namespace cannot express must fail here, with the vault's own
    /// input error, rather than downstream with an unrelated operating-system code.
    #[test]
    fn verbatim_prefix_rejects_spellings_it_cannot_express() {
        for input in [
            r"C:\profile\..\escape",
            "C:/profile/./same",
            r"\\.\PhysicalDrive0",
            "//./PhysicalDrive0",
        ] {
            assert!(
                matches!(verbatim_path(Path::new(input)), Err(error)
                    if error.kind() == io::ErrorKind::InvalidInput),
                "the verbatim namespace cannot express {input}"
            );
        }

        let interior_nul =
            OsString::from_wide(&[b'C' as u16, b':' as u16, b'\\' as u16, 0, b'a' as u16]);
        assert!(
            matches!(verbatim_path(Path::new(OsStr::new(&interior_nul))), Err(error)
                if error.kind() == io::ErrorKind::InvalidInput),
            "an interior NUL has no verbatim spelling"
        );
    }

    /// A forward-slash profile root is an ordinary Windows spelling, so every verbatim-namespace
    /// operation must reach the same objects a backslash spelling reaches.
    #[test]
    fn native_forward_slash_spelling_reaches_vault_operations() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let respelled = respell_with_forward_slashes(&directory.0);

        let lease_path = respelled.join("object.lease");
        let held = try_acquire_shared_object_lease(&lease_path)?;
        assert!(
            held.is_some(),
            "a forward-slash lease path must reach the lease namespace"
        );
        drop(held);

        let source = respelled.join("source.partial");
        let destination = respelled.join("destination.obj");
        let mut temp = create_locked_temp(&source)?;
        temp.file_mut().write_all(b"forward slash bytes")?;
        temp.file_mut().sync_all()?;
        assert!(publish_locked_no_replace(&mut temp, &source, &destination)?);
        sync_directory(&respelled)?;
        drop(temp);

        assert!(symlink_metadata_no_follow(&destination)?.is_file());
        let mut bytes = Vec::new();
        open_readonly_no_follow(&destination)?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"forward slash bytes");

        assert!(directory.0.join("destination.obj").is_file());
        Ok(())
    }

    /// Beyond the legacy limit the verbatim prefix is the only thing that makes the path usable,
    /// so the rewrite must normalise separators without losing it.
    #[test]
    fn native_long_forward_slash_path_publishes_through_the_verbatim_prefix() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let mut deep = directory.0.clone();
        for index in 0_u32..4 {
            deep.push(format!("{index}{}", "d".repeat(79)));
            fs::create_dir(verbatim_path(&deep)?)?;
        }
        let respelled = respell_with_forward_slashes(&deep);
        assert!(
            respelled.as_os_str().len() > 260,
            "the long-path case requires a spelling past the legacy limit"
        );

        let source = respelled.join("source.partial");
        let destination = respelled.join("destination.obj");
        let mut temp = create_locked_temp(&source)?;
        temp.file_mut().write_all(b"long path bytes")?;
        temp.file_mut().sync_all()?;
        assert!(publish_locked_no_replace(&mut temp, &source, &destination)?);
        drop(temp);

        let mut bytes = Vec::new();
        open_readonly_no_follow(&destination)?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"long path bytes");

        fs::remove_dir_all(verbatim_path(&directory.0)?)?;
        Ok(())
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
    fn native_directory_flush_requires_a_writable_handle() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let readonly = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_WRITE_THROUGH)
            .open(&directory.0)?;
        let refused = flush_handle(&readonly)
            .err()
            .and_then(|error| error.raw_os_error());
        assert_eq!(
            refused,
            Some(i32::try_from(ERROR_ACCESS_DENIED).unwrap_or(5)),
            "a read-only directory handle cannot flush, so the durable flush must request write"
        );
        drop(readonly);

        flush_directory(&directory.0)?;
        Ok(())
    }

    #[test]
    fn native_directory_sync_tolerates_a_concurrently_open_barrier() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        sync_directory(&directory.0)?;
        let held = open_directory_barrier(&directory.0.join(DIRECTORY_BARRIER_FILE))?;
        sync_directory(&directory.0)?;
        drop(held);
        Ok(())
    }

    #[test]
    fn native_lease_creation_tolerates_a_live_shared_lease() -> io::Result<()> {
        let directory = TestDirectory::create()?;
        let lease_path = directory.0.join("object.lease");
        let held = try_acquire_shared_object_lease(&lease_path)?;
        assert!(held.is_some(), "the first shared lease must be granted");

        let created = open_lease_file_for_creation(&verbatim_path(&lease_path)?)?;
        require_regular_non_reparse_handle(&created)?;
        drop(created);
        drop(held);
        Ok(())
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
