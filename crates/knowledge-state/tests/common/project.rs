//! `P2-R1` → `P2-R2` → `P2-R3` → `P2-R4`, restated for this suite.
//!
//! `crates/repository-classification/tests/classification_lanes.rs` builds this
//! and a test module is not a library target, so it is restated here from the
//! same public APIs. Nothing is fabricated: the snapshot comes out of
//! `capture_local`, the findings out of `EvidenceLadder::classify`, the
//! correlation out of `correlate` and the stances out of `classify`.
//!
//! Two corpora, and the difference between them is the whole of section 13.2's
//! fourth and seventh rows:
//!
//! * [`OBSERVED_REDIS`] imports the subject and calls it from a reachable entry
//!   point with a production configuration, which is `P2-R2`'s third rung and
//!   therefore `EvidenceTier::Observed` and therefore an `ObservedProof`; and
//! * [`INSTALLED_ONLY`] names the subject in a manifest and nowhere else, which
//!   is `P2-R2`'s first rung and therefore no `ObservedProof` at all.
//!
//! Neither of those decisions is taken here.

#![allow(dead_code)]

use std::{collections::BTreeMap, error::Error};

use academic_model_run::{
    CalibrationBin, CalibrationDataset, CalibrationDatasetId, CalibrationRegistry, Digest32,
    ModelVersion, ProviderId, Purpose,
};
use academic_repository::{
    CommitId, PathPolicy, RepositoryId, RepositorySnapshot, RepositorySource, SnapshotRequest,
    SourceEntry, SourceTree, ToolVersion, WorkingTreeFacts, capture_local,
};
use academic_repository_analysis::{
    AnalysisInput, AnalyzerIdentity, EvidenceLadder, Finding, RepositoryAnalysis, SourceUnit,
    Subject, SubjectId, analyze,
};
use academic_repository_classification::{
    BenefitContract, BenefitDimension, ClassificationInput, ClassificationSet, ConceptStance,
    GoalId, GoalScope, TradeOff, Trigger, TriggerState, classify,
};
use academic_repository_correlation::{Correlation, CorrelationInput, correlate};
use academic_untrusted_content::SourceIndex;

const CAPTURED_AT: u64 = 1_756_000_000_000;
const HEAD: &str = "abc1234def5678";
const BRANCH: &str = "main";
const ANALYZER: &str = "academic-repository-analysis";
const VERSION: &str = "0.1.0";
const NOW: u64 = 5_000;

const SPEC_PAGE: (&str, &str) = ("docs/spec.md", "# Order platform\n\nA specification.\n");

/// `redis` reachable from an entry point with a production configuration:
/// `P2-R2`'s third row, so `OBSERVED`.
pub const OBSERVED_REDIS: [(&str, &str); 4] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  redis.createClient();\n  return redis.connect();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    (
        "docker-compose.yml",
        "services:\n  cache:\n    image: redis\n",
    ),
    SPEC_PAGE,
];

/// `redis` named by a manifest and by nothing else: `P2-R2`'s first row, so
/// `PRESENT_ONLY` and no `OBSERVED`.
///
/// Section 18.1's own example, and section 13.2's seventh row read from the
/// repository side: `package.json에 redis만 존재`.
pub const INSTALLED_ONLY: [(&str, &str); 3] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/orders.ts",
        "export function place() {\n  return 1;\n}\n",
    ),
    SPEC_PAGE,
];

fn captured(files: &[(&str, &str)]) -> Result<(RepositorySnapshot, SourceIndex), Box<dyn Error>> {
    let entries: Vec<SourceEntry> = files
        .iter()
        .map(|(path, body)| SourceEntry::new(*path, body.as_bytes().to_vec()))
        .collect();
    let facts = WorkingTreeFacts::checkout(
        CommitId::new(HEAD)?,
        Some(BRANCH.to_owned()),
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
        analysis_policy_hash: academic_policy::ContentDigest::of(b"analysis-policy-v1"),
        tool_versions: vec![ToolVersion::new(ANALYZER, VERSION)?],
    };
    let (capture, sealed) = capture_local(&request)?;
    Ok((capture.snapshot, sealed))
}

fn analyzed(
    files: &[(&str, &str)],
) -> Result<(RepositorySnapshot, RepositoryAnalysis), Box<dyn Error>> {
    let (snapshot, sealed) = captured(files)?;
    let bodies: BTreeMap<&str, &str> = files.iter().copied().collect();
    let units: Vec<SourceUnit> = snapshot
        .manifest()
        .iter()
        .map(|entry| SourceUnit::new(entry.path(), bodies[entry.path()].as_bytes().to_vec()))
        .collect();
    let identity = AnalyzerIdentity::new(ANALYZER, VERSION)?;
    let input = AnalysisInput::of(&snapshot, &sealed, identity, units)?;
    let analysis = analyze(&input)?;
    Ok((snapshot, analysis))
}

fn registry() -> Result<CalibrationRegistry, Box<dyn Error>> {
    let mut registry = CalibrationRegistry::new();
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-knowledge-state")?,
        ProviderId::new(ANALYZER)?,
        ModelVersion::new(VERSION)?,
        purpose()?,
        Digest32::of(VERSION.as_bytes()),
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

/// The one subject every corpus here is searched for.
pub const SUBJECT: &str = "redis";

fn subjects() -> Result<Vec<Subject>, Box<dyn Error>> {
    Ok(vec![Subject::new(
        SubjectId::new(SUBJECT)?,
        &[SUBJECT],
        &[SUBJECT],
        &[SUBJECT],
        &[SUBJECT],
    )])
}

fn findings_of(analysis: &RepositoryAnalysis) -> Result<Vec<Finding>, Box<dyn Error>> {
    let registry = registry()?;
    let purpose = purpose()?;
    let mut found = Vec::new();
    for subject in subjects()? {
        // Propagated rather than swallowed: a ladder that refused would
        // otherwise reach this suite as an empty corpus, and an empty corpus is
        // the shape in which a fixture agrees with everything.
        found.extend(EvidenceLadder::classify(
            analysis,
            &subject,
            &registry,
            &purpose,
            &[],
            NOW,
        )?);
    }
    Ok(found)
}

/// One capture, one analysis and one correlation.
pub struct Corpus {
    snapshot: RepositorySnapshot,
    correlation: Correlation,
}

impl Corpus {
    /// The snapshot identity, as `P2-R1` minted it.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        self.snapshot.snapshot_id()
    }
}

/// Builds one corpus through the four crates below this one.
pub fn built(files: &[(&str, &str)]) -> Result<Corpus, Box<dyn Error>> {
    let (snapshot, analysis) = analyzed(files)?;
    let findings = findings_of(&analysis)?;
    let identity = AnalyzerIdentity::new(ANALYZER, VERSION)?;
    let correlation = correlate(&CorrelationInput {
        snapshot: &snapshot,
        analyzer: &identity,
        findings: &findings,
        intent_documents: &[],
        behavior_documents: &[],
        incidents: &[],
        feature_flags: &[],
        deployments: &[],
    })?;
    Ok(Corpus {
        snapshot,
        correlation,
    })
}

/// A goal scope.
pub fn goal(name: &str, version: u64) -> Result<GoalScope, Box<dyn Error>> {
    Ok(GoalScope::at(GoalId::new(name)?, version))
}

/// `P2-R4`'s classification of one corpus under one goal.
pub fn classified(corpus: &Corpus, goal: &GoalScope) -> Result<ClassificationSet, Box<dyn Error>> {
    classified_with(corpus, goal, &[])
}

/// The same, with forward-looking contracts.
///
/// `P2-R4` publishes a stance for a concept the goal names *or* the snapshot
/// observes, so a concept a project merely installs has no stance until
/// something names it. That is why the dependency-only fixture below carries a
/// benefit contract: section 18.1's own `package.json에 redis만 존재` case is a
/// concept the project has an interest in and the snapshot does **not**
/// observe.
pub fn classified_with(
    corpus: &Corpus,
    goal: &GoalScope,
    beneficial: &[BenefitContract],
) -> Result<ClassificationSet, Box<dyn Error>> {
    Ok(classify(&ClassificationInput {
        correlation: &corpus.correlation,
        goal,
        required: &[],
        beneficial,
        overrides: &[],
    })?)
}

/// A benefit contract naming [`SUBJECT`], with all four parts section 18.3
/// requires.
pub fn benefit_contract() -> Result<BenefitContract, Box<dyn Error>> {
    Ok(BenefitContract::new(
        &SubjectId::new(SUBJECT)?,
        vec![Trigger::new("read-load-above-measured-primary-capacity")?],
        TriggerState::NotMet,
        BenefitDimension::Scale,
        vec![TradeOff::new("operational-surface")?],
    )?)
}

/// The one stance for [`SUBJECT`] in a classification set.
pub fn stance_of(set: &ClassificationSet) -> Result<ConceptStance, Box<dyn Error>> {
    let matching: Vec<&ConceptStance> = set
        .stances()
        .iter()
        .filter(|stance| stance.key().concept() == SUBJECT)
        .collect();
    match matching.as_slice() {
        [one] => Ok((*one).clone()),
        other => Err(format!("{SUBJECT} has {} stances, not one", other.len()).into()),
    }
}
