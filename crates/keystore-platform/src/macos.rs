//! macOS data-protection Keychain generic-password storage, version 1.
//!
//! Main-executable entitlements and the signed-in user's login context govern
//! access. This is not a file keychain, Secure Enclave or hardware attestation.
//! See `docs/contracts/macos-device-keystore.md` for identity and memory limits.

use std::{ffi::c_void, fmt, ptr::NonNull};

use objc2::{
    MainThreadMarker,
    rc::{Retained, autoreleasepool},
};
use objc2_core_foundation::{
    CFBoolean, CFData, CFDictionary, CFMutableData, CFRetained, CFString, CFType,
    kCFTypeDictionaryKeyCallBacks,
};
use objc2_local_authentication::LAContext;
use objc2_security::{
    SecItemAdd, SecItemCopyMatching, SecItemDelete, errSecAuthFailed as ERR_AUTH_FAILED,
    errSecDuplicateItem as ERR_DUPLICATE_ITEM,
    errSecInteractionNotAllowed as ERR_INTERACTION_NOT_ALLOWED,
    errSecInteractionRequired as ERR_INTERACTION_REQUIRED,
    errSecItemNotFound as ERR_ITEM_NOT_FOUND, errSecMissingEntitlement as ERR_MISSING_ENTITLEMENT,
    errSecNotAvailable as ERR_NOT_AVAILABLE, errSecSuccess as ERR_SUCCESS,
    errSecUserCanceled as ERR_USER_CANCELED, kSecAttrAccessible,
    kSecAttrAccessibleWhenUnlockedThisDeviceOnly, kSecAttrAccount, kSecAttrGeneric,
    kSecAttrService, kSecAttrSynchronizable, kSecClass, kSecClassGenericPassword, kSecReturnData,
    kSecUseAuthenticationContext, kSecUseDataProtectionKeychain, kSecValueData,
};
use zeroize::Zeroize as _;

use crate::{
    KeystoreError, KeystoreErrorCode, KeystoreLabel, MAX_SECRET_BYTES, PROVIDER, PurgeOutcome,
    RecoveredSecret, encode_envelope, macos_blob,
};

const SERVICE: &str = "dev.academic-os.device-wrapping-key.data-protection.v1";

fn failure(code: KeystoreErrorCode, operation: &'static str) -> KeystoreError {
    KeystoreError::new(code, operation, None)
}

fn classify(status: i32, operation: &'static str) -> KeystoreError {
    let code = match status {
        ERR_ITEM_NOT_FOUND => KeystoreErrorCode::NotFound,
        ERR_DUPLICATE_ITEM => KeystoreErrorCode::DuplicateLabel,
        ERR_MISSING_ENTITLEMENT | ERR_NOT_AVAILABLE => KeystoreErrorCode::Unavailable,
        ERR_AUTH_FAILED
        | ERR_INTERACTION_NOT_ALLOWED
        | ERR_INTERACTION_REQUIRED
        | ERR_USER_CANCELED => KeystoreErrorCode::AccessDenied,
        _ => KeystoreErrorCode::OperatingSystem,
    };
    KeystoreError::new(code, operation, Some(i64::from(status)))
}

/// A mutable CF allocation created by us, never an immutable native result.
/// Its bytes are cleared before its last owned reference is released.
struct SecretData(CFRetained<CFMutableData>);

impl fmt::Debug for SecretData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretData(<redacted>)")
    }
}

impl SecretData {
    #[allow(unsafe_code)]
    fn new(secret: &[u8], operation: &'static str) -> Result<Self, KeystoreError> {
        if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
            return Err(failure(KeystoreErrorCode::SecretTooLarge, operation));
        }
        let length = secret.len() as isize;
        let data = CFMutableData::new(None, length)
            .ok_or_else(|| failure(KeystoreErrorCode::Unavailable, operation))?;
        // SAFETY: `data` is our live, empty mutable CFData, with capacity
        // exactly `length`. `secret` holds that many readable bytes through
        // this synchronous copy. No reference to the data has escaped.
        unsafe { CFMutableData::append_bytes(Some(&data), secret.as_ptr(), length) };
        Ok(Self(data))
    }
}

impl Drop for SecretData {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let length = self.0.len();
        let pointer = CFMutableData::mutable_byte_ptr(Some(&self.0));
        if !pointer.is_null() && length > 0 {
            // SAFETY: only `SecretData::new` creates this private wrapper, so
            // this is mutable CFData of the bounded, initialized length. The
            // native call has returned and the query dictionary is dropped
            // first. No slice or other caller can access these bytes now.
            unsafe { std::slice::from_raw_parts_mut(pointer, length) }.zeroize();
        }
    }
}

/// The dictionary borrows its values with null value callbacks. All values
/// are retained here and outlive the dictionary, including LAContext (which
/// is an Objective-C object, not a value to cast into a Rust `&CFType`).
struct Query {
    dictionary: CFRetained<CFDictionary>,
    _account: CFRetained<CFString>,
    _service: CFRetained<CFString>,
    _generation: CFRetained<CFData>,
    _authentication: Retained<LAContext>,
    _secret: Option<SecretData>,
}

impl fmt::Debug for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Query(<redacted>)")
    }
}

#[allow(unsafe_code)]
fn noninteractive_context() -> Retained<LAContext> {
    // SAFETY: LAContext is available since macOS 10.10, before the provider's
    // macOS 10.15 minimum. This fresh retained instance stays on this thread,
    // is never evaluated or shared, and permits no authentication UI. No
    // reusable authenticated context or biometric credential is supplied.
    unsafe {
        let context = LAContext::new();
        context.setInteractionNotAllowed(true);
        context
    }
}

#[allow(unsafe_code)]
fn query(
    label: &KeystoreLabel,
    generation: &[u8; macos_blob::GENERATION_BYTES],
    secret: Option<&[u8]>,
    return_data: bool,
    operation: &'static str,
) -> Result<Query, KeystoreError> {
    let account = CFString::from_str(label.as_str());
    let service = CFString::from_str(SERVICE);
    let generation = CFData::from_bytes(generation);
    let authentication = noninteractive_context();
    let secret = secret
        .map(|bytes| SecretData::new(bytes, operation))
        .transpose()?;
    // SAFETY: these imported Security constants are immutable, nonnull CFString
    // objects provided by the linked framework on macOS >= 10.15. All pointers
    // below refer either to them, static CFBooleans, or retained heap objects
    // moved into Query. LAContext is the documented type for its query key.
    let (mut keys, mut values): (Vec<*const c_void>, Vec<*const c_void>) = unsafe {
        (
            vec![
                NonNull::from(kSecClass).as_ptr().cast(),
                NonNull::from(kSecUseDataProtectionKeychain).as_ptr().cast(),
                NonNull::from(kSecAttrSynchronizable).as_ptr().cast(),
                NonNull::from(kSecAttrAccessible).as_ptr().cast(),
                NonNull::from(kSecAttrService).as_ptr().cast(),
                NonNull::from(kSecAttrAccount).as_ptr().cast(),
                NonNull::from(kSecAttrGeneric).as_ptr().cast(),
                NonNull::from(kSecUseAuthenticationContext).as_ptr().cast(),
            ],
            vec![
                NonNull::from(kSecClassGenericPassword).as_ptr().cast(),
                NonNull::from(CFBoolean::new(true)).as_ptr().cast(),
                NonNull::from(CFBoolean::new(false)).as_ptr().cast(),
                NonNull::from(kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
                    .as_ptr()
                    .cast(),
                NonNull::from(&*service).as_ptr().cast(),
                NonNull::from(&*account).as_ptr().cast(),
                NonNull::from(&*generation).as_ptr().cast(),
                NonNull::from(&*authentication).as_ptr().cast(),
            ],
        )
    };
    if let Some(bytes) = &secret {
        // SAFETY: same immutable framework-constant lifetime as above.
        keys.push(unsafe { NonNull::from(kSecValueData).as_ptr().cast() });
        values.push(NonNull::from(&*bytes.0).as_ptr().cast());
    }
    if return_data {
        // SAFETY: same immutable framework-constant lifetime as above.
        keys.push(unsafe { NonNull::from(kSecReturnData).as_ptr().cast() });
        values.push(NonNull::from(CFBoolean::new(true)).as_ptr().cast());
    }
    // SAFETY: equal, bounded arrays of live CFString keys and documented
    // SecItem value types. The key callbacks retain the CFString constants.
    // Null value callbacks borrow values without treating LAContext as a CF
    // object. Query owns every nonstatic value, drops the dictionary first,
    // and never exports it or mutates it during a synchronous Security call.
    let dictionary = unsafe {
        CFDictionary::new(
            None,
            keys.as_mut_ptr(),
            values.as_mut_ptr(),
            keys.len() as isize,
            &kCFTypeDictionaryKeyCallBacks,
            std::ptr::null(),
        )
    }
    .ok_or_else(|| failure(KeystoreErrorCode::Unavailable, operation))?;
    Ok(Query {
        dictionary,
        _account: account,
        _service: service,
        _generation: generation,
        _authentication: authentication,
        _secret: secret,
    })
}

fn require_background_thread(operation: &'static str) -> Result<(), KeystoreError> {
    if MainThreadMarker::new().is_some() {
        return Err(failure(KeystoreErrorCode::Unavailable, operation));
    }
    Ok(())
}

#[allow(unsafe_code)]
fn add(query: &Query, operation: &'static str) -> Result<(), KeystoreError> {
    // SAFETY: query owns all of its correctly typed values through the call.
    // A null output is explicitly permitted; no native result is requested.
    let status = unsafe { SecItemAdd(&query.dictionary, std::ptr::null_mut()) };
    if status == ERR_SUCCESS {
        Ok(())
    } else {
        Err(classify(status, operation))
    }
}

#[allow(unsafe_code)]
fn copy(query: &Query, operation: &'static str) -> Result<RecoveredSecret, KeystoreError> {
    let mut result: *const CFType = std::ptr::null();
    // SAFETY: the query lives through the synchronous call, and the writable
    // output starts null. The only return flag is kSecReturnData, with the
    // default one-match limit. SecItem's Copy rule gives us one owned result.
    let status = unsafe { SecItemCopyMatching(&query.dictionary, &raw mut result) };
    // SAFETY: a nonnull out-parameter is a +1 CF object produced by this Copy
    // call. It is adopted once, including on failure, and released by RAII.
    let owned = NonNull::new(result.cast_mut()).map(|p| unsafe { CFRetained::from_raw(p) });
    if status != ERR_SUCCESS {
        return Err(classify(status, operation));
    }
    let owned = owned.ok_or_else(|| failure(KeystoreErrorCode::OperatingSystem, operation))?;
    recover(&owned, operation)
}

fn recover(value: &CFType, operation: &'static str) -> Result<RecoveredSecret, KeystoreError> {
    // `downcast_ref` checks CFGetTypeID against CFDataGetTypeID. Never cast an
    // arbitrary result to CFData, or an immutable CFData to CFMutableData.
    let data = value
        .downcast_ref::<CFData>()
        .ok_or_else(|| failure(KeystoreErrorCode::OperatingSystem, operation))?;
    if data.is_empty() || data.len() > MAX_SECRET_BYTES {
        return Err(failure(KeystoreErrorCode::OperatingSystem, operation));
    }
    // This Rust-owned copy is wiped on drop. The immutable CF result and the
    // framework's IPC/internal copies cannot safely be overwritten here.
    Ok(RecoveredSecret::new(data.to_vec()))
}

#[allow(unsafe_code)]
fn delete(query: &Query, operation: &'static str) -> Result<PurgeOutcome, KeystoreError> {
    // SAFETY: all values live through the call. Class, provider service,
    // account, accessibility, no-sync and exact random generation restrict
    // the deletion; there is no return flag, search-list or wildcard query.
    let status = unsafe { SecItemDelete(&query.dictionary) };
    match status {
        ERR_SUCCESS => Ok(PurgeOutcome::Removed),
        ERR_ITEM_NOT_FOUND => Ok(PurgeOutcome::NothingStored),
        _ => Err(classify(status, operation)),
    }
}

pub(super) fn seal(
    label: &KeystoreLabel,
    secret: &[u8],
    operation: &'static str,
) -> Result<Vec<u8>, KeystoreError> {
    require_background_thread(operation)?;
    let mut generation = [0; macos_blob::GENERATION_BYTES];
    getrandom::fill(&mut generation)
        .map_err(|_| failure(KeystoreErrorCode::Unavailable, operation))?;
    let blob = encode_envelope(PROVIDER, &macos_blob::encode(label, &generation));
    autoreleasepool(|_| {
        let query = query(label, &generation, Some(secret), false, operation)?;
        add(&query, operation)?;
        Ok(blob)
    })
}

pub(super) fn open(
    label: &KeystoreLabel,
    payload: &[u8],
    operation: &'static str,
) -> Result<RecoveredSecret, KeystoreError> {
    let generation = macos_blob::decode(label, payload, operation)?;
    require_background_thread(operation)?;
    autoreleasepool(|_| copy(&query(label, generation, None, true, operation)?, operation))
}

pub(super) fn purge(
    label: &KeystoreLabel,
    payload: &[u8],
    operation: &'static str,
) -> Result<PurgeOutcome, KeystoreError> {
    let generation = macos_blob::decode(label, payload, operation)?;
    require_background_thread(operation)?;
    autoreleasepool(|_| {
        delete(
            &query(label, generation, None, false, operation)?,
            operation,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_native_result_type_and_length_are_checked_before_recovery() {
        let text = CFString::from_str("synthetic-not-key-data");
        assert!(recover(&text, "test").is_err());
        for length in [0, MAX_SECRET_BYTES + 1] {
            let data = CFData::from_bytes(&vec![0x37; length]);
            assert!(recover(&data, "test").is_err());
        }
    }

    #[test]
    fn macos_status_categories_do_not_disclose_native_objects() {
        for (status, code) in [
            (ERR_MISSING_ENTITLEMENT, KeystoreErrorCode::Unavailable),
            (ERR_NOT_AVAILABLE, KeystoreErrorCode::Unavailable),
            (ERR_INTERACTION_NOT_ALLOWED, KeystoreErrorCode::AccessDenied),
            (ERR_AUTH_FAILED, KeystoreErrorCode::AccessDenied),
            (ERR_DUPLICATE_ITEM, KeystoreErrorCode::DuplicateLabel),
            (ERR_ITEM_NOT_FOUND, KeystoreErrorCode::NotFound),
            (-50, KeystoreErrorCode::OperatingSystem),
        ] {
            let error = classify(status, "test");
            assert_eq!(error.code, code);
            assert_eq!(error.os_code, Some(i64::from(status)));
        }
    }
}
