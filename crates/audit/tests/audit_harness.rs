//! The §3.9 harness obligations `GRADUATION_AUDIT` takes on by becoming
//! `IMPLEMENTED`.
//!
//! `docs/contracts/engine-harness.md` says flipping a registry entry is "the
//! moment its harness obligations become due", and the audit in
//! `academic-domain` enforces that the artifacts *exist*. It cannot enforce
//! more: `academic-domain` is what this crate depends on, so it cannot call
//! this engine, and a fixture the audit only counts is a file that proves
//! nothing.
//!
//! This file is the other half. Every committed fixture is executed against the
//! real engine and byte-compared; every adverse fixture is required to land on
//! the outcome its directory names; and the whole corpus is re-rendered from
//! the deterministic builder and compared, so a fixture edited by hand into
//! agreement with a broken engine fails rather than passes.

mod support;

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_domain::engines::{EngineVersion, FrozenInputs, ProofStatus};

use academic_audit::{DegreeAudit, GRADUATION_ENGINE_ID, GRADUATION_HARNESS_DIR, encode};
use support::{TestResult, harness};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The committed corpus is exactly what the deterministic builder renders.
///
/// `CONTRIBUTING.md` rule 5 in executable form. Without this the `.expected`
/// files could be regenerated from a broken engine, or edited by hand, and
/// every fixture below would still pass.
#[test]
fn harness_corpus_matches_a_fresh_render() -> TestResult {
    let root = repository_root();
    let rendered = harness::corpus_files()?;
    assert!(
        rendered.len() >= 17,
        "the builder rendered only {} files; the directory is larger than that",
        rendered.len()
    );
    for file in &rendered {
        let path = root.join(&file.path);
        let committed = fs::read(&path).map_err(|error| format!("{}: {error}", file.path))?;
        assert_eq!(
            committed, file.bytes,
            "{} differs from a fresh render; re-run `cargo run -p academic-audit \
             --example emit_harness` and explain the semantic change",
            file.path
        );
    }

    // And nothing extra hides under this engine's directory.
    //
    // `oracle.expected` is the one file below it that this builder does not
    // render, because `tools/graduation-audit-oracle.mjs` renders it --
    // deliberately, so the expected statuses do not come from this crate. That
    // is the same allowance the `GPA` harness makes for its own oracle.
    let rendered_paths: BTreeSet<String> = rendered.iter().map(|file| file.path.clone()).collect();
    let allowed_outside = format!(
        "{}/{GRADUATION_HARNESS_DIR}/oracle.expected",
        harness::HARNESS_ROOT
    );
    let base = root
        .join(harness::HARNESS_ROOT)
        .join(GRADUATION_HARNESS_DIR);
    let mut found = 0_usize;
    for path in walk(&base)? {
        let relative = path
            .strip_prefix(&root)?
            .to_string_lossy()
            .replace('\\', "/");
        if relative == allowed_outside {
            continue;
        }
        assert!(
            rendered_paths.contains(&relative),
            "{relative} sits under the harness directory and the builder does not render it"
        );
        found += 1;
    }
    assert_eq!(
        found,
        rendered.len(),
        "the directory and the render disagree about how many files there are"
    );
    Ok(())
}

/// Every committed fixture runs against the real engine and reproduces its bytes.
#[test]
fn every_committed_fixture_runs_and_reproduces_its_expected_bytes() -> TestResult {
    let root = repository_root();
    let rules = support::baseline_rules()?;
    let engine = harness::engine(&rules)?;
    let hash = engine.rule_set_hash();

    // The rule-set hash is the SHA-256 of the committed `ruleset.txt`, which is
    // what the harness contract says it must be. If they drifted, every
    // `.expected` file would be attributed to a rule set nobody could read.
    let committed_rules = fs::read(
        root.join(harness::HARNESS_ROOT)
            .join(GRADUATION_HARNESS_DIR)
            .join("ruleset.txt"),
    )?;
    assert_eq!(
        academic_domain::ContentDigest::sha256(&committed_rules),
        hash.digest(),
        "ruleset.txt is not the rule set the fixtures were evaluated under"
    );

    let base = root
        .join(harness::HARNESS_ROOT)
        .join(GRADUATION_HARNESS_DIR);
    let mut executed = 0_usize;
    let mut adverse_seen = BTreeSet::new();
    for stem in harness::case_names(&rules)? {
        let input_path = base.join(format!("{stem}.input"));
        let committed = fs::read_to_string(&input_path)?;
        let inputs = FrozenInputs::parse(&committed)?;

        // The committed input text is what the builder froze, byte for byte.
        let facts =
            harness::facts_for(&rules, &stem)?.ok_or_else(|| format!("{stem} names no facts"))?;
        assert_eq!(
            encode(&facts)?.canonical_text(),
            committed,
            "{stem}.input is not the frozen input the builder rendered"
        );

        let audit = DegreeAudit::evaluate(&engine, &inputs)?;
        if stem.starts_with("version-compat/") {
            let expected = fs::read_to_string(base.join(format!("{stem}.explanation")))?;
            assert_eq!(
                audit.outcome().explanation_snapshot.as_str(),
                expected,
                "{stem} no longer produces the explanation every admitted version must"
            );
        } else {
            let expected = fs::read(base.join(format!("{stem}.expected")))?;
            assert_eq!(
                audit.outcome().canonical_bytes(
                    GRADUATION_ENGINE_ID,
                    hash,
                    EngineVersion::MIN,
                    &inputs
                ),
                expected,
                "{stem} does not reproduce its committed bytes"
            );
        }

        // The adverse directories are the point of the high-impact rule: a
        // fixture that merely exists proves nothing, so each is required to
        // land on the outcome its directory names.
        if let Some(path) = stem.strip_prefix("adverse/") {
            let arm = path.split('/').next().unwrap_or_default();
            adverse_seen.insert(arm.to_owned());
            match arm {
                "unknown" => {
                    assert_eq!(audit.outcome().result.status, ProofStatus::Unknown);
                    assert!(
                        audit.verdict().determinate().is_none(),
                        "an UNKNOWN audit reached a determination"
                    );
                }
                "conflict" => {
                    assert_eq!(audit.outcome().result.status, ProofStatus::Conflict);
                    assert!(
                        audit.outcome().result.values.is_empty(),
                        "a CONFLICT result published a derived value"
                    );
                    assert!(audit.verdict().determinate().is_none());
                }
                "partial_failure" => {
                    assert!(
                        audit.outcome().result.is_partial_failure(),
                        "a partial-failure fixture must leave a rule unevaluated"
                    );
                    assert!(audit.verdict().determinate().is_none());
                }
                other => return Err(format!("unknown adverse path {other:?}").into()),
            }
        }

        // A golden case is a case that reached a determination or said exactly
        // why it did not. Both are required to be present in the directory, so
        // a corpus in which nothing is ever determinate would fail here.
        executed += 1;
    }

    assert_eq!(
        adverse_seen,
        BTreeSet::from([
            "conflict".to_owned(),
            "partial_failure".to_owned(),
            "unknown".to_owned()
        ]),
        "a high-impact engine ships all three adverse sets"
    );
    assert!(executed >= 7, "only {executed} fixtures ran");
    Ok(())
}

/// The directory carries both a determinate case and an indeterminate one.
///
/// Without this the corpus could drift into one in which every case is
/// `INDETERMINATE`, and the `adverse/*` directories would then be indexing a
/// distinction the golden set no longer makes.
#[test]
fn the_corpus_shows_both_sides_of_the_three_gate_rule() -> TestResult {
    let rules = support::baseline_rules()?;
    let engine = harness::engine(&rules)?;

    let determinate = DegreeAudit::evaluate(
        &engine,
        &encode(&harness::facts_for(&rules, "golden/baseline")?.ok_or("no baseline case")?)?,
    )?;
    assert!(
        determinate.verdict().determinate().is_some(),
        "the baseline golden case is not determinate: {:?}",
        determinate.verdict().missing()
    );
    assert_eq!(
        determinate.verdict().missing(),
        &[],
        "a determinate verdict carries outstanding checks"
    );

    let indeterminate = DegreeAudit::evaluate(
        &engine,
        &encode(
            &harness::facts_for(&rules, "golden/no_freshness_criterion")?
                .ok_or("no freshness case")?,
        )?,
    )?;
    assert!(indeterminate.verdict().determinate().is_none());
    assert!(!indeterminate.verdict().missing().is_empty());
    Ok(())
}

/// The declared property bounds are the committed ones.
///
/// The generator bounds are data rather than literals inside a test, so the
/// committed artifact and any generator over this engine cannot drift.
#[test]
fn the_property_bounds_are_the_declared_ones() -> TestResult {
    let committed = fs::read_to_string(
        repository_root()
            .join(harness::HARNESS_ROOT)
            .join(GRADUATION_HARNESS_DIR)
            .join("property")
            .join("bounds.txt"),
    )?;
    for line in [
        format!("rules.max={}", harness::PROPERTY_MAX_RULES),
        format!("attempts.max={}", harness::PROPERTY_MAX_ATTEMPTS),
        format!("credit.threshold.max={}", harness::PROPERTY_MAX_THRESHOLD),
    ] {
        assert!(
            committed.contains(&line),
            "the committed bounds do not declare {line}"
        );
    }
    Ok(())
}

/// The baseline tree agrees with an oracle written somewhere else.
///
/// A proof tree checked against a tree the same engine produced proves only
/// that the engine is deterministic, and a proof tree is large enough that
/// comparing two of them *looks* like thorough evidence. So the expected
/// statuses and measures come from `tools/graduation-audit-oracle.mjs`: a
/// second transcription of the transcript, the grade table, the repeat ceiling
/// and the rules, in another language, with fixed-point `BigInt` units.
///
/// Changing the corpus, a category, a threshold, or the repeat ceiling on the
/// Rust side moves one side of this comparison and not the other.
#[test]
fn the_baseline_tree_agrees_with_an_independent_oracle() -> TestResult {
    let expected: std::collections::BTreeMap<String, String> = fs::read_to_string(
        repository_root()
            .join(harness::HARNESS_ROOT)
            .join(GRADUATION_HARNESS_DIR)
            .join("oracle.expected"),
    )?
    .lines()
    .filter(|line| !line.is_empty())
    .filter_map(|line| {
        line.split_once('=')
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
    })
    .collect();
    assert!(
        expected.len() >= 20,
        "the oracle block carries only {} rows",
        expected.len()
    );

    let rules = support::baseline_rules()?;
    let engine = harness::engine(&rules)?;
    let audit = DegreeAudit::evaluate(
        &engine,
        &encode(&harness::facts_for(&rules, "golden/baseline")?.ok_or("no baseline case")?)?,
    )?;

    let row = |key: &str| -> Result<String, Box<dyn Error>> {
        expected
            .get(key)
            .cloned()
            .ok_or_else(|| format!("the oracle carries no {key}").into())
    };

    let mut compared = 0_usize;
    for node in audit.walk() {
        let leaf = node.leaf();
        // An operand child carries its parent's rule identifier -- one rule,
        // several operands -- so the node id is what distinguishes them and the
        // measure is only the parent's.
        let is_rule_node = node.node_id().as_str() == leaf.rule().as_str();
        let key = if is_rule_node {
            format!("status.{}", leaf.rule())
        } else {
            format!("operand.{}", node.node_id())
        };
        let Some(oracle) = expected.get(&key) else {
            continue;
        };
        assert_eq!(
            leaf.status().as_str(),
            oracle,
            "{key} disagrees with the oracle"
        );
        compared += 1;

        if !is_rule_node {
            continue;
        }
        let measured = match leaf.measure() {
            Some(academic_requirement::Measure::Credits { attained, required })
            | Some(academic_requirement::Measure::Count { attained, required }) => {
                Some(format!("{attained}/{required}"))
            }
            _ => None,
        };
        if let Some(measured) = measured {
            let measure = format!("measure.{}", leaf.rule());
            if let Some(oracle) = expected.get(&measure) {
                assert_eq!(measured, *oracle, "{measure}");
                compared += 1;
            }
        }
    }
    assert!(
        compared >= 19,
        "only {compared} readings were compared against the oracle"
    );

    // The grade-point reading the rules were handed, in the oracle's units.
    let reading = audit
        .transcript()
        .reading(&academic_requirement::GpaScope::new(
            academic_audit::ALL_GPA_ELIGIBLE,
        )?)
        .ok_or("the baseline transcript published no cumulative reading")?;
    //
    // The oracle carries tenths and never rescales; the Rust side carries a
    // coefficient and a scale. Rescaling here is exact and refuses a fraction
    // of a tenth rather than rounding one away, so the two representations are
    // compared rather than one being converted into the other's answer.
    let scale = reading.weighted_points.scale();
    assert!(scale >= 1, "the weighted points carry no fractional digit");
    let divisor = 10_i128
        .checked_pow(u32::from(scale) - 1)
        .ok_or("the weighted-point scale is out of range")?;
    assert_eq!(
        reading.weighted_points.coefficient() % divisor,
        0,
        "the weighted points are not a whole number of tenths"
    );
    assert_eq!(
        (reading.weighted_points.coefficient() / divisor).to_string(),
        row("gpa.weighted_points_tenths")?,
        "the weighted points disagree with the oracle"
    );
    assert_eq!(
        reading.denominator_credits.to_string(),
        row("gpa.denominator_credits")?
    );

    // Earned credits and the denominator are different quantities, and the
    // oracle says so independently.
    let earned: u32 = audit
        .transcript()
        .entries()
        .iter()
        .filter_map(|entry| match entry.admission() {
            academic_audit::EntryAdmission::Counted { credits, .. } => {
                Some(u32::from(credits.get()))
            }
            _ => None,
        })
        .sum();
    assert_eq!(earned.to_string(), row("earned_credits")?);
    assert_ne!(
        earned, reading.denominator_credits,
        "earned credits and the grade-point denominator are the same number; \
         this corpus separates nothing"
    );

    assert_eq!(audit.root_status().as_str(), row("root.status")?);
    assert_eq!(audit.verdict().as_str(), row("verdict")?);
    let outcome = audit
        .verdict()
        .determinate()
        .ok_or("the oracle says DETERMINATE and the audit is not")?
        .outcome();
    assert_eq!(outcome.as_str(), row("outcome")?);
    Ok(())
}

/// Every `.rs`-free file under one directory, recursively.
fn walk(base: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut pending = vec![base.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}
