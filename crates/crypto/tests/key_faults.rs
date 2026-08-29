//! The `KY` fault rows `P2-K1` owns, driven as real process terminations.
//!
//! `KY01`, `KY06`, and `KY07` are error-induced and are asserted in
//! `key_hierarchy.rs` through the broker seam. `KY02` and `KY08` are
//! kill-induced: the two failpoints below abort the process at an exact point,
//! and the parent then inspects what the dead child left behind.
//!
//! Each fault is run twice. The armed run must abort and leave nothing; the
//! unarmed control run must complete and write the file. Without the control
//! run "nothing was written" would also hold for a child that never got that
//! far, and the test would prove nothing.
//!
//! Run with
//! `cargo test -p academic-crypto --features phase2-fault-injection --test key_faults`.

#![cfg(feature = "phase2-fault-injection")]

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, PoisonError},
};

use academic_crypto::{
    DeviceKeystore, FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE, IDENTIFIER_BYTES,
    KEY_BYTES, KeystoreFailure, PHASE2_KEY_FAULT_IDS, ProfileId, VaultMasterKey,
    create_device_recipient, unlock_with_device,
};

const PROFILE: ProfileId = ProfileId::from_bytes([0x61; IDENTIFIER_BYTES]);
const RECIPIENT: [u8; IDENTIFIER_BYTES] = [0x0B; IDENTIFIER_BYTES];
const LABEL: &str = "academic-os:fault:device";

/// Directory the child is told to work in.
const CHILD_DIRECTORY: &str = "ACADEMIC_CRYPTO_TEST_CHILD_DIR";

/// File the child writes *before* the guarded call, proving it reached it.
const PROGRESS_FILE: &str = "progress.txt";
/// File the child writes *after* the guarded call. It must never exist when
/// the failpoint fired.
const KY02_OUTPUT: &str = "unlocked.key";
const KY08_OUTPUT: &str = "recipients.cbor";

/// A broker that really holds what it was given, so an unarmed child succeeds.
#[derive(Debug)]
struct RecordingKeystore(Mutex<Option<Vec<u8>>>);

impl RecordingKeystore {
    const fn new() -> Self {
        Self(Mutex::new(None))
    }
}

impl DeviceKeystore for RecordingKeystore {
    fn provider(&self) -> &str {
        "TEST_RECORDING_BROKER"
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        *held = Some(secret.to_vec());
        Ok(label.as_bytes().to_vec())
    }

    fn open(
        &self,
        _label: &str,
        _blob: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeystoreFailure> {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.clone()
            .map(zeroize::Zeroizing::new)
            .ok_or(KeystoreFailure::NotFound)
    }
}

fn child_directory() -> Option<PathBuf> {
    std::env::var(CHILD_DIRECTORY).ok().map(PathBuf::from)
}

/// Every regular file under `root`, recursively.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn file_names(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = files_under(root)
        .iter()
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    names
}

struct ChildRun {
    directory: tempfile::TempDir,
    workspace: PathBuf,
    marker: PathBuf,
    aborted: bool,
}

/// Runs the child once, arming `fault` only when `armed` is set.
fn run_child(fault: &str, child_test: &str, armed: bool) -> ChildRun {
    let Ok(directory) = tempfile::tempdir() else {
        unreachable!("a temporary directory must be creatable");
    };
    let Ok(executable) = std::env::current_exe() else {
        unreachable!("the test executable path must be readable");
    };
    let marker = directory.path().join("fault-ready.marker");
    let workspace = directory.path().join("profile");
    if std::fs::create_dir(&workspace).is_err() {
        unreachable!("the child workspace must be creatable");
    }

    let mut command = Command::new(executable);
    command
        .args(["--exact", child_test, "--nocapture", "--ignored"])
        .env(CHILD_DIRECTORY, &workspace);
    if armed {
        command
            .env(FAULT_SELECTION_VARIABLE, fault)
            .env(FAULT_READY_MARKER_VARIABLE, &marker);
    } else {
        command
            .env_remove(FAULT_SELECTION_VARIABLE)
            .env_remove(FAULT_READY_MARKER_VARIABLE);
    }
    let Ok(output) = command.output() else {
        unreachable!("the child process must start");
    };

    ChildRun {
        directory,
        workspace,
        marker,
        aborted: !output.status.success(),
    }
}

/// Asserts the armed child aborted exactly at `fault` after reaching the call.
fn assert_armed(run: &ChildRun, fault: &str, output_file: &str) {
    assert!(
        run.aborted,
        "the child must not exit normally after taking {fault}"
    );
    assert!(
        run.marker.exists(),
        "the child must reach the {fault} failpoint"
    );
    let Ok(recorded) = std::fs::read_to_string(&run.marker) else {
        unreachable!("the ready marker must be readable");
    };
    assert_eq!(recorded, fault);

    // It reached the guarded call...
    assert!(
        run.workspace.join(PROGRESS_FILE).exists(),
        "the child did not reach the guarded call, so {fault} proves nothing"
    );
    // ...and produced nothing beyond that marker of progress.
    assert_eq!(
        file_names(&run.workspace),
        vec![PROGRESS_FILE.to_owned()],
        "the child left more than its progress marker after {fault}"
    );
    assert!(
        !run.workspace.join(output_file).exists(),
        "{output_file} survived {fault}"
    );

    // Nothing anywhere under the child's tree holds 32 bytes of key material.
    for path in files_under(run.directory.path()) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        assert!(
            bytes.len() < KEY_BYTES || path.ends_with(PROGRESS_FILE),
            "an unexpected {}-byte file survived {fault}: {}",
            bytes.len(),
            path.display()
        );
    }
}

/// Asserts the unarmed control child really would have written the output.
fn assert_control(run: &ChildRun, output_file: &str) {
    assert!(
        !run.aborted,
        "the control child must complete when no fault is armed"
    );
    let produced = run.workspace.join(output_file);
    assert!(
        produced.exists(),
        "the control child wrote nothing, so the armed assertion would be vacuous"
    );
    let Ok(bytes) = std::fs::read(&produced) else {
        unreachable!("the control output must be readable");
    };
    assert!(!bytes.is_empty(), "the control output is empty");
}

/// `KY02`: a termination between the broker returning the wrapping key and the
/// VMK becoming available leaves no key material behind.
#[test]
fn ky02_kill_during_vmk_unwrap_leaves_no_key_material() {
    let control = run_child("KY02", "ky02_child", false);
    assert_control(&control, KY02_OUTPUT);
    let Ok(bytes) = std::fs::read(control.workspace.join(KY02_OUTPUT)) else {
        unreachable!("the control output must be readable");
    };
    assert_eq!(
        bytes.len(),
        KEY_BYTES,
        "the control child must write a real recovered key"
    );

    let armed = run_child("KY02", "ky02_child", true);
    assert_armed(&armed, "KY02", KY02_OUTPUT);
}

/// `KY08`: a termination during first-run key generation leaves no profile at
/// all, so there is nothing partial for a later start to misread.
#[test]
fn ky08_kill_during_first_run_key_generation_leaves_no_profile() {
    let control = run_child("KY08", "ky08_child", false);
    assert_control(&control, KY08_OUTPUT);

    let armed = run_child("KY08", "ky08_child", true);
    assert_armed(&armed, "KY08", KY08_OUTPUT);

    // What remains is removable without a repair step: one progress marker.
    if std::fs::remove_file(armed.workspace.join(PROGRESS_FILE)).is_ok() {
        assert!(
            std::fs::remove_dir(&armed.workspace).is_ok(),
            "the abandoned profile directory must be safely removable"
        );
    }
}

/// The rows this task owns are exactly the ones it implements.
#[test]
fn owned_fault_rows_are_the_declared_set() {
    assert_eq!(
        PHASE2_KEY_FAULT_IDS,
        &["KY01", "KY02", "KY06", "KY07", "KY08"]
    );
}

// ---------------------------------------------------------------------------
// Child roles. `#[ignore]` keeps them out of an ordinary run; they do nothing
// unless the harness set the child environment.
// ---------------------------------------------------------------------------

fn mark_progress(workspace: &Path) {
    let _ = std::fs::write(workspace.join(PROGRESS_FILE), b"reached guarded call");
}

#[test]
#[ignore = "child role driven by ky02_kill_during_vmk_unwrap_leaves_no_key_material"]
fn ky02_child() {
    let Some(workspace) = child_directory() else {
        return;
    };
    let keystore = RecordingKeystore::new();
    let Ok(key) = VaultMasterKey::generate() else {
        unreachable!("randomness must be available");
    };
    // The record is built in memory only; nothing is written before the fault.
    let Ok(record) = create_device_recipient(&key, PROFILE, RECIPIENT, LABEL, &keystore) else {
        unreachable!("the recording broker must seal");
    };

    mark_progress(&workspace);

    // The failpoint fires inside this call, between the broker returning the
    // wrapping key and the VMK being produced.
    let unlocked = unlock_with_device(&record, PROFILE, &keystore);

    let Ok(unlocked) = unlocked else {
        unreachable!("an unarmed unlock must succeed");
    };
    let _ = std::fs::write(workspace.join(KY02_OUTPUT), unlocked.expose_secret());
}

#[test]
#[ignore = "child role driven by ky08_kill_during_first_run_key_generation_leaves_no_profile"]
fn ky08_child() {
    let Some(workspace) = child_directory() else {
        return;
    };
    let keystore = RecordingKeystore::new();
    let Ok(key) = VaultMasterKey::generate() else {
        unreachable!("randomness must be available");
    };

    mark_progress(&workspace);

    // The failpoint fires inside this call, after key material exists and
    // before the caller could persist it.
    let Ok(record) = create_device_recipient(&key, PROFILE, RECIPIENT, LABEL, &keystore) else {
        unreachable!("an unarmed creation must succeed");
    };
    let Ok(encoded) = record.to_canonical_cbor() else {
        unreachable!("the record must encode");
    };
    let _ = std::fs::write(workspace.join(KY08_OUTPUT), encoded);
}
