//! The named `P2-K1` acceptance evidence.
//!
//! Test names are the task's contract and are searched for by the `P2-A1`
//! audit; they are not renamed. `windows_dpapi_roundtrip_native` and
//! `linux_secret_service_roundtrip_native` need a host broker and live in
//! `native_keystore.rs` behind the `os-keystore` feature;
//! `keystore_leaf_public_facade_exposes_no_raw_handle` inspects the FFI leaf
//! and lives in that crate's own test.

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use academic_crypto::{
    AUDIT_INFO, DeviceKeystore, DomainId, IDENTIFIER_BYTES, KEK_INFO_PREFIX, KEY_BYTES,
    KeystoreFailure, PINNED_PROFILES, ProfileId, RECIPIENT_MAC_INFO, RECOVERY_ARGON2ID_V1,
    RecipientParameters, RecipientRecord, RecipientSet, RecordError, RecoverySecret, STORE_INFO,
    UnlockError, UnlockThrottle, VaultMasterKey, create_device_recipient,
    create_recovery_recipient, unlock_with_device, unlock_with_recovery,
};
use ciborium::value::{Integer, Value};
use zeroize::Zeroize as _;

const PROFILE: ProfileId = ProfileId::from_bytes([0x51; IDENTIFIER_BYTES]);
const DEVICE_RECIPIENT: [u8; IDENTIFIER_BYTES] = [0x01; IDENTIFIER_BYTES];
const RECOVERY_RECIPIENT: [u8; IDENTIFIER_BYTES] = [0x02; IDENTIFIER_BYTES];
const LABEL: &str = "academic-os:device:test";

/// A broker that really stores the secret, standing in for a working host.
#[derive(Debug)]
struct MemoryKeystore {
    provider: &'static str,
    stored: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryKeystore {
    fn new() -> Self {
        Self {
            provider: "TEST_MEMORY_BROKER",
            stored: Mutex::new(HashMap::new()),
        }
    }
}

impl DeviceKeystore for MemoryKeystore {
    fn provider(&self) -> &str {
        self.provider
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored.insert(label.to_owned(), secret.to_vec());
        Ok(label.as_bytes().to_vec())
    }

    fn open(
        &self,
        label: &str,
        blob: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeystoreFailure> {
        if blob != label.as_bytes() {
            return Err(KeystoreFailure::InvalidBlob);
        }
        let stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored
            .get(label)
            .cloned()
            .map(zeroize::Zeroizing::new)
            .ok_or(KeystoreFailure::NotFound)
    }
}

/// A broker that always fails in one specific way.
#[derive(Debug)]
struct FailingKeystore(KeystoreFailure);

impl DeviceKeystore for FailingKeystore {
    fn provider(&self) -> &str {
        // The same broker the record names, now unavailable: that is `KY01`.
        // A *different* broker is a separate, separately asserted outcome.
        "TEST_MEMORY_BROKER"
    }

    fn seal(&self, _label: &str, _secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        Err(self.0)
    }

    fn open(
        &self,
        _label: &str,
        _blob: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeystoreFailure> {
        Err(self.0)
    }
}

/// A broker that succeeds but hands back the wrong key: `KY07`'s injection.
#[derive(Debug)]
struct WrongKeyKeystore;

impl DeviceKeystore for WrongKeyKeystore {
    fn provider(&self) -> &str {
        "TEST_WRONG_KEY_BROKER"
    }

    fn seal(&self, label: &str, _secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        Ok(label.as_bytes().to_vec())
    }

    fn open(
        &self,
        _label: &str,
        _blob: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeystoreFailure> {
        Ok(zeroize::Zeroizing::new(vec![0xEE_u8; KEY_BYTES]))
    }
}

fn master() -> VaultMasterKey {
    match VaultMasterKey::generate() {
        Ok(key) => key,
        Err(error) => unreachable!("randomness must be available: {error}"),
    }
}

fn device_record(keystore: &dyn DeviceKeystore, key: &VaultMasterKey) -> RecipientRecord {
    match create_device_recipient(key, PROFILE, DEVICE_RECIPIENT, LABEL, keystore) {
        Ok(record) => record,
        Err(error) => unreachable!("device recipient creation must succeed: {error}"),
    }
}

fn recovery_record(secret: &RecoverySecret, key: &VaultMasterKey) -> RecipientRecord {
    match create_recovery_recipient(
        key,
        PROFILE,
        RECOVERY_RECIPIENT,
        secret,
        RECOVERY_ARGON2ID_V1,
    ) {
        Ok(record) => record,
        Err(error) => unreachable!("recovery recipient creation must succeed: {error}"),
    }
}

/// Whether `needle` occurs anywhere in `haystack`.
fn contains_window(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn decode_record_value(record: &RecipientRecord) -> Vec<(Value, Value)> {
    let Ok(encoded) = record.to_canonical_cbor() else {
        unreachable!("a built record must encode");
    };
    let Ok(Value::Map(entries)) = ciborium::from_reader::<Value, _>(encoded.as_slice()) else {
        unreachable!("a record encodes as a CBOR map");
    };
    entries
}

fn reencode(entries: Vec<(Value, Value)>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if ciborium::into_writer(&Value::Map(entries), &mut bytes).is_err() {
        unreachable!("a decoded record must re-encode");
    }
    bytes
}

/// Replaces one integer-keyed field of a record and returns the new bytes.
fn with_field(record: &RecipientRecord, key: u64, value: Value) -> Vec<u8> {
    let mut entries = decode_record_value(record);
    for entry in &mut entries {
        let matches = entry
            .0
            .as_integer()
            .and_then(|integer| u64::try_from(integer).ok())
            .is_some_and(|integer| integer == key);
        if matches {
            entry.1 = value;
            return reencode(entries);
        }
    }
    unreachable!("field {key} must exist in a record");
}

fn field_bytes(record: &RecipientRecord, key: u64) -> Vec<u8> {
    let entries = decode_record_value(record);
    for (candidate, value) in entries {
        let matches = candidate
            .as_integer()
            .and_then(|integer| u64::try_from(integer).ok())
            .is_some_and(|integer| integer == key);
        if matches {
            let Some(bytes) = value.as_bytes() else {
                unreachable!("field {key} must be a byte string");
            };
            return bytes.clone();
        }
    }
    unreachable!("field {key} must exist in a record");
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// Nothing written for a profile contains the Vault Master Key, any key derived
/// from it, or the recovery secret -- only ciphertext and non-secret parameters.
#[test]
fn vmk_never_persisted_unwrapped() {
    let keystore = MemoryKeystore::new();
    let key = master();
    let secret = match RecoverySecret::generate() {
        Ok(secret) => secret,
        Err(error) => unreachable!("randomness must be available: {error}"),
    };

    let mut set = RecipientSet::new(PROFILE);
    set.push(device_record(&keystore, &key));
    set.push(recovery_record(&secret, &key));
    let Ok(document) = set.to_canonical_cbor() else {
        unreachable!("the recipient set must encode");
    };

    // The document is written exactly as the profile would hold it.
    let Ok(directory) = tempfile::tempdir() else {
        unreachable!("a temporary directory must be creatable");
    };
    let path = directory.path().join("recipients.cbor");
    if std::fs::write(&path, &document).is_err() {
        unreachable!("the recipient set must be writable");
    }
    let Ok(on_disk) = std::fs::read(&path) else {
        unreachable!("the recipient set must be readable");
    };
    assert_eq!(on_disk, document);

    let Ok(store_key) = key.derive_store_key(PROFILE) else {
        unreachable!("store derivation must succeed");
    };
    let Ok(audit_key) = key.derive_audit_key(PROFILE) else {
        unreachable!("audit derivation must succeed");
    };
    let Ok(mac_key) = key.derive_recipient_mac_key(PROFILE) else {
        unreachable!("MAC key derivation must succeed");
    };

    for (name, material) in [
        ("vault master key", key.expose_secret()),
        ("recovery secret", secret.expose_secret()),
        ("store key", store_key.expose_secret()),
        ("audit key", audit_key.expose_secret()),
        ("recipient mac key", mac_key.expose_secret()),
    ] {
        assert!(
            !contains_window(&on_disk, material),
            "{name} appeared in keys/recipients.cbor"
        );
    }

    // The document is genuinely usable, so the absence above is not the absence
    // of content: both recipients still open the same key.
    let Ok(reloaded) = RecipientSet::from_canonical_cbor(&on_disk) else {
        unreachable!("the written document must decode");
    };
    assert_eq!(reloaded.records().len(), 2);
    let Ok(reopened) = unlock_with_device(&reloaded.records()[0], PROFILE, &keystore) else {
        unreachable!("the device recipient must reopen the key");
    };
    assert_eq!(reopened.expose_secret(), key.expose_secret());
}

/// A broker returning a wrong key, and a record whose MAC alone is wrong, are
/// both refused as integrity incidents with no plaintext produced.
#[test]
fn recipient_record_mac_detects_wrong_keystore_key() {
    let key = master();

    // (a) The broker succeeds but hands back a key that is not the one sealed.
    let wrong = WrongKeyKeystore;
    let record = device_record(&wrong, &key);
    let Err(error) = unlock_with_device(&record, PROFILE, &wrong) else {
        unreachable!("a wrong broker key must not open the record");
    };
    assert_eq!(
        error,
        UnlockError::KeystoreKeyRejected {
            provider: "TEST_WRONG_KEY_BROKER".to_owned()
        }
    );
    assert!(error.is_integrity_incident());
    assert!(error.leaves_profile_locked());

    // (b) The MAC field alone is corrupted. The wrapping key is correct and the
    //     AEAD opens, so the record MAC is the only thing that can detect this
    //     -- which is exactly what ADR-005 asks the MAC to do.
    let keystore = MemoryKeystore::new();
    let good = device_record(&keystore, &key);
    let Ok(unchanged) = unlock_with_device(&good, PROFILE, &keystore) else {
        unreachable!("an intact record must open");
    };
    assert_eq!(unchanged.expose_secret(), key.expose_secret());

    let mut mac = field_bytes(&good, 10);
    assert_eq!(mac.len(), 64);
    mac[0] ^= 0x01;
    let tampered_bytes = with_field(&good, 10, Value::Bytes(mac));
    let Ok(tampered) = RecipientRecord::from_canonical_cbor(&tampered_bytes) else {
        unreachable!("a MAC-only edit stays structurally valid");
    };
    let Err(error) = unlock_with_device(&tampered, PROFILE, &keystore) else {
        unreachable!("a corrupted record MAC must refuse the unlock");
    };
    assert_eq!(error, UnlockError::RecordIntegrity);
    assert!(error.is_integrity_incident());
    assert!(error.leaves_profile_locked());

    // (c) A tampered *parameter* is caught earlier still, by the AEAD's
    //     associated data, so no plaintext is produced before the MAC runs.
    let swapped = with_field(&good, 2, Value::Bytes(vec![0x77_u8; IDENTIFIER_BYTES]));
    let Ok(swapped_record) = RecipientRecord::from_canonical_cbor(&swapped) else {
        unreachable!("a recipient-id edit stays structurally valid");
    };
    let Err(error) = unlock_with_device(&swapped_record, PROFILE, &keystore) else {
        unreachable!("a tampered identity field must refuse the unlock");
    };
    assert!(error.is_integrity_incident(), "{error}");
}

/// Each purpose and each domain gets its own key, and the info strings are the
/// exact ADR-005 literals.
#[test]
fn hkdf_domain_separation_is_exact() {
    let key = master();
    let other_profile = ProfileId::from_bytes([0x52; IDENTIFIER_BYTES]);
    let first = DomainId::from_bytes([0xA1; IDENTIFIER_BYTES]);
    let second = DomainId::from_bytes([0xA2; IDENTIFIER_BYTES]);

    assert_eq!(KEK_INFO_PREFIX, b"academic-os/kek/v1");
    assert_eq!(STORE_INFO, b"academic-os/store/v1");
    assert_eq!(AUDIT_INFO, b"academic-os/audit/v1");
    assert_eq!(RECIPIENT_MAC_INFO, b"academic-os/recipient-mac/v1");

    let (Ok(kek_first), Ok(kek_second)) = (
        key.derive_domain_kek(PROFILE, first),
        key.derive_domain_kek(PROFILE, second),
    ) else {
        unreachable!("KEK derivation must succeed");
    };
    let (Ok(store), Ok(audit), Ok(mac)) = (
        key.derive_store_key(PROFILE),
        key.derive_audit_key(PROFILE),
        key.derive_recipient_mac_key(PROFILE),
    ) else {
        unreachable!("purpose derivation must succeed");
    };

    let purposes = [
        ("kek/a1", kek_first.expose_secret()),
        ("kek/a2", kek_second.expose_secret()),
        ("store", store.expose_secret()),
        ("audit", audit.expose_secret()),
        ("recipient-mac", mac.expose_secret()),
    ];
    for (index, (left_name, left)) in purposes.iter().enumerate() {
        for (right_name, right) in purposes.iter().skip(index + 1) {
            assert_ne!(left, right, "{left_name} collided with {right_name}");
        }
    }

    // The profile identity is a real salt: every purpose changes with it.
    let (Ok(other_kek), Ok(other_store), Ok(other_audit), Ok(other_mac)) = (
        key.derive_domain_kek(other_profile, first),
        key.derive_store_key(other_profile),
        key.derive_audit_key(other_profile),
        key.derive_recipient_mac_key(other_profile),
    ) else {
        unreachable!("derivation under another profile must succeed");
    };
    assert_ne!(other_kek.expose_secret(), kek_first.expose_secret());
    assert_ne!(other_store.expose_secret(), store.expose_secret());
    assert_ne!(other_audit.expose_secret(), audit.expose_secret());
    assert_ne!(other_mac.expose_secret(), mac.expose_secret());

    // Derivation is a function: the same inputs give the same key.
    let Ok(again) = key.derive_domain_kek(PROFILE, first) else {
        unreachable!("KEK derivation must succeed");
    };
    assert_eq!(again.expose_secret(), kek_first.expose_secret());
}

/// `KY01`: an unavailable broker leaves the profile locked with an actionable
/// reason and no fallback to a weaker key.
#[test]
fn keystore_unavailable_fails_closed_locked() {
    let key = master();
    let working = MemoryKeystore::new();
    let record = device_record(&working, &key);

    for (failure, expected) in [
        (
            KeystoreFailure::Unavailable,
            UnlockError::KeystoreUnavailable {
                provider: "TEST_MEMORY_BROKER".to_owned(),
                label: LABEL.to_owned(),
            },
        ),
        (
            KeystoreFailure::NotFound,
            UnlockError::KeystoreKeyMissing {
                provider: "TEST_MEMORY_BROKER".to_owned(),
                label: LABEL.to_owned(),
            },
        ),
        (
            KeystoreFailure::AccessDenied,
            UnlockError::KeystoreAccessDenied {
                provider: "TEST_MEMORY_BROKER".to_owned(),
            },
        ),
        (
            KeystoreFailure::Unsupported,
            UnlockError::KeystoreUnavailable {
                provider: "TEST_MEMORY_BROKER".to_owned(),
                label: LABEL.to_owned(),
            },
        ),
    ] {
        let broken = FailingKeystore(failure);
        let Err(error) = unlock_with_device(&record, PROFILE, &broken) else {
            unreachable!("{failure:?} must not produce a key");
        };
        assert_eq!(error, expected, "{failure:?}");
        assert!(error.leaves_profile_locked(), "{failure:?}");
        // An outage is not an integrity incident: it names something to fix.
        assert!(!error.is_integrity_incident(), "{failure:?}");
        let rendered = error.to_string();
        assert!(rendered.contains("stays locked"), "{rendered}");
        assert!(rendered.contains("TEST_MEMORY_BROKER"), "{rendered}");
    }

    // A record sealed under another broker is refused as a mismatch rather than
    // asked of the wrong broker and reported as an outage.
    let other = WrongKeyKeystore;
    let Err(error) = unlock_with_device(&record, PROFILE, &other) else {
        unreachable!("a foreign broker must not open this record");
    };
    assert_eq!(
        error,
        UnlockError::KeystoreProviderMismatch {
            expected: "TEST_MEMORY_BROKER".to_owned(),
            actual: "TEST_WRONG_KEY_BROKER".to_owned(),
        }
    );
    assert!(error.leaves_profile_locked());

    // Creating a recipient against an unavailable broker fails the same way,
    // rather than silently writing a record with no key behind it.
    let broken = FailingKeystore(KeystoreFailure::Unavailable);
    let Err(error) = create_device_recipient(&key, PROFILE, DEVICE_RECIPIENT, LABEL, &broken)
    else {
        unreachable!("recipient creation must fail with no broker");
    };
    assert!(error.leaves_profile_locked());
}

/// The Argon2id profile is versioned, written into the record verbatim, read
/// back on unlock, and cannot be downgraded or invented by editing the record.
#[test]
fn argon2id_profile_is_versioned_and_pinned() {
    let key = master();
    let Ok(secret) = RecoverySecret::generate() else {
        unreachable!("randomness must be available");
    };
    let record = recovery_record(&secret, &key);

    // Read-back: the parameters in the record are exactly the pinned profile.
    let RecipientParameters::RecoverySecret { profile, salt } = record.parameters() else {
        unreachable!("a recovery recipient carries recovery parameters");
    };
    assert_eq!(*profile, RECOVERY_ARGON2ID_V1);
    assert_eq!(profile.identifier, "RECOVERY_ARGON2ID_V1");
    assert_eq!(profile.memory_kib, 65_536);
    assert_eq!(profile.iterations, 3);
    assert_eq!(profile.parallelism, 1);
    assert_eq!(salt.len(), IDENTIFIER_BYTES);
    assert_eq!(PINNED_PROFILES, &[RECOVERY_ARGON2ID_V1]);

    // Read-back survives a persist/reload round trip.
    let Ok(encoded) = record.to_canonical_cbor() else {
        unreachable!("the record must encode");
    };
    let Ok(reloaded) = RecipientRecord::from_canonical_cbor(&encoded) else {
        unreachable!("the record must decode");
    };
    assert_eq!(reloaded.parameters(), record.parameters());

    // A record edited on disk cannot weaken the cost or name another profile.
    let downgrades = [
        (1_u64, Value::Integer(Integer::from(8_u32))),
        (2, Value::Integer(Integer::from(1_u32))),
        (3, Value::Integer(Integer::from(16_u32))),
        (0, Value::Text("RECOVERY_ARGON2ID_V0".to_owned())),
    ];
    for (parameter_key, replacement) in downgrades {
        let mut entries = decode_record_value(&record);
        let mut replaced = false;
        for entry in &mut entries {
            let is_parameters = entry
                .0
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
                .is_some_and(|integer| integer == 5);
            if !is_parameters {
                continue;
            }
            let Value::Map(parameters) = &mut entry.1 else {
                unreachable!("parameters are a CBOR map");
            };
            for parameter in parameters.iter_mut() {
                let matches = parameter
                    .0
                    .as_integer()
                    .and_then(|integer| u64::try_from(integer).ok())
                    .is_some_and(|integer| integer == parameter_key);
                if matches {
                    parameter.1 = replacement.clone();
                    replaced = true;
                }
            }
        }
        assert!(replaced, "parameter {parameter_key} must exist");
        let edited = reencode(entries);
        assert_eq!(
            RecipientRecord::from_canonical_cbor(&edited),
            Err(RecordError::UnpinnedKdfProfile),
            "parameter {parameter_key} was accepted after an edit"
        );
    }

    // `KY06`: a wrong recovery secret is refused identically every time, is not
    // an integrity incident, and is rate limited. Nothing in the API or the
    // outcome says anything about an individual word of a phrase.
    let mut throttle = UnlockThrottle::new();
    let first_wrong = RecoverySecret::from_entropy([0x01; KEY_BYTES]);
    let second_wrong = RecoverySecret::from_entropy([0x02; KEY_BYTES]);
    let Err(first) = unlock_with_recovery(&record, PROFILE, &first_wrong, &mut throttle, 0) else {
        unreachable!("a wrong secret must not open the record");
    };
    assert_eq!(first, UnlockError::WrongRecoverySecret);
    assert!(!first.is_integrity_incident());
    let Err(second) = unlock_with_recovery(&record, PROFILE, &second_wrong, &mut throttle, 0)
    else {
        unreachable!("a wrong secret must not open the record");
    };
    assert!(
        matches!(second, UnlockError::RateLimited { .. }),
        "{second}"
    );

    // The correct secret still opens it once the wait has elapsed.
    let Ok(opened) = unlock_with_recovery(&record, PROFILE, &secret, &mut throttle, 60_000) else {
        unreachable!("the correct secret must open the record");
    };
    assert_eq!(opened.expose_secret(), key.expose_secret());
    assert_eq!(throttle.consecutive_failures(), 0);
}

/// Every key type clears its buffer, and the marker trait that drives drop-time
/// zeroization is present on all of them.
///
/// What this proves: each key type implements `ZeroizeOnDrop` (checked by the
/// compiler through the bound below) and the `Zeroize` implementation that its
/// drop glue runs really clears the material.
///
/// What it does not prove: that the freed allocation is observably zero after
/// the value is dropped. Reading freed memory is undefined behaviour, so this
/// crate does not attempt it; the guarantee comes from `Zeroizing`'s drop
/// implementation, which the bound and the explicit call below exercise.
#[test]
fn key_material_is_zeroized_on_drop() {
    const fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
    assert_zeroize_on_drop::<VaultMasterKey>();
    assert_zeroize_on_drop::<academic_crypto::DomainKek>();
    assert_zeroize_on_drop::<academic_crypto::StoreKey>();
    assert_zeroize_on_drop::<academic_crypto::AuditKey>();
    assert_zeroize_on_drop::<academic_crypto::ArtifactDek>();
    assert_zeroize_on_drop::<academic_crypto::DeviceWrappingKey>();
    assert_zeroize_on_drop::<RecoverySecret>();
    assert_zeroize_on_drop::<academic_crypto::RecipientMacKey>();
    assert_zeroize_on_drop::<academic_crypto::RecipientWrapKey>();

    let mut key = master();
    assert_ne!(key.expose_secret(), &[0_u8; KEY_BYTES]);
    key.zeroize();
    assert_eq!(key.expose_secret(), &[0_u8; KEY_BYTES]);

    let mut secret = RecoverySecret::from_entropy([0x5A; KEY_BYTES]);
    assert_eq!(secret.expose_secret(), &[0x5A; KEY_BYTES]);
    secret.zeroize();
    assert_eq!(secret.expose_secret(), &[0_u8; KEY_BYTES]);

    // The rendered SQLCipher key is zeroizing too, so the hex text is cleared
    // rather than left in a plain `String`.
    let derived = master();
    let Ok(store) = derived.derive_store_key(PROFILE) else {
        unreachable!("store derivation must succeed");
    };
    let rendered = store.expose_raw_hex();
    assert_eq!(rendered.len(), 64);
}

/// No key byte, and no hex spelling of one, reaches a rendered error, a debug
/// line, or the persisted recipient document.
#[test]
fn no_key_bytes_in_logs_audit_or_export() {
    let keystore = MemoryKeystore::new();
    let key = master();
    let Ok(secret) = RecoverySecret::generate() else {
        unreachable!("randomness must be available");
    };
    let device = device_record(&keystore, &key);
    let recovery = recovery_record(&secret, &key);

    let Ok(store) = key.derive_store_key(PROFILE) else {
        unreachable!("store derivation must succeed");
    };
    let Ok(audit) = key.derive_audit_key(PROFILE) else {
        unreachable!("audit derivation must succeed");
    };
    let Ok(kek) = key.derive_domain_kek(PROFILE, DomainId::from_bytes([9; IDENTIFIER_BYTES]))
    else {
        unreachable!("KEK derivation must succeed");
    };

    let materials: Vec<(&str, Vec<u8>)> = vec![
        ("vault master key", key.expose_secret().to_vec()),
        ("recovery secret", secret.expose_secret().to_vec()),
        ("store key", store.expose_secret().to_vec()),
        ("audit key", audit.expose_secret().to_vec()),
        ("domain kek", kek.expose_secret().to_vec()),
    ];

    // Every rendering a log, audit row, or report could plausibly capture.
    let mut renderings = vec![
        format!("{key:?}"),
        format!("{secret:?}"),
        format!("{store:?}"),
        format!("{audit:?}"),
        format!("{kek:?}"),
        format!("{device:?}"),
        format!("{recovery:?}"),
    ];
    for error in [
        UnlockError::KeystoreUnavailable {
            provider: "P".to_owned(),
            label: LABEL.to_owned(),
        },
        UnlockError::KeystoreKeyMissing {
            provider: "P".to_owned(),
            label: LABEL.to_owned(),
        },
        UnlockError::KeystoreAccessDenied {
            provider: "P".to_owned(),
        },
        UnlockError::KeystoreKeyRejected {
            provider: "P".to_owned(),
        },
        UnlockError::WrongRecoverySecret,
        UnlockError::RecordIntegrity,
        UnlockError::RateLimited { retry_after_ms: 5 },
        UnlockError::RecipientKindMismatch,
        UnlockError::ProfileMismatch,
        UnlockError::KeySchedule,
        UnlockError::Randomness,
        UnlockError::Record(RecordError::NonCanonical),
    ] {
        renderings.push(error.to_string());
        renderings.push(format!("{error:?}"));
    }

    for rendering in &renderings {
        assert!(
            !rendering.contains("expose_secret"),
            "a rendering leaked an accessor: {rendering}"
        );
        for (name, material) in &materials {
            assert!(
                !contains_window(rendering.as_bytes(), material),
                "{name} appeared verbatim in a rendering: {rendering}"
            );
            assert!(
                !rendering.contains(&hex::encode(material)),
                "{name} appeared as hex in a rendering: {rendering}"
            );
        }
    }

    // The key types name themselves and redact their contents.
    assert!(format!("{key:?}").contains("<redacted>"));
    assert!(format!("{key:?}").contains("VaultMasterKey"));

    // The exported document carries no key material either.
    let mut set = RecipientSet::new(PROFILE);
    set.push(device);
    set.push(recovery);
    let Ok(document) = set.to_canonical_cbor() else {
        unreachable!("the recipient set must encode");
    };
    for (name, material) in &materials {
        assert!(
            !contains_window(&document, material),
            "{name} appeared in the exported recipient set"
        );
    }
}

// ---------------------------------------------------------------------------
// Supporting contracts
// ---------------------------------------------------------------------------

/// `KY06`'s structural half: the crate offers no word-level entry point, so no
/// API can be asked about, or answer with, an individual word of a phrase.
#[test]
fn no_public_api_accepts_or_reports_a_single_recovery_word() {
    let source = include_str!("../src/lib.rs");
    let keys_source = include_str!("../src/keys.rs");
    let recovery_source = include_str!("../src/recovery.rs");
    for haystack in [source, keys_source, recovery_source] {
        for forbidden in ["fn word", "word_index", "words(", "wordlist", "phrase_word"] {
            assert!(
                !haystack.contains(forbidden),
                "a word-level entry point named {forbidden} exists"
            );
        }
    }
    // The only recovery input is a whole 256-bit secret.
    assert!(recovery_source.contains("no word-level entry point"));
    assert_eq!(
        RecoverySecret::from_entropy([0; KEY_BYTES])
            .expose_secret()
            .len(),
        KEY_BYTES
    );
}

/// A recipient document must be the one canonical encoding.
#[test]
fn recipient_documents_are_canonical_deterministic_cbor() {
    let keystore = MemoryKeystore::new();
    let key = master();
    let record = device_record(&keystore, &key);
    let Ok(encoded) = record.to_canonical_cbor() else {
        unreachable!("the record must encode");
    };
    let Ok(decoded) = RecipientRecord::from_canonical_cbor(&encoded) else {
        unreachable!("the record must decode");
    };
    let Ok(reencoded) = decoded.to_canonical_cbor() else {
        unreachable!("the record must re-encode");
    };
    assert_eq!(encoded, reencoded);

    // A non-canonical spelling of the same map is refused rather than accepted.
    let mut entries = decode_record_value(&record);
    entries.reverse();
    let reordered = reencode(entries);
    assert_eq!(
        RecipientRecord::from_canonical_cbor(&reordered),
        Err(RecordError::Shape)
    );

    let mut truncated = encoded.clone();
    truncated.pop();
    assert_eq!(
        RecipientRecord::from_canonical_cbor(&truncated),
        Err(RecordError::Malformed)
    );
}
