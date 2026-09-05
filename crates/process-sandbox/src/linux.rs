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
//! # The filter gates an ABI, and the arch word is only half of one
//!
//! `seccomp_data` carries an `arch` token and a syscall number, and on x86 the
//! token does not identify the ABI: `AUDIT_ARCH_X86_64` is the token for the
//! 64-bit ABI *and* for x32, and the kernel tells the two apart by
//! [`X32_SYSCALL_BIT`] — bit 30 of the number — and by nothing else. A filter
//! that checks the token and then compares native numbers therefore lets every
//! x32 number fall past the comparisons to `SECCOMP_RET_ALLOW`. That is the
//! documented caveat in `seccomp(2)`, and `P2-A5` measured it here: a process
//! that reported `Seccomp: 2` opened an x32 socket and completed a TCP
//! handshake to a listener in another process.
//!
//! So the gate is two instructions, not one: the token has to be `AUDIT_ARCH`
//! *and* the number has to be below the bit. x32 numbers are `bit | n` for
//! `n` up to a few hundred and native numbers are all far below the bit, so an
//! unsigned floor separates the two ABIs exactly and needs no table.
//!
//! **Why the x32 numbers are not simply added to [`denied_syscalls`] instead.**
//! An x32 number is not always the native number with the bit set. x32 has its
//! own entry points from 512 up for calls whose argument layout differs, and
//! this deny list carries three of them: `recvfrom` is 517 and not 45,
//! `sendmsg` 518 and not 46, `recvmsg` 519 and not 47. The rest keep their
//! native numbers — `socket` 41, `socketpair` 53, `connect` 42, `bind` 49,
//! `listen` 50, `accept4` 288, `sendto` 44, and `io_uring_setup`/`_enter`/
//! `_register` 425/426/427. So a deny list built by setting the bit on each
//! native number would refuse seven of the ten socket calls and leave three
//! reachable. The ABI gate has no such table to get wrong, which is why the
//! rule lives there.
//!
//! aarch64 needs no floor: its 32-bit compat ABI carries `AUDIT_ARCH_ARM`, a
//! different token, so the arch check above already refuses it.
//!
//! # `libc::SYS_socket` in this file
//!
//! This file names the socket syscalls because it is the file that *refuses*
//! them. `only_egress_crate_has_a_socket` reads it, its allowance is those
//! exact spellings, and the allowance map is compared whole, so a spelling
//! added or removed here fails that scan until the table is edited in the same
//! commit. Every `SYS_` name in this file is either inside
//! [`denied_syscalls`] or one of the four this backend calls: the three it
//! installs *with*, and the `getpid` of [`x32_answer`], which it makes in order
//! to be refused.
//!
//! # Verification is the kernel's answer, not this file's
//!
//! A syscall that returned zero is not evidence that a restriction is in force.
//! After the installation, `/proc/self/status` is read back for `NoNewPrivs`
//! and `Seccomp`, and — when the write refusal was installed — `/dev/null` is
//! opened for writing and required to fail. Opening `/dev/null` creates
//! nothing, and a success there means the ruleset did not take, which is a
//! [`EnforcementError::NotVerified`] and therefore a refusal to start.
//!
//! `Seccomp: 2` is the weakest of those answers: it says a filter is attached
//! and nothing about what the filter covers, which is exactly how the x32
//! bypass above reported a true-looking receipt. So when the socket refusal was
//! installed, [`x32_answer`] makes one syscall on the other ABI and requires
//! `EPERM` — the filter's own answer, not the kernel's `ENOSYS` on a build
//! without x32 — before the receipt claims anything.
//!
//! What the receipt then carries is that answer, `x32(getpid)=-1`, and not a
//! word saying it was asked. The two differ under one edit: a check that reads
//! the syscall and drops its result leaves a receipt saying `EPERM` beside a
//! process that got its own pid, and a relayed number cannot say that.

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
/// Unsigned, and used only by the x32 floor, so it is compiled only where that
/// floor is. `aarch64-unknown-linux-gnu` refuses an unused constant.
#[cfg(target_arch = "x86_64")]
const BPF_JGE: u16 = 0x30;
const BPF_K: u16 = 0x00;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xc000_003e;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xc000_00b7;

/// The bit that selects the x32 ABI, which shares `AUDIT_ARCH_X86_64` with the
/// 64-bit one.
///
/// It is the whole reason the arch check above is not the ABI check. See the
/// module documentation.
#[cfg(target_arch = "x86_64")]
const X32_SYSCALL_BIT: i64 = 0x4000_0000;

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

/// The seccomp filter this backend installs, assembled.
///
/// Split out of [`refuse_every_socket`] so the program is a **value a test can
/// read**. `P2-A5` measured, three audits running, that removing the x32 floor
/// below and clamping [`x32_answer`] leaves this crate's whole suite green --
/// 20 passed on a build whose x32 ABI answered `getpid` with a real pid and
/// handed out a working socket descriptor. Nothing read the assembled program;
/// the verification was `run.outcome("enter").contains("x32(getpid)=-1")`, a
/// string the code under test prints about itself.
///
/// The test below reads it as the bytes the kernel is handed.
fn socket_filter_program() -> Result<Vec<SockFilter>, EnforcementError> {
    let denied = denied_syscalls();
    let mut program = vec![
        stmt(BPF_LD | BPF_W | BPF_ABS, 4),
        jump(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH, 1, 0),
        stmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        stmt(BPF_LD | BPF_W | BPF_ABS, 0),
    ];
    let count = denied.len();
    // The rest of the ABI check, which the arch word above cannot carry.
    //
    // Every x32 number is at or above the bit and every native one is below it,
    // so an unsigned floor separates the two ABIs exactly; `BPF_JGE` is
    // unsigned, and a number that is neither ABI's — a negative `nr` read as
    // `u32` — is refused with them, which is the safe direction.
    #[cfg(target_arch = "x86_64")]
    {
        // Past every native comparison and the allow, to the errno return.
        let (Ok(floor), Ok(jt)) = (u32::try_from(X32_SYSCALL_BIT), u8::try_from(count + 1)) else {
            return Err(EnforcementError::Syscall {
                step: "seccomp filter assembly",
                code: -1,
            });
        };
        program.push(jump(BPF_JMP | BPF_JGE | BPF_K, floor, jt, 0));
    }
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
    Ok(program)
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
    let program = socket_filter_program()?;
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

/// The kernel's answer to one x32 syscall, as a negated error number.
///
/// `getpid` is the number on purpose: it is not in [`denied_syscalls`], so the
/// answer separates a filter that refuses the x32 *ABI* from one that merely
/// carries x32 spellings of the denied numbers. Under the first it is `EPERM`;
/// under the second it is still this process's id. It reads nothing, writes
/// nothing and has no other effect, on either ABI.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
fn x32_answer() -> i64 {
    // SAFETY: `getpid` takes no argument and touches no user memory. The x32
    // bit selects which ABI the number is read under and nothing else.
    let returned = unsafe { libc::syscall(X32_SYSCALL_BIT | libc::SYS_getpid) };
    if returned < 0 { -errno() } else { returned }
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
        // `Seccomp: 2` says a filter is attached, not what it covers, and this
        // is where that difference was a `P2-A5` P1: the filter reported here
        // let an x32 `socket` through and a process that printed this line
        // completed a TCP handshake. So the second ABI is asked the same way
        // `/dev/null` asks about the ruleset — by making the call and requiring
        // the refusal to be the filter's own `EPERM` rather than the kernel's
        // `ENOSYS` on a build without x32.
        #[cfg(target_arch = "x86_64")]
        {
            let expected = -i64::from(libc::EPERM);
            let answer = x32_answer();
            if answer != expected {
                return Err(EnforcementError::NotVerified {
                    detail: format!(
                        "an x32 syscall answered {answer} rather than {expected}, so the \
                         filter does not refuse the x32 ABI"
                    ),
                });
            }
            answers.push(format!("x32(getpid)={answer}"));
        }
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
    use super::{
        EnforcementError, SockFilter, denied_syscalls, socket_filter_program, status_field,
    };

    /// The eight bytes of one filter instruction, as the kernel reads them.
    ///
    /// `sock_filter` is `#[repr(C)]` and the kernel copies it verbatim, so the
    /// little-endian encoding of the four fields **is** the instruction. This
    /// spells it out rather than transmuting, so the pin below is over bytes a
    /// reader can check against the ABI and not over this crate's memory
    /// layout believing itself.
    fn encoded(instruction: &SockFilter) -> [u8; 8] {
        let code = instruction.code.to_le_bytes();
        let k = instruction.k.to_le_bytes();
        [
            code[0],
            code[1],
            instruction.jt,
            instruction.jf,
            k[0],
            k[1],
            k[2],
            k[3],
        ]
    }

    /// The x32 floor is in the assembled program, as bytes.
    ///
    /// `P2-A5` scored this open for three rounds and named the close: the two
    /// acceptance tests over this backend read a string the code under test
    /// prints about itself, so removing the floor instruction and clamping
    /// `x32_answer` left `the_kernel_reports_the_filter_the_receipt_claims` and
    /// `no_class_reaches_the_second_abi_under_the_same_arch_token` both passing
    /// while the x32 ABI handed out a working socket descriptor. **A receipt
    /// that quotes the thing it is about is not a measurement.**
    ///
    /// This reads the program instead. The floor is instruction 4 -- after the
    /// arch load, the arch comparison, the kill and the syscall-number load --
    /// and its eight bytes are `BPF_JMP | BPF_JGE | BPF_K` (`0x35`), a jump
    /// past every denied comparison and the allow, no fall-through offset, and
    /// `X32_SYSCALL_BIT` little-endian. Removing the push fails here naming the
    /// instruction; moving it fails on the index; widening the deny list
    /// without moving the jump target fails on `jt`.
    ///
    /// x86_64 only, because the x32 ABI is: on `aarch64` there is no second ABI
    /// under one `AUDIT_ARCH` token and `BPF_JGE` is not compiled at all.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn the_x32_floor_is_pinned_as_the_bytes_the_kernel_reads() -> Result<(), EnforcementError> {
        let program = socket_filter_program()?;
        let denied = denied_syscalls().len();
        // Four setup instructions, the floor, one comparison per denied
        // syscall, the allow and the errno return.
        assert_eq!(
            program.len(),
            denied + 7,
            "the assembled filter is not the shape this pin is over"
        );
        // Saturating rather than panicking, the way the assembler's own
        // `u8::try_from(count - index - 1)` is: a deny list too long for a jump
        // offset produces a byte mismatch below, which is the failure that names
        // what changed.
        let jump_target = u8::try_from(denied + 1).unwrap_or(u8::MAX);
        assert_eq!(
            encoded(&program[4]),
            [0x35, 0x00, jump_target, 0x00, 0x00, 0x00, 0x00, 0x40],
            "the x32 floor is not the fifth instruction of the assembled filter"
        );
        // And it is the only unsigned jump, so a second one cannot stand in for
        // it while this one is gone.
        let floors = program
            .iter()
            .filter(|instruction| {
                instruction.code == super::BPF_JMP | super::BPF_JGE | super::BPF_K
            })
            .count();
        assert_eq!(floors, 1, "the filter carries {floors} unsigned jumps");
        Ok(())
    }

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
