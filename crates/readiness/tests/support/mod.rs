//! The deterministic synthetic corpus every `P2-Y3` fixture is built from.
//!
//! `CONTRIBUTING.md` rule 1 admits only synthetic fixtures and rule 5 admits a
//! golden fixture only through a deterministic builder. This module is that
//! builder.
//!
//! **Nothing here fabricates a value another crate owns.** Every competency is
//! declared through `P2-Y1`'s own `declare`, every bundle through `P2-Y2`'s own
//! `declare`, every knowledge-state item through `P2-N2`'s own four eligibility
//! checks, and every personal application claim through the whole `P2-R1` →
//! `P2-R2` → `P2-R3` → `P2-R4` → `P2-R5` chain — captured, analyzed,
//! correlated, classified and promoted by those crates' own functions. The two
//! `StageEvidence` doors are `P2-Y1`'s, and this crate adds neither a third nor
//! a shortcut past either.
//!
//! The repository chain is the same corpus `P2-Y1`'s own suite builds, for the
//! reason that suite gives: a change there should move this corpus too, and two
//! fixtures that drifted into disagreeing about one snapshot would each be
//! measuring a different thing while looking like they measured one.

#![allow(dead_code)]

use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

use academic_competency::{
    Competency, CompetencyId, ConceptRef, ContributionImportance, CriterionId, EnablingConcept,
    EvidenceRubric, EvidenceStage, Necessity, PerformanceCriterion, PromotingEvidence, RecordId,
    RubricRow, Situation, StageEvidence, declare,
};
use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, EntityId, EpistemicStatus,
    EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength, FreshnessBand,
    MasteryLevel, PredicateId, ScopeId, TimestampMillis, ValidInterval,
    entity_registry::EntityKind,
};
use academic_knowledge_state::{
    ConceptEvidence, ConceptLink, EligibilityOutcome, EvidenceDossier, ExerciseOutcome,
    IncidentRepair, Outcome, Participation, SelfExplanation, SourceIntegrity, UserConfirmation,
};
use academic_model_run::{
    CalibrationBin, CalibrationDataset, CalibrationDatasetId, CalibrationRegistry, Digest32,
    ModelVersion, ProviderId, Purpose,
};
use academic_policy::ContentDigest as PolicyDigest;
use academic_readiness::{
    AxisEvidence, EvidenceLocatorId, ReadinessAxis, ReadinessError, ReadinessMatrix, ReadinessView,
};
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
    PersonalApplicationClaim, PromotionInput, PromotionSet, RubricId, ScaffoldRubric, UserId,
    promote,
};
use academic_repository_correlation::{Correlation, CorrelationInput, correlate};
use academic_role_profile::{
    BundleEntry, BundleImportance, BundleScope, BundleSource, RecordedOn, RoleDirection, RoleError,
    RoleLabel, RoleProfile, RoleProfileId, declare as declare_bundle,
};
use academic_untrusted_content::SourceIndex;

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub const CAPTURED_AT: u64 = 1_756_000_000_000;
pub const HEAD: &str = "abc1234def5678";
pub const BRANCH: &str = "main";
pub const ANALYZER: &str = "academic-repository-analysis";
pub const VERSION: &str = "0.1.0";
pub const NOW: u64 = 5_000;
pub const USER: &str = "user-1";
pub const OWN_ADDRESS: &str = "owner@example.test";

// ---------------------------------------------------------------------------
// The design document, which is this suite's oracle for every list.
// ---------------------------------------------------------------------------

/// The workspace root, from this crate's own manifest directory.
#[must_use]
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// The design document, read from the workspace root.
pub fn design_page() -> TestResult<String> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// The text from `start` up to the next occurrence of `end`.
pub fn section(page: &str, start: &str, end: &str) -> TestResult<String> {
    let from = page
        .find(start)
        .ok_or_else(|| format!("{start} is not in the design document"))?;
    let rest = &page[from..];
    let to = rest
        .find(end)
        .ok_or_else(|| format!("{start} does not end before {end}"))?;
    Ok(rest[..to].to_owned())
}

// ---------------------------------------------------------------------------
// Deterministic identities. No clock and no random source.
// ---------------------------------------------------------------------------

fn uuid_of(tag: &str) -> uuid::Uuid {
    let digest = ContentDigest::sha256(tag.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

/// A deterministic security-domain identifier for one fixture name.
#[must_use]
pub fn domain_id(tag: &str) -> academic_domain::DomainId {
    academic_domain::DomainId::try_from_uuid(uuid_of(tag))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

/// A deterministic entity identifier for one fixture name.
#[must_use]
pub fn entity(tag: &str) -> EntityId {
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
    ScopeId::try_from_uuid(uuid_of("scope-readiness"))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn user_actor() -> Actor {
    Actor::User {
        user_id: entity("the-one-user"),
    }
}

// ---------------------------------------------------------------------------
// `P2-N2`'s door: admitted evidence, through that crate's own four checks.
// ---------------------------------------------------------------------------

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

fn confirmation(concept: EntityId) -> TestResult<UserConfirmation> {
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
pub fn admitted(
    evidence: ConceptEvidence,
    tag: &str,
    concept: EntityId,
) -> TestResult<academic_knowledge_state::EligibleEvidence> {
    EligibilityOutcome::admit(evidence, evidence_id(tag), &full_dossier(concept))
        .admitted()
        .cloned()
        .ok_or_else(|| format!("{tag} was not admitted by P2-N2").into())
}

/// `P2-N2` evidence of every promoting kind this fixture needs, by tag.
///
/// Three of section 13.2's rows, each built through that crate's own
/// constructor. They are the three whose inputs are evidence identifiers: the
/// two this fixture does not build -- `MeaningfulTeaching` and
/// `AuthoredProjectCode` -- take a `P2-L4` document node and a `P2-R4`
/// `OBSERVED` stance respectively, and the second of those is reached here by
/// the other door instead, through `P2-R5`'s own promotion.
pub fn concept_evidence(tag: &str, concept: EntityId) -> TestResult<ConceptEvidence> {
    Ok(match tag {
        "explained" => ConceptEvidence::SelfExplanation(SelfExplanation::confirmed_by(
            evidence_id("explained-artifact"),
            &confirmation(concept)?,
        )),
        "exercise" => ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id(
            "exercise-artifact",
        ))),
        "incident" => ConceptEvidence::IncidentDebugging(IncidentRepair::of(
            evidence_id("incident"),
            evidence_id("incident-cause"),
            evidence_id("incident-fix"),
            evidence_id("incident-verification"),
        )),
        other => return Err(format!("no fixture evidence is named {other}").into()),
    })
}

/// One `P2-Y1` record founded on `P2-N2`'s admitted evidence.
pub fn knowledge_record(
    tag: &str,
    stage: EvidenceStage,
    concept: EntityId,
) -> TestResult<StageEvidence> {
    let evidence = concept_evidence(tag, concept)?;
    let promoting = PromotingEvidence::of(admitted(evidence, tag, concept)?)?;
    Ok(StageEvidence::of_knowledge_state(
        RecordId::new(tag)?,
        stage,
        &promoting,
    ))
}

/// The `P2-N2` evidence identifier one knowledge record carries, as text.
///
/// This is the identity `StartingPoint::Course` names, so a fixture that wants
/// a walk to reach a record has to use the identifier the record itself
/// carries rather than one it typed.
#[must_use]
pub fn knowledge_evidence_id(tag: &str) -> String {
    evidence_id(tag).to_string()
}

// ---------------------------------------------------------------------------
// `P2-R5`'s door: the whole repository chain, run rather than fabricated.
// ---------------------------------------------------------------------------

/// `redis` is imported and called from an entry point, so `P2-R2` reaches its
/// `OBSERVED` rung. `express` appears in the manifest and nowhere else, which is
/// section 13.2's `dependency/install/import만 존재` row.
pub const MIXED_CORPUS: [(&str, &str); 4] = [
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

fn captured(files: &[(&str, &str)]) -> TestResult<(RepositorySnapshot, SourceIndex)> {
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

fn analyzed(files: &[(&str, &str)]) -> TestResult<(RepositorySnapshot, RepositoryAnalysis)> {
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

fn purpose() -> TestResult<Purpose> {
    Ok(Purpose::new("REPOSITORY_EVIDENCE_TIER")?)
}

fn registry() -> TestResult<CalibrationRegistry> {
    let mut registry = CalibrationRegistry::new();
    registry.register(CalibrationDataset::new(
        CalibrationDatasetId::new("cal-readiness")?,
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

fn subject_id(name: &str) -> TestResult<SubjectId> {
    Ok(SubjectId::new(name)?)
}

fn subjects() -> TestResult<Vec<Subject>> {
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

fn findings_of(analysis: &RepositoryAnalysis) -> TestResult<Vec<Finding>> {
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

/// One captured, analyzed and correlated snapshot.
pub struct Corpus {
    snapshot: RepositorySnapshot,
    findings: Vec<Finding>,
    correlation: Correlation,
}

impl Corpus {
    fn finding(&self, subject: &str) -> TestResult<&Finding> {
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

    fn site(&self, path: &str) -> TestResult<Locator> {
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

/// Builds the corpus by running `P2-R1`, `P2-R2` and `P2-R3`.
pub fn built(files: &[(&str, &str)]) -> TestResult<Corpus> {
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
        declared_dependencies: &[],
    })?;
    Ok(Corpus {
        snapshot,
        findings,
        correlation,
    })
}

fn order_goal() -> TestResult<GoalScope> {
    Ok(GoalScope::at(
        GoalId::new("no-duplicate-retryable-orders")?,
        1,
    ))
}

fn classified(corpus: &Corpus, goal: &GoalScope) -> TestResult<ClassificationSet> {
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
        goal,
        required: &[chain],
        beneficial: &[],
        overrides: &[],
    })?)
}

fn user() -> TestResult<UserId> {
    Ok(UserId::new(USER)?)
}

fn own_identity() -> TestResult<ExternalAuthorId> {
    Ok(ExternalAuthorId::new(
        IdentitySource::GitAuthorEmail,
        OWN_ADDRESS,
    )?)
}

fn mapping() -> TestResult<AuthorshipMap> {
    Ok(AuthorshipMap::of(user()?, 3, vec![own_identity()?]))
}

fn scaffold_rubric() -> TestResult<ScaffoldRubric> {
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

fn own_work(corpus: &Corpus) -> TestResult<AuthoredWork> {
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

fn promoted(corpus: &Corpus) -> TestResult<PromotionSet> {
    let classification = classified(corpus, &order_goal()?)?;
    let user = user()?;
    let works = vec![own_work(corpus)?];
    Ok(promote(&PromotionInput {
        classification: &classification,
        user: &user,
        works: &works,
        outcomes: &[],
    })?)
}

/// The one `P2-R5` personal application claim this corpus promotes.
pub fn personal_claim() -> TestResult<PersonalApplicationClaim> {
    let corpus = built(&MIXED_CORPUS)?;
    let set = promoted(&corpus)?;
    set.personal_claims()
        .first()
        .cloned()
        .ok_or_else(|| "the corpus promoted no personal application claim".into())
}

/// One `P2-Y1` record founded on that claim.
pub fn personal_record(tag: &str, stage: EvidenceStage) -> TestResult<StageEvidence> {
    let claim = personal_claim()?;
    Ok(StageEvidence::of_personal_claim(
        RecordId::new(tag)?,
        stage,
        &claim,
    )?)
}

// ---------------------------------------------------------------------------
// Competencies and bundles, through their own crates' doors.
// ---------------------------------------------------------------------------

/// Names the concept a criterion is about, in the ontology namespace.
#[must_use]
pub fn ontology(name: &str) -> ConceptRef {
    ConceptRef::ontology(entity(name))
}

/// Declares one competency with `count` criteria, each witnessed at every one
/// of `stages`, about `concept`.
pub fn competency_about(
    id: &str,
    concept: &ConceptRef,
    criteria_names: &[&str],
    stages: &[EvidenceStage],
) -> TestResult<Competency> {
    let enabled_by = vec![EnablingConcept::of(
        concept.clone(),
        ContributionImportance::Substantial,
        Necessity::Necessary,
    )];
    let mut criteria = Vec::new();
    let mut rubric_rows = Vec::new();
    for name in criteria_names {
        let criterion = PerformanceCriterion::of(
            CriterionId::new(*name)?,
            format!("demonstrates {name} in the fixture situation"),
            vec![concept.clone()],
        )?;
        for stage in stages {
            rubric_rows.push(RubricRow::of(
                criterion.id().clone(),
                *stage,
                format!("{name} at {}", stage.as_str()),
            )?);
        }
        criteria.push(criterion);
    }
    Ok(declare(
        CompetencyId::new(id)?,
        Situation::new("a multi-tier fixture service")?,
        criteria,
        enabled_by,
        EvidenceRubric::of(rubric_rows),
    )?)
}

/// Declares one bundle over the given entries.
pub fn bundle(id: &str, entries: Vec<BundleEntry>) -> Result<RoleProfile, RoleError> {
    declare_bundle(
        RoleProfileId::new(id)?,
        RoleLabel::new("Fixture Engineer")?,
        RoleDirection::Backend,
        RecordedOn::parse("2026-08-26")?,
        BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
        entries,
        vec![BundleSource::cited(
            "the fixture's own note",
            RecordedOn::parse("2026-08-20")?,
        )?],
    )
}

/// One entry, at the given importance.
pub fn entry(competency: &Competency, importance: BundleImportance) -> BundleEntry {
    BundleEntry::of(competency.id().clone(), importance)
}

/// Places one record in one column against one criterion.
pub fn placed(
    axis: ReadinessAxis,
    criterion: &str,
    locator: &str,
    record: &StageEvidence,
) -> Result<AxisEvidence, Box<dyn Error>> {
    Ok(AxisEvidence::place(
        axis,
        CriterionId::new(criterion)?,
        EvidenceLocatorId::new(locator)?,
        record,
    )?)
}

/// A view of one matrix over one competency slice.
pub fn view(
    matrix: ReadinessMatrix,
    competencies: &[&Competency],
) -> Result<ReadinessView, ReadinessError> {
    ReadinessView::of(matrix, competencies)
}
