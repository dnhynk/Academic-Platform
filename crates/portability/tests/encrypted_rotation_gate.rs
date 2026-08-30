//! The rotation refusal over the real store, the real vault, and a real backup.
//!
//! `rotation_gate.rs` in `academic-retention` states the refusal where the
//! engine lives. This states it where an orchestrator would actually stand: the
//! encrypted portability lane is the one place the store, the vault, and the
//! backup boundary link into a single process, so it is the only place that can
//! show a rotation being refused *and* a deletion still reaching the copies a
//! backup holds. Those are the two halves of the boundary this task drew.
//!
//! The rows that run a rotation live in `encrypted_rotation_seam.rs` and in
//! three rows of `encrypted_rotation.rs`, behind `encrypted-portability-rotation`.
//! They and this file never link into one binary.

#![cfg(all(
    feature = "encrypted-portability",
    not(feature = "encrypted-portability-rotation")
))]

mod encrypted_support;

use academic_portability::encrypted::{
    ProfileKeys,
    backup::{BackupPlan, backup_encrypted_profile, verify_encrypted_backup_directory},
    restore::{
        EncryptedRestorePlan, open_backup_with_secret, recover_profile_keys,
        restore_encrypted_profile,
    },
    rotation::{StoreDatabaseRekey, deletion_tombstone},
};
use academic_recovery::{BackupRecipientKind, RecoveryProfile};
use academic_retention::{
    AppendOnlyJournal, RotationId, RotationPlan, RotationUnit,
    engine::{
        EngineError, HeaderProbe, RotationEngine, TombstonedArtifact, probe_header,
        shred_with_tombstone,
    },
    journal::ROTATION_JOURNAL_RELATIVE_PATH,
    rotation::{KeyGeneration, StoreDatabaseExecutor as _},
    tombstone,
};
use academic_store::path_policy::NativePathProbe;
use encrypted_support::{EncryptedFixture, TestResult, backup_key_set, domain_id, recovery_secret};

/// The whole product rotation sequence, refused at every step it offers.
///
/// This is `encrypted_rotation.rs`'s own sequence — plan every object plus the
/// `STORE_DATABASE` unit, begin, move each object, run the executor bound to
/// `P2-K2`'s `PRAGMA rekey`, complete — driven against the real profile. Every
/// call refuses, the journal stays empty, and the database still opens under the
/// generation it was already under: a refusal that had already rekeyed a page
/// would not be one.
#[test]
fn the_product_rotation_sequence_is_refused_at_every_step_over_the_real_store() -> TestResult {
    let fixture = EncryptedFixture::new("gate-store-rotation")?;
    let descriptors = fixture.descriptors()?;
    let source_master = fixture.master();
    let target_master = academic_crypto::VaultMasterKey::generate()?;

    let units: Vec<RotationUnit> = descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let database = RotationUnit::store_database(encrypted_support::PROFILE_ID);
    let mut planned = units.clone();
    planned.push(database.clone());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x5C; 16]),
        encrypted_support::PROFILE_ID,
        KeyGeneration::of(source_master, encrypted_support::PROFILE_ID)?,
        KeyGeneration::of(&target_master, encrypted_support::PROFILE_ID)?,
        planned,
    )?;

    let source_vault = fixture.open_vault()?;
    let target_vault = fixture.open_vault_under(&target_master)?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    let journal_path = fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH);
    let mut journal = AppendOnlyJournal::open(&journal_path)?;

    let begun = engine.begin(&mut journal);
    assert!(
        matches!(begun, Err(EngineError::NotAccepted(_))),
        "a rotation was begun over the real store: {begun:?}"
    );
    let moved = engine.rotate_object(&mut journal, &units[0], &descriptors[0]);
    assert!(
        matches!(moved, Err(EngineError::NotAccepted(_))),
        "an object moved: {moved:?}"
    );

    let probe = NativePathProbe::default();
    let executor = StoreDatabaseRekey::new(
        fixture.profile_root(),
        &probe,
        encrypted_support::PROFILE_ID,
        source_master,
        &target_master,
    );
    let rekeyed = engine.rotate_store_database(&mut journal, &database, &executor);
    assert!(
        matches!(rekeyed, Err(EngineError::NotAccepted(_))),
        "the store database was rekeyed: {rekeyed:?}"
    );
    let completed = engine.complete(&mut journal);
    assert!(
        matches!(completed, Err(EngineError::NotAccepted(_))),
        "a rotation completed: {completed:?}"
    );

    assert!(
        journal.entries().next().is_none(),
        "a refused rotation left records in the profile's journal"
    );
    // The database is where it was. A rekey is a page rewrite and cannot be
    // undone by reading a journal afterwards, so "refused" has to mean "before".
    fixture.open_store()?;
    Ok(())
}

/// The two irreversible writes a rotation composes are **not** refused.
///
/// The gate wraps the journalled orchestration: the seven entry points above
/// are the calls that read or append the rotation journal. The primitives they
/// drive are in crates below this one — a re-seal is `academic-vault`'s and the
/// page rewrite is `academic-store`'s — and neither can reach the gate without
/// inverting a machine-checked acyclic crate graph or pulling
/// `academic-retention` into the default product graph. The contract says so;
/// this row is that sentence executed, because the fifth `P2-A1` audit found
/// the contract claiming the fourth audit's states were "behind the gate rather
/// than beside it" when one call to the executor reaches half of one of them.
///
/// What the call costs is the point: the profile then opens under neither the
/// generation the rest of it is under nor the one its backups were taken with,
/// and the rotation journal holds nothing to replay from. It is undone here
/// only because this row still has both master keys in memory.
#[test]
fn the_primitives_a_rotation_composes_are_not_refused_beside_the_gate() -> TestResult {
    let fixture = EncryptedFixture::new("gate-beside-primitives")?;
    let target_master = academic_crypto::VaultMasterKey::generate()?;
    let probe = NativePathProbe::default();

    // The object half: a re-seal into another generation's vault, which is what
    // `rotate_object` does between its two journal entries.
    let subject = fixture
        .descriptors()?
        .first()
        .ok_or("the corpus is empty")?
        .clone();
    {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target_master)?;
        let resealed = source_vault.reseal(&subject, &target_vault);
        assert!(
            resealed.is_ok(),
            "a re-seal into another generation was refused: {resealed:?}"
        );
    }

    // The database half: one call, and the store stops authenticating page one
    // under the generation everything else in the profile is still under.
    let executor = StoreDatabaseRekey::new(
        fixture.profile_root(),
        &probe,
        encrypted_support::PROFILE_ID,
        fixture.master(),
        &target_master,
    );
    let rekeyed = executor.rekey_store_database();
    assert!(
        rekeyed.is_ok(),
        "the store database rekey primitive was refused: {rekeyed:?}"
    );
    assert!(
        fixture.open_store().is_err(),
        "the store still opens under the generation the rekey moved it away from"
    );

    // Neither wrote a journal entry, so nothing replays or resumes.
    let journal =
        AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    assert!(
        journal.entries().next().is_none(),
        "a primitive outside the gate appended to the rotation journal"
    );

    // Put the profile back, so the row leaves nothing it could not undo.
    let back = StoreDatabaseRekey::new(
        fixture.profile_root(),
        &probe,
        encrypted_support::PROFILE_ID,
        &target_master,
        fixture.master(),
    );
    back.rekey_store_database()?;
    fixture.open_store()?;
    Ok(())
}

/// The deletion path is outside the gate, end to end, while rotation is refused.
///
/// Shred the live object, write the tombstone into a backup taken before it,
/// take another backup after it, restore both, and probe. This is
/// `backup_tombstone_is_present_and_re_deletes_on_restore`'s chain, run in a
/// profile whose rotation entry points all refuse — which is the boundary
/// stated as an execution rather than as a sentence in a contract.
#[test]
fn a_deletion_still_reaches_a_backup_while_every_rotation_entry_point_refuses() -> TestResult {
    let fixture = EncryptedFixture::new("gate-store-deletion")?;
    let descriptors = fixture.descriptors()?;
    let subject = descriptors.first().ok_or("the corpus is empty")?.clone();
    let bystander = descriptors
        .get(1)
        .ok_or("the corpus is one artifact")?
        .clone();

    let backup = take_backup(&fixture, "gate-deletion-backup")?;

    let stone = {
        let store = fixture.open_store()?;
        deletion_tombstone(
            &store,
            encrypted_support::hex_lower(&[0x71_u8; 16]),
            &subject,
            1_700_000_000_071,
        )?
    };
    {
        let vault = fixture.open_vault()?;
        let mut journal =
            AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        shred_with_tombstone(&mut journal, &vault, &subject, &stone)?;
    }
    tombstone::write_into_backup(&backup, &stone)?;

    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
    let destination = fixture.work_path("gate-deletion-restored");
    let receipt = restore_encrypted_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert_eq!(
        receipt.re_deleted_objects,
        vec![TombstonedArtifact {
            artifact_id: stone.artifact_id.clone(),
            locator: stone.locator.clone(),
        }],
        "the restore did not re-apply the tombstone"
    );

    let keys = ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
    let vault =
        academic_vault::EncryptedVault::open(&destination, keys.keyring(&recovered.master)?)?;
    let kek = {
        let domain = academic_crypto::DomainId::from_bytes(*domain_id()?.as_bytes());
        recovered
            .master
            .derive_domain_kek(recovered.profile_id, domain)?
    };
    assert_eq!(
        probe_header(&vault.layout().object_path(&subject)?, &kek),
        HeaderProbe::Shredded,
        "the restore published the deleted artifact readable"
    );
    assert_eq!(
        probe_header(&vault.layout().object_path(&bystander)?, &kek),
        HeaderProbe::Opened,
        "the restore destroyed an artifact no tombstone names"
    );
    Ok(())
}

/// Takes a real encrypted backup of `fixture` under the generation it holds.
fn take_backup(fixture: &EncryptedFixture, label: &str) -> TestResult<std::path::PathBuf> {
    let destination = fixture.work_path(label);
    let (backup_root, recipients) = backup_key_set()?;
    backup_encrypted_profile(
        fixture.profile_root(),
        &destination,
        fixture.master(),
        fixture.keys(),
        &BackupPlan {
            recovery_profile: RecoveryProfile::DevicePlusPhrase,
            backup_root: &backup_root,
            backup_recipients: &recipients,
            profile_recovery_recipients: &fixture.recovery_recipients_cbor()?,
        },
    )?;
    Ok(destination)
}
