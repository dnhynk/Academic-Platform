//! Reviewed Windows CNG DPAPI (DPAPI-NG) implementation.
//!
//! Every unsafe block is confined to a small private FFI function with a
//! concrete invariant. No descriptor handle, allocated buffer pointer, or key
//! material crosses this module boundary.
//!
//! The broker is *stateless*: `NCryptProtectSecret` seals under the current
//! user's `LOCAL=user` protection descriptor and stores nothing itself, so the
//! returned blob is the only artifact and the raw protection key never enters
//! this process at all.

use std::{
    ffi::c_void,
    ptr::{null, null_mut},
};

use windows_sys::Win32::Security::{
    Cryptography::{
        NCRYPT_SILENT_FLAG, NCryptCloseProtectionDescriptor, NCryptCreateProtectionDescriptor,
        NCryptFreeBuffer, NCryptProtectSecret, NCryptUnprotectSecret,
    },
    NCRYPT_DESCRIPTOR_HANDLE,
};

use crate::{
    KeystoreError, KeystoreErrorCode, KeystoreLabel, PROVIDER, RecoveredSecret, bind_label,
    encode_envelope, unbind_label,
};

/// Protection descriptor: the current interactive user on this machine.
const LOCAL_USER_DESCRIPTOR: &[u16] = &[
    b'L' as u16,
    b'O' as u16,
    b'C' as u16,
    b'A' as u16,
    b'L' as u16,
    b'=' as u16,
    b'u' as u16,
    b's' as u16,
    b'e' as u16,
    b'r' as u16,
    0,
];

/// `NTE_BAD_KEY_STATE`, returned when the user's protection key is unusable.
const NTE_BAD_KEY_STATE: i32 = -2146893813_i32;
/// `NTE_PERM`, returned when the caller may not use the descriptor.
const NTE_PERM: i32 = -2146893738_i32;

/// An `NCRYPT_DESCRIPTOR_HANDLE` closed exactly once on drop.
#[derive(Debug)]
struct OwnedDescriptor(NCRYPT_DESCRIPTOR_HANDLE);

impl Drop for OwnedDescriptor {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `self.0` is a descriptor produced by a successful
            // `NCryptCreateProtectionDescriptor` in `create_descriptor`, is not
            // null, and is closed exactly once because `OwnedDescriptor` is
            // neither `Copy` nor `Clone` and never hands the handle out.
            let _ = unsafe { NCryptCloseProtectionDescriptor(self.0) };
        }
    }
}

/// An NCrypt-allocated output buffer freed exactly once on drop.
#[derive(Debug)]
struct OwnedNcryptBuffer {
    pointer: *mut u8,
    len: u32,
}

impl OwnedNcryptBuffer {
    #[allow(unsafe_code)]
    fn to_vec(&self) -> Vec<u8> {
        if self.pointer.is_null() || self.len == 0 {
            return Vec::new();
        }
        // SAFETY: `pointer` and `len` are the out-parameters of a successful
        // `NCryptProtectSecret`/`NCryptUnprotectSecret` call, so the region is
        // one initialized allocation of exactly `len` bytes owned by `self` and
        // still live for the duration of this borrow.
        unsafe { std::slice::from_raw_parts(self.pointer, self.len as usize) }.to_vec()
    }
}

impl Drop for OwnedNcryptBuffer {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            // SAFETY: `pointer` was allocated by NCrypt on a successful protect
            // or unprotect call with a null `pMemPara`, so `NCryptFreeBuffer`
            // is its matching deallocator. It runs exactly once because the
            // type is neither `Copy` nor `Clone`.
            let _ = unsafe { NCryptFreeBuffer(self.pointer.cast::<c_void>()) };
            self.pointer = null_mut();
        }
    }
}

fn classify(status: i32) -> KeystoreErrorCode {
    match status {
        NTE_BAD_KEY_STATE => KeystoreErrorCode::Unavailable,
        NTE_PERM => KeystoreErrorCode::AccessDenied,
        _ => KeystoreErrorCode::OperatingSystem,
    }
}

fn os_error(status: i32, code: KeystoreErrorCode, operation: &'static str) -> KeystoreError {
    KeystoreError::new(code, operation, Some(i64::from(status)))
}

#[allow(unsafe_code)]
fn create_descriptor(operation: &'static str) -> Result<OwnedDescriptor, KeystoreError> {
    let mut descriptor: NCRYPT_DESCRIPTOR_HANDLE = null_mut();
    // SAFETY: the descriptor string is a null-terminated UTF-16 constant that
    // outlives the call, the flag word is zero, and `descriptor` is a live,
    // writable local that the callee only assigns on success.
    let status = unsafe {
        NCryptCreateProtectionDescriptor(LOCAL_USER_DESCRIPTOR.as_ptr(), 0, &raw mut descriptor)
    };
    if status != 0 {
        return Err(os_error(status, classify(status), operation));
    }
    if descriptor.is_null() {
        return Err(KeystoreError::new(
            KeystoreErrorCode::Unavailable,
            operation,
            None,
        ));
    }
    Ok(OwnedDescriptor(descriptor))
}

#[allow(unsafe_code)]
fn protect(
    descriptor: &OwnedDescriptor,
    plaintext: &[u8],
    operation: &'static str,
) -> Result<Vec<u8>, KeystoreError> {
    let length = u32::try_from(plaintext.len())
        .map_err(|_| KeystoreError::new(KeystoreErrorCode::SecretTooLarge, operation, None))?;
    let mut pointer: *mut u8 = null_mut();
    let mut produced: u32 = 0;
    // SAFETY: `descriptor.0` is a live descriptor owned by the caller,
    // `plaintext` is a readable region of exactly `length` bytes, `pMemPara`
    // and `hwnd` are null as the silent flag requires, and both out-parameters
    // are live writable locals. The callee writes them only on success.
    let status = unsafe {
        NCryptProtectSecret(
            descriptor.0,
            NCRYPT_SILENT_FLAG,
            plaintext.as_ptr(),
            length,
            null(),
            null_mut(),
            &raw mut pointer,
            &raw mut produced,
        )
    };
    if status != 0 {
        return Err(os_error(status, classify(status), operation));
    }
    let owned = OwnedNcryptBuffer {
        pointer,
        len: produced,
    };
    if owned.pointer.is_null() || owned.len == 0 {
        return Err(KeystoreError::new(
            KeystoreErrorCode::OperatingSystem,
            operation,
            None,
        ));
    }
    Ok(owned.to_vec())
}

#[allow(unsafe_code)]
fn unprotect(sealed: &[u8], operation: &'static str) -> Result<Vec<u8>, KeystoreError> {
    let length = u32::try_from(sealed.len())
        .map_err(|_| KeystoreError::new(KeystoreErrorCode::InvalidSealedBlob, operation, None))?;
    let mut pointer: *mut u8 = null_mut();
    let mut produced: u32 = 0;
    // SAFETY: the descriptor out-parameter is null because we do not want the
    // blob's descriptor back, `sealed` is a readable region of exactly `length`
    // bytes, `pMemPara` and `hwnd` are null as the silent flag requires, and
    // both out-parameters are live writable locals written only on success.
    let status = unsafe {
        NCryptUnprotectSecret(
            null_mut(),
            NCRYPT_SILENT_FLAG,
            sealed.as_ptr(),
            length,
            null(),
            null_mut(),
            &raw mut pointer,
            &raw mut produced,
        )
    };
    if status != 0 {
        // A tampered or foreign blob lands here; it is an integrity failure,
        // not an operating-system outage, so it must not read as `Unavailable`.
        let code = match classify(status) {
            KeystoreErrorCode::OperatingSystem => KeystoreErrorCode::InvalidSealedBlob,
            other => other,
        };
        return Err(os_error(status, code, operation));
    }
    let owned = OwnedNcryptBuffer {
        pointer,
        len: produced,
    };
    Ok(owned.to_vec())
}

pub(crate) fn seal(
    label: &KeystoreLabel,
    secret: &[u8],
    operation: &'static str,
) -> Result<Vec<u8>, KeystoreError> {
    let descriptor = create_descriptor(operation)?;
    let bound = bind_label(label, secret);
    let sealed = protect(&descriptor, &bound, operation)?;
    Ok(encode_envelope(PROVIDER, &sealed))
}

pub(crate) fn open(
    label: &KeystoreLabel,
    payload: &[u8],
    operation: &'static str,
) -> Result<RecoveredSecret, KeystoreError> {
    let bound = unprotect(payload, operation)?;
    unbind_label(label, bound, operation)
}
