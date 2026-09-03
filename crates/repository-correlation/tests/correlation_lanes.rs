//! `P2-R3`'s named acceptance evidence.
//!
//! Every corpus is synthetic and built in process, captured through `P2-R1`'s
//! own `capture_local` and analyzed through `P2-R2`'s own ladder, so every
//! finding correlated here passed the permission and secret gate, the frozen
//! manifest, and the sealed untrusted-content index before this file saw it.
//!
//! The one file this suite reads is the design document itself, and only to
//! compare section 17.5's relation list against the enumeration: a count
//! restated in a test is a count that can be restated wrongly.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
};

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
    AnalysisInput, AnalyzerIdentity, EvidenceLadder, Finding, RepositoryAnalysis, RuntimeTrace,
    SourceUnit, Subject, SubjectId, analyze,
};
use academic_repository_correlation::{
    AnswerSource, ApprovalStatus, AuthorityLane, BehaviorDocument, Candidate, ChangeCause,
    Correlation, CorrelationError, CorrelationInput, DeploymentRecord, DeploymentTarget,
    DocumentId, DriftKind, DriftScopeKind, EdgeEvidence, EvidenceRelation, FeatureFlagRecord,
    FlagKey, FlagState, IncidentId, IncidentRecord, IntentDocument, IntentDocumentKind,
    PresenceChange, RelationEdge, SemanticTransition, active_view, compare, correlate,
};
use academic_untrusted_content::SourceIndex;

type TestResult = Result<(), Box<dyn Error>>;

const CAPTURED_AT: u64 = 1_756_000_000_000;
const HEAD: &str = "abc1234def5678";
const BRANCH: &str = "main";
const ANALYZER: &str = "academic-repository-analysis";
const V1: &str = "0.1.0";
const V2: &str = "0.2.0";
const NOW: u64 = 5_000;

// ---------------------------------------------------------------------------
// The deterministic harness: `P2-R1`'s capture, `P2-R2`'s ladder, this crate.
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// Captures a synthetic tree on `branch` and hands back what `P2-R1` froze.
fn captured(
    files: &[(&str, &str)],
    branch: &str,
) -> Result<(RepositorySnapshot, SourceIndex), Box<dyn Error>> {
    let entries: Vec<SourceEntry> = files
        .iter()
        .map(|(path, body)| SourceEntry::new(*path, body.as_bytes().to_vec()))
        .collect();
    let facts = WorkingTreeFacts::checkout(
        CommitId::new(HEAD)?,
        Some(branch.to_owned()),
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
        // Both analyzer builds, so one snapshot can be read by either and
        // `same_bytes_two_analyzers_is_analysis_changed` has a snapshot to dual
        // run. `AnalysisInput::of` refuses a build the snapshot does not name.
        tool_versions: vec![
            ToolVersion::new(ANALYZER, V1)?,
            ToolVersion::new(ANALYZER, V2)?,
        ],
    };
    let (capture, sealed) = capture_local(&request)?;
    Ok((capture.snapshot, sealed))
}

/// Analyzes the frozen snapshot, offering only the paths in `offered`.
///
/// A manifest row not offered becomes a coverage gap rather than an error,
/// which is `P2-R2`'s `bytes_the_gate_manifested_and_did_not_ingest_are_a_gap`.
/// That is how one analyzer build reading fewer file kinds than another is
/// modelled here.
fn analyzed_with(
    files: &[(&str, &str)],
    branch: &str,
    version: &str,
    offered: Option<&[&str]>,
) -> Result<(RepositorySnapshot, RepositoryAnalysis), Box<dyn Error>> {
    let (snapshot, sealed) = captured(files, branch)?;
    let bodies: BTreeMap<&str, &str> = files.iter().copied().collect();
    let units: Vec<SourceUnit> = snapshot
        .manifest()
        .iter()
        .filter(|entry| offered.is_none_or(|list| list.contains(&entry.path())))
        .map(|entry| SourceUnit::new(entry.path(), bodies[entry.path()].as_bytes().to_vec()))
        .collect();
    let identity = AnalyzerIdentity::new(ANALYZER, version)?;
    let input = AnalysisInput::of(&snapshot, &sealed, identity, units)?;
    let analysis = analyze(&input)?;
    Ok((snapshot, analysis))
}

/// A registry holding one fresh dataset for each analyzer build.
fn registry() -> Result<CalibrationRegistry, Box<dyn Error>> {
    let mut registry = CalibrationRegistry::new();
    for version in [V1, V2] {
        registry.register(CalibrationDataset::new(
            CalibrationDatasetId::new(format!("cal-correlation-{version}"))?,
            ProviderId::new(ANALYZER)?,
            ModelVersion::new(version)?,
            purpose()?,
            Digest32::of(version.as_bytes()),
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
    }
    Ok(registry)
}

fn purpose() -> Result<Purpose, Box<dyn Error>> {
    Ok(Purpose::new("REPOSITORY_EVIDENCE_TIER")?)
}

fn subject_id(name: &str) -> Result<SubjectId, Box<dyn Error>> {
    Ok(SubjectId::new(name)?)
}

/// The two subjects every corpus here is about.
fn subjects() -> Result<Vec<Subject>, Box<dyn Error>> {
    Ok(vec![
        Subject::new(
            subject_id("redis")?,
            &["redis"],
            &["redis"],
            &["redis_open"],
            &["redis"],
        ),
        Subject::new(
            subject_id("distributed-lock")?,
            &["distlock"],
            &["distlock"],
            &["lock_acquire"],
            &["distlock"],
        ),
    ])
}

/// Every finding `P2-R2` produces for either subject over one corpus.
///
/// A subject with no promoting evidence is a refusal there, and an absence
/// here: the correlation's question is what the analysis found, not whether
/// every subject was searched for.
fn findings_of(
    analysis: &RepositoryAnalysis,
    traces: &[RuntimeTrace],
) -> Result<Vec<Finding>, Box<dyn Error>> {
    let registry = registry()?;
    let purpose = purpose()?;
    let mut found = Vec::new();
    for subject in subjects()? {
        if let Ok(findings) =
            EvidenceLadder::classify(analysis, &subject, &registry, &purpose, traces, NOW)
        {
            found.extend(findings);
        }
    }
    Ok(found)
}

/// Everything a correlation takes beside the findings.
#[derive(Default)]
struct Artifacts {
    intent: Vec<IntentDocument>,
    behavior: Vec<BehaviorDocument>,
    incidents: Vec<IncidentRecord>,
    flags: Vec<FeatureFlagRecord>,
    deployments: Vec<DeploymentRecord>,
}

/// One correlation over one corpus.
fn correlated(
    files: &[(&str, &str)],
    branch: &str,
    version: &str,
    offered: Option<&[&str]>,
    traces: &[RuntimeTrace],
    artifacts: &Artifacts,
) -> Result<Correlation, Box<dyn Error>> {
    let (snapshot, analysis) = analyzed_with(files, branch, version, offered)?;
    let traces: Vec<RuntimeTrace> = traces
        .iter()
        .map(|trace| RuntimeTrace::new(snapshot.snapshot_id(), trace.subject().clone()))
        .collect();
    let findings = findings_of(&analysis, &traces)?;
    let identity = AnalyzerIdentity::new(ANALYZER, version)?;
    let input = CorrelationInput {
        snapshot: &snapshot,
        analyzer: &identity,
        findings: &findings,
        intent_documents: &artifacts.intent,
        behavior_documents: &artifacts.behavior,
        incidents: &artifacts.incidents,
        feature_flags: &artifacts.flags,
        deployments: &artifacts.deployments,
    };
    Ok(correlate(&input)?)
}

/// The common case: `main`, the first analyzer build, everything offered.
fn simple(files: &[(&str, &str)], artifacts: &Artifacts) -> Result<Correlation, Box<dyn Error>> {
    correlated(files, BRANCH, V1, None, &[], artifacts)
}

fn relations_for(correlation: &Correlation, subject: &str) -> BTreeSet<EvidenceRelation> {
    correlation
        .relations()
        .iter()
        .filter(|edge| edge.subject() == subject)
        .map(RelationEdge::relation)
        .collect()
}

fn drift_for(correlation: &Correlation, subject: &str) -> Option<DriftKind> {
    correlation
        .drifts()
        .iter()
        .find(|drift| drift.subject() == subject)
        .map(academic_repository_correlation::ImplementationDrift::kind)
}

fn intent_document(
    id: &str,
    kind: IntentDocumentKind,
    status: ApprovalStatus,
    revision: u64,
    branch: Option<&str>,
    path: &str,
    mentions: &[&str],
) -> Result<IntentDocument, Box<dyn Error>> {
    let mut named = Vec::new();
    for subject in mentions {
        named.push(subject_id(subject)?);
    }
    Ok(IntentDocument::new(
        DocumentId::new(id)?,
        kind,
        status,
        revision,
        branch.map(str::to_owned),
        path,
        named,
    ))
}

fn behavior_document(
    id: &str,
    path: &str,
    explains: &[&str],
) -> Result<BehaviorDocument, Box<dyn Error>> {
    let mut named = Vec::new();
    for subject in explains {
        named.push(subject_id(subject)?);
    }
    Ok(BehaviorDocument::new(DocumentId::new(id)?, path, named))
}

// ---------------------------------------------------------------------------
// The corpora.
// ---------------------------------------------------------------------------

const PACKAGE_WITH_REDIS: (&str, &str) = (
    "package.json",
    "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
);

const SPEC_PAGE: (&str, &str) = ("docs/spec.md", "# Order platform\n\nA specification.\n");
const ADR_PAGE: (&str, &str) = ("docs/adr-001.md", "# ADR 001\n\nA decision.\n");
const BEHAVIOR_PAGE: (&str, &str) = ("docs/behaviour.md", "# Behaviour\n\nA description.\n");

/// `redis` reachable from an entry point, with a production configuration:
/// `P2-R2`'s third row, so `OBSERVED` and a `PROJECT_CODE_USES`.
const CODE_USES: [(&str, &str); 6] = [
    PACKAGE_WITH_REDIS,
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    ("src/app.config.yaml", "cache:\n  redis: enabled\n"),
    SPEC_PAGE,
    ADR_PAGE,
    BEHAVIOR_PAGE,
];

/// `redis` used nowhere but a test: `P2-R2`'s fourth row, so
/// `PROJECT_TEST_EXERCISES` and no `PROJECT_CODE_USES`.
const TEST_EXERCISES: [(&str, &str); 5] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"devDependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "tests/cache.test.ts",
        "import redis from \"redis\";\n\nexport function checkCache() {\n  return redis.createClient();\n}\n",
    ),
    ("tests/harness.config.yaml", "cache:\n  redis: enabled\n"),
    SPEC_PAGE,
    ADR_PAGE,
];

/// The manifest declares `redis` and nothing uses it: `P2-R2`'s first row, so
/// `PRESENT_ONLY` and no implementation-lane relation at all.
const MANIFEST_ONLY: [(&str, &str); 4] = [
    PACKAGE_WITH_REDIS,
    (
        "src/orders.ts",
        "export function place() {\n  return 1;\n}\n",
    ),
    SPEC_PAGE,
    ADR_PAGE,
];

/// `redis` imported and never reached: `P2-R2`'s second row, `보류`, so
/// `POSSIBLE` and no implementation-lane relation either.
const UNREACHABLE_IMPORT: [(&str, &str); 4] = [
    PACKAGE_WITH_REDIS,
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n",
    ),
    SPEC_PAGE,
    ADR_PAGE,
];

// ---------------------------------------------------------------------------
// 1. `seven_relation_types_are_distinct`
// ---------------------------------------------------------------------------

/// Section 17.5's own bullet list, read out of the design document.
///
/// Not a number written here. The relation vocabulary is compared against the
/// authority in both directions, so a relation added to the enumeration without
/// the design document naming it, or named there and missing here, is a
/// failure rather than a count nobody rechecks.
fn relations_in_design() -> Result<Vec<String>, Box<dyn Error>> {
    let page = fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let mut found = Vec::new();
    for line in page.lines() {
        let Some(rest) = line.trim().strip_prefix("- `PROJECT_") else {
            continue;
        };
        let Some(end) = rest.find('`') else {
            continue;
        };
        found.push(format!("PROJECT_{}", &rest[..end]));
    }
    Ok(found)
}

/// The lane each relation answers for, as section 30.3 rows four and five
/// assign them. Written out rather than counted, for the same reason.
const LANE_OF: [(EvidenceRelation, AuthorityLane); 7] = [
    (EvidenceRelation::SpecMentions, AuthorityLane::Intent),
    (
        EvidenceRelation::ArchitectureRequires,
        AuthorityLane::Intent,
    ),
    (EvidenceRelation::CodeUses, AuthorityLane::Implementation),
    (
        EvidenceRelation::TestExercises,
        AuthorityLane::Implementation,
    ),
    (
        EvidenceRelation::ConfigEnables,
        AuthorityLane::Implementation,
    ),
    (
        EvidenceRelation::IncidentExposed,
        AuthorityLane::Implementation,
    ),
    (EvidenceRelation::DocExplains, AuthorityLane::Description),
];

/// The corpus that produces all three analysis-derived relations at once.
///
/// It takes two subjects, because one cannot carry both: `P2-R2`'s fourth row
/// is `used **nowhere but** tests`, so a subject with a production use is never
/// test-scoped. `redis` is used in production with a configuration a trace
/// agrees with — section 17.3's fifth row — and `distributed-lock` is used
/// nowhere but the test tree.
const SEVEN: [(&str, &str); 8] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  },\n  \"devDependencies\": {\n    \"distlock\": \"1.0.0\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    ("src/app.config.yaml", "cache:\n  redis: enabled\n"),
    (
        "tests/lock.test.ts",
        "import distlock from \"distlock\";\n\nexport function checkLock() {\n  return distlock.acquire();\n}\n",
    ),
    ("tests/harness.config.yaml", "lock:\n  distlock: enabled\n"),
    SPEC_PAGE,
    ADR_PAGE,
    BEHAVIOR_PAGE,
];

/// The same corpus with the test tree removed, so `distributed-lock` has
/// nothing but its manifest entry left.
const SEVEN_WITHOUT_TESTS: [(&str, &str); 6] =
    [SEVEN[0], SEVEN[1], SEVEN[2], SEVEN[5], SEVEN[6], SEVEN[7]];

/// The same corpus with `redis` imported and never reached, and no
/// configuration for it, so section 17.3's second row applies.
const SEVEN_WITH_DEAD_IMPORT: [(&str, &str); 7] = [
    SEVEN[0],
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n",
    ),
    SEVEN[3],
    SEVEN[4],
    SEVEN[5],
    SEVEN[6],
    SEVEN[7],
];

/// Every artifact needed to produce the four relations that come from an
/// argument rather than from a finding.
fn everything() -> Result<Artifacts, Box<dyn Error>> {
    Ok(Artifacts {
        intent: vec![
            intent_document(
                "spec-1",
                IntentDocumentKind::Specification,
                ApprovalStatus::Approved,
                2,
                Some(BRANCH),
                "docs/spec.md",
                &["redis"],
            )?,
            intent_document(
                "adr-1",
                IntentDocumentKind::ArchitectureDecision,
                ApprovalStatus::Approved,
                1,
                Some(BRANCH),
                "docs/adr-001.md",
                &["redis"],
            )?,
        ],
        behavior: vec![behavior_document(
            "behaviour-1",
            "docs/behaviour.md",
            &["redis"],
        )?],
        incidents: vec![IncidentRecord::new(
            IncidentId::new("inc-1")?,
            "",
            1_700,
            vec![subject_id("redis")?],
        )],
        flags: Vec::new(),
        deployments: Vec::new(),
    })
}

/// The incident's snapshot is not known until the snapshot is frozen, so it is
/// filled in here rather than in the corpus.
fn bound_incidents(
    artifacts: &Artifacts,
    snapshot_id: &str,
) -> Result<Vec<IncidentRecord>, Box<dyn Error>> {
    let mut bound = Vec::new();
    for incident in &artifacts.incidents {
        bound.push(IncidentRecord::new(
            incident.id().clone(),
            snapshot_id,
            incident.occurred_at(),
            incident.exposed().to_vec(),
        ));
    }
    Ok(bound)
}

/// Correlates one corpus, binding the incident to the frozen snapshot and
/// tracing `redis`, which is what section 17.3's fifth row needs.
fn traced_run(
    files: &[(&str, &str)],
    artifacts: &Artifacts,
    trace: bool,
) -> Result<Correlation, Box<dyn Error>> {
    let (snapshot, analysis) = analyzed_with(files, BRANCH, V1, None)?;
    let traces = if trace {
        vec![RuntimeTrace::new(
            snapshot.snapshot_id(),
            subject_id("redis")?,
        )]
    } else {
        Vec::new()
    };
    let findings = findings_of(&analysis, &traces)?;
    let identity = AnalyzerIdentity::new(ANALYZER, V1)?;
    let incidents = bound_incidents(artifacts, snapshot.snapshot_id())?;
    let input = CorrelationInput {
        snapshot: &snapshot,
        analyzer: &identity,
        findings: &findings,
        intent_documents: &artifacts.intent,
        behavior_documents: &artifacts.behavior,
        incidents: &incidents,
        feature_flags: &artifacts.flags,
        deployments: &artifacts.deployments,
    };
    Ok(correlate(&input)?)
}

/// The corpus and the arguments that between them produce all seven.
fn all_seven(artifacts: &Artifacts) -> Result<Correlation, Box<dyn Error>> {
    traced_run(&SEVEN, artifacts, true)
}

/// Every relation one run produced, over every subject.
fn produced(correlation: &Correlation) -> BTreeSet<EvidenceRelation> {
    correlation
        .relations()
        .iter()
        .map(RelationEdge::relation)
        .collect()
}

#[test]
fn seven_relation_types_are_distinct() -> TestResult {
    // The vocabulary is the design document's, both directions.
    let declared: Vec<String> = EvidenceRelation::ALL
        .iter()
        .map(|relation| relation.as_str().to_owned())
        .collect();
    assert_eq!(
        relations_in_design()?,
        declared,
        "section 17.5's relation list and this enumeration disagree"
    );

    // Distinct spellings, and one lane each.
    let spellings: BTreeSet<&str> = EvidenceRelation::ALL
        .iter()
        .map(|relation| relation.as_str())
        .collect();
    assert_eq!(spellings.len(), EvidenceRelation::ALL.len());
    for (relation, lane) in LANE_OF {
        assert_eq!(relation.lane(), lane, "{} changed lane", relation.as_str());
    }
    let mapped: BTreeSet<EvidenceRelation> =
        LANE_OF.iter().map(|(relation, _)| *relation).collect();
    assert_eq!(
        mapped,
        EvidenceRelation::ALL.into_iter().collect::<BTreeSet<_>>(),
        "a relation has no lane assignment"
    );

    // Each of the seven is produced, and the seven are produced together.
    let artifacts = everything()?;
    let correlation = all_seven(&artifacts)?;
    assert_eq!(
        produced(&correlation),
        EvidenceRelation::ALL.into_iter().collect::<BTreeSet<_>>(),
        "a relation in the vocabulary is produced by no corpus in this suite"
    );
    // The two subjects are separate: one carries the production use and the
    // other the test-scoped one, which is `P2-R2`'s fourth row being `used
    // nowhere but tests` rather than `used in tests`.
    assert!(relations_for(&correlation, "redis").contains(&EvidenceRelation::CodeUses));
    assert_eq!(
        relations_for(&correlation, "distributed-lock"),
        BTreeSet::from([EvidenceRelation::TestExercises])
    );

    // The injections. Removing the one input that produces a relation removes
    // that relation and leaves the other six, so each assertion above is about
    // its own producer rather than about a set that happens to be full.
    let without_spec = Artifacts {
        intent: vec![artifacts.intent[1].clone()],
        behavior: artifacts.behavior.clone(),
        incidents: artifacts.incidents.clone(),
        ..Artifacts::default()
    };
    assert_missing(&all_seven(&without_spec)?, EvidenceRelation::SpecMentions)?;

    let without_adr = Artifacts {
        intent: vec![artifacts.intent[0].clone()],
        behavior: artifacts.behavior.clone(),
        incidents: artifacts.incidents.clone(),
        ..Artifacts::default()
    };
    assert_missing(
        &all_seven(&without_adr)?,
        EvidenceRelation::ArchitectureRequires,
    )?;

    let without_behaviour = Artifacts {
        intent: artifacts.intent.clone(),
        incidents: artifacts.incidents.clone(),
        ..Artifacts::default()
    };
    assert_missing(
        &all_seven(&without_behaviour)?,
        EvidenceRelation::DocExplains,
    )?;

    let without_incident = Artifacts {
        intent: artifacts.intent.clone(),
        behavior: artifacts.behavior.clone(),
        ..Artifacts::default()
    };
    assert_missing(
        &all_seven(&without_incident)?,
        EvidenceRelation::IncidentExposed,
    )?;

    // The three analysis-derived relations, each dropped by dropping its own
    // ingredient rather than by dropping an argument.
    //
    // The trace: section 17.3's fifth row is `runtime trace/production config와
    // 일치`, so the configuration alone is not `실행 구성에서 활성화` and the
    // use stays a use.
    let untraced = traced_run(&SEVEN, &artifacts, false)?;
    assert_missing(&untraced, EvidenceRelation::ConfigEnables)?;

    // The test tree: without it `distributed-lock` has only its manifest entry,
    // which is section 17.3's first row and no relation at all.
    let no_tests = traced_run(&SEVEN_WITHOUT_TESTS, &artifacts, true)?;
    assert_missing(&no_tests, EvidenceRelation::TestExercises)?;
    assert!(
        no_tests
            .declared_dependencies()
            .contains("distributed-lock"),
        "the manifest entry went away too, so this injection proves nothing"
    );

    // Reachability: an unreachable import is section 17.3's second row, `보류`.
    // The trace has to go with it -- section 17.3's fifth row does not ask
    // whether a call is reachable -- so this removes two relations and the
    // assertion names both.
    let dead_import = traced_run(&SEVEN_WITH_DEAD_IMPORT, &artifacts, false)?;
    let left = produced(&dead_import);
    assert!(!left.contains(&EvidenceRelation::CodeUses));
    assert!(!left.contains(&EvidenceRelation::ConfigEnables));
    assert_eq!(
        left,
        EvidenceRelation::ALL
            .into_iter()
            .filter(|relation| !matches!(
                relation,
                EvidenceRelation::CodeUses | EvidenceRelation::ConfigEnables
            ))
            .collect::<BTreeSet<_>>()
    );
    Ok(())
}

fn assert_missing(correlation: &Correlation, relation: EvidenceRelation) -> TestResult {
    let found = produced(correlation);
    assert!(
        !found.contains(&relation),
        "{} survived its producer being removed",
        relation.as_str()
    );
    let rest: BTreeSet<EvidenceRelation> = EvidenceRelation::ALL
        .into_iter()
        .filter(|other| *other != relation)
        .collect();
    assert_eq!(
        found,
        rest,
        "removing the producer of {} changed another relation too",
        relation.as_str()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `spec_only_is_intended_not_implemented`
// ---------------------------------------------------------------------------

#[test]
fn spec_only_is_intended_not_implemented() -> TestResult {
    // Section 17.5's first diagram: a specification names the subject and the
    // code snapshot has no evidence.
    let artifacts = Artifacts {
        intent: vec![intent_document(
            "spec-1",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            1,
            Some(BRANCH),
            "docs/spec.md",
            &["distributed-lock"],
        )?],
        ..Artifacts::default()
    };
    let correlation = simple(&MANIFEST_ONLY, &artifacts)?;

    assert_eq!(
        relations_for(&correlation, "distributed-lock"),
        BTreeSet::from([EvidenceRelation::SpecMentions])
    );
    assert_eq!(
        drift_for(&correlation, "distributed-lock"),
        Some(DriftKind::IntendedNotImplemented)
    );
    assert_eq!(
        DriftKind::IntendedNotImplemented.as_str(),
        "INTENDED_NOT_IMPLEMENTED"
    );

    // An architecture decision is the other half of section 30.3 row five's
    // `spec/ADR`, and it produces the same drift.
    let by_adr = Artifacts {
        intent: vec![intent_document(
            "adr-1",
            IntentDocumentKind::ArchitectureDecision,
            ApprovalStatus::Approved,
            1,
            Some(BRANCH),
            "docs/adr-001.md",
            &["distributed-lock"],
        )?],
        ..Artifacts::default()
    };
    assert_eq!(
        drift_for(&simple(&MANIFEST_ONLY, &by_adr)?, "distributed-lock"),
        Some(DriftKind::IntendedNotImplemented)
    );

    // The injections, each a way the drift could wrongly disappear.
    //
    // A dependency in the manifest is section 17.3's first row, `불가`. A
    // subject the manifest installs and nothing uses is still not implemented,
    // and reading manifest presence as implementation is section 18.1's own
    // `package.json에 redis만 존재 ... Caching concept: NOT OBSERVED`.
    let manifest_named = Artifacts {
        intent: vec![intent_document(
            "spec-2",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            1,
            Some(BRANCH),
            "docs/spec.md",
            &["redis"],
        )?],
        ..Artifacts::default()
    };
    let with_manifest = simple(&MANIFEST_ONLY, &manifest_named)?;
    assert!(
        with_manifest.declared_dependencies().contains("redis"),
        "the manifest entry was not seen at all, so this injection proves nothing"
    );
    assert_eq!(
        drift_for(&with_manifest, "redis"),
        Some(DriftKind::IntendedNotImplemented)
    );

    // An unreachable import is section 17.3's second row, `보류`. Held is not
    // implemented either.
    let with_dead_import = simple(&UNREACHABLE_IMPORT, &manifest_named)?;
    assert!(
        !relations_for(&with_dead_import, "redis").contains(&EvidenceRelation::CodeUses),
        "an unreachable import produced PROJECT_CODE_USES"
    );
    assert_eq!(
        drift_for(&with_dead_import, "redis"),
        Some(DriftKind::IntendedNotImplemented)
    );

    // And the control: a reachable call plus configuration *is* a use, so this
    // drift is about the absence of one rather than about the spec's presence.
    let implemented = simple(&CODE_USES, &manifest_named)?;
    assert!(relations_for(&implemented, "redis").contains(&EvidenceRelation::CodeUses));
    assert_ne!(
        drift_for(&implemented, "redis"),
        Some(DriftKind::IntendedNotImplemented)
    );

    // With no intent document at all there is no intent to be unimplemented.
    let no_spec = simple(&MANIFEST_ONLY, &Artifacts::default())?;
    assert_eq!(drift_for(&no_spec, "redis"), None);
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `code_only_is_implemented_not_documented`
// ---------------------------------------------------------------------------

#[test]
fn code_only_is_implemented_not_documented() -> TestResult {
    // Section 17.5's second diagram: the code snapshot uses the subject and the
    // specification has no `PROJECT_DOC_EXPLAINS`.
    let correlation = simple(&CODE_USES, &Artifacts::default())?;
    assert!(relations_for(&correlation, "redis").contains(&EvidenceRelation::CodeUses));
    assert_eq!(
        drift_for(&correlation, "redis"),
        Some(DriftKind::ImplementedNotDocumented)
    );
    assert_eq!(
        DriftKind::ImplementedNotDocumented.as_str(),
        "IMPLEMENTED_NOT_DOCUMENTED"
    );

    // The relation section 17.5's diagram names as absent is
    // `PROJECT_DOC_EXPLAINS` and not `PROJECT_SPEC_MENTIONS`. A specification
    // that names the subject is intent; it is not a description of what the
    // code does, so it does not close this drift.
    let spec_only = Artifacts {
        intent: vec![intent_document(
            "spec-1",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            1,
            Some(BRANCH),
            "docs/spec.md",
            &["redis"],
        )?],
        ..Artifacts::default()
    };
    let with_spec = simple(&CODE_USES, &spec_only)?;
    assert!(relations_for(&with_spec, "redis").contains(&EvidenceRelation::SpecMentions));
    assert_eq!(
        drift_for(&with_spec, "redis"),
        Some(DriftKind::ImplementedNotDocumented),
        "a specification closed a documentation drift"
    );

    // The injection: a behaviour document explaining the subject closes it, and
    // one explaining a different subject does not.
    let documented = Artifacts {
        behavior: vec![behavior_document(
            "behaviour-1",
            "docs/behaviour.md",
            &["redis"],
        )?],
        ..Artifacts::default()
    };
    assert_eq!(drift_for(&simple(&CODE_USES, &documented)?, "redis"), None);

    let documents_another = Artifacts {
        behavior: vec![behavior_document(
            "behaviour-1",
            "docs/behaviour.md",
            &["distributed-lock"],
        )?],
        ..Artifacts::default()
    };
    assert_eq!(
        drift_for(&simple(&CODE_USES, &documents_another)?, "redis"),
        Some(DriftKind::ImplementedNotDocumented)
    );

    // A test-scoped observation is not `PROJECT_CODE_USES`, so an undocumented
    // test helper is not an undocumented implementation.
    let test_only = simple(&TEST_EXERCISES, &Artifacts::default())?;
    assert!(relations_for(&test_only, "redis").contains(&EvidenceRelation::TestExercises));
    assert_eq!(drift_for(&test_only, "redis"), None);
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `current_execution_prefers_same_snapshot_evidence`
// ---------------------------------------------------------------------------

#[test]
fn current_execution_prefers_same_snapshot_evidence() -> TestResult {
    // Section 30.3 row four, quoted: `같은 snapshot의 runtime/config/code direct
    // evidence > user clarification > AI`.
    let here = "snap_here";
    let elsewhere = "snap_elsewhere";
    let candidates = vec![
        Candidate::new(
            "same-snapshot",
            AnswerSource::DirectEvidence {
                snapshot_id: here.to_owned(),
            },
        ),
        Candidate::new(
            "other-snapshot",
            AnswerSource::DirectEvidence {
                snapshot_id: elsewhere.to_owned(),
            },
        ),
        Candidate::new("clarification", AnswerSource::UserClarification),
        Candidate::new("model", AnswerSource::ModelInference),
    ];
    let answer = active_view(AuthorityLane::Implementation, here, &candidates)?;
    assert_eq!(
        answer
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("same-snapshot")
    );

    let rank = |id: &str| -> u16 {
        answer
            .ranked()
            .iter()
            .find(|candidate| candidate.id() == id)
            .map_or(
                u16::MAX,
                academic_repository_correlation::RankedCandidate::rank,
            )
    };
    // The whole order of row four, as `academic-ledger`'s table ranks it.
    assert!(rank("same-snapshot") > rank("clarification"));
    assert!(rank("clarification") > rank("model"));
    // And the qualifier this crate owns: `같은 snapshot`. Direct evidence about
    // another snapshot loses to a user clarification rather than beating it.
    assert!(rank("other-snapshot") < rank("clarification"));

    // The injection: with the same-snapshot candidate removed, the winner is
    // the clarification and not the other snapshot's direct evidence. Without
    // this the assertion above would pass on a resolver that ranked every
    // direct observation first and merely happened to list this one earlier.
    let without_here: Vec<Candidate> = candidates
        .iter()
        .filter(|candidate| candidate.id() != "same-snapshot")
        .cloned()
        .collect();
    let narrowed = active_view(AuthorityLane::Implementation, here, &without_here)?;
    assert_eq!(
        narrowed
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("clarification")
    );

    // Row four's conflict column is `spec은 intent lane에 보존`: an approved
    // specification is not an answer to *what currently executes*.
    let with_spec = vec![
        Candidate::new(
            "spec",
            AnswerSource::IntentDocument {
                document: DocumentId::new("spec-1")?,
                kind: IntentDocumentKind::Specification,
                status: ApprovalStatus::Approved,
                revision: 9,
            },
        ),
        Candidate::new("clarification", AnswerSource::UserClarification),
    ];
    let mixed = active_view(AuthorityLane::Implementation, here, &with_spec)?;
    assert_eq!(
        mixed
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("clarification"),
        "a specification answered the implementation question"
    );
    // Nothing was dropped: the specification is still listed, at rank zero.
    assert_eq!(mixed.ranked().len(), 2);
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `approved_intent_prefers_valid_spec_or_adr`
// ---------------------------------------------------------------------------

fn document_candidate(
    id: &str,
    kind: IntentDocumentKind,
    status: ApprovalStatus,
    revision: u64,
) -> Result<Candidate, Box<dyn Error>> {
    Ok(Candidate::new(
        id,
        AnswerSource::IntentDocument {
            document: DocumentId::new(id)?,
            kind,
            status,
            revision,
        },
    ))
}

#[test]
fn approved_intent_prefers_valid_spec_or_adr() -> TestResult {
    // Section 30.3 row five, quoted: `승인된 최신 spec/ADR > user clarification
    // > AI`.
    let here = "snap_here";
    let candidates = vec![
        document_candidate(
            "spec-current",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            3,
        )?,
        Candidate::new("clarification", AnswerSource::UserClarification),
        Candidate::new("model", AnswerSource::ModelInference),
    ];
    let answer = active_view(AuthorityLane::Intent, here, &candidates)?;
    assert_eq!(
        answer
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("spec-current")
    );
    let rank = |answer: &academic_repository_correlation::LaneAnswer, id: &str| -> u16 {
        answer
            .ranked()
            .iter()
            .find(|candidate| candidate.id() == id)
            .map_or(
                u16::MAX,
                academic_repository_correlation::RankedCandidate::rank,
            )
    };
    assert!(rank(&answer, "spec-current") > rank(&answer, "clarification"));
    assert!(rank(&answer, "clarification") > rank(&answer, "model"));

    // An architecture decision is the other half of `spec/ADR` and ranks the
    // same, so the same corpus with an ADR in place of the specification wins
    // the same way.
    let by_adr = vec![
        document_candidate(
            "adr-current",
            IntentDocumentKind::ArchitectureDecision,
            ApprovalStatus::Approved,
            3,
        )?,
        Candidate::new("clarification", AnswerSource::UserClarification),
    ];
    assert_eq!(
        active_view(AuthorityLane::Intent, here, &by_adr)?
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("adr-current")
    );

    // The three injections, one per word of `승인된 최신`. Each replaces the
    // winner with the clarification, which is what says the qualifier bites
    // rather than that a document always wins.
    for (name, status, revision) in [
        ("draft", ApprovalStatus::Draft, 3_u64),
        ("deprecated", ApprovalStatus::Deprecated, 3),
    ] {
        let weakened = vec![
            document_candidate(name, IntentDocumentKind::Specification, status, revision)?,
            Candidate::new("clarification", AnswerSource::UserClarification),
        ];
        let answer = active_view(AuthorityLane::Intent, here, &weakened)?;
        assert_eq!(
            answer
                .winner()
                .map(academic_repository_correlation::RankedCandidate::id),
            Some("clarification"),
            "a {name} specification answered the intent question"
        );
        assert_eq!(answer.ranked().len(), 2, "the {name} document was dropped");
    }

    // `최신`: an approved document below the highest approved revision has been
    // superseded, and a superseded approval is not the latest word.
    let superseded = vec![
        document_candidate(
            "spec-old",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            2,
        )?,
        document_candidate(
            "spec-new",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            5,
        )?,
        Candidate::new("clarification", AnswerSource::UserClarification),
    ];
    let answer = active_view(AuthorityLane::Intent, here, &superseded)?;
    assert_eq!(
        answer
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("spec-new")
    );
    assert!(rank(&answer, "spec-old") < rank(&answer, "clarification"));

    // Row five's conflict column is `code와 drift 생성`: a direct code
    // observation is not an answer to *what we approved to build*.
    let with_code = vec![
        Candidate::new(
            "code",
            AnswerSource::DirectEvidence {
                snapshot_id: here.to_owned(),
            },
        ),
        Candidate::new("clarification", AnswerSource::UserClarification),
    ];
    let mixed = active_view(AuthorityLane::Intent, here, &with_code)?;
    assert_eq!(
        mixed
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("clarification"),
        "a code observation answered the intent question"
    );
    assert_eq!(mixed.ranked().len(), 2);

    // Section 30.3 has no row for a document that describes current behaviour,
    // so there is no precedence to compute for that lane and none is invented.
    assert!(matches!(
        active_view(AuthorityLane::Description, here, &candidates),
        Err(CorrelationError::LaneHasNoAuthorityRow(
            AuthorityLane::Description
        ))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `conflict_creates_drift_without_overwrite`
// ---------------------------------------------------------------------------

/// One edge, reduced to what a comparison between two runs can be made over.
fn edge_shape(edge: &RelationEdge) -> (String, String, String) {
    let evidence = match edge.evidence() {
        EdgeEvidence::Analysis {
            rung,
            tier,
            artifact_scope,
            locators,
        } => format!(
            "analysis:{}:{}:{}:{}",
            rung.as_str(),
            tier.as_str(),
            artifact_scope.as_str(),
            locators
                .iter()
                .map(|locator| format!("{}#{}", locator.path(), locator.span().start()))
                .collect::<Vec<_>>()
                .join(",")
        ),
        EdgeEvidence::Document {
            document,
            status,
            revision,
            path,
        } => format!(
            "document:{}:{}:{revision}:{path}",
            document.as_str(),
            status.as_str()
        ),
        EdgeEvidence::Incident {
            incident,
            occurred_at,
        } => format!("incident:{}:{occurred_at}", incident.as_str()),
    };
    (
        edge.relation().as_str().to_owned(),
        edge.subject().to_owned(),
        evidence,
    )
}

fn shapes(edges: &[&RelationEdge]) -> Vec<(String, String, String)> {
    edges.iter().map(|edge| edge_shape(edge)).collect()
}

#[test]
fn conflict_creates_drift_without_overwrite() -> TestResult {
    let spec = intent_document(
        "spec-1",
        IntentDocumentKind::Specification,
        ApprovalStatus::Approved,
        1,
        Some(BRANCH),
        "docs/spec.md",
        &["redis"],
    )?;

    // The two sides on their own.
    let intent_alone = simple(
        &MANIFEST_ONLY,
        &Artifacts {
            intent: vec![spec.clone()],
            ..Artifacts::default()
        },
    )?;
    let implementation_alone = simple(&CODE_USES, &Artifacts::default())?;

    // The two sides together: the specification says one thing about the
    // subject and the snapshot says another.
    let both = simple(
        &CODE_USES,
        &Artifacts {
            intent: vec![spec],
            ..Artifacts::default()
        },
    )?;

    // Neither lane's view of the subject changed by the other being present.
    assert_eq!(
        shapes(&both.lane_view(AuthorityLane::Intent, "redis")),
        shapes(&intent_alone.lane_view(AuthorityLane::Intent, "redis")),
        "the intent lane was rewritten by the implementation lane"
    );
    assert_eq!(
        shapes(&both.lane_view(AuthorityLane::Implementation, "redis")),
        shapes(&implementation_alone.lane_view(AuthorityLane::Implementation, "redis")),
        "the implementation lane was rewritten by the intent lane"
    );
    assert!(!both.lane_view(AuthorityLane::Intent, "redis").is_empty());
    assert!(
        !both
            .lane_view(AuthorityLane::Implementation, "redis")
            .is_empty()
    );

    // Nothing was dropped: every edge is in exactly one lane view and the three
    // views account for the whole edge set.
    let counted: usize = AuthorityLane::ALL
        .iter()
        .map(|lane| both.lane_view(*lane, "redis").len())
        .sum();
    assert_eq!(
        counted,
        both.relations()
            .iter()
            .filter(|edge| edge.subject() == "redis")
            .count()
    );

    // And a drift stands beside both, carrying each side as it is.
    let drift = both
        .drifts()
        .iter()
        .find(|drift| drift.subject() == "redis")
        .ok_or("the conflict produced no drift")?;
    assert_eq!(drift.kind(), DriftKind::ImplementedNotDocumented);
    assert_eq!(
        shapes(&drift.intent_side().iter().collect::<Vec<_>>()),
        shapes(&both.lane_view(AuthorityLane::Intent, "redis"))
    );
    assert_eq!(
        shapes(&drift.implementation_side().iter().collect::<Vec<_>>()),
        shapes(&both.lane_view(AuthorityLane::Implementation, "redis"))
    );
    assert!(drift.description_side().is_empty());

    // The other direction of the same conflict: intent with no implementation.
    // Both drift kinds are reachable over one corpus, and neither erases the
    // other's side.
    let other_way = simple(
        &MANIFEST_ONLY,
        &Artifacts {
            intent: vec![intent_document(
                "spec-2",
                IntentDocumentKind::Specification,
                ApprovalStatus::Approved,
                1,
                Some(BRANCH),
                "docs/spec.md",
                &["distributed-lock"],
            )?],
            ..Artifacts::default()
        },
    )?;
    let other_drift = other_way
        .drifts()
        .iter()
        .find(|drift| drift.subject() == "distributed-lock")
        .ok_or("the intent-only conflict produced no drift")?;
    assert_eq!(other_drift.kind(), DriftKind::IntendedNotImplemented);
    assert_eq!(other_drift.intent_side().len(), 1);
    assert!(other_drift.implementation_side().is_empty());

    // Each lane still answers its own question with its own evidence, with the
    // drift standing.
    let implementation = active_view(
        AuthorityLane::Implementation,
        both.snapshot_id(),
        &[
            Candidate::new(
                "code",
                AnswerSource::DirectEvidence {
                    snapshot_id: both.snapshot_id().to_owned(),
                },
            ),
            document_candidate(
                "spec-1",
                IntentDocumentKind::Specification,
                ApprovalStatus::Approved,
                1,
            )?,
        ],
    )?;
    assert_eq!(
        implementation
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("code")
    );
    let intent = active_view(
        AuthorityLane::Intent,
        both.snapshot_id(),
        &[
            Candidate::new(
                "code",
                AnswerSource::DirectEvidence {
                    snapshot_id: both.snapshot_id().to_owned(),
                },
            ),
            document_candidate(
                "spec-1",
                IntentDocumentKind::Specification,
                ApprovalStatus::Approved,
                1,
            )?,
        ],
    )?;
    assert_eq!(
        intent
            .winner()
            .map(academic_repository_correlation::RankedCandidate::id),
        Some("spec-1")
    );

    // `CONTRIBUTING.md` rule 2: a correction is a new event. Correlating again
    // with one more document appends and rewrites nothing -- every edge of the
    // earlier run survives byte for byte in the later one.
    let corrected = simple(
        &CODE_USES,
        &Artifacts {
            intent: vec![intent_document(
                "spec-1",
                IntentDocumentKind::Specification,
                ApprovalStatus::Approved,
                1,
                Some(BRANCH),
                "docs/spec.md",
                &["redis"],
            )?],
            behavior: vec![behavior_document(
                "behaviour-1",
                "docs/behaviour.md",
                &["redis"],
            )?],
            ..Artifacts::default()
        },
    )?;
    let before: BTreeSet<(String, String, String)> =
        both.relations().iter().map(edge_shape).collect();
    let after: BTreeSet<(String, String, String)> =
        corrected.relations().iter().map(edge_shape).collect();
    assert!(
        before.is_subset(&after),
        "correlating again with more evidence lost an edge"
    );
    assert!(after.len() > before.len(), "the correction added nothing");
    // The correction resolves the drift by adding a description, not by
    // deleting either side.
    assert_eq!(drift_for(&corrected, "redis"), None);
    assert!(
        !corrected
            .lane_view(AuthorityLane::Intent, "redis")
            .is_empty()
    );
    assert!(
        !corrected
            .lane_view(AuthorityLane::Implementation, "redis")
            .is_empty()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `deprecated_flagged_undeployed_branch_scopes_are_distinct`
// ---------------------------------------------------------------------------

/// The base: an intent-only drift with every one of the four scopes absent.
///
/// The specification is approved rather than deprecated, names this snapshot's
/// own branch, no flag gates the subject, and a deployment target is running
/// this very snapshot. Each variant below flips exactly one of those.
fn scope_base(snapshot_id: &str) -> Result<Artifacts, Box<dyn Error>> {
    Ok(Artifacts {
        intent: vec![intent_document(
            "spec-1",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            1,
            Some(BRANCH),
            "docs/spec.md",
            &["distributed-lock"],
        )?],
        deployments: vec![DeploymentRecord::new(
            DeploymentTarget::new("production")?,
            snapshot_id,
        )],
        ..Artifacts::default()
    })
}

fn scopes_present(
    files: &[(&str, &str)],
    branch: &str,
    artifacts: &Artifacts,
) -> Result<Vec<DriftScopeKind>, Box<dyn Error>> {
    let correlation = correlated(files, branch, V1, None, &[], artifacts)?;
    let drift = correlation
        .drifts()
        .iter()
        .find(|drift| drift.subject() == "distributed-lock")
        .ok_or("no drift for the scoped subject")?;
    Ok(drift.scopes().present())
}

#[test]
fn deprecated_flagged_undeployed_branch_scopes_are_distinct() -> TestResult {
    // The snapshot identifier is needed before the artifacts can name it, so
    // the corpus is captured once here.
    let (snapshot, _) = analyzed_with(&MANIFEST_ONLY, BRANCH, V1, None)?;
    let here = snapshot.snapshot_id().to_owned();

    // The base has a drift and no scope at all, so every assertion below is
    // about the input that was flipped.
    let base = scope_base(&here)?;
    assert_eq!(
        scopes_present(&MANIFEST_ONLY, BRANCH, &base)?,
        Vec::<DriftScopeKind>::new(),
        "the base corpus already carries a scope"
    );

    // One flip each. The four inputs are four different arguments, so no two
    // scopes can be established by the same evidence.
    let deprecated = Artifacts {
        intent: vec![intent_document(
            "spec-1",
            IntentDocumentKind::Specification,
            ApprovalStatus::Deprecated,
            1,
            Some(BRANCH),
            "docs/spec.md",
            &["distributed-lock"],
        )?],
        deployments: base.deployments.clone(),
        ..Artifacts::default()
    };
    assert_eq!(
        scopes_present(&MANIFEST_ONLY, BRANCH, &deprecated)?,
        vec![DriftScopeKind::DeprecatedSpec]
    );

    let flagged = Artifacts {
        intent: base.intent.clone(),
        deployments: base.deployments.clone(),
        flags: vec![FeatureFlagRecord::new(
            FlagKey::new("lock-rollout")?,
            FlagState::Off,
            vec![subject_id("distributed-lock")?],
        )],
        ..Artifacts::default()
    };
    assert_eq!(
        scopes_present(&MANIFEST_ONLY, BRANCH, &flagged)?,
        vec![DriftScopeKind::FeatureFlag]
    );

    let undeployed = Artifacts {
        intent: base.intent.clone(),
        deployments: vec![DeploymentRecord::new(
            DeploymentTarget::new("production")?,
            "snap_something_else",
        )],
        ..Artifacts::default()
    };
    assert_eq!(
        scopes_present(&MANIFEST_ONLY, BRANCH, &undeployed)?,
        vec![DriftScopeKind::UndeployedCode]
    );

    let branched = Artifacts {
        intent: vec![intent_document(
            "spec-1",
            IntentDocumentKind::Specification,
            ApprovalStatus::Approved,
            1,
            Some("feature/lock"),
            "docs/spec.md",
            &["distributed-lock"],
        )?],
        deployments: base.deployments.clone(),
        ..Artifacts::default()
    };
    assert_eq!(
        scopes_present(&MANIFEST_ONLY, BRANCH, &branched)?,
        vec![DriftScopeKind::BranchDifference]
    );

    // All four at once, which one enumeration could not hold: they are four
    // independent qualifiers rather than four values of one field.
    let (other_branch_snapshot, _) = analyzed_with(&MANIFEST_ONLY, "release/2", V1, None)?;
    let everything = Artifacts {
        intent: vec![intent_document(
            "spec-1",
            IntentDocumentKind::Specification,
            ApprovalStatus::Deprecated,
            1,
            Some("feature/lock"),
            "docs/spec.md",
            &["distributed-lock"],
        )?],
        deployments: vec![DeploymentRecord::new(
            DeploymentTarget::new("production")?,
            "snap_something_else",
        )],
        flags: vec![FeatureFlagRecord::new(
            FlagKey::new("lock-rollout")?,
            FlagState::Off,
            vec![subject_id("distributed-lock")?],
        )],
        ..Artifacts::default()
    };
    let correlation = correlated(&MANIFEST_ONLY, "release/2", V1, None, &[], &everything)?;
    assert_eq!(
        correlation.snapshot_id(),
        other_branch_snapshot.snapshot_id()
    );
    let drift = correlation
        .drifts()
        .iter()
        .find(|drift| drift.subject() == "distributed-lock")
        .ok_or("no drift for the scoped subject")?;
    assert_eq!(drift.scopes().present(), DriftScopeKind::ALL.to_vec());

    // The four payloads are four different types, each carrying what only it
    // can: which document, which flag and its state, which target and what it
    // runs, which two branches.
    let scopes = drift.scopes();
    assert_eq!(
        scopes
            .deprecated_spec()
            .map(|scope| scope.document().as_str().to_owned()),
        Some("spec-1".to_owned())
    );
    assert_eq!(
        scopes
            .feature_flag()
            .map(academic_repository_correlation::GatingFlag::state),
        Some(FlagState::Off)
    );
    assert_eq!(
        scopes
            .undeployed_code()
            .map(|scope| scope.deployed_snapshot().to_owned()),
        Some("snap_something_else".to_owned())
    );
    assert_eq!(
        scopes.branch_difference().map(|scope| (
            scope.intent_branch().to_owned(),
            scope.snapshot_branch().map(str::to_owned)
        )),
        Some(("feature/lock".to_owned(), Some("release/2".to_owned())))
    );

    // Removing any one input clears exactly its own scope. Without this the
    // four assertions above would pass on an implementation that set all four
    // whenever any input was present.
    for (name, artifacts, cleared) in [
        (
            "deprecation",
            Artifacts {
                intent: vec![intent_document(
                    "spec-1",
                    IntentDocumentKind::Specification,
                    ApprovalStatus::Approved,
                    1,
                    Some("feature/lock"),
                    "docs/spec.md",
                    &["distributed-lock"],
                )?],
                deployments: everything.deployments.clone(),
                flags: everything.flags.clone(),
                ..Artifacts::default()
            },
            DriftScopeKind::DeprecatedSpec,
        ),
        (
            "flag",
            Artifacts {
                intent: everything.intent.clone(),
                deployments: everything.deployments.clone(),
                ..Artifacts::default()
            },
            DriftScopeKind::FeatureFlag,
        ),
        (
            "deployment",
            Artifacts {
                intent: everything.intent.clone(),
                deployments: vec![DeploymentRecord::new(
                    DeploymentTarget::new("production")?,
                    correlation.snapshot_id(),
                )],
                flags: everything.flags.clone(),
                ..Artifacts::default()
            },
            DriftScopeKind::UndeployedCode,
        ),
        (
            "branch",
            Artifacts {
                intent: vec![intent_document(
                    "spec-1",
                    IntentDocumentKind::Specification,
                    ApprovalStatus::Deprecated,
                    1,
                    Some("release/2"),
                    "docs/spec.md",
                    &["distributed-lock"],
                )?],
                deployments: everything.deployments.clone(),
                flags: everything.flags.clone(),
                ..Artifacts::default()
            },
            DriftScopeKind::BranchDifference,
        ),
    ] {
        let present = scopes_present(&MANIFEST_ONLY, "release/2", &artifacts)?;
        assert!(
            !present.contains(&cleared),
            "{name} was removed and {} stayed",
            cleared.as_str()
        );
        let expected: Vec<DriftScopeKind> = DriftScopeKind::ALL
            .into_iter()
            .filter(|kind| *kind != cleared)
            .collect();
        assert_eq!(
            present, expected,
            "removing the {name} input changed another scope too"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. `same_bytes_two_analyzers_is_analysis_changed`
// ---------------------------------------------------------------------------

/// The paths the older analyzer build has a reader for.
///
/// It has none for the configuration document, which is what one analyzer
/// version gaining a reader looks like from outside: the bytes are the same,
/// the manifest is the same, and the newer build reads one more of them.
const V1_READS: [&str; 5] = [
    "package.json",
    "src/cache.ts",
    "docs/spec.md",
    "docs/adr-001.md",
    "docs/behaviour.md",
];

#[test]
fn same_bytes_two_analyzers_is_analysis_changed() -> TestResult {
    let older = correlated(
        &CODE_USES,
        BRANCH,
        V1,
        Some(&V1_READS),
        &[],
        &Artifacts::default(),
    )?;
    let newer = correlated(&CODE_USES, BRANCH, V2, None, &[], &Artifacts::default())?;

    // The bytes are the same bytes: one snapshot, read twice.
    assert_eq!(older.snapshot_id(), newer.snapshot_id());
    assert_ne!(older.analyzer_version(), newer.analyzer_version());

    let comparison = compare(&older, &newer)?;
    assert_eq!(comparison.cause(), ChangeCause::AnalysisChanged);
    assert_eq!(ChangeCause::AnalysisChanged.as_str(), "ANALYSIS_CHANGED");
    let changes = comparison.semantic_diff();
    assert!(
        !changes.is_empty(),
        "the two analyzer builds produced identical correlations, so nothing was attributed"
    );
    for change in changes {
        assert_eq!(
            change.cause(),
            ChangeCause::AnalysisChanged,
            "{} was reported as a code change",
            change.subject()
        );
    }
    for change in comparison.dependency_diff() {
        assert_eq!(change.cause(), ChangeCause::AnalysisChanged);
    }
    // The difference is the one the newer reader produced.
    let redis = changes
        .iter()
        .find(|change| change.subject() == "redis")
        .ok_or("the newer analyzer build changed nothing about redis")?;
    assert_eq!(redis.transition(), SemanticTransition::Appeared);
    assert!(redis.after().contains(&EvidenceRelation::CodeUses));
    assert!(redis.before().is_empty());

    // The injection that stops this being a constant: the same analyzer over
    // two snapshots attributes to code instead.
    let moved_code = correlated(
        &UNREACHABLE_IMPORT,
        BRANCH,
        V1,
        None,
        &[],
        &Artifacts::default(),
    )?;
    let same_analyzer = correlated(&CODE_USES, BRANCH, V1, None, &[], &Artifacts::default())?;
    assert_ne!(moved_code.snapshot_id(), same_analyzer.snapshot_id());
    let code_comparison = compare(&moved_code, &same_analyzer)?;
    assert_eq!(code_comparison.cause(), ChangeCause::CodeChanged);
    assert!(!code_comparison.semantic_diff().is_empty());
    for change in code_comparison.semantic_diff() {
        assert_eq!(change.cause(), ChangeCause::CodeChanged);
    }

    // Both axes moving is refused rather than attributed to either.
    let both_moved = correlated(
        &UNREACHABLE_IMPORT,
        BRANCH,
        V2,
        None,
        &[],
        &Artifacts::default(),
    )?;
    assert!(matches!(
        compare(&same_analyzer, &both_moved),
        Err(CorrelationError::ConfoundedComparison(_, _))
    ));

    // Neither axis moving is refused too: there is no axis to attribute along,
    // and a difference between two such runs came from somewhere neither
    // bucket names.
    assert!(matches!(
        compare(&same_analyzer, &same_analyzer),
        Err(CorrelationError::NoComparisonAxis(_))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. `dependency_diff_and_semantic_diff_are_separate`
// ---------------------------------------------------------------------------

/// The same corpus as `MANIFEST_ONLY` with a second dependency declared and no
/// other change: a `단순 dependency diff` and nothing semantic.
const MANIFEST_PLUS_ONE: [(&str, &str); 4] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\",\n    \"distlock\": \"1.0.0\"\n  }\n}\n",
    ),
    (
        "src/orders.ts",
        "export function place() {\n  return 1;\n}\n",
    ),
    SPEC_PAGE,
    ADR_PAGE,
];

/// The same manifest as `MANIFEST_ONLY` with the code now reaching `redis`: a
/// semantic finding difference and no dependency difference.
const MANIFEST_SAME_CODE_USES: [(&str, &str); 5] = [
    PACKAGE_WITH_REDIS,
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    ("src/app.config.yaml", "cache:\n  redis: enabled\n"),
    SPEC_PAGE,
    ADR_PAGE,
];

#[test]
fn dependency_diff_and_semantic_diff_are_separate() -> TestResult {
    let base = simple(&MANIFEST_ONLY, &Artifacts::default())?;

    // A dependency declared and never used moves the first channel alone.
    let declared = simple(&MANIFEST_PLUS_ONE, &Artifacts::default())?;
    let first = compare(&base, &declared)?;
    assert_eq!(
        first
            .dependency_diff()
            .iter()
            .map(|change| (change.subject(), change.direction()))
            .collect::<Vec<_>>(),
        vec![("distributed-lock", PresenceChange::Added)]
    );
    assert!(
        first.semantic_diff().is_empty(),
        "a dependency nothing uses produced a semantic finding change: {:?}",
        first
            .semantic_diff()
            .iter()
            .map(|change| (change.subject().to_owned(), change.transition()))
            .collect::<Vec<_>>()
    );

    // A use that declares nothing new moves the second channel alone.
    let used = simple(&MANIFEST_SAME_CODE_USES, &Artifacts::default())?;
    let second = compare(&base, &used)?;
    assert!(
        second.dependency_diff().is_empty(),
        "a code change with an unchanged manifest produced a dependency change"
    );
    assert_eq!(
        second
            .semantic_diff()
            .iter()
            .map(|change| (change.subject().to_owned(), change.transition()))
            .collect::<Vec<_>>(),
        vec![
            ("redis".to_owned(), SemanticTransition::Appeared),
            ("redis".to_owned(), SemanticTransition::DriftAppeared),
        ]
    );

    // The two channels stay separate in the other direction too: removing the
    // use is section 18.1's `NO_LONGER_OBSERVED` in the second channel and
    // nothing in the first, because the dependency is still declared.
    let removed = compare(&used, &base)?;
    assert!(removed.dependency_diff().is_empty());
    assert!(
        removed
            .semantic_diff()
            .iter()
            .any(|change| change.transition() == SemanticTransition::NoLongerObserved)
    );
    assert_eq!(
        SemanticTransition::NoLongerObserved.as_str(),
        "NO_LONGER_OBSERVED"
    );

    // Both channels move when both things happen, and each entry is still in
    // its own channel: the split is not a claim that only one can move.
    let both = compare(&declared, &used)?;
    assert_eq!(
        both.dependency_diff()
            .iter()
            .map(|change| (change.subject(), change.direction()))
            .collect::<Vec<_>>(),
        vec![("distributed-lock", PresenceChange::Removed)]
    );
    assert!(
        both.semantic_diff()
            .iter()
            .any(|change| change.subject() == "redis")
    );
    assert!(
        !both
            .semantic_diff()
            .iter()
            .any(|change| change.subject() == "distributed-lock"),
        "a dependency-only subject reached the semantic channel"
    );
    Ok(())
}
