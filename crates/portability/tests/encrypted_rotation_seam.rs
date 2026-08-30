//! The seam a rotation leaves behind, over the real store (`T114`).
//!
//! `encrypted_rotation.rs` states what a rotation does. This file states what
//! everything *after* one does — an acceptance whose closure reaches a rotated
//! artifact, a backup taken once the profile has moved, a deletion that crosses
//! a rotation, a retirement gated on the store's own resolution, and a rotation
//! back to a generation the artifact has already been under.
//!
//! Every row is a `T114` reproduction. Each one failed before this repair with
//! no kill and no tampering: the shipped API, called in a legitimate order, was
//! enough. They are stated as properties, so a repair that regresses fails a
//! named row.
//!
//! This whole file compiles only in the encrypted lane:
//! `cargo test -p academic-portability --no-default-features --features encrypted-portability`.

#![cfg(feature = "encrypted-portability")]

mod encrypted_support;

use std::path::PathBuf;

use academic_crypto::VaultMasterKey;
use academic_domain::{ArtifactDescriptor, EventPayload};
use academic_portability::{
    encrypted::{
        ProfileKeys,
        backup::{BackupPlan, backup_encrypted_profile, verify_encrypted_backup_directory},
        restore::{
            EncryptedRestorePlan, open_backup_with_secret, recover_profile_keys,
            restore_encrypted_profile,
        },
        rotation::{StoreCanonicalReference, StoreDatabaseRekey, deletion_tombstone},
    },
    verify::read_artifact_descriptors,
};
use academic_recovery::{BackupRecipientKind, RecoveryProfile};
use academic_retention::{
    AppendOnlyJournal, RotationId, RotationPlan, RotationState, RotationUnit,
    engine::{HeaderProbe, RotationEngine, probe_header, retire_superseded_object},
    journal::ROTATION_JOURNAL_RELATIVE_PATH,
    rotation::{KeyGeneration, StoreDatabaseRekey as StoreDatabaseRekeyOutcome},
    tombstone,
};
use academic_store::{descriptor_migration::DescriptorMigration, path_policy::NativePathProbe};
use academic_vault::{EncryptedVault, SealedObjectVerifier as _};
use encrypted_support::{
    EncryptedFixture, PROFILE_ID, TestResult, backup_key_set, domain_id, hex_lower, id,
    importer_actor, recovery_secret, text_claim,
};

/// Runs the whole product rotation sequence: every object, then the database.
///
/// It is `encrypted_rotation.rs::rotate_every_object` with the rotation
/// identity a parameter, so a second rotation of the same profile is possible.
/// Returns the re-sealed descriptors in corpus order; the fixture is left
/// holding the generation the rotation moved to.
fn rotate_profile(
    fixture: &mut EncryptedFixture,
    target: VaultMasterKey,
    rotation_seed: u8,
    action_base: u64,
) -> TestResult<Vec<ArtifactDescriptor>> {
    let descriptors = fixture.descriptors()?;
    rotate_objects(fixture, target, rotation_seed, action_base, &descriptors)
}

/// The same sequence over the artifacts a plan is allowed to name.
///
/// A plan cannot name an artifact whose key slot was destroyed: the re-seal
/// opens the source object and there is no key slot left to open it with. So a
/// profile that deleted before it rotates plans the objects that are still
/// there, which is what `rotate_profile` reduces to when nothing was deleted.
fn rotate_objects(
    fixture: &mut EncryptedFixture,
    target: VaultMasterKey,
    rotation_seed: u8,
    action_base: u64,
    descriptors: &[ArtifactDescriptor],
) -> TestResult<Vec<ArtifactDescriptor>> {
    let source_generation = KeyGeneration::of(fixture.master(), PROFILE_ID)?;
    let target_generation = KeyGeneration::of(&target, PROFILE_ID)?;
    let mut units: Vec<RotationUnit> = descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let database_unit = RotationUnit::store_database(PROFILE_ID);
    units.push(database_unit.clone());
    let plan = RotationPlan::new(
        RotationId::from_bytes([rotation_seed; 16]),
        PROFILE_ID,
        source_generation,
        target_generation,
        units.clone(),
    )?;

    let mut journal =
        AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target)?;
        RotationEngine::new(&plan, &source_vault, &target_vault).begin(&mut journal)?;
    }
    let mut migrated = Vec::with_capacity(descriptors.len());
    for (index, descriptor) in descriptors.iter().enumerate() {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target)?;
        let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
        let resealed = engine.rotate_object(&mut journal, &units[index], descriptor)?;
        drop(source_vault);
        drop(target_vault);

        let action: academic_domain::RetentionActionId = id(action_base + u64::try_from(index)?)?;
        let sequence = fixture
            .open_store()?
            .next_descriptor_migration_seq(descriptor.id)?;
        let record = DescriptorMigration::of(*action.as_bytes(), descriptor, sequence, &resealed);
        fixture.accept_retention_action(action, record.record_digest())?;

        let target_vault = fixture.open_vault_under(&target)?;
        let mut store = fixture.open_store()?;
        store.record_descriptor_migration(&record, &resealed, &target_vault)?;
        drop(store);
        drop(target_vault);
        migrated.push(resealed);
    }

    {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target)?;
        let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
        let probe = NativePathProbe::default();
        let executor = StoreDatabaseRekey::new(
            fixture.profile_root(),
            &probe,
            PROFILE_ID,
            fixture.master(),
            &target,
        );
        let outcome = engine.rotate_store_database(&mut journal, &database_unit, &executor)?;
        assert_eq!(outcome, StoreDatabaseRekeyOutcome::Rekeyed);
        engine.complete(&mut journal)?;
    }

    fixture.adopt_generation(target)?;
    Ok(migrated)
}

/// Takes a real encrypted backup of the generation the fixture holds now.
fn take_backup(fixture: &EncryptedFixture, label: &str) -> TestResult<PathBuf> {
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

fn journal_of(fixture: &EncryptedFixture) -> TestResult<AppendOnlyJournal> {
    Ok(AppendOnlyJournal::open(
        &fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH),
    )?)
}

fn domain_kek_of(master: &VaultMasterKey) -> TestResult<academic_crypto::DomainKek> {
    let domain = academic_crypto::DomainId::from_bytes(*domain_id()?.as_bytes());
    Ok(master.derive_domain_kek(PROFILE_ID, domain)?)
}

// ---------------------------------------------------------------------------
// T114 P1-A
// ---------------------------------------------------------------------------

/// An acceptance whose closure reaches a rotated artifact is admitted.
///
/// `T114`'s reproduction: the store's own pre-commit revalidation loaded the
/// signed `artifact_descriptor` row and handed its locator straight to the
/// vault, so after a rotation every batch referencing that artifact was refused
/// — `SealingFailed: … has a locator that does not match its keyed descriptor`
/// — and after the superseded object was retired it was refused under both
/// generations. Registering evidence, a claim, or a decision about a rotated
/// artifact was permanently impossible while the contract said the opposite.
///
/// The preflight now resolves the same migration chain a backup and a restore
/// resolve.
#[test]
fn an_acceptance_that_references_a_rotated_artifact_is_admitted() -> TestResult {
    let mut fixture = EncryptedFixture::new("seam-acceptance-after-rotation")?;
    let before = fixture.descriptors()?;
    let migrated = rotate_profile(&mut fixture, VaultMasterKey::generate()?, 0x41, 0x0a00)?;
    let resolved = fixture.descriptors()?;
    assert_eq!(
        resolved[0].vault_locator, migrated[0].vault_locator,
        "precondition: the store resolves to the migrated locator"
    );

    // A claim on the corpus evidence whose artifact the rotation moved.
    let scope = id(0x0102)?;
    let evidence = id(0x0401)?;
    fixture.accept(
        importer_actor(),
        domain_id()?,
        vec![EventPayload::ClaimAsserted(text_claim(
            id(0x0f10)?,
            id(0x0fb0)?,
            "note.body",
            "a claim asserted after the rotation",
            scope,
            evidence,
        )?)],
    )?;

    // Retiring the superseded object does not change the answer: the closure
    // never names it again.
    let store = fixture.open_store()?;
    let vault = fixture.open_vault()?;
    let mut journal = journal_of(&fixture)?;
    let unit = RotationUnit::object(*before[0].vault_locator.as_bytes());
    retire_superseded_object(
        &mut journal,
        &vault,
        &unit,
        &before[0],
        &StoreCanonicalReference::new(&store),
    )?;
    drop(store);
    drop(vault);

    fixture.accept(
        importer_actor(),
        domain_id()?,
        vec![EventPayload::ClaimAsserted(text_claim(
            id(0x0f11)?,
            id(0x0fb1)?,
            "note.body",
            "a claim asserted after the retirement",
            scope,
            evidence,
        )?)],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P1-B
// ---------------------------------------------------------------------------

/// A backup taken after a rotation restores.
///
/// `T114`'s reproduction: the `STORE_DATABASE` unit had no executor, so a
/// rotated profile's database stayed under the superseded `SKEY_p` while its
/// objects moved. `backup_encrypted_profile` took the store key and the keyring
/// as two separate arguments and copied that split into the backup;
/// `restore_encrypted_profile` derives both halves from the one master it
/// recovers, so the backup verified and no recovery record could restore it —
/// either `file is not a database` or `locator does not match its keyed
/// descriptor`, depending on which generation the recovery records held.
///
/// The unit now runs, so the profile is wholly under one generation and the
/// restored database and objects agree.
#[test]
fn a_backup_taken_after_a_rotation_restores() -> TestResult {
    let mut fixture = EncryptedFixture::new("seam-restore-after-rotation")?;
    let migrated = rotate_profile(&mut fixture, VaultMasterKey::generate()?, 0x42, 0x0a00)?;

    // The profile's recovery records move with it, which is the other half of
    // the rotation: a backup carrying records of the superseded generation
    // would recover a master that opens neither half.
    fixture.rewrap_recovery_recipients()?;
    let destination = take_backup(&fixture, "backup-after-rotation")?;

    let (backup_root, _) = open_backup_with_secret(
        &destination,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&destination, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
    assert_eq!(
        KeyGeneration::of(&recovered.master, PROFILE_ID)?,
        KeyGeneration::of(fixture.master(), PROFILE_ID)?,
        "the recovered master is not the generation the rotation left"
    );

    let restored_root = fixture.work_path("restored-after-rotation");
    let receipt = restore_encrypted_profile(
        &destination,
        &restored_root,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert_eq!(receipt.restored_object_count, migrated.len() as u64);

    // The restored profile opens end to end under the recovered master: the
    // database under its `SKEY_p`, every object under its `KEK_d`, at the
    // locators the chain resolves to.
    let keys = ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
    let database = academic_portability::verify::CanonicalDatabase::open_source(
        &restored_root.join(academic_store::STORE_DATABASE_FILE),
        keys.store_key(),
    )?;
    let restored_descriptors = read_artifact_descriptors(&database)?;
    drop(database);
    let vault = EncryptedVault::open(&restored_root, keys.keyring(&recovered.master)?)?;
    for descriptor in &restored_descriptors {
        vault.verify_sealed_object(descriptor)?;
    }
    assert_eq!(
        restored_descriptors
            .iter()
            .map(|descriptor| descriptor.vault_locator.clone())
            .collect::<Vec<_>>(),
        migrated
            .iter()
            .map(|descriptor| descriptor.vault_locator.clone())
            .collect::<Vec<_>>(),
        "the restored profile does not resolve to the objects the rotation left"
    );
    Ok(())
}

/// A backup whose key set and master name two generations is refused.
///
/// This is the same defect stated as a guard. Whatever a caller assembles, a
/// backup that would pair a database under one generation with objects under
/// another is refused when it is taken, not discovered on the fresh machine
/// where the restore was supposed to work.
#[test]
fn a_backup_of_a_split_generation_profile_is_refused() -> TestResult {
    let fixture = EncryptedFixture::new("seam-backup-split-generation")?;
    let other = VaultMasterKey::generate()?;
    let (backup_root, recipients) = backup_key_set()?;
    let refused = backup_encrypted_profile(
        fixture.profile_root(),
        &fixture.work_path("split-backup"),
        &other,
        fixture.keys(),
        &BackupPlan {
            recovery_profile: RecoveryProfile::DevicePlusPhrase,
            backup_root: &backup_root,
            backup_recipients: &recipients,
            profile_recovery_recipients: &fixture.recovery_recipients_cbor()?,
        },
    );
    let message = refused
        .err()
        .ok_or("a backup paired a store key and a keyring of two generations")?
        .to_string();
    assert!(
        message.contains("backup profile key generation"),
        "the refusal did not name the generation mismatch: {message}"
    );
    assert!(
        !fixture.work_path("split-backup").exists(),
        "the refused backup still published a directory"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P1-C
// ---------------------------------------------------------------------------

/// A deletion after a rotation reaches the copy a pre-rotation backup holds.
///
/// `T114`'s reproduction: a `BackupTombstone` named one locator, a locator is a
/// function of the domain KEK, and a backup taken before the rotation holds the
/// object under the name it had then. The restore matched nothing, published a
/// profile whose copy of the deleted artifact opened normally, and returned a
/// receipt with an empty re-deletion list and no error. The contract said a
/// deletion reaches the copies a backup holds and that an unmatched tombstone is
/// reported as absent; both were false on this path.
#[test]
fn a_deletion_after_a_rotation_reaches_a_pre_rotation_backup() -> TestResult {
    let mut fixture = EncryptedFixture::new("seam-tombstone-across-rotation")?;
    let before = fixture.descriptors()?;
    let subject_before = before[0].clone();
    let backup = take_backup(&fixture, "backup-before-rotation")?;

    let migrated = rotate_profile(&mut fixture, VaultMasterKey::generate()?, 0x43, 0x0a00)?;
    let subject_after = migrated[0].clone();
    assert_ne!(subject_after.vault_locator, subject_before.vault_locator);

    // The deletion, the way the product does it: the tombstone names every
    // locator the store's chain holds for the artifact, then the live object is
    // shredded and the record is written into the backup.
    let store = fixture.open_store()?;
    let stone = deletion_tombstone(
        &store,
        hex_lower(&[0x31_u8; 16]),
        &subject_after,
        1_700_000_000_002,
    )?;
    drop(store);
    assert_eq!(
        stone.superseded_locators,
        vec![hex_lower(subject_before.vault_locator.as_bytes())],
        "the tombstone did not name the locator the artifact moved from"
    );
    {
        let vault = fixture.open_vault()?;
        let mut journal = journal_of(&fixture)?;
        academic_retention::engine::shred_with_tombstone(
            &mut journal,
            &vault,
            &subject_after,
            &stone,
        )?;
    }
    tombstone::write_into_backup(&backup, &stone)?;

    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
    let destination = fixture.work_path("restored-pre-rotation");
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
        receipt.re_deleted_locators,
        vec![hex_lower(subject_before.vault_locator.as_bytes())],
        "the restore did not re-delete the copy under the pre-rotation locator"
    );
    assert!(
        receipt.absent_locators.is_empty(),
        "a tombstone that reached its object was reported absent"
    );

    let restored_keys =
        ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
    let restored_vault =
        EncryptedVault::open(&destination, restored_keys.keyring(&recovered.master)?)?;
    let kek = domain_kek_of(&recovered.master)?;
    assert_eq!(
        probe_header(&restored_vault.layout().object_path(&subject_before)?, &kek),
        HeaderProbe::Shredded,
        "the restore resurrected an artifact the profile had deleted"
    );
    Ok(())
}

/// A tombstone that reaches nothing is carried on the receipt.
///
/// `T114`'s reproduction of the second half: `apply_tombstones` returned the
/// unmatched records and `restore_encrypted_profile` dropped them, so a
/// deletion the backup could not carry out was indistinguishable from one it
/// did. The receipt now carries both lists.
#[test]
fn a_tombstone_that_reaches_nothing_is_reported_on_the_receipt() -> TestResult {
    let fixture = EncryptedFixture::new("seam-tombstone-absent")?;
    let backup = take_backup(&fixture, "backup-for-absent-tombstone")?;
    let stone = academic_retention::BackupTombstone::new(
        hex_lower(&[0x32_u8; 16]),
        [0x99; 32],
        1_700_000_000_004,
    );
    tombstone::write_into_backup(&backup, &stone)?;

    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
    let receipt = restore_encrypted_profile(
        &backup,
        &fixture.work_path("restored-absent"),
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert!(receipt.re_deleted_locators.is_empty());
    assert_eq!(
        receipt.absent_locators,
        vec![hex_lower(&[0x99_u8; 32])],
        "an unmatched tombstone was dropped instead of reported"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P2-A, over the real store
// ---------------------------------------------------------------------------

/// A retirement before the store row is written is refused.
///
/// `T114`'s reproduction: the journal recorded the unit as migrated and the
/// caller passed the re-seal result as "the descriptor the store now resolves
/// to", which was a statement, not a fact — the migration row had not been
/// written. The superseded object's key slot was destroyed while the store
/// still named it, so no key on disk opened that artifact.
#[test]
fn a_retirement_before_the_store_row_is_refused() -> TestResult {
    let fixture = EncryptedFixture::new("seam-retire-before-row")?;
    let before = fixture.descriptors()?;
    let target = VaultMasterKey::generate()?;
    let source_kek = domain_kek_of(fixture.master())?;

    // The journal half only: reseal, `UnitResealed`, `UnitMigrated`. No
    // retention action and no migration row, which is the documented kill point.
    let unit = RotationUnit::object(*before[0].vault_locator.as_bytes());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x44; 16]),
        PROFILE_ID,
        KeyGeneration::of(fixture.master(), PROFILE_ID)?,
        KeyGeneration::of(&target, PROFILE_ID)?,
        vec![unit.clone()],
    )?;
    let mut journal = journal_of(&fixture)?;
    let source_vault = fixture.open_vault()?;
    let target_vault = fixture.open_vault_under(&target)?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;
    let resealed = engine.rotate_object(&mut journal, &unit, &before[0])?;
    engine.complete(&mut journal)?;

    let resolved = fixture.descriptors()?;
    assert_eq!(
        resolved[0].vault_locator, before[0].vault_locator,
        "precondition: the store still resolves to the superseded locator"
    );

    let store = fixture.open_store()?;
    let refused = retire_superseded_object(
        &mut journal,
        &target_vault,
        &unit,
        &before[0],
        &StoreCanonicalReference::new(&store),
    );
    let message = refused
        .err()
        .ok_or("a retirement ran before the store moved the reference")?
        .to_string();
    assert!(
        message.contains("the store resolves the artifact to"),
        "the refusal did not name the reference: {message}"
    );
    assert_eq!(
        probe_header(&target_vault.layout().object_path(&before[0])?, &source_kek),
        HeaderProbe::Opened,
        "the object the store still resolves to was destroyed"
    );
    source_vault.verify_sealed_object(&resolved[0])?;
    drop(store);
    let _ = resealed;
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P2-C
// ---------------------------------------------------------------------------

/// Rotating back to a generation an artifact has already been under is refused.
///
/// `T114`'s reproduction: `artifact_descriptor_migration` carries
/// `UNIQUE (artifact_id, vault_locator)` and a locator is a deterministic
/// function of the generation, so the third rotation of a `G1 → G2 → G1 → G2`
/// sequence hit the constraint *after* the journal had already recorded the
/// unit as resealed and migrated. The rotation could not finish, and the
/// journal and the store named different objects with no kill involved.
///
/// The refusal is now a named one the caller can ask for before the journal
/// moves, which is what the rotation contract's re-rotation section requires.
#[test]
fn a_rotation_back_to_a_used_generation_is_refused_before_the_journal_moves() -> TestResult {
    let mut fixture = EncryptedFixture::new("seam-re-rotation")?;
    let original = fixture.descriptors()?;
    let second = VaultMasterKey::generate()?;
    rotate_profile(&mut fixture, second, 0x51, 0x0a00)?;

    // The preflight a rotation orchestrator runs before it journals anything:
    // the locator the objects would land back on is already in the chain, so
    // the rotation is refused while the journal is still untouched.
    let store = fixture.open_store()?;
    let recorded = store.descriptor_migrations()?;
    assert!(
        recorded.iter().any(|migration| {
            migration.artifact_id == *original[0].id.as_bytes()
                && migration.superseded_locator == original[0].vault_locator
        }),
        "the chain does not hold the locator the artifact started under"
    );
    assert!(
        store.locator_is_already_in_chain(original[0].id, &original[0].vault_locator)?,
        "the preflight did not see the locator a rotation back would reuse"
    );
    assert!(
        !store.locator_is_already_in_chain(
            original[0].id,
            &academic_domain::VaultLocator::from_bytes([0x77; 32]),
        )?,
        "the preflight claims a locator the chain has never held"
    );
    drop(store);

    // Rotating back to the original generation would supersede a locator the
    // chain has already recorded, and the store refuses that row by name rather
    // than as a raw constraint violation.
    let descriptors = fixture.descriptors()?;
    let back = ArtifactDescriptor {
        vault_locator: original[0].vault_locator.clone(),
        ..descriptors[0].clone()
    };
    let action: academic_domain::RetentionActionId = id(0x0a90)?;
    let sequence = fixture
        .open_store()?
        .next_descriptor_migration_seq(descriptors[0].id)?;
    let record = DescriptorMigration::of(*action.as_bytes(), &descriptors[0], sequence, &back);
    fixture.accept_retention_action(action, record.record_digest())?;
    let vault = fixture.open_vault()?;
    let mut store = fixture.open_store()?;
    let refused = store.record_descriptor_migration(&record, &back, &vault);
    let message = refused
        .err()
        .ok_or("a rotation back to a used generation recorded a second chain entry")?
        .to_string();
    assert!(
        message.contains("already recorded this locator"),
        "the refusal did not name the chain constraint: {message}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T116 P1-N1
// ---------------------------------------------------------------------------

/// A profile that crypto-shredded an artifact before a rotation still backs up,
/// and both backups restore with the deletion still in force.
///
/// `T116`'s reproduction: a locator is a function of `KEK_d` and a destroyed key
/// slot can never be re-sealed, so a shredded artifact's `artifact_descriptor`
/// row keeps the locator of the generation it was destroyed under while every
/// other row moves. The vault re-derived that locator from the rotated keyring
/// and refused with `LocatorMismatch` before it read a byte, so the backup's
/// shredded-object branch was never reached and every backup of that profile
/// was refused — permanently, since the row is append-only. The contract said a
/// crypto-shredded object does not stop a backup.
///
/// The chain here is the whole one: delete, rotate the artifacts that are left,
/// back up, restore, and restore the pre-deletion backup the tombstone was
/// written into.
#[test]
fn a_deletion_before_a_rotation_still_backs_up_and_restores() -> TestResult {
    let mut fixture = EncryptedFixture::new("seam-delete-then-rotate")?;
    let before = fixture.descriptors()?;
    let subject = before[0].clone();
    let live: Vec<ArtifactDescriptor> = before[1..].to_vec();
    let pre_deletion_backup = take_backup(&fixture, "backup-before-deletion")?;

    // The deletion, the way the product does it.
    let store = fixture.open_store()?;
    let stone = deletion_tombstone(
        &store,
        hex_lower(&[0x51_u8; 16]),
        &subject,
        1_700_000_000_010,
    )?;
    drop(store);
    assert!(
        stone.superseded_locators.is_empty(),
        "the artifact had not moved yet, so its chain names no superseded locator"
    );
    {
        let vault = fixture.open_vault()?;
        let mut journal = journal_of(&fixture)?;
        academic_retention::engine::shred_with_tombstone(&mut journal, &vault, &subject, &stone)?;
    }
    tombstone::write_into_backup(&pre_deletion_backup, &stone)?;

    // A plan that names the destroyed object cannot move it: the reseal opens
    // the source and there is no key slot left to open it with. Nothing is
    // journalled, because the refusal is before the first record.
    let target = VaultMasterKey::generate()?;
    let shredded_unit = RotationUnit::object(*subject.vault_locator.as_bytes());
    let doomed = RotationPlan::new(
        RotationId::from_bytes([0x50; 16]),
        PROFILE_ID,
        KeyGeneration::of(fixture.master(), PROFILE_ID)?,
        KeyGeneration::of(&target, PROFILE_ID)?,
        vec![shredded_unit.clone()],
    )?;
    {
        let mut journal = journal_of(&fixture)?;
        let entries_before = journal.entries().count();
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target)?;
        let refused = RotationEngine::new(&doomed, &source_vault, &target_vault)
            .rotate_object(&mut journal, &shredded_unit, &subject)
            .err()
            .ok_or("a rotation re-sealed a crypto-shredded object")?
            .to_string();
        assert!(
            refused.contains("crypto-shredded"),
            "the refusal did not name the destroyed key slot: {refused}"
        );
        assert_eq!(journal_of(&fixture)?.entries().count(), entries_before);
    }

    // The plan names what is still there. The shredded artifact's row stays
    // where it is: nothing can move a reference to an object no key opens.
    let migrated = rotate_objects(&mut fixture, target, 0x51, 0x0a40, &live)?;
    fixture.rewrap_recovery_recipients()?;
    let after = fixture.descriptors()?;
    assert_eq!(
        after[0].vault_locator, subject.vault_locator,
        "the deleted artifact's reference could not have moved"
    );
    assert_eq!(after[1].vault_locator, migrated[0].vault_locator);

    // This is the call T116 found refused.
    let post_rotation_backup = take_backup(&fixture, "backup-after-rotation")?;
    let (backup_root, _) = open_backup_with_secret(
        &post_rotation_backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&post_rotation_backup, &backup_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
    let restored_root = fixture.work_path("restored-after-rotation");
    let receipt = restore_encrypted_profile(
        &post_rotation_backup,
        &restored_root,
        &NativePathProbe::default(),
        &backup_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert_eq!(receipt.restored_object_count, after.len() as u64);

    // The destroyed object arrives destroyed and the live one opens, both at
    // the names the store resolves to on the restored profile.
    let keys = ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
    let restored_vault = EncryptedVault::open(&restored_root, keys.keyring(&recovered.master)?)?;
    let kek = domain_kek_of(&recovered.master)?;
    assert_eq!(
        probe_header(&restored_vault.layout().object_path(&subject)?, &kek),
        HeaderProbe::Shredded,
        "the restore resurrected an artifact the profile had deleted"
    );
    restored_vault.verify_sealed_object(&migrated[0])?;

    // The backup taken before the deletion carries the tombstone, and the
    // restore re-applies it at the one name that artifact has ever had.
    let (older_root, _) = open_backup_with_secret(
        &pre_deletion_backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let older_verified = verify_encrypted_backup_directory(&pre_deletion_backup, &older_root)?;
    let older_recovered = recover_profile_keys(&older_verified, &recovery_secret(), 5_000)?;
    let older_restored = fixture.work_path("restored-before-deletion");
    let older_receipt = restore_encrypted_profile(
        &pre_deletion_backup,
        &older_restored,
        &NativePathProbe::default(),
        &older_root,
        &older_recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert_eq!(
        older_receipt.re_deleted_locators,
        vec![hex_lower(subject.vault_locator.as_bytes())],
        "the restore did not re-delete the copy the pre-deletion backup holds"
    );
    assert!(older_receipt.absent_locators.is_empty());
    let older_keys = ProfileKeys::derive(
        &older_recovered.master,
        older_recovered.profile_id,
        &[domain_id()?],
    )?;
    let older_vault = EncryptedVault::open(
        &older_restored,
        older_keys.keyring(&older_recovered.master)?,
    )?;
    assert_eq!(
        probe_header(
            &older_vault.layout().object_path(&subject)?,
            &domain_kek_of(&older_recovered.master)?
        ),
        HeaderProbe::Shredded,
        "the pre-deletion backup restored an artifact the profile had deleted"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The obligation the engine's own two vaults are, and what it costs
// ---------------------------------------------------------------------------

/// An engine built on a target vault outside the plan's generations leaves a
/// profile no generation the plan names can back up.
///
/// `RotationEngine::new` takes two vaults and can check neither against the
/// plan: a vault holds `KEK_d` and the locator key derived from it, never the
/// Vault Master Key a generation name is a function of. So this one stays an
/// obligation on an orchestrator while the database unit's executor is bound,
/// and what the obligation is worth is here rather than asserted in prose.
///
/// The journal does not lie about the objects — an object unit records the
/// locator the reseal actually produced and the store row is verified against
/// the object it names. What is false is the plan: `RotationStarted` names a
/// target generation nothing is under, and the retirement gates all compare the
/// journal with the store rather than with a key, so they would let the copies
/// that still open go.
#[test]
fn an_engine_outside_the_plans_generations_leaves_a_profile_no_backup_can_take() -> TestResult {
    let mut fixture = EncryptedFixture::new("seam-engine-generations")?;
    let descriptors = fixture.descriptors()?;
    let planned = VaultMasterKey::generate()?;
    let elsewhere = VaultMasterKey::generate()?;

    let mut units: Vec<RotationUnit> = descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let database_unit = RotationUnit::store_database(PROFILE_ID);
    units.push(database_unit.clone());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x53; 16]),
        PROFILE_ID,
        KeyGeneration::of(fixture.master(), PROFILE_ID)?,
        KeyGeneration::of(&planned, PROFILE_ID)?,
        units.clone(),
    )?;

    let mut journal = journal_of(&fixture)?;
    {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&elsewhere)?;
        RotationEngine::new(&plan, &source_vault, &target_vault).begin(&mut journal)?;
    }
    // Every object moves to the generation the *vault* holds, not the one the
    // plan names, and each move is recorded truthfully.
    let mut migrated = Vec::with_capacity(descriptors.len());
    for (index, descriptor) in descriptors.iter().enumerate() {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&elsewhere)?;
        let resealed = RotationEngine::new(&plan, &source_vault, &target_vault).rotate_object(
            &mut journal,
            &units[index],
            descriptor,
        )?;
        drop(source_vault);
        drop(target_vault);
        let action: academic_domain::RetentionActionId = id(0x0a60 + u64::try_from(index)?)?;
        let sequence = fixture
            .open_store()?
            .next_descriptor_migration_seq(descriptor.id)?;
        let record = DescriptorMigration::of(*action.as_bytes(), descriptor, sequence, &resealed);
        fixture.accept_retention_action(action, record.record_digest())?;
        let target_vault = fixture.open_vault_under(&elsewhere)?;
        let mut store = fixture.open_store()?;
        store.record_descriptor_migration(&record, &resealed, &target_vault)?;
        drop(store);
        drop(target_vault);
        migrated.push(resealed);
    }

    // The database unit's executor *is* bound, so it moves to the plan's target
    // and the rotation completes. The halves are now under two generations and
    // the journal names a third state that is true of neither.
    {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&planned)?;
        let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
        let probe = NativePathProbe::default();
        engine.rotate_store_database(
            &mut journal,
            &database_unit,
            &StoreDatabaseRekey::new(
                fixture.profile_root(),
                &probe,
                PROFILE_ID,
                fixture.master(),
                &planned,
            ),
        )?;
        engine.complete(&mut journal)?;
    }
    let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    assert!(state.is_complete());
    assert_eq!(state.target(), KeyGeneration::of(&planned, PROFILE_ID)?);

    // Only the generation the vault held opens the objects.
    let elsewhere_kek = domain_kek_of(&elsewhere)?;
    let planned_kek = domain_kek_of(&planned)?;
    let layout_vault = fixture.open_vault_under(&elsewhere)?;
    for descriptor in &migrated {
        let path = layout_vault.layout().object_path(descriptor)?;
        assert_eq!(probe_header(&path, &elsewhere_kek), HeaderProbe::Opened);
        assert_ne!(probe_header(&path, &planned_kek), HeaderProbe::Opened);
    }
    drop(layout_vault);

    // Neither generation backs the profile up: under the plan's target the
    // objects no longer derive their locators, and under the objects' own the
    // database does not open. The refusal is before anything is published.
    let (backup_root, recipients) = backup_key_set()?;
    for (label, master) in [("planned", &planned), ("elsewhere", &elsewhere)] {
        let keys = ProfileKeys::derive(master, PROFILE_ID, &[domain_id()?])?;
        let destination = fixture.work_path(&format!("split-backup-{label}"));
        let refused = backup_encrypted_profile(
            fixture.profile_root(),
            &destination,
            master,
            &keys,
            &BackupPlan {
                recovery_profile: RecoveryProfile::DevicePlusPhrase,
                backup_root: &backup_root,
                backup_recipients: &recipients,
                profile_recovery_recipients: &fixture.recovery_recipients_cbor()?,
            },
        );
        assert!(
            refused.is_err(),
            "a backup of a profile the plan's generations do not describe was published"
        );
        assert!(
            !destination.exists(),
            "the refused backup left a directory behind"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// T116 P2-N2
// ---------------------------------------------------------------------------

/// A `STORE_DATABASE` executor that does not hold the plan's generations is
/// refused before it runs.
///
/// `T116`'s reproduction: `rotate_store_database` called the executor and then
/// journalled `store_database_target_id(plan.target)` whatever it had done, and
/// the executor reported nothing about which pair of keys it held. An executor
/// built from the plan's source and an unrelated third master rekeyed the
/// database to that third generation while the journal recorded the unit
/// migrated to the plan's target with `RotationCompleted` behind it — a database
/// neither generation the journal names can open, and `retire_generation`
/// cleared to remove the only records that still could.
#[test]
fn a_store_database_executor_outside_the_plans_generations_is_refused() -> TestResult {
    let fixture = EncryptedFixture::new("seam-executor-binding")?;
    let descriptors = fixture.descriptors()?;
    let target = VaultMasterKey::generate()?;
    let stray = VaultMasterKey::generate()?;

    let mut units: Vec<RotationUnit> = descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let database_unit = RotationUnit::store_database(PROFILE_ID);
    units.push(database_unit.clone());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x52; 16]),
        PROFILE_ID,
        KeyGeneration::of(fixture.master(), PROFILE_ID)?,
        KeyGeneration::of(&target, PROFILE_ID)?,
        units,
    )?;

    let mut journal = journal_of(&fixture)?;
    let source_vault = fixture.open_vault()?;
    let target_vault = fixture.open_vault_under(&target)?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;
    // Every object first, so the only unit left is the database one and the
    // refusal below is the only thing that can stop the rotation.
    for (unit, descriptor) in plan.units().iter().zip(descriptors.iter()) {
        engine.rotate_object(&mut journal, unit, descriptor)?;
    }

    let probe = NativePathProbe::default();
    let refused = engine
        .rotate_store_database(
            &mut journal,
            &database_unit,
            &StoreDatabaseRekey::new(
                fixture.profile_root(),
                &probe,
                PROFILE_ID,
                fixture.master(),
                &stray,
            ),
        )
        .err()
        .ok_or("an executor outside the plan's generations rekeyed the database")?;
    let message = refused.to_string();
    assert!(
        message.contains("does not hold the generations this rotation plans"),
        "the refusal did not name the generations the executor holds: {message}"
    );

    // The guard runs before the rekey, so the database is where the plan left
    // it and the journal says nothing about the unit.
    assert!(
        academic_store::cipher::open_encrypted_profile(
            fixture.profile_root(),
            &probe,
            &fixture.master().derive_store_key(PROFILE_ID)?
        )
        .is_ok(),
        "the refused executor rekeyed the database anyway"
    );
    let state = RotationState::replay(journal.entries())?
        .ok_or("the journal holds no rotation after the refusal")?;
    let database_state = state
        .units()
        .iter()
        .find(|unit| unit.unit.unit_id_hex() == database_unit.unit_id_hex())
        .ok_or("the database unit is not in the replayed plan")?;
    assert_eq!(database_state.target_locator, None);
    let stopped = engine
        .complete(&mut journal)
        .err()
        .ok_or("a rotation completed with its database unit never run")?;
    assert!(
        stopped.to_string().contains("never ran its"),
        "the completion did not name the database unit: {stopped}"
    );
    Ok(())
}
