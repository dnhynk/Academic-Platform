//! The `OB01`-`OB09` rows of the t068 §7 fault matrix.
//!
//! `OB01`-`OB05` and `OB09` are process kills at a named failpoint; the child
//! process below dies inside `ingest` or `reseal` and the parent then proves
//! the required outcome. `OB06`-`OB08` are injected corruption rather than
//! kills, so they run in-process: §7 distinguishes the two and this file keeps
//! them distinguishable.
//!
//! Required outcomes, verbatim from §7:
//!
//! | ID | injection point | outcome |
//! |---|---|---|
//! | `OB01` | kill after DEK generation, before header write | N |
//! | `OB02` | kill mid chunk write | N; partial never appears sealed |
//! | `OB03` | kill after final chunk, before header tag | N |
//! | `OB04` | kill after temp sync, before rename | N |
//! | `OB05` | kill after rename, before directory sync | N or a valid orphan; never a DB reference |
//! | `OB06` | truncated object | Q; no partial plaintext emitted |
//! | `OB07` | reordered or spliced chunk | Q; AEAD fails at the first bad chunk |
//! | `OB08` | wrong-domain KEK | Q at the header tag, before any chunk read |
//! | `OB09` | kill during re-seal migration | old object stays reachable; new object quarantined |
//!
//! `N` is "no canonical reference, recoverable temp/orphan": nothing here ever
//! asks the store for anything, so the executable half of `N` is that no sealed
//! object exists, no partial is left behind after reconciliation, and a retry
//! produces the same descriptor.

#![cfg(all(feature = "aead-objects", feature = "phase2-fault-injection"))]

#[path = "../../test-support/src/encrypted_artifacts.rs"]
mod encrypted_artifacts;
#[path = "../../test-support/src/synthetic_artifacts.rs"]
mod synthetic_artifacts;

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use academic_crypto::{RecipientRecord, VaultMasterKey};
use academic_domain::{ArtifactDescriptor, RetentionClass};
use academic_vault::{
    EncryptedVault, ReconcileOptions, ReconcileState, SealDisposition, SealedObjectVerifier,
    VaultError,
    object::{HEADER_BYTES, WRAP_AAD_BYTES},
};
use encrypted_artifacts::{
    create_master, deterministic_bytes, open_encrypted_vault, unlock_master,
};
use synthetic_artifacts::{
    ARTIFACT_ID, DOMAIN_ID, PERMISSION_LINEAGE_ID, SECOND_ARTIFACT_ID, SECOND_DOMAIN_ID,
    SECOND_PERMISSION_LINEAGE_ID, SyntheticTestRoot, create_private_test_root, request_with,
};

const CHILD_ENV: &str = "ACADEMIC_VAULT_TEST_CHILD";
const PROFILE_ENV: &str = "ACADEMIC_VAULT_TEST_PROFILE";
const FAULT_ENV: &str = "ACADEMIC_VAULT_TEST_FAULT";
const READY_ENV: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";
const RECIPIENT_FILE: &str = "recovery-recipient.cbor";
/// Small enough that a few kilobytes span many chunks, so `OB02` lands mid
/// stream rather than on the only chunk there is.
const CHUNK_SIZE: u32 = 256;
const ARTIFACT_BYTES: usize = 4096 + 17;

// ---------------------------------------------------------------------------
// Child process
// ---------------------------------------------------------------------------

#[test]
fn encrypted_fault_child_entrypoint() -> Result<(), Box<dyn Error>> {
    if env::var(CHILD_ENV).ok().as_deref() != Some("1") {
        return Ok(());
    }
    let root = env::var_os(PROFILE_ENV)
        .map(PathBuf::from)
        .ok_or("fault child profile path was not supplied")?;
    let fault = env::var(FAULT_ENV)?;

    // The child never receives a key: it reopens the same Vault Master Key from
    // the recipient record the parent wrote, exactly as the product does.
    let record = RecipientRecord::from_canonical_cbor(&fs::read(root.join(RECIPIENT_FILE))?)?;
    let master = unlock_master(&record)?;
    let vault = open_encrypted_vault(&root, &master, &[DOMAIN_ID], CHUNK_SIZE)?;
    let bytes = deterministic_bytes(ARTIFACT_BYTES);

    if fault == "OB09" {
        // The object being re-sealed must already exist and be reachable.
        let receipt = vault.ingest(&default_request()?, bytes.as_slice())?;
        let descriptor = receipt.descriptor().clone();
        drop(receipt);
        let _ = vault.reseal(&descriptor, &vault)?;
        return Err("OB09 did not terminate the child process".into());
    }

    let _receipt = vault.ingest(&default_request()?, bytes.as_slice())?;
    Err("the selected encrypted-object fault did not terminate the child process".into())
}

fn default_request() -> Result<academic_vault::ArtifactIngestRequest, Box<dyn Error>> {
    request_with(
        ARTIFACT_ID,
        DOMAIN_ID,
        RetentionClass::UserManaged,
        PERMISSION_LINEAGE_ID,
    )
}

// ---------------------------------------------------------------------------
// OB01-OB05: kills during publication
// ---------------------------------------------------------------------------

#[test]
fn ob01_ob05_publication_kill_matrix_leaves_no_sealed_partial() -> Result<(), Box<dyn Error>> {
    for fault in ["OB01", "OB02", "OB03", "OB04", "OB05"] {
        let (root, master) = prepare_profile(&format!("encrypted-crash-{fault}"))?;
        run_child(root.path(), fault)?;

        let vault = open_encrypted_vault(root.path(), &master, &[DOMAIN_ID], CHUNK_SIZE)?;
        assert_partial_state(&vault, fault)?;
        let bytes = deterministic_bytes(ARTIFACT_BYTES);
        // Reconciliation is given the descriptor a retry would produce, which
        // is the only way it can call an opaque HMAC-named file a valid orphan
        // rather than guessing.
        let expected = expected_descriptor(&vault, &bytes)?;
        let candidates = [expected.clone()];
        let report = vault.reconcile(
            &ReconcileOptions::new(SystemTime::now() + Duration::from_secs(48 * 60 * 60))
                .with_retry_candidates(&candidates)
                .with_temp_expiry(Duration::from_secs(60 * 60))
                .with_orphan_grace(Duration::from_secs(60 * 60)),
        )?;

        // N: no partial survives reconciliation.
        assert_eq!(
            count_extension(vault.layout().temp_dir(), "partial")?,
            0,
            "{fault} left a partial behind"
        );

        let sealed = count_extension(vault.layout().objects_root(), "aobj")?;
        if fault == "OB05" {
            // OB05 dies after the rename. The object is complete and valid, but
            // no canonical reference exists, so it is a valid orphan.
            assert_eq!(sealed, 1, "{fault} lost its renamed object");
            assert!(
                report.records().iter().any(|record| {
                    record.state() == ReconcileState::ValidOrphan
                        && record.artifact_id() == Some(expected.id)
                }),
                "{fault} did not report a valid orphan"
            );
        } else {
            // Everything before the rename: the partial never appears sealed.
            assert_eq!(sealed, 0, "{fault} published before the rename");
        }

        // A retry always converges on the same descriptor and one object.
        let receipt = vault.ingest(&default_request()?, bytes.as_slice())?;
        let expected_disposition = if fault == "OB05" {
            SealDisposition::AdoptedExisting
        } else {
            SealDisposition::PublishedNew
        };
        assert_eq!(receipt.disposition(), expected_disposition, "{fault}");
        assert_eq!(receipt.descriptor(), &expected, "{fault}");
        let descriptor = receipt.descriptor().clone();
        drop(receipt);
        assert_eq!(count_extension(vault.layout().objects_root(), "aobj")?, 1);
        assert_eq!(read_all(&vault, &descriptor)?, bytes, "{fault}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// OB06: truncation
// ---------------------------------------------------------------------------

#[test]
fn ob06_truncated_object_is_quarantine_with_no_partial_plaintext() -> Result<(), Box<dyn Error>> {
    let (root, master) = prepare_profile("encrypted-ob06")?;
    let vault = open_encrypted_vault(root.path(), &master, &[DOMAIN_ID], CHUNK_SIZE)?;
    let bytes = deterministic_bytes(ARTIFACT_BYTES);
    let receipt = vault.ingest(&default_request()?, bytes.as_slice())?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    let original = fs::read(&path)?;
    let file = fs::OpenOptions::new().write(true).open(&path)?;
    file.set_len(u64::try_from(original.len() - 1)?)?;
    file.sync_all()?;
    drop(file);

    // Q: the reference is reported corrupt and the pass is repair-required.
    let referenced = [descriptor.clone()];
    let report =
        vault.reconcile(&ReconcileOptions::new(SystemTime::now()).with_referenced(&referenced))?;
    assert!(report.repair_required(), "a truncated object was not Q");
    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::ReferencedCorruptRepairRequired
            && record.artifact_id() == Some(descriptor.id)
    }));
    assert!(read_all(&vault, &descriptor).is_err());

    // Restoring the exact bytes clears the repair-required verdict, so the
    // check is the truncation and not a permanent refusal.
    fs::write(&path, &original)?;
    let healthy =
        vault.reconcile(&ReconcileOptions::new(SystemTime::now()).with_referenced(&referenced))?;
    assert!(!healthy.repair_required());
    assert_eq!(read_all(&vault, &descriptor)?, bytes);
    Ok(())
}

// ---------------------------------------------------------------------------
// OB07: reorder and splice
// ---------------------------------------------------------------------------

#[test]
fn ob07_reordered_or_spliced_chunk_is_quarantine() -> Result<(), Box<dyn Error>> {
    let (root, master) = prepare_profile("encrypted-ob07")?;
    let vault = open_encrypted_vault(root.path(), &master, &[DOMAIN_ID], CHUNK_SIZE)?;
    let bytes = deterministic_bytes(usize::try_from(CHUNK_SIZE)? * 4);
    let receipt = vault.ingest(&default_request()?, bytes.as_slice())?;
    let descriptor = receipt.descriptor().clone();
    let path = receipt.object_path().to_path_buf();
    drop(receipt);

    let donor = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            DOMAIN_ID,
            RetentionClass::UserManaged,
            SECOND_PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let donor_bytes = fs::read(donor.object_path())?;
    drop(donor);

    let original = fs::read(&path)?;
    let record = usize::try_from(CHUNK_SIZE)? + 16;
    let referenced = [descriptor.clone()];

    for (label, mutated) in [
        ("reorder", {
            let mut swapped = original.clone();
            let a = HEADER_BYTES;
            let b = HEADER_BYTES + record;
            swapped[a..a + record].copy_from_slice(&original[b..b + record]);
            swapped[b..b + record].copy_from_slice(&original[a..a + record]);
            swapped
        }),
        ("splice", {
            let mut spliced = original.clone();
            spliced[HEADER_BYTES..HEADER_BYTES + record]
                .copy_from_slice(&donor_bytes[HEADER_BYTES..HEADER_BYTES + record]);
            spliced
        }),
    ] {
        fs::write(&path, &mutated)?;
        let report = vault
            .reconcile(&ReconcileOptions::new(SystemTime::now()).with_referenced(&referenced))?;
        assert!(report.repair_required(), "{label} was not Q");
        assert!(read_all(&vault, &descriptor).is_err(), "{label} decrypted");

        fs::write(&path, &original)?;
        let healthy = vault
            .reconcile(&ReconcileOptions::new(SystemTime::now()).with_referenced(&referenced))?;
        assert!(!healthy.repair_required(), "{label} did not recover");
        assert_eq!(read_all(&vault, &descriptor)?, bytes, "{label}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// OB08: wrong-domain KEK
// ---------------------------------------------------------------------------

#[test]
fn ob08_wrong_domain_kek_is_quarantine_at_the_header() -> Result<(), Box<dyn Error>> {
    let (root, master) = prepare_profile("encrypted-ob08")?;
    let vault = open_encrypted_vault(
        root.path(),
        &master,
        &[DOMAIN_ID, SECOND_DOMAIN_ID],
        CHUNK_SIZE,
    )?;
    let bytes = deterministic_bytes(ARTIFACT_BYTES);

    // One object sealed under domain A's KEK.
    let receipt = vault.ingest(&default_request()?, bytes.as_slice())?;
    let sealed_under_a = fs::read(receipt.object_path())?;
    drop(receipt);

    // The same plaintext in domain B, to learn B's canonical path and
    // descriptor. Its locator differs because the locator key is derived from
    // the domain KEK, which is why a foreign key normally cannot even find an
    // object.
    let in_b = vault.ingest(
        &request_with(
            SECOND_ARTIFACT_ID,
            SECOND_DOMAIN_ID,
            RetentionClass::UserManaged,
            SECOND_PERMISSION_LINEAGE_ID,
        )?,
        bytes.as_slice(),
    )?;
    let descriptor_b = in_b.descriptor().clone();
    let path_b = in_b.object_path().to_path_buf();
    drop(in_b);

    // Now put domain A's ciphertext at domain B's canonical path. This is the
    // case §7 names: the file is where a reader will look and the reader holds
    // a key, but not the key the object was sealed under.
    fs::write(&path_b, &sealed_under_a)?;
    let outcome = SealedObjectVerifier::verify_sealed_object(&vault, &descriptor_b);
    assert!(
        matches!(
            outcome,
            Err(VaultError::ObjectFormat(
                academic_vault::object::ObjectFormatError::Aead
            ))
        ),
        "a foreign-domain object was not refused at the header tag: {outcome:?}"
    );
    assert!(read_all(&vault, &descriptor_b).is_err());

    // The refusal happens at the header, before any chunk is read. Cutting the
    // object down to its header plus one byte changes nothing for the wrong
    // key -- it still fails at the header -- while the right key gets past the
    // header and only then reports the truncation. The two different errors on
    // the same bytes are the ordering proof.
    let header_only = &sealed_under_a[..HEADER_BYTES + 1];
    fs::write(&path_b, header_only)?;
    assert!(
        matches!(
            SealedObjectVerifier::verify_sealed_object(&vault, &descriptor_b),
            Err(VaultError::ObjectFormat(
                academic_vault::object::ObjectFormatError::Aead
            ))
        ),
        "the wrong key stopped failing at the header"
    );

    let descriptor_a = expected_descriptor(&vault, &bytes)?;
    let path_a = vault
        .layout()
        .object_path(&descriptor_a)
        .map_err(|error| format!("{error}"))?;
    fs::write(&path_a, header_only)?;
    assert!(
        matches!(
            SealedObjectVerifier::verify_sealed_object(&vault, &descriptor_a),
            Err(VaultError::ObjectFormat(
                academic_vault::object::ObjectFormatError::Truncated
            ))
        ),
        "the right key did not get past the header"
    );

    // Restoring both files clears both refusals, so neither is a permanent one.
    fs::write(&path_a, &sealed_under_a)?;
    assert_eq!(read_all(&vault, &descriptor_a)?, bytes);
    Ok(())
}

// ---------------------------------------------------------------------------
// OB09: kill during re-seal
// ---------------------------------------------------------------------------

#[test]
fn ob09_reseal_kill_keeps_the_old_object_reachable() -> Result<(), Box<dyn Error>> {
    let (root, master) = prepare_profile("encrypted-ob09")?;
    run_child(root.path(), "OB09")?;

    let vault = open_encrypted_vault(root.path(), &master, &[DOMAIN_ID], CHUNK_SIZE)?;
    let bytes = deterministic_bytes(ARTIFACT_BYTES);
    let expected = expected_descriptor(&vault, &bytes)?;

    // The old object is still reachable: it verifies against its descriptor and
    // still decrypts to the exact plaintext.
    SealedObjectVerifier::verify_sealed_object(&vault, &expected)?;
    assert_eq!(read_all(&vault, &expected)?, bytes);

    // Re-sealing the same plaintext under the same domain key lands on the same
    // locator, so the interrupted re-seal left exactly one object rather than a
    // second, unreferenced copy.
    assert_eq!(count_extension(vault.layout().objects_root(), "aobj")?, 1);

    let referenced = [expected.clone()];
    let report = vault.reconcile(
        &ReconcileOptions::new(SystemTime::now() + Duration::from_secs(48 * 60 * 60))
            .with_referenced(&referenced)
            .with_orphan_grace(Duration::from_secs(60 * 60)),
    )?;
    assert!(
        !report.repair_required(),
        "the old object stopped being reachable"
    );
    assert_eq!(count_extension(vault.layout().temp_dir(), "partial")?, 0);
    assert!(report.records().iter().any(|record| {
        record.state() == ReconcileState::ReferencedValid
            && record.artifact_id() == Some(expected.id)
    }));

    // A re-seal that is *not* interrupted publishes into an unreferenced
    // namespace, and reconciliation quarantines what no reference reaches.
    let quarantine_root = SyntheticTestRoot::new("encrypted-ob09-quarantine")?;
    create_private_test_root(quarantine_root.path())?;
    let destination =
        open_encrypted_vault(quarantine_root.path(), &master, &[DOMAIN_ID], CHUNK_SIZE)?;
    let outcome = vault.reseal(&expected, &destination)?;
    assert_eq!(outcome.superseded, expected);
    let resealed = outcome.resealed.descriptor().clone();
    drop(outcome);
    assert_eq!(read_all(&destination, &resealed)?, bytes);

    // No migration event exists, so the new object has no reference and past
    // the grace window it is quarantined rather than adopted.
    let quarantined = destination.reconcile(
        &ReconcileOptions::new(SystemTime::now() + Duration::from_secs(48 * 60 * 60))
            .with_orphan_grace(Duration::from_secs(60 * 60)),
    )?;
    assert!(
        quarantined
            .records()
            .iter()
            .any(|record| record.state() == ReconcileState::QuarantinedOrphan),
        "an unreferenced re-sealed object was not quarantined"
    );
    assert_eq!(
        count_extension(destination.layout().objects_root(), "aobj")?,
        0,
        "the quarantined object stayed in the object namespace"
    );
    // The original is untouched by the whole exercise.
    assert_eq!(read_all(&vault, &expected)?, bytes);
    Ok(())
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// Observes the exact temp state each publication failpoint leaves behind.
///
/// The five kill points are only five distinct rows of the fault matrix if they
/// leave five distinguishable states on disk. `OB01` and `OB03` are the pair
/// that would collapse into one if the header were written in a single call:
/// "before header write" and "before header tag" would be the same instant.
/// Splitting the header write into `P` then `wrapped_dek` is what keeps them
/// apart, and this is where that separation is observed rather than assumed.
fn assert_partial_state(vault: &EncryptedVault, fault: &str) -> Result<(), Box<dyn Error>> {
    let mut partials = Vec::new();
    for entry in fs::read_dir(vault.layout().temp_dir())? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("partial") {
            partials.push(path);
        }
    }

    if fault == "OB05" {
        // The temp was renamed into the object namespace before the kill.
        assert!(partials.is_empty(), "OB05 left a partial behind");
        return Ok(());
    }
    assert_eq!(
        partials.len(),
        1,
        "{fault} did not leave exactly one partial"
    );
    let bytes = fs::read(&partials[0])?;

    match fault {
        // Killed between creating the temp and reserving the header, so not a
        // single byte exists.
        "OB01" => assert!(bytes.is_empty(), "OB01 wrote header or chunk bytes"),
        // Killed mid stream: the header region is still the reservation, and
        // at least one sealed chunk follows it.
        "OB02" => {
            assert!(bytes.len() > HEADER_BYTES, "OB02 wrote no chunk");
            assert!(
                bytes[..HEADER_BYTES].iter().all(|byte| *byte == 0),
                "OB02 wrote header bytes before the chunks"
            );
        }
        // Killed after the cleartext prefix `P` and before `wrapped_dek`. This
        // is the state that proves the two-stage write: the magic is on disk
        // and the tag is not.
        "OB03" => {
            assert_eq!(&bytes[..4], b"ACOB", "OB03 did not write the header prefix");
            assert!(
                bytes[WRAP_AAD_BYTES..HEADER_BYTES]
                    .iter()
                    .all(|byte| *byte == 0),
                "OB03 wrote the header tag it is defined to precede"
            );
        }
        // Killed after the whole header was written and synced, before the
        // rename. Both stages are present.
        "OB04" => {
            assert_eq!(&bytes[..4], b"ACOB", "OB04 lost the header prefix");
            assert!(
                bytes[WRAP_AAD_BYTES..HEADER_BYTES]
                    .iter()
                    .any(|byte| *byte != 0),
                "OB04 did not write the header tag"
            );
        }
        other => return Err(format!("unexpected publication fault {other}").into()),
    }
    Ok(())
}

fn prepare_profile(label: &str) -> Result<(SyntheticTestRoot, VaultMasterKey), Box<dyn Error>> {
    let root = SyntheticTestRoot::new(label)?;
    create_private_test_root(root.path())?;
    let (master, record) = create_master()?;
    fs::write(
        root.path().join(RECIPIENT_FILE),
        record.to_canonical_cbor()?,
    )?;
    Ok((root, master))
}

fn run_child(root: &Path, fault: &str) -> Result<(), Box<dyn Error>> {
    let ready = root.join(format!("{fault}.ready"));
    let status = Command::new(env::current_exe()?)
        .arg("--exact")
        .arg("encrypted_fault_child_entrypoint")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(PROFILE_ENV, root)
        .env(FAULT_ENV, fault)
        .env(READY_ENV, &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    assert!(!status.success(), "{fault} child unexpectedly succeeded");
    assert_eq!(
        fs::read_to_string(&ready)?,
        fault,
        "{fault} did not reach its failpoint"
    );
    fs::remove_file(&ready)?;
    Ok(())
}

fn expected_descriptor(
    vault: &EncryptedVault,
    bytes: &[u8],
) -> Result<ArtifactDescriptor, Box<dyn Error>> {
    // The descriptor a retry produces, obtained from the vault itself in a
    // disposable profile rather than recomputed here: the locator derivation is
    // the thing under test and a second copy of it would prove nothing.
    let probe_root = SyntheticTestRoot::new("descriptor-probe")?;
    create_private_test_root(probe_root.path())?;
    let record = RecipientRecord::from_canonical_cbor(&fs::read(
        vault.profile_root().join(RECIPIENT_FILE),
    )?)?;
    let master = unlock_master(&record)?;
    let probe = open_encrypted_vault(probe_root.path(), &master, &[DOMAIN_ID], CHUNK_SIZE)?;
    let receipt = probe.ingest(&default_request()?, bytes)?;
    let descriptor = receipt.descriptor().clone();
    drop(receipt);
    Ok(descriptor)
}

fn read_all(
    vault: &EncryptedVault,
    descriptor: &ArtifactDescriptor,
) -> Result<Vec<u8>, VaultError> {
    use std::io::Read as _;

    let mut reader = vault.open_reader(descriptor)?;
    let mut plaintext = Vec::new();
    match reader.read_to_end(&mut plaintext) {
        Ok(_) => Ok(plaintext),
        Err(error) => Err(
            match error.into_inner().map(|s| s.downcast::<VaultError>()) {
                Some(Ok(typed)) => *typed,
                _ => VaultError::UnsafeEntry(PathBuf::from("<io error with no typed source>")),
            },
        ),
    }
}

fn count_extension(root: &Path, extension: &str) -> Result<usize, Box<dyn Error>> {
    let mut count = 0_usize;
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_dir() {
            count = count.saturating_add(count_extension(&path, extension)?);
        } else if metadata.file_type().is_file()
            && path.extension().and_then(|value| value.to_str()) == Some(extension)
        {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}
