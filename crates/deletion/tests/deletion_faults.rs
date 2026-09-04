//! Faults `RB01`-`RB04`, executed through the product deletion flow.
//!
//! **Executed, not injected.** Three of the four are real failures at the real
//! write boundary and need no failpoint at all:
//!
//! * `RB03` — the index cannot answer for a class, so the deletion refuses to
//!   run and names the node. There is no error to inject: it is a resolver
//!   answer.
//! * `RB04` — a replica file that is a non-empty directory. `remove_file`
//!   returns the operating system's own error on Windows and on Linux.
//! * `RB02` — a backup root that is a file. `write_into_backup` calls
//!   `create_dir_all` first, which returns the operating system's own error on
//!   both.
//!
//! `RB01` is a **kill**, and a kill cannot be produced by an argument. It runs
//! a child process to `academic-vault`'s own `RB01` failpoint — the one beside
//! the key slot it destroys — through this crate's product flow, and observes
//! the object on disk afterwards. That row needs the `deletion-engine` and
//! `phase2-fault-injection` features, so it is compiled only when they are
//! selected.

pub mod support;

use std::{error::Error, fs};

use academic_deletion::{
    ClassTargets, DeletionConfirmation, DeletionDryRun, DeletionImpactPreview, EvidenceCitations,
    FilesystemExecutor, ProviderErasureLog, execute_deletion,
};
use academic_domain::{Actor, TimestampMillis};
use academic_retention::{
    ActionId, AppendOnlyJournal, DELETION_JOURNAL_RELATIVE_PATH, DerivativeClass, RetentionOutcome,
    UnresolvedReason,
};
use academic_student_voice::EvidenceIndex;

use support::{
    DECIDING_USER, NothingProtected, RecordingShredder, SHARED_A, SHARED_B, SUBJECT_ARTIFACT,
    StatedIndex, TestResult, TestRoot, digest, entity, paths_for, target, touch,
};

// ---------------------------------------------------------------------------
// RB03 — derivative not found while planning
// ---------------------------------------------------------------------------

/// `RB03`. A class the index cannot answer for stops the deletion before it
/// starts, and the unresolved node is named with its class.
///
/// Nothing runs: `settle` decides `RB03` before the first action, so a cache
/// file that would have been purged is still there afterwards. That is the half
/// that makes this a refusal rather than a partial deletion.
#[test]
fn rb03_an_unresolved_class_refuses_the_deletion_and_names_the_node() -> TestResult {
    let root = TestRoot::new("rb03")?;
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let cache = target(SHARED_A, 0x33)?;
    let cache_path = touch(root.path(), "cache.bin")?;

    let mut index = StatedIndex::all_empty("nothing of this class derives from the subject");
    index.state(DerivativeClass::Cache, ClassTargets::Targets(vec![cache]));
    // The graph-claim subsystem does not answer. It is not empty; it is unknown.
    index.state(
        DerivativeClass::GraphClaim,
        ClassTargets::Unresolved {
            reason: "the claim index is not available for this subject".to_owned(),
        },
    );

    let receipt = run(
        &root,
        subject,
        &index,
        paths_for(&[(cache, cache_path.clone())], &[]),
    )?;

    assert_eq!(receipt.outcome_word(), "REPAIR_REQUIRED");
    assert!(matches!(
        receipt.outcome(),
        RetentionOutcome::RepairRequired(_)
    ));
    assert_eq!(receipt.unresolved().len(), 1);
    let row = &receipt.unresolved()[0];
    assert_eq!(row.class, DerivativeClass::GraphClaim);
    assert_eq!(row.reason, UnresolvedReason::NotResolved);
    assert!(
        row.to_row().contains("GRAPH_CLAIM"),
        "the row does not name the class: {}",
        row.to_row()
    );
    assert!(
        row.to_row().contains("the claim index is not available"),
        "the row lost the index's own words: {}",
        row.to_row()
    );
    assert!(
        cache_path.exists(),
        "a deletion that refused to complete still purged a cache file"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// RB04 — replica or cache purge partial
// ---------------------------------------------------------------------------

/// `RB04`. One replica purge fails for real; the result is `PARTIAL` and names
/// exactly the artifact that is still there.
///
/// The failing path is a non-empty directory, so `remove_file` fails with the
/// host's own error rather than an injected one. Two artifacts that share a
/// locator are used deliberately: the one that succeeded and the one that
/// failed differ only in artifact id, so a report keyed by locator would name
/// the wrong one or report one row for two.
#[test]
fn rb04_a_partial_purge_reports_the_exact_remaining_artifacts() -> TestResult {
    let root = TestRoot::new("rb04")?;
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let purged = target(SHARED_A, 0x33)?;
    let held = target(SHARED_B, 0x33)?;

    let purged_path = touch(root.path(), "replica-a.bin")?;
    let held_path = root.path().join("replica-b.bin");
    fs::create_dir_all(&held_path)?;
    fs::write(held_path.join("occupant"), b"synthetic")?;

    let mut index = StatedIndex::all_empty("nothing of this class derives from the subject");
    index.state(
        DerivativeClass::Replica,
        ClassTargets::Targets(vec![purged, held]),
    );

    let receipt = run(
        &root,
        subject,
        &index,
        paths_for(
            &[(purged, purged_path.clone()), (held, held_path.clone())],
            &[],
        ),
    )?;

    assert_eq!(receipt.outcome_word(), "PARTIAL");
    assert!(matches!(receipt.outcome(), RetentionOutcome::Partial(_)));
    assert_eq!(
        receipt.unresolved().len(),
        1,
        "the partial purge reported {:?}",
        receipt.unresolved_rows()
    );
    let row = &receipt.unresolved()[0];
    assert_eq!(row.class, DerivativeClass::Replica);
    assert_eq!(row.reason, UnresolvedReason::PurgeFailed);
    assert_eq!(row.target, Some(held));

    // The exact artifact, not the locator the two share.
    let rendered = row.to_row();
    assert!(
        rendered.contains(&held.artifact_hex()),
        "the row does not name the artifact that is still there: {rendered}"
    );
    assert!(
        !rendered.contains(&purged.artifact_hex()),
        "the row names the artifact that was removed: {rendered}"
    );
    assert!(
        !purged_path.exists(),
        "the removable replica is still there"
    );
    assert!(held_path.exists(), "the held replica was removed after all");
    Ok(())
}

// ---------------------------------------------------------------------------
// RB02 — backup tombstone write fails
// ---------------------------------------------------------------------------

/// `RB02`. A tombstone write that fails makes the deletion `REPAIR_REQUIRED`,
/// not `PARTIAL`, and leaves no partial record behind.
///
/// The backup root is a regular file, so `write_into_backup`'s `create_dir_all`
/// fails with the host's own error before any byte is written. A deletion whose
/// tombstone did not land is not "mostly done": it will not re-apply on
/// restore, and an operator has to finish it.
#[test]
fn rb02_a_failed_tombstone_write_is_repair_required_and_leaves_nothing() -> TestResult {
    let root = TestRoot::new("rb02")?;
    let subject = target(SUBJECT_ARTIFACT, 0x11)?;
    let cache = target(SHARED_A, 0x33)?;
    let backup_target = target(SUBJECT_ARTIFACT, 0x44)?;

    let cache_path = touch(root.path(), "cache.bin")?;
    // Not a directory: a file with the name the backup root would have.
    let backup_root = touch(root.path(), "backup")?;

    let mut index = StatedIndex::all_empty("nothing of this class derives from the subject");
    index
        .state(DerivativeClass::Cache, ClassTargets::Targets(vec![cache]))
        .state(
            DerivativeClass::BackupExpiry,
            ClassTargets::Targets(vec![backup_target]),
        );

    let receipt = run(
        &root,
        subject,
        &index,
        paths_for(
            &[(cache, cache_path.clone())],
            &[(backup_target, backup_root.clone())],
        ),
    )?;

    assert_eq!(receipt.outcome_word(), "REPAIR_REQUIRED");
    assert_eq!(receipt.unresolved().len(), 1);
    let row = &receipt.unresolved()[0];
    assert_eq!(row.class, DerivativeClass::BackupExpiry);
    assert_eq!(row.reason, UnresolvedReason::TombstoneWriteFailed);
    assert_eq!(row.target, Some(backup_target));
    assert!(
        row.to_row().contains("TOMBSTONE_WRITE_FAILED"),
        "{}",
        row.to_row()
    );

    // No partial record: the backup root is still the file it was, and the
    // tombstone directory that would have held one does not exist.
    assert!(
        backup_root.is_file(),
        "the failed write created a directory"
    );
    assert_eq!(fs::read(&backup_root)?, b"synthetic");
    assert!(
        !root.path().join("backup").join("tombstones").exists(),
        "the failed write left a tombstone directory behind"
    );
    // The purge that ran before it still ran: `REPAIR_REQUIRED` is about the
    // tombstone, not about the whole action being abandoned.
    assert!(!cache_path.exists(), "the cache purge did not run");
    Ok(())
}

// ---------------------------------------------------------------------------
// RB01 — kill during crypto-shred
// ---------------------------------------------------------------------------

#[cfg(all(feature = "deletion-engine", feature = "phase2-fault-injection"))]
mod rb01 {
    use std::{
        collections::BTreeMap,
        env,
        error::Error,
        fs,
        io::Cursor,
        path::Path,
        process::{Command, Stdio},
    };

    use academic_crypto::{
        IDENTIFIER_BYTES, ProfileId, RECOVERY_ARGON2ID_V1, RecipientRecord, RecoverySecret,
        UnlockThrottle, VaultMasterKey, create_recovery_recipient, unlock_with_recovery,
    };
    use academic_deletion::{
        ClassTargets, DeletionConfirmation, DeletionDryRun, DeletionImpactPreview, DeletionPaths,
        DeletionTarget, EvidenceCitations, FilesystemExecutor, ProviderErasureLog,
        engine::VaultShredder, execute_deletion,
    };
    use academic_domain::{
        Actor, ArtifactDescriptor, Confidentiality, DomainId as CanonicalDomainId, MediaType,
        RetentionClass, TimestampMillis,
    };
    use academic_retention::{
        ActionId, AppendOnlyJournal, DELETION_JOURNAL_RELATIVE_PATH, DerivativeClass,
        ROTATION_JOURNAL_RELATIVE_PATH, recipients, rotation::KeyGeneration,
    };
    use academic_student_voice::EvidenceIndex;
    use academic_vault::{
        ArtifactIngestRequest, EncryptedDomainKeyring, EncryptedVault,
        object::{HEADER_BYTES, KEY_SLOT_OFFSET},
    };

    use crate::support::{
        DECIDING_USER, NothingProtected, StatedIndex, TestResult, TestRoot, digest, entity,
    };

    const PROFILE_ID_BYTES: [u8; IDENTIFIER_BYTES] = [0xA3; IDENTIFIER_BYTES];
    const RECIPIENT: [u8; IDENTIFIER_BYTES] = [0xB3; IDENTIFIER_BYTES];
    const ENTROPY: [u8; 32] = [0xC3; 32];
    const DOMAIN_ID: &str = "01900000-0000-7000-8000-000000000201";
    const LINEAGE_ID: &str = "01900000-0000-7000-8000-000000000301";
    const ARTIFACT: &str = "01900000-0000-7000-8000-000000000101";
    const CHUNK_SIZE: u32 = 256;
    const DESCRIPTOR_FILE: &str = "descriptor.json";

    const CHILD_ENV: &str = "ACADEMIC_DELETION_FAULT_CHILD";
    const PROFILE_ENV: &str = "ACADEMIC_DELETION_FAULT_PROFILE";
    const VAULT_FAULT_VARIABLE: &str = "ACADEMIC_VAULT_TEST_FAULT";
    const VAULT_READY_VARIABLE: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";

    fn profile_id() -> ProfileId {
        ProfileId::from_bytes(PROFILE_ID_BYTES)
    }

    fn domain() -> Result<CanonicalDomainId, Box<dyn Error>> {
        Ok(DOMAIN_ID.parse()?)
    }

    fn open_vault(root: &Path, master: &VaultMasterKey) -> Result<EncryptedVault, Box<dyn Error>> {
        let canonical = domain()?;
        let kek = master.derive_domain_kek(
            profile_id(),
            academic_crypto::DomainId::from_bytes(*canonical.as_bytes()),
        )?;
        let mut keyring = EncryptedDomainKeyring::new(profile_id());
        keyring.insert(canonical, kek)?;
        Ok(EncryptedVault::open_with_chunk_size(
            root, keyring, CHUNK_SIZE,
        )?)
    }

    fn publish_generation(root: &Path) -> Result<VaultMasterKey, Box<dyn Error>> {
        let master = VaultMasterKey::generate()?;
        let record = create_recovery_recipient(
            &master,
            profile_id(),
            RECIPIENT,
            &RecoverySecret::from_entropy(ENTROPY),
            RECOVERY_ARGON2ID_V1,
        )?;
        let mut journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        recipients::add_recipient(
            root,
            profile_id(),
            &mut journal,
            record,
            KeyGeneration::of(&master, profile_id())?,
        )?;
        Ok(master)
    }

    fn load_generation(root: &Path) -> Result<VaultMasterKey, Box<dyn Error>> {
        let set = recipients::read_set(root, profile_id())?;
        let record: RecipientRecord = set
            .records()
            .iter()
            .find(|record| record.recipient_id() == &RECIPIENT)
            .cloned()
            .ok_or("the profile holds no recipient record")?;
        let mut throttle = UnlockThrottle::default();
        Ok(unlock_with_recovery(
            &record,
            profile_id(),
            &RecoverySecret::from_entropy(ENTROPY),
            &mut throttle,
            0,
        )?)
    }

    fn seal(vault: &EncryptedVault) -> Result<ArtifactDescriptor, Box<dyn Error>> {
        let request = ArtifactIngestRequest::new(
            ARTIFACT.parse()?,
            MediaType::parse("application/pdf")?,
            domain()?,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            LINEAGE_ID.parse()?,
        );
        Ok(vault
            .ingest(&request, Cursor::new(vec![0x5A_u8; 2_048]))?
            .descriptor()
            .clone())
    }

    /// The descriptor the parent sealed, handed to the child as JSON.
    ///
    /// A child that re-derived it would be deriving from the same code the
    /// parent used; reading the parent's own record is what makes the two
    /// processes agree on one artifact rather than on one procedure.
    fn read_descriptor(root: &Path) -> Result<ArtifactDescriptor, Box<dyn Error>> {
        let bytes = fs::read(root.join(DESCRIPTOR_FILE))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Runs this crate's product deletion over the real object.
    fn delete_the_object(root: &Path) -> Result<String, Box<dyn Error>> {
        let master = load_generation(root)?;
        let vault = open_vault(root, &master)?;
        let descriptor = read_descriptor(root)?;
        let subject = DeletionTarget::new(descriptor.id, *descriptor.vault_locator.as_bytes());

        let mut index = StatedIndex::all_empty("this fixture holds one object and no derivative");
        index.state(
            DerivativeClass::Transcript,
            ClassTargets::Targets(vec![subject]),
        );
        let dry_run = DeletionDryRun::of(subject, &index, &NothingProtected);
        let mut citations = EvidenceCitations::new();
        citations.cite(subject, digest("subject-bytes"));
        let preview = DeletionImpactPreview::of(dry_run, &EvidenceIndex::default(), &citations, 1)?;
        let user = Actor::User {
            user_id: entity(DECIDING_USER)?,
        };
        let shown = preview.digest();
        let confirmation =
            DeletionConfirmation::given(preview, &user, shown, TimestampMillis::new(2))?;

        let mut journal = AppendOnlyJournal::open(&root.join(DELETION_JOURNAL_RELATIVE_PATH))?;
        let mut descriptors = BTreeMap::new();
        descriptors.insert(subject, descriptor);
        let mut shredder = VaultShredder::over(&vault, descriptors);
        let mut executor = FilesystemExecutor::new(
            &mut shredder,
            DeletionPaths::new(),
            "0102030405060708090a0b0c0d0e0f10".to_owned(),
            3,
        );
        let receipt = execute_deletion(
            &mut journal,
            ActionId::from_bytes([0x51; 16]),
            &confirmation,
            &mut executor,
            ProviderErasureLog::new(),
        )?;
        Ok(receipt.outcome_word().to_owned())
    }

    /// The child half of the `RB01` kill. Returns immediately in the parent.
    #[test]
    fn deletion_fault_child_entrypoint() {
        let Ok(root) = env::var(PROFILE_ENV) else {
            return;
        };
        if env::var(CHILD_ENV).is_err() {
            return;
        }
        // The failpoint aborts inside this call. If it does not, the child
        // exits cleanly and the parent fails on the exit status and the marker,
        // which is why nothing here has to report the error itself.
        drop(delete_the_object(Path::new(&root)));
    }

    /// `RB01`. A kill during the crypto-shred leaves the object shredded or
    /// intact and never anything in between, when the shred is driven by this
    /// crate's product deletion flow rather than by `P2-K5`'s own fixture.
    #[test]
    fn rb01_a_kill_during_the_product_shred_leaves_shredded_or_intact() -> TestResult {
        let root = TestRoot::new("rb01")?;
        let master = publish_generation(root.path())?;
        let vault = open_vault(root.path(), &master)?;
        let descriptor = seal(&vault)?;
        let path = vault.layout().object_path(&descriptor)?;
        let intact = fs::read(&path)?;
        fs::write(
            root.path().join(DESCRIPTOR_FILE),
            serde_json::to_vec(&descriptor)?,
        )?;
        drop(vault);

        let marker = root.path().join("ready-rb01");
        let status = Command::new(env::current_exe()?)
            .arg("rb01::deletion_fault_child_entrypoint")
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env(PROFILE_ENV, root.path())
            .env(VAULT_FAULT_VARIABLE, "RB01")
            .env(VAULT_READY_VARIABLE, &marker)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        assert!(
            !status.success(),
            "the RB01 child exited cleanly instead of aborting at its failpoint"
        );
        assert!(
            marker.is_file(),
            "the RB01 child never reached the failpoint"
        );
        assert_eq!(fs::read_to_string(&marker)?, "RB01");

        // Killed before the key slot write: the object is byte for byte what it
        // was, so a deletion interrupted there left nothing half-destroyed.
        assert_eq!(
            fs::read(&path)?,
            intact,
            "the object changed before the key slot write"
        );

        // Re-running to completion in this process shreds it, and nothing but
        // the key slot moves. That is the other half of "shredded or intact".
        let outcome = delete_the_object(root.path())?;
        assert_eq!(outcome, "COMPLETE");
        let shredded = fs::read(&path)?;
        assert_eq!(shredded.len(), intact.len());
        assert_eq!(shredded[..KEY_SLOT_OFFSET], intact[..KEY_SLOT_OFFSET]);
        assert_eq!(shredded[HEADER_BYTES..], intact[HEADER_BYTES..]);
        assert_ne!(
            shredded[KEY_SLOT_OFFSET..HEADER_BYTES],
            intact[KEY_SLOT_OFFSET..HEADER_BYTES],
            "the key slot was not destroyed"
        );

        // The journal records the shred, and it records it inside the action:
        // the `ArtifactShredded` row is between this action's plan record and
        // its settlement, which is what makes a kill in between recoverable.
        let journal = fs::read_to_string(root.path().join(DELETION_JOURNAL_RELATIVE_PATH))?;
        let kinds: Vec<&str> = journal
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                ["RetentionPlanned", "ArtifactShredded", "RetentionSettled"]
                    .into_iter()
                    .find(|kind| line.contains(kind))
            })
            .collect();
        // The killed run left its own plan record and nothing after it, which
        // is what makes a resume know what was going to be reached; the
        // completed run then wrote all three, in order.
        assert_eq!(
            kinds.first(),
            Some(&"RetentionPlanned"),
            "the killed run left no durable record of what it was about to reach"
        );
        assert_eq!(
            kinds[kinds.len() - 3..],
            ["RetentionPlanned", "ArtifactShredded", "RetentionSettled"],
            "the shred was not recorded inside the action it belongs to"
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == "ArtifactShredded")
                .count(),
            1,
            "the killed run recorded a shred it never performed"
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// shared
// ---------------------------------------------------------------------------

/// Runs the whole product flow and returns the receipt.
fn run(
    root: &TestRoot,
    subject: academic_deletion::DeletionTarget,
    index: &StatedIndex,
    paths: academic_deletion::DeletionPaths,
) -> Result<academic_deletion::ArtifactDeletionReceipt, Box<dyn Error>> {
    let dry_run = DeletionDryRun::of(subject, index, &NothingProtected);
    let mut citations = EvidenceCitations::new();
    for (position, target) in dry_run.reached().iter().enumerate() {
        citations.cite(*target, digest(&format!("evidence-{position}")));
    }
    let preview = DeletionImpactPreview::of(dry_run, &EvidenceIndex::default(), &citations, 1_000)?;
    let user = Actor::User {
        user_id: entity(DECIDING_USER)?,
    };
    let shown = preview.digest();
    let confirmation =
        DeletionConfirmation::given(preview, &user, shown, TimestampMillis::new(2_000))?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(DELETION_JOURNAL_RELATIVE_PATH))?;
    let mut shredder = RecordingShredder::default();
    let mut executor = FilesystemExecutor::new(
        &mut shredder,
        paths,
        "0102030405060708090a0b0c0d0e0f10".to_owned(),
        3_000,
    );
    Ok(execute_deletion(
        &mut journal,
        ActionId::from_bytes([0x51; 16]),
        &confirmation,
        &mut executor,
        ProviderErasureLog::new(),
    )?)
}
