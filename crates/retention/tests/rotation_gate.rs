//! Phase 2 has not accepted a key rotation, and this is what refuses one.
//!
//! The machinery is built, and every row that exercises it is behind the
//! non-default `rotation-orchestration` feature — `rotation_seam.rs`, four rows
//! of `rotation.rs`, `KY03` in `rotation_faults.rs`, `KY05` in `retention.rs`,
//! and the encrypted portability suite's rotation rows. This file is what runs
//! in its place: the same seven entry points, called the same way, refusing.
//!
//! The refusals matter more than the count. The fourth `P2-A1` audit reached
//! four states through the shipped API with no kill and no tampering — a
//! deletion landing inside an open rotation that then cannot finish and whose
//! profile no backup restores, a unit recorded as migrated for an object that
//! was never resealed, a database unit run before the objects, and a second
//! `begin` that makes the journal permanently unreplayable. Each one needs a
//! journalled rotation to exist first, so
//! `the_states_the_fourth_audit_reached_are_behind_the_first_call` walks those
//! sequences and stops at the first call — which is why they are out of reach
//! rather than merely unwitnessed.

use std::error::Error;

#[cfg(not(feature = "rotation-orchestration"))]
use academic_retention::{AppendOnlyJournal, journal::ROTATION_JOURNAL_RELATIVE_PATH, recipients};
use academic_retention::{
    RotationId, RotationPlan, RotationUnit, rotation::ROTATION_NOT_ACCEPTED_STATEMENT,
};

#[cfg(feature = "rotation-engine")]
mod rotation_support;

type TestResult = Result<(), Box<dyn Error>>;

/// The one place the crate decides whether a rotation is accepted.
const GATE_SOURCE: &str = "src/rotation.rs";

/// The gate, whitespace-collapsed. Nothing else may be in it.
const WHOLE_GATE: &str = concat!(
    "pub fn require_rotation_accepted() -> Result<(), RotationNotAccepted> { ",
    "if cfg!(feature = \"rotation-orchestration\") { return Ok(()); } ",
    "Err(RotationNotAccepted) }"
);

/// Every entry point the gate closes, with the file it lives in.
const GATED_ENTRY_POINTS: [(&str, &str); 7] = [
    ("src/engine.rs", "pub fn begin("),
    ("src/engine.rs", "pub fn rotate_object("),
    ("src/engine.rs", "pub fn rotate_store_database("),
    ("src/engine.rs", "pub fn complete("),
    ("src/engine.rs", "pub fn retire_superseded_object("),
    ("src/recipients.rs", "pub fn rewrap_for_generation<F>("),
    ("src/recipients.rs", "pub fn retire_generation<F>("),
];

fn crate_source(relative: &str) -> Result<String, Box<dyn Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    Ok(std::fs::read_to_string(path)?)
}

// ---------------------------------------------------------------------------
// The shape of the gate: one decision, no way around it
// ---------------------------------------------------------------------------

/// Every gated entry point calls the gate, and the gate is decided once.
///
/// A refusal spread over seven copies of a condition is seven chances to get one
/// of them wrong, and a refusal that reads an environment variable, a build
/// profile, or an argument is not a refusal — it is a default. `t068` section
/// 3.1 says it in those words: no quiet flag, no environment variable, no debug
/// build shortcut. So this reads the sources: the decision appears exactly once
/// in the crate, it is the feature and nothing else, and each entry point calls
/// it before it can do anything.
#[test]
fn the_rotation_gate_is_one_decision_with_no_flag_variable_or_debug_path() -> TestResult {
    let gate = crate_source(GATE_SOURCE)?;
    assert_eq!(
        gate.matches("cfg!(feature = \"rotation-orchestration\")")
            .count(),
        1,
        "the gate is decided in more than one place"
    );

    let body = gate
        .split_once("pub fn require_rotation_accepted()")
        .and_then(|(_, rest)| rest.split_once("\n}"))
        .map(|(body, _)| body.to_owned())
        .ok_or("require_rotation_accepted is not in the gate source")?;
    for forbidden in [
        "env",
        "var(",
        "debug_assertions",
        "test)",
        "cfg(feature",
        "argument",
    ] {
        assert!(
            !body.contains(forbidden),
            "the gate reads {forbidden}, so something other than the build selects it"
        );
    }
    assert!(
        gate.contains("pub fn require_rotation_accepted() -> Result<(), RotationNotAccepted>"),
        "the gate takes an argument, so a caller can ask to be let through"
    );

    // The whole decision, spelled out. A forbidden-token list cannot see the
    // two shapes the fifth `P2-A1` audit built and neither guard caught: an
    // environment read moved into a helper the gate calls, and a process-wide
    // `AtomicBool` with a public setter. Both leave the body free of every
    // token above. The gate is four lines, so the check that holds is that it
    // is *these* four and nothing else.
    let declared = gate
        .split_once("pub fn require_rotation_accepted")
        .and_then(|(_, rest)| rest.split_once("\n}"))
        .map(|(body, _)| format!("pub fn require_rotation_accepted{body}\n}}"))
        .ok_or("require_rotation_accepted is not in the gate source")?;
    assert_eq!(
        declared.split_whitespace().collect::<Vec<_>>().join(" "),
        WHOLE_GATE,
        "the gate decides on something other than the build feature"
    );

    for (file, signature) in GATED_ENTRY_POINTS {
        let source = crate_source(file)?;
        let (_, after) = source
            .split_once(signature)
            .ok_or_else(|| format!("{file} no longer holds {signature}"))?;
        let body = after
            .split_once('{')
            .map(|(_, body)| body)
            .ok_or("an entry point has no body")?;
        let first = body
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .ok_or("an entry point has an empty body")?;
        assert_eq!(
            first, "require_rotation_accepted()?;",
            "{signature} does something before it is refused"
        );
    }

    // No source in the crate turns the feature on for a caller.
    for file in ["src/engine.rs", "src/recipients.rs", "src/rotation.rs"] {
        let source = crate_source(file)?;
        assert!(
            !source.contains("#[cfg(feature = \"rotation-orchestration\")]"),
            "{file} compiles a second, ungated path"
        );
    }
    Ok(())
}

/// The refusal says what is refused and what is not, in one sentence.
#[test]
fn the_refusal_names_what_still_works() -> TestResult {
    for phrase in [
        "phase 2 has not accepted a key rotation",
        "Crypto-shredding, backup tombstones, and their re-application on restore",
        "keep working",
    ] {
        assert!(
            ROTATION_NOT_ACCEPTED_STATEMENT.contains(phrase),
            "the refusal no longer says {phrase:?}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The recipient half, in the default graph
// ---------------------------------------------------------------------------

/// The two recipient operations a rotation owns are refused.
///
/// They are refused on the first line, so neither reads the recipient set and
/// neither needs one to exist: what they are handed cannot make them proceed.
#[cfg(not(feature = "rotation-orchestration"))]
#[test]
fn the_recipient_half_of_a_rotation_is_refused() -> TestResult {
    // The name carries this process, because the next line removes whatever is
    // at it. A fixed name is the same path in every process on the machine, so
    // two lanes running this suite at once would delete each other's journal
    // mid-test and read the resulting `NotFound` as the gate's refusal.
    let root = std::env::temp_dir().join(format!(
        "academic-rotation-gate-recipients-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let profile = academic_crypto::ProfileId::from_bytes([0x5A; academic_crypto::IDENTIFIER_BYTES]);
    let generation = academic_retention::rotation::KeyGeneration::parse(&"ab".repeat(32))?;
    let mut journal = AppendOnlyJournal::open(&root.join(ROTATION_JOURNAL_RELATIVE_PATH))?;

    let rewrapped = recipients::rewrap_for_generation(&root, profile, &mut journal, generation, {
        |record| Ok(record.clone())
    });
    assert!(
        matches!(rewrapped, Err(recipients::RecipientError::NotAccepted(_))),
        "a rewrap for a new generation was not refused: {rewrapped:?}"
    );

    let retired = recipients::retire_generation(&root, profile, &journal, generation, |_| true);
    assert!(
        matches!(retired, Err(recipients::RecipientError::NotAccepted(_))),
        "retiring a generation was not refused: {retired:?}"
    );

    // Neither read the recipient set, which is how far in they got: the set
    // this profile root does not have would have failed them by another name.
    assert!(!root.join(recipients::RECIPIENTS_RELATIVE_PATH).exists());
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// Adding and revoking a recipient are not part of a rotation and still run.
///
/// The gate is drawn around rotation, not around key management: a revocation
/// records that a recipient receives no further key, which is a fact whether or
/// not a rotation ever follows. What it cannot do in Phase 2 is *be followed by
/// one*, and `REVOCATION_SCOPE_STATEMENT` already says revocation does not
/// reach a key someone already holds.
#[cfg(not(feature = "rotation-orchestration"))]
#[test]
fn adding_and_revoking_a_recipient_are_outside_the_gate() -> TestResult {
    let source = crate_source("src/recipients.rs")?;
    for signature in ["pub fn add_recipient(", "pub fn revoke_recipient("] {
        let (_, after) = source
            .split_once(signature)
            .ok_or_else(|| format!("recipients.rs no longer holds {signature}"))?;
        let body = after.split_once('{').map(|(_, body)| body).unwrap_or("");
        let head: String = body.lines().take(6).collect();
        assert!(
            !head.contains("require_rotation_accepted"),
            "{signature} was pulled inside the rotation gate"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The engine half
// ---------------------------------------------------------------------------

#[cfg(all(feature = "rotation-engine", not(feature = "rotation-orchestration")))]
mod engine_gate {
    use super::{TestResult, rotation_support};
    use academic_domain::ArtifactId;
    use academic_retention::{
        AppendOnlyJournal, RotationId, RotationPlan, RotationUnit,
        engine::{EngineError, RotationEngine, retire_superseded_object},
        journal::ROTATION_JOURNAL_RELATIVE_PATH,
        rotation::{
            CanonicalReference, CanonicalReferenceError, KeyGeneration, StoreDatabaseError,
            StoreDatabaseExecutor, StoreDatabaseRekey,
        },
    };
    use rotation_support::{
        SOURCE_ENTROPY, SOURCE_RECIPIENT, TARGET_ENTROPY, TARGET_RECIPIENT, TestRoot,
        create_generation, generation_of, open_vault, profile_id, seal_corpus,
    };

    /// A reference that would answer, if anything ever asked it.
    struct StatedReference([u8; 32]);

    impl CanonicalReference for StatedReference {
        fn resolved_locator(
            &self,
            _artifact: ArtifactId,
        ) -> Result<Option<[u8; 32]>, CanonicalReferenceError> {
            Ok(Some(self.0))
        }
    }

    /// An executor that would rekey the database, if anything ever called it.
    struct WouldRekey {
        source: KeyGeneration,
        target: KeyGeneration,
        called: std::cell::Cell<bool>,
    }

    impl StoreDatabaseExecutor for WouldRekey {
        fn generations(&self) -> Result<(KeyGeneration, KeyGeneration), StoreDatabaseError> {
            Ok((self.source, self.target))
        }

        fn rekey_store_database(&self) -> Result<StoreDatabaseRekey, StoreDatabaseError> {
            self.called.set(true);
            Ok(StoreDatabaseRekey::Rekeyed)
        }
    }

    /// Every engine entry point refuses, and the journal stays empty.
    ///
    /// An empty journal is the whole point: a record for a rotation that was
    /// refused would be a rotation that had started, and the append-only journal
    /// cannot take it back.
    #[test]
    fn every_rotation_engine_entry_point_is_refused_and_nothing_is_journalled() -> TestResult {
        let root = TestRoot::new("gate-engine")?;
        let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
        let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
        let source_vault = open_vault(root.path(), &source_master)?;
        let target_vault = open_vault(root.path(), &target_master)?;
        let descriptors = seal_corpus(&source_vault, 2)?;

        let units: Vec<RotationUnit> = descriptors
            .iter()
            .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
            .collect();
        let database = RotationUnit::store_database(profile_id());
        let mut planned = units.clone();
        planned.push(database.clone());
        let plan = RotationPlan::new(
            RotationId::from_bytes([0x5C; 16]),
            profile_id(),
            generation_of(&source_master)?,
            generation_of(&target_master)?,
            planned,
        )?;

        let journal_path = root.path().join(ROTATION_JOURNAL_RELATIVE_PATH);
        let mut journal = AppendOnlyJournal::open(&journal_path)?;
        let engine = RotationEngine::new(&plan, &source_vault, &target_vault);

        let begun = engine.begin(&mut journal);
        assert!(
            matches!(begun, Err(EngineError::NotAccepted(_))),
            "a rotation was begun: {begun:?}"
        );

        let moved = engine.rotate_object(&mut journal, &units[0], &descriptors[0]);
        assert!(
            matches!(moved, Err(EngineError::NotAccepted(_))),
            "an object unit moved: {moved:?}"
        );

        let executor = WouldRekey {
            source: generation_of(&source_master)?,
            target: generation_of(&target_master)?,
            called: std::cell::Cell::new(false),
        };
        let rekeyed = engine.rotate_store_database(&mut journal, &database, &executor);
        assert!(
            matches!(rekeyed, Err(EngineError::NotAccepted(_))),
            "the store database unit ran: {rekeyed:?}"
        );
        assert!(
            !executor.called.get(),
            "the refusal came after the executor rewrote the database"
        );

        let completed = engine.complete(&mut journal);
        assert!(
            matches!(completed, Err(EngineError::NotAccepted(_))),
            "a rotation completed: {completed:?}"
        );

        let retired = retire_superseded_object(
            &mut journal,
            &source_vault,
            &units[0],
            &descriptors[0],
            &StatedReference([0x99; 32]),
        );
        assert!(
            matches!(retired, Err(EngineError::NotAccepted(_))),
            "a superseded object was retired: {retired:?}"
        );

        assert!(
            journal.entries().next().is_none(),
            "a refused rotation left records behind"
        );
        Ok(())
    }

    /// The four states the fourth audit reached all need a begun rotation.
    ///
    /// Each sequence is the audit's own, run here as far as it gets. `P1-F2` is
    /// begin, then a deletion, then a resume; `P2-F3` is a unit moved under
    /// another artifact's descriptor; `P3-F5` is the database unit run before
    /// the objects; `P3-F6` is a second `begin` over an open rotation. All four
    /// stop at the first call, and the deletion in the middle of the first one
    /// still works — which is the boundary this task draws.
    #[test]
    fn the_states_the_fourth_audit_reached_are_behind_the_first_call() -> TestResult {
        let root = TestRoot::new("gate-unreachable")?;
        let (source_master, _) = create_generation(SOURCE_RECIPIENT, SOURCE_ENTROPY)?;
        let (target_master, _) = create_generation(TARGET_RECIPIENT, TARGET_ENTROPY)?;
        let vault = open_vault(root.path(), &source_master)?;
        let target_vault = open_vault(root.path(), &target_master)?;
        let descriptors = seal_corpus(&vault, 2)?;
        let units: Vec<RotationUnit> = descriptors
            .iter()
            .map(|descriptor| RotationUnit::object(*descriptor.vault_locator.as_bytes()))
            .collect();
        let database = RotationUnit::store_database(profile_id());
        let plan = RotationPlan::new(
            RotationId::from_bytes([0x5C; 16]),
            profile_id(),
            generation_of(&source_master)?,
            generation_of(&target_master)?,
            vec![units[0].clone(), units[1].clone(), database.clone()],
        )?;
        let mut journal =
            AppendOnlyJournal::open(&root.path().join(ROTATION_JOURNAL_RELATIVE_PATH))?;
        let engine = RotationEngine::new(&plan, &vault, &target_vault);

        // P1-F2: a deletion inside an open rotation. There is no open rotation.
        assert!(matches!(
            engine.begin(&mut journal),
            Err(EngineError::NotAccepted(_))
        ));
        let stone = academic_retention::BackupTombstone::new(
            hex::encode([0x71_u8; 16]),
            descriptors[1].id,
            *descriptors[1].vault_locator.as_bytes(),
            1_700_000_000_071,
        );
        academic_retention::engine::shred_with_tombstone(
            &mut journal,
            &vault,
            &descriptors[1],
            &stone,
        )?;
        assert!(matches!(
            engine.rotate_object(&mut journal, &units[1], &descriptors[1]),
            Err(EngineError::NotAccepted(_))
        ));

        // P2-F3: a unit moved under another artifact's descriptor.
        assert!(matches!(
            engine.rotate_object(&mut journal, &units[0], &descriptors[1]),
            Err(EngineError::NotAccepted(_))
        ));

        // P3-F5: the database unit before the objects.
        let executor = WouldRekey {
            source: generation_of(&source_master)?,
            target: generation_of(&target_master)?,
            called: std::cell::Cell::new(false),
        };
        assert!(matches!(
            engine.rotate_store_database(&mut journal, &database, &executor),
            Err(EngineError::NotAccepted(_))
        ));

        // P3-F6: a second begin over an open rotation.
        assert!(matches!(
            engine.begin(&mut journal),
            Err(EngineError::NotAccepted(_))
        ));

        // The deletion is the one thing in that sequence that happened, and the
        // journal holds exactly its record.
        let records: Vec<_> = journal.entries().collect();
        assert_eq!(
            records.len(),
            1,
            "the journal holds something other than the deletion: {records:?}"
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Keeping the two lanes apart
// ---------------------------------------------------------------------------

/// The rows that execute a rotation and the rows that refuse one never link
/// into one binary.
///
/// Same shape as the plaintext and encrypted vault lanes: a suite that could
/// hold both would be a suite in which one of them is not being tested.
#[test]
fn the_executing_rows_and_the_refusing_rows_are_two_lanes() -> TestResult {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let seam = std::fs::read_to_string(tests.join("rotation_seam.rs"))?;
    assert!(
        seam.contains(
            "#![cfg(all(feature = \"rotation-engine\", feature = \"rotation-orchestration\"))]"
        ),
        "the seam rows are no longer behind the rotation-orchestration lane"
    );
    let here = std::fs::read_to_string(tests.join("rotation_gate.rs"))?;
    assert!(
        here.contains("#[cfg(not(feature = \"rotation-orchestration\"))]")
            && here.contains(
                "#[cfg(all(feature = \"rotation-engine\", not(feature = \"rotation-orchestration\")))]"
            ),
        "the refusing rows are no longer behind the absence of that lane"
    );
    Ok(())
}

/// A plan is data and stays buildable: the refusal is on running one.
#[test]
fn planning_a_rotation_is_not_running_one() -> TestResult {
    let profile = academic_crypto::ProfileId::from_bytes([0x5A; academic_crypto::IDENTIFIER_BYTES]);
    let plan = RotationPlan::new(
        RotationId::from_bytes([0x5C; 16]),
        profile,
        academic_retention::rotation::KeyGeneration::parse(&"ab".repeat(32))?,
        academic_retention::rotation::KeyGeneration::parse(&"cd".repeat(32))?,
        vec![RotationUnit::object([0x11; 32])],
    )?;
    assert_eq!(plan.units().len(), 1);
    Ok(())
}
