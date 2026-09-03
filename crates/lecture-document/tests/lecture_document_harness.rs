//! The §3.9 harness obligations `TRANSCRIPT_COVERAGE` takes on by becoming
//! `IMPLEMENTED`.
//!
//! `docs/contracts/engine-harness.md` says flipping a registry entry is "the
//! moment its harness obligations become due", and the audit in
//! `academic-domain` enforces that the artifacts *exist*. It cannot enforce
//! more than that: this crate depends on `academic-domain`, so that crate
//! cannot call this engine, and a fixture the audit only counts is a file that
//! proves nothing. The page says it plainly — "an engine that flips to
//! `IMPLEMENTED` without that second half has satisfied the audit and
//! demonstrated nothing."
//!
//! This file is the other half, and it is `crates/record/tests/record_harness.rs`
//! reused rather than reinvented: every committed fixture is executed against
//! the real engine and byte-compared, and the whole corpus is re-rendered from
//! the builder and compared, so a fixture edited by hand into agreement with a
//! broken engine fails rather than passes.

mod common;

use std::{collections::BTreeSet, fs, path::PathBuf};

use academic_domain::engines::{EngineVersion, FrozenInputs, ProofStatus};
use academic_lecture_document::{
    RULES, RULESET_TEXT, TRANSCRIPT_COVERAGE_ENGINE_ID, TRANSCRIPT_COVERAGE_ENGINE_VERSION,
    TranscriptCoverageEngine,
    harness::{
        self, GOLDEN, HARNESS_DIR, HARNESS_ROOT, PROPERTY_MAX_SEGMENTS, SNAPSHOT_CASE,
        VERSION_COMPAT, case_input, corpus_files,
    },
    ruleset_hash,
};

use common::TestResult;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The committed corpus is exactly what the builder renders.
///
/// `CONTRIBUTING.md` rule 5 in executable form. Without this the `.expected`
/// files could be regenerated from a broken engine, or edited by hand, and
/// every fixture below would still pass.
#[test]
fn harness_corpus_matches_a_fresh_render() -> TestResult {
    let root = repository_root();
    let rendered = corpus_files()?;
    assert!(
        rendered.len() >= 13,
        "the builder rendered only {} files; the committed directory is larger than that",
        rendered.len()
    );
    for file in &rendered {
        let path = root.join(&file.path);
        let committed = fs::read(&path).map_err(|error| format!("{}: {error}", file.path))?;
        assert_eq!(
            committed, file.bytes,
            "{} differs from a fresh render; re-run `cargo run -p academic-lecture-document \
             --example emit_harness` and explain the semantic change",
            file.path
        );
    }

    // And nothing extra hides under this engine's directory.
    let rendered_paths: BTreeSet<String> = rendered.iter().map(|file| file.path.clone()).collect();
    let mut walked = Vec::new();
    walk(
        &root.join(HARNESS_ROOT).join(HARNESS_DIR),
        &root,
        &mut walked,
    )?;
    assert!(
        !walked.is_empty(),
        "the harness directory walk returned nothing"
    );
    for path in walked {
        assert!(
            rendered_paths.contains(&path),
            "{path} is under this engine's harness directory and the builder does not render it"
        );
    }
    Ok(())
}

fn walk(directory: &PathBuf, root: &PathBuf, found: &mut Vec<String>) -> TestResult {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, root, found)?;
        } else {
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .ok_or("a harness path is not UTF-8")?
                .replace('\\', "/");
            found.push(relative);
        }
    }
    Ok(())
}

/// Every golden case evaluates to its committed bytes, twice.
#[test]
fn golden_cases_evaluate_to_their_committed_bytes() -> TestResult {
    let root = repository_root();
    let hash = ruleset_hash();
    let version = EngineVersion::new(TRANSCRIPT_COVERAGE_ENGINE_VERSION)?;
    assert!(!GOLDEN.is_empty(), "the golden corpus is empty");
    for case in &GOLDEN {
        let input_path = root.join(format!(
            "{HARNESS_ROOT}/{HARNESS_DIR}/golden/{}.input",
            case.name
        ));
        let expected_path = root.join(format!(
            "{HARNESS_ROOT}/{HARNESS_DIR}/golden/{}.expected",
            case.name
        ));
        let text = fs::read_to_string(&input_path)?;
        let inputs = FrozenInputs::parse(&text)?;

        // The committed `.input` is what the builder encodes for this case.
        assert_eq!(
            inputs.canonical_text(),
            case_input(case)?.canonical_text(),
            "{}'s committed input is not what the builder renders",
            case.name
        );

        let outcome = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
        let bytes = outcome.canonical_bytes(TRANSCRIPT_COVERAGE_ENGINE_ID, hash, version, &inputs);
        assert_eq!(
            fs::read(&expected_path)?,
            bytes,
            "{} no longer evaluates to its committed bytes",
            case.name
        );

        let again = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
        assert_eq!(
            again.canonical_bytes(TRANSCRIPT_COVERAGE_ENGINE_ID, hash, version, &inputs),
            bytes,
            "{} evaluated twice to two different byte strings",
            case.name
        );

        // Every proof node cites a published rule and carries one of the five
        // statuses.
        for node in outcome.proof_tree.walk() {
            assert!(
                RULES.contains(&node.rule_id.as_str()),
                "{} cites a rule the published set does not hold: {}",
                case.name,
                node.rule_id
            );
            assert!(ProofStatus::ALL.contains(&node.status));
        }
    }

    // The corpus is not all one answer. Without this the byte comparisons above
    // would pass on an engine that returned a constant.
    let mut verdicts: Vec<ProofStatus> = Vec::new();
    for case in &GOLDEN {
        let inputs = case_input(case)?;
        verdicts.push(
            TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?
                .result
                .status,
        );
    }
    let mut distinct = verdicts.clone();
    distinct.sort();
    distinct.dedup();
    assert!(
        distinct.len() >= 2,
        "every golden case produced the same verdict, so the corpus discriminates nothing"
    );
    assert!(verdicts.contains(&ProofStatus::Satisfied));
    Ok(())
}

/// The version-compatibility case still produces its committed explanation, and
/// the committed snapshot is the one the engine renders.
#[test]
fn version_compat_and_snapshot_replay() -> TestResult {
    let root = repository_root();
    let hash = ruleset_hash();

    let compat_input = root.join(format!(
        "{HARNESS_ROOT}/{HARNESS_DIR}/version-compat/v1-{}.input",
        VERSION_COMPAT.name
    ));
    let compat_explanation = root.join(format!(
        "{HARNESS_ROOT}/{HARNESS_DIR}/version-compat/v1-{}.explanation",
        VERSION_COMPAT.name
    ));
    let inputs = FrozenInputs::parse(&fs::read_to_string(&compat_input)?)?;
    let outcome = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
    assert_eq!(
        fs::read_to_string(&compat_explanation)?,
        outcome.explanation_snapshot.as_str(),
        "the version-compatibility explanation moved"
    );

    let snapshot = root.join(format!("{HARNESS_ROOT}/{HARNESS_DIR}/explanation.snapshot"));
    let snapshot_inputs = case_input(&SNAPSHOT_CASE)?;
    let snapshot_outcome = TranscriptCoverageEngine::evaluate_coverage(&snapshot_inputs, hash)?;
    assert_eq!(
        fs::read_to_string(&snapshot)?,
        snapshot_outcome.explanation_snapshot.as_str(),
        "the committed explanation snapshot moved"
    );

    // The snapshot is normalized: LF endings, no trailing whitespace, and
    // nothing host- or time-dependent.
    let text = snapshot_outcome.explanation_snapshot.as_str();
    assert!(!text.contains('\r'));
    for line in text.lines() {
        assert_eq!(
            line.trim_end(),
            line,
            "the explanation has trailing whitespace"
        );
    }

    // The two cases are different, so the assertions above are not comparing
    // one file with itself.
    assert_ne!(
        outcome.explanation_snapshot.as_str(),
        snapshot_outcome.explanation_snapshot.as_str()
    );
    Ok(())
}

/// The published rule set on disk is the one the engine hashes.
#[test]
fn the_committed_ruleset_is_the_one_the_engine_evaluates_under() -> TestResult {
    let root = repository_root();
    let committed =
        fs::read_to_string(root.join(format!("{HARNESS_ROOT}/{HARNESS_DIR}/ruleset.txt")))?;
    assert_eq!(committed, RULESET_TEXT);
    let mut lines: Vec<&str> = committed.lines().collect();
    lines.sort_unstable();
    let mut rules: Vec<&str> = RULES.to_vec();
    rules.sort_unstable();
    assert_eq!(
        lines, rules,
        "the published rule set is not the engine's rules"
    );

    // A hash over anything else is refused rather than evaluated under.
    let inputs = case_input(&GOLDEN[0])?;
    let foreign = academic_domain::engines::RuleSetHash::new(
        academic_domain::ContentDigest::sha256(committed.trim_end().as_bytes()),
    );
    assert_ne!(foreign, ruleset_hash());
    assert!(TranscriptCoverageEngine::evaluate_coverage(&inputs, foreign).is_err());
    Ok(())
}

/// The property the corpus's `property/bounds.txt` describes, run.
///
/// Over every shape a transcript of up to [`PROPERTY_MAX_SEGMENTS`] segments
/// can take under the five outcomes, the partition reconciles: the declared
/// eligible count equals the number of segment blocks, and the declared
/// unmapped count equals the number of blocks spelling `UNMAPPED`.
#[test]
fn the_partition_property_holds_over_the_declared_bounds() -> TestResult {
    let hash = ruleset_hash();
    let statuses = [
        "MAPPED",
        "EXCLUDED_NON_SPEECH",
        "REDACTED_WITH_POLICY",
        "UNTRANSCRIBED_FAILURE",
        "UNMAPPED",
    ];
    let mut satisfied = 0_usize;
    let mut refused = 0_usize;
    for count in 1..=PROPERTY_MAX_SEGMENTS {
        for offset in 0..statuses.len() {
            let mut lines = String::new();
            let mut unmapped = 0_i64;
            let mut mapped = 0_i64;
            let mut excluded = 0_i64;
            for index in 0..count {
                let status = statuses[(index + offset) % statuses.len()];
                match status {
                    "UNMAPPED" => unmapped += 1,
                    "MAPPED" => mapped += 1,
                    "EXCLUDED_NON_SPEECH" => excluded += 1,
                    _ => {}
                }
                lines.push_str(&format!(
                    "coverage.segment.{index:04}.id=ref:raw_segment_{:04}\n",
                    index + 1
                ));
                lines.push_str(&format!(
                    "coverage.segment.{index:04}.status=ref:{status}\n"
                ));
                lines.push_str(&format!("coverage.segment.{index:04}.tokens=int:4\n"));
            }
            let eligible = i64::try_from(count)?;
            let denominator = eligible - excluded;
            let mut text = String::new();
            text.push_str("coverage.captures.excluded=int:0\n");
            text.push_str("coverage.captures.placed=int:0\n");
            text.push_str("coverage.captures.unaccounted=int:0\n");
            text.push_str("coverage.config.gap_threshold_nanos=int:2000000000\n");
            text.push_str("coverage.config.low_confidence_permille=int:700\n");
            text.push_str("coverage.config.version=int:1\n");
            text.push_str("coverage.gaps.total=int:0\n");
            text.push_str("coverage.gaps.unexplained=int:0\n");
            text.push_str("coverage.ordering.exceptions=int:0\n");
            text.push_str("coverage.ordering.findings=int:0\n");
            text.push_str("coverage.render.defects=int:0\n");
            text.push_str(&lines);
            text.push_str(&format!(
                "coverage.segment_coverage.denominator=int:{denominator}\n"
            ));
            text.push_str(&format!(
                "coverage.segment_coverage.numerator=int:{mapped}\n"
            ));
            text.push_str(&format!("coverage.segments.eligible=int:{eligible}\n"));
            text.push_str(&format!("coverage.segments.unmapped=int:{unmapped}\n"));
            text.push_str(&format!(
                "coverage.token_coverage.denominator=int:{}\n",
                denominator * 4
            ));
            text.push_str(&format!(
                "coverage.token_coverage.numerator=int:{}\n",
                mapped * 4
            ));
            let mut sorted: Vec<&str> = text.lines().collect();
            sorted.sort_unstable();
            let canonical = sorted
                .iter()
                .map(|line| format!("{line}\n"))
                .collect::<String>();

            let inputs = FrozenInputs::parse(&canonical)?;
            let outcome = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
            let partition = outcome
                .proof_tree
                .walk()
                .into_iter()
                .find(|node| node.node_id.as_str() == "n.partition")
                .ok_or("the proof tree lost its partition node")?;
            assert_ne!(
                partition.status,
                ProofStatus::Conflict,
                "a well-formed partition was reported as a conflict at {count}/{offset}"
            );
            if unmapped == 0 {
                assert_eq!(partition.status, ProofStatus::Satisfied);
                satisfied += 1;
            } else {
                assert_eq!(partition.status, ProofStatus::Needs);
                refused += 1;
            }
        }
    }
    assert!(
        satisfied > 0 && refused > 0,
        "the property test saw only one arm"
    );

    // A status the engine does not recognise is a `CONFLICT`, and a
    // `SATISFIED` result over a conflict is refused by the harness itself.
    let mut text = String::new();
    text.push_str("coverage.captures.excluded=int:0\n");
    text.push_str("coverage.captures.placed=int:0\n");
    text.push_str("coverage.captures.unaccounted=int:0\n");
    text.push_str("coverage.config.gap_threshold_nanos=int:2000000000\n");
    text.push_str("coverage.config.low_confidence_permille=int:700\n");
    text.push_str("coverage.config.version=int:1\n");
    text.push_str("coverage.gaps.total=int:0\n");
    text.push_str("coverage.gaps.unexplained=int:0\n");
    text.push_str("coverage.ordering.exceptions=int:0\n");
    text.push_str("coverage.ordering.findings=int:0\n");
    text.push_str("coverage.render.defects=int:0\n");
    text.push_str("coverage.segment.0000.id=ref:raw_segment_0001\n");
    text.push_str("coverage.segment.0000.status=ref:PROBABLY_FINE\n");
    text.push_str("coverage.segment.0000.tokens=int:4\n");
    text.push_str("coverage.segment_coverage.denominator=int:1\n");
    text.push_str("coverage.segment_coverage.numerator=int:1\n");
    text.push_str("coverage.segments.eligible=int:1\n");
    text.push_str("coverage.segments.unmapped=int:0\n");
    text.push_str("coverage.token_coverage.denominator=int:4\n");
    text.push_str("coverage.token_coverage.numerator=int:4\n");
    let inputs = FrozenInputs::parse(&text)?;
    let outcome = TranscriptCoverageEngine::evaluate_coverage(&inputs, hash)?;
    assert_eq!(outcome.result.status, ProofStatus::Conflict);

    // The generator bound the committed `property/bounds.txt` declares is the
    // one this test ran to.
    let bounds = fs::read_to_string(
        repository_root().join(format!("{HARNESS_ROOT}/{HARNESS_DIR}/property/bounds.txt")),
    )?;
    assert!(
        bounds.contains(&format!("segments 1..={PROPERTY_MAX_SEGMENTS}")),
        "the committed bounds and this test's generator disagree"
    );
    let _ = harness::HARNESS_ROOT;
    Ok(())
}
