//! `P2-R2`'s named acceptance evidence.
//!
//! Every corpus here is synthetic and built in process. No repository is
//! committed, no file is written outside a `SourceTree::Entries` list held in
//! memory, and every analysis runs over a snapshot this file froze through
//! `P2-R1`'s own `capture_local` — so the gate ran, the paths were classified,
//! and the bytes were sealed as `P2-G5` untrusted content before anything here
//! read them.

use std::{collections::BTreeMap, error::Error};

use academic_model_run::{
    CalibrationBin, CalibrationDataset, CalibrationDatasetId, CalibrationRegistry, Digest32,
    ModelVersion, ProviderId, Purpose,
};
use academic_policy::ContentDigest;
use academic_repository::{
    CommitId, PathPolicy, RepositoryId, RepositorySnapshot, RepositorySource, SnapshotRequest,
    SourceEntry, SourceTree, ToolVersion, WorkingTreeFacts, capture_local,
};
use academic_repository_analysis::{
    AnalysisError, AnalysisInput, AnalyzerIdentity, ArtifactScope, ComponentError, ComponentId,
    CoverageGapReason, CoverageOutcome, EvidenceLadder, EvidenceStrength, EvidenceTier,
    ExclusionReason, FileKind, Finding, FindingScope, IndexKind, LadderRung, PathClass,
    RepositoryAnalysis, RuntimeTrace, SourceUnit, Subject, SubjectId, Support, analyze, support,
};
use academic_untrusted_content::SourceIndex;

type TestResult = Result<(), Box<dyn Error>>;

const CAPTURED_AT: u64 = 1_756_000_000_000;
const HEAD: &str = "abc1234def5678";
const ANALYZER: &str = "academic-repository-analysis";
const ANALYZER_VERSION: &str = "0.1.0";
const NOW: u64 = 5_000;

// ---------------------------------------------------------------------------
// The deterministic harness.
// ---------------------------------------------------------------------------

/// Captures a synthetic tree through `P2-R1` and analyzes what it froze.
///
/// The units are built from the frozen manifest rather than from the input
/// list, so a path the gate excluded is not offered to the analyzer at all —
/// which is the composition `P2-R1`'s `analyzer_never_sees_an_excluded_path`
/// makes true one layer down.
fn analyzed(
    files: &[(&str, &str)],
) -> Result<(RepositorySnapshot, RepositoryAnalysis), Box<dyn Error>> {
    let (snapshot, sealed) = captured(files)?;
    let identity = AnalyzerIdentity::new(ANALYZER, ANALYZER_VERSION)?;
    let input = AnalysisInput::of(&snapshot, &sealed, identity, source_units(&snapshot, files))?;
    let analysis = analyze(&input)?;
    Ok((snapshot, analysis))
}

/// Captures a synthetic tree through `P2-R1` and hands back what it froze.
fn captured(files: &[(&str, &str)]) -> Result<(RepositorySnapshot, SourceIndex), Box<dyn Error>> {
    let entries: Vec<SourceEntry> = files
        .iter()
        .map(|(path, body)| SourceEntry::new(*path, body.as_bytes().to_vec()))
        .collect();
    let facts = WorkingTreeFacts::checkout(
        CommitId::new(HEAD)?,
        Some("main".to_owned()),
        files.iter().map(|(path, _)| (*path).to_owned()).collect(),
        Vec::new(),
        Vec::new(),
    );
    let policy = PathPolicy::new();
    let request = SnapshotRequest {
        repository: RepositoryId::new("repo_A")?,
        source: RepositorySource::LocalDirectory,
        tree: SourceTree::Entries(&entries),
        facts: &facts,
        policy: &policy,
        captured_at: CAPTURED_AT,
        parent_snapshots: Vec::new(),
        submodule_refs: Vec::new(),
        analysis_policy_hash: ContentDigest::of(b"analysis-policy-v1"),
        tool_versions: vec![ToolVersion::new(ANALYZER, ANALYZER_VERSION)?],
    };
    let (capture, sealed) = capture_local(&request)?;
    Ok((capture.snapshot, sealed))
}

/// One unit per frozen manifest row, so a path the gate excluded is never
/// offered to the analyzer at all.
fn source_units(snapshot: &RepositorySnapshot, files: &[(&str, &str)]) -> Vec<SourceUnit> {
    let bodies: BTreeMap<&str, &str> = files.iter().copied().collect();
    snapshot
        .manifest()
        .iter()
        .map(|entry| SourceUnit::new(entry.path(), bodies[entry.path()].as_bytes().to_vec()))
        .collect()
}

/// A registry holding one fresh dataset for this analyzer.
///
/// The bins map the ladder's five-step raw scale onto permille. They are the
/// analyzer's calibration and not a weighting invented at the display site:
/// `CalibrationRegistry::interpret` is the only producer of a
/// `CalibratedConfidence` in this workspace, and `DisplayedConfidence::of`
/// takes one, so this dataset existing is what makes a number showable at all.
fn registry() -> Result<CalibrationRegistry, Box<dyn Error>> {
    let mut registry = CalibrationRegistry::new();
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-analysis-v1")?,
        ProviderId::new(ANALYZER)?,
        ModelVersion::new(ANALYZER_VERSION)?,
        purpose()?,
        Digest32::of(b"cal-analysis-v1"),
        512,
        0,
        10_000,
        vec![
            CalibrationBin::new(200, 150)?,
            CalibrationBin::new(400, 400)?,
            CalibrationBin::new(600, 650)?,
            CalibrationBin::new(800, 820)?,
            CalibrationBin::new(1_000, 930)?,
        ],
    )?)?;
    Ok(registry)
}

fn purpose() -> Result<Purpose, Box<dyn Error>> {
    Ok(Purpose::new("REPOSITORY_EVIDENCE_TIER")?)
}

fn redis() -> Result<Subject, Box<dyn Error>> {
    Ok(Subject::new(
        SubjectId::new("redis")?,
        &["redis"],
        &["redis"],
        &["redis_open"],
        &["redis"],
    ))
}

/// Classifies the one subject over one corpus, with no runtime trace.
fn classify(files: &[(&str, &str)]) -> Result<Vec<Finding>, Box<dyn Error>> {
    classify_with(files, &[])
}

fn classify_with(
    files: &[(&str, &str)],
    traces: &[RuntimeTrace],
) -> Result<Vec<Finding>, Box<dyn Error>> {
    let (_, analysis) = analyzed(files)?;
    Ok(EvidenceLadder::classify(
        &analysis,
        &redis()?,
        &registry()?,
        &purpose()?,
        traces,
        NOW,
    )?)
}

/// The one finding a single-component corpus produces.
fn only(findings: Vec<Finding>) -> Result<Finding, Box<dyn Error>> {
    assert_eq!(
        findings.len(),
        1,
        "expected exactly one finding, got {:?}",
        findings
            .iter()
            .map(|finding| (
                finding.scope().component().as_str().to_owned(),
                finding.rung()
            ))
            .collect::<Vec<_>>()
    );
    findings
        .into_iter()
        .next()
        .ok_or_else(|| "one finding".into())
}

// ---------------------------------------------------------------------------
// The five corpora, one per section 17.3 row.
// ---------------------------------------------------------------------------

const MANIFEST_ONLY: [(&str, &str); 2] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/orders.ts",
        "export function place() {\n  return 1;\n}\n",
    ),
];

const UNREACHABLE_IMPORT: [(&str, &str); 2] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n",
    ),
];

const REACHABLE_CALL_AND_CONFIG: [(&str, &str); 3] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    ("src/app.config.yaml", "cache:\n  redis: enabled\n"),
];

const TEST_ONLY_USE: [(&str, &str); 3] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"devDependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "tests/cache.test.ts",
        "import redis from \"redis\";\n\nexport function checkCache() {\n  return redis.createClient();\n}\n",
    ),
    ("tests/harness.config.yaml", "cache:\n  redis: enabled\n"),
];

// ---------------------------------------------------------------------------
// The eight named acceptance tests.
// ---------------------------------------------------------------------------

#[test]
fn dependency_only_is_present_not_observed() -> TestResult {
    let finding = only(classify(&MANIFEST_ONLY)?)?;

    assert_eq!(finding.rung(), LadderRung::ManifestPresence);
    assert_eq!(finding.tier(), EvidenceTier::PresentOnly);
    assert_eq!(finding.tier().as_str(), "PRESENT_ONLY");
    // Section 17.3's first row is `불가`, and `INV-C-006` is that no `OBSERVED`
    // rests on manifest presence. A tier this weak also carries no confidence:
    // there is nothing to be confident about.
    assert!(finding.confidence().is_none());
    assert_eq!(finding.strength(), EvidenceStrength::Static);
    // Every site is the manifest and nothing else.
    assert_eq!(finding.locators().len(), 1);
    assert_eq!(finding.locators()[0].path(), "package.json");

    // The injection: the same corpus with the dependency removed produces no
    // finding at all, so the assertion above is about the entry rather than
    // about the analyzer producing one finding for everything.
    let without = [
        ("package.json", "{\n  \"name\": \"orders\"\n}\n"),
        MANIFEST_ONLY[1],
    ];
    assert!(matches!(
        classify(&without),
        Err(error) if matches!(
            error.downcast_ref::<AnalysisError>(),
            Some(AnalysisError::NoPromotingEvidence(_))
        )
    ));
    Ok(())
}

#[test]
fn unreachable_import_is_possible_not_observed() -> TestResult {
    let finding = only(classify(&UNREACHABLE_IMPORT)?)?;

    assert_eq!(finding.rung(), LadderRung::UnreachableImport);
    assert_eq!(finding.tier(), EvidenceTier::Possible);
    assert_eq!(finding.tier().as_str(), "POSSIBLE");
    assert!(finding.confidence().is_none());

    // The reachability half is what makes this row different from the third:
    // `warm` exists and calls into the subject, and nothing an entry point
    // reaches calls `warm`.
    let (_, analysis) = analyzed(&UNREACHABLE_IMPORT)?;
    let warm = analysis
        .symbols()
        .into_iter()
        .find(|symbol| symbol.path() == "src/cache.ts" && !symbol.reachable());
    assert!(
        warm.is_some(),
        "the dead function was not found unreachable"
    );
    Ok(())
}

#[test]
fn reachable_call_plus_config_is_observed_with_confidence() -> TestResult {
    let finding = only(classify(&REACHABLE_CALL_AND_CONFIG)?)?;

    assert_eq!(finding.rung(), LadderRung::ReachableCallWithConfig);
    assert_eq!(finding.tier(), EvidenceTier::Observed);
    assert_eq!(finding.strength(), EvidenceStrength::Static);

    // Section 17.3's third row is `가능, confidence 표시`. The number is a
    // `DisplayedConfidence`, which `P2-M1` issues only from a
    // `CalibratedConfidence`, which only `CalibrationRegistry::interpret`
    // produces — so what is shown names the dataset that interpreted it.
    let confidence = finding
        .confidence()
        .ok_or("an observed finding carried no confidence")?;
    assert_eq!(confidence.dataset().as_str(), "cal-analysis-v1");
    assert!(
        confidence
            .to_string()
            .contains("calibrated by cal-analysis-v1")
    );

    // Both locators section 17.3's row names are on the finding: the call and
    // the configuration.
    let paths: Vec<&str> = finding
        .locators()
        .iter()
        .map(academic_repository_analysis::Locator::path)
        .collect();
    assert!(paths.contains(&"src/cache.ts"), "{paths:?}");
    assert!(paths.contains(&"src/app.config.yaml"), "{paths:?}");

    // The injection: with the dataset gone there is no calibrated number, and
    // `P2-M1`'s contract is that an uncalibrated score cannot be displayed. So
    // the finding is refused rather than shown without one.
    let (_, analysis) = analyzed(&REACHABLE_CALL_AND_CONFIG)?;
    let empty = CalibrationRegistry::new();
    assert!(matches!(
        EvidenceLadder::classify(&analysis, &redis()?, &empty, &purpose()?, &[], NOW),
        Err(AnalysisError::UncalibratedConfidence(_))
    ));

    // And with the dataset present but stale — its refresh interval elapsed —
    // the same refusal, so freshness is load-bearing rather than decorative.
    let stale = NOW + 10_001;
    assert!(matches!(
        EvidenceLadder::classify(&analysis, &redis()?, &registry()?, &purpose()?, &[], stale),
        Err(AnalysisError::UncalibratedConfidence(_))
    ));
    Ok(())
}

#[test]
fn test_only_use_is_test_scoped() -> TestResult {
    let finding = only(classify(&TEST_ONLY_USE)?)?;

    assert_eq!(finding.rung(), LadderRung::TestScopedUse);
    assert_eq!(finding.tier(), EvidenceTier::Observed);
    assert_eq!(finding.artifact_scope(), ArtifactScope::Test);
    assert_ne!(finding.artifact_scope(), ArtifactScope::Production);
    assert!(finding.confidence().is_some());

    // Every counted site is at test scope. A single production site would make
    // the fold above wrong, so the whole set is asserted rather than one member.
    for locator in finding.locators() {
        assert_eq!(
            locator.scope(),
            ArtifactScope::Test,
            "{} counted at {}",
            locator.path(),
            locator.scope().as_str()
        );
    }

    // Section 18.1's five scopes each come out of a path, and the API has no
    // unscoped value to collapse to. `REQ-18-003`'s five fixtures:
    let scoped = [
        ("src/orders.ts", ArtifactScope::Production),
        ("tests/orders.test.ts", ArtifactScope::Test),
        (".github/workflows/ci.yml", ArtifactScope::Build),
        ("migrations/0001_orders.sql", ArtifactScope::Migration),
        ("tools/emit.mjs", ArtifactScope::Development),
    ];
    for (path, expected) in scoped {
        assert_eq!(
            academic_repository_analysis::paths::scope_of(path),
            expected,
            "{path}"
        );
    }
    assert_eq!(ArtifactScope::ALL.len(), 5);
    Ok(())
}

#[test]
fn runtime_and_production_config_is_strong_evidence() -> TestResult {
    let trace = RuntimeTrace::new(
        classify_snapshot_id(&REACHABLE_CALL_AND_CONFIG)?,
        SubjectId::new("redis")?,
    );
    let finding = only(classify_with(
        &REACHABLE_CALL_AND_CONFIG,
        std::slice::from_ref(&trace),
    )?)?;

    assert_eq!(finding.rung(), LadderRung::RuntimeAndProductionConfig);
    assert_eq!(finding.tier(), EvidenceTier::Observed);
    assert_eq!(finding.strength(), EvidenceStrength::Strong);
    assert_eq!(finding.artifact_scope(), ArtifactScope::Production);

    // `REQ-17-019`: a mismatched snapshot produces no strong classification.
    let other_snapshot = RuntimeTrace::new("snap_other", SubjectId::new("redis")?);
    let weaker = only(classify_with(
        &REACHABLE_CALL_AND_CONFIG,
        &[other_snapshot],
    )?)?;
    assert_eq!(weaker.rung(), LadderRung::ReachableCallWithConfig);
    assert_eq!(weaker.strength(), EvidenceStrength::Static);

    // A trace about another subject is not evidence about this one either.
    let other_subject = RuntimeTrace::new(
        classify_snapshot_id(&REACHABLE_CALL_AND_CONFIG)?,
        SubjectId::new("kafka")?,
    );
    let unrelated = only(classify_with(&REACHABLE_CALL_AND_CONFIG, &[other_subject])?)?;
    assert_eq!(unrelated.strength(), EvidenceStrength::Static);

    // And a matching trace over a corpus whose only configuration is at test
    // scope is not the fifth row: that row is `runtime trace/production config
    // 와 일치`, and half of it is missing.
    let test_trace = RuntimeTrace::new(
        classify_snapshot_id(&TEST_ONLY_USE)?,
        SubjectId::new("redis")?,
    );
    let test_scoped = only(classify_with(&TEST_ONLY_USE, &[test_trace])?)?;
    assert_eq!(test_scoped.rung(), LadderRung::TestScopedUse);
    assert_eq!(test_scoped.strength(), EvidenceStrength::Static);
    Ok(())
}

fn classify_snapshot_id(files: &[(&str, &str)]) -> Result<String, Box<dyn Error>> {
    let (snapshot, _) = analyzed(files)?;
    Ok(snapshot.snapshot_id().to_owned())
}

#[test]
fn vendored_generated_example_paths_do_not_promote() -> TestResult {
    // The same evidence that reaches `OBSERVED` under `src/` reaches nothing at
    // all under a vendored, generated or example tree. Each prefix is exercised
    // on its own, because a rule that caught only the first would leave the
    // other two open — which is `S-12`'s shape.
    for prefix in [
        "vendor/upstream",
        "dist/bundle",
        "generated/api",
        "examples/demo",
        "benches/load",
        "probes/smoke",
    ] {
        let moved: Vec<(String, String)> = REACHABLE_CALL_AND_CONFIG
            .iter()
            .map(|(path, body)| {
                let moved = path
                    .strip_prefix("src/")
                    .map_or_else(|| (*path).to_owned(), |tail| format!("{prefix}/{tail}"));
                (moved, (*body).to_owned())
            })
            .collect();
        let borrowed: Vec<(&str, &str)> = moved
            .iter()
            .map(|(path, body)| (path.as_str(), body.as_str()))
            .collect();
        let findings = classify(&borrowed)?;
        // What survives is the manifest entry, which is still first-party: the
        // subject is present and nothing observes it.
        let finding = only(findings)?;
        assert_eq!(
            finding.tier(),
            EvidenceTier::PresentOnly,
            "{prefix} promoted to {}",
            finding.tier().as_str()
        );
        // The sites are kept and labelled rather than dropped, so a reader is
        // told the analyzer saw the vendored copy.
        assert!(
            finding
                .excluded_sites()
                .iter()
                .any(|site| site.reason() == ExclusionReason::NonPromotingPath
                    && site.locator().path().starts_with(prefix)),
            "{prefix} left no excluded site"
        );
    }

    // `target/` is not in the loop above because it never reaches this crate:
    // `P2-R1`'s point-1 file policy holds `target` in its secret-file segments,
    // so the gate removes the whole subtree before the inventory opens
    // anything. Asserting that here is what keeps the two layers from both
    // assuming the other one covers it.
    let with_target = [
        REACHABLE_CALL_AND_CONFIG[0],
        REACHABLE_CALL_AND_CONFIG[1],
        REACHABLE_CALL_AND_CONFIG[2],
        ("target/debug/cache.ts", REACHABLE_CALL_AND_CONFIG[1].1),
    ];
    let (snapshot, analysis) = analyzed(&with_target)?;
    assert!(
        snapshot
            .manifest()
            .iter()
            .all(|entry| !entry.path().starts_with("target/")),
        "the gate admitted a target/ path"
    );
    assert!(
        analysis
            .coverage()
            .iter()
            .all(|row| !row.path().starts_with("target/"))
    );

    // `tests/` is the one of the five the task's list names that is *not* a
    // non-promoting class. It is first-party code at test scope, and collapsing
    // the two axes would make one of those two answers wrong.
    assert_eq!(
        academic_repository_analysis::paths::class_of("tests/cache.test.ts"),
        PathClass::FirstParty
    );
    assert_eq!(
        academic_repository_analysis::paths::scope_of("tests/cache.test.ts"),
        ArtifactScope::Test
    );
    for (path, expected) in [
        ("vendor/upstream/cache.ts", PathClass::Vendored),
        ("third_party/x/cache.ts", PathClass::Vendored),
        ("target/debug/build.rs", PathClass::Generated),
        ("src/schema.generated.ts", PathClass::Generated),
        ("examples/demo/cache.ts", PathClass::Example),
        ("benches/load/cache.ts", PathClass::Example),
        ("probes/smoke/cache.ts", PathClass::Example),
        ("src/cache.ts", PathClass::FirstParty),
    ] {
        assert_eq!(
            academic_repository_analysis::paths::class_of(path),
            expected,
            "{path}"
        );
    }
    // Exactly one of the four classes promotes.
    assert_eq!(
        PathClass::ALL
            .iter()
            .filter(|class| class.promotes())
            .count(),
        1
    );

    // The monorepo half: a use in another package does not corroborate this
    // package's manifest entry, and is recorded against the other package.
    let monorepo = [
        (
            "packages/a/package.json",
            "{\n  \"name\": \"a\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
        ),
        ("packages/b/package.json", "{\n  \"name\": \"b\"\n}\n"),
        (
            "packages/b/src/cache.ts",
            "import redis from \"redis\";\n\nexport function handle() {\n  return redis.createClient();\n}\n",
        ),
        (
            "packages/b/src/app.config.yaml",
            "cache:\n  redis: enabled\n",
        ),
    ];
    let findings = classify(&monorepo)?;
    let for_a = findings
        .iter()
        .find(|finding| {
            finding
                .scope()
                .component()
                .as_str()
                .starts_with("packages/a")
        })
        .ok_or("package a produced no finding")?;
    assert_eq!(for_a.tier(), EvidenceTier::PresentOnly);
    let for_b = findings
        .iter()
        .find(|finding| {
            finding
                .scope()
                .component()
                .as_str()
                .starts_with("packages/b")
        })
        .ok_or("package b produced no finding")?;
    assert_eq!(for_b.rung(), LadderRung::ReachableCallWithConfig);
    assert!(
        for_b
            .excluded_sites()
            .iter()
            .any(|site| site.reason() == ExclusionReason::OtherPackage),
        "package a's manifest entry was not recorded as another package's"
    );
    Ok(())
}

#[test]
fn unsupported_language_reports_a_coverage_gap() -> TestResult {
    let files = [
        REACHABLE_CALL_AND_CONFIG[0],
        REACHABLE_CALL_AND_CONFIG[1],
        REACHABLE_CALL_AND_CONFIG[2],
        ("src/cache.go", "package cache\n\nfunc Warm() {}\n"),
        ("Dockerfile", "FROM redis:7\nEXPOSE 6379\n"),
        ("docs/design.md", "# Design\n"),
    ];
    let (snapshot, analysis) = analyzed(&files)?;

    // Every manifest path has exactly one coverage row. Nothing is skipped, and
    // the equality is in both directions so neither a missing row nor an extra
    // one passes.
    let covered: Vec<&str> = analysis
        .coverage()
        .iter()
        .map(academic_repository_analysis::PathCoverage::path)
        .collect();
    let manifested: Vec<&str> = snapshot
        .manifest()
        .iter()
        .map(academic_repository::ManifestEntry::path)
        .collect();
    assert_eq!(covered, manifested);

    // The unsupported file is a gap for every one of the seven index kinds,
    // and the reason names the language rather than an internal error.
    let go = analysis
        .coverage()
        .iter()
        .find(|row| row.path() == "src/cache.go")
        .ok_or("the unsupported file has no coverage row")?;
    assert_eq!(go.file_kind(), FileKind::Unsupported);
    assert_eq!(go.gaps().len(), IndexKind::COUNT);
    for kind in IndexKind::ALL {
        assert_eq!(
            go.outcome(kind),
            CoverageOutcome::Gap(CoverageGapReason::UnsupportedLanguage),
            "{}",
            kind.as_str()
        );
    }
    assert!(
        analysis
            .gaps()
            .iter()
            .any(|(path, _, reason)| *path == "src/cache.go"
                && *reason == CoverageGapReason::UnsupportedLanguage)
    );

    // A supported file reports no gap at all, so the assertion above is about
    // the language rather than about every file being a gap.
    let supported = analysis
        .coverage()
        .iter()
        .find(|row| row.path() == "src/cache.ts")
        .ok_or("the supported file has no coverage row")?;
    assert_eq!(supported.gaps(), Vec::new());
    assert!(matches!(
        supported.outcome(IndexKind::Symbol),
        CoverageOutcome::Analyzed(count) if count > 0
    ));
    // And the kind that does not apply is not reported as a gap: a Dockerfile
    // has infrastructure facts and no symbols, and saying "gap" for the second
    // would make the gap list noise instead of a list of what is missing.
    let container = analysis
        .coverage()
        .iter()
        .find(|row| row.path() == "Dockerfile")
        .ok_or("the container file has no coverage row")?;
    assert_eq!(container.file_kind(), FileKind::ContainerFile);
    assert_eq!(
        container.outcome(IndexKind::Symbol),
        CoverageOutcome::NotApplicable
    );
    assert!(matches!(
        container.outcome(IndexKind::Iac),
        CoverageOutcome::Analyzed(count) if count > 0
    ));
    assert_eq!(container.gaps(), Vec::new());

    // The support matrix is total, and the language list a coverage report
    // prints is derived from it rather than written twice.
    for file in FileKind::ALL {
        for index in IndexKind::ALL {
            let answer = support(file, index);
            assert_eq!(
                answer == Support::Unsupported,
                file == FileKind::Unsupported,
                "{} x {}",
                file.as_str(),
                index.as_str()
            );
        }
    }
    let supported_kinds = RepositoryAnalysis::supported_file_kinds();
    assert_eq!(supported_kinds.len(), FileKind::ALL.len() - 1);
    assert!(!supported_kinds.contains(&FileKind::Unsupported));
    Ok(())
}

#[test]
fn new_finding_cannot_default_to_repository_scope() -> TestResult {
    // Every spelling of the repository root is refused as a component, *and
    // refused as the root*. The variant matters: the malformed-path rule below
    // happens to reject all four spellings too, so an `is_err()` assertion
    // would still pass with the root branch deleted -- which is exactly what an
    // injection of that deletion showed. Asserting the reason is what makes the
    // branch load-bearing rather than decorative.
    for root in ["", ".", "/", "./"] {
        assert!(
            matches!(
                ComponentId::new(root),
                Err(ComponentError::RepositoryRoot(_))
            ),
            "{root:?} was not refused as the repository root"
        );
    }
    for malformed in ["/abs/path", "a/../b", "a//b", "C:/x", "a\\b"] {
        assert!(
            ComponentId::new(malformed).is_err(),
            "{malformed:?} was accepted as a component"
        );
    }
    // A file at the repository root is its own component rather than widening
    // to the root, which is the one place the refusal above could have created
    // a hole.
    assert_eq!(
        ComponentId::containing("package.json")?.as_str(),
        "package.json"
    );
    assert_eq!(ComponentId::containing("src/cache.ts")?.as_str(), "src");

    // Every finding this crate produces names a component, and none of them is
    // a spelling of the root.
    let corpora: [&[(&str, &str)]; 4] = [
        &MANIFEST_ONLY,
        &UNREACHABLE_IMPORT,
        &REACHABLE_CALL_AND_CONFIG,
        &TEST_ONLY_USE,
    ];
    for corpus in corpora {
        for finding in classify(corpus)? {
            let component = finding.scope().component().as_str().to_owned();
            assert!(
                !["", ".", "/", "./"].contains(&component.as_str()),
                "a finding was scoped to {component:?}"
            );
            assert!(matches!(
                finding.scope(),
                FindingScope::Symbol { .. } | FindingScope::Component { .. }
            ));
        }
    }

    // Evidence spanning three components produces three findings rather than
    // one wider one. Widening is what a repository-wide default would look
    // like once the two obvious spellings are closed.
    let spread = [
        (
            "package.json",
            "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
        ),
        (
            "src/cache/client.ts",
            "import redis from \"redis\";\n\nexport function open() {\n  return redis.createClient();\n}\n",
        ),
        (
            "src/orders/service.ts",
            "import redis from \"redis\";\n\nexport function place() {\n  return redis.createClient();\n}\n",
        ),
        (
            "src/billing/ledger.ts",
            "import redis from \"redis\";\n\nexport function post() {\n  return redis.createClient();\n}\n",
        ),
    ];
    let findings = classify(&spread)?;
    let mut components: Vec<&str> = findings
        .iter()
        .map(|finding| finding.scope().component().as_str())
        .collect();
    components.sort_unstable();
    assert_eq!(
        components,
        vec!["src/billing", "src/cache", "src/orders"],
        "evidence in three components did not produce three findings"
    );
    // `REQ-34-093`'s denominator is the components this run read, not the
    // components in the tree, and it is on every finding.
    for finding in &findings {
        assert_eq!(finding.coverage().observed_components(), 1);
        assert!(finding.coverage().analyzed_components() >= 3);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The promotion injections. One per step of the ladder.
// ---------------------------------------------------------------------------

#[test]
fn each_promotion_needs_its_own_ingredient() -> TestResult {
    // Row one to row two needs an import that counts. An import in a vendored
    // tree is not one.
    let vendored_import = [
        MANIFEST_ONLY[0],
        MANIFEST_ONLY[1],
        (
            "vendor/upstream/cache.ts",
            "import redis from \"redis\";\n\nexport function warm() {\n  return redis.createClient();\n}\n",
        ),
    ];
    assert_eq!(
        only(classify(&vendored_import)?)?.rung(),
        LadderRung::ManifestPresence
    );
    assert_eq!(
        only(classify(&UNREACHABLE_IMPORT)?)?.rung(),
        LadderRung::UnreachableImport
    );

    // Row two to row three needs the call to be reachable. Configuration alone
    // does not make it so.
    let configured_but_dead = [
        UNREACHABLE_IMPORT[0],
        UNREACHABLE_IMPORT[1],
        ("src/app.config.yaml", "cache:\n  redis: enabled\n"),
    ];
    assert_eq!(
        only(classify(&configured_but_dead)?)?.rung(),
        LadderRung::UnreachableImport
    );
    assert_eq!(
        only(classify(&REACHABLE_CALL_AND_CONFIG)?)?.rung(),
        LadderRung::ReachableCallWithConfig
    );

    // Row four is a narrowing rather than a step up, and what takes a corpus
    // out of it is a use that is not at test scope. Adding a production
    // configuration does exactly that; removing it puts it back.
    let test_plus_production_config = [
        TEST_ONLY_USE[0],
        TEST_ONLY_USE[1],
        TEST_ONLY_USE[2],
        ("src/app.config.yaml", "cache:\n  redis: enabled\n"),
    ];
    let widened = only(classify(&test_plus_production_config)?)?;
    assert_eq!(widened.rung(), LadderRung::ReachableCallWithConfig);
    assert_eq!(widened.artifact_scope(), ArtifactScope::Production);
    assert_eq!(
        only(classify(&TEST_ONLY_USE)?)?.artifact_scope(),
        ArtifactScope::Test
    );

    // Row three to row five needs a trace of *this* snapshot naming *this*
    // subject. Both mismatches are covered in
    // `runtime_and_production_config_is_strong_evidence`; what is left here is
    // that the trace is required at all.
    assert_eq!(
        only(classify(&REACHABLE_CALL_AND_CONFIG)?)?.strength(),
        EvidenceStrength::Static
    );
    let trace = RuntimeTrace::new(
        classify_snapshot_id(&REACHABLE_CALL_AND_CONFIG)?,
        SubjectId::new("redis")?,
    );
    assert_eq!(
        only(classify_with(&REACHABLE_CALL_AND_CONFIG, &[trace])?)?.strength(),
        EvidenceStrength::Strong
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The composition with `P2-R1`, and the vocabulary's own shape.
// ---------------------------------------------------------------------------

#[test]
fn the_analysis_reads_only_what_the_snapshot_froze() -> TestResult {
    let entries: Vec<SourceEntry> = REACHABLE_CALL_AND_CONFIG
        .iter()
        .map(|(path, body)| SourceEntry::new(*path, body.as_bytes().to_vec()))
        .collect();
    let facts = WorkingTreeFacts::checkout(
        CommitId::new(HEAD)?,
        Some("main".to_owned()),
        REACHABLE_CALL_AND_CONFIG
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        Vec::new(),
        Vec::new(),
    );
    let policy = PathPolicy::new();
    let request = SnapshotRequest {
        repository: RepositoryId::new("repo_A")?,
        source: RepositorySource::LocalDirectory,
        tree: SourceTree::Entries(&entries),
        facts: &facts,
        policy: &policy,
        captured_at: CAPTURED_AT,
        parent_snapshots: Vec::new(),
        submodule_refs: Vec::new(),
        analysis_policy_hash: ContentDigest::of(b"analysis-policy-v1"),
        tool_versions: vec![ToolVersion::new(ANALYZER, ANALYZER_VERSION)?],
    };
    let (captured, sealed) = capture_local(&request)?;
    let identity = || AnalyzerIdentity::new(ANALYZER, ANALYZER_VERSION);

    // A path the manifest does not hold is refused.
    assert!(matches!(
        AnalysisInput::of(
            &captured.snapshot,
            &sealed,
            identity()?,
            vec![SourceUnit::new(
                "src/absent.ts",
                b"export const a = 1;\n".to_vec()
            )],
        ),
        Err(AnalysisError::PathNotInSnapshot(_))
    ));

    // A manifest path with different bytes is refused, so the analysis cannot
    // be about a file the snapshot did not freeze.
    assert!(matches!(
        AnalysisInput::of(
            &captured.snapshot,
            &sealed,
            identity()?,
            vec![SourceUnit::new(
                "src/cache.ts",
                b"export const a = 2;\n".to_vec()
            )],
        ),
        Err(AnalysisError::BytesDoNotMatchSnapshot(_))
    ));

    // An analyzer the snapshot's `toolVersions` does not record is refused, so
    // a calibration dataset cannot be registered for one build and applied to
    // another.
    assert!(matches!(
        AnalysisInput::of(
            &captured.snapshot,
            &sealed,
            AnalyzerIdentity::new(ANALYZER, "9.9.9")?,
            Vec::new(),
        ),
        Err(AnalysisError::AnalyzerNotInSnapshot(_, _))
    ));

    // The control: the units built from the manifest are accepted.
    let bodies: BTreeMap<&str, &str> = REACHABLE_CALL_AND_CONFIG.iter().copied().collect();
    let units: Vec<SourceUnit> = captured
        .snapshot
        .manifest()
        .iter()
        .map(|entry| SourceUnit::new(entry.path(), bodies[entry.path()].as_bytes().to_vec()))
        .collect();
    assert!(AnalysisInput::of(&captured.snapshot, &sealed, identity()?, units).is_ok());
    Ok(())
}

#[test]
fn bytes_the_gate_manifested_and_did_not_ingest_are_a_gap() -> TestResult {
    // The fourth check `AnalysisInput::of` makes, and the second coverage-gap
    // reason, both of which nothing else in this file produces.
    //
    // `academic-repository` manifests a file it cannot read as bounded text by
    // digest and does **not** ingest it, so a manifest row can exist with
    // nothing sealed behind it. That is a real state rather than a contrived
    // one: it is what the gate does with every binary asset in a repository.
    let mut files: Vec<(&str, &str)> = REACHABLE_CALL_AND_CONFIG.to_vec();
    // Invalid UTF-8, which is what makes the gate call it opaque. Written as a
    // `&str` of lone surrogate-range bytes would not compile, so the corpus
    // carries it as bytes through a second entry list below.
    let opaque_path = "assets/logo.bin";
    let opaque_bytes = vec![0xff_u8, 0xfe, 0x00, 0x01, 0x02];

    let mut entries: Vec<SourceEntry> = files
        .iter()
        .map(|(path, body)| SourceEntry::new(*path, body.as_bytes().to_vec()))
        .collect();
    entries.push(SourceEntry::new(opaque_path, opaque_bytes.clone()));
    let mut tracked: Vec<String> = files.iter().map(|(path, _)| (*path).to_owned()).collect();
    tracked.push(opaque_path.to_owned());
    let facts = WorkingTreeFacts::checkout(
        CommitId::new(HEAD)?,
        Some("main".to_owned()),
        tracked,
        Vec::new(),
        Vec::new(),
    );
    let policy = PathPolicy::new();
    let request = SnapshotRequest {
        repository: RepositoryId::new("repo_A")?,
        source: RepositorySource::LocalDirectory,
        tree: SourceTree::Entries(&entries),
        facts: &facts,
        policy: &policy,
        captured_at: CAPTURED_AT,
        parent_snapshots: Vec::new(),
        submodule_refs: Vec::new(),
        analysis_policy_hash: ContentDigest::of(b"analysis-policy-v1"),
        tool_versions: vec![ToolVersion::new(ANALYZER, ANALYZER_VERSION)?],
    };
    let (capture, sealed) = capture_local(&request)?;
    let snapshot = capture.snapshot;

    // The gate manifested it, so this is not the excluded-path case.
    assert!(
        snapshot
            .manifest()
            .iter()
            .any(|entry| entry.path() == opaque_path),
        "the opaque file is not in the manifest"
    );

    // Offering its bytes is refused: they hash to the manifest row, so the
    // third check passes and only the sealed-index check can refuse them.
    let unit = SourceUnit::new(opaque_path, opaque_bytes);
    assert!(matches!(
        AnalysisInput::of(
            &snapshot,
            &sealed,
            AnalyzerIdentity::new(ANALYZER, ANALYZER_VERSION)?,
            vec![unit],
        ),
        Err(AnalysisError::BytesNotSealed(path)) if path == opaque_path
    ));

    // Not offering them is not a silent skip either: the path still gets a
    // coverage row, and every one of its seven outcomes is a gap that says the
    // bytes never reached a reader.
    files.push((opaque_path, ""));
    let units: Vec<SourceUnit> = source_units(&snapshot, &files)
        .into_iter()
        .filter(|unit| unit.path() != opaque_path)
        .collect();
    let input = AnalysisInput::of(
        &snapshot,
        &sealed,
        AnalyzerIdentity::new(ANALYZER, ANALYZER_VERSION)?,
        units,
    )?;
    let analysis = analyze(&input)?;
    let row = analysis
        .coverage()
        .iter()
        .find(|row| row.path() == opaque_path)
        .ok_or("the opaque path has no coverage row")?;
    assert_eq!(row.gaps().len(), IndexKind::COUNT);
    for kind in IndexKind::ALL {
        assert_eq!(
            row.outcome(kind),
            CoverageOutcome::Gap(CoverageGapReason::BytesNotIngested),
            "{}",
            kind.as_str()
        );
    }
    // And the two gap reasons are distinguishable rather than one value with
    // two spellings: the unsupported-language corpus produces the other one.
    assert_eq!(CoverageGapReason::ALL.len(), 2);
    assert!(
        analysis
            .gaps()
            .iter()
            .all(|(_, _, reason)| *reason == CoverageGapReason::BytesNotIngested)
    );
    Ok(())
}

#[test]
fn the_tier_vocabulary_is_three_values_and_the_ladder_is_five_rungs() -> TestResult {
    // `REQ-34-081` names exactly three badges; section 17.3's table has five
    // rows this task owns. The fold is stated once, in `LadderRung::tier`, and
    // asserted here row by row so a change to either end is a failure rather
    // than a silent re-reading of the table.
    assert_eq!(EvidenceTier::ALL.len(), 3);
    assert_eq!(LadderRung::ALL.len(), 5);
    assert_eq!(
        EvidenceTier::ALL.map(EvidenceTier::as_str),
        ["PRESENT_ONLY", "POSSIBLE", "OBSERVED"]
    );
    let fold = [
        (LadderRung::ManifestPresence, EvidenceTier::PresentOnly),
        (LadderRung::UnreachableImport, EvidenceTier::Possible),
        (LadderRung::ReachableCallWithConfig, EvidenceTier::Observed),
        (LadderRung::TestScopedUse, EvidenceTier::Observed),
        (
            LadderRung::RuntimeAndProductionConfig,
            EvidenceTier::Observed,
        ),
    ];
    assert_eq!(fold.len(), LadderRung::ALL.len());
    for (rung, tier) in fold {
        assert_eq!(rung.tier(), tier, "{}", rung.as_str());
    }

    // Each of the five rungs is produced by one of the corpora above, so the
    // fold is exercised rather than only declared.
    let trace = RuntimeTrace::new(
        classify_snapshot_id(&REACHABLE_CALL_AND_CONFIG)?,
        SubjectId::new("redis")?,
    );
    let produced = [
        only(classify(&MANIFEST_ONLY)?)?.rung(),
        only(classify(&UNREACHABLE_IMPORT)?)?.rung(),
        only(classify(&REACHABLE_CALL_AND_CONFIG)?)?.rung(),
        only(classify(&TEST_ONLY_USE)?)?.rung(),
        only(classify_with(&REACHABLE_CALL_AND_CONFIG, &[trace])?)?.rung(),
    ];
    assert_eq!(produced, LadderRung::ALL);
    Ok(())
}

// ---------------------------------------------------------------------------
// The half of the untrusted-content boundary that lives one step outside
// `no_public_signature_hands_out_ingested_text`.
// ---------------------------------------------------------------------------

/// A string no path in the corpus holds and every analyzed identifier does.
///
/// It is deliberately not a word: a canary a reader could confuse with a real
/// identifier would make a failure ambiguous.
const CANARY: &str = "zqcanaryzq";

#[test]
fn no_analyzed_byte_reaches_a_text_accessor() -> TestResult {
    // `P2-G5`'s `no_public_signature_hands_out_ingested_text` refuses a `pub fn`
    // that takes an `Untrusted<…>` and returns text. This crate takes no
    // `Untrusted<…>` at all, so that scan covers none of its surface — and a
    // symbol name lifted out of a repository and handed back as a `&str` would
    // be the same leak by another route. `analysis_scans.rs` pins the whole set
    // of public functions that return text and why each one may; this is the
    // executed half, over a corpus whose every identifier, dependency name and
    // configuration key is a canary and whose paths hold none.
    let files = [
        (
            "package.json",
            "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\",\n    \"zqcanaryzq-client\": \"1.0.0\"\n  }\n}\n",
        ),
        (
            "src/cache.ts",
            "import redis from \"redis\";\n\nconst zqcanaryzqUrl = 1;\n\nfunction zqcanaryzqWarm() {\n  return redis.createClient(zqcanaryzqUrl);\n}\n\nexport function zqcanaryzqHandle() {\n  return zqcanaryzqWarm();\n}\n",
        ),
        (
            "src/app.config.yaml",
            "zqcanaryzq:\n  enabled: yes\ncache:\n  redis: zqcanaryzq\n",
        ),
        (
            "migrations/0001_orders.sql",
            "CREATE TABLE zqcanaryzq_orders(id TEXT);\n",
        ),
        ("Dockerfile", "FROM redis:7\nENV ZQCANARYZQ_URL=cache\n"),
        ("docs/zq.md", "# zqcanaryzq\n"),
    ];
    for (path, _) in files {
        assert!(!path.contains(CANARY), "{path} holds the canary");
    }

    let (_, analysis) = analyzed(&files)?;
    let findings = classify(&files)?;

    // Not vacuous: the analyzer really read those files.
    assert!(
        analysis
            .symbols()
            .iter()
            .any(|symbol| symbol.path() == "src/cache.ts"),
        "the canary corpus produced no symbols"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.tier() == EvidenceTier::Observed),
        "the canary corpus produced no observed finding"
    );

    let mut rendered = String::new();
    // Every public accessor that returns text, plus the `Debug` of every public
    // value — because an accessor is not the only way a `String` reaches a log.
    //
    // The input values come first, and they are the ones this test was missing.
    // `SourceUnit` is the only public type in this crate that *holds* the
    // analyzed bytes, and `AnalysisInput` holds a vector of them; walking only
    // the analysis outputs left the one value carrying the payload unobserved.
    // `tools/secret-debug-policy.test.mjs` refuses a derived `Debug` over a
    // field named `source_bytes` and refuses a hand-written one that prints it,
    // and both refusals were observed by injection — but that is another
    // crate's net, and a crate whose whole subject is untrusted repository
    // bytes should fail in its own suite too.
    let (snapshot, sealed) = captured(&files)?;
    let units: Vec<SourceUnit> = source_units(&snapshot, &files);
    for unit in &units {
        rendered.push_str(&format!("{unit:?}"));
        rendered.push_str(unit.path());
    }
    let input = AnalysisInput::of(
        &snapshot,
        &sealed,
        AnalyzerIdentity::new(ANALYZER, ANALYZER_VERSION)?,
        source_units(&snapshot, &files),
    )?;
    rendered.push_str(&format!("{input:?}"));

    rendered.push_str(&format!("{analysis:?}"));
    rendered.push_str(analysis.snapshot_id());
    for row in analysis.coverage() {
        rendered.push_str(&format!("{row:?}"));
        rendered.push_str(row.path());
        rendered.push_str(row.file_kind().as_str());
        rendered.push_str(row.class().as_str());
        rendered.push_str(row.scope().as_str());
        for kind in IndexKind::ALL {
            rendered.push_str(kind.as_str());
            rendered.push_str(&format!("{:?}", row.outcome(kind)));
        }
    }
    for symbol in analysis.symbols() {
        rendered.push_str(&format!("{symbol:?}"));
        rendered.push_str(symbol.path());
        rendered.push_str(symbol.fingerprint().as_str());
        rendered.push_str(symbol.kind().as_str());
    }
    for (path, kind, reason) in analysis.gaps() {
        rendered.push_str(path);
        rendered.push_str(kind.as_str());
        rendered.push_str(reason.as_str());
    }
    for package in analysis.packages().packages() {
        rendered.push_str(package.as_str());
    }
    for finding in &findings {
        rendered.push_str(&format!("{finding:?}"));
        rendered.push_str(finding.snapshot_id());
        rendered.push_str(finding.subject());
        rendered.push_str(finding.scope().as_str());
        rendered.push_str(finding.scope().component().as_str());
        rendered.push_str(finding.tier().as_str());
        rendered.push_str(finding.rung().as_str());
        rendered.push_str(finding.artifact_scope().as_str());
        rendered.push_str(finding.strength().as_str());
        if let Some(confidence) = finding.confidence() {
            rendered.push_str(&confidence.to_string());
            rendered.push_str(confidence.dataset().as_str());
        }
        for locator in finding.locators() {
            rendered.push_str(locator.path());
            rendered.push_str(locator.blob_digest().as_str());
            if let Some(symbol) = locator.symbol() {
                rendered.push_str(symbol.as_str());
            }
        }
        for excluded in finding.excluded_sites() {
            rendered.push_str(&format!("{excluded:?}"));
            rendered.push_str(excluded.reason().as_str());
        }
    }

    assert!(
        !rendered.contains(CANARY),
        "an analyzed identifier reached a public accessor or a Debug output"
    );
    // The canary is case-folded on the way in, so the upper-case spelling in the
    // container file would not be caught by the comparison above on its own.
    assert!(!rendered.to_ascii_lowercase().contains(CANARY));
    Ok(())
}
