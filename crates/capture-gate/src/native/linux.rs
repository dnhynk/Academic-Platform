//! The Linux device layer: a Landlock ruleset whose rules are the granted
//! device trees.
//!
//! # Why Landlock and not a filter
//!
//! A device on Linux is a path. `/dev/snd/pcmC0D0c` is what ALSA opens and
//! `/dev/video0` is what V4L2 opens, so restricting which paths a process may
//! open *is* restricting which devices it may hold. A seccomp filter would have
//! to refuse `openat` outright, which refuses the process's own image as well.
//!
//! # What is installed, and what the split by media is
//!
//! A ruleset handling every access class the kernel's ABI knows -- so the
//! default is "refused" -- and one `path_beneath` rule per device tree whose
//! class the token's [`DeviceRuleset`] permits. A tree the token does not name
//! gets no rule, and a path beneath it is `EACCES`. That is
//! `audio_only_permission_denies_camera` enforced by the kernel rather than by
//! this crate's own comparison.
//!
//! Three rules are not devices and each is here because without it the run
//! measures something else: the program image, or `execve` is refused before
//! the capture binary starts; the report directory, or the run cannot say what
//! the kernel answered; and nothing else at all -- no home, no vault, no
//! working directory.
//!
//! # Why the parent opens the paths
//!
//! The ruleset is installed between `fork` and `exec`, and a `pre_exec` closure
//! must make syscalls and nothing else: the child of a multi-threaded process
//! can deadlock on an allocator lock another thread held at `fork`. So every
//! path is resolved to an `O_PATH` descriptor **in the parent**, with its access
//! mask decided there, and the closure iterates a list of descriptors. It
//! allocates nothing.
//!
//! # Directory rights are not file rights
//!
//! `landlock_add_rule` refuses `EINVAL` when a rule over a regular file or a
//! device node carries a directory-only right such as `READ_DIR` or `MAKE_REG`.
//! The mask is therefore decided per path from what the path is, which is why
//! [`RuleFd`] carries one.
//!
//! # The three syscalls this file makes
//!
//! `landlock_create_ruleset`, `landlock_add_rule` and
//! `landlock_restrict_self`. Those three `SYS_` names are the whole list, they
//! are enumerated in `only_egress_crate_has_a_socket`'s allowance for this
//! file, and every `libc::syscall(` call below names one of them as its first
//! argument. A number, or a fourth name, fails that scan.

use std::{
    ffi::CString,
    io,
    os::unix::{ffi::OsStrExt as _, process::CommandExt as _},
    path::Path,
    process::Command,
};

use super::{LaunchSpec, NativeError, REPORT_DIR_VAR, REPORT_FILE};
use crate::device::{BackendId, DeviceLayer};

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

const ACCESS_EXECUTE: u64 = 1 << 0;
const ACCESS_WRITE_FILE: u64 = 1 << 1;
const ACCESS_READ_FILE: u64 = 1 << 2;
const ACCESS_READ_DIR: u64 = 1 << 3;
const ACCESS_MAKE_REG: u64 = 1 << 8;
const ACCESS_REFER: u64 = 1 << 13;
const ACCESS_TRUNCATE: u64 = 1 << 14;
const ACCESS_IOCTL_DEV: u64 = 1 << 15;

/// Every filesystem access class Landlock ABI 1 knows, which is the set a
/// ruleset must handle for the default to be "refused".
const ABI1_HANDLED: u64 = (1 << 13) - 1;

#[repr(C)]
#[derive(Debug, Default)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Where a dynamically linked program's loader and libraries live.
///
/// Read and execute only, and none of them is a device tree. A host that does
/// not have one of these is not a host where the rule is skipped silently: it
/// is skipped because the directory is not there, and the exec either works
/// without it or the launch fails loudly.
const RUNTIME_IMAGE_DIRECTORIES: [&str; 4] = ["/lib", "/lib64", "/usr/lib", "/usr/lib64"];

/// One resolved rule: a descriptor the parent opened and the rights it carries.
#[derive(Debug, Clone, Copy)]
struct RuleFd {
    fd: i32,
    allowed: u64,
}

fn errno() -> i64 {
    io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(-1)
        .into()
}

#[allow(unsafe_code)]
fn landlock_abi() -> i64 {
    // SAFETY: a version query passes a null attribute pointer and a zero size,
    // which is the documented form; it reads and writes no user memory.
    unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<LandlockRulesetAttr>(),
            0_usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    }
}

/// What this kernel can enforce.
pub(super) fn availability() -> DeviceLayer {
    if landlock_abi() < 1 {
        return DeviceLayer::Unavailable;
    }
    DeviceLayer::Enforced(BackendId::LinuxLandlock)
}

fn handled_mask(abi: i64) -> u64 {
    let mut mask = ABI1_HANDLED;
    if abi >= 2 {
        mask |= ACCESS_REFER;
    }
    if abi >= 3 {
        mask |= ACCESS_TRUNCATE;
    }
    if abi >= 5 {
        mask |= ACCESS_IOCTL_DEV;
    }
    mask
}

/// Resolves one path to a descriptor and the rights a rule over it may carry.
///
/// Runs in the parent. `writable` is the report directory's extra rights; a
/// device tree takes none of them.
#[allow(unsafe_code)]
fn resolve(path: &Path, abi: i64, writable: bool, executable: bool) -> Result<RuleFd, NativeError> {
    let raw = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| NativeError::Path(format!("{} contains an interior NUL", path.display())))?;
    // SAFETY: `raw` is a live NUL-terminated C string for the duration of the
    // call, and `O_PATH` opens a reference without reading the file.
    let fd = unsafe { libc::open(raw.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(NativeError::Syscall {
            step: "open(O_PATH)",
            code: errno(),
        });
    }
    let is_directory = path.is_dir();
    let mut allowed = ACCESS_READ_FILE;
    if executable {
        allowed |= ACCESS_EXECUTE;
    }
    if writable {
        allowed |= ACCESS_WRITE_FILE;
        if abi >= 3 {
            allowed |= ACCESS_TRUNCATE;
        }
    }
    // The directory-only rights. A rule over a regular file or a device node
    // that carries one of these is refused `EINVAL` by the kernel, which is why
    // the mask is decided from what the path is rather than from one constant.
    if is_directory {
        allowed |= ACCESS_READ_DIR;
        if writable {
            allowed |= ACCESS_MAKE_REG;
        }
    }
    Ok(RuleFd { fd, allowed })
}

/// Installs the ruleset on the calling process.
///
/// Called between `fork` and `exec`. Every call it makes is a syscall on a
/// descriptor the parent already opened; it allocates nothing and it reads no
/// path.
#[allow(unsafe_code)]
fn enter(rules: &[RuleFd], handled: u64) -> Result<(), NativeError> {
    // `landlock_restrict_self` refuses with `EPERM` unless the caller has set
    // `no_new_privs` first, so this is not a hardening extra: without it the
    // whole ruleset below is never applied.
    // SAFETY: `PR_SET_NO_NEW_PRIVS` takes scalars and touches no user memory.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(NativeError::Syscall {
            step: "prctl(PR_SET_NO_NEW_PRIVS)",
            code: errno(),
        });
    }
    let attr = LandlockRulesetAttr {
        handled_access_fs: handled,
    };
    // SAFETY: `attr` is a live, correctly shaped `landlock_ruleset_attr` whose
    // size is passed alongside it.
    let descriptor = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &raw const attr,
            size_of::<LandlockRulesetAttr>(),
            0_u32,
        )
    };
    let descriptor = i32::try_from(descriptor).unwrap_or(-1);
    if descriptor < 0 {
        return Err(NativeError::Syscall {
            step: "landlock_create_ruleset",
            code: errno(),
        });
    }
    for rule in rules {
        let beneath = LandlockPathBeneathAttr {
            allowed_access: rule.allowed,
            parent_fd: rule.fd,
        };
        // SAFETY: `beneath` is a live, correctly shaped
        // `landlock_path_beneath_attr` and both descriptors are live.
        let added = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                descriptor,
                LANDLOCK_RULE_PATH_BENEATH,
                &raw const beneath,
                0_u32,
            )
        };
        if added != 0 {
            return Err(NativeError::Syscall {
                step: "landlock_add_rule",
                code: errno(),
            });
        }
    }
    // SAFETY: `descriptor` is the ruleset just created; the call takes no
    // pointer.
    let restricted = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, descriptor, 0_u32) };
    if restricted != 0 {
        return Err(NativeError::Syscall {
            step: "landlock_restrict_self",
            code: errno(),
        });
    }
    Ok(())
}

/// Runs the probe under the ruleset and returns what it wrote.
///
/// The restriction is installed between `fork` and `exec`, so the probe image
/// never runs unrestricted and no argument to it can widen what it reaches.
///
/// The streams are inherited rather than redirected to the null device. That is
/// `academic-worker`'s lesson: a probe that opens `/dev/null` under a ruleset
/// that does not name it is refused for the redirection rather than for the
/// operation being measured, and the run is green and measures nothing.
#[allow(unsafe_code)]
pub(super) fn launch(spec: &LaunchSpec) -> Result<String, NativeError> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.targets)
        .env(REPORT_DIR_VAR, &spec.report_dir);
    if spec.contained {
        let abi = landlock_abi();
        if abi < 1 {
            return Err(NativeError::Unavailable(format!(
                "landlock version query returned {abi}"
            )));
        }
        let handled = handled_mask(abi);
        let mut rules = vec![
            resolve(&spec.program, abi, false, true)?,
            resolve(&spec.report_dir, abi, true, false)?,
        ];
        // The process's own runtime image. `execve` reads the program, and the
        // program reads the dynamic loader and the shared libraries it is
        // linked against; without these the run is refused `EACCES` before the
        // capture binary starts and every device row measures a failed exec
        // instead of a refused device.
        //
        // None of these is a device tree, and `/dev` is under none of them, so
        // granting them widens nothing this contract claims. They are listed
        // rather than derived so the set is reviewable, and a directory that is
        // absent on the host is skipped rather than failing the launch.
        for directory in RUNTIME_IMAGE_DIRECTORIES {
            let path = Path::new(directory);
            if path.is_dir() {
                rules.push(resolve(path, abi, false, true)?);
            }
        }
        for tree in &spec.trees {
            // No ruleset is no token, and no token grants no class. The `None`
            // arm is not a default that permits: it is the absence of a token,
            // and it adds no rule at all.
            if spec.permits(tree.class()) {
                rules.push(resolve(tree.path(), abi, false, false)?);
            }
        }
        // SAFETY: the closure runs between `fork` and `exec` in the child. Every
        // call it makes is a syscall on a descriptor this function already
        // opened, so it takes no allocator lock and reads no parent state.
        unsafe {
            command.pre_exec(move || {
                // The errno is carried out rather than a message: the child
                // sends the parent a raw OS error code and nothing else, so an
                // `io::Error::other` here reaches the caller as a bare `EINVAL`
                // and says which step failed to nobody.
                enter(&rules, handled).map_err(|error| match error {
                    NativeError::Syscall { code, .. } => {
                        io::Error::from_raw_os_error(i32::try_from(code).unwrap_or(libc::EINVAL))
                    }
                    other => io::Error::other(other.to_string()),
                })
            });
        }
    }
    let status = command
        .status()
        .map_err(|error| NativeError::Path(format!("probe would not start: {error}")))?;
    if !status.success() {
        return Err(NativeError::Path(format!(
            "probe exited with {status}, report may be incomplete"
        )));
    }
    std::fs::read_to_string(spec.report_dir.join(REPORT_FILE))
        .map_err(|error| NativeError::Path(format!("report unreadable: {error}")))
}
