//! The gates a rotation's irreversible operations are decided by (`T114`).
//!
//! Every test here is a `T114` reproduction that destroyed live data through
//! the shipped API with no kill and no tampering — only an ordering or an
//! argument the gates did not check. They are stated as properties of the
//! repaired gates, so a gate that stops biting fails a named row rather than
//! quietly widening.
//!
//! `retire_superseded_object` now reads the canonical reference instead of
//! being told it. The store cannot link into this crate, so what is exercised
//! here is the gate over a reference that states an answer; the same gate over
//! the *real* store is in the encrypted portability suite,
//! `a_retirement_before_the_store_row_is_refused`.

#![cfg(feature = "rotation-engine")]

mod rotation_support;

use std::error::Error;

use academic_domain::{ArtifactDescriptor, ArtifactId};
use academic_retention::{
    AppendOnlyJournal, BackupTombstone, RotationId, RotationPlan, RotationState, RotationUnit,
    engine::{
        HeaderProbe, RotationEngine, apply_tombstones, probe_header, retire_superseded_object,
        shred_with_tombstone,
    },
    journal::ROTATION_JOURNAL_RELATIVE_PATH,
    recipients::{add_recipient, read_set, retire_generation, rewrap_for_generation},
    rotation::{
        CanonicalReference, CanonicalReferenceError, KeyGeneration, StoreDatabaseError,
        StoreDatabaseExecutor, StoreDatabaseRekey, store_database_target_id,
    },
};
use academic_vault::SealedObjectVerifier as _;
use rotation_support::{
    SOURCE_ENTROPY, SOURCE_RECIPIENT, TARGET_ENTROPY, TARGET_RECIPIENT, TestRoot,
    create_generation, domain_kek, generation_of, open_vault, profile_id, publish_generations,
    seal_corpus, unlock_generation,
};

type TestResult = Result<(), Box<dyn Error>>;

/// A canonical reference that answers with the locator a test names.
///
/// It stands in for the store, which cannot link into this crate. What it lets
/// each row below state is the thing the audit exploited: the gates were
/// checked against the *caller's* claim, so a caller whose claim was true of
/// the journal and false of the store destroyed a live object.
struct StatedReference(Option<[u8; 32]>);

impl CanonicalReference for StatedReference {
    fn resolved_locator(
        &self,
        _artifact: ArtifactId,
    ) -> Result<Option<[u8; 32]>, CanonicalReferenceError> {
        Ok(self.0)
    }
}

/// An executor that reports the generations and the outcome a test tells it to.
///
/// The pair is separate from the plan's on purpose: the audit's reproduction is
/// an executor built from two masters that are not the two the plan names, which
/// nothing but the executor's own report can tell the engine.
struct StatedExecutor {
    source: KeyGeneration,
    target: KeyGeneration,
    outcome: Result<StoreDatabaseRekey, String>,
}

impl StatedExecutor {
    /// An executor holding exactly the pair its plan names.
    fn planned(plan: &RotationPlan, outcome: Result<StoreDatabaseRekey, String>) -> Self {
        Self {
            source: plan.source(),
            target: plan.target(),
            outcome,
        }
    }
}

impl StoreDatabaseExecutor for StatedExecutor {
    fn generations(&self) -> Result<(KeyGeneration, KeyGeneration), StoreDatabaseError> {
        Ok((self.source, self.target))
    }

    fn rekey_store_database(&self) -> Result<StoreDatabaseRekey, StoreDatabaseError> {
        self.outcome
            .as_ref()
            .copied()
            .map_err(|reason| StoreDatabaseError(reason.clone()))
    }
}

struct Rotated {
    root: TestRoot,
    source_master: academic_crypto::VaultMasterKey,
    target_master: academic_crypto::VaultMasterKey,
    descriptors: Vec<ArtifactDescriptor>,
    resealed: Vec<ArtifactDescriptor>,
    units: Vec<RotationUnit>,
}

fn journal_at(root: &TestRoot) -> Result<AppendOnlyJournal, Box<dyn Error>> {
    Ok(AppendOnlyJournal::open(
        &root.path().join(ROTATION_JOURNAL_RELATIVE_PATH),
    )?)
}

/// Seals `count` objects under the source generation and rotates the first
/// `rotate` of them to completion through the product engine.
fn rotate(label: &str, count: usize, rotate: usize) -> Result<Rotated, Box<dyn Error>> {
    let root = TestRoot::new(label)?;
    let (source_master, source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, target_record) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    publish_generations(
        root.path(),
        &source_master,
        &source_record,
        &target_master,
        &target_record,
    )?;
    let source = open_vault(root.path(), &source_master)?;
    let target = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source, count)?;
    let units: Vec<RotationUnit> = descriptors[..rotate]
        .iter()
        .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
        .collect();
    let plan = RotationPlan::new(
        RotationId::from_bytes([0xB1; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        units.clone(),
    )?;
    let engine = RotationEngine::new(&plan, &source, &target);
    let mut journal = journal_at(&root)?;
    engine.begin(&mut journal)?;
    let mut resealed = Vec::new();
    for (unit, descriptor) in units.iter().zip(&descriptors) {
        resealed.push(engine.rotate_object(&mut journal, unit, descriptor)?);
    }
    engine.complete(&mut journal)?;
    Ok(Rotated {
        root,
        source_master,
        target_master,
        descriptors,
        resealed,
        units,
    })
}

fn opens(
    record: &academic_crypto::RecipientRecord,
    entropy: [u8; 32],
    master: &academic_crypto::VaultMasterKey,
) -> bool {
    unlock_generation(record, entropy)
        .ok()
        .and_then(|opened| generation_of(&opened).ok())
        .zip(generation_of(master).ok())
        .is_some_and(|(left, right)| left == right)
}

// ---------------------------------------------------------------------------
// T114 P2-A — the retirement gates bind their arguments to the journal
// ---------------------------------------------------------------------------

/// A retirement destroys only the object its own unit superseded.
///
/// `T114`'s reproduction: the gates read the *unit* and the positioned write
/// followed the *descriptor*, so passing a rotated unit together with an
/// untouched artifact's live object crypto-shredded that artifact's only copy.
/// Nothing in the rotation had anything to do with it.
#[test]
fn a_retirement_only_destroys_the_object_its_own_unit_superseded() -> TestResult {
    let rotated = rotate("seam-retire-binding", 2, 1)?;
    let unrotated = rotated.descriptors[1].clone();
    let target_vault = open_vault(rotated.root.path(), &rotated.target_master)?;
    let source_kek = domain_kek(&rotated.source_master)?;
    let unrotated_path = target_vault.layout().object_path(&unrotated)?;
    assert_eq!(
        probe_header(&unrotated_path, &source_kek),
        HeaderProbe::Opened,
        "precondition: the untouched artifact's object opens"
    );

    let mut journal = journal_at(&rotated.root)?;
    let refused = retire_superseded_object(
        &mut journal,
        &target_vault,
        &rotated.units[0],
        &unrotated,
        &StatedReference(Some(*rotated.resealed[0].vault_locator.as_bytes())),
    );
    let message = refused
        .err()
        .ok_or("a rotated unit retired an artifact it never touched")?
        .to_string();
    assert!(
        message.contains("the object that unit supersedes is"),
        "the refusal did not name the mismatch: {message}"
    );
    assert_eq!(
        probe_header(&unrotated_path, &source_kek),
        HeaderProbe::Opened,
        "the untouched artifact's only object was destroyed"
    );
    Ok(())
}

/// Swapping the superseded and current descriptors destroys nothing.
///
/// `T114`'s reproduction: both gates were "these two locators differ" and "this
/// unit migrated", which the swap satisfies. The object the rotation moved *to*
/// was crypto-shredded while the journal went on saying the unit was migrated
/// to it, so the artifact was opened by neither generation.
#[test]
fn a_retirement_with_the_descriptors_swapped_destroys_nothing() -> TestResult {
    let rotated = rotate("seam-retire-swapped", 1, 1)?;
    let target_vault = open_vault(rotated.root.path(), &rotated.target_master)?;
    let target_kek = domain_kek(&rotated.target_master)?;
    let current_path = target_vault.layout().object_path(&rotated.resealed[0])?;
    assert_eq!(
        probe_header(&current_path, &target_kek),
        HeaderProbe::Opened
    );

    let mut journal = journal_at(&rotated.root)?;
    let refused = retire_superseded_object(
        &mut journal,
        &target_vault,
        &rotated.units[0],
        &rotated.resealed[0],
        &StatedReference(Some(*rotated.descriptors[0].vault_locator.as_bytes())),
    );
    assert!(
        refused.is_err(),
        "the object the rotation moved to was retired"
    );
    assert_eq!(
        probe_header(&current_path, &target_kek),
        HeaderProbe::Opened,
        "the reachable object's key slot was destroyed"
    );
    target_vault.verify_sealed_object(&rotated.resealed[0])?;
    Ok(())
}

/// A retirement is refused while the canonical reference has not moved.
///
/// `T114`'s reproduction: the journal said the unit migrated and the caller
/// passed the re-seal result as the descriptor "the store now resolves to",
/// which was simply not true yet — the store row had not been written. The old
/// object was destroyed and the store still named it, so no key opened the
/// artifact. The gate is now the reference itself.
#[test]
fn a_retirement_is_refused_while_the_reference_still_names_the_superseded_object() -> TestResult {
    let rotated = rotate("seam-retire-early", 1, 1)?;
    let target_vault = open_vault(rotated.root.path(), &rotated.target_master)?;
    let source_kek = domain_kek(&rotated.source_master)?;
    let superseded_path = target_vault.layout().object_path(&rotated.descriptors[0])?;

    let mut journal = journal_at(&rotated.root)?;
    let refused = retire_superseded_object(
        &mut journal,
        &target_vault,
        &rotated.units[0],
        &rotated.descriptors[0],
        &StatedReference(Some(*rotated.descriptors[0].vault_locator.as_bytes())),
    );
    let message = refused
        .err()
        .ok_or("a retirement ran while the reference still named the superseded object")?
        .to_string();
    assert!(
        message.contains("the store resolves the artifact to"),
        "the refusal did not name the reference: {message}"
    );
    assert_eq!(
        probe_header(&superseded_path, &source_kek),
        HeaderProbe::Opened,
        "the object the reference still names was destroyed"
    );

    // An artifact the store holds no descriptor for is a refusal, not a
    // permission: "nothing says it moved" is not "it moved".
    let absent = retire_superseded_object(
        &mut journal,
        &target_vault,
        &rotated.units[0],
        &rotated.descriptors[0],
        &StatedReference(None),
    );
    assert!(
        absent
            .err()
            .ok_or("an artifact absent from the store was retired")?
            .to_string()
            .contains("holds no descriptor"),
        "the refusal did not name the absent descriptor"
    );

    // With the reference moved to the locator the journal recorded, it runs.
    let retired = retire_superseded_object(
        &mut journal,
        &target_vault,
        &rotated.units[0],
        &rotated.descriptors[0],
        &StatedReference(Some(*rotated.resealed[0].vault_locator.as_bytes())),
    );
    assert!(retired.is_ok(), "the repaired gate refused a correct call");
    assert_eq!(
        probe_header(&superseded_path, &source_kek),
        HeaderProbe::Shredded
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P2-B — retire_generation keeps the generation the rotation left
// ---------------------------------------------------------------------------

/// Retiring a generation refuses to keep the one the rotation superseded.
///
/// `T114`'s reproduction: `kept_generation` reached only the error message. A
/// completed rotation followed by `retire_generation(kept = the old one)`
/// removed every record that opened the generation each reachable object was
/// now under, and reported success.
#[test]
fn retiring_a_generation_refuses_to_keep_the_superseded_one() -> TestResult {
    let rotated = rotate("seam-retire-wrong-generation", 2, 2)?;
    let journal = journal_at(&rotated.root)?;
    assert!(RotationState::replay(journal.entries())?.is_some_and(|state| state.is_complete()));

    let refused = retire_generation(
        rotated.root.path(),
        profile_id(),
        &journal,
        generation_of(&rotated.source_master)?,
        |record| opens(record, SOURCE_ENTROPY, &rotated.source_master),
    );
    let message = refused
        .err()
        .ok_or("the superseded generation was kept after a completed rotation")?
        .to_string();
    assert!(
        message.contains("no record on disk would open a reachable object"),
        "the refusal did not say what would be lost: {message}"
    );

    let set = read_set(rotated.root.path(), profile_id())?;
    assert!(
        set.records()
            .iter()
            .any(|record| opens(record, TARGET_ENTROPY, &rotated.target_master)),
        "the refused call still removed the generation the objects are under"
    );

    // Keeping the generation the rotation moved to is what it is for.
    let kept = retire_generation(
        rotated.root.path(),
        profile_id(),
        &journal,
        generation_of(&rotated.target_master)?,
        |record| opens(record, TARGET_ENTROPY, &rotated.target_master),
    )?;
    assert_eq!(kept.len(), 1);
    Ok(())
}

/// Retiring a generation is refused when no rotation is recorded at all.
///
/// `T114`'s reproduction: with `RotationState::replay` returning `None` the
/// gate did not run, so a profile that had never rotated could have the only
/// generation opening every one of its objects removed from disk.
#[test]
fn retiring_a_generation_is_refused_without_a_recorded_rotation() -> TestResult {
    let root = TestRoot::new("seam-retire-no-rotation")?;
    let (source_master, source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, target_record) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    publish_generations(
        root.path(),
        &source_master,
        &source_record,
        &target_master,
        &target_record,
    )?;
    let source = open_vault(root.path(), &source_master)?;
    let descriptors = seal_corpus(&source, 2)?;
    let journal = journal_at(&root)?;
    assert!(
        RotationState::replay(journal.entries())?.is_none(),
        "precondition: no rotation recorded"
    );

    let refused = retire_generation(
        root.path(),
        profile_id(),
        &journal,
        generation_of(&target_master)?,
        |record| opens(record, TARGET_ENTROPY, &target_master),
    );
    let message = refused
        .err()
        .ok_or("a generation was retired with no rotation to justify it")?
        .to_string();
    assert!(
        message.contains("records no rotation"),
        "the refusal did not say the journal records no rotation: {message}"
    );

    let set = read_set(root.path(), profile_id())?;
    assert!(
        set.records()
            .iter()
            .any(|record| opens(record, SOURCE_ENTROPY, &source_master)),
        "the generation every object is under was removed"
    );
    let source_kek = domain_kek(&source_master)?;
    assert_eq!(
        probe_header(&source.layout().object_path(&descriptors[0])?, &source_kek),
        HeaderProbe::Opened
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P3-B — a rewrap is neither a re-run nor a new identity
// ---------------------------------------------------------------------------

/// A rewrap that produces another identity's record is refused.
///
/// `T114`'s observation: the produced record was appended beside the survivors
/// with no check that it belonged to the recipient it was produced for, so one
/// stored identity ended up with three records and an unrelated identity gained
/// a copy of the new key.
#[test]
fn a_rewrap_that_changes_identity_is_refused() -> TestResult {
    let root = TestRoot::new("seam-rewrap-identity")?;
    let (source_master, source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, target_record) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let mut journal = journal_at(&root)?;
    add_recipient(
        root.path(),
        profile_id(),
        &mut journal,
        source_record,
        generation_of(&source_master)?,
    )?;

    let refused = rewrap_for_generation(
        root.path(),
        profile_id(),
        &mut journal,
        generation_of(&target_master)?,
        |_| Ok(target_record.clone()),
    );
    let message = refused
        .err()
        .ok_or("a rewrap minted a record for a different recipient")?
        .to_string();
    assert!(
        message.contains("mints no new identity"),
        "the refusal did not name the identity change: {message}"
    );
    assert_eq!(
        read_set(root.path(), profile_id())?.records().len(),
        1,
        "the refused rewrap still wrote a record"
    );
    Ok(())
}

/// A rewrap re-run is refused rather than adding a third record.
///
/// `T114`'s observation: the produced records were appended unconditionally, so
/// a resume that re-ran the rewrap doubled the set every time.
/// `recipients.cbor` is `P2-K1`'s frozen document and carries no generation, so
/// the record count per identity is the only thing this crate can check without
/// a key — and after a rewrap it is exactly two.
#[test]
fn a_rewrap_re_run_is_refused_rather_than_duplicated() -> TestResult {
    let root = TestRoot::new("seam-rewrap-rerun")?;
    let (source_master, source_record) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let target_master = academic_crypto::VaultMasterKey::generate()?;
    let rewrapped_record = academic_crypto::create_recovery_recipient(
        &target_master,
        profile_id(),
        SOURCE_RECIPIENT,
        &academic_crypto::RecoverySecret::from_entropy(SOURCE_ENTROPY),
        academic_crypto::RECOVERY_ARGON2ID_V1,
    )?;
    let mut journal = journal_at(&root)?;
    add_recipient(
        root.path(),
        profile_id(),
        &mut journal,
        source_record,
        generation_of(&source_master)?,
    )?;

    let first = rewrap_for_generation(
        root.path(),
        profile_id(),
        &mut journal,
        generation_of(&target_master)?,
        |_| Ok(rewrapped_record.clone()),
    )?;
    assert_eq!(first.len(), 1);
    assert_eq!(read_set(root.path(), profile_id())?.records().len(), 2);

    let refused = rewrap_for_generation(
        root.path(),
        profile_id(),
        &mut journal,
        generation_of(&target_master)?,
        |_| Ok(rewrapped_record.clone()),
    );
    let message = refused
        .err()
        .ok_or("a rewrap re-run added a third record")?
        .to_string();
    assert!(
        message.contains("has already been written"),
        "the refusal did not say the rewrap already ran: {message}"
    );
    assert_eq!(
        read_set(root.path(), profile_id())?.records().len(),
        2,
        "the re-run still duplicated the set"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P1-B — the store database unit has an executor
// ---------------------------------------------------------------------------

/// A rotation completes once its store database unit has run its executor.
///
/// The executor itself is the encrypted store lane's `PRAGMA rekey` and is
/// exercised over a real database in the encrypted portability suite. What this
/// states is the journal half: the unit records `UnitResealed` and
/// `UnitMigrated` like any other, so `complete` stops refusing — and an
/// executor that refuses stops the rotation instead of being recorded.
#[test]
fn a_rotation_completes_once_its_store_database_unit_has_run() -> TestResult {
    let root = TestRoot::new("seam-store-unit")?;
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source_vault = open_vault(root.path(), &source_master)?;
    let target_vault = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source_vault, 1)?;

    let object = RotationUnit::object(*descriptors[0].vault_locator.as_bytes());
    let database = RotationUnit::store_database(profile_id());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x67; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        vec![object.clone(), database.clone()],
    )?;
    let mut journal = journal_at(&root)?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;
    engine.rotate_object(&mut journal, &object, &descriptors[0])?;

    // An executor that refuses stops the rotation and says who refused.
    let refusal = engine
        .rotate_store_database(
            &mut journal,
            &database,
            &StatedExecutor::planned(&plan, Err("page one did not authenticate".to_owned())),
        )
        .err()
        .ok_or("a refused rekey was recorded as a migration")?
        .to_string();
    assert!(
        refusal.contains("could not be rekeyed"),
        "the refusal did not name the executor: {refusal}"
    );
    let incomplete = engine
        .complete(&mut journal)
        .err()
        .ok_or("a rotation completed over a database its executor refused")?
        .to_string();
    assert!(
        incomplete.contains("never ran its executor"),
        "the refusal did not name the database unit: {incomplete}"
    );

    // An object unit is not a database unit, and the engine says which.
    assert!(
        engine
            .rotate_store_database(
                &mut journal,
                &object,
                &StatedExecutor::planned(&plan, Ok(StoreDatabaseRekey::Rekeyed)),
            )
            .is_err(),
        "an object unit was rekeyed as a database"
    );

    let outcome = engine.rotate_store_database(
        &mut journal,
        &database,
        &StatedExecutor::planned(&plan, Ok(StoreDatabaseRekey::Rekeyed)),
    )?;
    assert_eq!(outcome, StoreDatabaseRekey::Rekeyed);
    engine.complete(&mut journal)?;

    let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    assert!(state.is_complete());
    assert!(state.remaining().is_empty());
    let recorded = state
        .units()
        .iter()
        .find(|unit| unit.unit.unit_id_hex() == database.unit_id_hex())
        .and_then(|unit| unit.target_locator.clone())
        .ok_or("the database unit recorded no target")?;
    assert_eq!(
        recorded,
        hex::encode(store_database_target_id(
            profile_id(),
            generation_of(&target_master)?,
        )),
        "the database unit recorded something other than the generation it now opens under"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// T116 P2-N2 — the database unit's executor is bound to the plan
// ---------------------------------------------------------------------------

/// A store database executor that does not hold the plan's generations is
/// refused, before it can rekey anything.
///
/// `T116`'s reproduction: the records this unit appends are pure functions of
/// the plan — `store_database_target_id(profile, plan target)` — and nothing
/// compared them against the pair the executor actually held. An executor built
/// from the plan's source and an unrelated third master returned `Rekeyed`, the
/// journal recorded the unit migrated to the plan's target and the rotation
/// complete, and the database opened under neither generation the journal named.
/// `retire_generation` would then pass its own gates and remove the records that
/// still opened it.
///
/// A rekey is not undone by reading the journal afterwards, so the check is in
/// front of the executor rather than after it.
#[test]
fn a_store_database_executor_outside_the_plan_is_refused_before_it_runs() -> TestResult {
    let root = TestRoot::new("seam-store-unit-generation")?;
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source_vault = open_vault(root.path(), &source_master)?;
    let target_vault = open_vault(root.path(), &target_master)?;
    let descriptors = seal_corpus(&source_vault, 1)?;

    let object = RotationUnit::object(*descriptors[0].vault_locator.as_bytes());
    let database = RotationUnit::store_database(profile_id());
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x68; 16]),
        profile_id(),
        generation_of(&source_master)?,
        generation_of(&target_master)?,
        vec![object.clone(), database.clone()],
    )?;
    let mut journal = journal_at(&root)?;
    let engine = RotationEngine::new(&plan, &source_vault, &target_vault);
    engine.begin(&mut journal)?;
    engine.rotate_object(&mut journal, &object, &descriptors[0])?;

    // A third generation, which is what an orchestrator holding the wrong
    // master produces.
    let (stray_master, _) = create_generation(TARGET_RECIPIENT, [0x7c; 32])?;
    let stray = generation_of(&stray_master)?;
    assert_ne!(stray, plan.target());
    for (source, target) in [
        (plan.source(), stray),
        (stray, plan.target()),
        (stray, plan.source()),
    ] {
        let refusal = engine
            .rotate_store_database(
                &mut journal,
                &database,
                &StatedExecutor {
                    source,
                    target,
                    outcome: Ok(StoreDatabaseRekey::Rekeyed),
                },
            )
            .err()
            .ok_or("an executor outside the plan's generations was recorded as a migration")?
            .to_string();
        assert!(
            refusal.contains("does not hold the generations this rotation plans"),
            "the refusal did not name the generations the executor holds: {refusal}"
        );
        assert!(
            refusal.contains(&source.to_hex()) && refusal.contains(&target.to_hex()),
            "the refusal did not print the executor's own pair: {refusal}"
        );
    }

    // Nothing was journalled for the unit, so the rotation is still stopped by
    // name and a correct executor still finishes it.
    let state = RotationState::replay(journal.entries())?.ok_or("no rotation replayed")?;
    let recorded = state
        .units()
        .iter()
        .find(|unit| unit.unit.unit_id_hex() == database.unit_id_hex())
        .ok_or("the database unit is not in the replayed plan")?;
    assert_eq!(recorded.target_locator, None);
    let incomplete = engine
        .complete(&mut journal)
        .err()
        .ok_or("a rotation completed over a database unit the engine refused")?
        .to_string();
    assert!(
        incomplete.contains("never ran its executor"),
        "the refusal did not name the database unit: {incomplete}"
    );
    engine.rotate_store_database(
        &mut journal,
        &database,
        &StatedExecutor::planned(&plan, Ok(StoreDatabaseRekey::Rekeyed)),
    )?;
    engine.complete(&mut journal)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// T114 P1-C — a tombstone reaches the copy a pre-rotation backup holds
// ---------------------------------------------------------------------------

/// A tombstone reaches a copy stored under a locator the artifact has left.
///
/// `T114`'s reproduction: a locator is a function of the domain KEK, so the
/// copy inside a backup taken before a rotation sits under a name the tombstone
/// never mentioned. The restore matched nothing, resurrected the artifact, and
/// reported an empty re-deletion list with no error.
#[test]
fn a_tombstone_reaches_a_copy_under_a_superseded_locator() -> TestResult {
    let rotated = rotate("seam-tombstone-chain", 1, 1)?;
    let source_vault = open_vault(rotated.root.path(), &rotated.source_master)?;
    let source_kek = domain_kek(&rotated.source_master)?;
    let pre_rotation_path = source_vault.layout().object_path(&rotated.descriptors[0])?;
    assert_eq!(
        probe_header(&pre_rotation_path, &source_kek),
        HeaderProbe::Opened,
        "precondition: the copy under the superseded locator opens"
    );

    // The deletion names the current locator and every locator the chain moved
    // through, which is what the store's `superseded_locators` returns.
    let stone = BackupTombstone::covering(
        hex::encode([0x31_u8; 16]),
        *rotated.resealed[0].vault_locator.as_bytes(),
        &[*rotated.descriptors[0].vault_locator.as_bytes()],
        1_700_000_000_002,
    );
    let target_vault = open_vault(rotated.root.path(), &rotated.target_master)?;
    let mut journal = journal_at(&rotated.root)?;
    shred_with_tombstone(&mut journal, &target_vault, &rotated.resealed[0], &stone)?;

    let objects_root = rotated.root.path().join("vault").join("v2");
    let applied = apply_tombstones(&objects_root, std::slice::from_ref(&stone))?;
    assert!(
        applied.applied.contains(&hex::encode(
            rotated.descriptors[0].vault_locator.as_bytes()
        )),
        "the deletion did not reach the copy under the superseded locator: {applied:?}"
    );
    assert!(
        applied.absent.is_empty(),
        "a tombstone that reached an object was reported absent: {applied:?}"
    );
    assert_eq!(
        probe_header(&pre_rotation_path, &source_kek),
        HeaderProbe::Shredded
    );

    // A tombstone that reaches nothing is reported rather than dropped.
    let unreachable =
        BackupTombstone::new(hex::encode([0x32_u8; 16]), [0x99; 32], 1_700_000_000_003);
    let nothing = apply_tombstones(&objects_root, &[unreachable])?;
    assert!(nothing.applied.is_empty());
    assert_eq!(nothing.absent, vec![hex::encode([0x99_u8; 32])]);
    Ok(())
}

/// The identity a store database unit records names its generation and nothing else.
#[test]
fn a_store_database_target_id_names_the_generation() -> TestResult {
    let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
    let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
    let source: KeyGeneration = generation_of(&source_master)?;
    let target: KeyGeneration = generation_of(&target_master)?;
    assert_ne!(
        store_database_target_id(profile_id(), source),
        store_database_target_id(profile_id(), target)
    );
    assert_eq!(
        store_database_target_id(profile_id(), target),
        store_database_target_id(profile_id(), target)
    );
    assert_eq!(
        hex::encode(store_database_target_id(profile_id(), target)).len(),
        64
    );
    Ok(())
}
