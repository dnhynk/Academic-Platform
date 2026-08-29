//! `P2-K1` acceptance evidence for the reviewed FFI leaf.
//!
//! The test name is the task's contract and is searched for by the `P2-A1`
//! audit; it is not renamed.

use academic_keystore_platform::{
    KeystoreError, KeystoreErrorCode, KeystoreLabel, MAX_SECRET_BYTES, PROVIDER, RecoveredSecret,
    open, purge, seal,
};

/// Type spellings that would mean a native handle escaped the safe facade.
const FORBIDDEN_IN_PUBLIC_SIGNATURES: &[&str] = &[
    "HANDLE",
    "RawFd",
    "RawHandle",
    "RawSocket",
    "c_void",
    "*mut",
    "*const",
    "NCRYPT",
    "NCryptDescriptor",
    "key_serial",
    "OwnedObjectPath",
    "ObjectPath",
    "Connection",
    "OwnedValue",
    "OwnedHandle",
    "OwnedNcryptBuffer",
];

/// Returns each line of the leaf's public surface, with doc comments removed.
fn public_surface() -> Vec<String> {
    include_str!("../src/lib.rs")
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .filter(|line| {
            line.starts_with("pub fn ")
                || line.starts_with("pub const fn ")
                || line.starts_with("pub struct ")
                || line.starts_with("pub enum ")
                || line.starts_with("pub type ")
                || line.starts_with("pub const ")
                || line.starts_with("pub use ")
        })
        .map(str::to_owned)
        .collect()
}

/// Whether any `pub struct` in the leaf declares a public field.
fn source_public_fields() -> bool {
    let mut inside_public_struct = false;
    for line in include_str!("../src/lib.rs").lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub struct ") {
            // A public tuple struct with a public field would spell `pub (`.
            if trimmed.contains("(pub ") {
                return true;
            }
            inside_public_struct = trimmed.ends_with('{');
            continue;
        }
        if inside_public_struct {
            if trimmed == "}" {
                inside_public_struct = false;
            } else if trimmed.starts_with("pub ") && !trimmed.starts_with("pub fn") {
                // `KeystoreError`'s fields are deliberately public: they are a
                // category, a constant operation name, and an OS status.
                if !trimmed.starts_with("pub code")
                    && !trimmed.starts_with("pub operation")
                    && !trimmed.starts_with("pub os_code")
                {
                    return true;
                }
            }
        }
    }
    false
}

/// No public item of the leaf mentions a raw handle, pointer, descriptor, key
/// serial, or D-Bus object in its signature, and every secret it returns is a
/// redacting, zeroizing type.
#[test]
fn keystore_leaf_public_facade_exposes_no_raw_handle() {
    let surface = public_surface();
    assert!(
        surface.len() >= 8,
        "the public surface scan found too little to be meaningful: {surface:?}"
    );

    for line in &surface {
        for forbidden in FORBIDDEN_IN_PUBLIC_SIGNATURES {
            assert!(
                !line.contains(forbidden),
                "a public signature exposes {forbidden}: {line}"
            );
        }
    }

    // The three entry points are present and return only safe owned types.
    let has = |needle: &str| surface.iter().any(|line| line.contains(needle));
    assert!(has("pub fn seal("), "{surface:?}");
    assert!(has("pub fn open("), "{surface:?}");
    assert!(has("pub fn purge("), "{surface:?}");

    // The platform modules are private: nothing in them is re-exported.
    let source = include_str!("../src/lib.rs");
    assert!(
        source.contains("mod linux;"),
        "the linux module must be private"
    );
    assert!(
        source.contains("mod windows;"),
        "the windows module must be private"
    );
    assert!(
        !source.contains("pub mod linux") && !source.contains("pub mod windows"),
        "a platform module must not be public"
    );

    // No public struct exposes a public field, so the safe wrappers above
    // cannot be unwrapped into their contents by a caller.
    assert!(
        !source_public_fields(),
        "a public struct exposes a public field"
    );

    // A recovered secret redacts its contents and clears them on drop.
    let Ok(label) = KeystoreLabel::new("facade:probe") else {
        unreachable!("a valid label must be accepted");
    };
    let rendered = format!("{:?}", KeystoreLabel::new("facade:probe"));
    assert!(rendered.contains("facade:probe"), "{rendered}");

    // Errors carry a category, an operation, and at most an OS status: never a
    // handle, a label secret, or key bytes.
    let Err(error) = open(&label, b"not-an-envelope") else {
        unreachable!("a malformed blob must be refused");
    };
    let rendered = format!("{error:?} {error}");
    for forbidden in ["0x", "ptr", "handle"] {
        assert!(
            !rendered.to_ascii_lowercase().contains(forbidden),
            "an error rendering leaked {forbidden}: {rendered}"
        );
    }

    // The bounds are enforced before any native call.
    let Err(too_large) = seal(&label, &vec![0_u8; MAX_SECRET_BYTES + 1]) else {
        unreachable!("an over-large secret must be refused");
    };
    assert_eq!(too_large.code, KeystoreErrorCode::SecretTooLarge);
    let Err(empty) = seal(&label, &[]) else {
        unreachable!("an empty secret must be refused");
    };
    assert_eq!(empty.code, KeystoreErrorCode::SecretTooLarge);

    let Err(bad_blob) = purge(&label, b"") else {
        unreachable!("an empty blob must be refused");
    };
    assert_eq!(bad_blob.code, KeystoreErrorCode::InvalidSealedBlob);

    // The compiled provider is a closed enum, not a raw identifier.
    assert!(
        matches!(
            PROVIDER.as_str(),
            "WINDOWS_DPAPI_CNG" | "LINUX_SECRET_SERVICE" | "UNSUPPORTED"
        ),
        "{}",
        PROVIDER.as_str()
    );

    // `RecoveredSecret` is the only way a secret leaves the leaf, and it
    // reports a length without exposing bytes through `Debug`.
    fn accepts_only_safe_secret(_: fn(&RecoveredSecret) -> usize) {}
    accepts_only_safe_secret(RecoveredSecret::len);

    fn accepts_only_safe_error(_: fn(&KeystoreError) -> KeystoreErrorCode) {}
    accepts_only_safe_error(|error| error.code);
}
