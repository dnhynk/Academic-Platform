//! The Linux backend: a Landlock ruleset with no rule, a seccomp filter over
//! the socket family, and then the kernel's own answer about both.
//!
//! # Why this shape
//!
//! Both restrictions are self-applied and unprivileged, so the whole backend
//! runs inside the process it restricts, at the top of `main`, before any work.
//!
//! `PR_SET_NO_NEW_PRIVS` first, because Landlock requires it and seccomp
//! requires it. Landlock second, because it needs `openat` on `/` to create its
//! ruleset descriptor and the filter does not deny `openat`. The seccomp filter
//! last, because installing it before Landlock would deny the `landlock_*`
//! syscalls the step needs.
//!
//! Each step is checked before the next is attempted; there is no path that
//! reports success with a restriction that did not install.
//!
//! # The Landlock ruleset has no rule, on purpose
//!
//! `WriteStagedArtifact` is refused by handling every write-shaped access class
//! and then granting none of them, which makes the whole filesystem read-only
//! for this process. `handled_access_fs` deliberately excludes `EXECUTE`,
//! `READ_FILE` and `READ_DIR`: the capability being refused is the write, and a
//! ruleset that also refused reads would be enforcing something the class never
//! declared — including the read of `/proc/self/status` this file makes to
//! verify itself.
//!
//! # `libc::SYS_socket` in this file
//!
//! This file names the socket syscalls because it is the file that *refuses*
//! them. `only_egress_crate_has_a_socket` reads it, its allowance is those
//! exact spellings, and the allowance map is compared whole, so a spelling
//! added or removed here fails that scan until the table is edited in the same
//! commit. Every `SYS_` name in this file is either inside
//! [`denied_syscalls`] or one of the three this backend installs *with*.
//!
//! # Verification is the kernel's answer, not this file's
//!
//! A syscall that returned zero is not evidence that a restriction is in force.
//! After the installation, `/proc/self/status` is read back for `NoNewPrivs`
//! and `Seccomp`, and — when the write refusal was installed — `/dev/null` is
//! opened for writing and required to fail. Opening `/dev/null` creates
//! nothing, and a success there means the ruleset did not take, which is a
//! [`EnforcementError::NotVerified`] and therefore a refusal to start.

use std::{fs, io};

use academic_policy::ProcessCapability;

use crate::EnforcementError;

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1;

const ACCESS_WRITE_FILE: u64 = 1 << 1;
const ACCESS_REMOVE_DIR: u64 = 1 << 4;
const ACCESS_REMOVE_FILE: u64 = 1 << 5;
const ACCESS_MAKE_CHAR: u64 = 1 << 6;
const ACCESS_MAKE_DIR: u64 = 1 << 7;
const ACCESS_MAKE_REG: u64 = 1 << 8;
const ACCESS_MAKE_SOCK: u64 = 1 << 9;
const ACCESS_MAKE_FIFO: u64 = 1 << 10;
const ACCESS_MAKE_BLOCK: u64 = 1 << 11;
const ACCESS_MAKE_SYM: u64 = 1 << 12;
const ACCESS_REFER: u64 = 1 << 13;
const ACCESS_TRUNCATE: u64 = 1 << 14;

/// Every write-shaped access class Landlock ABI 1 knows.
///
/// `EXECUTE`, `READ_FILE` and `READ_DIR` are deliberately absent: see the
/// module documentation.
const ABI1_WRITE_HANDLED: u64 = ACCESS_WRITE_FILE
    | ACCESS_REMOVE_DIR
    | ACCESS_REMOVE_FILE
    | ACCESS_MAKE_CHAR
    | ACCESS_MAKE_DIR
    | ACCESS_MAKE_REG
    | ACCESS_MAKE_SOCK
    | ACCESS_MAKE_FIFO
    | ACCESS_MAKE_BLOCK
    | ACCESS_MAKE_SYM;

const SECCOMP_SET_MODE_FILTER: u32 = 1;
const SECCOMP_GET_ACTION_AVAIL: u32 = 2;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;

const BPF_LD: u16 = 0x00;
const BPF_JMP: u16 = 0x05;
const BPF_RET: u16 = 0x06;
const BPF_W: u16 = 0x00;
const BPF_ABS: u16 = 0x20;
const BPF_JEQ: u16 = 0x10;
const BPF_K: u16 = 0x00;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xc000_003e;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xc000_00b7;

#[repr(C)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct SockFprog {
    len: u16,
    filter: *const SockFilter,
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

fn errno() -> i64 {
    io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(-1)
        .into()
}

/// The syscalls a class that does not declare `OpenOutboundSocket` may not
/// make.
///
/// The list is the socket family plus the three `io_uring` entry points.
/// `io_uring` is here because a submission queue performs socket operations
/// without the filter seeing them: refusing the family and leaving the ring
/// open would be a refusal with a documented way around it. Nothing else is
/// here — process creation, `ptrace` and the sandbox syscalls are `P2-G4`'s
/// job-level concerns and this backend refuses exactly the one capability the
/// class does not declare.
fn denied_syscalls() -> Vec<i64> {
    vec![
        // The socket family.
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
        // The submission-queue path around it.
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
    ]
}

/// The Landlock ABI this kernel reports, or a negative number.
#[allow(unsafe_code)]
fn landlock_abi() -> i64 {
    // SAFETY: the version query takes a null pointer and a zero size by
    // contract, and touches no user memory.
    unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<LandlockRulesetAttr>(),
            0_usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    }
}

/// Whether this kernel offers `SECCOMP_RET_ERRNO`.
#[allow(unsafe_code)]
fn seccomp_errno_available() -> i64 {
    let action = SECCOMP_RET_ERRNO;
    // SAFETY: `SECCOMP_GET_ACTION_AVAIL` reads one `u32` through the pointer
    // and writes nothing.
    unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            SECCOMP_GET_ACTION_AVAIL,
            0_u32,
            &raw const action,
        )
    }
}

/// Handles every write-shaped access this ABI knows and grants none of them.
#[allow(unsafe_code)]
fn refuse_every_write(abi: i64) -> Result<(), EnforcementError> {
    let mut handled = ABI1_WRITE_HANDLED;
    if abi >= 2 {
        handled |= ACCESS_REFER;
    }
    if abi >= 3 {
        handled |= ACCESS_TRUNCATE;
    }
    let attr = LandlockRulesetAttr {
        handled_access_fs: handled,
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
        return Err(EnforcementError::Syscall {
            step: "landlock_create_ruleset",
            code: errno(),
        });
    }
    // No `landlock_add_rule` call: a ruleset that grants nothing is what makes
    // every handled access refused everywhere.
    // SAFETY: `ruleset` is the descriptor just created; the call takes no
    // pointer.
    let restricted = unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0_u32) };
    // SAFETY: `ruleset` is owned here and not used again.
    unsafe { libc::close(ruleset) };
    if restricted != 0 {
        return Err(EnforcementError::Syscall {
            step: "landlock_restrict_self",
            code: errno(),
        });
    }
    Ok(())
}

/// Installs the seccomp filter that refuses the socket family.
#[allow(unsafe_code)]
fn refuse_every_socket() -> Result<(), EnforcementError> {
    let available = seccomp_errno_available();
    if available != 0 {
        return Err(EnforcementError::Unavailable {
            reason: format!(
                "seccomp(SECCOMP_GET_ACTION_AVAIL, SECCOMP_RET_ERRNO) returned {available} \
                 (errno {})",
                errno()
            ),
        });
    }
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
            return Err(EnforcementError::Syscall {
                step: "seccomp filter assembly",
                code: -1,
            });
        };
        let Ok(k) = u32::try_from(*number) else {
            return Err(EnforcementError::Syscall {
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
        return Err(EnforcementError::Syscall {
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
        return Err(EnforcementError::Syscall {
            step: "seccomp(SECCOMP_SET_MODE_FILTER)",
            code: errno(),
        });
    }
    drop(program);
    Ok(())
}

/// One `/proc/self/status` field, or `None` when the file does not carry it.
fn status_field(status: &str, name: &str) -> Option<String> {
    status.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key == name).then(|| value.trim().to_owned())
    })
}

/// Asks the kernel whether the refusals are in force.
///
/// Returns the answer as one line for the receipt, or the reason it could not
/// be confirmed.
fn verify(refused_write: bool, refused_socket: bool) -> Result<String, EnforcementError> {
    let status =
        fs::read_to_string("/proc/self/status").map_err(|error| EnforcementError::NotVerified {
            detail: format!("/proc/self/status could not be read: {error}"),
        })?;
    let field = |name: &str| -> Result<String, EnforcementError> {
        status_field(&status, name).ok_or_else(|| EnforcementError::NotVerified {
            detail: format!("/proc/self/status carries no {name} line"),
        })
    };
    let no_new_privs = field("NoNewPrivs")?;
    if no_new_privs != "1" {
        return Err(EnforcementError::NotVerified {
            detail: format!("NoNewPrivs is {no_new_privs}, not 1"),
        });
    }
    let mut answers = vec![format!("NoNewPrivs={no_new_privs}")];
    if refused_socket {
        let mode = field("Seccomp")?;
        if mode != "2" {
            return Err(EnforcementError::NotVerified {
                detail: format!("Seccomp is {mode}, not 2 (SECCOMP_MODE_FILTER)"),
            });
        }
        let filters = field("Seccomp_filters")?;
        if filters == "0" {
            return Err(EnforcementError::NotVerified {
                detail: String::from("Seccomp_filters is 0, so no filter is attached"),
            });
        }
        answers.push(format!("Seccomp={mode}"));
        answers.push(format!("Seccomp_filters={filters}"));
    }
    if refused_write {
        // A negative control the kernel answers, rather than a syscall return
        // value this file chose to trust. Opening `/dev/null` for writing
        // creates nothing; under the ruleset above it must fail.
        match fs::OpenOptions::new().write(true).open("/dev/null") {
            Ok(_handle) => {
                return Err(EnforcementError::NotVerified {
                    detail: String::from(
                        "/dev/null opened for writing, so the Landlock ruleset is not in force",
                    ),
                });
            }
            Err(error) => answers.push(format!(
                "write(/dev/null)={}",
                error.raw_os_error().unwrap_or(-1)
            )),
        }
    }
    Ok(answers.join(" "))
}

/// Applies every refusal to the calling process and returns the kernel's answer.
#[allow(unsafe_code)]
pub(super) fn enter(refused: &[ProcessCapability]) -> Result<String, EnforcementError> {
    let refused_socket = refused.contains(&ProcessCapability::OpenOutboundSocket);
    let refused_write = refused.contains(&ProcessCapability::WriteStagedArtifact);

    // 1. `no_new_privs`, which both restrictions require.
    // SAFETY: `PR_SET_NO_NEW_PRIVS` takes scalars and touches no user memory.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(EnforcementError::Syscall {
            step: "prctl(PR_SET_NO_NEW_PRIVS)",
            code: errno(),
        });
    }

    // 2. Landlock, when the class does not declare a write.
    if refused_write {
        let abi = landlock_abi();
        if abi < 1 {
            return Err(EnforcementError::Unavailable {
                reason: format!(
                    "landlock version query returned {abi} (errno {}), so this kernel cannot \
                     refuse a write",
                    errno()
                ),
            });
        }
        refuse_every_write(abi)?;
    }

    // 3. The seccomp filter, when the class does not declare a socket.
    if refused_socket {
        refuse_every_socket()?;
    }

    // 4. The kernel's own answer about both.
    verify(refused_write, refused_socket)
}

#[cfg(test)]
mod tests {
    use super::{denied_syscalls, status_field};

    #[test]
    fn the_deny_list_has_no_duplicate_and_no_negative_number() {
        let denied = denied_syscalls();
        let mut sorted = denied.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            denied.len(),
            "the deny list repeats a syscall"
        );
        assert!(
            denied.iter().all(|number| *number >= 0),
            "a denied syscall number is negative on this architecture"
        );
    }

    #[test]
    fn a_status_field_is_read_by_exact_key() {
        let status = "Name:\tprobe\nSeccomp:\t2\nSeccomp_filters:\t1\nNoNewPrivs:\t1\n";
        assert_eq!(status_field(status, "Seccomp").as_deref(), Some("2"));
        assert_eq!(
            status_field(status, "Seccomp_filters").as_deref(),
            Some("1")
        );
        assert_eq!(status_field(status, "NoNewPrivs").as_deref(), Some("1"));
        assert_eq!(status_field(status, "Secco"), None);
        assert_eq!(status_field(status, "Nonexistent"), None);
    }
}
