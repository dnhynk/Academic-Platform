//! `P2-C5`'s determinism contract, applied to section 16 without registering a
//! thirteenth engine.
//!
//! ## Why there is no registry row
//!
//! `docs/contracts/engine-harness.md` pins the registry to section 28's table
//! as an enumeration -- *exactly the §28 table rows, in table order* -- and
//! section 28 has twelve rows, none of them a critical path engine.
//! `engine_registry_is_complete` also refuses anything unregistered under
//! `testdata/engines/`. `the_registry_does_not_hold_a_critical_path_engine`
//! below asserts both halves of that reading against the specification and the
//! registry themselves, so a later edit that adds section 16 to the §28 table
//! fails here rather than leaving this file's premise silently false.
//!
//! ## What is proved instead
//!
//! `P2-C5`'s own vocabulary over this crate's own corpus at
//! `testdata/critical-path/`, the way the reference engine's lives at
//! `testdata/engine-harness-reference/`:
//!
//! * every committed case is evaluated against the **real** engine and
//!   byte-compared, so a fixture cannot be hand-edited into agreement with a
//!   broken engine;
//! * the whole corpus is re-rendered from the deterministic builder in
//!   `tests/corpus/mod.rs` and compared, which is `CONTRIBUTING.md` rule 5 in
//!   executable form, and the directory listing is compared too so a stale case
//!   left behind is visible;
//! * `same_inputs_and_rule_hash_yield_byte_equal_results` asserts **both**
//!   halves the contract requires -- equal bytes under one rule-set hash, and
//!   *different* bytes under another with an identical result. Without the
//!   second half the first would pass on an encoding that ignored the hash.
//!
//! The corpus is synthetic end to end: every identifier is a SHA-256 of its own
//! name, every instant is an offset from `P2-N5`'s `ORIGIN`, and the only files
//! opened at run time are the committed corpus and the design document.

#[path = "corpus/mod.rs"]
mod corpus;

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_critical_path::{
    BENEFIT_COMPONENTS, CONSTRAINTS, COST_COMPONENTS, CRITICAL_PATH_CORPUS_ROOT, ConceptEstimate,
    CostComponent, DISCLOSURE_GROUPS, STAGE_RULES, frozen_inputs, outcome, plan,
};
use academic_domain::engines::{
    ENGINE_REGISTRY, EngineVersion, FrozenInputs, HARNESS_ROOT, ProofStatus, RuleSetHash,
};

use corpus::{
    CORPUS_ENGINE_LABEL, ENGINE_VERSION, bounds_text, cases, corpus_files, evaluate, ruleset_text,
};

use corpus::common::{
    Scenario, TestResult, cost_except, disk_page, flat_benefit, flat_estimates, other_rule_set,
    permissive_constraints, reading_for, rule_set, section_36_4_gap, unmeasured, with_estimate,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

// ---------------------------------------------------------------------------
// The contract.
// ---------------------------------------------------------------------------

/// `P2-C5`'s own named assertion, both halves.
#[test]
fn same_inputs_and_rule_hash_yield_byte_equal_results() -> TestResult {
    let gap_case = section_36_4_gap()?;
    let version = EngineVersion::new(ENGINE_VERSION)?;
    let hash = RuleSetHash::new(rule_set());
    let other = RuleSetHash::new(other_rule_set());

    for case in cases()? {
        let first = evaluate(&gap_case, &case, hash, version)?;
        let second = evaluate(&gap_case, &case, hash, version)?;

        // The same request produces the same frozen inputs.
        assert_eq!(
            first.inputs.canonical_text(),
            second.inputs.canonical_text(),
            "{}: the frozen inputs are not a function of the request",
            case.name
        );
        assert_eq!(first.inputs.digest(), second.inputs.digest());

        // And the same bytes.
        assert_eq!(
            first.bytes, second.bytes,
            "{}: two evaluations of one case disagree",
            case.name
        );

        // The second half: a different rule-set hash produces different bytes
        // with an identical result.
        let under_other = evaluate(&gap_case, &case, other, version)?;
        assert_ne!(
            first.bytes, under_other.bytes,
            "{}: the rule-set hash does not reach the canonical bytes",
            case.name
        );

        // A different engine version likewise.
        let under_next = evaluate(
            &gap_case,
            &case,
            hash,
            EngineVersion::new(ENGINE_VERSION + 1)?,
        )?;
        assert_ne!(
            first.bytes, under_next.bytes,
            "{}: the engine version does not reach the canonical bytes",
            case.name
        );
    }
    Ok(())
}

/// Every committed case is what the real engine produces, byte for byte.
#[test]
fn the_committed_corpus_matches_a_fresh_render() -> TestResult {
    let root = workspace_root();
    let rendered = corpus_files()?;
    assert!(
        rendered.len() >= 11,
        "the builder rendered only {} files; the corpus is larger than that",
        rendered.len()
    );
    for file in &rendered {
        let path = root.join(&file.path);
        let committed = fs::read(&path).map_err(|error| format!("{}: {error}", file.path))?;
        assert_eq!(
            committed, file.bytes,
            "{} differs from a fresh render; re-run \
             `cargo run -p academic-critical-path --example emit_corpus` and explain \
             the semantic change",
            file.path
        );
    }

    // Nothing else sits in the corpus directory: a stale case left behind would
    // otherwise be invisible.
    let committed: BTreeSet<String> = walk(&root.join(CRITICAL_PATH_CORPUS_ROOT), &root)?;
    let expected: BTreeSet<String> = rendered.iter().map(|file| file.path.clone()).collect();
    assert_eq!(
        committed, expected,
        "the corpus directory holds files the builder does not render"
    );
    Ok(())
}

fn walk(
    directory: &std::path::Path,
    root: &std::path::Path,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            found.extend(walk(&path, root)?);
        } else {
            found.insert(
                path.strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(found)
}

/// The proof tree names every section 16 stage and every section 16.3
/// constraint, and the explanation renders them.
#[test]
fn the_proof_tree_names_every_stage_and_every_constraint() -> TestResult {
    let scenario = Scenario::new()?;
    let request = scenario.request();
    let result = plan(&request)?;
    let inputs = frozen_inputs(&request)?;
    let answer = outcome(&result, &inputs)?;

    answer.proof_tree.validate(&inputs)?;
    let rendered = answer.explanation_snapshot.as_str();
    for rule in STAGE_RULES {
        assert!(
            rendered.contains(rule),
            "the explanation does not name the {rule} stage"
        );
    }
    for constraint in CONSTRAINTS {
        assert!(
            rendered.contains(constraint.as_str()),
            "the explanation does not name {}",
            constraint.as_str()
        );
    }
    assert_eq!(answer.result.status, ProofStatus::Satisfied);
    assert!(rendered.ends_with('\n'));
    assert!(
        !rendered.contains(" \n"),
        "the explanation has trailing whitespace"
    );
    assert!(
        !rendered.contains('\r'),
        "the explanation carries a platform newline"
    );
    Ok(())
}

/// A refused run renders as refused, and an unmeasured cost renders as unknown.
///
/// Without this the snapshot above would be the only shape ever observed, and a
/// tree that answered `SATISFIED` to everything would still pass it.
#[test]
fn a_refused_run_and_an_unmeasured_cost_render_differently() -> TestResult {
    let gap_case = section_36_4_gap()?;
    let all_cases = cases()?;
    let refused = all_cases
        .iter()
        .find(|case| case.name == "no_feasible_route")
        .ok_or("the corpus has no refused case")?;
    let request = academic_critical_path::PlanRequest {
        gap_case: &gap_case,
        graph: &refused.graph,
        estimates: &refused.estimates,
        constraints: &refused.constraints,
        slider: &refused.slider,
        rule_set_hash: rule_set(),
        engine_version: ENGINE_VERSION,
    };
    let result = plan(&request)?;
    let inputs = frozen_inputs(&request)?;
    let answer = outcome(&result, &inputs)?;
    assert_eq!(answer.result.status, ProofStatus::NotSatisfied);
    assert!(result.ranked().is_empty());
    assert!(
        answer
            .explanation_snapshot
            .as_str()
            .contains("NOT_SATISFIED section-16"),
        "a refused run renders as satisfied"
    );

    // An unmeasured axis reaches the frozen inputs as `unknown`, which is
    // `P2-C5`'s value and not a zero.
    let scenario = Scenario::new()?;
    let estimates = with_estimate(
        flat_estimates()?,
        ConceptEstimate {
            concept: disk_page(),
            cost: cost_except(
                10,
                CostComponent::LearningEffort,
                unmeasured(CostComponent::LearningEffort, 30, 90)?,
            )?,
            benefit: flat_benefit(10)?,
            options: vec![reading_for(disk_page(), "dp")?],
        },
    );
    let unmeasured_request = academic_critical_path::PlanRequest {
        estimates: &estimates,
        ..scenario.request()
    };
    let text = frozen_inputs(&unmeasured_request)?.canonical_text();
    assert!(
        text.contains("learning_effort.basis=unknown"),
        "an unmeasured basis did not reach the frozen inputs as unknown"
    );
    assert!(
        text.contains("refresh_effort.basis=int:4"),
        "a measured basis did not reach the frozen inputs as a family count"
    );
    Ok(())
}

/// The frozen inputs are `P2-C5`'s canonical encoding and nothing else.
#[test]
fn the_frozen_inputs_round_trip_through_the_harness_encoding() -> TestResult {
    let scenario = Scenario::new()?;
    let request = scenario.request();
    let inputs = frozen_inputs(&request)?;
    let text = inputs.canonical_text();
    let parsed = FrozenInputs::parse(&text)?;
    assert_eq!(parsed, inputs);
    assert_eq!(parsed.digest(), inputs.digest());

    // Keys are strictly ascending and every value carries a type tag.
    let mut previous: Option<&str> = None;
    for line in text.trim_end_matches('\n').split('\n') {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("a frozen input line has no separator: {line}"))?;
        assert!(
            previous.is_none_or(|earlier| earlier < key),
            "the frozen inputs are not in ascending key order at {key}"
        );
        previous = Some(key);
        assert!(
            value == "unknown"
                || value.starts_with("int:")
                || value.starts_with("dec:")
                || value.starts_with("ref:"),
            "a frozen input value has no type tag: {value}"
        );
    }

    // Two structurally different requests do not share a digest.
    let mut other = permissive_constraints();
    other.credit_limit = 18;
    let changed = frozen_inputs(&academic_critical_path::PlanRequest {
        constraints: &other,
        ..scenario.request()
    })?;
    assert_ne!(changed.digest(), inputs.digest());
    Ok(())
}

/// The premise of this whole file: section 16 is not one of `P2-C5`'s twelve.
///
/// Asserted against the specification and the registry rather than stated, so
/// an edit that adds a critical path row to section 28's table fails here and
/// the contract page's recorded reading is revisited.
#[test]
fn the_registry_does_not_hold_a_critical_path_engine() -> TestResult {
    let page = fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let start = page
        .find("## 28. Deterministic Engines")
        .ok_or("the design document has no section 28")?;
    let rest = &page[start..];
    let end = rest.find("\n## 29.").unwrap_or(rest.len());
    let table = &rest[..end];
    let rows: Vec<&str> = table
        .lines()
        .filter(|line| line.trim_start().starts_with('|'))
        .skip(2)
        .collect();
    assert_eq!(
        rows.len(),
        12,
        "section 28 no longer tabulates twelve engines"
    );
    assert!(
        !table.to_lowercase().contains("critical path"),
        "section 28 now names a critical path engine; the reading recorded in \
         docs/contracts/critical-path.md is stale and a registry row is due"
    );
    assert_eq!(ENGINE_REGISTRY.len(), 12);
    assert!(
        ENGINE_REGISTRY
            .iter()
            .all(|descriptor| !descriptor.engine_id.contains("CRITICAL")),
        "the registry now holds a critical path engine"
    );
    assert!(
        academic_domain::engines::EngineName::parse(CORPUS_ENGINE_LABEL).is_none(),
        "this crate's corpus label entered the registry"
    );

    // And nothing of this crate's sits under the registry's harness root.
    let harness_root = workspace_root().join(HARNESS_ROOT);
    let registered: BTreeSet<&str> = ENGINE_REGISTRY
        .iter()
        .map(|descriptor| descriptor.harness_dir)
        .collect();
    for entry in fs::read_dir(&harness_root)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        assert!(
            registered.contains(name.as_str()),
            "{name} is under the registry's harness root and is not a registered engine"
        );
    }
    assert!(
        !CRITICAL_PATH_CORPUS_ROOT.starts_with(HARNESS_ROOT),
        "this crate's corpus is under the registry's harness root"
    );
    Ok(())
}

/// The corpus's four cases are four different answers.
///
/// Without this the byte comparison above would pass on a corpus whose cases
/// were all the same shape.
#[test]
fn the_corpus_cases_are_not_all_the_same_shape() -> TestResult {
    let gap_case = section_36_4_gap()?;
    let hash = RuleSetHash::new(rule_set());
    let version = EngineVersion::new(ENGINE_VERSION)?;
    let mut digests = BTreeSet::new();
    let mut ranked_counts = BTreeSet::new();
    for case in cases()? {
        let request = academic_critical_path::PlanRequest {
            gap_case: &gap_case,
            graph: &case.graph,
            estimates: &case.estimates,
            constraints: &case.constraints,
            slider: &case.slider,
            rule_set_hash: rule_set(),
            engine_version: ENGINE_VERSION,
        };
        let result = plan(&request)?;
        ranked_counts.insert(result.ranked().len());
        digests.insert(evaluate(&gap_case, &case, hash, version)?.bytes);
    }
    assert_eq!(digests.len(), 4, "two corpus cases produce the same bytes");
    assert!(
        ranked_counts.len() >= 3,
        "the corpus never varies how many routes survive: {ranked_counts:?}"
    );
    Ok(())
}

/// The rule set and the bounds file are derived from this crate's own arrays.
#[test]
fn the_rule_set_names_every_stage_and_constraint() -> TestResult {
    let text = ruleset_text();
    let lines: Vec<&str> = text.trim_end_matches('\n').split('\n').collect();
    assert_eq!(lines.len(), STAGE_RULES.len() + CONSTRAINTS.len());
    for rule in STAGE_RULES {
        assert!(lines.contains(&rule));
    }
    for constraint in CONSTRAINTS {
        assert!(lines.contains(&constraint.as_str()));
    }

    // The bounds file measures the crate rather than restating numbers.
    let bounds = bounds_text();
    assert!(bounds.contains(&format!("cost_axes={}", COST_COMPONENTS.len())));
    assert!(bounds.contains(&format!("benefit_axes={}", BENEFIT_COMPONENTS.len())));
    assert!(bounds.contains(&format!("constraints={}", CONSTRAINTS.len())));
    assert!(bounds.contains(&format!("disclosure_groups={}", DISCLOSURE_GROUPS.len())));
    Ok(())
}
