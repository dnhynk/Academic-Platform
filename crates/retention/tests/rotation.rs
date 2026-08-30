//! The `P2-K5` acceptance rows that need real `AEAD_CHUNKED_V2` objects.
//!
//! `interrupted_rewrap_has_exactly_one_opening_key` is not here: it needs real
//! process kills and lives in `rotation_faults.rs` beside the rest of the
//! `KY`/`RB` matrix.

#![cfg(feature = "rotation-engine")]

mod rotation_support;

use std::{error::Error, fs};

use academic_retention::{
    AppendOnlyJournal, BackupTombstone, JournalEntry, RotationId, RotationPlan, RotationState,
    RotationUnit, UnitProgress,
    engine::{
        HeaderProbe, OpeningObservation, RotationKeys, apply_tombstones, observe_reachable_opening,
        probe_header, shred_with_tombstone,
    },
    journal::ROTATION_JOURNAL_RELATIVE_PATH,
};
#[cfg(feature = "rotation-orchestration")]
use academic_retention::{
    UnitKind,
    engine::{RotationEngine, rebind_locator},
};
use academic_vault::object::{HEADER_BYTES, KEY_SLOT_OFFSET, KEY_SLOT_SHRED_MARKER};
#[cfg(feature = "rotation-orchestration")]
use rotation_support::CHUNK_SIZE;
use rotation_support::{
    SOURCE_ENTROPY, SOURCE_RECIPIENT, TARGET_ENTROPY, TARGET_RECIPIENT, TestRoot,
    create_generation, domain_kek, generation_of, open_vault, profile_id, seal_corpus,
    seal_in_lineage,
};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// rotation_journal_enumerates_remaining_objects
// ---------------------------------------------------------------------------

/// `KY03`. After a rotation stops part way, the journal alone names exactly the
/// objects that have not moved — by locator, not by count.
#[cfg(feature = "rotation-orchestration")]
#[test]
fn rotation_journal_enumerates_remaining_objects() -> TestResult {
    let root = TestRoot::new("remaining")?;
    let (source_master, _source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _target_record) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source_vault = open_vault(root.path(), &source_master)?;
    let target_vault = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source_vault, 3)?;

    let units: Vec<RotationUnit> = descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x11; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        units.clone(),
    )?;

    let journal_path = root.path().join(ROTATION_JOURNAL_RELATIVE_PATH);
    let mut journal = AppendOnlyJournal::open(&journal_path)?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;

    // Before anything moves, every unit is remaining.
    let opened = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    assert_eq!(opened.remaining().len(), 3);

    // Move exactly the first object, then stop.
    engine.rotate_object(&mut journal, &units[0], &descriptors[0])?;
    drop(journal);

    // A fresh process reads the journal and nothing else.
    let resumed = AppendOnlyJournal::open(&journal_path)?;
    let state = RotationState::replay(resumed.entries())?.ok_or("no rotation replayed")?;
    let remaining: Vec<String> = state
        .remaining()
        .iter()
        .filter_map(|unit| unit.unit.source_locator().map(hex::encode))
        .collect();
    let expected: Vec<String> = descriptors[1..]
        .iter()
        .map(|descriptor| hex::encode(descriptor.vault_locator.as_bytes()))
        .collect();
    assert_eq!(
        remaining, expected,
        "the journal did not enumerate the exact objects still to move"
    );
    let migrated: Vec<String> = state
        .migrated()
        .iter()
        .filter_map(|unit| unit.unit.source_locator().map(hex::encode))
        .collect();
    assert_eq!(
        migrated,
        vec![hex::encode(descriptors[0].vault_locator.as_bytes())]
    );

    // A `RotationCompleted` record cannot be written while anything remains.
    let mut journal = AppendOnlyJournal::open(&journal_path)?;
    assert!(
        engine.complete(&mut journal).is_err(),
        "a rotation completed while two objects had not moved"
    );

    // Finishing the remaining two makes the rotation completable, and then the
    // remaining list is empty.
    for (unit, descriptor) in units[1..].iter().zip(&descriptors[1..]) {
        engine.rotate_object(&mut journal, unit, descriptor)?;
    }
    engine.complete(&mut journal)?;
    let done = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    assert!(done.remaining().is_empty());
    assert!(done.is_complete());

    // A resume handed the wrong pair of keys is refused rather than replayed
    // into a wrong answer.
    let (other, _) = create_generation(TARGET_RECIPIENT, [0xEE; 32])?;
    assert!(
        done.require_generations(generation_of(&other)?, generation_of(&target_master)?)
            .is_err()
    );
    done.require_generations(
        generation_of(&source_master)?,
        generation_of(&target_master)?,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// crypto_shred_makes_ciphertext_unreadable
// ---------------------------------------------------------------------------

/// `RB01`. Destroying an object's key slot makes its ciphertext unreadable to
/// every key, and does not claim the file was deleted.
#[test]
fn crypto_shred_makes_ciphertext_unreadable() -> TestResult {
    let root = TestRoot::new("shred")?;
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let vault = open_vault(root.path(), &source_master)?;
    let descriptors = seal_corpus(&vault, 2)?;
    let subject = &descriptors[0];
    let bystander = &descriptors[1];

    let source_kek = domain_kek(&source_master)?;
    let target_kek = domain_kek(&target_master)?;
    let path = vault.layout().object_path(subject)?;
    let before_len = fs::metadata(&path)?.len();
    let before_bytes = fs::read(&path)?;
    assert_eq!(probe_header(&path, &source_kek), HeaderProbe::Opened);
    // Reading the plaintext really does work before the shred, so the refusal
    // afterwards is a change of state rather than a reader that never worked.
    vault.open_reader(subject)?;

    let mut journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let stone = BackupTombstone::new(
        hex::encode([0x09_u8; 16]),
        subject.id,
        *subject.vault_locator.as_bytes(),
        1_700_000_000_000,
    );
    shred_with_tombstone(&mut journal, &vault, subject, &stone)?;

    // 1. The ciphertext is unreadable, and not to one key: to every key. The
    //    domain KEK it was sealed under, a rotated generation's KEK, and the
    //    keyed reader all refuse.
    assert_eq!(probe_header(&path, &source_kek), HeaderProbe::Shredded);
    assert_eq!(probe_header(&path, &target_kek), HeaderProbe::Shredded);
    let refusal = vault
        .open_reader(subject)
        .err()
        .ok_or("a shredded object still opened")?
        .to_string();
    assert!(
        refusal.contains("crypto-shredded") && refusal.contains("key slot was destroyed"),
        "the refusal did not name the shred: {refusal}"
    );
    assert!(
        refusal.contains("was not deleted"),
        "the refusal claimed the file was deleted: {refusal}"
    );

    // 2. The file was not deleted and did not change length. A crypto-shred is
    //    a claim about keys, not about bytes on disk.
    assert!(path.is_file(), "the shred deleted the object file");
    assert_eq!(fs::metadata(&path)?.len(), before_len);

    // 3. Exactly the key slot changed, and nothing else.
    let after_bytes = fs::read(&path)?;
    assert_eq!(
        after_bytes[..KEY_SLOT_OFFSET],
        before_bytes[..KEY_SLOT_OFFSET]
    );
    assert_eq!(after_bytes[HEADER_BYTES..], before_bytes[HEADER_BYTES..]);
    assert_ne!(
        after_bytes[KEY_SLOT_OFFSET..HEADER_BYTES],
        before_bytes[KEY_SLOT_OFFSET..HEADER_BYTES]
    );
    assert_eq!(
        &after_bytes[KEY_SLOT_OFFSET..KEY_SLOT_OFFSET + KEY_SLOT_SHRED_MARKER.len()],
        KEY_SLOT_SHRED_MARKER.as_slice()
    );
    // The destroyed slot names the tombstone that authorized it.
    assert_eq!(
        &after_bytes[KEY_SLOT_OFFSET + KEY_SLOT_SHRED_MARKER.len()
            ..KEY_SLOT_OFFSET + KEY_SLOT_SHRED_MARKER.len() + 32],
        stone.digest().as_slice()
    );

    // 4. The wrapped DEK is gone from the file: none of its bytes survive.
    let old_slot = &before_bytes[KEY_SLOT_OFFSET..HEADER_BYTES];
    assert!(
        !after_bytes
            .windows(old_slot.len())
            .any(|window| window == old_slot),
        "the destroyed wrapped DEK is still present somewhere in the file"
    );

    // 5. The shred reached exactly one object. The bystander still opens.
    let bystander_path = vault.layout().object_path(bystander)?;
    assert_eq!(
        probe_header(&bystander_path, &source_kek),
        HeaderProbe::Opened
    );
    vault.open_reader(bystander)?;

    // 6. Re-applying is idempotent, which is what makes a resumed retention
    //    action safe.
    let receipt = vault.shred_key_slot(subject, &stone.digest())?;
    assert!(receipt.was_already_shredded());
    assert_eq!(fs::read(&path)?, after_bytes);

    // 7. The journal records the shred and the tombstone that authorized it.
    let recorded = journal
        .entries()
        .find_map(|entry| match entry {
            JournalEntry::ArtifactShredded {
                locator,
                tombstone_digest,
                ..
            } => Some((locator.clone(), tombstone_digest.clone())),
            _ => None,
        })
        .ok_or("the journal recorded no shred")?;
    assert_eq!(
        recorded,
        (
            hex::encode(subject.vault_locator.as_bytes()),
            stone.digest_hex()
        )
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The tombstone half a restore needs, without a restore
// ---------------------------------------------------------------------------

/// A shred writes the tombstone that authorizes it, and re-applying that
/// tombstone destroys the key slot of the object it names and of nothing else.
///
/// This is the half of a deletion that lives in this crate: the record, the
/// keyless positioned write, and the report of what was and was not reached.
/// The other half — that a real backup carries the tombstone and that the
/// product restore applies it — is
/// `backup_tombstone_is_present_and_re_deletes_on_restore`, and it lives in
/// `academic-portability`'s encrypted suite because only that crate can call
/// `backup_encrypted_profile` and `restore_encrypted_profile`. It used to live
/// here and imitate both with `fs::copy`, which is why it passed while the
/// product restore read no tombstone at all.
#[test]
fn a_tombstone_re_deletes_the_object_it_names_and_no_other() -> TestResult {
    let live = TestRoot::new("tombstone-live")?;
    let materialised = TestRoot::new("tombstone-materialised")?;

    let (master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let vault = open_vault(live.path(), &master)?;
    let descriptors = seal_corpus(&vault, 2)?;
    let subject = &descriptors[0];
    let bystander = &descriptors[1];
    let kek = domain_kek(&master)?;

    let mut journal = AppendOnlyJournal::open(&live.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let stone = BackupTombstone::new(
        hex::encode([0x21_u8; 16]),
        subject.id,
        *subject.vault_locator.as_bytes(),
        1_700_000_000_001,
    );
    shred_with_tombstone(&mut journal, &vault, subject, &stone)?;

    // The live object is gone and the record that authorized it is on disk.
    assert_eq!(
        probe_header(&vault.layout().object_path(subject)?, &kek),
        HeaderProbe::Shredded
    );

    // A tree of objects that were copied out before the shred: the state any
    // already-taken backup is in.
    let objects_root = materialised.path().join("vault/v2/objects");
    fs::create_dir_all(&objects_root)?;
    let intact = open_vault(live.path(), &master)?;
    for descriptor in &descriptors {
        let name = format!("{}.aobj", descriptor.id);
        let source = intact.layout().object_path(descriptor)?;
        if descriptor.id == subject.id {
            // The shredded live object cannot stand in for the copy a backup
            // took before the deletion, so the subject is re-sealed into a
            // second vault and taken from there.
            let untouched = TestRoot::new("tombstone-untouched")?;
            let second = open_vault(untouched.path(), &master)?;
            let resealed = seal_corpus(&second, 2)?;
            let from = second.layout().object_path(&resealed[0])?;
            fs::copy(&from, objects_root.join(&name))?;
        } else {
            fs::copy(&source, objects_root.join(&name))?;
        }
    }
    let materialised_subject = objects_root.join(format!("{}.aobj", subject.id));
    let materialised_bystander = objects_root.join(format!("{}.aobj", bystander.id));
    assert_eq!(
        probe_header(&materialised_subject, &kek),
        HeaderProbe::Opened
    );

    let applied = apply_tombstones(
        &materialised.path().join("vault/v2"),
        std::slice::from_ref(&stone),
    )?;
    assert_eq!(applied.applied, vec![stone.locator.clone()]);
    assert!(applied.absent.is_empty());
    assert_eq!(
        probe_header(&materialised_subject, &kek),
        HeaderProbe::Shredded,
        "the tombstone did not re-delete the object it names"
    );
    assert_eq!(
        probe_header(&materialised_bystander, &kek),
        HeaderProbe::Opened,
        "the tombstone re-deleted an object it does not name"
    );
    assert!(materialised_subject.is_file());

    // Applying again is idempotent, so a retried restore is safe.
    let again = apply_tombstones(
        &materialised.path().join("vault/v2"),
        std::slice::from_ref(&stone),
    )?;
    assert_eq!(again.applied, vec![stone.locator.clone()]);

    // A tombstone whose object is not in the tree is reported, not ignored.
    let orphan = BackupTombstone::new(hex::encode([0x22_u8; 16]), descriptors[1].id, [0xFE; 32], 1);
    let mixed = apply_tombstones(
        &materialised.path().join("vault/v2"),
        &[stone.clone(), orphan.clone()],
    )?;
    assert_eq!(mixed.absent, vec![orphan.locator]);
    Ok(())
}

// ---------------------------------------------------------------------------
// A locator is not an identity
// ---------------------------------------------------------------------------

/// Registers the same bytes in three permission lineages of one domain, deletes
/// the artifact at `deleted`, and applies the record to a tree that is a byte
/// copy of the live one — which is the state every already-taken backup is in.
///
/// The three share one locator and sit at three paths, so a re-deletion that
/// matched on the locator alone would reach whichever the directory walk saw
/// first: it would destroy a key slot the profile never deleted and leave the
/// deleted artifact readable, and the receipt would report the ordinary
/// success. `deleted` picks the lineage, so the two callers below cannot both
/// be favourable on any walk order.
fn tombstone_over_three_lineages(label: &str, deleted: usize) -> TestResult {
    let live = TestRoot::new(label)?;
    let materialised = TestRoot::new(&format!("{label}-copy"))?;
    let (master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let vault = open_vault(live.path(), &master)?;
    let kek = domain_kek(&master)?;

    let bytes = rotation_support::deterministic_bytes(777, 0x77);
    let trio = [
        seal_in_lineage(&vault, 0, &bytes)?,
        seal_in_lineage(&vault, 1, &bytes)?,
        seal_in_lineage(&vault, 2, &bytes)?,
    ];
    assert_eq!(trio[0].vault_locator, trio[1].vault_locator);
    assert_eq!(trio[1].vault_locator, trio[2].vault_locator);
    let paths = [
        vault.layout().object_path(&trio[0])?,
        vault.layout().object_path(&trio[1])?,
        vault.layout().object_path(&trio[2])?,
    ];
    assert!(paths[0] != paths[1] && paths[1] != paths[2] && paths[0] != paths[2]);

    // The tree an already-taken backup holds: every object still openable.
    let copied = materialised.path().join("vault/v2/objects");
    for (index, path) in paths.iter().enumerate() {
        let destination = copied.join(format!("{index}.aobj"));
        fs::create_dir_all(copied.as_path())?;
        fs::copy(path, &destination)?;
        assert_eq!(probe_header(&destination, &kek), HeaderProbe::Opened);
    }

    // The product deletion of exactly one of them.
    let mut journal = AppendOnlyJournal::open(&live.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let stone = BackupTombstone::new(
        hex::encode([0x51_u8; 16]),
        trio[deleted].id,
        *trio[deleted].vault_locator.as_bytes(),
        1_700_000_000_051,
    );
    shred_with_tombstone(&mut journal, &vault, &trio[deleted], &stone)?;
    for (index, path) in paths.iter().enumerate() {
        let expected = if index == deleted {
            HeaderProbe::Shredded
        } else {
            HeaderProbe::Opened
        };
        assert_eq!(
            probe_header(path, &kek),
            expected,
            "live tree, index {index}"
        );
    }

    let applied = apply_tombstones(
        &materialised.path().join("vault/v2"),
        std::slice::from_ref(&stone),
    )?;
    for index in 0..trio.len() {
        let probe = probe_header(&copied.join(format!("{index}.aobj")), &kek);
        if index == deleted {
            assert_eq!(
                probe,
                HeaderProbe::Shredded,
                "the deleted artifact was left readable in the copy (index {index})"
            );
        } else {
            assert_eq!(
                probe,
                HeaderProbe::Opened,
                "an artifact the tombstone does not name was destroyed (index {index})"
            );
        }
    }

    // The report names what it destroyed and what it deliberately did not.
    assert_eq!(applied.applied, vec![stone.locator.clone()]);
    assert!(applied.absent.is_empty());
    let spared: Vec<String> = applied
        .spared
        .iter()
        .map(|object| object.artifact_id.clone())
        .collect();
    let mut expected: Vec<String> = trio
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != deleted)
        .map(|(_, descriptor)| hex::encode(descriptor.id.as_bytes()))
        .collect();
    expected.sort();
    assert_eq!(
        spared, expected,
        "the copies left readable under another lineage are not on the report"
    );
    for object in &applied.spared {
        assert_eq!(object.locator, stone.locator);
    }
    Ok(())
}

/// The deleted artifact's lineage sorts after the two that keep their bytes.
#[test]
fn a_tombstone_reaches_its_own_artifact_when_the_deleted_lineage_sorts_last() -> TestResult {
    tombstone_over_three_lineages("cross-lineage-last", 2)
}

/// And before them, so neither variant depends on the directory walk order.
#[test]
fn a_tombstone_reaches_its_own_artifact_when_the_deleted_lineage_sorts_first() -> TestResult {
    tombstone_over_three_lineages("cross-lineage-first", 0)
}

// ---------------------------------------------------------------------------
// The locator moves when the key moves
// ---------------------------------------------------------------------------

/// A rotation lands the object on a new path, so the two generations are two
/// files rather than one file rewritten in place.
#[cfg(feature = "rotation-orchestration")]
#[test]
fn a_rotated_object_lands_on_a_new_locator() -> TestResult {
    let root = TestRoot::new("relocate")?;
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source_vault = open_vault(root.path(), &source_master)?;
    let target_vault = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source_vault, 1)?;
    let subject = &descriptors[0];

    let rebound = rebind_locator(subject, &domain_kek(&target_master)?, profile_id())?;
    assert_ne!(
        rebound.vault_locator, subject.vault_locator,
        "a rotation left the object on the same locator"
    );
    assert_eq!(rebound.content_digest, subject.content_digest);
    assert_eq!(rebound.id, subject.id);

    let units = vec![RotationUnit::object(*subject.vault_locator.as_bytes())];
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x33; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        units.clone(),
    )?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;
    let resealed = engine.rotate_object(&mut journal, &units[0], subject)?;
    assert_eq!(resealed.vault_locator, rebound.vault_locator);

    // Both files exist. The source object is now unreferenced, which is what
    // the vault's reconciliation quarantines; the rotation never removed it,
    // so a kill could not have left an artifact neither key opens.
    assert!(source_vault.layout().object_path(subject)?.is_file());
    assert!(target_vault.layout().object_path(&resealed)?.is_file());

    // The re-sealed object really carries the same plaintext.
    use std::io::Read as _;
    let mut original = Vec::new();
    let mut rotated = Vec::new();
    source_vault
        .open_reader(subject)?
        .read_to_end(&mut original)?;
    target_vault
        .open_reader(&resealed)?
        .read_to_end(&mut rotated)?;
    assert_eq!(original, rotated);
    assert_eq!(original.len(), 1024);
    assert_eq!(source_vault.chunk_size(), CHUNK_SIZE);

    // And a "rotation" that does not change the key is refused before anything
    // is written, because both keys would open every object.
    let refusal = RotationPlan::new(
        RotationId::from_bytes([0x34; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&source_master)?,
        units,
    )
    .err()
    .ok_or("a rotation to the same generation was accepted")?
    .to_string();
    assert!(
        refusal.contains("same generation") && refusal.contains("both keys would open"),
        "the refusal did not name the invariant it protects: {refusal}"
    );

    let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    assert_eq!(state.units()[0].progress, UnitProgress::Migrated);
    Ok(())
}

// ---------------------------------------------------------------------------
// The invariant checker can report a violation
// ---------------------------------------------------------------------------

/// Both halves of "exactly one" are reachable, so the invariant is a real check
/// rather than a shape the checker can only ever report as satisfied.
///
/// A suite that can only ever observe `OnlySource` or `OnlyTarget` proves
/// nothing about "both open" and "neither opens". This one constructs each
/// violation on purpose and then shows what stands between the engine and it.
#[test]
fn the_invariant_checker_reports_both_and_neither_when_they_happen() -> TestResult {
    let root = TestRoot::new("violations")?;
    let (master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (other, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let vault = open_vault(root.path(), &master)?;
    let descriptors = seal_corpus(&vault, 1)?;
    let subject = &descriptors[0];

    // "Both open": the two generations are the same key. Every object is opened
    // by both, which is exactly the state a rotation that did not change the key
    // would leave behind.
    let same = domain_kek(&master)?;
    let also_same = domain_kek(&master)?;
    let both = RotationKeys {
        profile: profile_id(),
        source_kek: &same,
        target_kek: &also_same,
    };
    assert_eq!(
        observe_reachable_opening(&vault, &both, subject, UnitProgress::Planned)?,
        OpeningObservation::Both,
        "the checker cannot report the both-open violation"
    );

    // And that state is unreachable through the engine, because planning it is
    // refused before anything is written.
    let refusal = RotationPlan::new(
        RotationId::from_bytes([0x55; 16]),
        profile_id(),
        generation_of(&master)?,
        generation_of(&master)?,
        vec![RotationUnit::object(*subject.vault_locator.as_bytes())],
    )
    .err()
    .ok_or("a same-generation rotation was planned")?
    .to_string();
    assert!(refusal.contains("both keys would open every object"));

    // "Neither opens": reachability points at an object that is not there. That
    // is what appending `UnitMigrated` before the target object was written and
    // verified would produce.
    let target_kek = domain_kek(&other)?;
    let neither = RotationKeys {
        profile: profile_id(),
        source_kek: &same,
        target_kek: &target_kek,
    };
    assert_eq!(
        observe_reachable_opening(&vault, &neither, subject, UnitProgress::Migrated)?,
        OpeningObservation::Neither,
        "the checker cannot report the neither-opens violation"
    );
    // The same unit before migration is the correct, satisfied case, so the two
    // outcomes differ by reachability alone.
    assert_eq!(
        observe_reachable_opening(&vault, &neither, subject, UnitProgress::Planned)?,
        OpeningObservation::OnlySource
    );

    // And the journal cannot record that state either: a replay that migrates a
    // unit with no preceding reseal is refused rather than replayed.
    let units = vec![RotationUnit::object(*subject.vault_locator.as_bytes())];
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x56; 16]),
        profile_id(),
        generation_of(&master)?,
        generation_of(&other)?,
        units.clone(),
    )?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    journal.append(plan.started_entry())?;
    journal.append(JournalEntry::UnitMigrated {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: units[0].unit_id_hex(),
    })?;
    let replay = RotationState::replay(journal.entries())
        .err()
        .ok_or("a migration with no preceding reseal was replayed")?
        .to_string();
    assert!(
        replay.contains("without a preceding UnitResealed"),
        "the refusal did not name the ordering rule: {replay}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// A settled deletion really shreds
// ---------------------------------------------------------------------------

/// The vocabulary and the mechanism compose: a plan whose transcript class
/// resolves to real objects, executed by a real shredding executor, settles
/// `COMPLETE`, and those objects are afterwards unreadable.
#[test]
fn a_settled_deletion_really_shreds_its_derivatives() -> TestResult {
    use academic_domain::ArtifactDescriptor;
    use academic_retention::{
        ActionId, ClassResolution, DeletionPlan, DerivativeClass, DerivativeResolver,
        ExecutionFailure, PlannedAction, RetentionExecutor, RetentionSubject, UnresolvedReason,
        execute::settle,
    };
    use academic_vault::EncryptedVault;

    struct Resolver {
        derivatives: Vec<[u8; 32]>,
    }

    impl DerivativeResolver for Resolver {
        fn resolve(&self, class: DerivativeClass, _subject: &RetentionSubject) -> ClassResolution {
            if class == DerivativeClass::Transcript {
                ClassResolution::Locators(self.derivatives.clone())
            } else {
                ClassResolution::NothingToDelete {
                    reason: format!("this corpus holds no {}", class.as_str()),
                }
            }
        }
    }

    struct ShreddingExecutor<'a> {
        vault: &'a EncryptedVault,
        descriptors: &'a [ArtifactDescriptor],
        digest: [u8; 32],
    }

    impl RetentionExecutor for ShreddingExecutor<'_> {
        fn execute(&mut self, action: &PlannedAction) -> Result<(), ExecutionFailure> {
            let Some(descriptor) = self
                .descriptors
                .iter()
                .find(|candidate| candidate.vault_locator.as_bytes() == &action.locator)
            else {
                return Err(ExecutionFailure {
                    reason: UnresolvedReason::ShredFailed,
                    detail: "no descriptor names this locator".to_owned(),
                });
            };
            self.vault
                .shred_key_slot(descriptor, &self.digest)
                .map(|_| ())
                .map_err(|error| ExecutionFailure {
                    reason: UnresolvedReason::ShredFailed,
                    detail: error.to_string(),
                })
        }
    }

    let root = TestRoot::new("settled-shred")?;
    let (master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let vault = open_vault(root.path(), &master)?;
    let descriptors = seal_corpus(&vault, 3)?;
    let kek = domain_kek(&master)?;

    let derivatives = vec![
        *descriptors[1].vault_locator.as_bytes(),
        *descriptors[2].vault_locator.as_bytes(),
    ];
    let plan = DeletionPlan::build(
        RetentionSubject::whole_object(*descriptors[0].vault_locator.as_bytes()),
        &Resolver { derivatives },
    );
    let stone = BackupTombstone::new(
        hex::encode([0x61_u8; 16]),
        descriptors[0].id,
        *descriptors[0].vault_locator.as_bytes(),
        4,
    );
    let mut journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let mut executor = ShreddingExecutor {
        vault: &vault,
        descriptors: &descriptors,
        digest: stone.digest(),
    };
    let outcome = settle(
        &mut journal,
        ActionId::from_bytes([0x62; 16]),
        &plan,
        &mut executor,
    )?;
    assert_eq!(outcome.as_str(), "COMPLETE");
    assert!(outcome.unresolved().is_empty());

    for descriptor in &descriptors[1..] {
        let path = vault.layout().object_path(descriptor)?;
        assert_eq!(probe_header(&path, &kek), HeaderProbe::Shredded);
    }
    // The subject itself belonged to no derivative class, so the plan did not
    // touch it: a deletion reaches what it enumerated and nothing else.
    assert_eq!(
        probe_header(&vault.layout().object_path(&descriptors[0])?, &kek),
        HeaderProbe::Opened
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The store database unit
// ---------------------------------------------------------------------------

/// The store database is planned, journalled, and invariant-checked here; its
/// executor is bound outside this crate, in the one lane that links the
/// encrypted store. A rotation that never ran it stops and says so, rather than
/// recording a migration that did not happen.
///
/// `rotation_seam.rs::a_rotation_completes_once_its_store_database_unit_has_run`
/// is the other half: the same plan, with the unit run.
#[cfg(feature = "rotation-orchestration")]
#[test]
fn a_rotation_will_not_complete_over_a_store_database_it_never_rekeyed() -> TestResult {
    let root = TestRoot::new("store-unit")?;
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source_vault = open_vault(root.path(), &source_master)?;
    let target_vault = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source_vault, 1)?;

    let object = RotationUnit::object(*descriptors[0].vault_locator.as_bytes());
    let database = RotationUnit::store_database(profile_id());
    assert_eq!(database.kind(), UnitKind::StoreDatabase);
    assert!(
        database.source_locator().is_none(),
        "the store database is not an object and has no locator"
    );
    assert_ne!(object.unit_id(), database.unit_id());

    let plan = RotationPlan::new(
        RotationId::from_bytes([0x66; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        vec![object.clone(), database.clone()],
    )?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;
    engine.rotate_object(&mut journal, &object, &descriptors[0])?;

    // Every object has moved and the rotation still refuses to complete, and it
    // refuses by naming the database rather than by reporting a missing
    // descriptor: "its executor never ran" and "you forgot a descriptor" are
    // different facts.
    let refusal = engine
        .complete(&mut journal)
        .err()
        .ok_or("a rotation completed over a database it never rekeyed")?
        .to_string();
    assert!(
        refusal.contains("store database") && refusal.contains("never ran its executor"),
        "the refusal did not name the unrun executor: {refusal}"
    );

    // The journal still enumerates it as remaining, so the operator is told
    // exactly what is left.
    let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    let remaining: Vec<UnitKind> = state
        .remaining()
        .iter()
        .map(|unit| unit.unit.kind())
        .collect();
    assert_eq!(remaining, vec![UnitKind::StoreDatabase]);
    assert!(!state.is_complete());
    Ok(())
}

/// The plan refuses to move the database before an object, and the engine
/// refuses to move a unit the plan does not hold.
///
/// `T116`'s two observations, stated as gates. "The database moves last" was a
/// documented obligation on an orchestrator that does not exist yet, and a plan
/// that ordered it first was accepted and run — after which
/// `record_descriptor_migration` has to open the store for every object still
/// to move and the plan names no key that does. And `rotate_object` and
/// `rotate_store_database` took any unit at all: a record written for one
/// outside the plan makes `RotationState::replay` refuse the whole journal,
/// which is permanent — the records are append-only — and needs no kill.
#[cfg(feature = "rotation-orchestration")]
#[test]
fn a_plan_orders_its_database_unit_last_and_the_engine_moves_nothing_else() -> TestResult {
    let root = TestRoot::new("plan-gates")?;
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source_vault = open_vault(root.path(), &source_master)?;
    let target_vault = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source_vault, 2)?;

    let first = RotationUnit::object(*descriptors[0].vault_locator.as_bytes());
    let second = RotationUnit::object(*descriptors[1].vault_locator.as_bytes());
    let database = RotationUnit::store_database(profile_id());

    let refused = RotationPlan::new(
        RotationId::from_bytes([0x69; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        vec![database.clone(), first.clone()],
    )
    .err()
    .ok_or("a plan that rekeys the database before an object was accepted")?
    .to_string();
    assert!(
        refused.contains("moves it last"),
        "the refusal did not name the ordering rule: {refused}"
    );

    // The same units in the order the contract states are a plan.
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x69; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        vec![first.clone(), database.clone()],
    )?;
    let mut journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;

    // `second` is a real unit of a real object; it is simply not in this plan.
    let outside = engine
        .rotate_object(&mut journal, &second, &descriptors[1])
        .err()
        .ok_or("a unit outside the plan was moved under it")?
        .to_string();
    assert!(
        outside.contains("the rotation plan does not hold unit"),
        "the refusal did not name the plan: {outside}"
    );

    // Nothing was written for it, so the journal still replays.
    let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    assert_eq!(state.units().len(), 2);
    assert_eq!(state.remaining().len(), 2);
    engine.rotate_object(&mut journal, &first, &descriptors[0])?;
    assert!(RotationState::replay(journal.entries())?.is_some());
    Ok(())
}

/// The fault rows this crate answers for, named rather than counted.
#[test]
fn the_named_fault_rows_are_the_ones_t068_assigns() -> TestResult {
    assert_eq!(
        academic_retention::PHASE2_ROTATION_FAULT_IDS,
        &["KY03", "KY04", "KY05"]
    );
    assert_eq!(
        academic_retention::PHASE2_RETENTION_FAULT_IDS,
        &["RB01", "RB02", "RB03", "RB04"]
    );
    // Every failpoint this crate compiles is one of those rows, spelled with the
    // row identifier so a report can be grepped for it. `RB01`'s failpoint is
    // `academic-vault`'s and is deliberately absent here.
    for selector in academic_retention::FAULT_SELECTORS {
        let row = selector.split(':').next().unwrap_or_default();
        assert!(
            academic_retention::PHASE2_ROTATION_FAULT_IDS.contains(&row)
                || academic_retention::PHASE2_RETENTION_FAULT_IDS.contains(&row),
            "failpoint {selector} names {row}, which is not a fault row this crate owns"
        );
    }
    assert!(
        !academic_retention::FAULT_SELECTORS
            .iter()
            .any(|selector| selector.starts_with("RB01")),
        "RB01's failpoint belongs to academic-vault, beside the key slot it destroys"
    );
    Ok(())
}
