//! `P2-R4`'s named acceptance evidence.
//!
//! Every corpus is synthetic and built in process, captured through `P2-R1`'s
//! own `capture_local`, analyzed through `P2-R2`'s own ladder and correlated
//! through `P2-R3`'s own `correlate`, so every classification here rests on
//! evidence that passed the permission and secret gate, the frozen manifest,
//! the sealed untrusted-content index, the evidence ladder and the two
//! authority lanes before this file saw it.
//!
//! The one file this suite reads is the design document itself, and only to
//! compare section 18's own classification names and section 18.2's own chain
//! steps against the enumerations: a list restated in a test is a list that can
//! be restated wrongly.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::PathBuf,
};

use academic_domain::{EpistemicStatus, FreshnessBand, MasteryLevel, entity_registry::EntityKind};
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
    AnalysisInput, AnalyzerIdentity, EvidenceLadder, EvidenceTier, Finding, FindingScope,
    RepositoryAnalysis, RuntimeTrace, SourceUnit, Subject, SubjectId, analyze,
};
use academic_repository_classification::{
    BenefitContract, BenefitDimension, BenefitDraft, BenefitPart, ChainDraft, ChainStep,
    ClassificationError, ClassificationInput, ClassificationLabel, ClassificationSet,
    ConceptStance, ConcreteNeed, ControllingMechanism, CurrentBasis, GoalId, GoalScope,
    MigrationOutcome, NeedKind, Outlook, OverrideDecision, ProjectConceptRequirement, ProofChain,
    RequiredConcept, RequirementId, ResolutionStatus, RetirementReason, TradeOff, Trigger,
    TriggerState, UnmatchedReason, UserEvidenceGap, UserOverride, classify, migrate_locators,
};
use academic_repository_correlation::{
    ApprovalStatus, BehaviorDocument, Correlation, CorrelationInput, DocumentId, IntentDocument,
    IntentDocumentKind, correlate,
};
use academic_untrusted_content::SourceIndex;

type TestResult = Result<(), Box<dyn Error>>;

const CAPTURED_AT: u64 = 1_756_000_000_000;
const HEAD: &str = "abc1234def5678";
const BRANCH: &str = "main";
const ANALYZER: &str = "academic-repository-analysis";
const VERSION: &str = "0.1.0";
const NOW: u64 = 5_000;

// ---------------------------------------------------------------------------
// The deterministic harness: `P2-R1`, `P2-R2`, `P2-R3`, then this crate.
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

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
        analysis_policy_hash: ContentDigest::of(b"analysis-policy-v1"),
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
        CalibrationDatasetId::new("cal-classification")?,
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

/// The subjects every corpus in this file is searched for.
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

/// Everything `P2-R3` needs beside the findings.
#[derive(Default)]
struct Artifacts {
    intent: Vec<IntentDocument>,
    behavior: Vec<BehaviorDocument>,
}

/// One capture, one analysis, one correlation, and the findings behind it.
struct Corpus {
    snapshot: RepositorySnapshot,
    analysis: RepositoryAnalysis,
    findings: Vec<Finding>,
    correlation: Correlation,
}

fn built(files: &[(&str, &str)], artifacts: &Artifacts) -> Result<Corpus, Box<dyn Error>> {
    let (snapshot, analysis) = analyzed(files)?;
    let findings = findings_of(&analysis)?;
    let identity = AnalyzerIdentity::new(ANALYZER, VERSION)?;
    let traces: Vec<RuntimeTrace> = Vec::new();
    let correlation = correlate(&CorrelationInput {
        snapshot: &snapshot,
        analyzer: &identity,
        findings: &findings,
        intent_documents: &artifacts.intent,
        behavior_documents: &artifacts.behavior,
        incidents: &[],
        feature_flags: &[],
        deployments: &[],
        declared_dependencies: &[],
    })?;
    let _ = &traces;
    Ok(Corpus {
        snapshot,
        analysis,
        findings,
        correlation,
    })
}

impl Corpus {
    /// The one finding for `subject`, which every corpus here produces exactly
    /// one of. A corpus that started producing two would fail here rather than
    /// have one of them silently chosen.
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
}

fn goal(name: &str, version: u64) -> Result<GoalScope, Box<dyn Error>> {
    Ok(GoalScope::at(GoalId::new(name)?, version))
}

/// The goal every corpus is classified under unless a test says otherwise.
fn order_goal() -> Result<GoalScope, Box<dyn Error>> {
    goal("no-duplicate-retryable-orders", 1)
}

/// Section 18.2's example chain, over one corpus.
///
/// `read-modify-write 경로 + 동시 실행 가능성 → lost update risk →
/// atomicity/isolation mechanism → Concurrency/Isolation REQUIRED`, closed by a
/// user who has not applied isolation.
fn isolation_chain(corpus: &Corpus) -> Result<ProofChain, Box<dyn Error>> {
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
    Ok(ProofChain::closed_by(
        concept,
        UserEvidenceGap::of(
            MasteryLevel::Understood,
            FreshnessBand::High,
            EpistemicStatus::UserConfirmed,
        )
        .ok_or("an unapplied concept has an evidence gap")?,
    ))
}

/// Section 18.3's example contract.
fn replication_contract() -> Result<BenefitContract, Box<dyn Error>> {
    Ok(BenefitContract::new(
        &subject_id("replication")?,
        vec![
            Trigger::new("availability-target-above-recovery-objective")?,
            Trigger::new("read-load-above-measured-primary-capacity")?,
        ],
        TriggerState::NotMet,
        BenefitDimension::Scale,
        vec![
            TradeOff::new("consistency")?,
            TradeOff::new("failover-complexity")?,
            TradeOff::new("cost")?,
        ],
    )?)
}

fn classified(
    corpus: &Corpus,
    goal: &GoalScope,
    required: &[ProofChain],
    beneficial: &[BenefitContract],
    overrides: &[UserOverride],
) -> Result<ClassificationSet, ClassificationError> {
    classify(&ClassificationInput {
        correlation: &corpus.correlation,
        goal,
        required,
        beneficial,
        overrides,
    })
}

fn intent_document(
    id: &str,
    status: ApprovalStatus,
    revision: u64,
    path: &str,
    mentions: &[&str],
) -> Result<IntentDocument, Box<dyn Error>> {
    let mut named = Vec::new();
    for subject in mentions {
        named.push(subject_id(subject)?);
    }
    Ok(IntentDocument::new(
        DocumentId::new(id)?,
        IntentDocumentKind::Specification,
        status,
        revision,
        Some(BRANCH.to_owned()),
        path,
        named,
    ))
}

// ---------------------------------------------------------------------------
// The corpora.
// ---------------------------------------------------------------------------

const SPEC_PAGE: (&str, &str) = ("docs/spec.md", "# Order platform\n\nA specification.\n");
const BEHAVIOR_PAGE: (&str, &str) = ("docs/behaviour.md", "# Behaviour\n\nA description.\n");

/// `redis` reachable from an entry point with a production configuration:
/// `P2-R2`'s third row, so `OBSERVED`.
///
/// The configuration is an infrastructure document on purpose. `P2-R2`'s
/// extractor pushes a scalar in one to **both** the configuration index and the
/// IaC index at one span, and the ladder reads
/// `config_tokens().chain(iac_tokens())`, so the finding carries two locators
/// that are equal in every field — which is the collapsing shape
/// `finding_locator_migration_preserves_original_evidence` is written against.
const OBSERVED_REDIS: [(&str, &str); 4] = [
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

/// The same tree with a comment block inserted above `warm`.
///
/// Every byte of `warm` is unchanged and its line span has moved, which is the
/// exact input `REQ-17-022` describes: *insert lines before symbol in snapshot
/// B*.
const OBSERVED_REDIS_SHIFTED: [(&str, &str); 4] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\n// A comment block that did not exist in snapshot A.\n// It moves every line below it and changes no symbol.\n//\n//\nfunction warm() {\n  redis.createClient();\n  return redis.connect();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
    (
        "docker-compose.yml",
        "services:\n  cache:\n    image: redis\n",
    ),
    SPEC_PAGE,
];

/// The tree with `warm` removed, so its fingerprint is not in snapshot C.
const OBSERVED_REDIS_WITHOUT_WARM: [(&str, &str); 4] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";\n\nexport function handle() {\n  redis.createClient();\n  return redis.connect();\n}\n",
    ),
    (
        "docker-compose.yml",
        "services:\n  cache:\n    image: redis\n",
    ),
    SPEC_PAGE,
];

/// The *broad category* corpus: a manifest says this is a backend and nothing
/// in the tree uses the framework.
///
/// `P2-R2` calls that `PRESENT_ONLY`. Section 18.2's `단지 backend라는 이유로`
/// is exactly this evidence, and section 36.5 has the user correct such an
/// entry as `template 잔재`.
const BACKEND_LABEL_ONLY: [(&str, &str); 3] = [
    (
        "package.json",
        "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"express\": \"4.19.0\"\n  }\n}\n",
    ),
    (
        "src/orders.ts",
        "export function place() {\n  return 1;\n}\n",
    ),
    SPEC_PAGE,
];

// ---------------------------------------------------------------------------
// 1. `required_failure_chain`
// ---------------------------------------------------------------------------

/// Section 18.2's own five steps, read out of the design document.
///
/// Not a list written here. The arrow diagram is parsed from
/// `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and its length is
/// compared against [`ChainStep::ALL`], so a step added to the design without
/// one here — or the reverse — is a failure rather than a count nobody
/// rechecks.
fn chain_steps_in_design() -> Result<Vec<String>, Box<dyn Error>> {
    let page = fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let start = page
        .find("current code/goal")
        .ok_or("section 18.2's chain diagram is not in the design document")?;
    let block = &page[start..];
    let end = block.find("```").ok_or("the chain diagram does not end")?;
    Ok(block[..end]
        .lines()
        .map(|line| line.trim().trim_start_matches("→").trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect())
}

#[test]
fn required_failure_chain() -> TestResult {
    let steps = chain_steps_in_design()?;
    assert_eq!(
        steps.len(),
        ChainStep::ALL.len(),
        "section 18.2 draws {} steps and this crate enumerates {}: {steps:?}",
        steps.len(),
        ChainStep::ALL.len()
    );

    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let chain = isolation_chain(&corpus)?;

    // Every step is reachable from the published chain, and each one holds the
    // one below it rather than a copy of a name.
    assert_eq!(chain.concept().concept(), "isolation");
    assert_eq!(chain.concept().tier(), EntityKind::Concept);
    assert_eq!(chain.mechanism().name(), "atomicity");
    assert_eq!(chain.need().name(), "lost-update");
    assert_eq!(chain.need().kind(), NeedKind::FailureScenario);
    assert!(
        !chain.need().sites().is_empty(),
        "a concrete need names at least one site"
    );
    assert_eq!(chain.basis().as_str(), "CURRENT_CODE");
    assert_eq!(chain.basis().snapshot_id(), corpus.snapshot.snapshot_id());
    assert_eq!(chain.gap().as_str(), "INSUFFICIENT");

    // The requirement is no wider than the finding that founded it: `P2-R2`
    // refused a repository-wide scope and this carries that refusal through.
    match chain.basis().finding_scope() {
        Some(FindingScope::Symbol { .. } | FindingScope::Component { .. }) => (),
        other => return Err(format!("the basis carries scope {other:?}").into()),
    }

    // And the chain publishes: one REQUIRED, one materialized entity.
    let set = classified(
        &corpus,
        &order_goal()?,
        std::slice::from_ref(&chain),
        &[],
        &[],
    )?;
    let required = set.labelled(ClassificationLabel::Required);
    assert_eq!(required.len(), 1);
    assert_eq!(required[0].key().concept(), "isolation");
    assert_eq!(set.requirements().len(), 1);
    assert_eq!(
        set.requirements()[0].need().name(),
        "lost-update",
        "REQ-18-016: the entity binds the concrete failure scenario"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `removing_any_chain_step_blocks_publish`
// ---------------------------------------------------------------------------

/// Builds a draft of section 18.2's chain with `omit` left out.
fn draft_without(corpus: &Corpus, omit: Option<ChainStep>) -> Result<ChainDraft, Box<dyn Error>> {
    let finding = corpus.finding("redis")?;
    let mut draft = ChainDraft::new();
    if omit != Some(ChainStep::CurrentBasis) {
        draft = draft.with_basis(CurrentBasis::of_current_code(finding)?);
    }
    if omit != Some(ChainStep::ConcreteNeed) {
        draft = draft.with_need(
            NeedKind::FailureScenario,
            &subject_id("lost-update")?,
            finding.locators().to_vec(),
        );
    }
    if omit != Some(ChainStep::ControllingMechanism) {
        draft = draft.with_mechanism(&subject_id("atomicity")?);
    }
    if omit != Some(ChainStep::RequiredConcept) {
        draft = draft.with_concept(&subject_id("isolation")?, EntityKind::Concept);
    }
    if omit != Some(ChainStep::UserEvidenceGap) {
        draft = draft.with_gap(
            UserEvidenceGap::of(
                MasteryLevel::Understood,
                FreshnessBand::High,
                EpistemicStatus::UserConfirmed,
            )
            .ok_or("an unapplied concept has an evidence gap")?,
        );
    }
    Ok(draft)
}

#[test]
fn removing_any_chain_step_blocks_publish() -> TestResult {
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;

    // The complete draft seals, so the five refusals below are about the
    // missing step and not about a draft that never worked.
    let whole = draft_without(&corpus, None)?.seal()?;
    assert_eq!(whole.concept().concept(), "isolation");

    for step in ChainStep::ALL {
        let refused = draft_without(&corpus, Some(step))?.seal();
        assert_eq!(
            refused.err(),
            Some(ClassificationError::ProofChainStepMissing(step)),
            "removing {} did not block the publish with its own code",
            step.as_str()
        );
    }

    // The five codes are distinct, so a reader can tell which step is missing.
    let codes: std::collections::BTreeSet<&str> =
        ChainStep::ALL.iter().map(|step| step.as_str()).collect();
    assert_eq!(codes.len(), ChainStep::ALL.len());

    // And a blocked chain publishes nothing: the classification over a corpus
    // with no complete chain carries no REQUIRED at all.
    let set = classified(&corpus, &order_goal()?, &[], &[], &[])?;
    assert!(set.labelled(ClassificationLabel::Required).is_empty());
    assert!(set.requirements().is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `broad_category_cannot_require_a_whole_field`
// ---------------------------------------------------------------------------

#[test]
fn broad_category_cannot_require_a_whole_field() -> TestResult {
    // Half one: the label. The only evidence that this is a backend is a
    // manifest row, and a manifest row is not an implementation to require
    // against, so there is no first step to start a chain from.
    let labelled = built(&BACKEND_LABEL_ONLY, &Artifacts::default())?;
    let express = labelled.finding("express")?;
    assert_eq!(express.tier(), EvidenceTier::PresentOnly);
    assert_eq!(
        CurrentBasis::of_current_code(express).err(),
        Some(ClassificationError::PresentOnlyIsNotAnImplementation(
            "express".to_owned()
        ))
    );
    let set = classified(&labelled, &order_goal()?, &[], &[], &[])?;
    assert!(
        set.labelled(ClassificationLabel::Required).is_empty(),
        "a backend label produced a requirement"
    );

    // Half two: the field. Even with a complete chain over real evidence, a
    // concept at `FIELD` tier — section 7.4's `broad area that carries no
    // independent prerequisite of its own` — has no route to `REQUIRED`. So
    // does an `ALIAS`, which section 7.4 says never carries evidence at all.
    let real = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let finding = real.finding("redis")?;
    for tier in [EntityKind::Field, EntityKind::Alias] {
        let need = ConcreteNeed::shown_by(
            CurrentBasis::of_current_code(finding)?,
            NeedKind::FailureScenario,
            &subject_id("lost-update")?,
            finding.locators().to_vec(),
        )?;
        let mechanism = ControllingMechanism::controlling(need, &subject_id("atomicity")?);
        assert_eq!(
            RequiredConcept::realizing(mechanism, &subject_id("distributed-systems")?, tier).err(),
            Some(ClassificationError::TierCannotBeRequired {
                concept: "distributed-systems".to_owned(),
                tier,
            }),
            "tier {} reached REQUIRED",
            tier.as_str()
        );
    }

    // Half three: the same evidence, at concept granularity, is precise rather
    // than absent. REQ-18-007's own acceptance sentence is *backend-label-only
    // fixture → no Distributed Systems requirement; lost-update chain →
    // precise Isolation requirement*, and both halves are here.
    let chain = isolation_chain(&real)?;
    let published = classified(
        &real,
        &order_goal()?,
        std::slice::from_ref(&chain),
        &[],
        &[],
    )?;
    let required = published.labelled(ClassificationLabel::Required);
    assert_eq!(required.len(), 1);
    assert_eq!(required[0].key().concept(), "isolation");

    // One chain requires one concept. A field's sibling concepts are not
    // dragged in by the one that was proved.
    assert!(
        published
            .stances()
            .iter()
            .all(|stance| stance.key().concept() != "distributed-systems"),
        "a field appeared in the published set"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `beneficial_trigger_contract`
// ---------------------------------------------------------------------------

#[test]
fn beneficial_trigger_contract() -> TestResult {
    let contract = replication_contract()?;
    assert_eq!(contract.concept(), "replication");
    assert_eq!(contract.triggers().len(), 2);
    assert_eq!(contract.state(), TriggerState::NotMet);
    assert_eq!(contract.benefit(), BenefitDimension::Scale);
    assert_eq!(contract.tradeoffs().len(), 3);

    // The contract's **own** constructor refuses an empty trigger list and an
    // empty trade-off list. Asserting only through `BenefitDraft::seal` would
    // leave these two unmeasured: the draft raises its own refusal first, so an
    // injection that removed the constructor's check passed a test that only
    // went through the door. That injection is recorded in the task report.
    let concept = subject_id("replication")?;
    assert_eq!(
        BenefitContract::new(
            &concept,
            Vec::new(),
            TriggerState::NotMet,
            BenefitDimension::Scale,
            vec![TradeOff::new("consistency")?],
        )
        .err(),
        Some(ClassificationError::BenefitPartMissing {
            concept: "replication".to_owned(),
            part: BenefitPart::Trigger,
        })
    );
    assert_eq!(
        BenefitContract::new(
            &concept,
            vec![Trigger::new("read-load-above-capacity")?],
            TriggerState::NotMet,
            BenefitDimension::Scale,
            Vec::new(),
        )
        .err(),
        Some(ClassificationError::BenefitPartMissing {
            concept: "replication".to_owned(),
            part: BenefitPart::TradeOff,
        })
    );

    // Each of the four parts is required at the draft door too, and each
    // refusal names its own part.
    let full = || -> Result<BenefitDraft, Box<dyn Error>> {
        Ok(BenefitDraft::new()
            .with_concept(&concept)
            .with_triggers(vec![Trigger::new("read-load-above-capacity")?])
            .with_state(TriggerState::NotMet)
            .with_benefit(BenefitDimension::Scale)
            .with_tradeoffs(vec![TradeOff::new("consistency")?]))
    };
    full()?.seal()?;

    let missing: [(BenefitPart, BenefitDraft); 4] = [
        (BenefitPart::Trigger, full()?.with_triggers(Vec::new())),
        (
            BenefitPart::TriggerState,
            BenefitDraft::new()
                .with_concept(&concept)
                .with_triggers(vec![Trigger::new("read-load-above-capacity")?])
                .with_benefit(BenefitDimension::Scale)
                .with_tradeoffs(vec![TradeOff::new("consistency")?]),
        ),
        (
            BenefitPart::Benefit,
            BenefitDraft::new()
                .with_concept(&concept)
                .with_triggers(vec![Trigger::new("read-load-above-capacity")?])
                .with_state(TriggerState::NotMet)
                .with_tradeoffs(vec![TradeOff::new("consistency")?]),
        ),
        (BenefitPart::TradeOff, full()?.with_tradeoffs(Vec::new())),
    ];
    for (part, draft) in missing {
        assert_eq!(
            draft.seal().err(),
            Some(ClassificationError::BenefitPartMissing {
                concept: "replication".to_owned(),
                part,
            }),
            "a contract without {} was accepted",
            part.as_str()
        );
    }

    // The contract publishes, and it publishes as `WOULD_BENEFIT_FROM` with the
    // trigger state and the trade-offs a reader is shown (`REQ-34-097`).
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let set = classified(
        &corpus,
        &order_goal()?,
        &[],
        std::slice::from_ref(&contract),
        &[],
    )?;
    let beneficial = set.labelled(ClassificationLabel::WouldBenefitFrom);
    assert_eq!(beneficial.len(), 1);
    let shown = beneficial[0]
        .outlook()
        .and_then(Outlook::contract)
        .ok_or("the published benefit carries no contract")?;
    assert_eq!(shown.state(), TriggerState::NotMet);
    assert_eq!(shown.tradeoffs().len(), 3);

    // A met trigger is still a benefit and never becomes a requirement: only
    // section 18.2's chain produces one.
    let met = BenefitContract::new(
        &subject_id("replication")?,
        vec![Trigger::new("read-load-above-capacity")?],
        TriggerState::Met,
        BenefitDimension::Scale,
        vec![TradeOff::new("consistency")?],
    )?;
    let after = classified(
        &corpus,
        &order_goal()?,
        &[],
        std::slice::from_ref(&met),
        &[],
    )?;
    assert_eq!(after.labelled(ClassificationLabel::Required).len(), 0);
    assert_eq!(
        after.labelled(ClassificationLabel::WouldBenefitFrom).len(),
        1
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `generic_nice_to_have_list_produces_zero_findings`
// ---------------------------------------------------------------------------

#[test]
fn generic_nice_to_have_list_produces_zero_findings() -> TestResult {
    // The list a technology-shopping prompt produces: names and nothing else.
    let list = [
        "graphql",
        "kubernetes",
        "kafka",
        "service-mesh",
        "elasticsearch",
    ];

    let mut contracts = Vec::new();
    let mut refusals = Vec::new();
    for name in list {
        match BenefitDraft::new().with_concept(&subject_id(name)?).seal() {
            Ok(contract) => contracts.push(contract),
            Err(error) => refusals.push(error),
        }
    }
    assert!(
        contracts.is_empty(),
        "a bare concept list produced {} contracts",
        contracts.len()
    );
    assert_eq!(refusals.len(), list.len());
    for (name, refusal) in list.iter().zip(&refusals) {
        assert_eq!(
            refusal,
            &ClassificationError::BenefitPartMissing {
                concept: (*name).to_owned(),
                part: BenefitPart::Trigger,
            },
            "the refusal for {name} did not name the missing trigger"
        );
    }

    // And the publication over that list is empty: zero findings, not five
    // findings that a later layer is expected to filter.
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let set = classified(&corpus, &order_goal()?, &[], &contracts, &[])?;
    assert!(
        set.labelled(ClassificationLabel::WouldBenefitFrom)
            .is_empty()
    );
    assert!(set.labelled(ClassificationLabel::Required).is_empty());

    // The same list with a trigger and a trade-off added is not refused, so
    // this test measures the missing contract rather than the names in it.
    let repaired = BenefitDraft::new()
        .with_concept(&subject_id("graphql")?)
        .with_triggers(vec![Trigger::new("client-shape-churn-above-baseline")?])
        .with_state(TriggerState::NotMet)
        .with_benefit(BenefitDimension::Maintainability)
        .with_tradeoffs(vec![TradeOff::new("query-cost-control")?])
        .seal()?;
    assert_eq!(repaired.concept(), "graphql");
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `observed_and_required_coexist`
// ---------------------------------------------------------------------------

#[test]
fn observed_and_required_coexist() -> TestResult {
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;

    // `redis` is observed. Build a chain that requires *the same* subject, so
    // the two classifications are about one concept and not two.
    let finding = corpus.finding("redis")?;
    assert_eq!(finding.tier(), EvidenceTier::Observed);
    let need = ConcreteNeed::shown_by(
        CurrentBasis::of_current_code(finding)?,
        NeedKind::Responsibility,
        &subject_id("cache-invalidation")?,
        finding.locators().to_vec(),
    )?;
    let mechanism = ControllingMechanism::controlling(need, &subject_id("expiry-policy")?);
    let concept =
        RequiredConcept::realizing(mechanism, &subject_id("redis")?, EntityKind::Concept)?;
    let chain = ProofChain::closed_by(
        concept,
        UserEvidenceGap::of(
            MasteryLevel::Applied,
            FreshnessBand::Stale,
            EpistemicStatus::UserConfirmed,
        )
        .ok_or("a stale applied state is uncertain")?,
    );

    let set = classified(
        &corpus,
        &order_goal()?,
        std::slice::from_ref(&chain),
        &[],
        &[],
    )?;
    let stance = set
        .stance("redis")
        .ok_or("redis has no stance in the published set")?;
    assert_eq!(
        stance.labels(),
        vec![ClassificationLabel::Observed, ClassificationLabel::Required],
        "section 18.4's first bullet: 사용 중이지만 이해 evidence가 부족할 수 있다"
    );
    let observed = stance.observed().ok_or("the observed half is gone")?;
    assert_eq!(observed.tier(), EvidenceTier::Observed);
    assert!(!observed.locators().is_empty());
    assert!(stance.outlook().and_then(Outlook::chain).is_some());
    assert_eq!(
        stance.outlook().map(Outlook::label),
        Some(ClassificationLabel::Required)
    );

    // The fifth step is what makes a concept the user *uses* still required:
    // applied but stale is a gap, and there is no `SUFFICIENT` value to pass.
    assert_eq!(chain.gap().as_str(), "UNCERTAIN");
    assert_eq!(
        UserEvidenceGap::of(
            MasteryLevel::Applied,
            FreshnessBand::High,
            EpistemicStatus::UserConfirmed
        ),
        None,
        "an applied, fresh, user-confirmed state is not a gap, so it closes no chain"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `required_and_benefit_conflict_in_one_scope`
// ---------------------------------------------------------------------------

#[test]
fn required_and_benefit_conflict_in_one_scope() -> TestResult {
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let finding = corpus.finding("redis")?;

    // One concept, one goal scope, both classifications offered.
    let chain = {
        let need = ConcreteNeed::shown_by(
            CurrentBasis::of_current_code(finding)?,
            NeedKind::FailureScenario,
            &subject_id("primary-unavailable")?,
            finding.locators().to_vec(),
        )?;
        let mechanism = ControllingMechanism::controlling(need, &subject_id("failover")?);
        let concept = RequiredConcept::realizing(
            mechanism,
            &subject_id("replication")?,
            EntityKind::Concept,
        )?;
        ProofChain::closed_by(
            concept,
            UserEvidenceGap::of(
                MasteryLevel::Exposed,
                FreshnessBand::High,
                EpistemicStatus::UserConfirmed,
            )
            .ok_or("an unapplied concept has an evidence gap")?,
        )
    };
    let contract = replication_contract()?;
    let one_goal = order_goal()?;
    assert_eq!(
        classified(
            &corpus,
            &one_goal,
            std::slice::from_ref(&chain),
            std::slice::from_ref(&contract),
            &[],
        )
        .err(),
        Some(ClassificationError::RequiredAndBenefitInOneScope(
            "replication".to_owned(),
            "no-duplicate-retryable-orders".to_owned(),
            1,
        )),
        "section 18.4's second bullet was not enforced"
    );

    // 서로 다른 goal에는 가능하다: the same two, one per goal, both publish.
    let other = goal("read-scale-headroom", 1)?;
    let required_here = classified(&corpus, &one_goal, std::slice::from_ref(&chain), &[], &[])?;
    let beneficial_there = classified(&corpus, &other, &[], std::slice::from_ref(&contract), &[])?;
    assert_eq!(
        required_here
            .stance("replication")
            .and_then(ConceptStanceLabel::outlook_label),
        Some(ClassificationLabel::Required)
    );
    assert_eq!(
        beneficial_there
            .stance("replication")
            .and_then(ConceptStanceLabel::outlook_label),
        Some(ClassificationLabel::WouldBenefitFrom)
    );
    assert_ne!(
        required_here
            .stance("replication")
            .map(|stance| stance.key().clone()),
        beneficial_there
            .stance("replication")
            .map(|stance| stance.key().clone()),
        "two goals produced one key"
    );

    // And no stance anywhere shows both labels: the outlook is one slot.
    for set in [&required_here, &beneficial_there] {
        for stance in set.stances() {
            let labels = stance.labels();
            assert!(
                !(labels.contains(&ClassificationLabel::Required)
                    && labels.contains(&ClassificationLabel::WouldBenefitFrom)),
                "{} carries both outlook labels",
                stance.key().concept()
            );
        }
    }
    Ok(())
}

/// Reads a stance's outlook label, which several tests below want.
trait ConceptStanceLabel {
    fn outlook_label(&self) -> Option<ClassificationLabel>;
}

impl ConceptStanceLabel for academic_repository_classification::ConceptStance {
    fn outlook_label(&self) -> Option<ClassificationLabel> {
        self.outlook().map(Outlook::label)
    }
}

// ---------------------------------------------------------------------------
// 8. `classification_is_snapshot_and_goal_scoped`
// ---------------------------------------------------------------------------

#[test]
fn classification_is_snapshot_and_goal_scoped() -> TestResult {
    let first = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let second = built(&OBSERVED_REDIS_SHIFTED, &Artifacts::default())?;
    assert_ne!(
        first.snapshot.snapshot_id(),
        second.snapshot.snapshot_id(),
        "the two corpora have to be two snapshots for this test to measure anything"
    );

    let v1 = order_goal()?;
    let v2 = goal("no-duplicate-retryable-orders", 2)?;

    let chain_first = isolation_chain(&first)?;
    let chain_second = isolation_chain(&second)?;

    let a1 = classified(&first, &v1, std::slice::from_ref(&chain_first), &[], &[])?;
    let a2 = classified(&first, &v2, std::slice::from_ref(&chain_first), &[], &[])?;
    let b1 = classified(&second, &v1, std::slice::from_ref(&chain_second), &[], &[])?;

    let key = |set: &ClassificationSet| {
        set.stance("isolation")
            .map(|stance| stance.key().clone())
            .ok_or("isolation has no stance")
    };
    let (ka1, ka2, kb1) = (key(&a1)?, key(&a2)?, key(&b1)?);
    assert_ne!(
        ka1, ka2,
        "two goal versions produced one classification key"
    );
    assert_ne!(ka1, kb1, "two snapshots produced one classification key");
    assert_eq!(ka1.concept(), ka2.concept());
    assert_eq!(ka1.goal().version(), 1);
    assert_eq!(ka2.goal().version(), 2);
    assert_eq!(ka1.snapshot_id(), first.snapshot.snapshot_id());
    assert_eq!(kb1.snapshot_id(), second.snapshot.snapshot_id());

    // A chain read over one snapshot cannot be published against another: the
    // binding is checked at the door rather than trusted.
    assert_eq!(
        classified(&second, &v1, std::slice::from_ref(&chain_first), &[], &[]).err(),
        Some(ClassificationError::ChainIsAboutAnotherSnapshot(
            "isolation".to_owned(),
            first.snapshot.snapshot_id().to_owned(),
        ))
    );

    // The materialized entity carries the same binding, so a requirement
    // cannot outlive the goal version it was raised under without saying so.
    assert_eq!(a1.requirements()[0].key(), &ka1);
    assert_eq!(a2.requirements()[0].key(), &ka2);
    assert_eq!(b1.requirements()[0].key(), &kb1);

    // The entity's identity separates on every axis the key separates on, and
    // on the concept too. A joined identity truncated to `RequirementId`'s 64
    // bytes does not: the snapshot identifier alone is most of that, and this
    // assertion measured exactly that collapse before the identity became a
    // digest.
    let identities = [
        a1.requirements()[0].id().as_str(),
        a2.requirements()[0].id().as_str(),
        b1.requirements()[0].id().as_str(),
    ];
    let distinct: std::collections::BTreeSet<&str> = identities.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        identities.len(),
        "two requirements differing only in snapshot or goal version share one identity:          {identities:?}"
    );

    // Re-running the same classification over the same evidence produces the
    // same identity rather than a second entity for one requirement.
    let again = classified(&first, &v1, std::slice::from_ref(&chain_first), &[], &[])?;
    assert_eq!(again.requirements()[0].id(), a1.requirements()[0].id());

    // And a different concept under one key is a different identity.
    let other_concept = {
        let finding = first.finding("redis")?;
        let need = ConcreteNeed::shown_by(
            CurrentBasis::of_current_code(finding)?,
            NeedKind::Responsibility,
            &subject_id("retry-duplication")?,
            finding.locators().to_vec(),
        )?;
        let mechanism = ControllingMechanism::controlling(need, &subject_id("idempotency-key")?);
        ProofChain::closed_by(
            RequiredConcept::realizing(
                mechanism,
                &subject_id("idempotency")?,
                EntityKind::Concept,
            )?,
            UserEvidenceGap::of(
                MasteryLevel::Exposed,
                FreshnessBand::High,
                EpistemicStatus::UserConfirmed,
            )
            .ok_or("an unapplied concept has an evidence gap")?,
        )
    };
    let two = classified(&first, &v1, &[chain_first.clone(), other_concept], &[], &[])?;
    assert_eq!(two.requirements().len(), 2);
    assert_ne!(two.requirements()[0].id(), two.requirements()[1].id());
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. `user_override_creates_conflict_not_reclassification`
// ---------------------------------------------------------------------------

#[test]
fn user_override_creates_conflict_not_reclassification() -> TestResult {
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let chain = isolation_chain(&corpus)?;
    let goal = order_goal()?;

    // Without the override the analysis publishes its requirement.
    let before = classified(&corpus, &goal, std::slice::from_ref(&chain), &[], &[])?;
    assert_eq!(
        before
            .stance("isolation")
            .and_then(ConceptStanceLabel::outlook_label),
        Some(ClassificationLabel::Required)
    );
    assert!(before.conflicts().is_empty());

    // The user says it is not required. A later analysis proposes it again.
    let decision = UserOverride::new(
        goal.clone(),
        "isolation",
        OverrideDecision::NotRequired,
        corpus.snapshot.snapshot_id(),
        4_000,
    )?;
    let after = classified(
        &corpus,
        &goal,
        std::slice::from_ref(&chain),
        &[],
        std::slice::from_ref(&decision),
    )?;

    // Nothing was reclassified: the published stance carries no outlook the
    // user struck, and no requirement entity was materialized.
    assert_eq!(
        after
            .stance("isolation")
            .and_then(ConceptStanceLabel::outlook_label),
        None,
        "the analysis overwrote the user's decision"
    );
    assert!(after.requirements().is_empty());

    // And the proposal was not discarded: it is the conflict's second side,
    // with the user's decision unchanged beside it.
    assert_eq!(after.conflicts().len(), 1);
    let conflict = &after.conflicts()[0];
    assert_eq!(conflict.key().concept(), "isolation");
    assert_eq!(conflict.proposed_label(), ClassificationLabel::Required);
    assert_eq!(
        conflict.proposed().chain(),
        Some(&chain),
        "the proposed chain was not preserved as it was"
    );
    assert_eq!(conflict.standing_override(), &decision);
    assert_eq!(
        conflict.standing_override().decision(),
        OverrideDecision::NotRequired
    );

    // Section 36.5: the override survives the next capture. It is keyed on the
    // goal and the concept, not on the snapshot it was made from.
    let next = built(&OBSERVED_REDIS_SHIFTED, &Artifacts::default())?;
    assert_ne!(next.snapshot.snapshot_id(), corpus.snapshot.snapshot_id());
    let rerun = classified(
        &next,
        &goal,
        std::slice::from_ref(&isolation_chain(&next)?),
        &[],
        std::slice::from_ref(&decision),
    )?;
    assert_eq!(
        rerun
            .stance("isolation")
            .and_then(ConceptStanceLabel::outlook_label),
        None,
        "REQ-36-029: the correction did not survive the next analysis"
    );
    assert_eq!(rerun.conflicts().len(), 1);

    // An override under another goal governs nothing here, so a decision is
    // not a global veto.
    let elsewhere = UserOverride::new(
        goal2()?,
        "isolation",
        OverrideDecision::NotRequired,
        corpus.snapshot.snapshot_id(),
        4_000,
    )?;
    let unaffected = classified(
        &corpus,
        &goal,
        std::slice::from_ref(&chain),
        &[],
        std::slice::from_ref(&elsewhere),
    )?;
    assert_eq!(
        unaffected
            .stance("isolation")
            .and_then(ConceptStanceLabel::outlook_label),
        Some(ClassificationLabel::Required)
    );
    assert!(unaffected.conflicts().is_empty());
    Ok(())
}

fn goal2() -> Result<GoalScope, Box<dyn Error>> {
    goal("read-scale-headroom", 1)
}

// ---------------------------------------------------------------------------
// 10. `requirement_entity_lifecycle_tracks_satisfied_retired_replaced`
// ---------------------------------------------------------------------------

#[test]
fn requirement_entity_lifecycle_tracks_satisfied_retired_replaced() -> TestResult {
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let chain = isolation_chain(&corpus)?;
    let goal = order_goal()?;
    let set = classified(&corpus, &goal, std::slice::from_ref(&chain), &[], &[])?;
    let opened: &ProjectConceptRequirement = &set.requirements()[0];

    // Section 18.4's six bound facts are all on the entity.
    assert_eq!(opened.key().concept(), "isolation");
    assert_eq!(opened.key().snapshot_id(), corpus.snapshot.snapshot_id());
    assert_eq!(opened.goal(), &goal);
    assert_eq!(opened.need().name(), "lost-update");
    assert_eq!(opened.user_state().as_str(), "INSUFFICIENT");
    assert_eq!(opened.status().as_str(), "OPEN");
    assert_eq!(opened.history().len(), 1);

    let later = built(&OBSERVED_REDIS_SHIFTED, &Artifacts::default())?;
    let evidence = later.finding("redis")?.locators().to_vec();

    // 충족.
    let satisfied =
        opened
            .clone()
            .satisfied(later.snapshot.snapshot_id(), evidence.clone(), 6_000)?;
    assert_eq!(satisfied.status().as_str(), "SATISFIED");
    assert_eq!(satisfied.history().len(), 2);
    assert_eq!(satisfied.history()[0].status().as_str(), "OPEN");
    assert_eq!(satisfied.history()[1].at(), 6_000);
    match satisfied.status() {
        ResolutionStatus::Satisfied {
            snapshot_id,
            evidence: shown,
        } => {
            assert_eq!(snapshot_id, later.snapshot.snapshot_id());
            assert_eq!(shown, &evidence);
        }
        other => return Err(format!("satisfied became {other:?}").into()),
    }

    // 소멸.
    let retired = opened.clone().retired(
        later.snapshot.snapshot_id(),
        RetirementReason::BasisRemoved,
        6_100,
    )?;
    assert_eq!(retired.status().as_str(), "RETIRED");

    // 대체. Section 36.6's Path A giving way to Path B.
    let successor = RequirementId::new("req-idempotency-key")?;
    let replaced =
        opened
            .clone()
            .replaced(later.snapshot.snapshot_id(), successor.clone(), 6_200)?;
    assert_eq!(replaced.status().as_str(), "REPLACED");
    match replaced.status() {
        ResolutionStatus::Replaced { by, .. } => assert_eq!(by, &successor),
        other => return Err(format!("replaced became {other:?}").into()),
    }

    // `REQ-18-018`'s *without deleting A*: the value that recorded the earlier
    // status still records it, and every successor carries the whole history.
    assert_eq!(opened.status().as_str(), "OPEN");
    assert_eq!(opened.history().len(), 1);
    for settled in [&satisfied, &retired, &replaced] {
        assert_eq!(settled.id(), opened.id());
        assert_eq!(settled.need(), opened.need());
        assert_eq!(settled.history()[0].status().as_str(), "OPEN");
        assert_eq!(settled.history().len(), 2);
    }

    // A terminal status is terminal, and the refusal names what it already is.
    assert_eq!(
        satisfied
            .clone()
            .retired(
                later.snapshot.snapshot_id(),
                RetirementReason::GoalWithdrawn,
                6_300
            )
            .err(),
        Some(ClassificationError::RequirementAlreadySettled(
            opened.id().as_str().to_owned(),
            "SATISFIED",
        ))
    );

    // The two shapes a settled status cannot take: a satisfaction with no
    // evidence in the new snapshot, and a replacement by itself.
    assert_eq!(
        opened
            .clone()
            .satisfied(later.snapshot.snapshot_id(), Vec::new(), 6_400)
            .err(),
        Some(ClassificationError::SatisfactionHasNoEvidence(
            opened.id().as_str().to_owned()
        ))
    );
    assert_eq!(
        opened
            .clone()
            .replaced(later.snapshot.snapshot_id(), opened.id().clone(), 6_500)
            .err(),
        Some(ClassificationError::RequirementReplacesItself(
            opened.id().as_str().to_owned()
        ))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 11. `finding_locator_migration_preserves_original_evidence`
// ---------------------------------------------------------------------------

#[test]
fn finding_locator_migration_preserves_original_evidence() -> TestResult {
    let before = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let after = built(&OBSERVED_REDIS_SHIFTED, &Artifacts::default())?;
    let original = before.finding("redis")?;

    // The corpus really does hold the two collapsing shapes this guard is
    // about. Without them the assertions below would pass vacuously.
    let equal_pairs = original
        .locators()
        .iter()
        .enumerate()
        .flat_map(|(at, left)| {
            original
                .locators()
                .iter()
                .skip(at + 1)
                .filter(move |right| *right == left)
        })
        .count();
    assert!(
        equal_pairs >= 1,
        "the corpus produced no pair of byte-identical locators; the duplicate guard would be \
         vacuous"
    );
    let with_symbol: Vec<_> = original
        .locators()
        .iter()
        .filter_map(academic_repository_analysis::Locator::symbol)
        .collect();
    assert!(
        with_symbol.len() >= 2 && with_symbol[0] == with_symbol[1],
        "the corpus produced no pair of locators sharing one enclosing symbol; the collapse \
         guard would be vacuous"
    );

    let migrated = migrate_locators(original, &after.analysis);

    // One record per original locator, in order, whatever the originals or the
    // targets are equal to. This is the `P2-A1` defect refused: identity is the
    // position, never the content.
    assert_eq!(migrated.migrations().len(), original.locators().len());
    for (at, migration) in migrated.migrations().iter().enumerate() {
        assert_eq!(migration.ordinal(), at);
        assert_eq!(migration.original(), &original.locators()[at]);
    }

    // The original evidence is preserved whole, and it still names snapshot A.
    assert_eq!(migrated.original(), original);
    assert_eq!(migrated.from_snapshot(), before.snapshot.snapshot_id());
    assert_eq!(migrated.to_snapshot(), after.snapshot.snapshot_id());
    assert_ne!(migrated.from_snapshot(), migrated.to_snapshot());

    // The symbol-anchored locators migrated, and their spans moved because the
    // comment block above `warm` moved them.
    assert!(migrated.migrated_count() >= 2);
    let mut moved = 0_usize;
    for migration in migrated.migrations() {
        match migration.outcome() {
            MigrationOutcome::Migrated(site) => {
                assert_eq!(
                    Some(site.symbol()),
                    migration.original().symbol(),
                    "a migration landed on another symbol"
                );
                assert_eq!(site.path(), migration.original().path());
                if site.span() != migration.original().span() {
                    moved += 1;
                }
            }
            MigrationOutcome::Unmatched(reason) => assert_eq!(
                *reason,
                UnmatchedReason::NoSymbolAnchor,
                "a locator inside this corpus failed for an unexpected reason"
            ),
        }
    }
    assert!(
        moved >= 2,
        "no span moved; the shifted corpus is not exercising the migration"
    );

    // A symbol that is gone is reported as gone rather than dropped, and the
    // original locator is still there to read.
    let without = built(&OBSERVED_REDIS_WITHOUT_WARM, &Artifacts::default())?;
    let lost = migrate_locators(original, &without.analysis);
    assert_eq!(lost.migrations().len(), original.locators().len());
    assert_eq!(lost.migrated_count(), 0);
    assert!(
        lost.migrations().iter().any(|migration| matches!(
            migration.outcome(),
            MigrationOutcome::Unmatched(UnmatchedReason::SymbolGone)
        )),
        "a removed symbol was not reported as gone"
    );
    assert_eq!(lost.original(), original);
    Ok(())
}

// ---------------------------------------------------------------------------
// The floor under the eleven.
// ---------------------------------------------------------------------------

/// Section 18's three classification names, read out of the design document.
#[test]
fn the_three_classifications_are_the_design_document_s() -> TestResult {
    let page = fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let section = page
        .find("## 18. Project Concept Classification")
        .ok_or("section 18 is not in the design document")?;
    let end = page[section..]
        .find("## 19. Project Lens")
        .map_or(page.len(), |offset| section + offset);
    let body = &page[section..end];
    for label in ClassificationLabel::ALL {
        assert!(
            body.contains(label.as_str()),
            "section 18 does not name {}",
            label.as_str()
        );
    }
    for glyph in ClassificationLabel::ALL.map(ClassificationLabel::glyph) {
        assert!(
            page.contains(glyph),
            "section 19's legend does not carry {glyph}"
        );
    }

    // The corpora are what every test above rests on: an observed subject and a
    // manifest-only one. A corpus that stopped producing either would make
    // several assertions above vacuous.
    let observed = built(&OBSERVED_REDIS, &Artifacts::default())?;
    assert_eq!(observed.finding("redis")?.tier(), EvidenceTier::Observed);
    let labelled = built(&BACKEND_LABEL_ONLY, &Artifacts::default())?;
    assert_eq!(
        labelled.finding("express")?.tier(),
        EvidenceTier::PresentOnly
    );
    Ok(())
}

/// An approved goal is a chain basis and a draft one is not.
#[test]
fn only_an_approved_goal_founds_a_requirement() -> TestResult {
    let artifacts = Artifacts {
        intent: vec![intent_document(
            "spec-1",
            ApprovalStatus::Approved,
            3,
            "docs/spec.md",
            &["idempotency"],
        )?],
        behavior: vec![BehaviorDocument::new(
            DocumentId::new("beh-1")?,
            "docs/behaviour.md",
            vec![subject_id("redis")?],
        )],
    };
    let files = [
        OBSERVED_REDIS[0],
        OBSERVED_REDIS[1],
        OBSERVED_REDIS[2],
        SPEC_PAGE,
        BEHAVIOR_PAGE,
    ];
    let corpus = built(&files, &artifacts)?;
    let goal = order_goal()?;

    let approved = intent_document(
        "spec-1",
        ApprovalStatus::Approved,
        3,
        "docs/spec.md",
        &["idempotency"],
    )?;
    let basis =
        CurrentBasis::of_approved_goal(corpus.snapshot.snapshot_id(), goal.clone(), &approved)?;
    assert_eq!(basis.as_str(), "APPROVED_GOAL");
    assert_eq!(basis.goal(), Some(&goal));

    for status in [ApprovalStatus::Draft, ApprovalStatus::Deprecated] {
        let document = intent_document("spec-1", status, 3, "docs/spec.md", &["idempotency"])?;
        assert_eq!(
            CurrentBasis::of_approved_goal(corpus.snapshot.snapshot_id(), goal.clone(), &document)
                .err(),
            Some(ClassificationError::GoalIsNotApproved(
                "spec-1".to_owned(),
                status.as_str(),
            ))
        );
    }
    Ok(())
}

/// A need with no site in the snapshot is not concrete.
#[test]
fn a_need_without_a_site_is_not_concrete() -> TestResult {
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let finding = corpus.finding("redis")?;
    assert_eq!(
        ConcreteNeed::shown_by(
            CurrentBasis::of_current_code(finding)?,
            NeedKind::FailureScenario,
            &subject_id("lost-update")?,
            Vec::new(),
        )
        .err(),
        Some(ClassificationError::NeedHasNoSite("lost-update".to_owned()))
    );
    Ok(())
}

/// Every identifier this crate takes is the shape it says it admits.
///
/// A whole-set classification rather than a list of rejected spellings: every
/// ASCII byte is offered inside an otherwise legal identifier and required to
/// be admitted **exactly** when this test's own independent predicate says it
/// belongs, in both directions, for all five constructors that reach
/// `scope::validated`, and the length bound is asserted on both sides.
///
/// It is here because `P2-Y2` measured the gap and `P2-A5` measured it again
/// and wider: adding `+` to the character class, and moving the length bound
/// from 64 to 65, each left this crate's whole suite green. A rule that is
/// declared and unmeasured is a rule the next edit may widen for free. It is
/// the port of `P2-R5`'s `every_identifier_is_the_shape_this_crate_admits`,
/// which is where the same shape was first measured this way.
#[test]
fn every_identifier_is_the_shape_this_crate_admits() -> TestResult {
    // Written here rather than read from the crate, so the two are independent.
    let belongs =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-';
    let scope = GoalScope::at(GoalId::new("g")?, 1);

    for byte in 0_u8..=127 {
        let candidate = format!("a{}b", char::from(byte));
        for taken in [
            GoalId::new(candidate.clone()).is_ok(),
            RequirementId::new(candidate.clone()).is_ok(),
            Trigger::new(candidate.clone()).is_ok(),
            TradeOff::new(candidate.clone()).is_ok(),
            UserOverride::new(
                scope.clone(),
                candidate.clone(),
                OverrideDecision::NotRequired,
                "snap_shape",
                NOW,
            )
            .is_ok(),
        ] {
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
                GoalId::new(outside),
                Err(ClassificationError::InvalidIdentifier("goal", _))
            ),
            "{outside:?} was admitted as a goal identifier"
        );
    }

    // The length boundary, on both sides of it, and the empty value.
    let longest = "a".repeat(64);
    assert!(GoalId::new(longest.as_str()).is_ok());
    assert!(RequirementId::new(longest.as_str()).is_ok());
    assert!(Trigger::new(longest.as_str()).is_ok());
    assert!(TradeOff::new(longest.as_str()).is_ok());
    for refused in [String::new(), "a".repeat(65)] {
        let length = refused.len();
        for outcome in [
            GoalId::new(refused.as_str()).err(),
            RequirementId::new(refused.as_str()).err(),
            Trigger::new(refused.as_str()).err(),
            TradeOff::new(refused.as_str()).err(),
            UserOverride::new(
                scope.clone(),
                refused.as_str(),
                OverrideDecision::NotRequired,
                "snap_shape",
                NOW,
            )
            .err(),
        ] {
            assert!(
                matches!(outcome, Some(ClassificationError::InvalidIdentifier(_, _))),
                "a {length}-byte identifier was admitted"
            );
        }
    }

    // Each constructor names itself in its refusal, so a reader is told which
    // identifier was wrong rather than that one of five was.
    let named: BTreeSet<&'static str> = [
        GoalId::new("").err(),
        RequirementId::new("").err(),
        Trigger::new("").err(),
        TradeOff::new("").err(),
        UserOverride::new(scope, "", OverrideDecision::NotRequired, "snap_shape", NOW).err(),
    ]
    .into_iter()
    .filter_map(|error| match error {
        Some(ClassificationError::InvalidIdentifier(what, _)) => Some(what),
        _ => None,
    })
    .collect();
    assert_eq!(
        named,
        BTreeSet::from(["concept", "goal", "requirement", "tradeoff", "trigger"]),
        "two identifiers share a name in their refusal, or one does not name itself"
    );
    Ok(())
}

/// Section 18.2's chain, but about `redis` itself.
///
/// The same shape as [`isolation_chain`] with the realized concept changed, so
/// one concept arrives at `classify` through both routes at once: the
/// correlation observes `redis` and this chain requires it.
fn redis_chain(corpus: &Corpus) -> Result<ProofChain, Box<dyn Error>> {
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
        RequiredConcept::realizing(mechanism, &subject_id("redis")?, EntityKind::Concept)?;
    Ok(ProofChain::closed_by(
        concept,
        UserEvidenceGap::of(
            MasteryLevel::Understood,
            FreshnessBand::High,
            EpistemicStatus::UserConfirmed,
        )
        .ok_or("an unapplied concept has an evidence gap")?,
    ))
}

#[test]
fn one_concept_is_one_stance_however_many_routes_reach_it() -> TestResult {
    // What `P2-R5`'s `promote` rests on. It walks the stances and keeps one
    // work per concept; a second refusal keyed on the concept would be one
    // nothing can reach, because this crate emits one stance per concept
    // whatever combination of routes named it. `P2-A5` measured such a refusal
    // sitting there undriven, and this is the fact that made it undriven,
    // stated where it is true rather than defended where it is not.
    let corpus = built(&OBSERVED_REDIS, &Artifacts::default())?;
    let goal = order_goal()?;

    // Both routes at once: the correlation observes `redis` and the chain
    // requires it.
    let set = classified(&corpus, &goal, &[redis_chain(&corpus)?], &[], &[])?;
    let redis: Vec<&ConceptStance> = set
        .stances()
        .iter()
        .filter(|stance| stance.key().concept() == "redis")
        .collect();
    assert_eq!(
        redis.len(),
        1,
        "one concept produced {} stances",
        redis.len()
    );
    let only = redis.first().ok_or("no stance for redis")?;
    assert!(
        only.observed().is_some(),
        "the observation route was dropped, so the two routes did not meet here"
    );
    assert!(
        matches!(only.outlook(), Some(Outlook::Required(_))),
        "the requirement route was dropped, so the two routes did not meet here"
    );

    // And as a property of the whole set rather than of `redis`: no concept
    // appears twice, over every combination of routes this suite can build.
    for required in [
        Vec::new(),
        vec![redis_chain(&corpus)?],
        vec![isolation_chain(&corpus)?],
    ] {
        for beneficial in [Vec::new(), vec![replication_contract()?]] {
            let set = classified(&corpus, &goal, &required, &beneficial, &[])?;
            let concepts: Vec<&str> = set
                .stances()
                .iter()
                .map(|stance| stance.key().concept())
                .collect();
            let distinct: BTreeSet<&str> = concepts.iter().copied().collect();
            assert_eq!(
                concepts.len(),
                distinct.len(),
                "a concept has two stances: {concepts:?}"
            );
            assert!(
                !concepts.is_empty(),
                "the set is empty, so the property above says nothing"
            );
        }
    }
    Ok(())
}
