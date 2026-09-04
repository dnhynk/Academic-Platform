//! `P2-Y1`'s named acceptance evidence.
//!
//! Two things this suite refuses to restate.
//!
//! **The design document is the oracle for every list.** Section 24.1's
//! example competency — its context, its three performance criteria, its six
//! enabling concepts and its three rubric rows — is read out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and used as the
//! fixture, and section 24.3's six evidence stages are read out of that
//! section's own sentence. No count in this file is written as a number that
//! the document is not also asked for.
//!
//! **The evidence is produced rather than fabricated.** Every repository claim
//! here is captured through `P2-R1`'s own `capture_local`, analyzed through
//! `P2-R2`'s own ladder, correlated through `P2-R3`'s own `correlate`,
//! classified through `P2-R4`'s own `classify` and promoted through `P2-R5`'s
//! own `promote`; every knowledge-state item is admitted through `P2-N2`'s own
//! four eligibility checks. This crate has no constructor that skips either
//! chain.

use std::{collections::BTreeMap, collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_competency::{
    CellState, Competency, CompetencyError, CompetencyId, ConceptNamespace, ConceptRef,
    ContributionImportance, CriterionId, EnablingConcept, EnablingGraph, EvidenceOrigin,
    EvidenceRubric, EvidenceSource, EvidenceStage, Necessity, PerformanceCriterion,
    PromotingEvidence, RecordId, RubricRow, Situation, StageEvidence, declare, fill,
};
use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, EntityId, EpistemicStatus,
    EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength, FreshnessBand,
    MasteryLevel, PredicateId, ScopeId, TimestampMillis, ValidInterval,
    entity_registry::EntityKind,
    predicates::{Cardinality, EdgeDirection, NodeType, QualifierKind},
};
use academic_knowledge_state::{
    ConceptEvidence, ConceptLink, DependencyOnly, EligibilityOutcome, EvidenceCeiling,
    EvidenceDossier, EvidenceKind, ExerciseOutcome, IncidentRepair, Outcome, Participation,
    ProjectUse, SelfExplanation, SourceIntegrity, TransferContext, TransferRepetition,
    UserConfirmation,
};
use academic_model_run::{
    CalibrationBin, CalibrationDataset, CalibrationDatasetId, CalibrationRegistry, Digest32,
    ModelVersion, ProviderId, Purpose,
};
use academic_policy::ContentDigest as PolicyDigest;
use academic_repository::{
    CommitId, PathPolicy, RepositoryId, RepositorySnapshot, RepositorySource, SnapshotRequest,
    SourceEntry, SourceTree, ToolVersion, WorkingTreeFacts, capture_local,
};
use academic_repository_analysis::{
    AnalysisInput, AnalyzerIdentity, EvidenceLadder, Finding, Locator, PathClass,
    RepositoryAnalysis, SourceUnit, Subject, SubjectId, analyze,
};
use academic_repository_classification::{
    ClassificationInput, ClassificationSet, ConcreteNeed, ControllingMechanism, CurrentBasis,
    GoalId, GoalScope, NeedKind, ProofChain, RequiredConcept, UserEvidenceGap, classify,
};
use academic_repository_competency::{
    AuthoredWork, AuthorshipMap, ChangeId, ChangeKind, ChangedSite, ContributionDraft,
    ContributionKind, ContributionRecord, ExternalAuthorId, IdentitySource, OriginReport,
    PersonalApplicationClaim, PromotionInput, PromotionSet, RejectionReason, RubricId,
    ScaffoldRubric, UserId, promote,
};
use academic_repository_correlation::{Correlation, CorrelationInput, correlate};
use academic_untrusted_content::SourceIndex;

type TestResult = Result<(), Box<dyn Error>>;

const CAPTURED_AT: u64 = 1_756_000_000_000;
const HEAD: &str = "abc1234def5678";
const BRANCH: &str = "main";
const ANALYZER: &str = "academic-repository-analysis";
const VERSION: &str = "0.1.0";
const NOW: u64 = 5_000;
const USER: &str = "user-1";
const OWN_ADDRESS: &str = "owner@example.test";

// ---------------------------------------------------------------------------
// The design document, which is this suite's oracle for every list.
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn design_page() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

fn section(page: &str, start: &str, end: &str) -> Result<String, Box<dyn Error>> {
    let from = page
        .find(start)
        .ok_or_else(|| format!("{start} is not in the design document"))?;
    let rest = &page[from..];
    let to = rest
        .find(end)
        .ok_or_else(|| format!("{start} does not end before {end}"))?;
    Ok(rest[..to].to_owned())
}

/// Section 24.1's example competency, exactly as the document writes it.
///
/// Nothing here is a list this file chose. The criteria, the enabling concepts
/// and the rubric rows are the document's own, so their number is a measurement
/// of the specification.
#[derive(Debug)]
struct SpecExample {
    id: String,
    statement: String,
    context: String,
    criteria: Vec<String>,
    concepts: Vec<String>,
    rubric: Vec<String>,
}

fn section_24_1_example() -> Result<SpecExample, Box<dyn Error>> {
    let page = design_page()?;
    let block = section(&page, "### 24.1", "### 24.2")?;
    let yaml = section(&block, "```yaml", "```\n")?;

    let scalar = |key: &str| -> Result<String, Box<dyn Error>> {
        let prefix = format!("  {key}: ");
        yaml.lines()
            .find_map(|line| line.strip_prefix(prefix.as_str()))
            .map(|value| value.trim().trim_matches('"').to_owned())
            .ok_or_else(|| format!("section 24.1's example has no {key}").into())
    };
    let items = |key: &str| -> Result<Vec<String>, Box<dyn Error>> {
        let header = format!("  {key}:");
        let mut rows = Vec::new();
        let mut inside = false;
        for line in yaml.lines() {
            if line.trim_end() == header {
                inside = true;
                continue;
            }
            if inside {
                match line.strip_prefix("    - ") {
                    Some(item) => rows.push(item.trim().to_owned()),
                    None => break,
                }
            }
        }
        if rows.is_empty() {
            return Err(format!("section 24.1's example lists no {key}").into());
        }
        Ok(rows)
    };
    let inline = |key: &str| -> Result<Vec<String>, Box<dyn Error>> {
        let raw = scalar(key)?;
        let inner = raw
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .ok_or_else(|| format!("section 24.1's {key} is not an inline list"))?;
        Ok(inner
            .split(',')
            .map(|item| item.trim().to_owned())
            .filter(|item| !item.is_empty())
            .collect())
    };

    Ok(SpecExample {
        id: scalar("id")?,
        statement: scalar("statement")?,
        context: scalar("context")?,
        criteria: items("performanceCriteria")?,
        concepts: inline("enabledByConcepts")?,
        rubric: items("evidenceRubric")?,
    })
}

/// Section 24.3's evidence stage names, read out of the sentence that names
/// them.
///
/// The line is located by the phrase it ends with, and every backticked span on
/// it is taken whole. A stage renamed, added or dropped in the design document
/// changes what this returns.
fn section_24_3_stage_names() -> Result<Vec<String>, Box<dyn Error>> {
    let page = design_page()?;
    let block = section(&page, "### 24.3", "### 24.4")?;
    let line = block
        .lines()
        .find(|line| line.contains("evidence를 구분한다"))
        .ok_or("section 24.3 does not separate its evidence stages")?;
    let head = line
        .split_once("evidence를 구분한다")
        .map(|(before, _)| before)
        .ok_or("section 24.3's stage sentence has no head")?;
    Ok(head
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect())
}

// ---------------------------------------------------------------------------
// Domain fixtures.
// ---------------------------------------------------------------------------

/// A deterministic UUIDv7-shaped identifier for one fixture name.
fn uuid_of(tag: &str) -> uuid::Uuid {
    let digest = ContentDigest::sha256(tag.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

fn entity(tag: &str) -> EntityId {
    EntityId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn evidence_id(tag: &str) -> EvidenceId {
    EvidenceId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn artifact_id(tag: &str) -> academic_domain::ArtifactId {
    academic_domain::ArtifactId::try_from_uuid(uuid_of(tag))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn claim_id(tag: &str) -> ClaimId {
    ClaimId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn scope() -> ScopeId {
    ScopeId::try_from_uuid(uuid_of("scope-competency"))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn user_actor() -> Actor {
    Actor::User {
        user_id: entity("the-one-user"),
    }
}

fn evidence_item(tag: &str) -> EvidenceItem {
    EvidenceItem {
        id: evidence_id(tag),
        artifact_id: artifact_id(tag),
        locator: EvidenceLocator::Page { page_number: 1 },
        excerpt_digest: ContentDigest::sha256(tag.as_bytes()),
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "fixture".to_owned(),
        extractor_version: "1".to_owned(),
    }
}

fn confirmation(concept: EntityId) -> Result<UserConfirmation, Box<dyn Error>> {
    let evidence = evidence_item("confirmation");
    let claim = Claim {
        id: claim_id("claim"),
        subject_entity_id: concept,
        predicate_id: PredicateId::parse(academic_knowledge_state::STATE_CONFIRMATION_PREDICATE)?,
        object: ClaimObject::Mastery(MasteryLevel::Understood),
        scope_id: scope(),
        authority_class: AuthorityClass::UserExplicit,
        epistemic_status: EpistemicStatus::UserConfirmed,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::new(TimestampMillis::new(0), None)?,
        evidence_ids: vec![evidence.id],
    };
    Ok(UserConfirmation::verify(
        &user_actor(),
        &claim,
        &evidence,
        concept,
        MasteryLevel::Understood,
        TimestampMillis::new(i64::try_from(NOW)?),
    )?)
}

fn full_dossier(concept: EntityId) -> EvidenceDossier {
    EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Authored,
        Outcome::Succeeded,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    )
}

/// One admitted `P2-N2` item, through that crate's own four checks.
fn admitted(
    evidence: ConceptEvidence,
    tag: &str,
    concept: EntityId,
) -> Result<academic_knowledge_state::EligibleEvidence, Box<dyn Error>> {
    EligibilityOutcome::admit(evidence, evidence_id(tag), &full_dossier(concept))
        .admitted()
        .cloned()
        .ok_or_else(|| format!("{tag} was not admitted by P2-N2").into())
}

// ---------------------------------------------------------------------------
// The repository chain: `P2-R1`, `P2-R2`, `P2-R3`, `P2-R4`, then `P2-R5`.
// ---------------------------------------------------------------------------

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
        analysis_policy_hash: PolicyDigest::of(b"analysis-policy-v1"),
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
    drop(input);
    Ok((snapshot, analysis))
}

fn registry() -> Result<CalibrationRegistry, Box<dyn Error>> {
    let mut registry = CalibrationRegistry::new();
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-competency")?,
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

fn subject_id(name: &str) -> Result<SubjectId, Box<dyn Error>> {
    Ok(SubjectId::new(name)?)
}

/// The two subjects every corpus here is searched for.
///
/// `redis` is used by the code and `express` is only declared, which is what
/// makes one corpus carry both halves of section 24.3's first sentence.
fn subjects() -> Result<Vec<Subject>, Box<dyn Error>> {
    Ok(vec![
        Subject::new(
            subject_id("redis")?,
            &["redis"],
            &["redis"],
            &["redis"],
            &["redis"],
        ),
        Subject::new(
            subject_id("express")?,
            &["express"],
            &["express"],
            &["express"],
            &["express"],
        ),
    ])
}

fn findings_of(analysis: &RepositoryAnalysis) -> Result<Vec<Finding>, Box<dyn Error>> {
    let registry = registry()?;
    let purpose = purpose()?;
    let mut found = Vec::new();
    for subject in subjects()? {
        if let Ok(findings) =
            EvidenceLadder::classify(analysis, &subject, &registry, &purpose, &[], NOW)
        {
            found.extend(findings);
        }
    }
    Ok(found)
}

struct Corpus {
    snapshot: RepositorySnapshot,
    findings: Vec<Finding>,
    correlation: Correlation,
}

fn built(files: &[(&str, &str)]) -> Result<Corpus, Box<dyn Error>> {
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
        findings,
        correlation,
    })
}

impl Corpus {
    fn finding(&self, subject: &str) -> Result<&Finding, Box<dyn Error>> {
        let matching: Vec<&Finding> = self
            .findings
            .iter()
            .filter(|finding| finding.subject() == subject)
            .collect();
        match matching.as_slice() {
            [one] => Ok(one),
            other => Err(format!("{subject} has {} findings, not one", other.len()).into()),
        }
    }

    fn site(&self, path: &str) -> Result<Locator, Box<dyn Error>> {
        let finding = self.finding("redis")?;
        finding
            .locators()
            .iter()
            .find(|locator| locator.path() == path)
            .cloned()
            .ok_or_else(|| format!("the redis finding names no locator at {path}").into())
    }

    fn snapshot_id(&self) -> &str {
        self.snapshot.snapshot_id()
    }
}

fn order_goal() -> Result<GoalScope, Box<dyn Error>> {
    Ok(GoalScope::at(
        GoalId::new("no-duplicate-retryable-orders")?,
        1,
    ))
}

fn classified(corpus: &Corpus, scope: &GoalScope) -> Result<ClassificationSet, Box<dyn Error>> {
    let finding = corpus.finding("redis")?;
    let basis = CurrentBasis::of_current_code(finding)?;
    let need = ConcreteNeed::shown_by(
        basis,
        NeedKind::FailureScenario,
        &subject_id("lost-update")?,
        finding.locators().to_vec(),
    )?;
    let mechanism = ControllingMechanism::controlling(need, &subject_id("atomicity")?);
    let concept =
        RequiredConcept::realizing(mechanism, &subject_id("isolation")?, EntityKind::Concept)?;
    let chain = ProofChain::closed_by(
        concept,
        UserEvidenceGap::of(
            MasteryLevel::Understood,
            FreshnessBand::High,
            EpistemicStatus::UserConfirmed,
        )
        .ok_or("an unapplied concept has an evidence gap")?,
    );
    Ok(classify(&ClassificationInput {
        correlation: &corpus.correlation,
        goal: scope,
        required: &[chain],
        beneficial: &[],
        overrides: &[],
    })?)
}

fn user() -> Result<UserId, Box<dyn Error>> {
    Ok(UserId::new(USER)?)
}

fn own_identity() -> Result<ExternalAuthorId, Box<dyn Error>> {
    Ok(ExternalAuthorId::new(
        IdentitySource::GitAuthorEmail,
        OWN_ADDRESS,
    )?)
}

fn mapping() -> Result<AuthorshipMap, Box<dyn Error>> {
    Ok(AuthorshipMap::of(user()?, 3, vec![own_identity()?]))
}

fn scaffold_rubric() -> Result<ScaffoldRubric, Box<dyn Error>> {
    Ok(ScaffoldRubric::of(
        RubricId::new("scaffold-v2")?,
        2,
        vec![
            ChangeKind::DependencyPin,
            ChangeKind::ConfigurationValue,
            ChangeKind::GeneratedArtifact,
            ChangeKind::Formatting,
            ChangeKind::ProjectScaffold,
        ],
        vec![PathClass::Vendored, PathClass::Generated],
        1,
    )?)
}

/// The user's own hand-written meaningful change, sealed by `P2-R5`.
fn own_work(corpus: &Corpus) -> Result<AuthoredWork, Box<dyn Error>> {
    let record = ContributionRecord {
        change: ChangeId::new("c-own")?,
        snapshot_id: corpus.snapshot_id().to_owned(),
        author: own_identity()?,
        kind: ContributionKind::Authored,
        origin: OriginReport::HandWritten,
        sites: vec![ChangedSite::new(
            corpus.site("src/cache.ts")?,
            ChangeKind::ControlFlow,
        )],
        recorded_at: 1_756_100_000_000,
    };
    let map = mapping()?;
    let rubric = scaffold_rubric()?;
    Ok(ContributionDraft::over(&record, &map, &rubric).seal()?)
}

fn promoted(corpus: &Corpus, works: &[AuthoredWork]) -> Result<PromotionSet, Box<dyn Error>> {
    let classification = classified(corpus, &order_goal()?)?;
    let user = user()?;
    Ok(promote(&PromotionInput {
        classification: &classification,
        user: &user,
        works,
        outcomes: &[],
    })?)
}

// ---------------------------------------------------------------------------
// The corpus: one snapshot, one concept used and one concept only declared.
// ---------------------------------------------------------------------------

/// `redis` is imported and called from an entry point, so `P2-R2` reaches its
/// `OBSERVED` rung. `express` appears in the manifest and nowhere else, which is
/// section 13.2's `dependency/install/import만 존재` row.
const MIXED_CORPUS: [(&str, &str); 4] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\",\n    \"express\": \"4.19.2\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  redis.createClient();\n  return redis.connect();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    (
        "docker-compose.yml",
        "services:\n  cache:\n    image: redis\n",
    ),
    ("docs/spec.md", "# Order platform\n\nA specification.\n"),
];

// ---------------------------------------------------------------------------
// The competency fixtures. Section 24.1's example is the document's own; the
// two beside it carry names section 24.2 lists in its role bundle.
// ---------------------------------------------------------------------------

const LATENCY_CRITERION: [&str; 3] = ["latency-1", "latency-2", "latency-3"];

/// The concept that the criteria naming none of the document's concepts are
/// about.
///
/// Section 24.1 lists a competency's concepts and its criteria and does not
/// bind one to the other. This crate requires the binding — see
/// `crates/competency/src/criterion.rs` — so the fixture supplies it: a
/// criterion is about every enabling concept whose own name appears in its
/// text, and about `OBSERVABILITY` when none does, because measurement is what
/// the remaining two criteria are written in terms of.
const MEASUREMENT_CONCEPT: &str = "OBSERVABILITY";

fn ontology(name: &str) -> ConceptRef {
    ConceptRef::ontology(entity(name))
}

/// Section 24.1's example competency, built from the document's own parts.
fn latency_competency() -> Result<Competency, Box<dyn Error>> {
    let example = section_24_1_example()?;
    let enabled_by: Vec<EnablingConcept> = example
        .concepts
        .iter()
        .map(|name| {
            EnablingConcept::of(
                ontology(name),
                ContributionImportance::Substantial,
                Necessity::Necessary,
            )
        })
        .collect();

    let mut criteria = Vec::new();
    for (index, text) in example.criteria.iter().enumerate() {
        let named: Vec<ConceptRef> = example
            .concepts
            .iter()
            .filter(|concept| text.contains(concept.as_str()))
            .map(|concept| ontology(concept))
            .collect();
        let about = if named.is_empty() {
            vec![ontology(MEASUREMENT_CONCEPT)]
        } else {
            named
        };
        let id = LATENCY_CRITERION
            .get(index)
            .ok_or("section 24.1 lists more criteria than this fixture names")?;
        criteria.push(PerformanceCriterion::of(
            CriterionId::new(*id)?,
            text.clone(),
            about,
        )?);
    }

    // The document lists three rubric rows and does not say which criterion or
    // which stage each settles, so the fixture pairs them in order across three
    // different stages.
    let stages = [
        EvidenceStage::UnderstoodStructure,
        EvidenceStage::MadeDesignChoice,
        EvidenceStage::DebuggedIncident,
    ];
    let mut rows = Vec::new();
    for (index, text) in example.rubric.iter().enumerate() {
        let criterion = LATENCY_CRITERION
            .get(index)
            .ok_or("section 24.1 lists more rubric rows than this fixture names")?;
        let stage = stages
            .get(index)
            .ok_or("section 24.1 lists more rubric rows than this fixture stages")?;
        rows.push(RubricRow::of(
            CriterionId::new(*criterion)?,
            *stage,
            text.clone(),
        )?);
    }

    Ok(declare(
        CompetencyId::new(example.id)?,
        Situation::new(example.context)?,
        criteria,
        enabled_by,
        EvidenceRubric::of(rows),
    )?)
}

/// One competency whose rubric admits every one of section 24.3's stages.
///
/// Its name is section 24.2's own `PRODUCTION_DEBUGGING`. One criterion, about
/// `TCP`, with a rubric row at each stage, so a sheet over it has one cell per
/// stage and nothing else.
fn production_debugging() -> Result<Competency, Box<dyn Error>> {
    let criterion = CriterionId::new("pd-1")?;
    let rows = EvidenceStage::ALL
        .iter()
        .map(|stage| {
            RubricRow::of(
                criterion.clone(),
                *stage,
                format!("what a reader opens for {}", stage.as_str()),
            )
        })
        .collect::<Result<Vec<RubricRow>, CompetencyError>>()?;
    Ok(declare(
        CompetencyId::new("PRODUCTION_DEBUGGING")?,
        Situation::new("a production service under load")?,
        vec![PerformanceCriterion::of(
            criterion,
            "restores service and states what failed and why",
            vec![ontology("TCP")],
        )?],
        vec![
            EnablingConcept::of(
                ontology("TCP"),
                ContributionImportance::Critical,
                Necessity::Necessary,
            ),
            EnablingConcept::of(
                ontology(MEASUREMENT_CONCEPT),
                ContributionImportance::Substantial,
                Necessity::Optional,
            ),
        ],
        EvidenceRubric::of(rows),
    )?)
}

/// A third competency `TCP` also enables. Section 24.2's own name.
fn distributed_failure_reasoning() -> Result<Competency, Box<dyn Error>> {
    let criterion = CriterionId::new("dfr-1")?;
    Ok(declare(
        CompetencyId::new("DISTRIBUTED_FAILURE_REASONING")?,
        Situation::new("a service whose dependency is partially unavailable")?,
        vec![PerformanceCriterion::of(
            criterion.clone(),
            "separates a partition from a slow dependency",
            vec![ontology("TCP")],
        )?],
        vec![EnablingConcept::of(
            ontology("TCP"),
            ContributionImportance::Critical,
            Necessity::Necessary,
        )],
        EvidenceRubric::of(vec![RubricRow::of(
            criterion,
            EvidenceStage::TransferredToNovel,
            "a second incident in another service",
        )?]),
    )?)
}

/// A competency in `P2-R4`'s classification namespace, enabled by two concepts,
/// with one criterion about each.
///
/// This is the shape section 24.3's first sentence is about: `redis` and
/// `express` both enable it, and a claim about one of them settles only the
/// criterion that names it.
fn cache_staleness() -> Result<Competency, Box<dyn Error>> {
    let used = CriterionId::new("cs-redis")?;
    let declared = CriterionId::new("cs-express")?;
    Ok(declare(
        CompetencyId::new("cache_staleness_diagnosis")?,
        Situation::new("a read-through cache in front of an order store")?,
        vec![
            PerformanceCriterion::of(
                used.clone(),
                "invalidates on write rather than on a timer",
                vec![ConceptRef::classification("redis")?],
            )?,
            PerformanceCriterion::of(
                declared.clone(),
                "separates request routing from cache lookup",
                vec![ConceptRef::classification("express")?],
            )?,
        ],
        vec![
            EnablingConcept::of(
                ConceptRef::classification("redis")?,
                ContributionImportance::Critical,
                Necessity::Necessary,
            ),
            EnablingConcept::of(
                ConceptRef::classification("express")?,
                ContributionImportance::Minor,
                Necessity::Optional,
            ),
        ],
        EvidenceRubric::of(vec![
            RubricRow::of(
                used,
                EvidenceStage::Used,
                "the invalidation path in the code",
            )?,
            RubricRow::of(
                declared,
                EvidenceStage::Used,
                "the routing layer in the code",
            )?,
        ]),
    )?)
}

/// The user's `User APPLIED Concept` claim for `redis`, promoted by `P2-R5`.
fn redis_personal_claim(corpus: &Corpus) -> Result<PersonalApplicationClaim, Box<dyn Error>> {
    let work = own_work(corpus)?;
    let set = promoted(corpus, std::slice::from_ref(&work))?;
    set.personal_claim("redis")
        .cloned()
        .ok_or_else(|| "P2-R5 promoted no personal claim for redis".into())
}

// ---------------------------------------------------------------------------
// 1. `competency_observability`
// ---------------------------------------------------------------------------

/// Section 7.1: a competency is an observable performance, never `knows X`.
///
/// Five separate refusals and one acceptance. The last refusal is the one that
/// carries the section's own example wording, and it is not a token check: the
/// statement is re-rendered from the parts and compared whole, so **every**
/// sentence that is not the one the parts render is refused, in any language
/// and any spelling.
#[test]
fn competency_observability() -> TestResult {
    let example = section_24_1_example()?;
    let competency = latency_competency()?;

    // Section 24.1's own example statement is a can-do sentence — it ends in
    // `수 있다` rather than `안다` — which is the shape section 7.1 asks for and
    // the shape it refuses, in the document's own words. If the example ever
    // became a knowledge claim, this fails here rather than being copied in.
    assert!(
        example.statement.ends_with("수 있다"),
        "section 24.1's example statement is not a can-do sentence: {:?}",
        example.statement
    );
    assert!(
        !example.statement.ends_with("안다"),
        "section 24.1's example statement is a knowledge claim: {:?}",
        example.statement
    );

    // The accepted shape: a situation, a performance in it, and what a reader
    // would open. Every part of the rendered statement is one of those three.
    let statement = competency.statement();
    assert_eq!(statement.situation(), example.context);
    assert_eq!(
        statement.performances(),
        example.criteria.as_slice(),
        "the rendered statement does not carry section 24.1's own criteria"
    );
    assert_eq!(
        statement.witnesses().len(),
        example.rubric.len(),
        "the rendered statement witnesses a different number of things than section 24.1 lists"
    );
    let rendered = statement.to_string();
    for part in example
        .criteria
        .iter()
        .chain(example.rubric.iter())
        .chain(std::iter::once(&example.context))
    {
        assert!(
            rendered.contains(part.as_str()),
            "the rendered statement drops {part:?}"
        );
    }
    for stage in EvidenceStage::ALL {
        let declared = competency
            .rubric()
            .rows()
            .iter()
            .any(|row| row.stage() == stage);
        assert_eq!(
            declared,
            rendered.contains(stage.as_str()),
            "the rendered statement disagrees with the rubric about {}",
            stage.as_str()
        );
    }

    // A statement with no situation. There is no `Situation` to pass.
    assert!(matches!(
        Situation::new("   "),
        Err(CompetencyError::EmptyText("context"))
    ));

    // A statement with no performance criterion.
    let bare = declare(
        CompetencyId::new("knows_b_plus_tree")?,
        Situation::new("a database course")?,
        Vec::new(),
        vec![EnablingConcept::of(
            ontology("B_PLUS_TREE"),
            ContributionImportance::Critical,
            Necessity::Necessary,
        )],
        EvidenceRubric::of(Vec::new()),
    );
    assert!(matches!(
        bare,
        Err(CompetencyError::NoPerformanceCriterion(_))
    ));

    // A criterion about nothing, which is what a knowledge claim is: no concept
    // it could be joined to, so no evidence could ever settle it.
    assert!(matches!(
        PerformanceCriterion::of(CriterionId::new("k-1")?, "knows B+ Tree", Vec::new()),
        Err(CompetencyError::CriterionNamesNoConcept(_))
    ));

    // A criterion nothing in the rubric witnesses.
    let unwitnessed = declare(
        CompetencyId::new("knows_b_plus_tree")?,
        Situation::new("a database course")?,
        vec![PerformanceCriterion::of(
            CriterionId::new("k-1")?,
            "knows B+ Tree",
            vec![ontology("B_PLUS_TREE")],
        )?],
        vec![EnablingConcept::of(
            ontology("B_PLUS_TREE"),
            ContributionImportance::Critical,
            Necessity::Necessary,
        )],
        EvidenceRubric::of(Vec::new()),
    );
    assert!(matches!(
        unwitnessed,
        Err(CompetencyError::CriterionHasNoRubricRow { .. })
    ));

    // And a hand-written statement cannot arrive through the schema either.
    // The document below differs from a valid one in exactly one field.
    let mut document: serde_json::Value = serde_json::to_value(&competency)?;
    let valid: Competency = serde_json::from_value(document.clone())?;
    assert_eq!(valid, competency, "the unedited document is accepted");
    document["statement"] = serde_json::Value::String("B+ Tree를 안다".to_owned());
    let refused = serde_json::from_value::<Competency>(document.clone());
    assert!(
        refused.is_err(),
        "a hand-written statement rode in through the schema"
    );
    // Not because of what the sentence says. Section 24.1's own example
    // statement is refused in the same field for the same reason: the statement
    // is what the parts render, so no sentence written beside them is accepted,
    // whatever it says.
    document["statement"] = serde_json::Value::String(example.statement.clone());
    assert!(
        serde_json::from_value::<Competency>(document.clone()).is_err(),
        "a statement written beside the parts was accepted"
    );
    // The refusal is over the whole rendered sentence, so a statement that
    // names no forbidden word is refused just the same.
    document["statement"] = serde_json::Value::String(format!("{rendered} "));
    assert!(
        serde_json::from_value::<Competency>(document).is_err(),
        "a statement differing from the rendered one by a single byte was accepted"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `competency_schema_round_trip`
// ---------------------------------------------------------------------------

/// Section 24.1's six keys survive a round trip, and dropping any of the three
/// the section names as required fails the read.
#[test]
fn competency_schema_round_trip() -> TestResult {
    let example = section_24_1_example()?;
    let competency = latency_competency()?;
    let document = serde_json::to_value(&competency)?;

    // Every key section 24.1's own example writes, with the document's own
    // values under them.
    let object = document
        .as_object()
        .ok_or("a competency does not serialize to an object")?;
    let keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "id",
            "statement",
            "context",
            "performanceCriteria",
            "enabledByConcepts",
            "evidenceRubric",
        ]),
        "the serialized shape is not section 24.1's"
    );
    assert_eq!(document["id"], serde_json::json!(example.id));
    assert_eq!(document["context"], serde_json::json!(example.context));
    assert_eq!(
        document["performanceCriteria"]
            .as_array()
            .map(Vec::len)
            .ok_or("performanceCriteria is not a list")?,
        example.criteria.len()
    );
    assert_eq!(
        document["enabledByConcepts"]
            .as_array()
            .map(Vec::len)
            .ok_or("enabledByConcepts is not a list")?,
        example.concepts.len()
    );
    assert_eq!(
        document["evidenceRubric"]
            .as_array()
            .map(Vec::len)
            .ok_or("evidenceRubric is not a list")?,
        example.rubric.len()
    );

    let text = serde_json::to_string(&competency)?;
    let read: Competency = serde_json::from_str(&text)?;
    assert_eq!(read, competency, "the round trip lost something");
    assert_eq!(
        serde_json::to_string(&read)?,
        text,
        "the second serialization is not byte-identical to the first"
    );

    // Each of the three parts section 24.1 names as required, removed one at a
    // time. Each removal is its own document, so one refusal cannot stand in
    // for another.
    for key in ["context", "performanceCriteria", "evidenceRubric"] {
        let mut broken = document.clone();
        broken
            .as_object_mut()
            .ok_or("a competency does not serialize to an object")?
            .remove(key);
        assert!(
            serde_json::from_value::<Competency>(broken).is_err(),
            "a competency with no {key} was accepted"
        );
    }

    // Empty rather than absent, which is a different document and a different
    // refusal.
    let mut emptied = document.clone();
    emptied["performanceCriteria"] = serde_json::json!([]);
    assert!(
        serde_json::from_value::<Competency>(emptied).is_err(),
        "a competency with no criterion was accepted"
    );
    let mut emptied = document.clone();
    emptied["evidenceRubric"] = serde_json::json!([]);
    assert!(
        serde_json::from_value::<Competency>(emptied).is_err(),
        "a competency whose rubric witnesses nothing was accepted"
    );
    let mut emptied = document;
    emptied["context"] = serde_json::json!("");
    assert!(
        serde_json::from_value::<Competency>(emptied).is_err(),
        "a competency with an empty context was accepted"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `concept_and_competency_are_distinct_types`
// ---------------------------------------------------------------------------

/// Section 24.1's first sentence, held by the type system and by the registry.
#[test]
fn concept_and_competency_are_distinct_types() -> TestResult {
    // The shared vocabulary already separates them.
    assert_ne!(Competency::node_type(), NodeType::Concept);
    assert_eq!(Competency::node_type(), NodeType::Competency);

    let descriptor = Competency::enabling_predicate().descriptor();
    assert!(
        !descriptor.subject_types.contains(&NodeType::Competency),
        "a competency is not the subject of ENABLES_COMPETENCY"
    );
    assert!(
        descriptor.subject_types.contains(&NodeType::Concept),
        "a concept is the subject of ENABLES_COMPETENCY"
    );
    assert_eq!(
        descriptor.object_types,
        &[NodeType::Competency],
        "ENABLES_COMPETENCY points at a competency and at nothing else"
    );

    // One spelling, two namespaces, and neither is the other. The identifier is
    // the same text in all three values below.
    let shared = "cache_staleness_diagnosis";
    let competency = CompetencyId::new(shared)?;
    let concept = ConceptRef::classification(shared)?;
    assert_eq!(competency.as_str(), shared);
    assert_eq!(concept.namespace(), ConceptNamespace::Classification);

    // The two namespaces do not resolve into one another either. The strongest
    // case is a classification token spelled out as the ontology identifier
    // itself: it is a legal token, its text is byte-identical, and the two
    // references are still not equal, because the namespace is part of the
    // value rather than context a caller is trusted to carry.
    let id = entity("TCP");
    let spelled = id.to_string();
    let as_token = ConceptRef::classification(spelled.as_str())?;
    assert_eq!(
        serde_json::to_value(&as_token)?["id"],
        serde_json::to_value(ConceptRef::ontology(id))?["id"],
        "the two references carry the same text"
    );
    assert_ne!(
        as_token,
        ConceptRef::ontology(id),
        "one spelling in two namespaces is two concepts"
    );
    assert_ne!(ConceptRef::ontology(id), ConceptRef::classification("TCP")?);

    // They serialize differently: a concept carries the namespace that named
    // it, a competency identity is a bare string.
    assert_eq!(
        serde_json::to_value(&competency)?,
        serde_json::json!(shared),
        "a competency identity is its own string"
    );
    assert_eq!(
        serde_json::to_value(&concept)?,
        serde_json::json!({"namespace": "CLASSIFICATION", "id": shared}),
        "a concept reference carries its namespace"
    );

    // And in a declared competency the two ends stay apart: the identity is a
    // competency's, every enabling entry is a concept's, and no accessor hands
    // one out where the other belongs.
    let latency = latency_competency()?;
    let ids: BTreeSet<String> = latency
        .enabled_by()
        .iter()
        .map(|entry| serde_json::to_string(entry.concept()))
        .collect::<Result<BTreeSet<String>, serde_json::Error>>()?;
    assert!(
        !ids.contains(&serde_json::to_string(latency.id())?),
        "a competency identity appeared among its own enabling concepts"
    );

    Ok(())
}

/// Section 7.2's closed qualifier schema for `ENABLES_COMPETENCY`, measured.
///
/// Both enumerations are compared against the predicate registry in both
/// directions, so a value added or dropped there is a failure here rather than
/// a silent divergence.
#[test]
fn enabling_qualifiers_are_the_registry_s() -> TestResult {
    let descriptor = Competency::enabling_predicate().descriptor();
    assert_eq!(descriptor.direction, EdgeDirection::Directed);
    assert_eq!(descriptor.cardinality, Cardinality::ManyToMany);
    assert_eq!(descriptor.spec_direction, "concept → competency");

    let mut seen = BTreeSet::new();
    for qualifier in descriptor.qualifiers {
        let QualifierKind::Enumeration(values) = qualifier.kind else {
            continue;
        };
        let registry: BTreeSet<&str> = values.iter().copied().collect();
        let ours: BTreeSet<&str> = match qualifier.key {
            "contribution_importance" => ContributionImportance::ALL
                .iter()
                .map(|value| value.as_str())
                .collect(),
            "necessity" => Necessity::ALL.iter().map(|value| value.as_str()).collect(),
            other => {
                return Err(format!(
                    "the registry grew an enumerated qualifier this crate does not carry: {other}"
                )
                .into());
            }
        };
        assert_eq!(
            ours, registry,
            "{} does not match the registry",
            qualifier.key
        );
        assert!(qualifier.required, "{} is required", qualifier.key);
        seen.insert(qualifier.key);
    }
    assert_eq!(
        seen,
        BTreeSet::from(["contribution_importance", "necessity"]),
        "the registry's enumerated qualifiers are not the two this crate carries"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `many_to_many_enabling_edges_query_both_ways`
// ---------------------------------------------------------------------------

/// Section 24.1's many-to-many relation, queried in both directions over one
/// stored list.
///
/// The two counts are the document's: section 24.1's own `enabledByConcepts`
/// gives the first, and `TCP` is one of them, which is what makes the second
/// possible at all.
#[test]
fn many_to_many_enabling_edges_query_both_ways() -> TestResult {
    let example = section_24_1_example()?;
    let latency = latency_competency()?;
    let debugging = production_debugging()?;
    let reasoning = distributed_failure_reasoning()?;
    let competencies = vec![latency, debugging, reasoning];
    let graph = EnablingGraph::of(&competencies);

    // Nothing is stored twice, and nothing is stored in reverse: the whole of
    // the graph is one forward row per `enabledByConcepts` entry.
    let declared: usize = competencies
        .iter()
        .map(|competency| competency.enabled_by().len())
        .sum();
    assert_eq!(
        graph.edges().len(),
        declared,
        "the graph holds a different number of rows than the competencies declare"
    );
    let pairs: BTreeSet<(String, String)> = graph
        .edges()
        .iter()
        .map(|edge| {
            Ok::<(String, String), serde_json::Error>((
                serde_json::to_string(edge.concept())?,
                edge.competency().as_str().to_owned(),
            ))
        })
        .collect::<Result<BTreeSet<(String, String)>, serde_json::Error>>()?;
    assert_eq!(
        pairs.len(),
        graph.edges().len(),
        "one concept-competency pair is stored twice"
    );

    // Many concepts enable one competency: section 24.1's own six.
    let latency_id = CompetencyId::new(example.id.as_str())?;
    let enabling = graph.concepts_enabling(&latency_id);
    assert_eq!(
        enabling.len(),
        example.concepts.len(),
        "section 24.1 lists {} enabling concepts and the graph answers with {}",
        example.concepts.len(),
        enabling.len()
    );
    let named: BTreeSet<ConceptRef> = enabling.iter().map(|edge| edge.concept().clone()).collect();
    let expected: BTreeSet<ConceptRef> =
        example.concepts.iter().map(|name| ontology(name)).collect();
    assert_eq!(named, expected, "the inverse view names other concepts");

    // One concept enables many competencies.
    let tcp = ontology("TCP");
    let enabled = graph.competencies_enabled_by(&tcp);
    let reached: BTreeSet<String> = enabled
        .iter()
        .map(|edge| edge.competency().as_str().to_owned())
        .collect();
    assert_eq!(
        reached,
        BTreeSet::from([
            example.id.clone(),
            "PRODUCTION_DEBUGGING".to_owned(),
            "DISTRIBUTED_FAILURE_REASONING".to_owned(),
        ]),
        "TCP does not reach the three competencies it enables"
    );

    // The two views are the same rows read two ways: every stored edge appears
    // exactly once in each direction, and neither view invents one.
    for edge in graph.edges() {
        let forward = graph.competencies_enabled_by(edge.concept());
        let inverse = graph.concepts_enabling(edge.competency());
        assert_eq!(
            forward.iter().filter(|found| **found == edge).count(),
            1,
            "the forward view does not hold this edge exactly once"
        );
        assert_eq!(
            inverse.iter().filter(|found| **found == edge).count(),
            1,
            "the inverse view does not hold this edge exactly once"
        );
        // And the row carries the qualifiers the competency wrote, so the view
        // is the declaration read back rather than a pair of ends.
        let declared = competencies
            .iter()
            .find(|competency| competency.id() == edge.competency())
            .and_then(|competency| {
                competency
                    .enabled_by()
                    .iter()
                    .find(|entry| entry.concept() == edge.concept())
            })
            .ok_or("the graph holds an edge no competency declared")?;
        assert_eq!(edge.importance(), declared.importance());
        assert_eq!(edge.necessity(), declared.necessity());
    }

    // A concept nothing declares reaches nothing, and so does a competency that
    // is not in the set, so neither query answers with a default.
    assert!(
        graph.competencies_enabled_by(&ontology("QUIC")).is_empty(),
        "a concept no competency names reached something"
    );
    assert!(
        graph
            .concepts_enabling(&CompetencyId::new("API_ARCHITECTURE")?)
            .is_empty(),
        "a competency not in the set was enabled by something"
    );

    // The registry's own inverse label, so the reverse reading has a name and
    // still no row.
    assert_eq!(
        Competency::enabling_predicate().descriptor().inverse_label,
        "is enabled by"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `six_evidence_stages_are_distinct`
// ---------------------------------------------------------------------------

/// One artifact per stage, and each settles its own cell and no other.
///
/// The six names are read out of section 24.3's own sentence and compared
/// against the enumeration in both directions, so the number six is a
/// measurement of the design document rather than a constant in this file.
#[test]
fn six_evidence_stages_are_distinct() -> TestResult {
    let named = section_24_3_stage_names()?;
    assert_eq!(
        named.len(),
        EvidenceStage::ALL.len(),
        "section 24.3 separates {} stages and this crate enumerates {}: {named:?}",
        named.len(),
        EvidenceStage::ALL.len()
    );
    let ours: Vec<&str> = EvidenceStage::ALL
        .iter()
        .map(|stage| stage.spec_name())
        .collect();
    assert_eq!(
        named, ours,
        "the enumeration is not section 24.3's list, in its order"
    );
    let spellings: BTreeSet<&str> = EvidenceStage::ALL
        .iter()
        .map(|stage| stage.as_str())
        .collect();
    assert_eq!(
        spellings.len(),
        EvidenceStage::ALL.len(),
        "two stages share a spelling"
    );

    // Six artifacts, each admitted by `P2-N2`'s own four checks, each about the
    // one concept this competency's criterion names.
    let competency = production_debugging()?;
    let concept = entity("TCP");
    let corpus = built(&MIXED_CORPUS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let observed = classification
        .stances()
        .iter()
        .find(|stance| stance.observed().is_some())
        .ok_or("the corpus observed nothing")?;

    let mut records = Vec::new();
    for stage in EvidenceStage::ALL {
        let tag = stage.as_str();
        let evidence = match stage {
            EvidenceStage::Used | EvidenceStage::MadeDesignChoice => {
                ConceptEvidence::AuthoredProjectCode(
                    ProjectUse::of_stance(observed)
                        .ok_or("the observed stance carries no project use")?,
                )
            }
            EvidenceStage::UnderstoodStructure => ConceptEvidence::SelfExplanation(
                SelfExplanation::confirmed_by(evidence_id(tag), &confirmation(concept)?),
            ),
            EvidenceStage::SolvedProblem => {
                ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id(tag)))
            }
            EvidenceStage::DebuggedIncident => {
                ConceptEvidence::IncidentDebugging(IncidentRepair::of(
                    evidence_id("incident"),
                    evidence_id("cause"),
                    evidence_id("fix"),
                    evidence_id("verified"),
                ))
            }
            EvidenceStage::TransferredToNovel => ConceptEvidence::RepeatedTransfer(
                TransferRepetition::across(vec![
                    TransferContext::of("service-a", evidence_id("ctx-a"), true),
                    TransferContext::of("service-b", evidence_id("ctx-b"), true),
                ])
                .ok_or("two independent contexts are repetition")?,
            ),
        };
        let promoting = PromotingEvidence::of(admitted(evidence, tag, concept)?)?;
        records.push(StageEvidence::of_knowledge_state(
            RecordId::new(tag)?,
            stage,
            &promoting,
        ));
    }

    let sheet = fill(&competency, &records);
    assert_eq!(
        sheet.cells().len(),
        EvidenceStage::ALL.len(),
        "one criterion at six stages is six cells"
    );
    assert!(
        sheet.unmatched().is_empty(),
        "every record settles a cell: {:?}",
        sheet.unmatched()
    );
    for (index, stage) in EvidenceStage::ALL.iter().enumerate() {
        let cell = sheet
            .cell(competency.criteria()[0].id(), *stage)
            .ok_or("a stage has no cell")?;
        let settling = cell.state().records();
        assert_eq!(settling.len(), 1, "{} is settled once", stage.as_str());
        assert_eq!(
            settling[0].id(),
            records[index].id(),
            "{} is settled by another stage's artifact",
            stage.as_str()
        );
        assert_eq!(
            cell.admits(),
            Some(format!("what a reader opens for {}", stage.as_str()).as_str()),
            "the cell does not carry its own rubric row"
        );
    }

    // Removing one artifact empties exactly its own cell, so the six are not
    // interchangeable and no cell is settled by something else.
    for (index, stage) in EvidenceStage::ALL.iter().enumerate() {
        let mut fewer = records.clone();
        fewer.remove(index);
        let reduced = fill(&competency, &fewer);
        for other in EvidenceStage::ALL {
            let cell = reduced
                .cell(competency.criteria()[0].id(), other)
                .ok_or("a stage has no cell")?;
            assert_eq!(
                cell.state().is_filled(),
                other != *stage,
                "removing {} changed {}",
                stage.as_str(),
                other.as_str()
            );
        }
    }

    // Serialization keeps the source and the rubric row, which is what a reader
    // navigates to.
    let document = serde_json::to_value(&sheet)?;
    let text = serde_json::to_string(&document)?;
    for stage in EvidenceStage::ALL {
        assert!(
            text.contains(stage.as_str()),
            "the serialized sheet drops {}",
            stage.as_str()
        );
        assert!(
            text.contains(&format!("what a reader opens for {}", stage.as_str())),
            "the serialized sheet drops {}'s rubric row",
            stage.as_str()
        );
    }
    for record in &records {
        let EvidenceSource::KnowledgeState(id) = record.source() else {
            return Err("a knowledge-state record does not carry a knowledge-state source".into());
        };
        assert!(
            text.contains(&id.to_string()),
            "the serialized sheet drops a record's source"
        );
        assert_eq!(record.source().origin(), EvidenceOrigin::KnowledgeState);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `dependency_use_does_not_fill_a_cell`
// ---------------------------------------------------------------------------

/// Section 24.3's first sentence, in the three places it could have failed.
///
/// The corpus is one snapshot that carries both halves: `redis` is imported and
/// called, so `P2-R2` reaches its `OBSERVED` rung; `express` sits in the
/// manifest and nowhere else, which is section 13.2's
/// `dependency/install/import만 존재`.
#[test]
fn dependency_use_does_not_fill_a_cell() -> TestResult {
    // Section 13.2's own row, read back out of the design document rather than
    // restated, so this test's premise is the specification's.
    let ceilings = section_13_2_ceilings()?;
    let installed_only = ceilings
        .iter()
        .find(|(evidence, _)| evidence.contains("dependency/install/import만 존재"))
        .ok_or("section 13.2 has no row for a dependency that is only present")?;
    assert_eq!(
        installed_only.1, "mastery 승격 없음",
        "section 13.2's ceiling for a present-only dependency has changed"
    );

    let corpus = built(&MIXED_CORPUS)?;
    let classification = classified(&corpus, &order_goal()?)?;

    // The analyzer really did see `express`: it is a finding, at a rung below
    // the observed one. Without this the rest of the test would be measuring an
    // absent corpus.
    assert_eq!(corpus.finding("express")?.subject(), "express");
    assert_ne!(
        corpus.finding("express")?.tier(),
        corpus.finding("redis")?.tier(),
        "the two subjects reached the same rung, so the corpus proves nothing"
    );

    // Arm one. `P2-R5` promotes nothing at all about a dependency that is only
    // present, so there is no claim to found stage evidence on.
    let work = own_work(&corpus)?;
    let promotion = promoted(&corpus, std::slice::from_ref(&work))?;
    assert!(
        promotion.personal_claim("redis").is_some(),
        "the used concept has a personal claim, so the refusal below is about the other one"
    );
    assert!(
        promotion.project_claim("express").is_none(),
        "a present-only dependency produced a project claim"
    );
    assert!(
        promotion.personal_claim("express").is_none(),
        "a present-only dependency produced a personal claim"
    );

    // Arm two. `P2-N2` admits a dependency-presence item — it passes all four
    // eligibility checks — and section 13.2's ceiling still refuses it here,
    // while the same dossier with a project-use item is admitted. The only
    // difference between the two calls is which row the evidence is.
    let unobserved = classification
        .stances()
        .iter()
        .find(|stance| stance.observed().is_none())
        .ok_or("the classification carries no unobserved stance")?;
    let observed = classification
        .stances()
        .iter()
        .find(|stance| stance.observed().is_some())
        .ok_or("the classification carries no observed stance")?;
    let concept = entity("TCP");

    let declared_only = ConceptEvidence::DependencyPresence(
        DependencyOnly::of_stance(unobserved).ok_or("an unobserved stance is dependency-only")?,
    );
    assert_eq!(declared_only.kind(), EvidenceKind::DependencyPresenceOnly);
    assert_eq!(declared_only.ceiling(), EvidenceCeiling::NoPromotion);
    let admitted_declared = admitted(declared_only, "declared", concept)?;
    assert!(
        matches!(
            PromotingEvidence::of(admitted_declared),
            Err(CompetencyError::EvidenceLicensesNoPromotion(
                "DEPENDENCY_PRESENCE_ONLY"
            ))
        ),
        "a dependency-presence item founded stage evidence"
    );

    let used = ConceptEvidence::AuthoredProjectCode(
        ProjectUse::of_stance(observed).ok_or("an observed stance carries a project use")?,
    );
    assert!(
        PromotingEvidence::of(admitted(used, "used", concept)?).is_ok(),
        "the control did not pass, so the refusal above measured nothing"
    );

    // `ProjectUse` and `DependencyOnly` are two readings of the same stance
    // shape, and each answers for exactly one of them. Neither is a fallback for
    // the other.
    assert!(ProjectUse::of_stance(unobserved).is_none());
    assert!(DependencyOnly::of_stance(observed).is_none());

    // Arm three. The join. `cache_staleness_diagnosis` is enabled by `redis`
    // and by `express`, and its two criteria name one each. A personal claim
    // about `redis` settles the criterion that names `redis` and **not** the
    // one that names `express`, even though `redis` is in the competency's own
    // enabling set — which is the fallback `P2-R5` measured one layer down.
    let competency = cache_staleness()?;
    let claim = redis_personal_claim(&corpus)?;
    let record =
        StageEvidence::of_personal_claim(RecordId::new("redis-use")?, EvidenceStage::Used, &claim)?;
    assert_eq!(
        record.concept(),
        &ConceptRef::classification("redis")?,
        "the record's concept is the claim's own"
    );

    let sheet = fill(&competency, std::slice::from_ref(&record));
    let redis_cell = sheet
        .cell(&CriterionId::new("cs-redis")?, EvidenceStage::Used)
        .ok_or("the redis criterion has no USED cell")?;
    assert!(
        redis_cell.state().is_filled(),
        "the criterion that names redis was not settled, so the refusal below proves nothing"
    );
    let express_cell = sheet
        .cell(&CriterionId::new("cs-express")?, EvidenceStage::Used)
        .ok_or("the express criterion has no USED cell")?;
    assert_eq!(
        express_cell.state(),
        &CellState::Empty,
        "a claim about another of the competency's enabling concepts settled this criterion"
    );
    assert_eq!(
        sheet.filled().len(),
        1,
        "exactly one cell is settled by one record"
    );

    // And the same record against a competency whose criteria name neither of
    // its concepts settles nothing at all, and says so.
    let elsewhere = fill(&production_debugging()?, std::slice::from_ref(&record));
    assert!(
        elsewhere.filled().is_empty(),
        "a claim in another namespace settled a cell"
    );
    assert_eq!(
        elsewhere.unmatched().len(),
        1,
        "a record that settles nothing is reported rather than dropped"
    );

    Ok(())
}

/// Section 13.2's `자동 상한` table, as `evidence → ceiling` pairs.
fn section_13_2_ceilings() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let page = design_page()?;
    let block = section(&page, "### 13.2", "### 13.3")?;
    Ok(block
        .lines()
        .filter(|line| line.starts_with('|'))
        .map(|line| {
            line.split('|')
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .map(str::to_owned)
                .collect::<Vec<String>>()
        })
        .filter(|cells| cells.len() == 3 && !cells[0].starts_with("---"))
        .map(|cells| (cells[0].clone(), cells[2].clone()))
        .collect())
}

// ---------------------------------------------------------------------------
// The refusals no named test drove, and the controls that keep them honest.
// ---------------------------------------------------------------------------

/// A claim `P2-R5` has taken back founds nothing.
#[test]
fn a_rejected_claim_founds_no_stage_evidence() -> TestResult {
    let corpus = built(&MIXED_CORPUS)?;
    let claim = redis_personal_claim(&corpus)?;
    assert!(
        StageEvidence::of_personal_claim(RecordId::new("standing")?, EvidenceStage::Used, &claim)
            .is_ok(),
        "a standing claim founds evidence, so the refusal below is about the rejection"
    );
    let withdrawn = claim.rejected(RejectionReason::EvidenceWithdrawn, 9_000)?;
    assert!(matches!(
        StageEvidence::of_personal_claim(
            RecordId::new("withdrawn")?,
            EvidenceStage::Used,
            &withdrawn
        ),
        Err(CompetencyError::ClaimIsRejected(_))
    ));
    Ok(())
}

/// Every refusal `declare` makes, one competency at a time.
///
/// Each case differs from a declarable one in exactly one way, so no refusal
/// can be standing in for another.
#[test]
fn each_declaration_rule_refuses_on_its_own() -> TestResult {
    let concept = ontology("TCP");
    let other = ontology(MEASUREMENT_CONCEPT);
    let criterion = CriterionId::new("c-1")?;
    let enabling = || {
        vec![EnablingConcept::of(
            ontology("TCP"),
            ContributionImportance::Critical,
            Necessity::Necessary,
        )]
    };
    let ok_criterion = || {
        PerformanceCriterion::of(
            CriterionId::new("c-1").map_err(|error| error.to_string())?,
            "restores service",
            vec![concept.clone()],
        )
        .map_err(|error| error.to_string())
    };
    let ok_row = || {
        RubricRow::of(
            CriterionId::new("c-1").map_err(|error| error.to_string())?,
            EvidenceStage::DebuggedIncident,
            "the incident record",
        )
        .map_err(|error| error.to_string())
    };
    let situation = || Situation::new("a production service");

    // The control: everything present, and it declares.
    assert!(
        declare(
            CompetencyId::new("control")?,
            situation()?,
            vec![ok_criterion()?],
            enabling(),
            EvidenceRubric::of(vec![ok_row()?]),
        )
        .is_ok(),
        "the control declaration failed, so every refusal below would be vacuous"
    );

    assert!(matches!(
        declare(
            CompetencyId::new("no-enabling")?,
            situation()?,
            vec![ok_criterion()?],
            Vec::new(),
            EvidenceRubric::of(vec![ok_row()?]),
        ),
        Err(CompetencyError::NoEnablingConcept(_))
    ));

    assert!(matches!(
        declare(
            CompetencyId::new("twice")?,
            situation()?,
            vec![ok_criterion()?, ok_criterion()?],
            enabling(),
            EvidenceRubric::of(vec![ok_row()?]),
        ),
        Err(CompetencyError::DuplicateCriterion { .. })
    ));

    assert!(matches!(
        declare(
            CompetencyId::new("concept-twice")?,
            situation()?,
            vec![ok_criterion()?],
            [enabling(), enabling()].concat(),
            EvidenceRubric::of(vec![ok_row()?]),
        ),
        Err(CompetencyError::DuplicateEnablingConcept(_))
    ));

    assert!(matches!(
        declare(
            CompetencyId::new("stranger")?,
            situation()?,
            vec![PerformanceCriterion::of(
                criterion.clone(),
                "restores service",
                vec![other.clone()],
            )?],
            enabling(),
            EvidenceRubric::of(vec![ok_row()?]),
        ),
        Err(CompetencyError::CriterionNamesUnenablingConcept { .. })
    ));

    assert!(matches!(
        declare(
            CompetencyId::new("unknown-row")?,
            situation()?,
            vec![ok_criterion()?],
            enabling(),
            EvidenceRubric::of(vec![RubricRow::of(
                CriterionId::new("c-2")?,
                EvidenceStage::DebuggedIncident,
                "the incident record",
            )?]),
        ),
        Err(CompetencyError::RubricRowNamesUnknownCriterion { .. })
    ));

    assert!(matches!(
        RubricRow::of(criterion, EvidenceStage::Used, "  "),
        Err(CompetencyError::EmptyText("evidence rubric"))
    ));

    Ok(())
}

/// A cell exists only where the rubric declares a row.
///
/// The competency section 24.1 writes has three criteria and three rubric rows
/// at three stages, so most of its cells are outside the rubric — a different
/// reading from one the rubric admits and nothing has settled.
#[test]
fn a_stage_the_rubric_does_not_admit_is_not_an_empty_cell() -> TestResult {
    let competency = latency_competency()?;
    let sheet = fill(&competency, &[]);
    assert_eq!(
        sheet.cells().len(),
        competency.criteria().len() * EvidenceStage::ALL.len(),
        "a sheet holds one cell per criterion per stage"
    );
    let admitted = sheet
        .cells()
        .iter()
        .filter(|cell| cell.state() == &CellState::Empty)
        .count();
    assert_eq!(
        admitted,
        competency.rubric().rows().len(),
        "every rubric row is an empty cell and nothing else is"
    );
    let outside = sheet
        .cells()
        .iter()
        .filter(|cell| cell.state() == &CellState::NotInRubric)
        .count();
    assert_eq!(admitted + outside, sheet.cells().len());
    for cell in sheet.cells() {
        assert_eq!(
            cell.admits().is_some(),
            cell.state() == &CellState::Empty,
            "a cell carries its rubric row exactly when the rubric declares one"
        );
    }
    Ok(())
}

/// Every identifier this crate takes is the shape `P2-R4` issues.
///
/// A whole-set classification rather than a list of rejected spellings: every
/// ASCII byte is offered inside an otherwise legal identifier and required to
/// be admitted **exactly** when this test's own independent predicate says it
/// belongs, in both directions, for all four constructors. A byte nobody
/// thought of is therefore covered by the same assertion as a byte somebody
/// did.
///
/// It exists because the campaign found it missing: reducing
/// `identity::validated` to a non-empty check passed the whole suite, so the
/// rule that a concept token arriving here is the shape `P2-R4` issues was
/// declared and unmeasured.
#[test]
fn every_identifier_is_the_shape_p2_r4_issues() -> TestResult {
    // Written here rather than read from the crate, so the two are independent.
    let belongs =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-';

    for byte in 0_u8..=127 {
        let candidate = format!("a{}b", char::from(byte));
        let admitted = [
            CompetencyId::new(candidate.clone()).is_ok(),
            CriterionId::new(candidate.clone()).is_ok(),
            RecordId::new(candidate.clone()).is_ok(),
            ConceptRef::classification(candidate.clone()).is_ok(),
        ];
        for taken in admitted {
            assert_eq!(
                taken,
                belongs(byte),
                "byte {byte} in {candidate:?} is admitted {taken} and belongs {}",
                belongs(byte)
            );
        }
    }

    // Beyond ASCII, where a byte-wise reader and a character-wise one disagree.
    for outside in ["개념", "a개념b", "a\u{00e9}b", "a\u{1f600}b"] {
        assert!(
            matches!(
                CompetencyId::new(outside),
                Err(CompetencyError::InvalidIdentifier("competency", _))
            ),
            "{outside:?} was admitted as a competency identity"
        );
        assert!(
            matches!(
                ConceptRef::classification(outside),
                Err(CompetencyError::InvalidIdentifier("concept", _))
            ),
            "{outside:?} was admitted as a concept token"
        );
    }

    // The length boundary, on both sides of it, and the empty value.
    let longest = "a".repeat(64);
    assert!(CompetencyId::new(longest.as_str()).is_ok());
    assert!(CriterionId::new(longest.as_str()).is_ok());
    assert!(RecordId::new(longest.as_str()).is_ok());
    assert!(ConceptRef::classification(longest.as_str()).is_ok());
    let overlong = "a".repeat(65);
    for outcome in [
        CompetencyId::new(overlong.as_str()).err(),
        CriterionId::new(overlong.as_str()).err(),
        RecordId::new(overlong.as_str()).err(),
        ConceptRef::classification(overlong.as_str()).err(),
    ] {
        assert!(
            matches!(outcome, Some(CompetencyError::InvalidIdentifier(_, _))),
            "a 65-byte identifier was admitted"
        );
    }
    assert!(matches!(
        CriterionId::new(""),
        Err(CompetencyError::InvalidIdentifier("criterion", _))
    ));
    assert!(matches!(
        RecordId::new(""),
        Err(CompetencyError::InvalidIdentifier("record", _))
    ));

    // Each constructor names itself in its refusal, so a reader is told which
    // identifier was wrong rather than that one of four was.
    let named: BTreeSet<&'static str> = [
        CompetencyId::new("").err(),
        CriterionId::new("").err(),
        RecordId::new("").err(),
        ConceptRef::classification("").err(),
    ]
    .into_iter()
    .filter_map(|error| match error {
        Some(CompetencyError::InvalidIdentifier(what, _)) => Some(what),
        _ => None,
    })
    .collect();
    assert_eq!(
        named,
        BTreeSet::from(["competency", "concept", "criterion", "record"]),
        "the four constructors do not name themselves apart"
    );
    Ok(())
}

/// The two closed vocabularies this crate declares beside the stages, and the
/// spellings they serialize under.
///
/// Small, and here because the campaign showed the cost of a value that is
/// declared and never read: a wrong spelling in either of these reaches a
/// serialized sheet and nothing else would notice.
#[test]
fn the_namespace_and_origin_spellings_are_pinned() -> TestResult {
    assert_eq!(
        ConceptNamespace::ALL
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
        vec!["ONTOLOGY", "CLASSIFICATION"]
    );
    assert_eq!(
        EvidenceOrigin::ALL
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
        vec!["KNOWLEDGE_STATE", "PERSONAL_APPLICATION"]
    );

    // And each of those spellings is the one a value serializes under, so the
    // constant and the wire form cannot drift apart.
    assert_eq!(
        serde_json::to_value(ConceptRef::classification("redis")?)?["namespace"],
        serde_json::json!(ConceptNamespace::Classification.as_str())
    );
    assert_eq!(
        serde_json::to_value(ConceptRef::ontology(entity("TCP")))?["namespace"],
        serde_json::json!(ConceptNamespace::Ontology.as_str())
    );

    let corpus = built(&MIXED_CORPUS)?;
    let claim = redis_personal_claim(&corpus)?;
    let record =
        StageEvidence::of_personal_claim(RecordId::new("origin")?, EvidenceStage::Used, &claim)?;
    assert_eq!(
        serde_json::to_value(&record)?["source"]["origin"],
        serde_json::json!(EvidenceOrigin::PersonalApplication.as_str())
    );
    assert_eq!(
        record.source().origin(),
        EvidenceOrigin::PersonalApplication
    );
    Ok(())
}
