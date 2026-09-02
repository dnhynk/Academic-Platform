//! The Linux backend: `setrlimit`, Landlock, and a seccomp filter.
//!
//! All three are self-applied and unprivileged, so the whole backend runs in
//! the child, at the top of its `main`, before it has read a job byte. The
//! parent owns only the wall clock and the reaping.
//!
//! # Why this order
//!
//! `setrlimit` first, because it is the only step that still needs a
//! `prlimit`-class syscall the filter later denies to nobody in particular but
//! whose absence would be confusing to debug. Landlock second, because it needs
//! `openat` on the three staged directories and the filter does not deny
//! `openat`. `PR_SET_NO_NEW_PRIVS` before either restriction is installed,
//! because Landlock requires it and seccomp requires it, and installing it once
//! ahead of both is one fact rather than two. The seccomp filter last, because
//! it denies the `landlock_*` syscalls the step before it needs.
//!
//! Each step is checked before the next is attempted. There is no path that
//! reports success with a filter that did not install.
//!
//! # `libc::SYS_socket` in this file
//!
//! This file names the socket syscalls because it is the file that *refuses*
//! them. `only_egress_crate_has_a_socket` reads it, its allowance is those
//! exact spellings, and the allowance map is compared whole, so a spelling
//! added or removed here fails that scan until the table is edited in the same
//! commit.

use std::{
    ffi::CString,
    io,
    os::unix::ffi::OsStrExt as _,
    path::Path,
    time::{Duration, Instant},
};

use super::{Availability, BackendId, LaunchSpec, SandboxError, SandboxUnavailable};
use crate::{
    capability::CapabilityDescriptor,
    job::ProbeReport,
    receipt::{LimitKind, ResourceReceipt, RunOutcome},
};

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

const ACCESS_WRITE_FILE: u64 = 1 << 1;
const ACCESS_READ_FILE: u64 = 1 << 2;
const ACCESS_READ_DIR: u64 = 1 << 3;
const ACCESS_REMOVE_DIR: u64 = 1 << 4;
const ACCESS_REMOVE_FILE: u64 = 1 << 5;
const ACCESS_MAKE_DIR: u64 = 1 << 7;
const ACCESS_MAKE_REG: u64 = 1 << 8;
const ACCESS_REFER: u64 = 1 << 13;
const ACCESS_TRUNCATE: u64 = 1 << 14;
const ACCESS_IOCTL_DEV: u64 = 1 << 15;

/// Every filesystem access class Landlock ABI 1 knows, which is the set a
/// ruleset must handle for the default to be "refused".
const ABI1_HANDLED: u64 = (1 << 13) - 1;

const SECCOMP_SET_MODE_FILTER: u32 = 1;
const SECCOMP_GET_ACTION_AVAIL: u32 = 2;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xc000_003e;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xc000_00b7;

const BPF_LD: u16 = 0x00;
const BPF_JMP: u16 = 0x05;
const BPF_RET: u16 = 0x06;
const BPF_W: u16 = 0x00;
const BPF_ABS: u16 = 0x20;
const BPF_JEQ: u16 = 0x10;
const BPF_K: u16 = 0x00;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
#[derive(Debug)]
struct SockFprog {
    len: u16,
    filter: *const SockFilter,
}

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

const fn stmt(code: u16, k: u32) -> SockFilter {
    SockFilter {
        code,
        jt: 0,
        jf: 0,
        k,
    }
}

const fn jump(code: u16, k: u32, jt: u8, jf: u8) -> SockFilter {
    SockFilter { code, jt, jf, k }
}

/// The syscalls a contained job may not make.
///
/// Three groups: everything that reaches a socket, everything that creates a
/// process, and everything that would let the job take the sandbox back off.
/// `libc::SYS_socketcall` exists only on 32-bit targets and is absent here on
/// purpose: this backend is measured on 64-bit and the multiplexed entry point
/// does not exist on it.
fn denied_syscalls() -> Vec<i64> {
    // `fork` and `vfork` are separate entry points only where the architecture
    // has them. AArch64 has neither: glibc reaches both through `clone`, which
    // is denied below, so the refusal is the same one either way and this list
    // is complete on both. Naming them unconditionally is a compile error on
    // `aarch64-unknown-linux-gnu`, which is how this was found — the hosted
    // `rust-ubuntu-24.04-arm` job refused the lint step.
    #[cfg(target_arch = "x86_64")]
    let legacy_process_creation = vec![libc::SYS_fork, libc::SYS_vfork];
    #[cfg(not(target_arch = "x86_64"))]
    let legacy_process_creation: Vec<i64> = Vec::new();

    let mut denied = vec![
        // Sockets.
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_bind,
        libc::SYS_listen,
        libc::SYS_accept4,
        libc::SYS_sendto,
        libc::SYS_recvfrom,
        libc::SYS_sendmsg,
        libc::SYS_recvmsg,
        // Process creation. With only `clone` and `clone3` denied,
        // `std::process::Command` still forked on x86-64 and its child was
        // refused at `execve` by Landlock instead — a refusal one layer later
        // than this filter intends. `legacy_process_creation` above is what
        // closes that, per architecture.
        libc::SYS_clone,
        libc::SYS_clone3,
        libc::SYS_execve,
        libc::SYS_execveat,
        // Taking the sandbox back off, or reaching into another process.
        libc::SYS_ptrace,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_mount,
        libc::SYS_pivot_root,
        libc::SYS_chroot,
        libc::SYS_seccomp,
        libc::SYS_landlock_create_ruleset,
        libc::SYS_landlock_add_rule,
        libc::SYS_landlock_restrict_self,
    ];
    denied.extend(legacy_process_creation);
    denied
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

/// Whether this kernel supports both halves of the backend.
#[allow(unsafe_code)]
pub(super) fn availability() -> Availability {
    let abi = landlock_abi();
    if abi < 1 {
        return Availability::Unavailable(SandboxUnavailable {
            backend: BackendId::LinuxSeccompLandlock,
            reason: format!(
                "landlock_create_ruleset version query returned {abi} (errno {}); \
                 this kernel has no Landlock, so the filesystem half cannot run",
                errno()
            ),
        });
    }
    let action = SECCOMP_RET_ERRNO;
    // SAFETY: `SECCOMP_GET_ACTION_AVAIL` reads one `u32` through the pointer and
    // writes nothing; the pointer is to a live local.
    let available = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            SECCOMP_GET_ACTION_AVAIL,
            0_u32,
            &raw const action,
        )
    };
    if available != 0 {
        return Availability::Unavailable(SandboxUnavailable {
            backend: BackendId::LinuxSeccompLandlock,
            reason: format!(
                "seccomp(SECCOMP_GET_ACTION_AVAIL, SECCOMP_RET_ERRNO) returned \
                 {available} (errno {}); this kernel cannot install the filter",
                errno()
            ),
        });
    }
    Availability::Available(BackendId::LinuxSeccompLandlock)
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

#[allow(unsafe_code)]
fn open_path(path: &Path) -> Result<i32, SandboxError> {
    let raw = CString::new(path.as_os_str().as_bytes()).map_err(|_| SandboxError::StagedPath {
        path: path.to_path_buf(),
        detail: String::from("path contains an interior NUL"),
    })?;
    // SAFETY: `raw` is a live NUL-terminated C string for the duration of the
    // call, and `O_PATH` opens a reference without reading the file.
    let fd = unsafe { libc::open(raw.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(SandboxError::StagedPath {
            path: path.to_path_buf(),
            detail: io::Error::last_os_error().to_string(),
        });
    }
    Ok(fd)
}

#[allow(unsafe_code)]
fn add_rule(ruleset: i32, path: &Path, allowed: u64) -> Result<(), SandboxError> {
    let fd = open_path(path)?;
    let attr = LandlockPathBeneathAttr {
        allowed_access: allowed,
        parent_fd: fd,
    };
    // SAFETY: `attr` is a live, correctly shaped `landlock_path_beneath_attr`
    // and the ruleset descriptor is one this function's caller just created.
    let result = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset,
            LANDLOCK_RULE_PATH_BENEATH,
            &raw const attr,
            0_u32,
        )
    };
    // SAFETY: `fd` came from `open_path` and is not used again.
    unsafe { libc::close(fd) };
    if result != 0 {
        return Err(SandboxError::Syscall {
            step: "landlock_add_rule",
            code: errno(),
        });
    }
    Ok(())
}

/// Sets one resource limit.
///
/// The soft and hard bounds are separate because `RLIMIT_CPU` uses both: the
/// kernel raises `SIGXCPU` when the soft bound is passed and `SIGKILL` when the
/// hard one is, and a run with them equal is killed outright with no signal
/// that says why. One second of headroom on the hard bound is what makes the
/// receipt able to say `Cpu` rather than an anonymous kill.
#[allow(unsafe_code)]
fn set_limit(resource: u32, soft: u64, hard: u64) -> Result<(), SandboxError> {
    let limit = libc::rlimit {
        rlim_cur: soft,
        rlim_max: hard,
    };
    // SAFETY: `limit` is a live `rlimit` and `resource` is one of the constants
    // below, all of which `setrlimit` accepts.
    let result = unsafe { libc::setrlimit(resource, &raw const limit) };
    if result != 0 {
        return Err(SandboxError::Syscall {
            step: "setrlimit",
            code: errno(),
        });
    }
    Ok(())
}

/// Applies every restriction to the calling process.
#[allow(unsafe_code)]
pub(super) fn enter(
    descriptor: &CapabilityDescriptor,
    report_dir: &Path,
) -> Result<BackendId, SandboxError> {
    let abi = landlock_abi();
    if abi < 1 {
        return Err(SandboxError::Unavailable(SandboxUnavailable {
            backend: BackendId::LinuxSeccompLandlock,
            reason: format!("landlock version query returned {abi}"),
        }));
    }
    let limits = descriptor.limits();

    // 1. Resource limits. CPU is whole seconds at the kernel boundary, and a
    //    bound under one second rounds up rather than down to zero, which would
    //    be no bound at all.
    let cpu_seconds = limits.cpu_millis().div_ceil(1_000).max(1);
    set_limit(libc::RLIMIT_CPU, cpu_seconds, cpu_seconds.saturating_add(1))?;
    set_limit(
        libc::RLIMIT_AS,
        limits.memory_bytes(),
        limits.memory_bytes(),
    )?;
    set_limit(
        libc::RLIMIT_FSIZE,
        limits.output_bytes(),
        limits.output_bytes(),
    )?;
    set_limit(libc::RLIMIT_CORE, 0, 0)?;

    // 2. `no_new_privs`, which both remaining steps require.
    // SAFETY: `PR_SET_NO_NEW_PRIVS` takes scalars and touches no user memory.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(SandboxError::Syscall {
            step: "prctl(PR_SET_NO_NEW_PRIVS)",
            code: errno(),
        });
    }

    // 3. Landlock. Everything is refused except the three staged directories.
    let attr = LandlockRulesetAttr {
        handled_access_fs: handled_mask(abi),
    };
    // SAFETY: `attr` is a live, correctly shaped `landlock_ruleset_attr` whose
    // size is passed alongside it.
    let ruleset = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &raw const attr,
            size_of::<LandlockRulesetAttr>(),
            0_u32,
        )
    };
    let ruleset = i32::try_from(ruleset).unwrap_or(-1);
    if ruleset < 0 {
        return Err(SandboxError::Syscall {
            step: "landlock_create_ruleset",
            code: errno(),
        });
    }
    let read_only = ACCESS_READ_FILE | ACCESS_READ_DIR;
    let mut writable = read_only
        | ACCESS_WRITE_FILE
        | ACCESS_MAKE_REG
        | ACCESS_MAKE_DIR
        | ACCESS_REMOVE_FILE
        | ACCESS_REMOVE_DIR;
    if abi >= 3 {
        writable |= ACCESS_TRUNCATE;
    }
    add_rule(ruleset, descriptor.staged_input(), read_only)?;
    add_rule(ruleset, descriptor.staged_output(), writable)?;
    add_rule(ruleset, report_dir, writable)?;
    // SAFETY: `ruleset` is the descriptor just created; the call takes no
    // pointer.
    let restricted = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0_u32) };
    // SAFETY: `ruleset` is owned here and not used again.
    unsafe { libc::close(ruleset) };
    if restricted != 0 {
        return Err(SandboxError::Syscall {
            step: "landlock_restrict_self",
            code: errno(),
        });
    }

    // 4. The seccomp filter.
    let denied = denied_syscalls();
    let mut program = vec![
        stmt(BPF_LD | BPF_W | BPF_ABS, 4),
        jump(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH, 1, 0),
        stmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        stmt(BPF_LD | BPF_W | BPF_ABS, 0),
    ];
    let count = denied.len();
    for (index, number) in denied.iter().enumerate() {
        let remaining = u8::try_from(count - index - 1).unwrap_or(u8::MAX);
        let Some(jt) = remaining.checked_add(1) else {
            return Err(SandboxError::Syscall {
                step: "seccomp filter assembly",
                code: -1,
            });
        };
        let Ok(k) = u32::try_from(*number) else {
            return Err(SandboxError::Syscall {
                step: "seccomp filter assembly",
                code: -1,
            });
        };
        program.push(jump(BPF_JMP | BPF_JEQ | BPF_K, k, jt, 0));
    }
    program.push(stmt(BPF_RET | BPF_K, SECCOMP_RET_ALLOW));
    program.push(stmt(
        BPF_RET | BPF_K,
        SECCOMP_RET_ERRNO | u32::try_from(libc::EPERM).unwrap_or(1),
    ));
    let Ok(len) = u16::try_from(program.len()) else {
        return Err(SandboxError::Syscall {
            step: "seccomp filter assembly",
            code: -1,
        });
    };
    let fprog = SockFprog {
        len,
        filter: program.as_ptr(),
    };
    // SAFETY: `program` outlives the call and `fprog` points into it with a
    // matching length; the kernel copies the filter before returning.
    let installed = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            SECCOMP_SET_MODE_FILTER,
            0_u32,
            &raw const fprog,
        )
    };
    if installed != 0 {
        return Err(SandboxError::Syscall {
            step: "seccomp(SECCOMP_SET_MODE_FILTER)",
            code: errno(),
        });
    }
    drop(program);
    Ok(BackendId::LinuxSeccompLandlock)
}

/// Launches the probe, waits for it against the wall bound, and measures it.
#[allow(unsafe_code)]
pub(super) fn launch(spec: &LaunchSpec) -> Result<(ResourceReceipt, ProbeReport), SandboxError> {
    if let Availability::Unavailable(unavailable) = availability() {
        return Err(SandboxError::Unavailable(unavailable));
    }
    super::write_job_inputs(spec)?;
    let started = Instant::now();
    let child = std::process::Command::new(&spec.program)
        .arg("run")
        .env_clear()
        .env(super::INPUT_DIR_VAR, spec.plan.descriptor.staged_input())
        .env(super::REPORT_DIR_VAR, &spec.report_dir)
        .env(super::HOME_CANARY_VAR, &spec.plan.home_canary)
        .env(super::VAULT_CANARY_VAR, &spec.plan.vault_canary)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| SandboxError::Launch {
            path: spec.program.clone(),
            detail: error.to_string(),
        })?;
    let pid = i32::try_from(child.id()).unwrap_or(-1);
    let deadline = Duration::from_millis(spec.plan.descriptor.limits().wall_millis());

    let mut status = 0_i32;
    // SAFETY: `usage` is written by `wait4` and read only after it reports a
    // reaped child.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let mut reaped = false;
    let mut killed_for_wall = false;
    loop {
        // SAFETY: `pid` is this process's own child and both out-pointers are
        // live locals.
        let result = unsafe { libc::wait4(pid, &raw mut status, libc::WNOHANG, &raw mut usage) };
        if result == pid {
            reaped = true;
            break;
        }
        if result < 0 {
            break;
        }
        if started.elapsed() >= deadline {
            killed_for_wall = true;
            // SAFETY: `pid` is this process's own child, not yet reaped.
            unsafe { libc::kill(pid, libc::SIGKILL) };
            // SAFETY: the same child, now blocking until it is reaped.
            let final_result = unsafe { libc::wait4(pid, &raw mut status, 0, &raw mut usage) };
            reaped = final_result == pid;
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let wall_millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    drop(child);

    let cpu_millis = if reaped {
        rusage_millis(&usage.ru_utime) + rusage_millis(&usage.ru_stime)
    } else {
        0
    };
    // `ru_maxrss` is kilobytes on Linux.
    let peak_memory_bytes = if reaped {
        u64::try_from(usage.ru_maxrss)
            .unwrap_or(0)
            .saturating_mul(1024)
    } else {
        0
    };
    let output_bytes = super::staged_output_bytes(&spec.plan.descriptor)?;
    let limits = *spec.plan.descriptor.limits();
    let outcome = if killed_for_wall {
        RunOutcome::KilledByLimit(LimitKind::WallTime)
    } else if !reaped {
        RunOutcome::NotStarted {
            detail: String::from("the child could not be reaped"),
        }
    } else if libc::WIFSIGNALED(status) {
        match libc::WTERMSIG(status) {
            libc::SIGXCPU => RunOutcome::KilledByLimit(LimitKind::Cpu),
            libc::SIGXFSZ => RunOutcome::KilledByLimit(LimitKind::OutputBytes),
            signal => RunOutcome::Failed {
                exit_code: i64::from(-signal),
            },
        }
    } else if libc::WEXITSTATUS(status) == 0 {
        RunOutcome::Completed
    } else {
        RunOutcome::Failed {
            exit_code: i64::from(libc::WEXITSTATUS(status)),
        }
    };
    let outcome = super::apply_output_bound(outcome, output_bytes, &limits);
    let report = super::read_report(&spec.report_dir);
    Ok((
        ResourceReceipt::new(
            BackendId::LinuxSeccompLandlock,
            limits,
            cpu_millis,
            peak_memory_bytes,
            wall_millis,
            output_bytes,
            outcome,
        ),
        report,
    ))
}

fn rusage_millis(value: &libc::timeval) -> u64 {
    let seconds = u64::try_from(value.tv_sec).unwrap_or(0);
    let micros = u64::try_from(value.tv_usec).unwrap_or(0);
    seconds.saturating_mul(1_000) + micros / 1_000
}
