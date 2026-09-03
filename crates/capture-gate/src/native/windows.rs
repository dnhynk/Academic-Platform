//! The Windows device layer: an AppContainer with no capability SID.
//!
//! # What refuses, and why it is the parent that applies it
//!
//! A Windows capture device is a kernel-streaming filter reached by
//! `CreateFileW` on a device interface path. Its security descriptor is the
//! driver's, not the caller's: on the host this contract was measured on, the
//! microphone filter's DACL is
//! `D:P(A;;FA;;;SY)(A;;0x1201bf;;;BA)(A;;0x1201bf;;;WD)(A;;0x1201bf;;;RC)` --
//! four entries and **no** `ALL APPLICATION PACKAGES`. An AppContainer's access
//! check needs one, so the open is `ERROR_ACCESS_DENIED` (5) inside the
//! container and succeeds outside it.
//!
//! That contrast is the exact inverse of `academic-worker`'s socket row, where
//! `\Device\Afd` *does* grant `ALL APPLICATION PACKAGES` and the handle is
//! therefore created inside the container. The mechanism is the same one; the
//! answer differs because the two device objects are ACLed differently, and
//! both are written down as measured rather than as expected.
//!
//! # What this backend cannot do
//!
//! It cannot widen. A device object's DACL is not the caller's to edit -- it
//! needs `WRITE_DAC`, which the driver grants to `SYSTEM` and administrators --
//! so there is no user-mode way to add the container SID for the classes a
//! token *does* grant. The Windows container therefore refuses every class, and
//! the granted classes are reached by the parent, which is unrestricted, rather
//! than by widening the child. `DeviceRuleset` is still what decides which
//! classes the parent will open; what this file does not claim is that the
//! kernel enforces the split. Linux does; this does not; the contract page says
//! so per platform.

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    path::Path,
    ptr::{null, null_mut},
};

use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::{
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW,
            CM_Get_Device_Interface_ListW,
        },
        Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_TIMEOUT},
        Security::{
            ACL,
            Authorization::{
                EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, SE_FILE_OBJECT,
                SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID,
                TRUSTEE_IS_WELL_KNOWN_GROUP,
            },
            DACL_SECURITY_INFORMATION,
            Isolation::{CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName},
            PSID, SECURITY_CAPABILITIES,
        },
        System::Threading::{
            CREATE_SUSPENDED, CreateProcessW, DeleteProcThreadAttributeList,
            EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE,
            InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_INFORMATION, ResumeThread,
            STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
    core::GUID,
};

use super::{LaunchSpec, NativeError, REPORT_DIR_VAR, REPORT_FILE};
use crate::device::{BackendId, DeviceClass, DeviceLayer};

/// The container the contained runs use. One name, so a run does not leave a
/// new profile behind on every launch.
const CONTAINER_NAME: &str = "academic-capture-gate-probe";

const RIGHTS_READ_EXECUTE: u32 = 0x0012_00a9;
const RIGHTS_READ_WRITE: u32 = 0x0012_01bf;
const INHERIT_ALL: u32 = 0x0000_0003;

/// `KSCATEGORY_CAPTURE`. Audio and video capture filters register here.
const KSCATEGORY_CAPTURE: GUID = GUID::from_u128(0x65e8_773d_8f56_11d0_a3b9_00a0_c922_3196);
/// `KSCATEGORY_VIDEO_CAMERA`.
const KSCATEGORY_VIDEO_CAMERA: GUID = GUID::from_u128(0xe532_3777_f976_4f5b_9b55_b946_99c4_6e44);

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(unsafe_code)]
fn last_error() -> i64 {
    // SAFETY: reads this thread's last-error value and touches no user memory.
    i64::from(unsafe { GetLastError() })
}

/// The device interface paths this host exposes for `class`.
///
/// Enumerated from the configuration manager rather than compiled in, and only
/// the present ones: an interface that is registered but not enabled is not a
/// device this host has.
#[allow(unsafe_code)]
pub(super) fn device_interface_paths(class: DeviceClass) -> Vec<String> {
    let guid = match class {
        DeviceClass::Microphone => KSCATEGORY_CAPTURE,
        DeviceClass::Camera => KSCATEGORY_VIDEO_CAMERA,
        // The screen is not a device interface class. A host exposes no path
        // for it, and the row that would name one says `NOT_RUN` instead.
        DeviceClass::Screen => return Vec::new(),
    };
    let mut length: u32 = 0;
    // SAFETY: `length` is a live out-pointer, `guid` is a live GUID for the
    // duration of the call, and a null device id means "every device".
    let sized = unsafe {
        CM_Get_Device_Interface_List_SizeW(
            &raw mut length,
            &raw const guid,
            null(),
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };
    if sized != 0 || length == 0 {
        return Vec::new();
    }
    let mut buffer = vec![0_u16; length as usize];
    // SAFETY: `buffer` is at least `length` wide characters and outlives the
    // call; the same GUID and null device id as the size query.
    let listed = unsafe {
        CM_Get_Device_Interface_ListW(
            &raw const guid,
            null(),
            buffer.as_mut_ptr(),
            length,
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };
    if listed != 0 {
        return Vec::new();
    }
    String::from_utf16_lossy(&buffer)
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// Creates the container profile if it is absent, then returns its SID.
#[allow(unsafe_code)]
fn container_sid() -> Result<PSID, NativeError> {
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
    Err(NativeError::Syscall {
        step: "CreateAppContainerProfile/DeriveAppContainerSidFromAppContainerName",
        code: i64::from(derived),
    })
}

/// Adds one allow ACE for `sid` to `path`'s DACL, keeping every existing entry.
#[allow(unsafe_code)]
fn grant(path: &Path, sid: PSID, rights: u32, inherit: u32) -> Result<(), NativeError> {
    let Some(text) = path.to_str() else {
        return Err(NativeError::Path(format!(
            "{} is not valid Unicode",
            path.display()
        )));
    };
    let mut raw = wide(text);
    let mut existing: *mut ACL = null_mut();
    let mut descriptor: *mut c_void = null_mut();
    // SAFETY: `raw` is a live wide string and the two out-pointers are live
    // locals; the descriptor is freed by the process exiting rather than leaked
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
        return Err(NativeError::Path(format!(
            "GetNamedSecurityInfoW on {} returned {read}",
            path.display()
        )));
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
        return Err(NativeError::Path(format!(
            "SetEntriesInAclW on {} returned {built}",
            path.display()
        )));
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
        return Err(NativeError::Path(format!(
            "SetNamedSecurityInfoW on {} returned {written}",
            path.display()
        )));
    }
    Ok(())
}

/// Whether an AppContainer can be created here.
pub(super) fn availability() -> DeviceLayer {
    match container_sid() {
        Ok(_) => DeviceLayer::Enforced(BackendId::WindowsAppContainer),
        Err(_) => DeviceLayer::Unavailable,
    }
}

/// There is nothing for the child to install.
#[allow(dead_code)]
///
/// The container was applied by [`launch`] before this process existed, so this
/// reports that no backend was installed *here* rather than claiming one.
pub(super) fn enter() -> Result<BackendId, NativeError> {
    Ok(BackendId::None)
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: the handle was returned by `CreateProcessW` and is closed
            // exactly once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Runs the probe, contained or not, and returns what it wrote.
#[allow(unsafe_code)]
pub(super) fn launch(spec: &LaunchSpec) -> Result<String, NativeError> {
    let mut command = String::new();
    command.push('"');
    command.push_str(&spec.program.display().to_string());
    command.push('"');
    for argument in &spec.targets {
        command.push_str(" \"");
        command.push_str(argument);
        command.push('"');
    }
    let mut command = wide(&command);
    let environment = environment_block(&spec.report_dir);

    if !spec.contained {
        // The paired permission: the same binary and the same arguments with no
        // container, so a refusal inside one is a refusal the machine would not
        // have given anyway.
        let status = std::process::Command::new(&spec.program)
            .args(&spec.targets)
            .env(REPORT_DIR_VAR, &spec.report_dir)
            .status()
            .map_err(|error| NativeError::Path(format!("probe would not start: {error}")))?;
        if !status.success() {
            return Err(NativeError::Path(format!("probe exited with {status}")));
        }
        return read_report(&spec.report_dir);
    }

    let sid = container_sid()?;
    grant(&spec.report_dir, sid, RIGHTS_READ_WRITE, INHERIT_ALL)?;
    if let Some(parent) = spec.program.parent() {
        grant(parent, sid, RIGHTS_READ_EXECUTE, 0)?;
    }
    grant(&spec.program, sid, RIGHTS_READ_EXECUTE, 0)?;

    // SAFETY: zeroing a `SECURITY_CAPABILITIES` is the documented way to start
    // one; a zero capability count with a null array is "no capabilities", and
    // no capability is what refuses the device.
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
        return Err(NativeError::Syscall {
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
        return Err(NativeError::Syscall {
            step: "UpdateProcThreadAttribute",
            code: last_error(),
        });
    }

    // SAFETY: zeroing a `STARTUPINFOEXW` is the documented way to start one.
    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = u32::try_from(size_of::<STARTUPINFOEXW>()).unwrap_or(0);
    startup.lpAttributeList = attributes;
    // SAFETY: zeroing a `PROCESS_INFORMATION` is the documented way to start one.
    let mut process: PROCESS_INFORMATION = unsafe { zeroed() };

    // SAFETY: `command` and `environment` are live wide buffers for the
    // duration of the call; the attribute list carries the container SID.
    let created = unsafe {
        CreateProcessW(
            null(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | 0x0000_0400,
            environment.as_ptr().cast(),
            null(),
            (&raw const startup).cast(),
            &raw mut process,
        )
    };
    // SAFETY: initialized above, deleted exactly once.
    unsafe { DeleteProcThreadAttributeList(attributes) };
    if created == 0 {
        return Err(NativeError::Syscall {
            step: "CreateProcessW",
            code: last_error(),
        });
    }
    let process_handle = OwnedHandle(process.hProcess);
    let thread_handle = OwnedHandle(process.hThread);
    // SAFETY: the thread was created suspended by this call and is resumed once.
    unsafe { ResumeThread(thread_handle.0) };
    // SAFETY: a live process handle and the documented infinite timeout.
    if unsafe { WaitForSingleObject(process_handle.0, INFINITE) } == WAIT_TIMEOUT {
        return Err(NativeError::Syscall {
            step: "WaitForSingleObject",
            code: last_error(),
        });
    }
    let mut code: u32 = 0;
    // SAFETY: a live process handle and a live out-pointer.
    unsafe { GetExitCodeProcess(process_handle.0, &raw mut code) };
    read_report(&spec.report_dir)
}

fn read_report(report_dir: &Path) -> Result<String, NativeError> {
    std::fs::read_to_string(report_dir.join(REPORT_FILE))
        .map_err(|error| NativeError::Path(format!("report unreadable: {error}")))
}

/// The environment the contained probe runs with.
///
/// Inherited rather than minimal, plus the report directory. `academic-worker`
/// measured that a hand-built minimal block is refused `ERROR_ENVVAR_NOT_FOUND`
/// by `CreateProcessW` into an AppContainer; the environment is not part of
/// this boundary either.
fn environment_block(report_dir: &Path) -> Vec<u16> {
    let mut block = Vec::new();
    for (key, value) in std::env::vars() {
        if key.eq_ignore_ascii_case(REPORT_DIR_VAR) {
            continue;
        }
        block.extend(wide(&format!("{key}={value}")));
    }
    block.extend(wide(&format!("{REPORT_DIR_VAR}={}", report_dir.display())));
    block.push(0);
    block
}
