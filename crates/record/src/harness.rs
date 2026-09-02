//! The deterministic builder for both engines' §3.9 harness corpora.
//!
//! `docs/contracts/engine-harness.md` fixes what an `IMPLEMENTED` engine must
//! ship: golden fixtures, a property-test bound declaration, version-compat
//! fixtures, an explanation snapshot, and — for a high-impact engine — an
//! `unknown`, a `conflict`, and a `partial_failure` adverse set. `GPA` and
//! `CREDIT_ACCOUNTING` flip to `IMPLEMENTED` with this task, so both corpora
//! become due, and `CONTRIBUTING.md` rule 5 says a golden fixture may only be
//! updated through a deterministic builder. This is that builder.
//!
//! [`corpus_files`] returns every committed path and its exact bytes.
//! `cargo run -p academic-record --example emit_harness` writes them;
//! `harness_corpus_matches_a_fresh_render` re-renders and byte-compares, so a
//! committed fixture cannot be hand-edited into agreement with a broken engine.
//!
//! Every case runs under one rule book, because `ruleset.txt`'s SHA-256 is the
//! `rule_set_hash` the whole directory is evaluated under. The three adverse
//! states are therefore reachable under the *baseline* book rather than under a
//! book chosen to produce them:
//!
//! | path | how the baseline book reaches it |
//! |---|---|
//! | `adverse/unknown` | an exchange attempt in 2003, which no dated external row reaches |
//! | `adverse/conflict` | two settled attempts at one course in one term, neither a repeat |
//! | `adverse/partial_failure` | a term scope the attempt set has nothing in |

use academic_domain::engines::{EngineVersion, FrozenInputs};

use crate::{
    RecordError,
    attempt::AttemptHistory,
    classify::ProgramId,
    corpus,
    engine::{CreditAccountingEngine, GpaEngine},
    facts::{AttemptFacts, GpaScope, encode},
    policy::RuleBook,
    term::{Semester, TermKey},
};

/// The harness root every registered engine's directory lives under.
pub const HARNESS_ROOT: &str = "testdata/engines";
/// The `GPA` engine's harness directory, as the registry names it.
pub const GPA_HARNESS_DIR: &str = "gpa";
/// The `CREDIT_ACCOUNTING` engine's harness directory.
pub const CREDIT_HARNESS_DIR: &str = "credit_accounting";

/// The generator bounds the property test is driven from.
///
/// Declared as data rather than as literals inside the test so the committed
/// artifact and the generator cannot drift: the property test reads these same
/// constants.
pub const CREDIT_TENTHS_LOW: i128 = 5;
/// Upper bound of the property test's credit generator, in tenths.
pub const CREDIT_TENTHS_HIGH: i128 = 60;
/// Largest attempt set the property test generates.
pub const PROPERTY_MAX_ATTEMPTS: usize = 12;

/// One committed harness file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusFile {
    /// Path relative to the repository root.
    pub path: String,
    /// Exact bytes.
    pub bytes: Vec<u8>,
}

/// Encodes one history under one scope.
fn inputs_for(history: &AttemptHistory, scope: &GpaScope) -> Result<FrozenInputs, RecordError> {
    let classification = corpus::classification_v1()?;
    let facts: Vec<AttemptFacts> = history
        .current()
        .into_iter()
        .map(|attempt| AttemptFacts::from_attempt(attempt, &classification))
        .collect();
    encode(&facts, scope)
}

/// The cases the `GPA` harness ships, in committed order.
fn gpa_cases() -> Result<Vec<(String, FrozenInputs)>, RecordError> {
    Ok(vec![
        (
            "golden/cumulative".to_owned(),
            inputs_for(&corpus::baseline_history()?, &GpaScope::Cumulative)?,
        ),
        (
            "golden/term_2015_spring".to_owned(),
            inputs_for(
                &corpus::baseline_history()?,
                &GpaScope::Term(TermKey::new(2015, Semester::Spring)?),
            )?,
        ),
        (
            "golden/major_cse".to_owned(),
            inputs_for(
                &corpus::baseline_history()?,
                &GpaScope::Major(ProgramId::new(corpus::PRIMARY_PROGRAM)?),
            )?,
        ),
        (
            "adverse/unknown/undated_external".to_owned(),
            inputs_for(
                &corpus::history_with_undated_external()?,
                &GpaScope::Cumulative,
            )?,
        ),
        (
            "adverse/conflict/two_records_one_slot".to_owned(),
            inputs_for(
                &corpus::history_with_conflicting_records()?,
                &GpaScope::Cumulative,
            )?,
        ),
        (
            "adverse/partial_failure/term_with_no_attempts".to_owned(),
            inputs_for(
                &corpus::baseline_history()?,
                &GpaScope::Term(TermKey::new(2030, Semester::Spring)?),
            )?,
        ),
        (
            "version-compat/v1-cumulative".to_owned(),
            inputs_for(&corpus::baseline_history()?, &GpaScope::Cumulative)?,
        ),
    ])
}

/// The cases the `CREDIT_ACCOUNTING` harness ships, in committed order.
fn credit_cases() -> Result<Vec<(String, FrozenInputs)>, RecordError> {
    Ok(vec![
        (
            "golden/baseline".to_owned(),
            inputs_for(&corpus::baseline_history()?, &GpaScope::Cumulative)?,
        ),
        (
            "golden/undecided_external".to_owned(),
            inputs_for(
                &corpus::history_with_undated_external()?,
                &GpaScope::Cumulative,
            )?,
        ),
        (
            "version-compat/v1-baseline".to_owned(),
            inputs_for(&corpus::baseline_history()?, &GpaScope::Cumulative)?,
        ),
    ])
}

fn bounds_text() -> String {
    format!(
        "credits.tenths.low={CREDIT_TENTHS_LOW}\n\
         credits.tenths.high={CREDIT_TENTHS_HIGH}\n\
         attempts.max={PROPERTY_MAX_ATTEMPTS}\n\
         grade=every symbol in GradeSymbol::ALL\n\
         origin=every origin in AttemptOrigin::ALL\n\
         status=every status in SettledStatus::ALL\n"
    )
}

/// Builds every committed file of both harness directories.
///
/// The two engines share one rule book, so both `ruleset.txt` files hold the
/// same bytes: an average and a credit total computed under two different books
/// could not be read against each other, and section 10 asks a reader to
/// compare exactly those two.
pub fn corpus_files() -> Result<Vec<CorpusFile>, RecordError> {
    let rules: RuleBook = corpus::baseline_rules()?;
    let gpa = GpaEngine::new(rules.clone(), EngineVersion::MIN);
    let credit = CreditAccountingEngine::new(rules.clone(), EngineVersion::MIN);
    let hash = gpa.rule_set_hash();
    let version = EngineVersion::MIN;

    let mut files = Vec::new();
    let mut push = |path: String, bytes: Vec<u8>| files.push(CorpusFile { path, bytes });

    for directory in [GPA_HARNESS_DIR, CREDIT_HARNESS_DIR] {
        push(
            format!("{HARNESS_ROOT}/{directory}/ruleset.txt"),
            rules.canonical_text().into_bytes(),
        );
        push(
            format!("{HARNESS_ROOT}/{directory}/property/bounds.txt"),
            bounds_text().into_bytes(),
        );
    }

    let mut gpa_snapshot = None;
    for (case, inputs) in gpa_cases()? {
        let outcome = gpa.evaluate_record(&inputs, hash)?;
        push(
            format!("{HARNESS_ROOT}/{GPA_HARNESS_DIR}/{case}.input"),
            inputs.canonical_text().into_bytes(),
        );
        if case.starts_with("version-compat/") {
            push(
                format!("{HARNESS_ROOT}/{GPA_HARNESS_DIR}/{case}.explanation"),
                outcome.explanation_snapshot.as_str().as_bytes().to_vec(),
            );
        } else {
            push(
                format!("{HARNESS_ROOT}/{GPA_HARNESS_DIR}/{case}.expected"),
                outcome.canonical_bytes(crate::engine::GPA_ENGINE_ID, hash, version, &inputs),
            );
        }
        if case == "golden/cumulative" {
            gpa_snapshot = Some(outcome.explanation_snapshot.as_str().as_bytes().to_vec());
        }
    }
    push(
        format!("{HARNESS_ROOT}/{GPA_HARNESS_DIR}/explanation.snapshot"),
        gpa_snapshot.ok_or(RecordError::DispositionMissing)?,
    );

    let mut credit_snapshot = None;
    for (case, inputs) in credit_cases()? {
        let outcome = credit.evaluate_record(&inputs, hash)?;
        push(
            format!("{HARNESS_ROOT}/{CREDIT_HARNESS_DIR}/{case}.input"),
            inputs.canonical_text().into_bytes(),
        );
        if case.starts_with("version-compat/") {
            push(
                format!("{HARNESS_ROOT}/{CREDIT_HARNESS_DIR}/{case}.explanation"),
                outcome.explanation_snapshot.as_str().as_bytes().to_vec(),
            );
        } else {
            push(
                format!("{HARNESS_ROOT}/{CREDIT_HARNESS_DIR}/{case}.expected"),
                outcome.canonical_bytes(crate::engine::CREDIT_ENGINE_ID, hash, version, &inputs),
            );
        }
        if case == "golden/baseline" {
            credit_snapshot = Some(outcome.explanation_snapshot.as_str().as_bytes().to_vec());
        }
    }
    push(
        format!("{HARNESS_ROOT}/{CREDIT_HARNESS_DIR}/explanation.snapshot"),
        credit_snapshot.ok_or(RecordError::DispositionMissing)?,
    );

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}
