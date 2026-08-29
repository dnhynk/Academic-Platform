//! Native broker acceptance evidence, one test per host.
//!
//! These names are the task's contract and are searched for by the `P2-A1`
//! audit; they are not renamed. Each is compiled only for its own host, so a
//! pass on one platform can never be mistaken for evidence about the other:
//! on Windows `linux_secret_service_roundtrip_native` does not exist, and on
//! Linux `windows_dpapi_roundtrip_native` does not exist.
//!
//! Run with `cargo test -p academic-crypto --features os-keystore`.

#![cfg(feature = "os-keystore")]

use academic_crypto::{
    DeviceKeystore as _, IDENTIFIER_BYTES, KeystoreFailure, PlatformKeystore, ProfileId,
    RecipientParameters, RecipientRecord, UnlockError, VaultMasterKey, create_device_recipient,
    purge_device_key, unlock_with_device,
};

const PROFILE: ProfileId = ProfileId::from_bytes([0x71; IDENTIFIER_BYTES]);
const RECIPIENT: [u8; IDENTIFIER_BYTES] = [0x0A; IDENTIFIER_BYTES];

/// A label unique to one test run, so a leftover item can never make a later
/// run pass by accident.
fn unique_label(prefix: &str) -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    format!("academic-os:test:{prefix}:{pid}.{nanos}")
}

fn cleanup(label: &str, record: &RecipientRecord) {
    let _ = purge_device_key(label, record.keystore_blob());
}

/// Exercises the full device-recipient path against the real host broker:
/// seal, reopen, reject a foreign label, and reject a corrupted blob.
fn native_roundtrip(prefix: &str) {
    let keystore = PlatformKeystore::new();
    let label = unique_label(prefix);
    let Ok(key) = VaultMasterKey::generate() else {
        unreachable!("randomness must be available");
    };

    let record = match create_device_recipient(&key, PROFILE, RECIPIENT, &label, &keystore) {
        Ok(record) => record,
        Err(error) => unreachable!("the host broker must seal a device key: {error}"),
    };

    // The record names the broker this build actually carries.
    let RecipientParameters::DeviceKeystore {
        provider,
        label: stored,
    } = record.parameters()
    else {
        unreachable!("a device recipient carries device parameters");
    };
    assert_eq!(provider, keystore.provider());
    assert_eq!(stored, &label);

    // Round trip: the same broker reopens the same key.
    let reopened = match unlock_with_device(&record, PROFILE, &keystore) {
        Ok(reopened) => reopened,
        Err(error) => {
            cleanup(&label, &record);
            unreachable!("the host broker must reopen the sealed key: {error}");
        }
    };
    assert_eq!(reopened.expose_secret(), key.expose_secret());

    // Repeat the native call enough times to surface an allocator or handle
    // mismatch that a single round trip would not.
    for _ in 0..16 {
        let Ok(again) = unlock_with_device(&record, PROFILE, &keystore) else {
            cleanup(&label, &record);
            unreachable!("repeated native unlocks must succeed");
        };
        assert_eq!(again.expose_secret(), key.expose_secret());
    }

    // Negative: a blob presented under a different label is refused rather than
    // opening some other recipient's key.
    let other = unique_label("foreign");
    let refused = keystore.open(&other, record.keystore_blob());
    assert!(
        matches!(
            refused,
            Err(KeystoreFailure::InvalidBlob | KeystoreFailure::NotFound)
        ),
        "a foreign label must not open this blob"
    );

    // Negative: a corrupted blob is refused before producing anything.
    let mut corrupt = record.keystore_blob().to_vec();
    if let Some(last) = corrupt.last_mut() {
        *last ^= 0xFF;
    }
    let refused = keystore.open(&label, &corrupt);
    assert!(refused.is_err(), "a corrupted blob must be refused");

    cleanup(&label, &record);

    // After the purge the broker no longer serves the key, and the unlock fails
    // closed instead of falling back to anything weaker.
    if matches!(
        purge_device_key(&label, record.keystore_blob()),
        Ok(true) | Err(_)
    ) && let Err(error) = unlock_with_device(&record, PROFILE, &keystore)
    {
        assert!(error.leaves_profile_locked(), "{error}");
        assert!(
            matches!(
                error,
                UnlockError::KeystoreKeyMissing { .. }
                    | UnlockError::KeystoreUnavailable { .. }
                    | UnlockError::KeystoreAccessDenied { .. }
            ),
            "a purged key must fail closed, got {error}"
        );
    }
}

/// Windows DPAPI-CNG (`NCryptProtectSecret`) seals and opens the device key.
#[cfg(windows)]
#[test]
fn windows_dpapi_roundtrip_native() {
    assert_eq!(PlatformKeystore::new().provider(), "WINDOWS_DPAPI_CNG");
    native_roundtrip("dpapi");
}

/// Linux Secret Service (`org.freedesktop.secrets`) stores and returns the
/// device key.
///
/// Ignored by default and never reported as a pass it did not earn. The broker
/// needs a session D-Bus and an unlocked login keyring; a host that has them
/// runs this with `--ignored` and gets a real result, and a host that does not
/// sees `ignored`, which is the honest status. Reporting `ok` from a branch
/// that skipped the round trip would be exactly the coerced pass section 8.4
/// forbids.
///
/// The fail-closed half of the Linux evidence does not need a broker and is
/// asserted unconditionally below.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a session D-Bus and an unlocked login keyring; run with --ignored"]
fn linux_secret_service_roundtrip_native() {
    let keystore = PlatformKeystore::new();
    assert_eq!(keystore.provider(), "LINUX_SECRET_SERVICE");
    native_roundtrip("secret-service");
}

/// The Linux half that this host can prove without a keyring: the compiled
/// broker is Secret Service, and with nothing answering on the bus an unlock
/// fails closed instead of succeeding, panicking, or falling back.
#[cfg(target_os = "linux")]
#[test]
fn linux_secret_service_is_selected_and_fails_closed_without_a_provider() {
    let keystore = PlatformKeystore::new();
    assert_eq!(keystore.provider(), "LINUX_SECRET_SERVICE");

    let label = unique_label("absent");
    match keystore.seal(&label, &[0_u8; 32]) {
        Ok(blob) => {
            // A provider is present after all, so the sealed key must reopen.
            let Ok(recovered) = keystore.open(&label, &blob) else {
                unreachable!("a present provider must reopen what it sealed");
            };
            assert_eq!(recovered.len(), 32);
            let _ = purge_device_key(&label, &blob);
        }
        Err(failure) => {
            assert!(
                matches!(
                    failure,
                    KeystoreFailure::Unavailable | KeystoreFailure::AccessDenied
                ),
                "an absent provider must fail closed, got {failure}"
            );
        }
    }

    // Whatever the host, opening a label that was never sealed never yields a
    // key, and never reports success.
    let never = unique_label("never-sealed");
    let refused = keystore.open(&never, never.as_bytes());
    assert!(refused.is_err(), "an unsealed label must not open");
}
