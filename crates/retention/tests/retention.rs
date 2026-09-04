//! The `P2-K5` acceptance rows that need no encrypted object.
//!
//! Every test here is one of the eight rows t068 section 5 names for this task,
//! under the exact name it gives. The four rows that need real
//! `AEAD_CHUNKED_V2` objects live in `rotation.rs`, and the kill matrix lives
//! in `faults.rs`.
//!
//! These run in the default workspace lane, so hosted CI runs them on every
//! platform.

use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use academic_crypto::{
    DeviceKeystore, IDENTIFIER_BYTES, KeystoreFailure, ProfileId, VaultMasterKey,
    create_device_recipient,
};
#[cfg(feature = "rotation-orchestration")]
use academic_crypto::{
    RECOVERY_ARGON2ID_V1, RecoverySecret, UnlockThrottle, create_recovery_recipient,
    unlock_with_device, unlock_with_recovery,
};
#[cfg(feature = "rotation-orchestration")]
use academic_retention::RotationState;
use academic_retention::{
    ActionId, AppendOnlyJournal, ClassResolution, DERIVATIVE_CLASSES, DeletionPlan,
    DerivativeClass, DerivativeResolver, ExecutionFailure, GATE_38_026_STATEMENT, JournalEntry,
    OriginalVoiceAuthority, PlannedAction, REVOCATION_SCOPE_STATEMENT, RetentionExecutor,
    RetentionOutcome, RetentionSubject, RotationId, RotationPlan, RotationUnit, UnresolvedReason,
    VoiceSpan, execute::settle, journal, journal::ROTATION_JOURNAL_RELATIVE_PATH, recipients,
    rotation::KeyGeneration,
};

type TestResult = Result<(), Box<dyn Error>>;

const PROFILE_BYTES: [u8; IDENTIFIER_BYTES] = [0x5A; IDENTIFIER_BYTES];
const DEVICE_A: [u8; IDENTIFIER_BYTES] = [0xA1; IDENTIFIER_BYTES];
const DEVICE_B: [u8; IDENTIFIER_BYTES] = [0xB2; IDENTIFIER_BYTES];
#[cfg(feature = "rotation-orchestration")]
const PHRASE_RECIPIENT: [u8; IDENTIFIER_BYTES] = [0xC3; IDENTIFIER_BYTES];
const LABEL_A: &str = "academic-os:device:a";
const LABEL_B: &str = "academic-os:device:b";

fn profile() -> ProfileId {
    ProfileId::from_bytes(PROFILE_BYTES)
}

/// One disposable directory per test.
#[derive(Debug)]
struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "academic-retention-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A broker that really holds the secret, standing in for a working host.
///
/// Two synthetic device identities exist in this corpus on purpose. t068
/// section 12.2 leaves "whether a second local device exists at all" an open
/// user decision; section 1.3 forbids a pairing protocol either way. Two
/// recipients is what makes revocation observable without implying one.
#[derive(Debug)]
struct MemoryKeystore {
    stored: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemoryKeystore {
    fn new() -> Self {
        Self {
            stored: Mutex::new(HashMap::new()),
        }
    }
}

impl DeviceKeystore for MemoryKeystore {
    fn provider(&self) -> &str {
        "TEST_MEMORY_BROKER"
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        let mut stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored.insert(label.to_owned(), secret.to_vec());
        Ok(label.as_bytes().to_vec())
    }

    fn open(
        &self,
        label: &str,
        blob: &[u8],
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeystoreFailure> {
        if blob != label.as_bytes() {
            return Err(KeystoreFailure::InvalidBlob);
        }
        let stored = self.stored.lock().unwrap_or_else(PoisonError::into_inner);
        stored
            .get(label)
            .cloned()
            .map(zeroize::Zeroizing::new)
            .ok_or(KeystoreFailure::NotFound)
    }
}

#[cfg(feature = "rotation-orchestration")]
fn phrase(byte: u8) -> RecoverySecret {
    RecoverySecret::from_entropy([byte; 32])
}

/// Opens one recovery-class record with the phrase that wrapped it.
#[cfg(feature = "rotation-orchestration")]
fn open_with_phrase(
    record: &academic_crypto::RecipientRecord,
    secret: &RecoverySecret,
) -> Result<VaultMasterKey, Box<dyn Error>> {
    let mut throttle = UnlockThrottle::new();
    Ok(unlock_with_recovery(
        record,
        profile(),
        secret,
        &mut throttle,
        0,
    )?)
}

/// Reports whether one stored record wraps `generation`.
///
/// This is the caller-side test [`recipients::retire_generation`] takes: the
/// retention crate holds no key, so the generation a record wraps is answered
/// by whoever can open it. A device record is tried through the broker and a
/// recovery record through the phrase; a record neither opens is not kept.
#[cfg(feature = "rotation-orchestration")]
fn opens_generation(
    record: &academic_crypto::RecipientRecord,
    secret: &RecoverySecret,
    keystore: &MemoryKeystore,
    generation: KeyGeneration,
) -> bool {
    let opened = unlock_with_device(record, profile(), keystore)
        .ok()
        .or_else(|| {
            let mut throttle = UnlockThrottle::new();
            unlock_with_recovery(record, profile(), secret, &mut throttle, 0).ok()
        });
    opened
        .and_then(|master| KeyGeneration::of(&master, profile()).ok())
        .is_some_and(|opened| opened == generation)
}

fn open_journal(root: &Path) -> Result<AppendOnlyJournal, Box<dyn Error>> {
    Ok(AppendOnlyJournal::open(
        &root.join(ROTATION_JOURNAL_RELATIVE_PATH),
    )?)
}

// ---------------------------------------------------------------------------
// revoked_recipient_gets_no_new_key
// ---------------------------------------------------------------------------

/// `KY05`. A revoked recipient is not offered the next generation's key, and
/// the objects still under the revoked generation are enumerated exactly.
///
/// The withholding is what `rewrap_for_generation` does, and that is rotation
/// orchestration Phase 2 does not accept, so this row runs in the
/// `rotation-orchestration-lane` job rather than in the default graph. What the
/// default graph proves in its place is the refusal, in
/// `a_rewrap_for_a_new_generation_is_refused` — and that revocation still
/// records what it records, in `revocation_does_not_claim_prior_plaintext_erasure`.
#[cfg(feature = "rotation-orchestration")]
#[test]
fn revoked_recipient_gets_no_new_key() -> TestResult {
    let root = TestRoot::new("revoke")?;
    let keystore = MemoryKeystore::new();
    let source = VaultMasterKey::generate()?;
    let source_generation = KeyGeneration::of(&source, profile())?;

    // Two device recipients and one recovery recipient hold generation one.
    let device_a = create_device_recipient(&source, profile(), DEVICE_A, LABEL_A, &keystore)?;
    let device_b = create_device_recipient(&source, profile(), DEVICE_B, LABEL_B, &keystore)?;
    let phrase_record = create_recovery_recipient(
        &source,
        profile(),
        PHRASE_RECIPIENT,
        &phrase(0x11),
        RECOVERY_ARGON2ID_V1,
    )?;
    let mut journal = open_journal(root.path())?;
    for record in [device_a.clone(), device_b.clone(), phrase_record.clone()] {
        recipients::add_recipient(
            root.path(),
            profile(),
            &mut journal,
            record,
            source_generation,
        )?;
    }
    // Device A really does open generation one before it is revoked, so the
    // refusal below is a change of state rather than a broker that never worked.
    let recovered = unlock_with_device(&device_a, profile(), &keystore)?;
    assert_eq!(
        KeyGeneration::of(&recovered, profile())?,
        source_generation,
        "device A did not open the generation it was wrapped for"
    );

    // A rotation is planned over two objects and interrupted after one.
    let target = VaultMasterKey::generate()?;
    let target_generation = KeyGeneration::of(&target, profile())?;
    let first = RotationUnit::object([0x01; 32]);
    let second = RotationUnit::object([0x02; 32]);
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x77; 16]),
        profile(),
        source_generation,
        target_generation,
        vec![first.clone(), second.clone()],
    )?;
    journal.append(plan.started_entry())?;
    journal.append(JournalEntry::UnitResealed {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: first.unit_id_hex(),
        target_locator: hex::encode([0xAA; 32]),
    })?;
    journal.append(JournalEntry::UnitMigrated {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: first.unit_id_hex(),
    })?;

    let state = RotationState::replay(journal.entries())?.ok_or("no rotation was replayed")?;
    let still_under_source: Vec<String> = state
        .remaining()
        .iter()
        .filter_map(|unit| unit.unit.source_locator().map(hex::encode))
        .collect();

    let outcome = recipients::revoke_recipient(
        root.path(),
        profile(),
        &mut journal,
        &DEVICE_A,
        source_generation,
        still_under_source.clone(),
    )?;

    // The exact enumeration, not a count: `KY05` requires the objects still
    // under the revoked generation to be named.
    assert_eq!(
        outcome.still_under_revoked_generation(),
        &[hex::encode([0x02_u8; 32])],
        "the revocation did not enumerate the exact objects still under the old key"
    );
    assert_eq!(outcome.remaining_recipients(), 2);

    // The revoked record is gone from the set on disk.
    let set = recipients::read_set(root.path(), profile())?;
    assert!(
        !set.records()
            .iter()
            .any(|record| record.recipient_id() == &DEVICE_A),
        "the revoked recipient is still in the stored set"
    );

    // The next generation is wrapped for the survivors and for nobody else.
    let rewrapped = recipients::rewrap_for_generation(
        root.path(),
        profile(),
        &mut journal,
        target_generation,
        |record| {
            if record.recipient_id() == &DEVICE_B {
                Ok(create_device_recipient(
                    &target,
                    profile(),
                    DEVICE_B,
                    LABEL_B,
                    &keystore,
                )?)
            } else {
                Ok(create_recovery_recipient(
                    &target,
                    profile(),
                    PHRASE_RECIPIENT,
                    &phrase(0x11),
                    RECOVERY_ARGON2ID_V1,
                )?)
            }
        },
    )?;
    let wrapped_ids: Vec<[u8; IDENTIFIER_BYTES]> = rewrapped
        .iter()
        .map(|record| *record.recipient_id())
        .collect();
    assert_eq!(
        wrapped_ids,
        vec![DEVICE_B, PHRASE_RECIPIENT],
        "the new generation was wrapped for the wrong recipient set"
    );

    // The physical consequence: device A's broker key opens nothing that names
    // the new generation, because no record for it exists to open.
    let stored = recipients::read_set(root.path(), profile())?;
    assert!(
        !stored
            .records()
            .iter()
            .any(|record| record.recipient_id() == &DEVICE_A),
        "a record for the revoked recipient reappeared"
    );
    // The new generation's records are written beside the old ones, not over
    // them. One object has not migrated yet, so a set holding only the new
    // generation would name a key that opens nothing for it while the key that
    // does open it is no longer on disk.
    //
    // The recovery recipient is the one this corpus can read both ways: its
    // records derive from the phrase, while the memory broker holds one secret
    // per device label and the second `create_device_recipient` call replaces
    // the first. So the phrase is what shows that both generations are on disk.
    let mut phrase_generations = Vec::new();
    for record in stored.records() {
        if record.recipient_id() == &PHRASE_RECIPIENT {
            let opened = open_with_phrase(record, &phrase(0x11))?;
            phrase_generations.push(KeyGeneration::of(&opened, profile())?);
        }
    }
    assert_eq!(
        phrase_generations,
        vec![source_generation, target_generation],
        "the rewrap did not leave both generations openable while a unit is still under the old key"
    );

    // Retiring the old generation is refused while that unit is outstanding.
    let refused = recipients::retire_generation(
        root.path(),
        profile(),
        &journal,
        target_generation,
        |record| opens_generation(record, &phrase(0x11), &keystore, target_generation),
    );
    let message = refused
        .err()
        .ok_or("the old generation was retired while a unit was still under it")?
        .to_string();
    assert!(
        message.contains("rotation is not complete") && message.contains("1 of 2"),
        "the refusal did not name the outstanding units: {message}"
    );

    // The rotation finishes, and only then does the old generation leave.
    journal.append(JournalEntry::UnitResealed {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: second.unit_id_hex(),
        target_locator: hex::encode([0xBB; 32]),
    })?;
    journal.append(JournalEntry::UnitMigrated {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: second.unit_id_hex(),
    })?;
    journal.append(JournalEntry::RotationCompleted {
        rotation_id: plan.rotation_id().to_hex(),
        unit_count: 2,
    })?;
    let kept = recipients::retire_generation(
        root.path(),
        profile(),
        &journal,
        target_generation,
        |record| opens_generation(record, &phrase(0x11), &keystore, target_generation),
    )?;
    assert_eq!(
        kept.len(),
        2,
        "retiring kept the wrong number of records: {kept:?}"
    );

    // And device B, which was not revoked, really does open the new generation.
    let stored = recipients::read_set(root.path(), profile())?;
    assert_eq!(
        stored.records().len(),
        2,
        "the old generation did not leave the recipient set"
    );
    let device_b_new = stored
        .records()
        .iter()
        .find(|record| record.recipient_id() == &DEVICE_B)
        .ok_or("device B lost its record")?;
    let opened = unlock_with_device(device_b_new, profile(), &keystore)?;
    assert_eq!(
        KeyGeneration::of(&opened, profile())?,
        target_generation,
        "the surviving recipient did not receive the new generation"
    );

    // Re-adding the revoked identity is refused as an identity, not as a
    // record: a caller minting a brand-new record under the same
    // `recipient_id` would otherwise hand it the current key straight back.
    let reminted = create_device_recipient(&target, profile(), DEVICE_A, LABEL_A, &keystore)?;
    let readded = recipients::add_recipient(
        root.path(),
        profile(),
        &mut journal,
        reminted,
        target_generation,
    );
    let message = readded
        .err()
        .ok_or("a revoked identity was added back to the recipient set")?
        .to_string();
    assert!(
        message.contains("recorded as revoked") && message.contains("receives no key"),
        "the refusal did not say a revoked identity receives no key: {message}"
    );
    let unchanged = recipients::read_set(root.path(), profile())?;
    assert!(
        !unchanged
            .records()
            .iter()
            .any(|record| record.recipient_id() == &DEVICE_A),
        "the refused addition still wrote the revoked identity"
    );

    // A caller holding a stale record cannot smuggle the revoked identity back
    // in: the journal's revocation history is checked against every produced
    // record.
    let replay = recipients::rewrap_for_generation(
        root.path(),
        profile(),
        &mut journal,
        target_generation,
        |_| Ok(device_a.clone()),
    );
    let message = replay
        .err()
        .ok_or("a rewrap for the revoked recipient was accepted")?
        .to_string();
    assert!(
        message.contains("revoked") && message.contains("receives no new key"),
        "the refusal did not say the revoked recipient receives no new key: {message}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// revocation_does_not_claim_prior_plaintext_erasure
// ---------------------------------------------------------------------------

/// The revocation surface says what revocation does and does not do, in the
/// exact words, and no source in this crate claims more.
///
/// This is a false-safety-claim test. It fails if a surface stops carrying the
/// statement, and it fails if any source starts asserting that revocation
/// erased, destroyed, wiped, or unrecovered plaintext that was already read.
#[test]
fn revocation_does_not_claim_prior_plaintext_erasure() -> TestResult {
    let root = TestRoot::new("revocation-claim")?;
    let keystore = MemoryKeystore::new();
    let master = VaultMasterKey::generate()?;
    let generation = KeyGeneration::of(&master, profile())?;
    let mut journal = open_journal(root.path())?;
    for (id, label) in [(DEVICE_A, LABEL_A), (DEVICE_B, LABEL_B)] {
        let record = create_device_recipient(&master, profile(), id, label, &keystore)?;
        recipients::add_recipient(root.path(), profile(), &mut journal, record, generation)?;
    }
    let outcome = recipients::revoke_recipient(
        root.path(),
        profile(),
        &mut journal,
        &DEVICE_A,
        generation,
        Vec::new(),
    )?;

    // 1. The statement itself, word for word.
    assert_eq!(
        REVOCATION_SCOPE_STATEMENT,
        "revocation stops this recipient from receiving any future key; \
         it does not erase plaintext that was already read, and it does not reach \
         a copy taken while the recipient was live"
    );
    assert_eq!(outcome.scope_statement(), REVOCATION_SCOPE_STATEMENT);

    // 2. The journal carries it, so an audit of the file alone cannot read a
    //    revocation as an erasure.
    let recorded = journal
        .entries()
        .find_map(|entry| match entry {
            JournalEntry::RecipientRevoked {
                scope_statement, ..
            } => Some(scope_statement.clone()),
            _ => None,
        })
        .ok_or("the journal recorded no revocation")?;
    assert_eq!(recorded, REVOCATION_SCOPE_STATEMENT);

    // 3. The three clauses are each present, so a future edit cannot keep the
    //    constant's name while dropping what it denies.
    for clause in [
        "does not erase plaintext that was already read",
        "does not reach a copy taken while the recipient was live",
        "stops this recipient from receiving any future key",
    ] {
        assert!(
            REVOCATION_SCOPE_STATEMENT.contains(clause),
            "the scope statement dropped the clause {clause:?}"
        );
    }

    // 4. No source in this crate claims a revocation erased anything. The scan
    //    is over claim-shaped phrases rather than bare words, so the denial
    //    above is not itself a hit.
    for (path, text) in read_crate_sources()? {
        for forbidden in [
            "revocation erases",
            "revoking erases",
            "revocation destroys",
            "revocation wipes",
            "revoked plaintext is erased",
            "revocation makes prior plaintext unrecoverable",
            "revocation deletes",
        ] {
            assert!(
                !text.to_lowercase().contains(forbidden),
                "{} claims {forbidden:?}, which revocation does not do",
                path.display()
            );
        }
    }
    Ok(())
}

/// Reads every library source of this crate.
///
/// `src` only, exactly as `P2-K4`'s scan does: the claim being checked is what
/// the *library* asserts, and a suite that lists the forbidden phrases in order
/// to forbid them would otherwise be its own first hit.
fn read_crate_sources() -> Result<Vec<(PathBuf, String)>, Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    let mut stack = vec![root];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                let text = fs::read_to_string(&path)?;
                sources.push((path, text));
            }
        }
    }
    assert!(
        sources.len() >= 8,
        "the source scan found only {} files",
        sources.len()
    );
    Ok(sources)
}

// ---------------------------------------------------------------------------
// deletion_plan_enumerates_every_derivative_class
// ---------------------------------------------------------------------------

/// A resolver that answers exactly what a test tells it to.
#[derive(Debug, Default)]
struct ScriptedResolver {
    answers: HashMap<&'static str, ClassResolution>,
}

impl ScriptedResolver {
    fn with(mut self, class: DerivativeClass, resolution: ClassResolution) -> Self {
        self.answers.insert(class.as_str(), resolution);
        self
    }
}

impl DerivativeResolver for ScriptedResolver {
    fn resolve(&self, class: DerivativeClass, _subject: &RetentionSubject) -> ClassResolution {
        self.answers
            .get(class.as_str())
            .cloned()
            .unwrap_or(ClassResolution::NothingToDelete {
                reason: "the scripted resolver was given nothing for this class".to_owned(),
            })
    }
}

/// Every class appears in every plan, in registry order, whether or not it
/// holds anything — and the seven classes are exactly t068 section 5's.
#[test]
fn deletion_plan_enumerates_every_derivative_class() -> TestResult {
    assert_eq!(
        DERIVATIVE_CLASSES
            .iter()
            .map(|class| class.as_str())
            .collect::<Vec<_>>(),
        vec![
            "TRANSCRIPT",
            "EMBEDDING",
            "GRAPH_CLAIM",
            "DOCUMENT",
            "CACHE",
            "REPLICA",
            "BACKUP_EXPIRY",
        ],
        "the derivative class registry is not t068 section 5's list"
    );

    // A subject whose transcript exists, whose embedding does not, and whose
    // other classes were never scripted. Every one still gets a node.
    let resolver = ScriptedResolver::default()
        .with(
            DerivativeClass::Transcript,
            ClassResolution::Locators(vec![[0x10; 32], [0x11; 32]]),
        )
        .with(
            DerivativeClass::Embedding,
            ClassResolution::NothingToDelete {
                reason: "no embedding was ever computed for this subject".to_owned(),
            },
        );
    let plan = DeletionPlan::build(RetentionSubject::whole_object([0x01; 32]), &resolver);

    assert_eq!(
        plan.enumerated_classes(),
        DERIVATIVE_CLASSES.to_vec(),
        "the plan did not enumerate every class in registry order"
    );
    assert_eq!(plan.nodes().len(), DERIVATIVE_CLASSES.len());

    // The classes that hold something contribute actions with the right kind.
    let actions = plan.actions();
    assert_eq!(actions.len(), 2);
    for action in &actions {
        assert_eq!(action.class, DerivativeClass::Transcript);
        assert_eq!(action.kind, DerivativeClass::Transcript.action_kind());
    }

    // A class that holds nothing says why, rather than being absent.
    let reasons = academic_retention::execute::empty_class_reasons(&plan);
    assert!(
        reasons
            .iter()
            .any(|(class, reason)| *class == "EMBEDDING" && !reason.is_empty()),
        "an empty class did not state its reason: {reasons:?}"
    );
    assert_eq!(
        reasons.len(),
        DERIVATIVE_CLASSES.len() - 1,
        "the classes that hold nothing were not all reported"
    );

    // `RB03`: a class the resolver cannot answer for is not an empty class. The
    // deletion refuses to complete and the node is named.
    let blind = ScriptedResolver::default().with(
        DerivativeClass::Replica,
        ClassResolution::Unresolved {
            reason: "the replica index is offline".to_owned(),
        },
    );
    let blind_plan = DeletionPlan::build(RetentionSubject::whole_object([0x01; 32]), &blind);
    assert_eq!(blind_plan.enumerated_classes(), DERIVATIVE_CLASSES.to_vec());

    let root = TestRoot::new("plan")?;
    let mut journal = open_journal(root.path())?;
    let mut executor = RefusingExecutor::default();
    let outcome = settle(
        &mut journal,
        ActionId::from_bytes([0x01; 16]),
        &blind_plan,
        &mut executor,
    )?;
    assert_eq!(outcome.as_str(), "REPAIR_REQUIRED");
    assert_eq!(outcome.unresolved().len(), 1);
    assert_eq!(outcome.unresolved()[0].class, DerivativeClass::Replica);
    assert_eq!(
        outcome.unresolved()[0].reason,
        UnresolvedReason::NotResolved
    );
    assert!(
        outcome.unresolved()[0].detail.contains("replica index"),
        "the unresolved node did not carry the resolver's reason"
    );
    assert_eq!(
        executor.executed, 0,
        "an unresolved class did not stop the deletion before it ran"
    );

    // The vocabulary is exactly four words.
    assert_eq!(
        academic_retention::RETENTION_OUTCOMES,
        &["PLANNED", "COMPLETE", "PARTIAL", "REPAIR_REQUIRED"]
    );
    assert_eq!(RetentionOutcome::Planned.as_str(), "PLANNED");
    assert_eq!(RetentionOutcome::Complete.as_str(), "COMPLETE");
    Ok(())
}

#[derive(Debug, Default)]
struct RefusingExecutor {
    executed: usize,
}

impl RetentionExecutor for RefusingExecutor {
    fn execute(
        &mut self,
        _journal: &mut AppendOnlyJournal,
        _action: &PlannedAction,
    ) -> Result<(), ExecutionFailure> {
        self.executed += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// partial_purge_reports_exact_remaining_locators
// ---------------------------------------------------------------------------

/// An executor that fails for a named set of locators and succeeds otherwise.
#[derive(Debug)]
struct PartialExecutor {
    failing: Vec<[u8; 32]>,
    succeeded: Vec<String>,
}

impl RetentionExecutor for PartialExecutor {
    fn execute(
        &mut self,
        _journal: &mut AppendOnlyJournal,
        action: &PlannedAction,
    ) -> Result<(), ExecutionFailure> {
        if self.failing.contains(&action.locator) {
            return Err(ExecutionFailure {
                reason: UnresolvedReason::PurgeFailed,
                detail: "the replica host refused the purge".to_owned(),
            });
        }
        self.succeeded.push(action.locator_hex());
        Ok(())
    }
}

/// `RB04`. A partial cache or replica purge is `PARTIAL` and names the exact
/// locators that are still there — never a count, never "mostly".
#[test]
fn partial_purge_reports_exact_remaining_locators() -> TestResult {
    let root = TestRoot::new("partial")?;
    let mut journal = open_journal(root.path())?;

    let cache = [[0x21_u8; 32], [0x22; 32], [0x23; 32]];
    let replica = [[0x31_u8; 32], [0x32; 32]];
    let resolver = ScriptedResolver::default()
        .with(
            DerivativeClass::Cache,
            ClassResolution::Locators(cache.to_vec()),
        )
        .with(
            DerivativeClass::Replica,
            ClassResolution::Locators(replica.to_vec()),
        );
    let plan = DeletionPlan::build(RetentionSubject::whole_object([0x01; 32]), &resolver);

    let mut executor = PartialExecutor {
        failing: vec![cache[1], replica[0]],
        succeeded: Vec::new(),
    };
    let outcome = settle(
        &mut journal,
        ActionId::from_bytes([0x02; 16]),
        &plan,
        &mut executor,
    )?;

    assert_eq!(outcome.as_str(), "PARTIAL");
    let remaining: Vec<String> = outcome
        .unresolved()
        .iter()
        .map(|row| row.locator.clone())
        .collect();
    assert_eq!(
        remaining,
        vec![hex::encode(cache[1]), hex::encode(replica[0])],
        "the partial result did not name the exact remaining locators"
    );
    for row in outcome.unresolved() {
        assert_eq!(row.reason, UnresolvedReason::PurgeFailed);
        assert!(row.detail.contains("refused the purge"));
    }
    // Everything that did succeed is not in the remaining list.
    assert_eq!(
        executor.succeeded,
        vec![
            hex::encode(cache[0]),
            hex::encode(cache[2]),
            hex::encode(replica[1]),
        ]
    );

    // The journal carries the same exact list, so the report and the record
    // cannot drift.
    let settled = journal
        .entries()
        .find_map(|entry| match entry {
            JournalEntry::RetentionSettled {
                outcome,
                unresolved,
                ..
            } => Some((outcome.clone(), unresolved.clone())),
            _ => None,
        })
        .ok_or("the journal recorded no settlement")?;
    assert_eq!(settled.0, "PARTIAL");
    assert_eq!(settled.1.len(), 2);
    for (row, locator) in settled.1.iter().zip([cache[1], replica[0]]) {
        assert!(
            row.contains(&hex::encode(locator)),
            "the journal row {row} does not name {}",
            hex::encode(locator)
        );
    }

    // A `COMPLETE` result is only reachable when nothing is left: the same plan
    // with an executor that refuses nothing settles complete and names nothing.
    let mut clean = PartialExecutor {
        failing: Vec::new(),
        succeeded: Vec::new(),
    };
    let complete = settle(
        &mut journal,
        ActionId::from_bytes([0x03; 16]),
        &plan,
        &mut clean,
    )?;
    assert_eq!(complete.as_str(), "COMPLETE");
    assert!(complete.unresolved().is_empty());

    // And `PARTIAL` cannot exist without a list: the set constructor refuses an
    // empty one, so there is no way to build "mostly deleted".
    assert!(academic_retention::UnresolvedSet::new(Vec::new()).is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// GATE-38-026: mechanism only, no policy
// ---------------------------------------------------------------------------

/// The mechanism for removing non-instructor voices from an original exists;
/// the policy that would decide to use it does not.
#[test]
fn gate_38_026_ships_the_mechanism_and_selects_no_policy() -> TestResult {
    // The mechanism: a caller can name spans inside an original, and must state
    // who authorized it. There is no constructor that omits the authority.
    let subject = RetentionSubject::voice_spans_in_original(
        [0x44; 32],
        OriginalVoiceAuthority::new("instructor:synthetic-0001".to_owned(), "0".repeat(64)),
        vec![VoiceSpan {
            start_ms: 1_000,
            end_ms: 2_500,
        }],
    );
    assert_eq!(
        subject.open_gate_statement(),
        Some(GATE_38_026_STATEMENT),
        "a voice-scoped subject did not carry the open gate statement"
    );
    assert!(GATE_38_026_STATEMENT.contains("GATE-38-026"));
    assert!(GATE_38_026_STATEMENT.contains("open user decision"));
    assert!(GATE_38_026_STATEMENT.contains("selects no policy"));

    // A whole-object subject is not a voice decision and carries no such claim.
    assert_eq!(
        RetentionSubject::whole_object([0x44; 32]).open_gate_statement(),
        None
    );

    // The policy: nothing in this crate picks one. A `Default`, a named default
    // constant, or a compiled-in authority would each be the silent default the
    // plan forbids.
    for (path, text) in read_crate_sources()? {
        for forbidden in [
            "impl Default for OriginalVoiceAuthority",
            "DEFAULT_ORIGINAL_VOICE_AUTHORITY",
            "DEFAULT_VOICE_REDACTION",
            "ORIGINAL_VOICE_DELETION_ALLOWED",
            "REMOVE_STUDENT_VOICES",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} decides GATE-38-026 through {forbidden}",
                path.display()
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The journal is append-only, and that is checkable
// ---------------------------------------------------------------------------

/// A rewritten, removed, or reordered line is refused rather than replayed.
#[test]
fn rotation_journal_is_append_only_and_says_so_when_it_is_not() -> TestResult {
    let root = TestRoot::new("append-only")?;
    let path = root.path().join(ROTATION_JOURNAL_RELATIVE_PATH);
    {
        let mut journal = AppendOnlyJournal::open(&path)?;
        for index in 0..4_u8 {
            journal.append(JournalEntry::RecipientAdded {
                recipient_id: hex::encode([index; 16]),
                recipient_kind: "DEVICE_KEYSTORE".to_owned(),
                generation: hex::encode([index; 32]),
            })?;
        }
        assert_eq!(journal.records().len(), 4);
    }

    let original = fs::read_to_string(&path)?;
    let lines: Vec<&str> = original.lines().collect();

    // Removing a line breaks the chain.
    fs::write(&path, format!("{}\n{}\n", lines[0], lines[2]))?;
    let removed = AppendOnlyJournal::open(&path)
        .err()
        .ok_or("a journal with a removed line was accepted")?
        .to_string();
    assert!(
        removed.contains("not append-only"),
        "the refusal did not name the append-only violation: {removed}"
    );

    // Reordering two lines breaks the chain.
    fs::write(
        &path,
        format!("{}\n{}\n{}\n{}\n", lines[0], lines[2], lines[1], lines[3]),
    )?;
    assert!(AppendOnlyJournal::open(&path).is_err());

    // Rewriting a line's payload breaks its own digest.
    let tampered = lines[1].replace("DEVICE_KEYSTORE", "RECOVERY_SECRET");
    assert_ne!(tampered, lines[1]);
    fs::write(
        &path,
        format!("{}\n{}\n{}\n{}\n", lines[0], tampered, lines[2], lines[3]),
    )?;
    assert!(AppendOnlyJournal::open(&path).is_err());

    // Restoring the exact bytes makes it readable again, so the check is about
    // the chain and not about the file being touched.
    fs::write(&path, &original)?;
    assert_eq!(AppendOnlyJournal::open(&path)?.records().len(), 4);
    Ok(())
}

// ---------------------------------------------------------------------------
// T111 P2-3
// ---------------------------------------------------------------------------

/// A removed tail is detected, and a torn tail is repaired rather than fatal.
///
/// `T111`'s reproduction: a backward-linked chain cannot see its own tail being
/// cut off, so dropping `UnitMigrated` and `RotationCompleted` left a journal
/// that opened cleanly and replayed the unit's reachability back to the source.
/// The head file closes that. Its counterpart is the torn final line: a
/// fragment with no newline was `Malformed` and blocked the resume the rotation
/// contract promises, and it is now dropped, because the writer syncs a whole
/// line before it returns and a fragment was therefore never a record.
#[test]
fn a_removed_journal_tail_is_refused_and_a_torn_one_resumes() -> TestResult {
    let root = TestRoot::new("journal-tail")?;
    let path = root.path().join(ROTATION_JOURNAL_RELATIVE_PATH);
    let master = VaultMasterKey::generate()?;
    let generation = KeyGeneration::of(&master, profile())?;
    let target = VaultMasterKey::generate()?;
    let target_generation = KeyGeneration::of(&target, profile())?;
    let unit = RotationUnit::object([0x01; 32]);
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x78; 16]),
        profile(),
        generation,
        target_generation,
        vec![unit.clone()],
    )?;

    let mut journal = open_journal(root.path())?;
    journal.append(plan.started_entry())?;
    journal.append(JournalEntry::UnitResealed {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: unit.unit_id_hex(),
        target_locator: hex::encode([0xAA; 32]),
    })?;
    journal.append(JournalEntry::UnitMigrated {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: unit.unit_id_hex(),
    })?;
    journal.append(JournalEntry::RotationCompleted {
        rotation_id: plan.rotation_id().to_hex(),
        unit_count: 1,
    })?;
    drop(journal);
    let complete = fs::read_to_string(&path)?;
    let lines: Vec<&str> = complete.lines().collect();
    assert_eq!(lines.len(), 4);

    // The head file names the last record, beside the journal.
    let head = fs::read_to_string(journal::head_path(&path))?;
    assert!(
        head.contains("\"record_count\":4"),
        "the head file is {head}"
    );

    // Cutting the tail off: the remaining prefix still chains, and that is
    // exactly why the chain alone could not see it.
    fs::write(&path, format!("{}\n{}\n", lines[0], lines[1]))?;
    let refused = AppendOnlyJournal::open(&path)
        .err()
        .ok_or("a journal with its tail removed was accepted")?
        .to_string();
    assert!(
        refused.contains("the tail was removed"),
        "the refusal did not name the removed tail: {refused}"
    );

    // Restoring the exact bytes makes it readable again, so the check is about
    // the missing records and not about the file having been touched.
    fs::write(&path, &complete)?;
    let restored = AppendOnlyJournal::open(&path)?;
    assert_eq!(restored.records().len(), 4);
    drop(restored);

    // A torn final line — the state a kill during `append` leaves — is dropped
    // and truncated away, so the rotation resumes instead of stopping.
    let torn = format!("{}\n{}\n{}", lines[0], lines[1], &lines[2][..20]);
    fs::write(&path, &torn)?;
    fs::write(
        journal::head_path(&path),
        format!(
            "{{\"journal_version\":1,\"record_count\":2,\"head_digest\":\"{}\"}}\n",
            serde_json::from_str::<serde_json::Value>(lines[1])
                .ok()
                .and_then(|value| value
                    .get("entry_digest")
                    .and_then(|digest| digest.as_str().map(str::to_owned)))
                .ok_or("the second record has no digest")?
        ),
    )?;
    let mut resumed = AppendOnlyJournal::open(&path)?;
    assert_eq!(
        resumed.records().len(),
        2,
        "the torn fragment was counted as a record"
    );
    assert_eq!(
        fs::read(&path)?.len(),
        format!("{}\n{}\n", lines[0], lines[1]).len(),
        "the torn fragment was left in the file for the next append to extend"
    );
    resumed.append(JournalEntry::UnitMigrated {
        rotation_id: plan.rotation_id().to_hex(),
        unit_id: unit.unit_id_hex(),
    })?;
    drop(resumed);
    assert_eq!(AppendOnlyJournal::open(&path)?.records().len(), 3);
    Ok(())
}

/// Nothing in this crate opens a journal for anything but appending.
///
/// `set_len` has exactly one reviewed call site: the torn-tail repair in
/// `journal.rs`, which drops a trailing fragment that has no newline and was
/// therefore never a durable record. That repair is what makes an interrupted
/// rotation resumable, and it is bounded here — one occurrence, in one file —
/// so a second truncation anywhere in the crate fails this test.
#[test]
fn no_source_truncates_or_rewrites_a_journal() -> TestResult {
    let journal_source = Path::new("src").join("journal.rs");
    let mut repairs = 0;
    for (path, text) in read_crate_sources()? {
        for forbidden in [".truncate(true)", "File::create(&path)"] {
            assert!(
                !text.contains(forbidden),
                "{} can shorten or rewrite a journal through {forbidden}",
                path.display()
            );
        }
        let truncations = text.matches("set_len(").count();
        if path.ends_with(&journal_source) {
            repairs += truncations;
            assert!(
                text.contains("truncate torn journal record"),
                "the one reviewed truncation is no longer the torn-tail repair"
            );
        } else {
            assert_eq!(
                truncations,
                0,
                "{} can shorten a journal through set_len",
                path.display()
            );
        }
    }
    assert_eq!(
        repairs, 1,
        "journal.rs holds {repairs} truncations; the torn-tail repair is the only reviewed one"
    );
    Ok(())
}
