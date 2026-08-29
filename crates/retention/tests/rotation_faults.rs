//! The `KY03`-`KY05` and `RB01`-`RB02` rows of the t068 section 7 fault matrix.
//!
//! Required outcomes, verbatim from section 7:
//!
//! | ID | injection point | outcome |
//! |---|---|---|
//! | `KY03` | kill mid domain-KEK rewrap | exactly one of old/new KEK opens every object; journal lists the remainder; resumable |
//! | `KY04` | kill during recipient add | recipient set is old or new, never partial |
//! | `KY05` | kill during recipient revoke | revoked recipient never receives a new key; objects still under the old key are enumerated |
//! | `RB01` | kill during crypto-shred | S or intact; the key slot removal is a single atomic write plus fsync |
//! | `RB02` | backup tombstone write fails | deletion incomplete; explicit repair-required |
//!
//! `KY03` is one row with four distinguishable on-disk states, and every one of
//! them is reached by a real process kill here. Each stage asserts what the
//! child actually got to before it died — a target object that exists but is
//! not journalled is a different state from one that was never written — so a
//! child that aborted early could not pass as a child that aborted late.
//!
//! `RB03` and `RB04` are error-induced rather than kill-induced and live in
//! `retention.rs`, driven through the resolver and executor seams.

#![cfg(all(feature = "rotation-engine", feature = "phase2-fault-injection"))]

mod rotation_support;

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use academic_crypto::VaultMasterKey;
use academic_domain::ArtifactDescriptor;
use academic_retention::{
    AppendOnlyJournal, BackupTombstone, FAULT_READY_MARKER_VARIABLE, FAULT_SELECTION_VARIABLE,
    RotationId, RotationPlan, RotationState, RotationUnit,
    engine::{
        HeaderProbe, OpeningObservation, RotationEngine, RotationKeys, observe_reachable_opening,
        probe_header, rebind_locator, shred_with_tombstone,
    },
    journal::ROTATION_JOURNAL_RELATIVE_PATH,
    recipients, tombstone,
};
use academic_vault::object::{HEADER_BYTES, KEY_SLOT_OFFSET, KEY_SLOT_SHRED_MARKER};
use rotation_support::{
    SOURCE_ENTROPY, SOURCE_RECIPIENT, TARGET_ENTROPY, TARGET_RECIPIENT, TestRoot,
    create_generation, domain_kek, generation_of, load_generations, open_vault,
    persist_generations, profile_id, seal_corpus,
};

type TestResult = Result<(), Box<dyn Error>>;

const CHILD_ENV: &str = "ACADEMIC_RETENTION_TEST_CHILD";
const PROFILE_ENV: &str = "ACADEMIC_RETENTION_TEST_PROFILE";
const UNIT_ENV: &str = "ACADEMIC_RETENTION_TEST_UNIT";
const OPERATION_ENV: &str = "ACADEMIC_RETENTION_TEST_OPERATION";
const CORPUS: usize = 3;
const ROTATION: [u8; 16] = [0x5C; 16];

/// Every `KY03` stage, in the order the engine reaches them.
const KY03_STAGES: [&str; 4] = [
    "KY03:before-reseal",
    "KY03:after-reseal",
    "KY03:after-resealed-record",
    "KY03:after-migrated-record",
];

// ---------------------------------------------------------------------------
// Child process
// ---------------------------------------------------------------------------

/// Re-entry point for the killed child. Never runs in a normal test pass.
#[test]
fn retention_fault_child_entrypoint() -> TestResult {
    if env::var(CHILD_ENV).ok().as_deref() != Some("1") {
        return Ok(());
    }
    let root = env::var_os(PROFILE_ENV)
        .map(PathBuf::from)
        .ok_or("fault child profile path was not supplied")?;
    let fault = env::var(FAULT_SELECTION_VARIABLE)?;

    // The child never receives a key: it reopens both generations from the
    // recipient records the parent wrote, exactly as the product does.
    let (source_master, target_master) = load_generations(&root)?;

    if fault.starts_with("KY04") {
        return run_recipient_child(&root, &source_master);
    }
    if fault.starts_with("RB01") {
        return run_shred_child(&root, &source_master);
    }
    if fault.starts_with("RB02") {
        return run_tombstone_child(&root, &source_master);
    }

    let source_vault = open_vault(&root, &source_master)?;
    let target_vault = open_vault(&root, &target_master)?;
    // Re-sealing the fixed corpus into the source vault adopts the objects the
    // parent already published, so the child recovers the identical descriptors
    // without the parent shipping them across the boundary.
    let descriptors = seal_corpus(&source_vault, CORPUS)?;
    let units = units_for(&descriptors);
    let plan = rotation_plan(&source_master, &target_master, &units)?;

    let mut journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    if RotationState::replay(journal.entries())?.is_none() {
        engine.begin(&mut journal)?;
    }
    let index: usize = env::var(UNIT_ENV)?.parse()?;
    engine.rotate_object(&mut journal, &units[index], &descriptors[index])?;
    Err("the child was expected to abort at its failpoint".into())
}

/// Runs the recipient-set operation the parent asked for.
///
/// `KY04` and `KY05` are two rows with one atomicity boundary: the rename that
/// publishes the replacement set. The failpoint is therefore shared and the
/// operation is chosen here, so neither row can pass by running the other.
fn run_recipient_child(root: &Path, source_master: &VaultMasterKey) -> TestResult {
    let mut journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    match env::var(OPERATION_ENV)?.as_str() {
        "add" => {
            let (_, added) = create_generation([0xD4; 16], [0xD4; 32])?;
            recipients::add_recipient(
                root,
                profile_id(),
                &mut journal,
                added,
                generation_of(source_master)?,
            )?;
        }
        "revoke" => {
            recipients::revoke_recipient(
                root,
                profile_id(),
                &mut journal,
                &SOURCE_RECIPIENT,
                generation_of(source_master)?,
                Vec::new(),
            )?;
        }
        other => return Err(format!("unknown recipient operation {other}").into()),
    }
    Err("the child was expected to abort at its failpoint".into())
}

fn run_shred_child(root: &Path, master: &VaultMasterKey) -> TestResult {
    let vault = open_vault(root, master)?;
    let descriptors = seal_corpus(&vault, CORPUS)?;
    let mut journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let stone = BackupTombstone::new(
        hex::encode([0x41_u8; 16]),
        *descriptors[0].vault_locator.as_bytes(),
        1,
    );
    shred_with_tombstone(&mut journal, &vault, &descriptors[0], &stone)?;
    Err("the child was expected to abort at its failpoint".into())
}

fn run_tombstone_child(root: &Path, master: &VaultMasterKey) -> TestResult {
    let vault = open_vault(root, master)?;
    let descriptors = seal_corpus(&vault, CORPUS)?;
    let stone = BackupTombstone::new(
        hex::encode([0x42_u8; 16]),
        *descriptors[0].vault_locator.as_bytes(),
        2,
    );
    tombstone::write_into_backup(&root.join("backup"), &stone)?;
    Err("the child was expected to abort at its failpoint".into())
}

// ---------------------------------------------------------------------------
// Parent helpers
// ---------------------------------------------------------------------------

fn units_for(descriptors: &[ArtifactDescriptor]) -> Vec<RotationUnit> {
    descriptors
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect()
}

fn rotation_plan(
    source_master: &VaultMasterKey,
    target_master: &VaultMasterKey,
    units: &[RotationUnit],
) -> Result<RotationPlan, Box<dyn Error>> {
    Ok(RotationPlan::new(
        RotationId::from_bytes(ROTATION),
        profile_id(),
        generation_of(source_master)?,
        generation_of(target_master)?,
        units.to_vec(),
    )?)
}

/// `academic-vault`'s failpoint selector. `RB01` lives there, beside the key
/// slot it destroys, so the harness has to arm that crate rather than this one.
const VAULT_FAULT_VARIABLE: &str = "ACADEMIC_VAULT_TEST_FAULT";
/// `academic-vault`'s ready-marker variable.
const VAULT_READY_VARIABLE: &str = "ACADEMIC_VAULT_TEST_READY_MARKER";

/// Runs one child to its failpoint and proves it reached it.
fn kill_at(root: &Path, fault: &str, unit_index: usize) -> TestResult {
    let marker = root.join(format!("ready-{}", fault.replace(':', "-")));
    let mut command = Command::new(env::current_exe()?);
    command
        .arg("retention_fault_child_entrypoint")
        .arg("--exact")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env(PROFILE_ENV, root)
        .env(UNIT_ENV, unit_index.to_string())
        .env(FAULT_SELECTION_VARIABLE, fault)
        .env(FAULT_READY_MARKER_VARIABLE, &marker);
    if fault.starts_with("RB01") {
        command
            .env(VAULT_FAULT_VARIABLE, "RB01")
            .env(VAULT_READY_VARIABLE, &marker);
    }
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    assert!(
        !status.success(),
        "the {fault} child exited cleanly instead of aborting at its failpoint"
    );
    assert!(
        marker.is_file(),
        "the {fault} child never reached its failpoint"
    );
    assert_eq!(fs::read_to_string(&marker)?, fault);
    Ok(())
}

/// Returns the generation that must open one unit at one `KY03` stage.
///
/// Stated per stage rather than read back from the journal's own mapping, so
/// the test does not agree with the implementation by construction: a build
/// that decided a re-sealed unit was already reachable would satisfy
/// `agrees_with` and still fail here.
fn expected_observation(stage: &str) -> OpeningObservation {
    if stage == "KY03:after-migrated-record" {
        OpeningObservation::OnlyTarget
    } else {
        OpeningObservation::OnlySource
    }
}

/// Asserts the rotation invariant over every unit, from the journal alone.
fn assert_exactly_one_opening_key(
    root: &Path,
    source_master: &VaultMasterKey,
    target_master: &VaultMasterKey,
    descriptors: &[ArtifactDescriptor],
) -> TestResult {
    let source_vault = open_vault(root, source_master)?;
    let source_kek = domain_kek(source_master)?;
    let target_kek = domain_kek(target_master)?;
    let keys = RotationKeys {
        profile: profile_id(),
        source_kek: &source_kek,
        target_kek: &target_kek,
    };
    let journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;
    let state = RotationState::replay(journal.entries())?.ok_or("no rotation was replayed")?;
    assert_eq!(state.units().len(), descriptors.len());
    for (unit, descriptor) in state.units().iter().zip(descriptors) {
        let observation =
            observe_reachable_opening(&source_vault, &keys, descriptor, unit.progress)?;
        assert!(
            observation.is_exactly_one(),
            "unit {} is opened by {observation:?}, not by exactly one key",
            unit.unit.unit_id_hex()
        );
        assert!(
            observation.agrees_with(unit.progress.opening_generation()),
            "unit {} opens under {observation:?} but the journal says {:?}",
            unit.unit.unit_id_hex(),
            unit.progress.opening_generation()
        );
    }
    Ok(())
}

/// One prepared profile: the fixed corpus under the source generation, with
/// both generations' recipient records persisted for the child to reopen.
struct Prepared {
    root: TestRoot,
    source_master: VaultMasterKey,
    target_master: VaultMasterKey,
    descriptors: Vec<ArtifactDescriptor>,
}

/// Builds a profile with the fixed corpus and both generations persisted.
fn prepare(label: &str) -> Result<Prepared, Box<dyn Error>> {
    let root = TestRoot::new(label)?;
    let (source_master, source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, target_record) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    persist_generations(root.path(), &source_record, &target_record)?;
    let vault = open_vault(root.path(), &source_master)?;
    let descriptors = seal_corpus(&vault, CORPUS)?;
    Ok(Prepared {
        root,
        source_master,
        target_master,
        descriptors,
    })
}

// ---------------------------------------------------------------------------
// interrupted_rewrap_has_exactly_one_opening_key
// ---------------------------------------------------------------------------

/// `KY03`. A process killed at any of the four points a rewrap passes through
/// leaves every object opened by exactly one of the old and new keys, and the
/// key that opens it is the one the journal says.
#[test]
fn interrupted_rewrap_has_exactly_one_opening_key() -> TestResult {
    for (stage_index, stage) in KY03_STAGES.iter().enumerate() {
        let Prepared {
            root,
            source_master,
            target_master,
            descriptors,
        } = prepare(&format!("ky03-{stage_index}"))?;
        let units = units_for(&descriptors);
        let source_vault = open_vault(root.path(), &source_master)?;
        let target_vault = open_vault(root.path(), &target_master)?;

        // Two units move cleanly first, so the interrupted unit is not also the
        // first unit and the journal has real history behind it.
        let plan = rotation_plan(&source_master, &target_master, &units)?;
        {
            let mut journal =
                AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
            let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
            engine.begin(&mut journal)?;
            engine.rotate_object(&mut journal, &units[0], &descriptors[0])?;
        }

        // The third unit is left untouched, so every stage is observed with a
        // migrated unit, an interrupted unit, and an untouched unit at once.
        kill_at(root.path(), stage, 1)?;

        // The child really got where it says it did. Without this, a child that
        // aborted on its first instruction would pass every stage.
        let journal = AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
        let target_descriptor =
            rebind_locator(&descriptors[1], &domain_kek(&target_master)?, profile_id())?;
        let target_path = target_vault.layout().object_path(&target_descriptor)?;
        let progress = state.units()[1].progress;
        match *stage {
            "KY03:before-reseal" => {
                assert!(
                    !target_path.exists(),
                    "{stage}: a target object was written before the reseal point"
                );
                assert_eq!(progress, academic_retention::UnitProgress::Planned);
            }
            "KY03:after-reseal" => {
                assert!(
                    target_path.is_file(),
                    "{stage}: the reseal did not happen before the kill"
                );
                assert_eq!(
                    progress,
                    academic_retention::UnitProgress::Planned,
                    "{stage}: the journal recorded a reseal that had not been journalled yet"
                );
            }
            "KY03:after-resealed-record" => {
                assert!(target_path.is_file(), "{stage}: the reseal did not happen");
                assert_eq!(
                    progress,
                    academic_retention::UnitProgress::Resealed,
                    "{stage}: the journal did not record the reseal"
                );
            }
            "KY03:after-migrated-record" => {
                assert!(target_path.is_file(), "{stage}: the reseal did not happen");
                assert_eq!(
                    progress,
                    academic_retention::UnitProgress::Migrated,
                    "{stage}: the journal did not record the migration"
                );
                // The source object is still there. The rotation never removes
                // it, which is why a kill here cannot leave an object neither
                // key opens.
                assert!(
                    source_vault
                        .layout()
                        .object_path(&descriptors[1])?
                        .is_file(),
                    "{stage}: the rotation removed the superseded object"
                );
            }
            other => return Err(format!("unhandled stage {other}").into()),
        }
        drop(journal);

        // The invariant, over every unit, at this exact on-disk state.
        assert_exactly_one_opening_key(root.path(), &source_master, &target_master, &descriptors)?;

        // And the interrupted unit specifically opens under the generation this
        // stage says it must, named independently of the journal's own mapping.
        let source_kek = domain_kek(&source_master)?;
        let target_kek = domain_kek(&target_master)?;
        let keys = RotationKeys {
            profile: profile_id(),
            source_kek: &source_kek,
            target_kek: &target_kek,
        };
        assert_eq!(
            observe_reachable_opening(&source_vault, &keys, &descriptors[1], progress)?,
            expected_observation(stage),
            "{stage}: the interrupted unit opens under the wrong generation"
        );

        // Resumable: the same engine picks up exactly the remaining units and
        // finishes, and the invariant still holds afterwards.
        {
            let mut journal =
                AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
            let resumed =
                RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
            resumed.require_generations(
                generation_of(&source_master)?,
                generation_of(&target_master)?,
            )?;
            let remaining: Vec<String> = resumed
                .remaining()
                .iter()
                .filter_map(|unit| unit.unit.source_locator().map(hex::encode))
                .collect();
            // The interrupted unit is unit 1. It has moved only at the last
            // stage, where reachability was journalled before the kill; at every
            // earlier stage it is still the source generation's, however much of
            // its target object exists on disk.
            let first_remaining = usize::from(*stage == "KY03:after-migrated-record") + 1;
            let expected: Vec<String> = descriptors[first_remaining..]
                .iter()
                .map(|descriptor| hex::encode(descriptor.vault_locator.as_bytes()))
                .collect();
            assert_eq!(
                remaining, expected,
                "{stage}: the journal did not enumerate the exact remaining objects"
            );

            // The resume moves exactly what the journal named, and nothing else.
            let by_locator: std::collections::BTreeMap<String, &ArtifactDescriptor> = descriptors
                .iter()
                .map(|descriptor| (hex::encode(descriptor.vault_locator.as_bytes()), descriptor))
                .collect();
            let pending: Vec<(RotationUnit, ArtifactDescriptor)> = resumed
                .remaining()
                .iter()
                .filter_map(|state| {
                    let locator = hex::encode(state.unit.source_locator()?);
                    Some((state.unit.clone(), (*by_locator.get(&locator)?).clone()))
                })
                .collect();
            assert_eq!(pending.len(), expected.len());
            let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
            for (unit, descriptor) in &pending {
                engine.rotate_object(&mut journal, unit, descriptor)?;
            }
            engine.complete(&mut journal)?;
        }
        assert_exactly_one_opening_key(root.path(), &source_master, &target_master, &descriptors)?;

        // And after a completed rotation every object really is under the new
        // key and not under the old one.
        for descriptor in &descriptors {
            let rebound = rebind_locator(descriptor, &domain_kek(&target_master)?, profile_id())?;
            let path = target_vault.layout().object_path(&rebound)?;
            assert_eq!(
                probe_header(&path, &domain_kek(&target_master)?),
                HeaderProbe::Opened
            );
            assert_eq!(
                probe_header(&path, &domain_kek(&source_master)?),
                HeaderProbe::WrongKey
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// KY04 / KY05
// ---------------------------------------------------------------------------

/// `KY04` and `KY05`. A kill between writing the replacement recipient set and
/// renaming it over the live one leaves the old set exactly as it was.
#[test]
fn recipient_set_is_old_or_new_and_never_partial() -> TestResult {
    const RENAME_FAULT: &str = "KY04:recipient-set-rename";
    for (label, operation) in [("ky04-add", "add"), ("ky05-revoke", "revoke")] {
        let root = TestRoot::new(label)?;
        let (source_master, source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
        let (target_master, target_record) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
        persist_generations(root.path(), &source_record, &target_record)?;

        // Two recipients hold the source generation before the kill.
        let mut journal =
            AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        for record in [source_record.clone(), target_record.clone()] {
            recipients::add_recipient(
                root.path(),
                profile_id(),
                &mut journal,
                record,
                generation_of(&source_master)?,
            )?;
        }
        drop(journal);
        let before = fs::read(
            root.path()
                .join(academic_retention::RECIPIENTS_RELATIVE_PATH),
        )?;

        let is_revoke = operation == "revoke";
        let marker = root.path().join(format!("ready-{label}"));
        let status = Command::new(env::current_exe()?)
            .arg("retention_fault_child_entrypoint")
            .arg("--exact")
            .env(CHILD_ENV, "1")
            .env(PROFILE_ENV, root.path())
            .env(UNIT_ENV, "0")
            .env(FAULT_SELECTION_VARIABLE, RENAME_FAULT)
            .env(FAULT_READY_MARKER_VARIABLE, &marker)
            .env(OPERATION_ENV, operation)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        assert!(!status.success(), "{label}: the child exited cleanly");
        assert!(
            marker.is_file(),
            "{label}: the child never reached the rename"
        );

        // The set on disk is byte-identical to the old one: not a partial set,
        // and not the new one.
        let after = fs::read(
            root.path()
                .join(academic_retention::RECIPIENTS_RELATIVE_PATH),
        )?;
        assert_eq!(
            after, before,
            "{label}: the recipient set changed despite the kill before the rename"
        );
        let set = recipients::read_set(root.path(), profile_id())?;
        assert_eq!(set.records().len(), 2, "{label}: the set is partial");

        // Re-running without the fault produces the new set, so the failpoint
        // was the only thing standing between old and new.
        let mut journal =
            AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        if is_revoke {
            let outcome = recipients::revoke_recipient(
                root.path(),
                profile_id(),
                &mut journal,
                &SOURCE_RECIPIENT,
                generation_of(&source_master)?,
                vec![hex::encode([0xAB_u8; 32])],
            )?;
            assert_eq!(outcome.remaining_recipients(), 1);
            // `KY05`: the objects still under the revoked generation are named.
            assert_eq!(
                outcome.still_under_revoked_generation(),
                &[hex::encode([0xAB_u8; 32])]
            );
            let set = recipients::read_set(root.path(), profile_id())?;
            assert_eq!(set.records().len(), 1);
            assert!(
                !set.records()
                    .iter()
                    .any(|record| record.recipient_id() == &SOURCE_RECIPIENT)
            );
            // And the revoked recipient receives no new key.
            let refused = recipients::rewrap_for_generation(
                root.path(),
                profile_id(),
                &mut journal,
                generation_of(&target_master)?,
                |_| Ok(source_record.clone()),
            );
            assert!(
                refused.is_err(),
                "{label}: the revoked recipient was rewrapped"
            );
        } else {
            let (_, added) = create_generation([0xD4; 16], [0xD4; 32])?;
            recipients::add_recipient(
                root.path(),
                profile_id(),
                &mut journal,
                added,
                generation_of(&source_master)?,
            )?;
            let set = recipients::read_set(root.path(), profile_id())?;
            assert_eq!(set.records().len(), 3);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// RB01
// ---------------------------------------------------------------------------

/// `RB01`. A kill before the key slot write leaves the object intact; a slot
/// write that was interrupted part way has already destroyed the key, and a
/// re-application repairs its label. Never anything in between.
#[test]
fn crypto_shred_kill_leaves_shredded_or_intact() -> TestResult {
    let Prepared {
        root,
        source_master,
        descriptors,
        ..
    } = prepare("rb01")?;
    let vault = open_vault(root.path(), &source_master)?;
    let kek = domain_kek(&source_master)?;
    let path = vault.layout().object_path(&descriptors[0])?;
    let intact = fs::read(&path)?;

    // Kill before the slot write: intact.
    kill_at(root.path(), "RB01", 0)?;
    assert_eq!(
        fs::read(&path)?,
        intact,
        "the object changed before the write"
    );
    assert_eq!(probe_header(&path, &kek), HeaderProbe::Opened);

    // A slot write that landed only part way: the key is already destroyed, so
    // the object is unreadable, but it is not yet labelled as shredded.
    let stone = BackupTombstone::new(
        hex::encode([0x51_u8; 16]),
        *descriptors[0].vault_locator.as_bytes(),
        3,
    );
    let mut torn = intact.clone();
    let full = academic_vault::object::shredded_key_slot(&stone.digest());
    torn[KEY_SLOT_OFFSET..KEY_SLOT_OFFSET + 8].copy_from_slice(&full[..8]);
    fs::write(&path, &torn)?;
    assert_ne!(
        probe_header(&path, &kek),
        HeaderProbe::Opened,
        "a torn slot write left the object readable"
    );
    assert_ne!(
        probe_header(&path, &kek),
        HeaderProbe::Shredded,
        "a torn slot write was already labelled as a completed shred"
    );

    // Re-applying repairs the label without touching anything else.
    let receipt = vault.shred_key_slot(&descriptors[0], &stone.digest())?;
    assert!(!receipt.was_already_shredded());
    assert_eq!(probe_header(&path, &kek), HeaderProbe::Shredded);
    let repaired = fs::read(&path)?;
    assert_eq!(repaired[..KEY_SLOT_OFFSET], intact[..KEY_SLOT_OFFSET]);
    assert_eq!(repaired[HEADER_BYTES..], intact[HEADER_BYTES..]);
    assert_eq!(
        &repaired[KEY_SLOT_OFFSET..KEY_SLOT_OFFSET + KEY_SLOT_SHRED_MARKER.len()],
        KEY_SLOT_SHRED_MARKER.as_slice()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// RB02
// ---------------------------------------------------------------------------

/// `RB02`. A kill before a backup tombstone is written leaves the backup with
/// no tombstone at all — never a partial one — so the deletion is visibly
/// incomplete rather than quietly complete.
#[test]
fn interrupted_backup_tombstone_leaves_no_partial_tombstone() -> TestResult {
    let Prepared { root, .. } = prepare("rb02")?;
    let backup = root.path().join("backup");
    fs::create_dir_all(&backup)?;

    kill_at(root.path(), "RB02:before-tombstone", 0)?;

    let present = tombstone::read_from_backup(&backup)?;
    assert!(
        present.is_empty(),
        "a tombstone survived a kill that happened before it was written"
    );
    let directory = tombstone::tombstone_dir(&backup);
    if directory.exists() {
        let leftovers: Vec<PathBuf> = fs::read_dir(&directory)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect();
        assert!(
            leftovers.is_empty(),
            "the interrupted write left {leftovers:?} behind"
        );
    }
    Ok(())
}
