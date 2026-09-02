//! The Windows backend: an AppContainer token and a job object.
//!
//! Both are applied by the *creator*, so unlike Linux the whole backend runs in
//! the parent and [`enter`] has nothing left to do.
//!
//! # What each piece refuses
//!
//! The AppContainer is created with **no capability SIDs at all**. That is what
//! denies the home and vault reads: the container's SID appears in no ACL on
//! the user's profile, so every open returns `ERROR_ACCESS_DENIED`. It is also
//! what denies the network: with no `internetClient` or
//! `privateNetworkClientServer` capability, the platform refuses every
//! `connect` — `WSAEACCES` for a routable address, and a silent drop for
//! loopback. The three paths the job may reach are granted explicitly, by
//! adding one allow ACE for the container SID to each.
//!
//! The job object holds `ActiveProcessLimit = 1`, which is what denies a child
//! process: `CreateProcess` inside the container fails with
//! `ERROR_NOT_ENOUGH_QUOTA`. It also carries `ProcessMemoryLimit` and
//! `PerProcessUserTimeLimit`, and `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` so a
//! parent that dies takes the job with it.
//!
//! # The one thing this backend does not refuse
//!
//! It does not refuse the *creation* of a socket handle. `\Device\Afd` grants
//! `ALL APPLICATION PACKAGES`, and no user-mode mechanism removes that without
//! a filter driver or an administrator. `socket()` therefore succeeds inside the
//! container and every attempt to use it is refused. That is what the
//! acceptance suite asserts and what the contract says; neither claims more.

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    path::Path,
    ptr::{null, null_mut},
    time::Instant,
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_TIMEOUT},
    Security::{
        ACL,
        Authorization::*,
        DACL_SECURITY_INFORMATION,
        Isolation::{CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName},
        PSID, SECURITY_CAPABILITIES,
    },
    System::{
        JobObjects::*,
        Threading::{
            CREATE_SUSPENDED, CreateProcessW, DeleteProcThreadAttributeList,
            EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, InitializeProcThreadAttributeList,
            LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            PROCESS_INFORMATION, ResumeThread, STARTUPINFOEXW, UpdateProcThreadAttribute,
            WaitForSingleObject,
        },
    },
};

use super::{Availability, BackendId, LaunchSpec, SandboxError, SandboxUnavailable};
use crate::{
    job::ProbeReport,
    receipt::{LimitKind, ResourceReceipt, RunOutcome},
};

/// The container the worker runs in. One profile, reused: creating it is
/// idempotent and deriving the SID from the name gives the same value whether
/// the profile was created by this run or an earlier one.
const CONTAINER_NAME: &str = "academic-worker-p2-g4";

/// `FILE_GENERIC_READ | FILE_GENERIC_EXECUTE`.
const RIGHTS_READ_EXECUTE: u32 = 0x0012_00a9;
/// `FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE`.
const RIGHTS_READ_WRITE: u32 = 0x0013_01bf;
/// `OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE`.
const INHERIT_ALL: u32 = 0x3;
/// `STATUS_QUOTA_EXCEEDED`, the code a job-object time kill leaves behind.
const STATUS_QUOTA_EXCEEDED: u32 = 0xc000_0044;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(unsafe_code)]
fn last_error() -> i64 {
    // SAFETY: reads this thread's last-error value and touches no user memory.
    i64::from(unsafe { GetLastError() })
}

/// Creates the container profile if it is absent, then returns its SID.
#[allow(unsafe_code)]
fn container_sid() -> Result<PSID, SandboxError> {
    let name = wide(CONTAINER_NAME);
    let mut sid: PSID = null_mut();
    // SAFETY: all four string pointers are live NUL-terminated wide strings for
    // the duration of the call and `sid` is a live out-pointer.
    let created = unsafe {
        CreateAppContainerProfile(
            name.as_ptr(),
            name.as_ptr(),
            name.as_ptr(),
            null(),
            0,
            &raw mut sid,
        )
    };
    if created == 0 {
        return Ok(sid);
    }
    // SAFETY: same contract; the derive path is what answers when the profile
    // already exists.
    let derived = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &raw mut sid) };
    if derived == 0 {
        return Ok(sid);
    }
    Err(SandboxError::Syscall {
        step: "CreateAppContainerProfile/DeriveAppContainerSidFromAppContainerName",
        code: i64::from(derived),
    })
}

/// Adds one allow ACE for `sid` to `path`'s DACL, keeping every existing entry.
#[allow(unsafe_code)]
fn grant(path: &Path, sid: PSID, rights: u32, inherit: u32) -> Result<(), SandboxError> {
    let Some(text) = path.to_str() else {
        return Err(SandboxError::StagedPath {
            path: path.to_path_buf(),
            detail: String::from("path is not valid Unicode"),
        });
    };
    let mut raw = wide(text);
    let mut existing: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
    // SAFETY: `raw` is a live wide string; the two out-pointers are live locals
    // and the descriptor is freed by the process exiting rather than leaked
    // across a loop.
    let read = unsafe {
        GetNamedSecurityInfoW(
            raw.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &raw mut existing,
            null_mut(),
            (&raw mut descriptor).cast(),
        )
    };
    if read != 0 {
        return Err(SandboxError::StagedPath {
            path: path.to_path_buf(),
            detail: format!("GetNamedSecurityInfoW returned {read}"),
        });
    }
    // SAFETY: zeroing an `EXPLICIT_ACCESS_W` is the documented way to start one.
    let mut entry: EXPLICIT_ACCESS_W = unsafe { zeroed() };
    entry.grfAccessPermissions = rights;
    entry.grfAccessMode = GRANT_ACCESS;
    entry.grfInheritance = inherit;
    entry.Trustee.TrusteeForm = TRUSTEE_IS_SID;
    entry.Trustee.TrusteeType = TRUSTEE_IS_WELL_KNOWN_GROUP;
    entry.Trustee.ptstrName = sid.cast();
    let mut merged: *mut ACL = null_mut();
    // SAFETY: one live entry, the ACL just read, and a live out-pointer.
    let built = unsafe { SetEntriesInAclW(1, &raw const entry, existing, &raw mut merged) };
    if built != 0 {
        return Err(SandboxError::StagedPath {
            path: path.to_path_buf(),
            detail: format!("SetEntriesInAclW returned {built}"),
        });
    }
    // SAFETY: `merged` is the ACL just built and `raw` is still live.
    let written = unsafe {
        SetNamedSecurityInfoW(
            raw.as_mut_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            merged,
            null_mut(),
        )
    };
    if written != 0 {
        return Err(SandboxError::StagedPath {
            path: path.to_path_buf(),
            detail: format!("SetNamedSecurityInfoW returned {written}"),
        });
    }
    Ok(())
}

/// Whether an AppContainer can be created here.
pub(super) fn availability() -> Availability {
    match container_sid() {
        Ok(_) => Availability::Available(BackendId::WindowsAppContainerJob),
        Err(error) => Availability::Unavailable(SandboxUnavailable {
            backend: BackendId::WindowsAppContainerJob,
            reason: format!("this Windows build cannot create an AppContainer profile: {error}"),
        }),
    }
}

/// The child side, which has nothing to do: the parent contained it already.
pub(super) fn enter() -> Result<BackendId, SandboxError> {
    Ok(BackendId::WindowsAppContainerJob)
}

struct JobHandle(HANDLE);

impl Drop for JobHandle {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: the handle was created by `CreateJobObjectW` and is closed
        // exactly once. Closing it terminates the job, which is the intent:
        // `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` is set.
        unsafe { CloseHandle(self.0) };
    }
}

/// Launches the probe inside the container and the job, then measures it.
#[allow(clippy::too_many_lines)]
#[allow(unsafe_code)]
pub(super) fn launch(spec: &LaunchSpec) -> Result<(ResourceReceipt, ProbeReport), SandboxError> {
    let sid = container_sid()?;
    super::write_job_inputs(spec)?;

    let descriptor = &spec.plan.descriptor;
    grant(
        descriptor.staged_input(),
        sid,
        RIGHTS_READ_EXECUTE,
        INHERIT_ALL,
    )?;
    grant(
        descriptor.staged_output(),
        sid,
        RIGHTS_READ_WRITE,
        INHERIT_ALL,
    )?;
    grant(&spec.report_dir, sid, RIGHTS_READ_WRITE, INHERIT_ALL)?;
    if let Some(parent) = spec.program.parent() {
        grant(parent, sid, RIGHTS_READ_EXECUTE, 0)?;
    }
    grant(&spec.program, sid, RIGHTS_READ_EXECUTE, 0)?;

    // SAFETY: zeroing a `SECURITY_CAPABILITIES` is the documented way to start
    // one; a zero capability count with a null array is "no capabilities".
    let mut capabilities: SECURITY_CAPABILITIES = unsafe { zeroed() };
    capabilities.AppContainerSid = sid;
    capabilities.CapabilityCount = 0;

    let mut size: usize = 0;
    // SAFETY: the documented size query; a null list with a live size pointer.
    unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &raw mut size) };
    let mut backing = vec![0_u8; size];
    let attributes = backing.as_mut_ptr().cast::<c_void>() as LPPROC_THREAD_ATTRIBUTE_LIST;
    // SAFETY: `backing` is at least `size` bytes and outlives every use below.
    if unsafe { InitializeProcThreadAttributeList(attributes, 1, 0, &raw mut size) } == 0 {
        return Err(SandboxError::Syscall {
            step: "InitializeProcThreadAttributeList",
            code: last_error(),
        });
    }
    // SAFETY: `capabilities` outlives `CreateProcessW`, which is what the
    // attribute list requires: it stores the pointer rather than copying.
    let updated = unsafe {
        UpdateProcThreadAttribute(
            attributes,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            (&raw const capabilities).cast(),
            size_of::<SECURITY_CAPABILITIES>(),
            null_mut(),
            null(),
        )
    };
    if updated == 0 {
        // SAFETY: the list was initialized above and is deleted exactly once.
        unsafe { DeleteProcThreadAttributeList(attributes) };
        return Err(SandboxError::Syscall {
            step: "UpdateProcThreadAttribute",
            code: last_error(),
        });
    }

    let limits = *descriptor.limits();
    // SAFETY: `CreateJobObjectW` with two nulls creates an unnamed job with a
    // default descriptor.
    let job = JobHandle(unsafe { CreateJobObjectW(null(), null()) });
    if job.0.is_null() {
        // SAFETY: initialized above, deleted exactly once.
        unsafe { DeleteProcThreadAttributeList(attributes) };
        return Err(SandboxError::Syscall {
            step: "CreateJobObjectW",
            code: last_error(),
        });
    }
    // SAFETY: zeroing the limit structure is the documented way to start one.
    let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    information.BasicLimitInformation.ActiveProcessLimit = 1;
    let cpu_100ns = i64::try_from(limits.cpu_millis().saturating_mul(10_000)).unwrap_or(i64::MAX);
    information.BasicLimitInformation.PerProcessUserTimeLimit = cpu_100ns;
    information.ProcessMemoryLimit = usize::try_from(limits.memory_bytes()).unwrap_or(usize::MAX);
    // SAFETY: the structure and its size match the class being set.
    let configured = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const information).cast(),
            u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0),
        )
    };
    if configured == 0 {
        // SAFETY: initialized above, deleted exactly once.
        unsafe { DeleteProcThreadAttributeList(attributes) };
        return Err(SandboxError::Syscall {
            step: "SetInformationJobObject",
            code: last_error(),
        });
    }

    // SAFETY: zeroing a `STARTUPINFOEXW` is the documented way to start one.
    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap_or(0);
    startup.lpAttributeList = attributes;
    // SAFETY: zeroing a `PROCESS_INFORMATION` is the documented way to start one.
    let mut process: PROCESS_INFORMATION = unsafe { zeroed() };
    let mut command = wide(&format!("\"{}\" run", spec.program.display()));
    let environment = job_environment(spec);
    let started = Instant::now();
    // SAFETY: `command`, `environment`, `startup` and `process` are live for the
    // call; the command line is mutable as `CreateProcessW` requires.
    let created = unsafe {
        CreateProcessW(
            null(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | 0x0000_0400,
            environment.as_ptr().cast::<c_void>().cast_mut(),
            null(),
            (&raw mut startup).cast(),
            &raw mut process,
        )
    };
    // The error is read before anything else runs, because every intervening
    // call may replace it.
    let create_error = last_error();
    // SAFETY: initialized above, deleted exactly once, after the only call that
    // reads it.
    unsafe { DeleteProcThreadAttributeList(attributes) };
    if created == 0 {
        return Err(SandboxError::Launch {
            path: spec.program.clone(),
            detail: format!("CreateProcessW returned 0 (last error {create_error})"),
        });
    }
    // SAFETY: the handles come from the successful `CreateProcessW` above.
    let assigned = unsafe { AssignProcessToJobObject(job.0, process.hProcess) };
    if assigned == 0 {
        // SAFETY: the job is live; terminating it kills the suspended child.
        unsafe { TerminateJobObject(job.0, 1) };
        return Err(SandboxError::Syscall {
            step: "AssignProcessToJobObject",
            code: last_error(),
        });
    }
    // SAFETY: the thread handle comes from the successful `CreateProcessW`.
    unsafe { ResumeThread(process.hThread) };

    let wall = u32::try_from(limits.wall_millis()).unwrap_or(u32::MAX);
    // SAFETY: the process handle is live and not yet closed.
    let waited = unsafe { WaitForSingleObject(process.hProcess, wall) };
    let killed_for_wall = waited == WAIT_TIMEOUT;
    if killed_for_wall {
        // SAFETY: the job is live and holds exactly this process.
        unsafe { TerminateJobObject(job.0, 1) };
        // SAFETY: the process is terminating; this waits for it to finish.
        unsafe { WaitForSingleObject(process.hProcess, 10_000) };
    }
    let wall_millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let mut exit_code = 0_u32;
    // SAFETY: the process handle is live and `exit_code` is a live out-pointer.
    unsafe { GetExitCodeProcess(process.hProcess, &raw mut exit_code) };
    let (cpu_millis, peak_memory_bytes) = accounting(job.0);
    // SAFETY: both handles come from the successful `CreateProcessW` and are
    // closed exactly once.
    unsafe {
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }

    let output_bytes = super::staged_output_bytes(descriptor)?;
    let outcome = if killed_for_wall {
        RunOutcome::KilledByLimit(LimitKind::WallTime)
    } else if exit_code == 0 {
        RunOutcome::Completed
    } else if exit_code == STATUS_QUOTA_EXCEEDED || cpu_millis >= limits.cpu_millis() {
        RunOutcome::KilledByLimit(LimitKind::Cpu)
    } else {
        RunOutcome::Failed {
            exit_code: i64::from(exit_code),
        }
    };
    let outcome = super::apply_output_bound(outcome, output_bytes, &limits);
    let report = super::read_report(&spec.report_dir);
    Ok((
        ResourceReceipt::new(
            BackendId::WindowsAppContainerJob,
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

/// Total CPU milliseconds and peak process memory the job accounted for.
#[allow(unsafe_code)]
fn accounting(job: HANDLE) -> (u64, u64) {
    // SAFETY: zeroing the accounting structure is the documented way to start
    // one; the query writes exactly `size_of` bytes into it.
    let mut basic: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
    // SAFETY: the handle is live and the structure and size match the class.
    let read = unsafe {
        QueryInformationJobObject(
            job,
            JobObjectBasicAccountingInformation,
            (&raw mut basic).cast(),
            u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>()).unwrap_or(0),
            null_mut(),
        )
    };
    let cpu_millis = if read == 0 {
        0
    } else {
        let hundred_nanos = basic
            .TotalUserTime
            .saturating_add(basic.TotalKernelTime)
            .max(0);
        u64::try_from(hundred_nanos).unwrap_or(0) / 10_000
    };
    // SAFETY: as above, for the extended class.
    let mut extended: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    // SAFETY: the handle is live and the structure and size match the class.
    let read_extended = unsafe {
        QueryInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw mut extended).cast(),
            u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).unwrap_or(0),
            null_mut(),
        )
    };
    let peak = if read_extended == 0 {
        0
    } else {
        u64::try_from(extended.PeakProcessMemoryUsed).unwrap_or(0)
    };
    (cpu_millis, peak)
}

/// The child's environment block: the parent's, plus the four the job needs.
///
/// A hand-built minimal block is what this wanted to be, and it does not work:
/// `CreateProcessW` into an AppContainer refuses one with
/// `ERROR_ENVVAR_NOT_FOUND` (203). That was measured rather than assumed —
/// `SystemRoot`, `windir` and `SystemDrive` alone are refused, and so are those
/// plus `PATH`, `PATHEXT` and `COMSPEC`; the full block is accepted. The
/// environment is not part of the containment boundary either way: the
/// container's rights come from its token and the ACEs granted to its SID, and
/// nothing a variable names is reachable without one.
///
/// The entries are sorted case-insensitively, which is what `CreateProcess`
/// documents for an environment block it is handed.
fn job_environment(spec: &LaunchSpec) -> Vec<u16> {
    let mut block: Vec<u16> = Vec::new();
    let mut inherited: Vec<(String, String)> = std::env::vars()
        .filter(|(name, _)| !name.starts_with(super::VAR_PREFIX))
        .collect();
    inherited.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, value) in inherited {
        block.extend(wide(&format!("{name}={value}")));
    }
    for (name, value) in [
        (super::INPUT_DIR_VAR, spec.plan.descriptor.staged_input()),
        (super::REPORT_DIR_VAR, spec.report_dir.as_path()),
        (super::HOME_CANARY_VAR, spec.plan.home_canary.as_path()),
        (super::VAULT_CANARY_VAR, spec.plan.vault_canary.as_path()),
    ] {
        block.extend(wide(&format!("{name}={}", value.display())));
    }
    block.push(0);
    block
}
