//! `P2-R5`'s named acceptance evidence.
//!
//! Every corpus is synthetic and built in process, captured through `P2-R1`'s
//! own `capture_local`, analyzed through `P2-R2`'s own ladder, correlated
//! through `P2-R3`'s own `correlate` and classified through `P2-R4`'s own
//! `classify`, so every promotion here rests on evidence that passed the
//! permission and secret gate, the frozen manifest, the sealed
//! untrusted-content index, the evidence ladder, the two authority lanes and
//! section 18's publication rules before this file saw it.
//!
//! A [`ChangedSite`] can only be built over a `P2-R2` [`Locator`], whose
//! constructor is crate-private to that crate, so a contribution in this suite
//! cannot name a place the analyzer did not see.
//!
//! The one file this suite reads is the design document itself, and only to
//! compare section 17.6's own five bullets and section 13.2's own ceiling rows
//! against the enumerations: a list restated in a test is a list that can be
//! restated wrongly.

use std::{collections::BTreeMap, collections::BTreeSet, error::Error, fs, path::PathBuf};

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
    AnalysisInput, AnalyzerIdentity, EvidenceLadder, Finding, Locator, PathClass,
    RepositoryAnalysis, SourceUnit, Subject, SubjectId, analyze,
};
use academic_repository_classification::{
    ClassificationInput, ClassificationSet, ConcreteNeed, ControllingMechanism, CurrentBasis,
    GoalId, GoalScope, NeedKind, ProofChain, RequiredConcept, UserEvidenceGap, classify,
};
use academic_repository_competency::{
    AuthoredWork, AuthorshipMap, AuthorshipMode, CandidateSupport, ChangeId, ChangeKind,
    ChangeVerdict, ChangedSite, ClaimStanding, CodeOrigin, CompetencyError, ContributionDraft,
    ContributionKind, ContributionRecord, ExplainedByUser, ExternalAuthorId, GeneratedCodeWarrant,
    IdentitySource, ModifiedByUser, OriginReport, OutcomeArtifact, OutcomeKind, PromotionCheck,
    PromotionInput, PromotionSet, RejectionReason, RubricId, ScaffoldRubric, UserId,
    VerifiedByUser, WarrantStep, observation_alone_promotes, promote,
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

/// The user every corpus here is promoted for.
const USER: &str = "user-1";
/// The address that user recorded as theirs.
const OWN_ADDRESS: &str = "owner@example.test";
/// A second person on the same repository.
const OTHER_ADDRESS: &str = "colleague@example.test";

// ---------------------------------------------------------------------------
// The deterministic harness: `P2-R1`, `P2-R2`, `P2-R3`, `P2-R4`, then this
// crate.
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

/// One capture, one analysis, one correlation, one classification.
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
        declared_dependencies: &[],
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

    /// One locator of the `redis` finding, by path.
    ///
    /// A `P2-R2` locator is the only thing a [`ChangedSite`] can be built over,
    /// so this is how the suite names a place at all.
    fn site(&self, path: &str) -> Result<Locator, Box<dyn Error>> {
        let finding = self.finding("redis")?;
        finding
            .locators()
            .iter()
            .find(|locator| locator.path() == path)
            .cloned()
            .ok_or_else(|| format!("the redis finding names no locator at {path}").into())
    }

    /// One **excluded** locator of the `redis` finding, by path.
    ///
    /// A vendored, generated or example site is recorded on the finding and
    /// never counted, so it is this suite's only route to a locator whose
    /// `PathClass` is not `FIRST_PARTY`.
    fn excluded_site(&self, path: &str) -> Result<Locator, Box<dyn Error>> {
        let finding = self.finding("redis")?;
        finding
            .excluded_sites()
            .iter()
            .find(|site| site.locator().path() == path)
            .map(|site| site.locator().clone())
            .ok_or_else(|| format!("the redis finding excludes no site at {path}").into())
    }

    /// The first locator of the `redis` finding at `path` that sits **inside a
    /// declaration**, so it carries a `P2-R2` symbol fingerprint.
    ///
    /// `site` takes the first locator at a path, and for `src/cache.ts` that is
    /// the module-level import, which has no symbol — so a work built on it is
    /// joined to an observation by path. This one is how the fingerprint branch
    /// is reached.
    fn symbol_site(&self, path: &str) -> Result<Locator, Box<dyn Error>> {
        let finding = self.finding("redis")?;
        finding
            .locators()
            .iter()
            .find(|locator| locator.path() == path && locator.symbol().is_some())
            .cloned()
            .ok_or_else(|| format!("no locator at {path} sits inside a declaration").into())
    }

    fn snapshot_id(&self) -> &str {
        self.snapshot.snapshot_id()
    }
}

fn goal(name: &str, version: u64) -> Result<GoalScope, Box<dyn Error>> {
    Ok(GoalScope::at(GoalId::new(name)?, version))
}

fn order_goal() -> Result<GoalScope, Box<dyn Error>> {
    goal("no-duplicate-retryable-orders", 1)
}

/// `P2-R4` over one corpus, with section 18.2's chain for `isolation` so the
/// classification carries a requirement beside its observation.
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

// ---------------------------------------------------------------------------
// The user, the mapping and the rubric.
// ---------------------------------------------------------------------------

fn user() -> Result<UserId, Box<dyn Error>> {
    Ok(UserId::new(USER)?)
}

fn own_identity() -> Result<ExternalAuthorId, Box<dyn Error>> {
    Ok(ExternalAuthorId::new(
        IdentitySource::GitAuthorEmail,
        OWN_ADDRESS,
    )?)
}

/// The user's recorded mapping: one address, in one namespace, at version 3.
fn mapping() -> Result<AuthorshipMap, Box<dyn Error>> {
    Ok(AuthorshipMap::of(user()?, 3, vec![own_identity()?]))
}

/// The rubric every corpus here is judged under.
///
/// Configuration, supplied here and recorded in every claim it decides. Its
/// parts are written out rather than defaulted, because there is no default.
fn rubric() -> Result<ScaffoldRubric, Box<dyn Error>> {
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

fn change(id: &str) -> Result<ChangeId, Box<dyn Error>> {
    Ok(ChangeId::new(id)?)
}

/// A report the connector could produce, with every part an argument.
fn report(
    corpus: &Corpus,
    id: &str,
    author: ExternalAuthorId,
    kind: ContributionKind,
    origin: OriginReport,
    sites: Vec<ChangedSite>,
) -> Result<ContributionRecord, Box<dyn Error>> {
    Ok(ContributionRecord {
        change: change(id)?,
        snapshot_id: corpus.snapshot_id().to_owned(),
        author,
        kind,
        origin,
        sites,
        recorded_at: 1_756_100_000_000,
    })
}

/// The sites of a change that touched the cache module's control flow: one
/// understanding-bearing site at the place the `redis` observation names.
fn meaningful_sites(corpus: &Corpus) -> Result<Vec<ChangedSite>, Box<dyn Error>> {
    Ok(vec![ChangedSite::new(
        corpus.site("src/cache.ts")?,
        ChangeKind::ControlFlow,
    )])
}

/// The sites of a change that bumped a pin and edited a compose file.
fn scaffold_sites(corpus: &Corpus) -> Result<Vec<ChangedSite>, Box<dyn Error>> {
    Ok(vec![
        ChangedSite::new(corpus.site("package.json")?, ChangeKind::DependencyPin),
        ChangedSite::new(
            corpus.site("docker-compose.yml")?,
            ChangeKind::ConfigurationValue,
        ),
    ])
}

/// The user's own hand-written meaningful change, sealed.
fn own_work(corpus: &Corpus) -> Result<AuthoredWork, Box<dyn Error>> {
    let record = report(
        corpus,
        "c-own",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        meaningful_sites(corpus)?,
    )?;
    let map = mapping()?;
    let rubric = rubric()?;
    Ok(ContributionDraft::over(&record, &map, &rubric).seal()?)
}

fn promoted(
    classification: &ClassificationSet,
    works: &[AuthoredWork],
    outcomes: &[OutcomeArtifact],
) -> Result<PromotionSet, Box<dyn Error>> {
    let user = user()?;
    Ok(promote(&PromotionInput {
        classification,
        user: &user,
        works,
        outcomes,
    })?)
}

// ---------------------------------------------------------------------------
// The corpus.
// ---------------------------------------------------------------------------

const SPEC_PAGE: (&str, &str) = ("docs/spec.md", "# Order platform\n\nA specification.\n");

/// `redis` reachable from an entry point with a production configuration:
/// `P2-R2`'s third row, so `OBSERVED`.
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

/// The same path and the same call, inside a differently named declaration.
///
/// `warm` becomes `heat`, so `P2-R2`'s symbol fingerprint — a digest of path,
/// symbol kind and name — differs while the path does not. That is what
/// separates the two branches of `AuthoredWork::touches`.
const OBSERVED_REDIS_RENAMED: [(&str, &str); 4] = [
    (
        "package.json",
        "{
  \"name\": \"orders\",
  \"dependencies\": {
    \"redis\": \"4.6.0\"
  }
}
",
    ),
    (
        "src/cache.ts",
        "import redis from \"redis\";

function heat() {
  redis.createClient();
  return redis.connect();
}

export function handle() {
  return heat();
}
",
    ),
    (
        "docker-compose.yml",
        "services:
  cache:
    image: redis
",
    ),
    SPEC_PAGE,
];

// ---------------------------------------------------------------------------
// The design document, which is this suite's oracle for two lists.
// ---------------------------------------------------------------------------

fn design_page() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// Section 17.6's own bullets, read out of the design document.
///
/// Not a list written here: the section is located by its own heading and the
/// bullet block after `다음을 별도로 확인한다` is taken whole.
fn section_17_6_bullets() -> Result<Vec<String>, Box<dyn Error>> {
    let page = design_page()?;
    let start = page
        .find("### 17.6")
        .ok_or("section 17.6 is not in the design document")?;
    let section = &page[start..];
    let end = section
        .find("\n## ")
        .ok_or("section 17.6 does not end before the next section")?;
    let block = &section[..end];
    let after = block
        .find("다음을 별도로 확인한다")
        .ok_or("section 17.6 does not introduce its checks")?;
    Ok(block[after..]
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::to_owned)
        .collect())
}

/// Section 13.2's `자동 상한` table, as `evidence → ceiling` pairs.
fn section_13_2_ceilings() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let page = design_page()?;
    let start = page
        .find("### 13.2")
        .ok_or("section 13.2 is not in the design document")?;
    let section = &page[start..];
    let end = section
        .find("### 13.3")
        .ok_or("section 13.2 does not end")?;
    Ok(section[..end]
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
// 1. `repo_use_alone_creates_no_personal_claim`
// ---------------------------------------------------------------------------

/// Section 17.6's first sentence and section 13.2's own ceiling for it.
///
/// Three halves, and the third is the one that is not a restatement: the design
/// document's own row for `dependency/install/import만 존재` is read back and
/// required to say `mastery 승격 없음`, so the claim this test makes is the
/// specification's claim rather than this file's.
#[test]
fn repo_use_alone_creates_no_personal_claim() -> TestResult {
    let ceilings = section_13_2_ceilings()?;
    let installed_only = ceilings
        .iter()
        .find(|(evidence, _)| evidence.contains("dependency/install/import만 존재"))
        .ok_or("section 13.2 has no row for a dependency that is only present")?;
    assert_eq!(
        installed_only.1, "mastery 승격 없음",
        "section 13.2's ceiling for a present-only dependency has changed"
    );

    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;

    // The classification is not empty: this corpus really does observe.
    assert!(
        classification
            .stances()
            .iter()
            .any(|stance| stance.observed().is_some()),
        "the corpus produced no observation, so the rest of this test would be vacuous"
    );

    let set = promoted(&classification, &[], &[])?;
    assert!(
        !set.project_claims().is_empty(),
        "the snapshot observes a concept and no project claim was published"
    );
    assert!(
        set.personal_claims().is_empty(),
        "a repository observation alone produced a personal claim"
    );
    assert!(!observation_alone_promotes());

    // And the project claim says nothing about the user: it has no field that
    // could. The whole-set half of that is in the scans suite; here it is the
    // predicate the two claims are read under.
    let project = set
        .project_claim("redis")
        .ok_or("the redis observation published no project claim")?;
    assert_eq!(project.predicate(), "OBSERVES");
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `other_author_commit_is_ineligible`
// ---------------------------------------------------------------------------

/// Section 17.6's first bullet, and the three shapes a string comparison would
/// have let through.
///
/// The colleague's address is the control. The other three spell no forbidden
/// name and are each a way an identity could be *nearly* the user's:
///
/// * the same address in another namespace;
/// * the same address differing only in ASCII case; and
/// * a display name that reads like the user's while the address is not.
#[test]
fn other_author_commit_is_ineligible() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;

    let ineligible = [
        (
            "another person entirely",
            ExternalAuthorId::new(IdentitySource::GitAuthorEmail, OTHER_ADDRESS)?,
        ),
        (
            "the user's own address in another namespace",
            ExternalAuthorId::new(IdentitySource::ForgeLogin, OWN_ADDRESS)?,
        ),
        (
            "the user's own address in another case",
            ExternalAuthorId::new(IdentitySource::GitAuthorEmail, "Owner@Example.Test")?,
        ),
        (
            "a display name that reads like the user",
            ExternalAuthorId::new(IdentitySource::GitAuthorEmail, "user-1 <someone@else.test>")?,
        ),
    ];
    for (what, author) in ineligible {
        let record = report(
            &corpus,
            "c-other",
            author,
            ContributionKind::Authored,
            OriginReport::HandWritten,
            meaningful_sites(&corpus)?,
        )?;
        let refused = ContributionDraft::over(&record, &map, &rubric).seal();
        assert!(
            matches!(refused, Err(CompetencyError::AuthorIsNotTheUser { .. })),
            "{what} was admitted as the user: {refused:?}"
        );
    }

    // The control: the recorded identity, unchanged, seals.
    let work = own_work(&corpus)?;
    assert_eq!(work.user(), &user()?);

    // And a run over the colleague's change alone publishes the project claim
    // and no personal one.
    let set = promoted(&classification, &[], &[])?;
    assert!(!set.project_claims().is_empty());
    assert!(set.personal_claims().is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `scaffold_only_change_is_ineligible`
// ---------------------------------------------------------------------------

/// Section 17.6's second bullet, under a rubric that is configuration.
///
/// The verdict names the rubric and the version that produced it, and a second
/// rubric version disagreeing about the same change is admitted rather than
/// being a contradiction — which is what *versioned configuration* means.
#[test]
fn scaffold_only_change_is_ineligible() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;

    let scaffold = report(
        &corpus,
        "c-bump",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        scaffold_sites(&corpus)?,
    )?;
    let refused = ContributionDraft::over(&scaffold, &map, &rubric).seal();
    match &refused {
        Err(CompetencyError::ChangeIsScaffoldOnly {
            rubric: named,
            version,
            bearing_sites,
            required,
            ..
        }) => {
            assert_eq!(named, "scaffold-v2");
            assert_eq!(*version, 2);
            assert_eq!(*bearing_sites, 0);
            assert_eq!(*required, 1);
        }
        other => return Err(format!("a pin bump and a compose edit sealed: {other:?}").into()),
    }

    // The rubric is what decided, not this crate: a version that does not call
    // a configuration edit scaffold admits the same change.
    let permissive = ScaffoldRubric::of(
        RubricId::new("scaffold-v2")?,
        3,
        vec![ChangeKind::DependencyPin],
        vec![PathClass::Vendored, PathClass::Generated],
        1,
    )?;
    let admitted = ContributionDraft::over(&scaffold, &map, &permissive).seal()?;
    assert_eq!(admitted.verdict().version(), 3);
    assert_eq!(admitted.verdict().as_str(), "MEANINGFUL");

    // And the two answers are recorded rather than reconciled: the claim the
    // permissive rubric produces says which version admitted it.
    let set = promoted(&classification, &[admitted], &[])?;
    let personal = set
        .personal_claim("redis")
        .ok_or("the permissive rubric produced no personal claim")?;
    assert_eq!(personal.provenance().rubric().as_str(), "scaffold-v2");
    assert_eq!(personal.provenance().rubric_version(), 3);

    // The strict rubric, over the same corpus, publishes the project claim and
    // no personal one.
    let strict = promoted(&classification, &[], &[])?;
    assert!(!strict.project_claims().is_empty());
    assert!(strict.personal_claims().is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `outcome_artifact_strengthens_candidate`
// ---------------------------------------------------------------------------

/// Section 17.6's third bullet: an outcome raises what is carrying a candidate
/// and never produces one.
///
/// Both directions. The rising half walks all four [`OutcomeKind`]s; the
/// creating half offers every one of them with **no** authorship at all and
/// requires zero personal claims each time.
#[test]
fn outcome_artifact_strengthens_candidate() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let work = own_work(&corpus)?;
    let site = corpus.site("src/cache.ts")?;

    // The floor: the user's own meaningful change with nothing beside it is
    // already a candidate.
    let bare = promoted(&classification, std::slice::from_ref(&work), &[])?;
    let candidate = bare
        .personal_claim("redis")
        .ok_or("a meaningful authored change produced no candidate")?;
    assert_eq!(candidate.support(), CandidateSupport::AuthorshipOnly);
    assert!(candidate.provenance().outcomes().is_empty());

    // Each kind raises it, and `DEBUGGING` raises it further, which is section
    // 13.2's own two rows.
    let mut reached: BTreeMap<&'static str, CandidateSupport> = BTreeMap::new();
    for kind in OutcomeKind::ALL {
        let artifact = OutcomeArtifact::new(kind, "redis", change("c-own")?, site.clone(), NOW)?;
        let set = promoted(&classification, std::slice::from_ref(&work), &[artifact])?;
        let claim = set
            .personal_claim("redis")
            .ok_or("an outcome removed the candidate it was meant to strengthen")?;
        assert!(
            claim.support() > CandidateSupport::AuthorshipOnly,
            "{} did not strengthen the candidate",
            kind.as_str()
        );
        assert_eq!(claim.provenance().outcomes(), &[kind]);
        reached.insert(kind.as_str(), claim.support());
    }
    assert_eq!(
        reached.get("DEBUGGING"),
        Some(&CandidateSupport::DiagnosedFailure),
        "section 13.2 gives incident debugging its own ceiling"
    );
    assert_eq!(reached.get("TEST"), Some(&CandidateSupport::CodeAndOutcome));

    // The other direction: outcomes with no authorship create nothing. All
    // four, each on its own and then all four together.
    let mut every = Vec::new();
    for kind in OutcomeKind::ALL {
        let artifact = OutcomeArtifact::new(kind, "redis", change("c-own")?, site.clone(), NOW)?;
        let alone = promoted(&classification, &[], std::slice::from_ref(&artifact))?;
        assert!(
            alone.personal_claims().is_empty(),
            "{} created a personal claim on its own",
            kind.as_str()
        );
        assert!(!alone.project_claims().is_empty());
        every.push(artifact);
    }
    let together = promoted(&classification, &[], &every)?;
    assert!(
        together.personal_claims().is_empty(),
        "four outcomes with no authorship created a personal claim"
    );

    // And an outcome about another change is not counted toward this one.
    let elsewhere = OutcomeArtifact::new(
        OutcomeKind::Test,
        "redis",
        change("c-elsewhere")?,
        site,
        NOW,
    )?;
    let unmoved = promoted(&classification, &[work], &[elsewhere])?;
    assert_eq!(
        unmoved
            .personal_claim("redis")
            .ok_or("the candidate disappeared")?
            .support(),
        CandidateSupport::AuthorshipOnly
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `review_is_never_serialized_as_authored`
// ---------------------------------------------------------------------------

/// Section 17.6's fourth bullet, held by a value that does not exist.
///
/// The compiled half is `crates/repository-competency/tests/compile_fail/`.
/// What is here is the whole-set half: every [`ContributionKind`] a connector
/// can report is sealed, and the set that produces an [`AuthorshipMode`] is
/// compared against the set that produces a claim, in both directions. Then the
/// vocabulary a claim serializes its authorship into is compared against
/// [`OutcomeKind`]'s, and the two must not meet.
#[test]
fn review_is_never_serialized_as_authored() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;

    let mut sealed: BTreeSet<&'static str> = BTreeSet::new();
    let mut refused: BTreeSet<&'static str> = BTreeSet::new();
    let mut serialized: BTreeSet<&'static str> = BTreeSet::new();
    for kind in ContributionKind::ALL {
        let record = report(
            &corpus,
            "c-kind",
            own_identity()?,
            kind,
            OriginReport::HandWritten,
            meaningful_sites(&corpus)?,
        )?;
        match ContributionDraft::over(&record, &map, &rubric).seal() {
            Ok(work) => {
                sealed.insert(kind.as_str());
                let set = promoted(&classification, &[work], &[])?;
                let claim = set
                    .personal_claim("redis")
                    .ok_or("a sealed work produced no claim")?;
                serialized.insert(claim.provenance().mode().as_str());
            }
            Err(CompetencyError::ContributionIsNotAuthorship { kind: named, .. }) => {
                assert_eq!(named, kind);
                refused.insert(kind.as_str());
            }
            Err(other) => {
                return Err(format!(
                    "{} was refused for the wrong reason: {other:?}",
                    kind.as_str()
                )
                .into());
            }
        }
    }

    // Both sets are the whole enumeration between them, and neither is empty.
    assert_eq!(
        sealed.union(&refused).count(),
        ContributionKind::ALL.len(),
        "a contribution kind neither sealed nor refused"
    );
    assert!(sealed.is_disjoint(&refused));
    assert_eq!(refused, BTreeSet::from(["REVIEWED", "READ"]));
    assert_eq!(sealed, BTreeSet::from(["AUTHORED", "MODIFIED"]));

    // What a claim can serialize is exactly what `AuthorshipMode` enumerates,
    // and it does not reach the outcome vocabulary.
    let modes: BTreeSet<&'static str> = AuthorshipMode::ALL.iter().map(|m| m.as_str()).collect();
    assert_eq!(serialized, modes);
    let outcomes: BTreeSet<&'static str> = OutcomeKind::ALL.iter().map(|k| k.as_str()).collect();
    assert!(
        modes.is_disjoint(&outcomes),
        "an authorship spelling is also an outcome spelling"
    );
    assert!(outcomes.contains("REVIEW"));

    // The one door from a kind to a mode agrees with what sealing did, over the
    // whole enumeration, so a kind cannot be admitted through one and refused
    // through the other.
    for kind in ContributionKind::ALL {
        assert_eq!(
            kind.authorship_mode().is_some(),
            sealed.contains(kind.as_str()),
            "{} disagrees between authorship_mode and seal",
            kind.as_str()
        );
    }

    // A review beside real authorship is an outcome and nothing else: it is in
    // the outcome list, and the authorship field still reads `AUTHORED`.
    let work = own_work(&corpus)?;
    let review = OutcomeArtifact::new(
        OutcomeKind::Review,
        "redis",
        change("c-own")?,
        corpus.site("src/cache.ts")?,
        NOW,
    )?;
    let set = promoted(&classification, &[work], &[review])?;
    let claim = set
        .personal_claim("redis")
        .ok_or("the candidate disappeared")?;
    assert_eq!(claim.provenance().mode(), AuthorshipMode::Authored);
    assert_eq!(claim.provenance().outcomes(), &[OutcomeKind::Review]);
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `unmodified_generated_code_creates_no_applied_claim`
// ---------------------------------------------------------------------------

/// Section 17.6's fifth bullet.
///
/// The runtime half is here: generated code with no warrant is refused, and a
/// warrant with all three steps admits. The half that says a *partial* warrant
/// has no representation is the compile-fail suite, one case per step.
#[test]
fn unmodified_generated_code_creates_no_applied_claim() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;
    let site = corpus.site("src/cache.ts")?;

    let generated = report(
        &corpus,
        "c-generated",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::Generated,
        meaningful_sites(&corpus)?,
    )?;
    let refused = ContributionDraft::over(&generated, &map, &rubric).seal();
    assert!(
        matches!(
            refused,
            Err(CompetencyError::GeneratedCodeHasNoWarrant { .. })
        ),
        "unmodified generated code sealed: {refused:?}"
    );
    let bare = promoted(&classification, &[], &[])?;
    assert!(bare.personal_claims().is_empty());
    assert!(!bare.project_claims().is_empty());

    // The same report is hand-written: the origin is what was refused, not the
    // change. Without this the test would pass for the wrong reason.
    let hand = report(
        &corpus,
        "c-generated",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        meaningful_sites(&corpus)?,
    )?;
    let sealed = ContributionDraft::over(&hand, &map, &rubric).seal()?;
    assert_eq!(sealed.origin(), &CodeOrigin::HandWritten);

    // With all three steps it admits, and the claim carries what the user did.
    let verified = VerifiedByUser::at(vec![site.clone()], "ran the reconnect path under a fault")?;
    let modified = ModifiedByUser::after(
        verified,
        vec![ChangedSite::new(site, ChangeKind::ErrorHandling)],
    )?;
    let explained = ExplainedByUser::after(
        modified,
        "the generated client retried forever; the retry is now bounded and logged",
    )?;
    let warrant = GeneratedCodeWarrant::sealed(explained);
    let admitted = ContributionDraft::over(&generated, &map, &rubric)
        .warranted_by(warrant)
        .seal()?;
    let set = promoted(&classification, &[admitted], &[])?;
    let claim = set
        .personal_claim("redis")
        .ok_or("warranted generated code produced no claim")?;
    let origin = claim.provenance().origin();
    assert_eq!(origin.as_str(), "GENERATED");
    let carried = origin
        .warrant()
        .ok_or("a generated origin carries no warrant")?;
    assert_eq!(carried.modified().edits().len(), 1);
    assert!(!carried.verified().sites().is_empty());
    assert!(carried.explained().explanation().contains("bounded"));

    // Each of the three steps refuses an empty offering on its own, so none of
    // them is a field that can be filled in with nothing.
    assert!(VerifiedByUser::at(Vec::new(), "checked").is_err());
    assert!(VerifiedByUser::at(vec![corpus.site("src/cache.ts")?], "  ").is_err());
    let ok = VerifiedByUser::at(vec![corpus.site("src/cache.ts")?], "checked")?;
    assert!(ModifiedByUser::after(ok.clone(), Vec::new()).is_err());
    let changed = ModifiedByUser::after(
        ok,
        vec![ChangedSite::new(
            corpus.site("src/cache.ts")?,
            ChangeKind::ControlFlow,
        )],
    )?;
    assert!(ExplainedByUser::after(changed, "\n\t ").is_err());
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `two_claims_have_independent_ids_and_provenance`
// ---------------------------------------------------------------------------

/// Section 17.6's last sentence, and the identity trap `P2-R4` fell into.
///
/// `P2-R4`'s materialized requirement was four facts joined and truncated to 64
/// bytes, so two requirements differing only in the last fact shared an
/// identity. Both identities here are domain-separated digests, and the three
/// things that follow are measured rather than asserted in prose.
#[test]
fn two_claims_have_independent_ids_and_provenance() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let work = own_work(&corpus)?;
    let set = promoted(&classification, std::slice::from_ref(&work), &[])?;

    let project = set
        .project_claim("redis")
        .ok_or("no project claim")?
        .clone();
    let personal = set.personal_claim("redis").ok_or("no personal claim")?;

    // They are about the same subject and they are two claims.
    assert_eq!(project.key(), personal.key());
    assert_ne!(project.predicate(), personal.predicate());
    assert_ne!(
        project.id(),
        personal.id(),
        "one subject produced one identity for two claims"
    );

    // Neither identity is a prefix of the other, and neither is derivable from
    // the parts the other exposes: a joined-and-truncated identity would make
    // both start with the snapshot identifier.
    assert!(!project.id().as_str().starts_with(corpus.snapshot_id()));
    assert!(!personal.id().as_str().starts_with(corpus.snapshot_id()));
    assert_eq!(project.id().as_str().len(), 64);
    assert_eq!(personal.id().as_str().len(), 64);

    // The truncation collision, measured: the fact bound **last** is the one a
    // 64-byte join would drop. Two runs differing only in the change — the last
    // fact a personal identity binds — must be two identities, and the snapshot
    // identifier alone is already 43 bytes of the 64.
    assert!(
        corpus.snapshot_id().len() > 32,
        "the snapshot identifier is too short for this corpus to exercise the trap"
    );
    let second = report(
        &corpus,
        "c-second",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        meaningful_sites(&corpus)?,
    )?;
    let map = mapping()?;
    let rubric = rubric()?;
    let other_work = ContributionDraft::over(&second, &map, &rubric).seal()?;
    let other = promoted(&classification, &[other_work], &[])?;
    let other_personal = other.personal_claim("redis").ok_or("no second claim")?;
    assert_ne!(
        personal.id(),
        other_personal.id(),
        "two changes differing only in the last bound fact share one identity"
    );
    // And the project claim is unchanged by that: it binds neither.
    assert_eq!(
        other.project_claim("redis").map(|c| c.id()),
        Some(project.id())
    );

    // Two provenance records, and no field of one is a field of the other. The
    // personal one names the project claim by identifier and does not hold it.
    assert_eq!(personal.provenance().observed_by(), project.id());
    assert_eq!(personal.provenance().user().as_str(), USER);
    assert_eq!(personal.provenance().mapping_version(), 3);
    assert_eq!(personal.provenance().rubric_version(), 2);
    assert_eq!(project.provenance().tier().as_str(), "OBSERVED");
    assert!(!project.provenance().locators().is_empty());

    // Rejecting the personal claim leaves the project claim as it was, byte for
    // byte, and the rejection is a new value rather than an edit.
    let before = project.clone();
    let rejected = personal
        .clone()
        .rejected(RejectionReason::NotTheUsersWork, 9_000)?;
    assert_eq!(rejected.id(), personal.id());
    assert!(matches!(
        rejected.standing(),
        ClaimStanding::Rejected {
            reason: RejectionReason::NotTheUsersWork,
            at: 9_000
        }
    ));
    assert_eq!(personal.standing(), &ClaimStanding::Candidate);
    assert_eq!(before, project);
    assert_eq!(
        set.project_claim("redis"),
        Some(&project),
        "rejecting the personal claim changed the published project claim"
    );

    // A second rejection is refused and names the claim.
    let again = rejected.rejected(RejectionReason::EvidenceWithdrawn, 9_100);
    assert!(matches!(
        again,
        Err(CompetencyError::ClaimAlreadyRejected(_))
    ));

    // The mastery level a candidate is offered at is section 13.1's `APPLIED`
    // and nothing above it.
    assert_eq!(personal.offered_at(), MasteryLevel::Applied);
    let _ = work;
    Ok(())
}

// ---------------------------------------------------------------------------
// Beside the seven.
// ---------------------------------------------------------------------------

/// [`PromotionCheck::ALL`] is section 17.6's own bullet list.
///
/// The number is a measurement of the design document, not a constant here.
#[test]
fn the_promotion_checks_are_section_17_6_s() -> TestResult {
    let bullets = section_17_6_bullets()?;
    assert_eq!(
        bullets.len(),
        PromotionCheck::ALL.len(),
        "section 17.6 lists {} checks and this crate enumerates {}: {bullets:?}",
        bullets.len(),
        PromotionCheck::ALL.len()
    );
    // Each bullet's own subject word appears in the check that stands for it,
    // in the section's order.
    let anchors = [
        "authorship",
        "scaffold",
        "evidence",
        "읽은 것인지",
        "생성형 AI",
    ];
    for (index, check) in PromotionCheck::ALL.iter().enumerate() {
        assert!(
            bullets[index].contains(anchors[index]),
            "section 17.6's bullet {index} is {:?}, which is not the one {} stands for",
            bullets[index],
            check.as_str()
        );
    }
    assert_eq!(
        PromotionCheck::ALL
            .iter()
            .filter(|check| check.blocks())
            .count(),
        4,
        "outcome evidence is the one check that grades rather than blocks"
    );
    Ok(())
}

/// Every one of the five changes what a run produces.
///
/// One corpus per check, failing only that check, so no entry can be registered
/// without biting. This is `P2-R4`'s `removing_any_chain_step_blocks_publish`
/// applied to section 17.6's list.
#[test]
fn each_of_section_17_6_s_checks_changes_the_outcome() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;
    let site = corpus.site("src/cache.ts")?;

    for check in PromotionCheck::ALL {
        let (author, kind, origin, sites) = match check {
            PromotionCheck::Authorship => (
                ExternalAuthorId::new(IdentitySource::GitAuthorEmail, OTHER_ADDRESS)?,
                ContributionKind::Authored,
                OriginReport::HandWritten,
                meaningful_sites(&corpus)?,
            ),
            PromotionCheck::MeaningfulChange => (
                own_identity()?,
                ContributionKind::Authored,
                OriginReport::HandWritten,
                scaffold_sites(&corpus)?,
            ),
            PromotionCheck::ReadVersusAuthored => (
                own_identity()?,
                ContributionKind::Reviewed,
                OriginReport::HandWritten,
                meaningful_sites(&corpus)?,
            ),
            PromotionCheck::GeneratedCodeWarrant => (
                own_identity()?,
                ContributionKind::Authored,
                OriginReport::Generated,
                meaningful_sites(&corpus)?,
            ),
            PromotionCheck::OutcomeEvidence => (
                own_identity()?,
                ContributionKind::Authored,
                OriginReport::HandWritten,
                meaningful_sites(&corpus)?,
            ),
        };
        let record = report(&corpus, "c-only", author, kind, origin, sites)?;
        let sealed = ContributionDraft::over(&record, &map, &rubric).seal();
        if check.blocks() {
            assert!(
                sealed.is_err(),
                "{} failed and the contribution still sealed",
                check.as_str()
            );
            continue;
        }
        // The grading check: the same contribution seals, and what changes is
        // the support the claim carries.
        let work = sealed?;
        let without = promoted(&classification, std::slice::from_ref(&work), &[])?;
        let artifact = OutcomeArtifact::new(
            OutcomeKind::Debugging,
            "redis",
            change("c-only")?,
            site.clone(),
            NOW,
        )?;
        let with = promoted(&classification, &[work], &[artifact])?;
        let weaker = without.personal_claim("redis").ok_or("no claim")?.support();
        let stronger = with.personal_claim("redis").ok_or("no claim")?.support();
        assert!(
            stronger > weaker,
            "{} is registered and changes nothing",
            check.as_str()
        );
    }
    Ok(())
}

/// A work that touched somewhere else promotes nothing.
///
/// Authoring anything in a repository that observes a concept is not authoring
/// that concept's use. The `express` corpus has no `redis` at all, so a work
/// over its sites meets none of the observation's locators.
#[test]
fn a_change_elsewhere_promotes_no_concept() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;

    // `docs/spec.md` is in the snapshot and is not a site the `redis`
    // observation names, so a meaningful change there touches nothing.
    let elsewhere = built(&[
        (
            "package.json",
            "{\n  \"name\": \"orders\",\n  \"dependencies\": {\n    \"redis\": \"4.6.0\"\n  }\n}\n",
        ),
        (
            "src/other.ts",
            "import redis from \"redis\";\n\nexport function other() {\n  return redis.createClient();\n}\n",
        ),
        (
            "docker-compose.yml",
            "services:\n  cache:\n    image: redis\n",
        ),
        SPEC_PAGE,
    ])?;
    let far = ChangedSite::new(elsewhere.site("src/other.ts")?, ChangeKind::ControlFlow);
    let record = ContributionRecord {
        change: change("c-far")?,
        snapshot_id: corpus.snapshot_id().to_owned(),
        author: own_identity()?,
        kind: ContributionKind::Authored,
        origin: OriginReport::HandWritten,
        sites: vec![far],
        recorded_at: 1_756_100_000_000,
    };
    let work = ContributionDraft::over(&record, &map, &rubric).seal()?;
    let set = promoted(&classification, &[work], &[])?;
    assert!(
        !set.project_claims().is_empty(),
        "the observation disappeared, so this test would be vacuous"
    );
    assert!(
        set.personal_claims().is_empty(),
        "a change at another place promoted the concept"
    );

    // The control: the same shape at the observed place does promote, so the
    // refusal above is about the place and not about the corpus.
    let near = own_work(&corpus)?;
    let promoted_here = promoted(&classification, &[near], &[])?;
    assert!(promoted_here.personal_claim("redis").is_some());
    Ok(())
}

/// A work read over another snapshot, or belonging to another user, is refused
/// rather than promoted against this run.
#[test]
fn a_work_is_bound_to_its_snapshot_and_its_user() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;

    let mut record = report(
        &corpus,
        "c-own",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        meaningful_sites(&corpus)?,
    )?;
    record.snapshot_id = "snap_repoA_other_20260101".to_owned();
    let elsewhere = ContributionDraft::over(&record, &map, &rubric).seal()?;
    let user = user()?;
    let refused = promote(&PromotionInput {
        classification: &classification,
        user: &user,
        works: &[elsewhere],
        outcomes: &[],
    });
    assert!(matches!(
        refused,
        Err(CompetencyError::WorkIsAboutAnotherSnapshot(_, _))
    ));

    let second = UserId::new("user-2")?;
    let refused = promote(&PromotionInput {
        classification: &classification,
        user: &second,
        works: &[own_work(&corpus)?],
        outcomes: &[],
    });
    assert!(matches!(
        refused,
        Err(CompetencyError::WorkBelongsToAnotherUser(_, _, _))
    ));
    Ok(())
}

/// A rubric that counts no site is refused rather than admitting everything.
#[test]
fn a_rubric_that_requires_nothing_is_not_a_rubric() -> TestResult {
    let refused = ScaffoldRubric::of(RubricId::new("empty")?, 1, Vec::new(), Vec::new(), 0);
    assert!(matches!(
        refused,
        Err(CompetencyError::RubricAdmitsNothing(_, 1))
    ));
    Ok(())
}

/// The work-to-observation join reads `P2-R2`'s fingerprint when both sides
/// have one, and the path only when they do not.
///
/// Both branches, and the second half is what makes the claim measurable: a
/// changed site at the **same path** inside a **different declaration** does not
/// meet an observation, which a path comparison would have admitted. Without
/// this the fingerprint branch would never run in this suite, because `site`
/// returns `src/cache.ts`'s module-level import, which carries no symbol.
#[test]
fn a_work_meets_an_observation_by_fingerprint_before_by_path() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;
    let map = mapping()?;
    let rubric = rubric()?;

    // The corpus really does carry both shapes, so neither half is vacuous.
    let inside = corpus.symbol_site("src/cache.ts")?;
    let outside = corpus.site("src/cache.ts")?;
    assert!(inside.symbol().is_some());
    assert!(outside.symbol().is_none());

    // A work inside the declaration the observation names meets it.
    let record = ContributionRecord {
        change: change("c-inside")?,
        snapshot_id: corpus.snapshot_id().to_owned(),
        author: own_identity()?,
        kind: ContributionKind::Authored,
        origin: OriginReport::HandWritten,
        sites: vec![ChangedSite::new(inside.clone(), ChangeKind::ControlFlow)],
        recorded_at: 1_756_100_000_000,
    };
    let work = ContributionDraft::over(&record, &map, &rubric).seal()?;
    let set = promoted(&classification, std::slice::from_ref(&work), &[])?;
    assert!(
        set.personal_claim("redis").is_some(),
        "a change inside the declaration the observation names did not meet it"
    );

    // A work at the same path inside a *different* declaration does not.
    let renamed = built(&OBSERVED_REDIS_RENAMED)?;
    let elsewhere = renamed.symbol_site("src/cache.ts")?;
    assert_eq!(
        elsewhere.path(),
        inside.path(),
        "the two paths must be equal"
    );
    assert_ne!(
        elsewhere.symbol(),
        inside.symbol(),
        "the two declarations must have different fingerprints"
    );
    let record = ContributionRecord {
        change: change("c-renamed")?,
        snapshot_id: corpus.snapshot_id().to_owned(),
        author: own_identity()?,
        kind: ContributionKind::Authored,
        origin: OriginReport::HandWritten,
        sites: vec![ChangedSite::new(elsewhere, ChangeKind::ControlFlow)],
        recorded_at: 1_756_100_000_000,
    };
    let other = ContributionDraft::over(&record, &map, &rubric).seal()?;
    let set = promoted(&classification, std::slice::from_ref(&other), &[])?;
    assert!(
        !set.project_claims().is_empty(),
        "the observation disappeared, so this half would be vacuous"
    );
    assert!(
        set.personal_claim("redis").is_none(),
        "a change in another declaration at the same path met the observation"
    );
    Ok(())
}

/// Every identifier this crate takes is the shape it says it admits.
///
/// A whole-set classification rather than a list of rejected spellings: every
/// ASCII byte is offered inside an otherwise legal identifier and required to
/// be admitted **exactly** when this test's own independent predicate says it
/// belongs, in both directions, for all four constructors.
///
/// It is here because `P2-Y1`'s injection campaign measured the gap: reducing
/// `identity::validated` to a non-empty check — keeping the `matches!` macro, so
/// `competency_scans.rs`'s macro inventory is unchanged — passed this crate's
/// whole suite, so the rule was declared and unmeasured.
#[test]
fn every_identifier_is_the_shape_this_crate_admits() -> TestResult {
    // Written here rather than read from the crate, so the two are independent.
    let belongs =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-';

    for byte in 0_u8..=127 {
        let candidate = format!("a{}b", char::from(byte));
        for taken in [
            UserId::new(candidate.clone()).is_ok(),
            ChangeId::new(candidate.clone()).is_ok(),
            RubricId::new(candidate.clone()).is_ok(),
            OutcomeArtifact::new(
                OutcomeKind::Test,
                candidate.clone(),
                change("c-shape")?,
                shape_site()?,
                1,
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
                UserId::new(outside),
                Err(CompetencyError::InvalidIdentifier("user", _))
            ),
            "{outside:?} was admitted as a user identity"
        );
    }

    // The length boundary, on both sides of it, and the empty value.
    let longest = "a".repeat(64);
    assert!(UserId::new(longest.as_str()).is_ok());
    assert!(ChangeId::new(longest.as_str()).is_ok());
    assert!(RubricId::new(longest.as_str()).is_ok());
    let overlong = "a".repeat(65);
    for outcome in [
        UserId::new(overlong.as_str()).err(),
        ChangeId::new(overlong.as_str()).err(),
        RubricId::new(overlong.as_str()).err(),
    ] {
        assert!(
            matches!(outcome, Some(CompetencyError::InvalidIdentifier(_, _))),
            "a 65-byte identifier was admitted"
        );
    }

    // Each constructor names itself in its refusal, so a reader is told which
    // identifier was wrong rather than that one of four was.
    let named: BTreeSet<&'static str> = [
        UserId::new("").err(),
        ChangeId::new("").err(),
        RubricId::new("").err(),
        OutcomeArtifact::new(OutcomeKind::Test, "", change("c-shape")?, shape_site()?, 1).err(),
    ]
    .into_iter()
    .filter_map(|error| match error {
        Some(CompetencyError::InvalidIdentifier(what, _)) => Some(what),
        _ => None,
    })
    .collect();
    assert_eq!(
        named,
        BTreeSet::from(["change", "concept", "rubric", "user"]),
        "the four constructors do not name themselves apart"
    );
    Ok(())
}

/// One `P2-R2` locator, so the outcome constructor above has a real place to
/// name. A locator's own constructor is crate-private to that crate.
fn shape_site() -> Result<Locator, Box<dyn Error>> {
    built(&OBSERVED_REDIS)?.site("src/cache.ts")
}

/// `OBSERVED_REDIS` with a vendored copy of the cache module beside it.
///
/// The vendored site is recorded on the finding and never counted, which makes
/// it the only route in this suite to a `P2-R2` locator whose path class is
/// `VENDORED` — and therefore the only way to drive the path-class half of
/// `ScaffoldRubric::bears_understanding`.
const OBSERVED_REDIS_WITH_VENDOR: [(&str, &str); 5] = [
    OBSERVED_REDIS[0],
    OBSERVED_REDIS[1],
    OBSERVED_REDIS[2],
    OBSERVED_REDIS[3],
    (
        "vendor/upstream/cache.ts",
        "import redis from \"redis\";\n\nfunction warm() {\n  return redis.createClient();\n}\n\nexport function handle() {\n  return warm();\n}\n",
    ),
];

#[test]
fn a_control_flow_edit_inside_vendored_source_bears_no_understanding() -> TestResult {
    // `P2-A5` measured this half of `bears_understanding` undriven: deleting
    // `!self.scaffold_path_classes.contains(...)` left the whole crate green.
    // `rubric.rs` says "a `CONTROL_FLOW` edit inside vendored source is somebody
    // else's control flow", and until now nothing said it back.
    let corpus = built(&OBSERVED_REDIS_WITH_VENDOR)?;
    let vendored = corpus.excluded_site("vendor/upstream/cache.ts")?;
    assert_eq!(vendored.class(), PathClass::Vendored);
    let rubric = rubric()?;

    // The change kind is one the rubric counts, so the kind half says yes and
    // only the path-class half is left to refuse.
    assert!(
        !rubric
            .scaffold_change_kinds()
            .contains(&ChangeKind::ControlFlow),
        "the kind half already refuses, so this fixture would not reach the path half"
    );
    assert!(
        rubric
            .scaffold_path_classes()
            .contains(&PathClass::Vendored)
    );
    let site = ChangedSite::new(vendored, ChangeKind::ControlFlow);
    assert!(!rubric.bears_understanding(&site));
    assert!(matches!(
        rubric.judge(std::slice::from_ref(&site)),
        ChangeVerdict::ScaffoldOnly { .. }
    ));

    // And through the one producer of an `AuthoredWork`, so what is refused is
    // the path a claim would actually take.
    let record = report(
        &corpus,
        "c-vendored",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        vec![site],
    )?;
    assert!(matches!(
        ContributionDraft::over(&record, &mapping()?, &rubric).seal(),
        Err(CompetencyError::ChangeIsScaffoldOnly { .. })
    ));

    // The control: the same change kind at a first-party path is meaningful and
    // seals, so what refused above is the path class and not the kind.
    let first_party = ChangedSite::new(corpus.site("src/cache.ts")?, ChangeKind::ControlFlow);
    assert!(rubric.bears_understanding(&first_party));
    let sealed = report(
        &corpus,
        "c-first-party",
        own_identity()?,
        ContributionKind::Authored,
        OriginReport::HandWritten,
        vec![first_party],
    )?;
    assert!(
        ContributionDraft::over(&sealed, &mapping()?, &rubric)
            .seal()
            .is_ok()
    );
    Ok(())
}

#[test]
fn two_works_touching_one_observation_are_refused_rather_than_chosen_between() -> TestResult {
    // `P2-A5` measured this refusal undriven: deleting `touching.next()
    // .is_some()` left the whole crate green, and the crate then silently
    // promoted whichever work came first in iteration order. `lib.rs` says
    // "picking between two pieces of evidence about the same subject is a
    // judgement, and this crate does not make it", and until now nothing said
    // it back.
    let corpus = built(&OBSERVED_REDIS)?;
    let classification = classified(&corpus, &order_goal()?)?;

    let one = own_work(&corpus)?;
    let two = ContributionDraft::over(
        &report(
            &corpus,
            "c-second",
            own_identity()?,
            ContributionKind::Authored,
            OriginReport::HandWritten,
            meaningful_sites(&corpus)?,
        )?,
        &mapping()?,
        &rubric()?,
    )
    .seal()?;
    assert_ne!(
        one.change(),
        two.change(),
        "the two works are one work, so this fixture proves nothing"
    );

    // Each on its own promotes, so what refuses below is the pair and not
    // either work.
    for single in [&one, &two] {
        let set = promoted(&classification, std::slice::from_ref(single), &[])?;
        assert!(
            set.personal_claim("redis").is_some(),
            "one work alone did not promote, so the pair proves nothing"
        );
    }

    let refused = promote(&PromotionInput {
        classification: &classification,
        user: &user()?,
        works: &[one, two],
        outcomes: &[],
    });
    assert!(
        matches!(
            refused,
            Err(CompetencyError::DuplicatePromotion(ref concept, ref goal, version))
                if concept == "redis" && goal == "no-duplicate-retryable-orders" && version == 1
        ),
        "two works touching one observation were not refused: {refused:?}"
    );
    Ok(())
}

/// The length bound on an external identity is measured, not merely present.
///
/// `P2-A5`'s F8 swept every refusal site of this crate and found one that no
/// test drives: deleting `ExternalAuthorId::new`'s only `return Err` outright
/// left the crate at 26 passed, 0 failed. Its own `# Errors` section names the
/// refusal and the scan documentation rests on it -- "bounded in length and
/// **not** put through `validated` ... so a mapping cannot be made to hold an
/// arbitrarily large blob under an identity's name" -- and nothing measured
/// the bound. Both sides of it, and the empty value, are measured here.
#[test]
fn an_external_identity_is_bounded_in_length_and_not_in_shape() -> TestResult {
    // 320 is the longest address RFC 5321 admits, and the doc comment says so.
    for (length, admitted) in [(1_usize, true), (320, true), (321, false), (0, false)] {
        let value = "a".repeat(length);
        // Every namespace, from the crate's own exhaustive order, so a fourth
        // one added with its own bound is measured here rather than assumed.
        for source in IdentitySource::ALL {
            let outcome = ExternalAuthorId::new(source, value.as_str());
            assert_eq!(
                outcome.is_ok(),
                admitted,
                "an external identity of {length} bytes in {} was admitted {} and should be \
                 {admitted}",
                source.as_str(),
                outcome.is_ok()
            );
        }
    }

    // The other half of the sentence: the value is *not* put through the
    // identifier shape, so an address and a display name survive. A test that
    // only drove the length would pass if somebody added `validated` here and
    // dropped every identity this type exists to carry.
    for value in [
        "ada@example.invalid",
        "Ada Lovelace",
        "ada+notes@example.invalid",
    ] {
        assert!(
            ExternalAuthorId::new(IdentitySource::GitAuthorEmail, value).is_ok(),
            "{value:?} was refused by a type that is documented not to check shape"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The refusal `P2-A5`'s sixth audit found nothing executes.
// ---------------------------------------------------------------------------

/// `generated.rs:172`, reached.
///
/// The sixth audit instrumented all 93 `return Err(` sites in the six `P2-R`
/// crates and ran the whole workspace four times: 63 fired and **30 did not**.
/// One of the thirty is in this crate, and it is the third step of the
/// generated-code warrant: the note the user writes when they explain what
/// they changed. The suite built the whole three-step warrant and never built
/// it with the last step empty, so the rule that a warrant carries an
/// explanation was itself unmeasured.
///
/// Driven on both sides of `trim`, because an explanation of spaces is the
/// form a caller actually produces.
#[test]
fn a_generated_code_warrant_needs_an_explanation() -> TestResult {
    let corpus = built(&OBSERVED_REDIS)?;
    let site = corpus.site("src/cache.ts")?;
    let verified = VerifiedByUser::at(vec![site.clone()], "ran the reconnect path under a fault")?;
    let modified = ModifiedByUser::after(
        verified,
        vec![ChangedSite::new(site, ChangeKind::ErrorHandling)],
    )?;

    for empty in [
        "", "   ", "	
 ",
    ] {
        assert!(
            matches!(
                ExplainedByUser::after(modified.clone(), empty),
                Err(CompetencyError::WarrantStepHasNoNote(
                    WarrantStep::Explained
                ))
            ),
            "an explanation of {empty:?} was admitted"
        );
    }

    // The control: a written explanation is admitted, so the refusal is about
    // the note and not about the step.
    assert!(ExplainedByUser::after(modified, "the retry is now bounded").is_ok());
    Ok(())
}
