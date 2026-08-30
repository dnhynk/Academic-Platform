//! Where `P2-K5`'s rotation and deletion meet `P2-K2`'s store and `P2-K4`'s
//! backup and restore.
//!
//! Everything here needs all three lanes in one process, which is why it lives
//! in the encrypted portability suite rather than in `academic-retention`: the
//! retention crate cannot depend on the store or on the backup boundary, so a
//! test written there can only imitate them. That imitation is precisely what
//! the `T111` audit found — a named acceptance row that copied files with
//! `fs::copy` and applied tombstones itself, passing while the product restore
//! did neither. `backup_tombstone_is_present_and_re_deletes_on_restore` now
//! lives here and calls `restore_encrypted_profile`.
//!
//! Each test below names the `T111` finding it closes.
//!
//! This whole file compiles only in the encrypted lane:
//! `cargo test -p academic-portability --no-default-features --features encrypted-portability`.

#![cfg(feature = "encrypted-portability")]

mod encrypted_support;

use std::fs;

use academic_domain::ArtifactDescriptor;
use academic_portability::{
    PortabilityError,
    encrypted::{
        ProfileKeys,
        backup::{BackupPlan, backup_encrypted_profile, verify_encrypted_backup_directory},
        restore::{
            EncryptedRestorePlan, open_backup_with_secret, recover_profile_keys,
            restore_encrypted_profile,
        },
        rotation::{StoreCanonicalReference, StoreDatabaseRekey, deletion_tombstone},
    },
};
use academic_recovery::{BackupRecipientKind, RecoveryProfile};
use academic_retention::{
    AppendOnlyJournal, BackupTombstone, RotationId, RotationPlan, RotationUnit,
    engine::{
        HeaderProbe, RotationEngine, probe_header, retire_superseded_object, shred_with_tombstone,
    },
    journal::ROTATION_JOURNAL_RELATIVE_PATH,
    rotation::KeyGeneration,
    rotation::StoreDatabaseRekey as StoreDatabaseRekeyOutcome,
    tombstone,
};
use academic_store::descriptor_migration::DescriptorMigration;
use academic_vault::SealedObjectVerifier as _;
use encrypted_support::{EncryptedFixture, TestResult, backup_key_set, domain_id, recovery_secret};

use academic_crypto::VaultMasterKey;
use academic_store::path_policy::NativePathProbe;

/// Takes a real encrypted backup of `fixture` under the generation `master` names.
fn take_backup(
    fixture: &EncryptedFixture,
    master: &VaultMasterKey,
    label: &str,
) -> TestResult<std::path::PathBuf> {
    let destination = fixture.work_path(label);
    let (backup_root, recipients) = backup_key_set()?;
    backup_encrypted_profile(
        fixture.profile_root(),
        &destination,
        master,
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

/// Runs a complete rotation and moves every canonical reference with it.
///
/// This is the product sequence in the order the invariant depends on: re-seal,
/// journal, accept the `RETENTION_ACTION_RECORDED` event that authorizes the
/// move, append the typed migration row, and only then complete the rotation.
///
/// The plan holds the `STORE_DATABASE` unit as well as one unit per object, and
/// it is rotated last through `StoreDatabaseRekey`, the encrypted lane's
/// binding of `P2-K2`'s `PRAGMA rekey`. Leaving it out is what produced a
/// profile with its database under one generation and its objects under
/// another: still usable on the machine that holds both keys, and restorable
/// from no backup of it, because a restore recovers one master and derives both
/// halves from it. The fixture adopts the target generation afterwards, which
/// is the caller-side half of the same move.
fn rotate_every_object(
    fixture: &mut EncryptedFixture,
    target: VaultMasterKey,
) -> TestResult<Vec<ArtifactDescriptor>> {
    let source_generation = KeyGeneration::of(fixture.master(), encrypted_support::PROFILE_ID)?;
    let target_generation = KeyGeneration::of(&target, encrypted_support::PROFILE_ID)?;
    let descriptors = fixture.descriptors()?;
    let mut units: Vec<RotationUnit> = descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let database_unit = RotationUnit::store_database(encrypted_support::PROFILE_ID);
    units.push(database_unit.clone());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x31; 16]),
        encrypted_support::PROFILE_ID,
        source_generation,
        target_generation,
        units.clone(),
    )?;

    let mut journal =
        AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let mut migrated = Vec::with_capacity(descriptors.len());
    for (index, descriptor) in descriptors.iter().enumerate() {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target)?;
        let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
        if index == 0 {
            engine.begin(&mut journal)?;
        }
        let resealed = engine.rotate_object(&mut journal, &units[index], descriptor)?;
        drop(source_vault);
        drop(target_vault);

        // The aggregate identity is chosen first, so the event and the typed
        // row commit to one digest over one finished record.
        let action: academic_domain::RetentionActionId =
            encrypted_support::id(0x0a00_u64 + u64::try_from(index)?)?;
        // The chain position comes from the store, which is what a resume after
        // a kill reads rather than assuming this is the artifact's first move.
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
    assert_eq!(
        fixture.open_store()?.descriptor_migrations()?.len(),
        descriptors.len(),
        "the rotation did not record one migration per unit"
    );

    // The database moves last. While objects are still moving, the store key
    // that opens the ones already migrated and the ones not yet migrated is the
    // same one, and it has to stay in force until every object has landed.
    {
        let source_vault = fixture.open_vault()?;
        let target_vault = fixture.open_vault_under(&target)?;
        let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
        let probe = NativePathProbe::default();
        let executor = StoreDatabaseRekey::new(
            fixture.profile_root(),
            &probe,
            encrypted_support::PROFILE_ID,
            fixture.master(),
            &target,
        );
        let outcome = engine.rotate_store_database(&mut journal, &database_unit, &executor)?;
        assert_eq!(outcome, StoreDatabaseRekeyOutcome::Rekeyed);
        engine.complete(&mut journal)?;
    }

    // The caller-side half: from here the profile is wholly under the target
    // generation, so `fixture.master()` is that generation and nothing derived
    // from the superseded one opens either half.
    fixture.adopt_generation(target)?;
    Ok(migrated)
}

// ---------------------------------------------------------------------------
// T111 P1-1
// ---------------------------------------------------------------------------

/// The canonical reference follows the object a rotation moved.
///
/// `T111`'s reproduction A: after a complete rotation the new keyring could not
/// verify the stored descriptor, because the store still named the superseded
/// locator and nothing in the workspace could move it. It now resolves through
/// the appended migration chain, so `read_artifact_descriptors` names the
/// object the new key opens.
#[test]
fn store_descriptors_follow_a_completed_rotation() -> TestResult {
    let mut fixture = EncryptedFixture::new("rotation-reference")?;
    let before = fixture.descriptors()?;
    assert_eq!(before.len(), 2, "the corpus is two artifacts");

    // Held before the rotation: the superseded generation's keyring, which is
    // what proves the old key stops opening the reachable object. Neither a
    // master key nor a KEK is cloneable, so it is captured rather than re-derived.
    let stale = fixture.open_vault()?;
    let migrated = rotate_every_object(&mut fixture, VaultMasterKey::generate()?)?;

    let after = fixture.descriptors()?;
    assert_eq!(after.len(), before.len());
    for (resolved, expected) in after.iter().zip(migrated.iter()) {
        assert_eq!(
            resolved.vault_locator, expected.vault_locator,
            "the store still names the superseded locator"
        );
        assert_ne!(
            resolved.vault_locator,
            before
                .iter()
                .find(|descriptor| descriptor.id == resolved.id)
                .map(|descriptor| descriptor.vault_locator.clone())
                .ok_or("a descriptor disappeared")?,
            "the rotation did not move the locator at all"
        );
    }

    // The whole point: the new generation's vault authenticates every stored
    // descriptor. Before the migration chain existed this was `LocatorMismatch`.
    let vault = fixture.open_vault()?;
    for descriptor in &after {
        vault.verify_sealed_object(descriptor)?;
    }

    // And the old generation no longer opens what the store points at.
    for descriptor in &after {
        assert!(
            stale.verify_sealed_object(descriptor).is_err(),
            "the superseded generation still opens the reachable object"
        );
    }
    Ok(())
}

/// A backup taken after a rotation copies the objects the new key opens.
///
/// `T111`'s scenario for P1-1: `read_artifact_descriptors` feeds the backup's
/// object closure, so a store that had not moved its references made every
/// post-rotation backup fail.
#[test]
fn a_backup_after_a_rotation_closes_over_the_migrated_objects() -> TestResult {
    let mut fixture = EncryptedFixture::new("rotation-backup")?;
    let migrated = rotate_every_object(&mut fixture, VaultMasterKey::generate()?)?;

    let destination = take_backup(&fixture, fixture.master(), "backup-after-rotation")?;
    let (backup_root, _) = open_backup_with_secret(
        &destination,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&destination, &backup_root)?;
    assert_eq!(verified.manifest.semantic.objects.len(), migrated.len());
    for (entry, descriptor) in verified
        .manifest
        .semantic
        .objects
        .iter()
        .zip(migrated.iter())
    {
        assert_eq!(
            entry.vault_locator,
            encrypted_support::hex_lower(descriptor.vault_locator.as_bytes()),
            "the backup manifest names a locator the rotation superseded"
        );
    }
    Ok(())
}

/// A migration no canonical event authorized cannot move a reference.
///
/// Migration `0005`'s two triggers are the enforcement, and this is what makes
/// them more than a comment: a row whose `record_digest` is not the
/// `source_digest` its retention action carries is refused, and so is a row that
/// does not continue the artifact's chain. Both refusals leave the reference
/// where it was, which the resolved descriptor is then read back to confirm.
#[test]
fn a_descriptor_migration_no_event_authorized_is_refused() -> TestResult {
    let mut fixture = EncryptedFixture::new("migration-authority")?;
    let descriptors = fixture.descriptors()?;
    let subject = descriptors.first().ok_or("the corpus is empty")?.clone();
    let target = VaultMasterKey::generate()?;

    // A real re-seal, so the object at the new locator genuinely exists and
    // the refusals below are about authority rather than about missing bytes.
    let source_vault = fixture.open_vault()?;
    let target_vault = fixture.open_vault_under(&target)?;
    let resealed = source_vault.reseal(&subject, &target_vault)?;
    let migrated = resealed.resealed.descriptor().clone();
    drop(source_vault);
    drop(target_vault);

    let action: academic_domain::RetentionActionId = encrypted_support::id(0x0b01)?;
    let honest = DescriptorMigration::of(*action.as_bytes(), &subject, 1, &migrated);

    // No event at all: the foreign key has nothing to point at.
    {
        let vault = fixture.open_vault_under(&target)?;
        let mut store = fixture.open_store()?;
        assert!(
            store
                .record_descriptor_migration(&honest, &migrated, &vault)
                .is_err(),
            "a migration with no retention action moved a reference"
        );
    }

    // An event that authorizes a *different* move: the digests disagree.
    let second = descriptors
        .get(1)
        .ok_or("the corpus is one artifact")?
        .clone();
    let other = DescriptorMigration::of(*action.as_bytes(), &second, 1, &migrated);
    fixture.accept_retention_action(action, other.record_digest())?;
    {
        let vault = fixture.open_vault_under(&target)?;
        let mut store = fixture.open_store()?;
        let refused = store.record_descriptor_migration(&honest, &migrated, &vault);
        let message = refused
            .err()
            .ok_or("a migration its event did not authorize moved a reference")?
            .to_string();
        assert!(
            message.contains("retention action authorized"),
            "the refusal did not name the missing authority: {message}"
        );
    }

    // An event that authorizes a move starting from a locator this artifact
    // never had: the chain trigger refuses it even though the digests agree.
    let forked_action: academic_domain::RetentionActionId = encrypted_support::id(0x0b02)?;
    let forked = DescriptorMigration {
        retention_action_id: *forked_action.as_bytes(),
        superseded_locator: second.vault_locator.clone(),
        ..DescriptorMigration::of(*forked_action.as_bytes(), &subject, 1, &migrated)
    };
    fixture.accept_retention_action(forked_action, forked.record_digest())?;
    {
        let vault = fixture.open_vault_under(&target)?;
        let mut store = fixture.open_store()?;
        let refused = store.record_descriptor_migration(&forked, &migrated, &vault);
        let message = refused
            .err()
            .ok_or("a migration that forks the reference chain was accepted")?
            .to_string();
        assert!(
            message.contains("continue the reference chain"),
            "the refusal did not name the broken chain: {message}"
        );
    }

    // Nothing moved: the store still resolves to the signed locator.
    let after = fixture.descriptors()?;
    assert_eq!(
        after
            .first()
            .map(|descriptor| descriptor.vault_locator.clone()),
        Some(subject.vault_locator.clone()),
        "a refused migration moved the reference anyway"
    );

    // And the honest one, with its own event, is accepted — so the refusals
    // above are about authority and not about the path being closed.
    let honest_action: academic_domain::RetentionActionId = encrypted_support::id(0x0b03)?;
    let honest = DescriptorMigration::of(*honest_action.as_bytes(), &subject, 1, &migrated);
    fixture.accept_retention_action(honest_action, honest.record_digest())?;
    let vault = fixture.open_vault_under(&target)?;
    let mut store = fixture.open_store()?;
    store.record_descriptor_migration(&honest, &migrated, &vault)?;
    drop(store);
    drop(vault);
    let moved = fixture.descriptors()?;
    assert_eq!(
        moved
            .first()
            .map(|descriptor| descriptor.vault_locator.clone()),
        Some(migrated.vault_locator.clone())
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T111 P2-8
// ---------------------------------------------------------------------------

/// After retirement, exactly one key opens the artifact — as files, not as a
/// journal reading.
///
/// `T111`'s observation: a completed rotation left the superseded object fully
/// readable under the old key, so a recipient the rotation revoked kept opening
/// every pre-rotation copy in the live tree forever. `ADR-004` quarantines the
/// superseded object and leaves the collection point open; this is that point.
/// The test states both halves of the window: openable while the object is
/// merely unreferenced, and opened by nothing once it is retired.
#[test]
fn a_retired_source_object_is_opened_by_neither_generation() -> TestResult {
    let mut fixture = EncryptedFixture::new("rotation-retirement")?;
    let before = fixture.descriptors()?;
    // The superseded generation's KEK is derived before the rotation moves the
    // profile onto the next one: a `DomainKek` is owned, a `VaultMasterKey` is
    // not cloneable, and after `rotate_every_object` the fixture holds the
    // generation the rotation moved to.
    let source_kek = domain_kek_of(fixture.master())?;
    rotate_every_object(&mut fixture, VaultMasterKey::generate()?)?;
    let after = fixture.descriptors()?;

    let target_kek = domain_kek_of(fixture.master())?;
    let vault = fixture.open_vault()?;

    // Before retirement: the superseded copy is unreferenced but intact, which
    // is exactly the state the audit found and the window `ADR-004` allows.
    let superseded = before.first().ok_or("the corpus is empty")?;
    let current = after.first().ok_or("the corpus is empty")?;
    let superseded_path = vault.layout().object_path(superseded)?;
    assert_eq!(
        probe_header(&superseded_path, &source_kek),
        HeaderProbe::Opened,
        "the superseded object was already unreadable before it was retired"
    );

    let unit = RotationUnit::object(*superseded.vault_locator.as_bytes());
    let mut journal =
        AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    // The gate is the store's own resolution, not the caller's word for it:
    // `StoreCanonicalReference` walks the signed row through its migration
    // chain, exactly as a backup and a restore do.
    let store = fixture.open_store()?;
    retire_superseded_object(
        &mut journal,
        &vault,
        &unit,
        superseded,
        &StoreCanonicalReference::new(&store),
    )?;
    drop(store);

    // After it: neither generation opens the superseded copy, and the current
    // one still opens under the generation the rotation moved to.
    assert_eq!(
        probe_header(&superseded_path, &source_kek),
        HeaderProbe::Shredded,
        "a holder of the superseded generation still opens the retired object"
    );
    assert_eq!(
        probe_header(&superseded_path, &target_kek),
        HeaderProbe::Shredded
    );
    assert_eq!(
        probe_header(&vault.layout().object_path(current)?, &target_kek),
        HeaderProbe::Opened,
        "the retirement reached the object the store resolves to"
    );
    vault.verify_sealed_object(current)?;

    // Retiring an object the store still resolves to is refused, because that
    // would leave an artifact no key opens. The unit here names the object the
    // rotation moved *to*, so the reference the store resolves is not the one
    // that unit supersedes and the refusal is structural rather than advisory.
    let second = after.get(1).ok_or("the corpus is one artifact")?;
    let second_unit = RotationUnit::object(*second.vault_locator.as_bytes());
    let store = fixture.open_store()?;
    let refused = retire_superseded_object(
        &mut journal,
        &vault,
        &second_unit,
        second,
        &StoreCanonicalReference::new(&store),
    );
    assert!(
        refused.is_err(),
        "an object the store still names was retired"
    );
    drop(store);

    Ok(())
}

// ---------------------------------------------------------------------------
// T111 P1-2 (a)
// ---------------------------------------------------------------------------

/// A profile that crypto-shredded an artifact can still be backed up.
///
/// `T111`'s reproduction: one shredded object made `backup_encrypted_profile`
/// refuse the whole profile, because every `artifact_descriptor` row was
/// required to authenticate. A destroyed key slot is not damage.
#[test]
fn a_backup_is_still_possible_after_a_crypto_shred() -> TestResult {
    let fixture = EncryptedFixture::new("shred-backup")?;
    let before = take_backup(&fixture, fixture.master(), "backup-before-shred")?;
    assert!(before.is_dir());

    let descriptors = fixture.descriptors()?;
    let subject = descriptors.first().ok_or("the corpus is empty")?.clone();
    let stone = BackupTombstone::new(
        encrypted_support::hex_lower(&[0x21_u8; 16]),
        subject.id,
        *subject.vault_locator.as_bytes(),
        1_700_000_000_001,
    );
    {
        let vault = fixture.open_vault()?;
        let mut journal =
            AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        shred_with_tombstone(&mut journal, &vault, &subject, &stone)?;
    }

    let after = take_backup(&fixture, fixture.master(), "backup-after-shred")?;
    let (backup_root, _) = open_backup_with_secret(
        &after,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&after, &backup_root)?;
    assert_eq!(
        verified.manifest.semantic.objects.len(),
        descriptors.len(),
        "the shredded artifact left the object closure"
    );
    // The shredded object really is shredded, and its bytes really did travel
    // into the backup: a backup that silently dropped it would also make this
    // assertion pass on the live tree alone.
    let vault = fixture.open_vault()?;
    let kek = domain_kek_of(fixture.master())?;
    assert_eq!(
        probe_header(&vault.layout().object_path(&subject)?, &kek),
        HeaderProbe::Shredded
    );
    let copied = verified
        .manifest
        .semantic
        .objects
        .iter()
        .find(|object| object.artifact_id == subject.id.to_string())
        .ok_or("the shredded artifact is absent from the manifest")?;
    let copied_path = after.join(&copied.path);
    assert_eq!(probe_header(&copied_path, &kek), HeaderProbe::Shredded);
    assert_eq!(stone.locator.len(), 64);
    Ok(())
}

// ---------------------------------------------------------------------------
// backup_tombstone_is_present_and_re_deletes_on_restore  (T111 P1-2 b and c)
// ---------------------------------------------------------------------------

/// A deletion reaches the copy inside an already-taken backup: the tombstone is
/// present in the backup, the backup still verifies and restores, and the
/// product restore re-deletes.
///
/// This is the t068 section 5 `P2-K5` acceptance row. It used to live in
/// `academic-retention`, where it imitated a restore with `fs::copy` and
/// applied the tombstones itself; every assertion passed while
/// `restore_encrypted_profile` neither read a tombstone nor applied one. It
/// calls the product functions now: `backup_encrypted_profile`,
/// `tombstone::write_into_backup`, `verify_encrypted_backup_directory`, and
/// `restore_encrypted_profile`.
#[test]
fn backup_tombstone_is_present_and_re_deletes_on_restore() -> TestResult {
    let fixture = EncryptedFixture::new("tombstone-restore")?;
    let descriptors = fixture.descriptors()?;
    let subject = descriptors.first().ok_or("the corpus is empty")?.clone();
    let bystander = descriptors
        .get(1)
        .ok_or("the corpus is one artifact")?
        .clone();

    // The backup is taken before the deletion, so it holds both objects byte
    // for byte. That is exactly why shredding the live object is not enough.
    let backup = take_backup(&fixture, fixture.master(), "tombstone-backup")?;
    let (backup_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    verify_encrypted_backup_directory(&backup, &backup_root)?;

    // The deletion: shred live, then tombstone the published backup.
    let stone = BackupTombstone::new(
        encrypted_support::hex_lower(&[0x21_u8; 16]),
        subject.id,
        *subject.vault_locator.as_bytes(),
        1_700_000_000_001,
    );
    {
        let vault = fixture.open_vault()?;
        let mut journal =
            AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        shred_with_tombstone(&mut journal, &vault, &subject, &stone)?;
    }
    let written = tombstone::write_into_backup(&backup, &stone)?;
    assert!(written.is_file(), "the tombstone was not written");
    assert_eq!(
        written.file_name().and_then(|name| name.to_str()),
        Some(format!("{}.tombstone", stone.locator).as_str())
    );
    assert_eq!(tombstone::read_from_backup(&backup)?, vec![stone.clone()]);

    // A tombstoned backup still verifies: the tombstone is the one path the
    // sealed manifest cannot cover, and treating it as an inventory mismatch
    // made a deletion destroy its own backups.
    verify_encrypted_backup_directory(&backup, &backup_root)?;

    // The restore: the product one.
    let destination = fixture.work_path("tombstone-restored");
    let opened_root = backup_root;
    let verified = verify_encrypted_backup_directory(&backup, &opened_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
    let receipt = restore_encrypted_profile(
        &backup,
        &destination,
        &NativePathProbe::default(),
        &opened_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert_eq!(
        receipt.re_deleted_locators,
        vec![stone.locator.clone()],
        "the product restore did not re-apply the backup's tombstone"
    );

    // The physical consequence, probed with no vault at all: the restored copy
    // of the deleted artifact has no key slot, and the bystander is untouched.
    let restored_keys =
        ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
    let restored_vault = academic_vault::EncryptedVault::open(
        &destination,
        restored_keys.keyring(&recovered.master)?,
    )?;
    let kek = domain_kek_of(&recovered.master)?;
    let subject_path = restored_vault.layout().object_path(&subject)?;
    let bystander_path = restored_vault.layout().object_path(&bystander)?;
    assert_eq!(
        probe_header(&subject_path, &kek),
        HeaderProbe::Shredded,
        "the restore published a profile in which the deletion had not happened"
    );
    assert_eq!(
        probe_header(&bystander_path, &kek),
        HeaderProbe::Opened,
        "the restore re-deleted an object no tombstone named"
    );
    assert!(subject_path.is_file(), "the object file itself was removed");
    Ok(())
}

// ---------------------------------------------------------------------------
// A locator is not an identity, over the product restore
// ---------------------------------------------------------------------------

/// Registers the fixture's first bytes twice more, in a lineage that sorts
/// before its own and one that sorts after.
///
/// A locator is `HMAC(LOC_d, format || media || 0 || digest)` and carries no
/// permission lineage, so all three artifacts share it and sit at three paths.
fn same_bytes_in_three_lineages(
    fixture: &mut EncryptedFixture,
) -> TestResult<[ArtifactDescriptor; 3]> {
    let first_digest = fixture_first_digest(fixture)?;
    let mid = fixture
        .descriptors()?
        .into_iter()
        .find(|descriptor| descriptor.content_digest == first_digest)
        .ok_or("the fixture corpus holds no first artifact")?;
    let low = fixture.register_artifact_in_lineage(
        0x0290,
        0x0300,
        0x0490,
        encrypted_support::FIRST_ARTIFACT_BYTES,
    )?;
    let high = fixture.register_artifact_in_lineage(
        0x0299,
        0x0399,
        0x0499,
        encrypted_support::FIRST_ARTIFACT_BYTES,
    )?;
    assert_eq!(
        low.vault_locator, mid.vault_locator,
        "same domain and same bytes must give one locator"
    );
    assert_eq!(high.vault_locator, mid.vault_locator);
    assert!(low.permission_lineage_id < mid.permission_lineage_id);
    assert!(mid.permission_lineage_id < high.permission_lineage_id);
    Ok([low, mid, high])
}

fn fixture_first_digest(fixture: &EncryptedFixture) -> TestResult<academic_domain::ContentDigest> {
    let descriptors = fixture.descriptors()?;
    let first = descriptors
        .iter()
        .find(|descriptor| descriptor.byte_length == FIRST_ARTIFACT_LENGTH)
        .ok_or("the fixture corpus holds no first artifact")?;
    Ok(first.content_digest)
}

const FIRST_ARTIFACT_LENGTH: u64 = encrypted_support::FIRST_ARTIFACT_BYTES.len() as u64;

/// Deletes one of three artifacts that hold the same bytes and restores both the
/// backup taken before the deletion and the one taken after.
///
/// The product deletion writes its tombstone into every backup that still holds
/// a copy, so both restores apply it. Only the artifact the deletion named may
/// arrive destroyed; the other two must arrive readable, and the receipt has to
/// say which is which. `deleted` picks the lineage, so the two callers below
/// cannot both be favourable on any directory walk order.
fn restore_after_deleting_one_of_three(label: &str, deleted: usize) -> TestResult {
    let mut fixture = EncryptedFixture::new(label)?;
    let trio = same_bytes_in_three_lineages(&mut fixture)?;
    let before_deletion = take_backup(&fixture, fixture.master(), "backup-before-deletion")?;

    // The product deletion: the record comes from the store's own chain.
    let stone = {
        let store = fixture.open_store()?;
        deletion_tombstone(
            &store,
            encrypted_support::hex_lower(&[0xc1_u8; 16]),
            &trio[deleted],
            1_700_000_000_060,
        )?
    };
    {
        let vault = fixture.open_vault()?;
        let mut journal =
            AppendOnlyJournal::open(&fixture.profile_root().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        shred_with_tombstone(&mut journal, &vault, &trio[deleted], &stone)?;
    }
    tombstone::write_into_backup(&before_deletion, &stone)?;

    // The live tree is right on its own: a live shred is one positioned write
    // at one path, and it is the copies inside a backup that need the record.
    {
        let vault = fixture.open_vault()?;
        let kek = domain_kek_of(fixture.master())?;
        for (index, descriptor) in trio.iter().enumerate() {
            let expected = if index == deleted {
                HeaderProbe::Shredded
            } else {
                HeaderProbe::Opened
            };
            assert_eq!(
                probe_header(&vault.layout().object_path(descriptor)?, &kek),
                expected,
                "live tree, index {index}"
            );
        }
    }

    let after_deletion = take_backup(&fixture, fixture.master(), "backup-after-deletion")?;
    tombstone::write_into_backup(&after_deletion, &stone)?;

    for (backup, restored) in [
        (&before_deletion, "restored-before-deletion"),
        (&after_deletion, "restored-after-deletion"),
    ] {
        let destination = fixture.work_path(restored);
        let (backup_root, _) = open_backup_with_secret(
            backup,
            BackupRecipientKind::RecoveryPhrase,
            &recovery_secret(),
        )?;
        let verified = verify_encrypted_backup_directory(backup, &backup_root)?;
        let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;
        let receipt = restore_encrypted_profile(
            backup,
            &destination,
            &NativePathProbe::default(),
            &backup_root,
            &recovered,
            &EncryptedRestorePlan {
                authorizations: &fixture.authorizations(),
            },
        )?;

        let keys = ProfileKeys::derive(&recovered.master, recovered.profile_id, &[domain_id()?])?;
        let vault =
            academic_vault::EncryptedVault::open(&destination, keys.keyring(&recovered.master)?)?;
        let kek = domain_kek_of(&recovered.master)?;
        for (index, descriptor) in trio.iter().enumerate() {
            let probe = probe_header(&vault.layout().object_path(descriptor)?, &kek);
            if index == deleted {
                assert_eq!(
                    probe,
                    HeaderProbe::Shredded,
                    "{restored}: the restore published the deleted artifact readable (index {index})"
                );
            } else {
                assert_eq!(
                    probe,
                    HeaderProbe::Opened,
                    "{restored}: the restore destroyed an artifact no tombstone names (index {index})"
                );
            }
        }

        assert_eq!(
            receipt.re_deleted_locators,
            vec![stone.locator.clone()],
            "{restored}: the receipt does not name the re-deletion"
        );
        assert!(
            receipt.absent_locators.is_empty(),
            "{restored}: a tombstone that reached its artifact was reported absent"
        );
        let spared: Vec<String> = receipt
            .spared_objects
            .iter()
            .map(|object| object.artifact_id.clone())
            .collect();
        let mut expected: Vec<String> = trio
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != deleted)
            .map(|(_, descriptor)| encrypted_support::hex_lower(descriptor.id.as_bytes()))
            .collect();
        expected.sort();
        assert_eq!(
            spared, expected,
            "{restored}: the copies the deletion left readable are not on the receipt"
        );
    }
    Ok(())
}

/// The deleted artifact's lineage sorts after the two that keep their bytes.
#[test]
fn a_restore_re_deletes_only_the_named_artifact_when_its_lineage_sorts_last() -> TestResult {
    restore_after_deleting_one_of_three("cross-lineage-last", 2)
}

/// And before them, so neither variant depends on the directory walk order.
#[test]
fn a_restore_re_deletes_only_the_named_artifact_when_its_lineage_sorts_first() -> TestResult {
    restore_after_deleting_one_of_three("cross-lineage-first", 0)
}

// ---------------------------------------------------------------------------
// T111 P2-7
// ---------------------------------------------------------------------------

/// A restore into the backup's own directory is refused.
///
/// `T111`'s reproduction: the destination looked like a perfectly good new
/// empty directory, the restore succeeded, and the backup was then permanently
/// unverifiable against its own manifest.
#[test]
fn a_restore_refuses_a_destination_inside_the_backup() -> TestResult {
    let fixture = EncryptedFixture::new("restore-containment")?;
    let backup = take_backup(&fixture, fixture.master(), "containment-backup")?;
    let (opened_root, _) = open_backup_with_secret(
        &backup,
        BackupRecipientKind::RecoveryPhrase,
        &recovery_secret(),
    )?;
    let verified = verify_encrypted_backup_directory(&backup, &opened_root)?;
    let recovered = recover_profile_keys(&verified, &recovery_secret(), 5_000)?;

    let inside = backup.join("restored-inside-backup");
    let refused = restore_encrypted_profile(
        &backup,
        &inside,
        &NativePathProbe::default(),
        &opened_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    );
    assert!(
        matches!(refused, Err(PortabilityError::UnsafeEntry(_))),
        "a restore into the backup was accepted: {refused:?}"
    );
    assert!(
        !inside.exists(),
        "the refused restore left a directory behind"
    );

    // The backup still verifies, which is the fact the refusal protects.
    verify_encrypted_backup_directory(&backup, &opened_root)?;

    // And a destination outside it is still accepted, so the check is about
    // containment and not about refusing every destination.
    let outside = fixture.work_path("containment-restored");
    restore_encrypted_profile(
        &backup,
        &outside,
        &NativePathProbe::default(),
        &opened_root,
        &recovered,
        &EncryptedRestorePlan {
            authorizations: &fixture.authorizations(),
        },
    )?;
    assert!(outside.join(academic_store::STORE_DATABASE_FILE).is_file());
    Ok(())
}

/// Derives the one domain KEK the fixture corpus uses.
fn domain_kek_of(master: &VaultMasterKey) -> TestResult<academic_crypto::DomainKek> {
    let domain = academic_crypto::DomainId::from_bytes(*domain_id()?.as_bytes());
    Ok(master.derive_domain_kek(encrypted_support::PROFILE_ID, domain)?)
}

/// The tombstone row is here, calls the product restore, and is nowhere else.
///
/// `T111` found the failure this guards against: the acceptance row existed,
/// was named exactly right, and never entered the restore it claimed to
/// exercise. Two facts keep that from coming back — this file really does call
/// `restore_encrypted_profile`, and no second definition of the row survives in
/// `academic-retention`, where it could only imitate one.
#[test]
fn the_tombstone_row_calls_the_product_restore_and_lives_only_here() -> TestResult {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("encrypted_rotation.rs");
    let source = fs::read_to_string(&here)?;
    let row = "fn backup_tombstone_is_present_and_re_deletes_on_restore";
    assert!(source.contains(row), "the acceptance row left this file");
    assert!(
        source.contains("restore_encrypted_profile(\n        &backup,"),
        "the acceptance row no longer calls the product restore"
    );

    let retention = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("retention")
        .join("tests")
        .join("rotation.rs");
    let other = fs::read_to_string(&retention)?;
    assert!(
        !other.contains(row),
        "a second definition of the acceptance row is back in academic-retention, \
         where it cannot reach a restore"
    );
    Ok(())
}
