//! The deterministic builder for the `GRADUATION_AUDIT` harness corpus.
//!
//! `docs/contracts/engine-harness.md` fixes what an `IMPLEMENTED` engine ships:
//! golden fixtures, a property-test bound declaration, version-compat fixtures,
//! an explanation snapshot, and -- because `GRADUATION_AUDIT` is one of the
//! four high-impact paths -- an `unknown`, a `conflict` and a `partial_failure`
//! adverse set. `GRADUATION_AUDIT` flips to `IMPLEMENTED` with this task, so
//! its corpus becomes due, and `CONTRIBUTING.md` rule 5 says a golden fixture
//! may only be updated through a deterministic builder. This is that builder.
//!
//! `cargo run -p academic-audit --example emit_harness` writes every file;
//! `harness_corpus_matches_a_fresh_render` re-renders and byte-compares, so a
//! committed fixture cannot be hand-edited into agreement with a broken engine.
//!
//! Every case runs under one rule set, because `ruleset.txt`'s SHA-256 is the
//! `rule_set_hash` the whole directory is evaluated under. The three adverse
//! states are therefore reached from the **inputs** rather than from a
//! differently configured engine:
//!
//! | path | how the baseline set reaches it |
//! |---|---|
//! | `adverse/unknown` | the exchange attempt in 2003, which no dated policy row reaches, so `P2-U4` withholds the average and the `GPA_MINIMUM` rule reads `UNKNOWN` |
//! | `adverse/conflict` | two settled attempts at one course in one term, which breaks the `MUTUALLY_EXCLUSIVE` ceiling |
//! | `adverse/partial_failure` | a published rule the source index does not place, which cannot become a leaf and is left unevaluated |
//!
//! That the source placements are frozen inputs is what makes the third one an
//! input file at all. An engine that held them would have made
//! `partial_failure` a second engine, and two engines presenting one
//! `rule_set_hash` would have broken the byte comparison the whole directory
//! rests on.

use std::error::Error;

use academic_domain::engines::EngineVersion;
use academic_requirement::RuleSet;

use academic_audit::{
    AuditFacts, DegreeAudit, GRADUATION_ENGINE_ID, GRADUATION_HARNESS_DIR, GraduationAuditEngine,
    RuleSourceIndex, SourceFreshnessPolicy, TranscriptSnapshot, encode, select,
    verdict::ConflictReference,
};

use super::{
    FRESHNESS, audit_facts, baseline_rules, catalog, profile, sources, sources_missing, transcript,
    transcript_with_conflicting_records, transcript_with_undated_external,
};

/// The harness root every registered engine's directory lives under.
pub const HARNESS_ROOT: &str = "testdata/engines";

/// The generator bounds a property test over this engine would be driven from.
///
/// Declared as data rather than as literals inside a test so the committed
/// artifact and any generator cannot drift.
pub const PROPERTY_MAX_RULES: usize = 16;
/// Largest transcript the declared bounds admit.
pub const PROPERTY_MAX_ATTEMPTS: usize = 24;
/// Largest credit threshold the declared bounds admit.
pub const PROPERTY_MAX_THRESHOLD: u16 = 200;

/// One committed harness file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusFile {
    /// Path relative to the repository root.
    pub path: String,
    /// Exact bytes.
    pub bytes: Vec<u8>,
}

/// The cases the harness ships, in committed order.
fn cases(rules: &RuleSet) -> Result<Vec<(String, AuditFacts)>, Box<dyn Error>> {
    Ok(vec![
        (
            "golden/baseline".to_owned(),
            audit_facts(transcript()?, sources(rules)?, Vec::new(), Some(FRESHNESS))?,
        ),
        (
            "golden/no_freshness_criterion".to_owned(),
            audit_facts(transcript()?, sources(rules)?, Vec::new(), None)?,
        ),
        (
            "golden/unresolved_source_conflict".to_owned(),
            audit_facts(
                transcript()?,
                sources(rules)?,
                vec![super::unresolved_conflict()?],
                Some(FRESHNESS),
            )?,
        ),
        (
            "adverse/unknown/undated_external".to_owned(),
            audit_facts(
                transcript_with_undated_external()?,
                sources(rules)?,
                Vec::new(),
                Some(FRESHNESS),
            )?,
        ),
        (
            "adverse/conflict/two_records_one_slot".to_owned(),
            audit_facts(
                transcript_with_conflicting_records()?,
                sources(rules)?,
                Vec::new(),
                Some(FRESHNESS),
            )?,
        ),
        (
            "adverse/partial_failure/rule_with_no_recorded_page".to_owned(),
            audit_facts(
                transcript()?,
                sources_missing(rules, "total_credits")?,
                Vec::new(),
                Some(FRESHNESS),
            )?,
        ),
        (
            "version-compat/v1-baseline".to_owned(),
            audit_facts(transcript()?, sources(rules)?, Vec::new(), Some(FRESHNESS))?,
        ),
    ])
}

fn bounds_text() -> String {
    format!(
        "rules.max={PROPERTY_MAX_RULES}\n\
         attempts.max={PROPERTY_MAX_ATTEMPTS}\n\
         credit.threshold.max={PROPERTY_MAX_THRESHOLD}\n\
         profile.fields=every field in ProfileField::ALL, recorded or not\n\
         status=every status in ProofStatus::ALL\n\
         gate=every cell in OpenGate::ALL\n"
    )
}

/// The engine every case in the directory is evaluated by.
pub fn engine(rules: &RuleSet) -> Result<GraduationAuditEngine, Box<dyn Error>> {
    let selection = select(&profile()?, &catalog(rules)?);
    let selected = selection
        .selected()
        .ok_or("the corpus profile selects no rule set")?
        .clone();
    Ok(GraduationAuditEngine::new(selected, EngineVersion::MIN))
}

/// Builds every committed file of the harness directory.
pub fn corpus_files() -> Result<Vec<CorpusFile>, Box<dyn Error>> {
    let rules = baseline_rules()?;
    let engine = engine(&rules)?;
    let hash = engine.rule_set_hash();
    let version = EngineVersion::MIN;

    let mut files = Vec::new();
    let mut push = |path: String, bytes: Vec<u8>| files.push(CorpusFile { path, bytes });

    push(
        format!("{HARNESS_ROOT}/{GRADUATION_HARNESS_DIR}/ruleset.txt"),
        rules.canonical_text().into_bytes(),
    );
    push(
        format!("{HARNESS_ROOT}/{GRADUATION_HARNESS_DIR}/property/bounds.txt"),
        bounds_text().into_bytes(),
    );

    let mut snapshot = None;
    for (case, facts) in cases(&rules)? {
        let inputs = encode(&facts)?;
        let audit = DegreeAudit::evaluate(&engine, &inputs)?;
        push(
            format!("{HARNESS_ROOT}/{GRADUATION_HARNESS_DIR}/{case}.input"),
            inputs.canonical_text().into_bytes(),
        );
        if case.starts_with("version-compat/") {
            push(
                format!("{HARNESS_ROOT}/{GRADUATION_HARNESS_DIR}/{case}.explanation"),
                audit
                    .outcome()
                    .explanation_snapshot
                    .as_str()
                    .as_bytes()
                    .to_vec(),
            );
        } else {
            push(
                format!("{HARNESS_ROOT}/{GRADUATION_HARNESS_DIR}/{case}.expected"),
                audit
                    .outcome()
                    .canonical_bytes(GRADUATION_ENGINE_ID, hash, version, &inputs),
            );
        }
        if case == "golden/baseline" {
            snapshot = Some(
                audit
                    .outcome()
                    .explanation_snapshot
                    .as_str()
                    .as_bytes()
                    .to_vec(),
            );
        }
    }
    push(
        format!("{HARNESS_ROOT}/{GRADUATION_HARNESS_DIR}/explanation.snapshot"),
        snapshot.ok_or("the builder rendered no baseline explanation")?,
    );

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

/// The facts one committed case names, for the executing half of the harness.
pub fn facts_for(rules: &RuleSet, case: &str) -> Result<Option<AuditFacts>, Box<dyn Error>> {
    Ok(cases(rules)?
        .into_iter()
        .find(|(name, _)| name == case)
        .map(|(_, facts)| facts))
}

/// Every case name the builder renders, in committed order.
pub fn case_names(rules: &RuleSet) -> Result<Vec<String>, Box<dyn Error>> {
    Ok(cases(rules)?.into_iter().map(|(name, _)| name).collect())
}

/// The types the signatures above name, re-exported so the example and the
/// executing test do not each import them.
pub type Snapshot = TranscriptSnapshot;
/// See [`Snapshot`].
pub type Sources = RuleSourceIndex;
/// See [`Snapshot`].
pub type Freshness = SourceFreshnessPolicy;
/// See [`Snapshot`].
pub type Conflicts = Vec<ConflictReference>;
