//! The §3.9 harness obligations `GPA` and `CREDIT_ACCOUNTING` take on by
//! becoming `IMPLEMENTED`.
//!
//! `docs/contracts/engine-harness.md` says flipping a registry entry is "the
//! moment its harness obligations become due", and the audit in
//! `academic-domain` enforces that the artifacts *exist*. It cannot enforce
//! more than that: `academic-domain` is what this crate depends on, so it
//! cannot call these engines, and a fixture the audit only counts is a file
//! that proves nothing.
//!
//! This file is the other half. Every committed fixture is executed against the
//! real engine and byte-compared; every adverse fixture is required to land on
//! the outcome its directory names; and the whole corpus is re-rendered from
//! the deterministic builder and compared, so a fixture edited by hand into
//! agreement with a broken engine fails rather than passes.

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_domain::engines::{EngineVersion, FrozenInputs, ProofStatus};
use academic_record::{
    attempt::{AttemptHistory, CourseAttempt, SettledStatus},
    corpus, decimal,
    engine::{CREDIT_ENGINE_ID, CreditAccountingEngine, GPA_ENGINE_ID, GpaEngine},
    facts::{AttemptFacts, GpaScope, encode},
    grade::GradeSymbol,
    harness::{self, CREDIT_TENTHS_HIGH, CREDIT_TENTHS_LOW, PROPERTY_MAX_ATTEMPTS},
    policy::AttemptOrigin,
    term::{Semester, TermKey},
    views::{AverageContribution, GpaValue, RecordViews},
};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn Error>>;

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
        rendered.len() >= 26,
        "the builder rendered only {} files; both harness directories are larger than that",
        rendered.len()
    );
    for file in &rendered {
        let path = root.join(&file.path);
        let committed = fs::read(&path).map_err(|error| format!("{}: {error}", file.path))?;
        assert_eq!(
            committed, file.bytes,
            "{} differs from a fresh render; re-run `cargo run -p academic-record \
             --example emit_harness` and explain the semantic change",
            file.path
        );
    }

    // And nothing extra hides under **either of this crate's two directories**.
    // `oracle.expected` is the one file below the GPA harness that the builder
    // does not render, because `tools/gpa-oracle.mjs` renders it — deliberately,
    // so the expected GPA values do not come from this crate.
    //
    // The walk is scoped to the two directories this builder owns rather than
    // to the whole harness root, which is what the sentence above always meant.
    // `P2-L4` flipped `TRANSCRIPT_COVERAGE` and rendered its corpus from
    // `academic-lecture-document`, and a root-wide walk here would have failed
    // on files this crate cannot render and has no business asserting about.
    // The engine-wide "nothing else sits under the root" rule belongs to the
    // audit in `academic-domain`, which knows every registered engine's
    // directory; this half knows two.
    let rendered_paths: BTreeSet<String> = rendered.iter().map(|file| file.path.clone()).collect();
    let allowed_outside = format!("{}/gpa/oracle.expected", harness::HARNESS_ROOT);
    let mut walked = Vec::new();
    for directory in [harness::GPA_HARNESS_DIR, harness::CREDIT_HARNESS_DIR] {
        walk(
            &root.join(harness::HARNESS_ROOT).join(directory),
            &root,
            &mut walked,
        )?;
    }
    assert!(
        !walked.is_empty(),
        "the harness directory walk returned nothing"
    );
    for path in walked {
        assert!(
            rendered_paths.contains(&path) || path == allowed_outside,
            "{path} is under this crate's harness directories and the builder does not render it"
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

/// Every golden and adverse fixture runs and produces its committed bytes.
#[test]
fn every_committed_fixture_runs_and_reproduces_its_expected_bytes() -> TestResult {
    let root = repository_root();
    let rules = corpus::baseline_rules()?;
    let gpa = GpaEngine::new(rules.clone(), EngineVersion::MIN);
    let credit = CreditAccountingEngine::new(rules.clone(), EngineVersion::MIN);
    let hash = gpa.rule_set_hash();

    // The rule-set hash is the SHA-256 of the committed `ruleset.txt`, which is
    // what the harness contract says it must be. If they drifted, every
    // `.expected` file would be attributed to a rule set nobody could read.
    for directory in [harness::GPA_HARNESS_DIR, harness::CREDIT_HARNESS_DIR] {
        let committed = fs::read(
            root.join(harness::HARNESS_ROOT)
                .join(directory)
                .join("ruleset.txt"),
        )?;
        assert_eq!(
            academic_domain::ContentDigest::sha256(&committed),
            hash.digest(),
            "{directory}/ruleset.txt is not the rule set the fixtures were evaluated under"
        );
    }

    let mut executed = 0_usize;
    for (directory, engine_id) in [
        (harness::GPA_HARNESS_DIR, GPA_ENGINE_ID),
        (harness::CREDIT_HARNESS_DIR, CREDIT_ENGINE_ID),
    ] {
        let base = root.join(harness::HARNESS_ROOT).join(directory);
        let mut inputs_found = Vec::new();
        walk(&base, &base, &mut inputs_found)?;
        for relative in inputs_found {
            let Some(stem) = relative.strip_suffix(".input") else {
                continue;
            };
            let text = fs::read_to_string(base.join(&relative))?;
            let inputs = FrozenInputs::parse(&text)?;
            let outcome = if engine_id == GPA_ENGINE_ID {
                gpa.evaluate_record(&inputs, hash)?
            } else {
                credit.evaluate_record(&inputs, hash)?
            };

            if stem.starts_with("version-compat/") {
                let committed = fs::read_to_string(base.join(format!("{stem}.explanation")))?;
                assert_eq!(
                    outcome.explanation_snapshot.as_str(),
                    committed,
                    "{directory}/{stem} no longer produces its admitted explanation"
                );
            } else {
                let committed = fs::read(base.join(format!("{stem}.expected")))?;
                assert_eq!(
                    outcome.canonical_bytes(engine_id, hash, EngineVersion::MIN, &inputs),
                    committed,
                    "{directory}/{stem} no longer produces its committed bytes"
                );
            }

            // The adverse directories are the point of the high-impact rule: a
            // fixture that merely exists proves nothing, so each is required to
            // land on the outcome its directory names.
            if let Some(path) = stem.strip_prefix("adverse/") {
                let expected = match path.split('/').next() {
                    Some("unknown") => ProofStatus::Unknown,
                    Some("conflict") => ProofStatus::Conflict,
                    Some("partial_failure") => ProofStatus::Needs,
                    other => return Err(format!("unknown adverse path {other:?}").into()),
                };
                assert_eq!(
                    outcome.result.status, expected,
                    "{directory}/{stem} does not land on the outcome its directory names"
                );
                if path.starts_with("partial_failure/") {
                    assert!(
                        outcome.result.is_partial_failure(),
                        "a partial-failure fixture must leave a rule unevaluated"
                    );
                }
                // A record that disagrees with itself publishes no average.
                if path.starts_with("conflict/") {
                    assert!(
                        !outcome.result.values.contains_key("gpa"),
                        "a CONFLICT result must not publish an average"
                    );
                }
                if path.starts_with("unknown/") {
                    assert!(
                        !outcome.result.values.contains_key("gpa"),
                        "an UNKNOWN result must not publish an average"
                    );
                }
            }
            executed += 1;
        }
    }
    assert!(
        executed >= 10,
        "only {executed} fixtures ran; the corpus is larger than that, so the walk stopped short"
    );

    // The explanation snapshot is the representative case's, byte for byte.
    let snapshot = fs::read_to_string(
        root.join(harness::HARNESS_ROOT)
            .join(harness::GPA_HARNESS_DIR)
            .join("explanation.snapshot"),
    )?;
    let cumulative = fs::read_to_string(
        root.join(harness::HARNESS_ROOT)
            .join(harness::GPA_HARNESS_DIR)
            .join("golden/cumulative.input"),
    )?;
    let inputs = FrozenInputs::parse(&cumulative)?;
    assert_eq!(
        gpa.evaluate_record(&inputs, hash)?
            .explanation_snapshot
            .as_str(),
        snapshot
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The property the `property/bounds.txt` artifact declares
// ---------------------------------------------------------------------------

/// Builds a history from generated rows. Every row is a distinct course, so no
/// repeat group forms and the property is about the average alone.
fn history_from(rows: &[(usize, GradeSymbol, i128)]) -> Result<AttemptHistory, Box<dyn Error>> {
    let mut history = AttemptHistory::new();
    for (index, grade, tenths) in rows {
        let earns = matches!(
            grade,
            GradeSymbol::F | GradeSymbol::U | GradeSymbol::W | GradeSymbol::I
        );
        history.append(CourseAttempt::from_confirmed_row(
            corpus::synthetic_attempt_id(u8::try_from(*index).unwrap_or(u8::MAX))?,
            format!("GEN.{index:03}"),
            TermKey::new(2020, Semester::Spring)?,
            SettledStatus::Completed,
            AttemptOrigin::Internal,
            academic_domain::Decimal::new(*tenths, 1)?,
            if earns {
                academic_domain::Decimal::new(0, 1)?
            } else {
                academic_domain::Decimal::new(*tenths, 1)?
            },
            Some(*grade),
            "snu_4_3_v1",
            vec![corpus::synthetic_evidence_id(
                u8::try_from(*index).unwrap_or(u8::MAX),
            )?],
        )?)?;
    }
    Ok(history)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// A published average never leaves the range of the grade points it averaged.
    ///
    /// The bound the `property/bounds.txt` artifact declares. It catches the
    /// whole class of arithmetic error that a single golden fixture cannot: a
    /// misplaced scale, a numerator and denominator swapped, a rounding step
    /// applied to the wrong side. All of those leave the range.
    #[test]
    fn a_published_average_stays_within_the_grade_points_it_averaged(
        rows in prop::collection::vec(
            (
                prop::sample::select(GradeSymbol::ALL.to_vec()),
                CREDIT_TENTHS_LOW..=CREDIT_TENTHS_HIGH,
            ),
            1..=PROPERTY_MAX_ATTEMPTS,
        )
    ) {
        let indexed: Vec<(usize, GradeSymbol, i128)> = rows
            .into_iter()
            .enumerate()
            .map(|(index, (grade, tenths))| (index + 1, grade, tenths))
            .collect();
        let history = history_from(&indexed).map_err(|error| {
            TestCaseError::fail(format!("corpus build failed: {error}"))
        })?;
        let rules = corpus::baseline_rules().map_err(|error| {
            TestCaseError::fail(format!("rules failed: {error}"))
        })?;
        let classification = corpus::classification_v1().map_err(|error| {
            TestCaseError::fail(format!("classification failed: {error}"))
        })?;
        let views = RecordViews::compute(&history, &rules, &classification).map_err(|error| {
            TestCaseError::fail(format!("compute failed: {error}"))
        })?;
        let value = views.cumulative_gpa().map_err(|error| {
            TestCaseError::fail(format!("average failed: {error}"))
        })?;
        let GpaValue::Known(published) = value else {
            // Every generated row is either graded or outside the average; a set
            // with no graded row has no average, which is not a violation.
            return Ok(());
        };

        let mut low: Option<academic_domain::Decimal> = None;
        let mut high: Option<academic_domain::Decimal> = None;
        for disposition in views.dispositions() {
            let AverageContribution::Included { effective_grade, .. } = disposition.average() else {
                continue;
            };
            let points = rules
                .scheme()
                .treatment(effective_grade)
                .grade_points()
                .ok_or_else(|| TestCaseError::fail("an included attempt has no grade points"))?;
            let update = |slot: &mut Option<academic_domain::Decimal>, keep_lower: bool| {
                *slot = Some(match *slot {
                    None => points,
                    Some(current) => {
                        let ordering = decimal::compare(points, current).unwrap_or(core::cmp::Ordering::Equal);
                        if (keep_lower && ordering.is_lt()) || (!keep_lower && ordering.is_gt()) {
                            points
                        } else {
                            current
                        }
                    }
                });
            };
            update(&mut low, true);
            update(&mut high, false);
        }
        let (Some(low), Some(high)) = (low, high) else {
            return Ok(());
        };
        // The published value is rounded, so it may sit up to half a unit of the
        // published scale outside the exact range. One unit of slack at the
        // published scale is therefore the correct bound, and it is still far
        // tighter than "somewhere between 0 and 4.3".
        let slack = academic_domain::Decimal::new(1, views.published_scale())
            .map_err(|error| TestCaseError::fail(format!("slack failed: {error}")))?;
        let floor = decimal::sub(low, slack)
            .map_err(|error| TestCaseError::fail(format!("floor failed: {error}")))?;
        let ceiling = decimal::add(high, slack)
            .map_err(|error| TestCaseError::fail(format!("ceiling failed: {error}")))?;
        prop_assert!(
            decimal::compare(published, floor).unwrap_or(core::cmp::Ordering::Less).is_ge(),
            "published {published:?} is below the lowest grade point averaged"
        );
        prop_assert!(
            decimal::compare(published, ceiling).unwrap_or(core::cmp::Ordering::Greater).is_le(),
            "published {published:?} is above the highest grade point averaged"
        );
    }

    /// Adding an `S` moves the earned total and never the average.
    ///
    /// The `credits_vs_denominator` contract as a property rather than as one
    /// corpus: whatever else is in the set, an `S` is on exactly one side.
    #[test]
    fn an_s_moves_credits_and_never_the_average(
        rows in prop::collection::vec(
            (
                prop::sample::select(GradeSymbol::ALL.to_vec()),
                CREDIT_TENTHS_LOW..=CREDIT_TENTHS_HIGH,
            ),
            1..=PROPERTY_MAX_ATTEMPTS,
        ),
        satisfactory in CREDIT_TENTHS_LOW..=CREDIT_TENTHS_HIGH,
    ) {
        let indexed: Vec<(usize, GradeSymbol, i128)> = rows
            .into_iter()
            .enumerate()
            .map(|(index, (grade, tenths))| (index + 1, grade, tenths))
            .collect();
        let rules = corpus::baseline_rules().map_err(|e| TestCaseError::fail(e.to_string()))?;
        let classification =
            corpus::classification_v1().map_err(|e| TestCaseError::fail(e.to_string()))?;

        let before_history =
            history_from(&indexed).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let before = RecordViews::compute(&before_history, &rules, &classification)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

        let mut with_s = indexed.clone();
        with_s.push((PROPERTY_MAX_ATTEMPTS + 5, GradeSymbol::S, satisfactory));
        let after_history = history_from(&with_s).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let after = RecordViews::compute(&after_history, &rules, &classification)
            .map_err(|e| TestCaseError::fail(e.to_string()))?;

        prop_assert_eq!(
            before.cumulative_gpa().map_err(|e| TestCaseError::fail(e.to_string()))?,
            after.cumulative_gpa().map_err(|e| TestCaseError::fail(e.to_string()))?,
            "an S must not move the average"
        );
        let before_credits = before
            .earned_credits()
            .map_err(|e| TestCaseError::fail(e.to_string()))?
            .partial();
        let after_credits = after
            .earned_credits()
            .map_err(|e| TestCaseError::fail(e.to_string()))?
            .partial();
        prop_assert!(
            decimal::compare(after_credits, before_credits)
                .unwrap_or(core::cmp::Ordering::Equal)
                .is_gt(),
            "an S must raise the earned total"
        );
        prop_assert_eq!(
            before.gpa_denominator().map_err(|e| TestCaseError::fail(e.to_string()))?.partial(),
            after.gpa_denominator().map_err(|e| TestCaseError::fail(e.to_string()))?.partial(),
            "an S must not enter the denominator"
        );
    }
}

/// The declared generator bounds are the ones the property test actually uses.
///
/// `property/bounds.txt` is a committed artifact the harness audit counts, and
/// a bounds file that drifted from the generator would describe a test that no
/// longer exists. Both read the same constants, and this asserts the file says
/// what they say.
#[test]
fn the_committed_bounds_are_the_generators_bounds() -> TestResult {
    for directory in [harness::GPA_HARNESS_DIR, harness::CREDIT_HARNESS_DIR] {
        let text = fs::read_to_string(
            repository_root()
                .join(harness::HARNESS_ROOT)
                .join(directory)
                .join("property/bounds.txt"),
        )?;
        assert!(text.contains(&format!("credits.tenths.low={CREDIT_TENTHS_LOW}")));
        assert!(text.contains(&format!("credits.tenths.high={CREDIT_TENTHS_HIGH}")));
        assert!(text.contains(&format!("attempts.max={PROPERTY_MAX_ATTEMPTS}")));
    }
    Ok(())
}

/// The frozen inputs the fixtures carry round-trip through the codec.
#[test]
fn frozen_inputs_round_trip_through_the_codec() -> TestResult {
    let classification = corpus::classification_v1()?;
    let history = corpus::baseline_history()?;
    let facts: Vec<AttemptFacts> = history
        .current()
        .into_iter()
        .map(|attempt| AttemptFacts::from_attempt(attempt, &classification))
        .collect();
    for scope in [
        GpaScope::Cumulative,
        GpaScope::Term(TermKey::new(2015, Semester::Spring)?),
        GpaScope::Major(academic_record::classify::ProgramId::new(
            corpus::PRIMARY_PROGRAM,
        )?),
    ] {
        let inputs = encode(&facts, &scope)?;
        let (decoded, decoded_scope) = academic_record::facts::decode(&inputs)?;
        assert_eq!(decoded_scope, scope);
        assert_eq!(decoded.len(), facts.len());
        let reencoded = encode(&decoded, &decoded_scope)?;
        assert_eq!(
            reencoded.canonical_text(),
            inputs.canonical_text(),
            "the frozen-input codec is not a round trip"
        );
    }
    Ok(())
}
