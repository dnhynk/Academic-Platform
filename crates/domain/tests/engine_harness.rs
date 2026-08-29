//! Named acceptance evidence for the §3.9 deterministic engine harness.
//!
//! None of the twelve §28 engines is implemented yet and this task invents
//! none. Two of the named acceptance tests nevertheless need an engine to exist:
//! `same_inputs_and_rule_hash_yield_byte_equal_results` has to run one twice,
//! and the harness audit's `IMPLEMENTED` branch has to run against a complete
//! artifact set or the guard would never be observed to bite. [`Reference`]
//! below is that engine. It is test-only, it is deliberately not one of the
//! twelve, `reference_engine_is_not_registered` proves it, and it ships in no
//! product build.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use academic_domain::engines::{
    AdversePath, ArtifactClass, ENGINE_REGISTRY, ENGINE_REGISTRY_VERSION, EngineDescriptor,
    EngineHarnessArtifacts, EngineLifecycle, EngineName, EngineOutcome, EngineResult,
    EngineVersion, ExplanationSnapshot, FrozenInputs, HARNESS_ROOT, HarnessViolation,
    HighImpactPath, InputKey, InputValue, NodeId, ProofNode, ProofStatus, RuleId, RuleSetHash,
    SourceLocator, audit_engine_harness,
};
use academic_domain::{ArtifactId, ContentDigest, Decimal, EvidenceLocator};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const REGISTRY_SOURCE: &str = include_str!("../../../schemas/registry/engine-registry-v1.json");

/// The reference harness lives outside [`HARNESS_ROOT`] on purpose: everything
/// under that root belongs to a registered engine, and the reference engine is
/// not one.
const REFERENCE_ROOT: &str = "testdata/engine-harness-reference";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

// ---------------------------------------------------------------------------
// The reference engine
// ---------------------------------------------------------------------------

/// A test-only engine with no product meaning.
///
/// It evaluates two rules over frozen inputs and folds them into a root. The
/// point is not the arithmetic; it is that every proof status, the partial
/// failure path, and the byte-equality contract are exercised by something that
/// actually runs.
#[derive(Debug)]
struct Reference;

impl Reference {
    const ID: &'static str = "engine.reference.harness";
    const ROOT_NODE: &'static str = "n.root";
    const AGREEMENT_NODE: &'static str = "n.agreement";
    const THRESHOLD_NODE: &'static str = "n.threshold";
    const ROOT_RULE: &'static str = "rule.reference.root";
    const AGREEMENT_RULE: &'static str = "rule.source.agreement";
    const THRESHOLD_RULE: &'static str = "rule.threshold";

    /// A fixed synthetic artifact the proof tree's locators point into.
    fn artifact() -> Result<ArtifactId, Box<dyn std::error::Error>> {
        Ok(ArtifactId::from_str(
            "01900000-0000-7000-8000-0000000000a1",
        )?)
    }

    fn integer(inputs: &FrozenInputs, key: &InputKey) -> Option<Option<i64>> {
        match inputs.get(key) {
            Some(InputValue::Integer(value)) => Some(Some(*value)),
            Some(InputValue::Unknown) => Some(None),
            _ => None,
        }
    }

    fn reference(inputs: &FrozenInputs, key: &InputKey) -> Option<Option<String>> {
        match inputs.get(key) {
            Some(InputValue::Reference(value)) => Some(Some(value.clone())),
            Some(InputValue::Unknown) => Some(None),
            _ => None,
        }
    }
}

impl academic_domain::engines::DeterministicEngine for Reference {
    fn engine_id(&self) -> &'static str {
        Self::ID
    }

    fn engine_version(&self) -> EngineVersion {
        EngineVersion::MIN
    }

    fn evaluate(
        &self,
        inputs: &FrozenInputs,
        _rule_set_hash: RuleSetHash,
        _engine_version: EngineVersion,
    ) -> Result<EngineOutcome, academic_domain::engines::EngineError> {
        let value_key = InputKey::new("reference.value")?;
        let threshold_key = InputKey::new("reference.threshold")?;
        let left_key = InputKey::new("reference.source.a")?;
        let right_key = InputKey::new("reference.source.b")?;

        let mut children = Vec::new();
        let mut unevaluated = Vec::new();
        let mut values = BTreeMap::new();

        // `rule.source.agreement` — two admitted sources for the same fact.
        match (
            Self::reference(inputs, &left_key),
            Self::reference(inputs, &right_key),
        ) {
            (Some(left), Some(right)) => {
                let status = match (left, right) {
                    (Some(left), Some(right)) if left == right => ProofStatus::Satisfied,
                    (Some(_), Some(_)) => ProofStatus::Conflict,
                    _ => ProofStatus::Unknown,
                };
                children.push(ProofNode {
                    node_id: NodeId::new(Self::AGREEMENT_NODE)?,
                    rule_id: RuleId::new(Self::AGREEMENT_RULE)?,
                    status,
                    inputs: vec![left_key.clone(), right_key.clone()],
                    source_locators: Vec::new(),
                    children: Vec::new(),
                });
            }
            _ => unevaluated.push(RuleId::new(Self::AGREEMENT_RULE)?),
        }

        // `rule.threshold` — a quantified shortfall, not a bare boolean.
        match (
            Self::integer(inputs, &value_key),
            Self::integer(inputs, &threshold_key),
        ) {
            (Some(value), Some(threshold)) => {
                let status = match (value, threshold) {
                    (Some(value), Some(_)) if value < 0 => ProofStatus::NotSatisfied,
                    (Some(value), Some(threshold)) if value >= threshold => ProofStatus::Satisfied,
                    (Some(_), Some(_)) => ProofStatus::Needs,
                    _ => ProofStatus::Unknown,
                };
                if let (Some(value), Some(threshold)) = (value, threshold) {
                    values.insert(
                        "reference.margin".to_owned(),
                        Decimal::new(i128::from(value) - i128::from(threshold), 0)?,
                    );
                }
                children.push(ProofNode {
                    node_id: NodeId::new(Self::THRESHOLD_NODE)?,
                    rule_id: RuleId::new(Self::THRESHOLD_RULE)?,
                    status,
                    inputs: vec![threshold_key.clone(), value_key.clone()],
                    source_locators: vec![SourceLocator {
                        artifact_id: Self::artifact().map_err(|_| {
                            academic_domain::engines::EngineError::MalformedInput(
                                "reference artifact id",
                            )
                        })?,
                        locator: EvidenceLocator::Page { page_number: 1 },
                    }],
                    children: Vec::new(),
                });
            }
            _ => unevaluated.push(RuleId::new(Self::THRESHOLD_RULE)?),
        }

        children.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        let status = children
            .iter()
            .map(|child| child.status)
            .max()
            .unwrap_or(ProofStatus::Unknown);
        unevaluated.sort();

        let proof_tree = ProofNode {
            node_id: NodeId::new(Self::ROOT_NODE)?,
            rule_id: RuleId::new(Self::ROOT_RULE)?,
            status,
            inputs: Vec::new(),
            source_locators: Vec::new(),
            children,
        };
        EngineOutcome::new(
            EngineResult {
                status,
                values,
                unevaluated,
            },
            proof_tree,
            inputs,
        )
    }
}

/// The reference engine's published rule set, hashed into its rule-set hash.
fn reference_rule_set_hash() -> Result<RuleSetHash, Box<dyn std::error::Error>> {
    let text = fs::read(repository_root().join(REFERENCE_ROOT).join("ruleset.txt"))?;
    Ok(RuleSetHash::new(ContentDigest::sha256(&text)))
}

/// Evaluates one canonical input file under the reference rule set.
fn evaluate_case(text: &str, version: u16) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use academic_domain::engines::DeterministicEngine as _;

    let inputs = FrozenInputs::parse(text)?;
    let hash = reference_rule_set_hash()?;
    let version = EngineVersion::new(version)?;
    let outcome = Reference.evaluate(&inputs, hash, version)?;
    Ok(outcome.canonical_bytes(Reference::ID, hash, version, &inputs))
}

fn read_cases(directory: &Path) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "input")
        {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("case file has no stem")?
                .to_owned();
            cases.push((name, fs::read_to_string(&path)?));
        }
    }
    cases.sort();
    Ok(cases)
}

// ---------------------------------------------------------------------------
// Harness inventory collection
// ---------------------------------------------------------------------------

fn is_populated(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_some())
        }
        Ok(metadata) => metadata.len() > 0,
        Err(_) => false,
    }
}

/// Collects what an engine's harness directory actually holds.
fn collect_artifacts(root: &Path) -> EngineHarnessArtifacts {
    let mut classes = BTreeSet::new();
    for class in ArtifactClass::ALL {
        if is_populated(&root.join(class.path())) {
            classes.insert(class);
        }
    }
    let mut adverse = BTreeSet::new();
    for path in AdversePath::ALL {
        if is_populated(&root.join("adverse").join(path.directory())) {
            adverse.insert(path);
        }
    }
    EngineHarnessArtifacts {
        classes,
        adverse,
        directory_exists: root.exists(),
        implementation_sites: Vec::new(),
    }
}

/// Every workspace Rust source file, excluding this crate's own test tree.
fn workspace_sources(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, found)?;
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    walk(&root.join("crates"), &mut found)?;
    found.sort();
    Ok(found)
}

/// The synthetic descriptor the audit's `IMPLEMENTED` branch is exercised with.
///
/// The audit is name-agnostic, so the descriptor borrows a registered name and
/// points at the reference harness. Nothing here changes the real registry.
fn implemented_descriptor(high_impact: Option<HighImpactPath>) -> EngineDescriptor {
    EngineDescriptor {
        lifecycle: EngineLifecycle::Implemented,
        high_impact_path: high_impact,
        ..*EngineName::Gpa.descriptor()
    }
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// The §28 table, enumerated rather than counted.
///
/// t068 §3.9 calls this a "thirteen-engine registry". §28 tabulates twelve, and
/// the thirteenth t068 implies is the property sentence under the table — that a
/// published rule executes deterministically even when a model proposed it —
/// which has no inputs, no outputs, and no invariant of its own, and which the
/// twelve engines below are the subjects of. The specification is the source of
/// truth; the list below is checked against it rather than against t068.
const SECTION_28_ENGINES: [&str; 12] = [
    "GPA",
    "CREDIT_ACCOUNTING",
    "GRADUATION_AUDIT",
    "TIMETABLE",
    "OFFICIAL_PREREQUISITE",
    "EQUIVALENCY",
    "TRANSCRIPT_COVERAGE",
    "ARTIFACT_INTEGRITY",
    "REPOSITORY_DIFF",
    "OVERRIDE_RESOLVER",
    "PERMISSION_BROKER",
    "RETENTION_DELETION",
];

/// Reads the `Engine` cell of every §28 table row, in specification order.
fn section_28_table_engines() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let spec = fs::read_to_string(
        repository_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let start = spec
        .find("## 28. Deterministic Engines")
        .ok_or("§28 heading is missing")?;
    let end = spec
        .find("## 29. Data Ingestion")
        .ok_or("§29 heading is missing")?;
    Ok(spec[start..end]
        .lines()
        .filter(|line| line.starts_with("| "))
        .filter_map(|line| line.split('|').nth(1))
        .map(str::trim)
        .filter(|cell| *cell != "Engine")
        .map(|cell| {
            let screaming: String = cell
                .to_uppercase()
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() {
                        character
                    } else {
                        '_'
                    }
                })
                .collect();
            screaming.trim_matches('_').to_owned()
        })
        .collect())
}

#[test]
fn engine_registry_is_complete() -> TestResult {
    // The registry is the §28 table and nothing else. Enumerating rather than
    // counting is what makes both directions fail: dropping a tabulated engine
    // and registering one the table does not name are the same mismatch.
    assert_eq!(
        section_28_table_engines()?,
        SECTION_28_ENGINES,
        "the §28 table no longer holds exactly the engines this registry names"
    );
    assert_eq!(
        EngineName::ALL.map(EngineName::as_str).to_vec(),
        SECTION_28_ENGINES.to_vec(),
        "the registered engines are not the §28 table"
    );

    let source: serde_json::Value = serde_json::from_str(REGISTRY_SOURCE)?;
    let rows = source["engines"]
        .as_array()
        .ok_or("registry must list engines")?;
    assert_eq!(
        rows.iter()
            .map(|row| row["name"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        SECTION_28_ENGINES.to_vec(),
        "the registry source is not the §28 table"
    );
    assert_eq!(ENGINE_REGISTRY.len(), SECTION_28_ENGINES.len());
    assert_eq!(
        ENGINE_REGISTRY_VERSION,
        u16::try_from(source["registry_version"].as_u64().ok_or("version")?)?
    );

    let mut ids = BTreeSet::new();
    for (index, engine) in EngineName::ALL.into_iter().enumerate() {
        let descriptor = engine.descriptor();
        let row = &rows[index];

        assert_eq!(descriptor.name, engine, "registry is indexed by name");
        assert_eq!(index, engine as usize, "index must equal discriminant");
        assert_eq!(Some(descriptor.engine_id), row["engine_id"].as_str());
        assert_eq!(
            descriptor.requirement_id,
            format!("REQ-28-{:03}", index + 1),
            "{} must close its own §28 requirement",
            engine.as_str()
        );
        assert!(
            ids.insert(descriptor.engine_id),
            "{} reuses an engine id",
            engine.as_str()
        );
        assert_eq!(
            EngineName::parse(engine.as_str()),
            Some(engine),
            "{} must resolve from its own name",
            engine.as_str()
        );
    }

    // Every registered engine is PLANNED today: this task builds the harness,
    // the registry, and the CI enforcement, and implements no engine.
    assert!(
        ENGINE_REGISTRY
            .iter()
            .all(|descriptor| descriptor.lifecycle == EngineLifecycle::Planned),
        "an engine became IMPLEMENTED without its harness artifacts landing here"
    );

    // The high-impact four are exactly the §3.9 set, one engine per path.
    let mut paths: Vec<HighImpactPath> = ENGINE_REGISTRY
        .iter()
        .filter_map(|descriptor| descriptor.high_impact_path)
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        HighImpactPath::ALL.to_vec(),
        "GPA, graduation, deletion, and egress must each be carried by exactly one engine"
    );
    assert_eq!(
        EngineName::Gpa.descriptor().high_impact_path,
        Some(HighImpactPath::Gpa)
    );
    assert_eq!(
        EngineName::GraduationAudit.descriptor().high_impact_path,
        Some(HighImpactPath::Graduation)
    );
    assert_eq!(
        EngineName::RetentionDeletion.descriptor().high_impact_path,
        Some(HighImpactPath::Deletion)
    );
    assert_eq!(
        EngineName::PermissionBroker.descriptor().high_impact_path,
        Some(HighImpactPath::Egress)
    );

    // Nothing unregistered may hide under the harness root.
    let root = repository_root().join(HARNESS_ROOT);
    if root.exists() {
        let registered: BTreeSet<&str> = ENGINE_REGISTRY
            .iter()
            .map(|descriptor| descriptor.harness_dir)
            .collect();
        for entry in fs::read_dir(&root)? {
            let name = entry?.file_name();
            let name = name
                .to_str()
                .ok_or("harness entry is not UTF-8")?
                .to_owned();
            assert!(
                registered.contains(name.as_str()),
                "{name} is under the harness root and is not a registered engine"
            );
        }
    }
    Ok(())
}

#[test]
fn reference_engine_is_not_registered() {
    assert!(
        EngineName::parse("REFERENCE_HARNESS").is_none(),
        "the reference engine must never enter the registry"
    );
    assert!(
        ENGINE_REGISTRY
            .iter()
            .all(|descriptor| descriptor.engine_id != Reference::ID),
        "the reference engine must never claim a registered engine id"
    );
}

#[test]
fn engine_without_golden_fixtures_fails_ci() -> TestResult {
    assert_missing_class_bites(ArtifactClass::GoldenFixtures)
}

#[test]
fn engine_without_property_tests_fails_ci() -> TestResult {
    assert_missing_class_bites(ArtifactClass::PropertyTests)
}

#[test]
fn engine_without_version_compat_fixtures_fails_ci() -> TestResult {
    assert_missing_class_bites(ArtifactClass::VersionCompatFixtures)
}

#[test]
fn engine_without_explanation_snapshot_fails_ci() -> TestResult {
    assert_missing_class_bites(ArtifactClass::ExplanationSnapshot)
}

/// Removing one artifact class from a real, complete inventory must produce
/// exactly one violation, and putting it back must produce none.
fn assert_missing_class_bites(class: ArtifactClass) -> TestResult {
    let registry = [implemented_descriptor(None)];
    let complete = collect_artifacts(&repository_root().join(REFERENCE_ROOT));
    assert_eq!(
        complete.classes.len(),
        ArtifactClass::ALL.len(),
        "the reference harness must ship every artifact class or this test is vacuous"
    );

    let discovered = BTreeMap::from([(EngineName::Gpa, complete.clone())]);
    assert_eq!(
        audit_engine_harness(&registry, &discovered),
        Vec::new(),
        "a complete harness must raise no violation"
    );

    let mut damaged = complete;
    damaged.classes.remove(&class);
    let discovered = BTreeMap::from([(EngineName::Gpa, damaged)]);
    assert_eq!(
        audit_engine_harness(&registry, &discovered),
        vec![HarnessViolation::MissingArtifactClass {
            engine: EngineName::Gpa,
            class,
        }],
        "removing {} must fail the audit",
        class.as_str()
    );
    Ok(())
}

#[test]
fn high_impact_engines_cover_unknown_conflict_partial() -> TestResult {
    let registry = [implemented_descriptor(Some(HighImpactPath::Gpa))];
    let root = repository_root().join(REFERENCE_ROOT);
    let complete = collect_artifacts(&root);
    assert_eq!(
        complete.adverse.len(),
        AdversePath::ALL.len(),
        "the reference harness must ship every adverse path or this test is vacuous"
    );

    let discovered = BTreeMap::from([(EngineName::Gpa, complete.clone())]);
    assert_eq!(
        audit_engine_harness(&registry, &discovered),
        Vec::new(),
        "a high-impact engine with all three adverse paths must raise no violation"
    );

    for path in AdversePath::ALL {
        let mut damaged = complete.clone();
        damaged.adverse.remove(&path);
        let discovered = BTreeMap::from([(EngineName::Gpa, damaged)]);
        assert_eq!(
            audit_engine_harness(&registry, &discovered),
            vec![HarnessViolation::MissingAdversePath {
                engine: EngineName::Gpa,
                path,
            }],
            "removing the {} fixture must fail the audit",
            path.as_str()
        );
    }

    // An engine that is not high impact keeps the same three fixtures optional,
    // so the rule is the high-impact one and not a blanket requirement.
    let mut stripped = complete;
    stripped.adverse.clear();
    let discovered = BTreeMap::from([(EngineName::Gpa, stripped)]);
    assert_eq!(
        audit_engine_harness(&[implemented_descriptor(None)], &discovered),
        Vec::new()
    );

    // Each adverse fixture is executable, not a placeholder: it runs and lands
    // on the outcome its directory names.
    let hash = reference_rule_set_hash()?;
    for path in AdversePath::ALL {
        let cases = read_cases(&root.join("adverse").join(path.directory()))?;
        assert!(!cases.is_empty(), "{} has no case", path.as_str());
        for (name, text) in cases {
            let inputs = FrozenInputs::parse(&text)?;
            let version = EngineVersion::new(1)?;
            let outcome = {
                use academic_domain::engines::DeterministicEngine as _;
                Reference.evaluate(&inputs, hash, version)?
            };
            match path {
                AdversePath::Unknown => assert_eq!(
                    outcome.result.status,
                    ProofStatus::Unknown,
                    "{name} must land on UNKNOWN"
                ),
                AdversePath::Conflict => assert_eq!(
                    outcome.result.status,
                    ProofStatus::Conflict,
                    "{name} must land on CONFLICT"
                ),
                AdversePath::PartialFailure => assert!(
                    outcome.result.is_partial_failure(),
                    "{name} must leave a rule unevaluated"
                ),
            }
            let expected = fs::read(
                root.join("adverse")
                    .join(path.directory())
                    .join(format!("{name}.expected")),
            )?;
            assert_eq!(
                outcome.canonical_bytes(Reference::ID, hash, version, &inputs),
                expected,
                "{name} drifted from its recorded adverse outcome"
            );
        }
    }
    Ok(())
}

#[test]
fn planned_engine_that_gains_an_implementation_fails_ci() -> TestResult {
    // The real registry with the real inventory: twelve planned engines, no
    // harness artifacts, and no workspace source naming any engine id.
    let root = repository_root();
    let mut discovered = BTreeMap::new();
    for descriptor in &ENGINE_REGISTRY {
        let mut artifacts =
            collect_artifacts(&root.join(HARNESS_ROOT).join(descriptor.harness_dir));
        artifacts.implementation_sites = workspace_sources(&root)?
            .into_iter()
            .filter(|path| {
                fs::read_to_string(path).is_ok_and(|source| source.contains(descriptor.engine_id))
            })
            .filter(|path| !path.ends_with("generated.rs"))
            .map(|path| path.display().to_string())
            .collect();
        discovered.insert(descriptor.name, artifacts);
    }
    assert_eq!(
        audit_engine_harness(&ENGINE_REGISTRY, &discovered),
        Vec::new(),
        "the committed registry and tree must agree"
    );

    // Inject an implementation for one planned engine and observe the bite.
    let mut injected = discovered.clone();
    injected
        .entry(EngineName::GraduationAudit)
        .or_default()
        .implementation_sites
        .push("crates/domain/src/injected.rs".to_owned());
    assert_eq!(
        audit_engine_harness(&ENGINE_REGISTRY, &injected),
        vec![HarnessViolation::PlannedEngineHasImplementation {
            engine: EngineName::GraduationAudit,
            site: "crates/domain/src/injected.rs".to_owned(),
        }],
    );

    // Inject harness artifacts for a planned engine and observe the same.
    let mut injected = discovered;
    injected
        .entry(EngineName::Gpa)
        .or_default()
        .classes
        .insert(ArtifactClass::GoldenFixtures);
    assert_eq!(
        audit_engine_harness(&ENGINE_REGISTRY, &injected),
        vec![HarnessViolation::PlannedEngineHasArtifacts {
            engine: EngineName::Gpa
        }],
    );
    Ok(())
}

#[test]
fn same_inputs_and_rule_hash_yield_byte_equal_results() -> TestResult {
    let root = repository_root().join(REFERENCE_ROOT);
    let hash = reference_rule_set_hash()?;
    let version = EngineVersion::new(1)?;

    for (name, text) in read_cases(&root.join("golden"))? {
        // Two independently parsed input sets and two engine instances.
        let first = FrozenInputs::parse(&text)?;
        let second = FrozenInputs::parse(&text)?;
        assert_eq!(first.digest(), second.digest());

        let (left, right) = {
            use academic_domain::engines::DeterministicEngine as _;
            (
                Reference.evaluate(&first, hash, version)?,
                Reference.evaluate(&second, hash, version)?,
            )
        };
        let left_bytes = left.canonical_bytes(Reference::ID, hash, version, &first);
        let right_bytes = right.canonical_bytes(Reference::ID, hash, version, &second);
        assert_eq!(left_bytes, right_bytes, "{name} is not byte-reproducible");

        // Against the committed golden bytes, not merely against itself.
        let expected = fs::read(root.join("golden").join(format!("{name}.expected")))?;
        assert_eq!(left_bytes, expected, "{name} drifted from its golden bytes");

        // Insertion order must not reach the bytes.
        let mut pairs: Vec<(InputKey, InputValue)> = first
            .keys()
            .map(|key| {
                first
                    .get(key)
                    .map(|value| (key.clone(), value.clone()))
                    .ok_or("declared key must resolve")
            })
            .collect::<Result<_, _>>()?;
        pairs.reverse();
        let shuffled = FrozenInputs::new(pairs)?;
        assert_eq!(shuffled.digest(), first.digest());
        let shuffled_outcome = {
            use academic_domain::engines::DeterministicEngine as _;
            Reference.evaluate(&shuffled, hash, version)?
        };
        assert_eq!(
            shuffled_outcome.canonical_bytes(Reference::ID, hash, version, &shuffled),
            left_bytes,
            "{name} depends on input insertion order"
        );

        // The assertion is not vacuous: a different rule-set hash must change
        // the bytes even though the result is identical.
        let other = RuleSetHash::new(ContentDigest::sha256(b"a different rule set"));
        let under_other = {
            use academic_domain::engines::DeterministicEngine as _;
            Reference.evaluate(&first, other, version)?
        };
        assert_eq!(under_other.result, left.result);
        assert_ne!(
            under_other.canonical_bytes(Reference::ID, other, version, &first),
            left_bytes,
            "{name} does not bind its bytes to the rule-set hash"
        );
    }
    Ok(())
}

#[test]
fn version_compatibility_fixtures_replay_across_the_declared_window() -> TestResult {
    let root = repository_root()
        .join(REFERENCE_ROOT)
        .join("version-compat");
    let cases = read_cases(&root)?;
    assert!(!cases.is_empty(), "the version window must be exercised");

    for (name, text) in cases {
        let recorded = fs::read_to_string(root.join(format!("{name}.explanation")))?;
        // The declared window: the explanation an earlier engine version
        // produced must still be produced by every later admitted version.
        for version in 1..=2 {
            let bytes = evaluate_case(&text, version)?;
            let rendered = String::from_utf8(bytes)?;
            let explanation = rendered.split_inclusive('\n').skip(4).collect::<String>();
            assert_eq!(
                explanation, recorded,
                "{name} changed meaning between engine versions"
            );
        }
    }
    Ok(())
}

#[test]
fn explanation_snapshot_is_normalized_and_stable() -> TestResult {
    let root = repository_root().join(REFERENCE_ROOT);
    let recorded = fs::read_to_string(root.join("explanation.snapshot"))?;
    let text = fs::read_to_string(root.join("golden").join("needs.input"))?;
    let inputs = FrozenInputs::parse(&text)?;
    let outcome = {
        use academic_domain::engines::DeterministicEngine as _;
        Reference.evaluate(&inputs, reference_rule_set_hash()?, EngineVersion::new(1)?)?
    };
    assert_eq!(outcome.explanation_snapshot.as_str(), recorded);

    // Normalization is what makes the snapshot comparable at all.
    assert!(!recorded.contains('\r'), "snapshots are LF only");
    assert!(recorded.ends_with('\n'), "snapshots end with a newline");
    for line in recorded.lines() {
        assert_eq!(line.trim_end(), line, "a snapshot line has trailing space");
    }

    // A snapshot rendered from the outcome cannot disagree with the outcome:
    // `EngineOutcome::new` renders it rather than accepting one.
    assert_eq!(
        outcome.explanation_snapshot,
        ExplanationSnapshot::render(&outcome.result, &outcome.proof_tree)
    );
    Ok(())
}

#[test]
fn malformed_inputs_return_typed_errors_and_never_panic() -> TestResult {
    use academic_domain::engines::EngineError;

    for malformed in [
        "reference.value",
        "reference.value=\n",
        "reference.value=int:007\n",
        "reference.value=int:+7\n",
        "reference.value=dec:1/19\n",
        "reference.value=dec:1\n",
        "reference.value=ref:has space\n",
        "reference.value=float:1.5\n",
        "reference.value=int:1",
        "b=int:1\na=int:1\n",
        "a=int:1\na=int:2\n",
        "référence=int:1\n",
    ] {
        assert!(
            matches!(
                FrozenInputs::parse(malformed),
                Err(EngineError::MalformedInput(_) | EngineError::InvalidIdentifier { .. })
                    | Err(EngineError::Domain(_))
            ),
            "{malformed:?} must return a typed error"
        );
    }

    Ok(())
}

/// A well-formed input the engine cannot use is reported, never guessed at.
#[test]
fn unusable_input_is_reported_rather_than_folded_into_a_verdict() -> TestResult {
    use academic_domain::engines::DeterministicEngine as _;

    let inputs = FrozenInputs::parse("reference.value=ref:not.an.integer\n")?;
    let outcome = Reference.evaluate(
        &inputs,
        RuleSetHash::new(ContentDigest::sha256(b"")),
        EngineVersion::MIN,
    )?;
    assert!(outcome.result.is_partial_failure());
    assert_eq!(outcome.result.status, ProofStatus::Unknown);
    Ok(())
}

#[test]
fn proof_tree_node_shape_is_fixed() -> TestResult {
    use academic_domain::engines::EngineError;

    assert_eq!(
        ProofStatus::ALL.map(ProofStatus::as_str),
        ["SATISFIED", "NEEDS", "NOT_SATISFIED", "UNKNOWN", "CONFLICT"],
        "the §3.9 status set is fixed"
    );

    let inputs = FrozenInputs::parse("a.one=int:1\n")?;
    let leaf = |node: &str, status| -> Result<ProofNode, EngineError> {
        Ok(ProofNode {
            node_id: NodeId::new(node)?,
            rule_id: RuleId::new("rule.leaf")?,
            status,
            inputs: vec![InputKey::new("a.one")?],
            source_locators: Vec::new(),
            children: Vec::new(),
        })
    };

    // A node reading an input the frozen set does not declare is rejected.
    let mut orphan = leaf("n.a", ProofStatus::Satisfied)?;
    orphan.inputs = vec![InputKey::new("a.missing")?];
    assert_eq!(
        orphan.validate(&inputs),
        Err(EngineError::UndeclaredInput("a.missing".to_owned()))
    );

    // Duplicate node identifiers are rejected.
    let duplicated = ProofNode {
        children: vec![leaf("n.same", ProofStatus::Satisfied)?],
        ..leaf("n.same", ProofStatus::Satisfied)?
    };
    assert_eq!(
        duplicated.validate(&inputs),
        Err(EngineError::DuplicateNodeId("n.same".to_owned()))
    );

    // Unordered children are rejected, because ordering is what makes the
    // canonical bytes a function of the tree rather than of the walk.
    let unordered = ProofNode {
        children: vec![
            leaf("n.b", ProofStatus::Satisfied)?,
            leaf("n.a", ProofStatus::Satisfied)?,
        ],
        ..leaf("n.root", ProofStatus::Satisfied)?
    };
    assert_eq!(
        unordered.validate(&inputs),
        Err(EngineError::UnorderedProofField {
            node_id: "n.root".to_owned(),
            field: "children",
        })
    );

    // A SATISFIED result cannot rest on a tree containing a CONFLICT.
    let conflicted = ProofNode {
        children: vec![leaf("n.a", ProofStatus::Conflict)?],
        ..leaf("n.root", ProofStatus::Satisfied)?
    };
    assert_eq!(
        EngineOutcome::new(
            EngineResult {
                status: ProofStatus::Satisfied,
                values: BTreeMap::new(),
                unevaluated: Vec::new(),
            },
            conflicted,
            &inputs,
        )
        .err(),
        Some(EngineError::SatisfiedOverConflict)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Property-based evidence
// ---------------------------------------------------------------------------

/// Reads the generator bounds the property corpus fixes.
fn property_bounds() -> Result<(i64, i64, Vec<String>), Box<dyn std::error::Error>> {
    let text = fs::read_to_string(
        repository_root()
            .join(REFERENCE_ROOT)
            .join("property")
            .join("bounds.txt"),
    )?;
    let mut low = 0;
    let mut high = 0;
    let mut references = Vec::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or("bounds line has no '='")?;
        match key {
            "integer.low" => low = value.parse()?,
            "integer.high" => high = value.parse()?,
            "reference" => references.push(value.to_owned()),
            _ => return Err(format!("unknown bounds key {key}").into()),
        }
    }
    Ok((low, high, references))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Over the whole declared generator space the engine never panics, always
    /// produces a tree that validates against the inputs it was given, and is
    /// byte-reproducible.
    #[test]
    fn reference_engine_is_total_and_reproducible(
        value in -1000i64..1000,
        threshold in -1000i64..1000,
        left in 0usize..3,
        right in 0usize..3,
        value_known in any::<bool>(),
    ) {
        use academic_domain::engines::DeterministicEngine as _;

        let (low, high, references) = property_bounds().map_err(|error| {
            TestCaseError::fail(error.to_string())
        })?;
        let value = value.clamp(low, high);
        let threshold = threshold.clamp(low, high);
        let render = |index: usize| references.get(index).map_or_else(
            || "unknown".to_owned(),
            |name| format!("ref:{name}"),
        );
        let text = format!(
            "reference.source.a={}\nreference.source.b={}\nreference.threshold=int:{threshold}\nreference.value={}\n",
            render(left),
            render(right),
            if value_known { format!("int:{value}") } else { "unknown".to_owned() },
        );

        let inputs = FrozenInputs::parse(&text).map_err(|error| {
            TestCaseError::fail(error.to_string())
        })?;
        let hash = RuleSetHash::new(ContentDigest::sha256(text.as_bytes()));
        let version = EngineVersion::MIN;
        let first = Reference.evaluate(&inputs, hash, version).map_err(|error| {
            TestCaseError::fail(error.to_string())
        })?;
        let second = Reference.evaluate(&inputs, hash, version).map_err(|error| {
            TestCaseError::fail(error.to_string())
        })?;
        prop_assert_eq!(
            first.canonical_bytes(Reference::ID, hash, version, &inputs),
            second.canonical_bytes(Reference::ID, hash, version, &inputs)
        );
        prop_assert!(first.proof_tree.validate(&inputs).is_ok());

        // An input that is not known never becomes a pass or a fail.
        let unknown_read = first.proof_tree.walk().iter().any(|node| {
            node.inputs.iter().any(|key| {
                matches!(inputs.get(key), Some(InputValue::Unknown))
            })
        });
        if unknown_read {
            prop_assert!(matches!(
                first.result.status,
                ProofStatus::Unknown | ProofStatus::Conflict
            ));
        }
    }
}
