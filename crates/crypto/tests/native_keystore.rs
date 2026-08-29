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
/// seal, reopen, reject a foreign label, reject a corrupted blob, and hold the
/// broker to its own revocation contract.
///
/// `purge_removes` is that contract, and the two hosts genuinely differ: a
/// stored-key broker removes an object, a stateless sealing broker has nothing
/// to remove. Passing it in keeps both branches unconditional, so neither host
/// can pass by skipping the assertion the other one runs.
fn native_roundtrip(prefix: &str, purge_removes: bool) {
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

    // The purge is issued exactly once and its outcome is asserted. An earlier
    // shape called `cleanup` first and then guarded the assertions on a second
    // purge reporting `Ok(true)`; that second purge reports `Ok(false)` on both
    // hosts — on Linux because the first one already removed the item, on
    // Windows because the broker is stateless — so every assertion inside the
    // guard was skipped on every platform.
    let purged = purge_device_key(&label, record.keystore_blob());
    let reopened = unlock_with_device(&record, PROFILE, &keystore);
    if purge_removes {
        assert!(
            matches!(purged, Ok(true)),
            "a stored-key broker must report the item removed, got {purged:?}"
        );
        // The key is gone, so the unlock fails closed rather than falling back
        // to anything weaker.
        let Err(error) = reopened else {
            unreachable!("a purged device key must not unlock the profile");
        };
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
    } else {
        // A stateless sealing broker stores nothing to remove and cannot revoke
        // a blob it already issued. ADR-005 carries that asymmetry in
        // `PurgeOutcome` rather than hiding it, so the test asserts it instead
        // of skipping: the purge reports nothing stored, and the blob still
        // opens for its owner.
        assert!(
            matches!(purged, Ok(false)),
            "a stateless broker must report nothing stored, got {purged:?}"
        );
        let Ok(still) = reopened else {
            unreachable!("a stateless broker's blob stays openable after a purge");
        };
        assert_eq!(still.expose_secret(), key.expose_secret());
    }

    cleanup(&label, &record);
}

/// Windows DPAPI-CNG (`NCryptProtectSecret`) seals and opens the device key.
#[cfg(windows)]
#[test]
fn windows_dpapi_roundtrip_native() {
    assert_eq!(PlatformKeystore::new().provider(), "WINDOWS_DPAPI_CNG");
    // DPAPI-CNG seals statelessly and stores nothing, so a purge removes
    // nothing and the issued blob stays openable.
    native_roundtrip("dpapi", false);
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
    // Secret Service stores the key in the default collection, so a purge
    // removes it and the next unlock must fail closed.
    native_roundtrip("secret-service", true);
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
