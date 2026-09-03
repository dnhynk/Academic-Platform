//! `P2-N2`'s thirteen named acceptance rows.
//!
//! Four of them read `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compare what is in this crate against what is in the document, in both
//! directions. Section 13.1's ladder, its facet block, section 13.2's eight
//! rows and section 13.4's four checks are therefore **measurements** rather
//! than counts restated in a test — which is the discipline `P2-R4` used for
//! section 18.2's step count and section 18's three classification names.

mod common;

use std::{error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ConfidencePermille, ContentDigest, EntityId,
    EpistemicStatus, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength,
    FreshnessBand, MasteryLevel, ModelRunId, PredicateId, ScopeId, TimestampMillis, ValidInterval,
    entity_registry::EntityKind,
};
use academic_knowledge_state::{
    AdjustmentDirection, AiProposal, AutomaticLevel, BroadSignal, CEILINGS, ConceptEvidence,
    ConceptLink, CourseGradeSignal, DependencyOnly, EligibilityCheck, EligibilityOutcome,
    EligibilityReasonCode, EligibleEvidence, EvidenceCeiling, EvidenceDossier, EvidenceKind,
    EvidenceRetraction, ExerciseOutcome, FacetProfile, FacetStrength, FluentAuthorization,
    FreshnessInput, HistoryEntry, IncidentRepair, KnowledgeStateAssertion, KnowledgeStateError,
    KnowledgeStateHistory, LADDER, MasteryFacet, Outcome, Participation, ProjectUse,
    ProposalOutcome, SelfExplanation, SourceIntegrity, SufficiencyGap, TeachingSite,
    TransferContext, TransferRepetition, UNSEEN_MEANING, UnseenBasis, UserConfirmation, level_token,
    project, rung,
};
use academic_lecture_document::{LectureDocument, NodeId};

type TestResult = Result<(), Box<dyn Error>>;

const NOW: u64 = 1_800_000;
const LATER: u64 = 1_900_000;

// ---------------------------------------------------------------------------
// Reading the design document.
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?)
}

/// The rows of the one markdown table that follows `heading`.
fn table_after(page: &str, heading: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let start = page
        .find(heading)
        .ok_or_else(|| format!("the design document has no {heading}"))?;
    let mut rows = Vec::new();
    let mut seen_header = false;
    for line in page[start..].lines().skip(1) {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if rows.is_empty() && !seen_header {
                continue;
            }
            break;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect();
        if !seen_header {
            seen_header = true;
            continue;
        }
        if cells.iter().all(|cell| cell.chars().all(|c| c == '-' || c == ':')) {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err(format!("no table rows after {heading}").into());
    }
    Ok(rows)
}

/// The lines of the one fenced block that follows `heading`.
fn block_after(page: &str, heading: &str, fence: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let start = page
        .find(heading)
        .ok_or_else(|| format!("the design document has no {heading}"))?;
    let opened = page[start..]
        .find(fence)
        .ok_or_else(|| format!("no {fence} block after {heading}"))?;
    let body_start = start + opened + fence.len();
    let closed = page[body_start..]
        .find("```")
        .ok_or_else(|| format!("unterminated {fence} block after {heading}"))?;
    Ok(lines_of(&page[body_start..body_start + closed]))
}

/// The lines of the one fenced block that **contains** `marker`.
///
/// Section 13.1's schema example is inside its own fence, so searching forward
/// from the marker would find the *next* block. This searches backwards for the
/// opening fence instead.
fn block_containing(page: &str, marker: &str, fence: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let at = page
        .find(marker)
        .ok_or_else(|| format!("the design document has no {marker}"))?;
    let opened = page[..at]
        .rfind(fence)
        .ok_or_else(|| format!("{marker} is not inside a {fence} block"))?;
    let body_start = opened + fence.len();
    let closed = page[body_start..]
        .find("```")
        .ok_or_else(|| format!("unterminated {fence} block around {marker}"))?;
    Ok(lines_of(&page[body_start..body_start + closed]))
}

fn lines_of(body: &str) -> Vec<String> {
    body.lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// The `facets:` sub-keys of section 13.1's schema example, in its own order.
fn designed_facets() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let page = specification()?;
    let block = block_containing(&page, "KnowledgeStateAssertion:", "```yaml")?;
    let rows: Vec<(String, String)> = block
        .iter()
        .skip_while(|line| line.trim() != "facets:")
        .skip(1)
        .take_while(|line| line.starts_with("    ") && !line.starts_with("     "))
        .filter_map(|line| {
            let (key, value) = line.trim().split_once(':')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect();
    if rows.is_empty() {
        return Err("section 13.1's schema example has no facets".into());
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Domain fixtures.
// ---------------------------------------------------------------------------

/// A deterministic UUIDv7-shaped identifier for one fixture name.
///
/// The bytes are a SHA-256 of the tag with the version and variant nibbles set,
/// so a fixture's identity is a function of its name rather than of a clock.
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

fn model_run_id(tag: &str) -> ModelRunId {
    ModelRunId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn scope() -> ScopeId {
    ScopeId::try_from_uuid(uuid_of("scope-knowledge-state"))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

fn user_actor() -> Actor {
    Actor::User {
        user_id: entity("the-one-user"),
    }
}

fn model_actor() -> Actor {
    Actor::ModelRun {
        run_id: entity("a-model-run"),
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

fn confirmation_claim(
    concept: EntityId,
    level: MasteryLevel,
    evidence: &EvidenceItem,
    authority: AuthorityClass,
    status: EpistemicStatus,
    predicate: &str,
) -> Result<Claim, Box<dyn Error>> {
    Ok(Claim {
        id: claim_id("claim"),
        subject_entity_id: concept,
        predicate_id: PredicateId::parse(predicate)?,
        object: ClaimObject::Mastery(level),
        scope_id: scope(),
        authority_class: authority,
        epistemic_status: status,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::new(TimestampMillis::new(0), None)?,
        evidence_ids: vec![evidence.id],
    })
}

fn confirmation(concept: EntityId, level: MasteryLevel) -> Result<UserConfirmation, Box<dyn Error>> {
    let evidence = evidence_item("confirmation");
    let claim = confirmation_claim(
        concept,
        level,
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        academic_knowledge_state::STATE_CONFIRMATION_PREDICATE,
    )?;
    Ok(UserConfirmation::verify(
        &user_actor(),
        &claim,
        &evidence,
        concept,
        level,
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

fn facets() -> FacetProfile {
    FacetProfile::of(
        FacetStrength::Strong,
        FacetStrength::Moderate,
        FacetStrength::Strong,
        FacetStrength::Strong,
        FacetStrength::LimitedEvidence,
    )
}

fn freshness() -> Result<FreshnessInput, Box<dyn Error>> {
    Ok(FreshnessInput::of(
        FreshnessBand::VeryHigh,
        ConfidencePermille::new(920)?,
    ))
}

fn admitted(
    evidence: ConceptEvidence,
    tag: &str,
    dossier: &EvidenceDossier,
) -> Result<EligibleEvidence, Box<dyn Error>> {
    match EligibilityOutcome::admit(evidence, evidence_id(tag), dossier) {
        EligibilityOutcome::Admitted(item) => Ok(item),
        EligibilityOutcome::Blocked(blocked) => {
            Err(format!("the fixture was blocked: {:?}", blocked.reasons()).into())
        }
    }
}

// ---------------------------------------------------------------------------
// Lecture and project evidence, built through `P2-L4` and `P2-R4`.
// ---------------------------------------------------------------------------

struct Lecture {
    _directory: tempfile::TempDir,
    document: LectureDocument,
}

fn lecture_document(tag: &str) -> Result<Lecture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let capture = common::lecture::clean_capture(&directory, tag)?;
    let manifest = common::lecture::full_manifest(&capture)?;
    let transcribed = common::lecture::transcribe(&manifest)?;
    let capture_seq = common::lecture::capture_frame_seq(&capture)
        .ok_or("the fixture capture holds no board photograph")?;
    let document = common::lecture::whole_document(transcribed.lineage(), &manifest, capture_seq)?;
    Ok(Lecture {
        _directory: directory,
        document,
    })
}

fn teaching(lecture: &Lecture) -> Result<TeachingSite, Box<dyn Error>> {
    let node: &NodeId = lecture
        .document
        .nodes()
        .first()
        .ok_or("the fixture document has no node")?
        .id();
    Ok(TeachingSite::in_document(&lecture.document, node)?)
}

fn observed_use() -> Result<ProjectUse, Box<dyn Error>> {
    let corpus = common::project::built(&common::project::OBSERVED_REDIS)?;
    let set = common::project::classified(&corpus, &common::project::goal("cache-warmup", 1)?)?;
    let stance = common::project::stance_of(&set)?;
    ProjectUse::of_stance(&stance).ok_or_else(|| "the observed corpus produced no use".into())
}

/// The stance of a concept the goal has an interest in and the snapshot merely
/// installs.
///
/// `P2-R4` publishes a stance for a concept the goal names or the snapshot
/// observes, so the benefit contract is what puts `redis` in the set at all;
/// what makes this the seventh row rather than the fourth is that the
/// classification carries **no** `ObservedProof`, which is `P2-R2`'s decision
/// and not this suite's.
fn installed_only() -> Result<DependencyOnly, Box<dyn Error>> {
    let corpus = common::project::built(&common::project::INSTALLED_ONLY)?;
    let set = common::project::classified_with(
        &corpus,
        &common::project::goal("cache-warmup", 1)?,
        std::slice::from_ref(&common::project::benefit_contract()?),
    )?;
    let stance = common::project::stance_of(&set)?;
    DependencyOnly::of_stance(&stance)
        .ok_or_else(|| "the manifest-only corpus produced an observation".into())
}

// ---------------------------------------------------------------------------
// 1. `mastery_enum_is_exactly_six_ordered`
// ---------------------------------------------------------------------------

#[test]
fn mastery_enum_is_exactly_six_ordered() -> TestResult {
    let page = specification()?;
    let rows = table_after(&page, "### 13.1 Mastery는 학습 깊이")?;

    // The design document's own rows, in its own order.
    let designed: Vec<(u8, String)> = rows
        .iter()
        .map(|cells| -> Result<(u8, String), Box<dyn Error>> {
            let level: u8 = cells
                .first()
                .ok_or("a 13.1 row has no level cell")?
                .parse()?;
            let name = cells
                .get(1)
                .ok_or("a 13.1 row has no name cell")?
                .trim_matches('`')
                .to_owned();
            Ok((level, name))
        })
        .collect::<Result<_, _>>()?;

    let held: Vec<(u8, String)> = LADDER
        .iter()
        .map(|level| (rung(*level), level_token(*level).to_owned()))
        .collect();

    // Both directions. A level in the document without one here, or one here
    // without a row there, is a failure rather than a count nobody rechecks.
    assert_eq!(designed, held, "section 13.1's ladder and LADDER disagree");
    assert_eq!(LADDER.len(), designed.len());

    // Ordered, and the order is strictly increasing under the domain enum's own
    // `Ord` as well as under the design document's `Level` column.
    for pair in LADDER.windows(2) {
        assert!(pair[0] < pair[1], "{pair:?} is not ordered");
        assert_eq!(rung(pair[1]), rung(pair[0]) + 1);
    }

    // The five facets, read out of the same section's YAML block.
    let keys: Vec<String> = designed_facets()?.into_iter().map(|(key, _)| key).collect();
    let facet_keys: Vec<String> = MasteryFacet::ALL
        .iter()
        .map(|facet| facet.key().to_owned())
        .collect();
    assert_eq!(keys, facet_keys, "section 13.1's facet keys disagree");
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `ks_applied_mixed_facets`
// ---------------------------------------------------------------------------

#[test]
fn ks_applied_mixed_facets() -> TestResult {
    // The design document's own example, key by key.
    let designed = designed_facets()?;

    let profile = FacetProfile::of(
        FacetStrength::Strong,
        FacetStrength::Moderate,
        FacetStrength::Strong,
        FacetStrength::Strong,
        FacetStrength::LimitedEvidence,
    );
    let held: Vec<(String, String)> = MasteryFacet::ALL
        .iter()
        .map(|facet| {
            (
                facet.key().to_owned(),
                profile.strength(*facet).as_str().to_owned(),
            )
        })
        .collect();
    assert_eq!(designed, held, "the 13.1 example and the profile disagree");

    // Every strength the design's example exhibits is one this crate has, and
    // every one this crate has appears in the example.
    let mut designed_strengths: Vec<&str> = designed.iter().map(|(_, v)| v.as_str()).collect();
    designed_strengths.sort_unstable();
    designed_strengths.dedup();
    let mut held_strengths: Vec<&str> = FacetStrength::ALL
        .iter()
        .map(|strength| strength.as_str())
        .collect();
    held_strengths.sort_unstable();
    assert_eq!(designed_strengths, held_strengths);

    // One `APPLIED` state whose five facets are those five values, carried
    // through a real assertion and back out of its wire form unchanged.
    let concept = entity("transaction");
    let lecture = lecture_document("ks-mixed")?;
    let dossier = full_dossier(concept);
    let evidence = vec![
        admitted(
            ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
            "teaching",
            &dossier,
        )?,
        admitted(
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
            "project",
            &dossier,
        )?,
    ];
    let projection = project(&evidence, &[])?;
    assert_eq!(projection.level(), MasteryLevel::Applied);
    assert_eq!(projection.automatic(), AutomaticLevel::Applied);

    let assertion = KnowledgeStateAssertion::open(
        concept,
        TimestampMillis::new(i64::try_from(NOW)?),
        &projection,
        profile,
        FreshnessBand::VeryHigh,
        ConfidencePermille::new(920)?,
        Vec::new(),
    )?;
    for facet in MasteryFacet::ALL {
        assert_eq!(
            assertion.facets().strength(facet),
            profile.strength(facet),
            "{facet:?} did not survive"
        );
    }

    let json = serde_json::to_string(&assertion)?;
    let restored: KnowledgeStateAssertion = serde_json::from_str(&json)?;
    assert_eq!(restored, assertion, "the assertion did not round-trip");

    // The single UI level and the whole internal facet set are both readable at
    // once, which is `REQ-13-001`.
    assert_eq!(restored.mastery_level(), MasteryLevel::Applied);
    assert_eq!(restored.facets(), profile);
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `evidence_ceilings_are_never_exceeded`
// ---------------------------------------------------------------------------

#[test]
fn evidence_ceilings_are_never_exceeded() -> TestResult {
    let page = specification()?;
    let rows = table_after(&page, "### 13.2 Evidence가 Mastery에 미치는 영향")?;

    // Both text cells of every row, in both directions.
    let designed: Vec<(String, String)> = rows
        .iter()
        .map(|cells| -> Result<(String, String), Box<dyn Error>> {
            Ok((
                cells.get(1).ok_or("a 13.2 row has no meaning cell")?.clone(),
                cells.get(2).ok_or("a 13.2 row has no ceiling cell")?.clone(),
            ))
        })
        .collect::<Result<_, _>>()?;
    let held: Vec<(String, String)> = CEILINGS
        .iter()
        .map(|row| (row.interpretation.to_owned(), row.ceiling_cell.to_owned()))
        .collect();
    assert_eq!(designed, held, "section 13.2's rows and CEILINGS disagree");
    assert_eq!(CEILINGS.len(), EvidenceKind::ALL.len());

    // Every row's own ceiling is the one the enumeration answers with.
    for row in &CEILINGS {
        assert_eq!(row.kind.ceiling(), row.ceiling);
    }

    // And no projection over any single row's evidence exceeds that row's
    // ceiling. The two rows this crate has no `ConceptEvidence` variant for are
    // covered below and by `grade_creates_no_concept_promotion`.
    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("ceilings")?;
    for (kind, evidence) in sample_evidence(&lecture, concept)? {
        let item = admitted(evidence, kind.as_str(), &dossier)?;
        let projection = project(std::slice::from_ref(&item), &[])?;
        let ceiling = kind.ceiling();
        assert!(
            ceiling.admits(projection.level()),
            "{kind:?} projected {:?}, above {ceiling:?}",
            projection.level()
        );
        assert_eq!(projection.ceiling().ceiling(), ceiling);
    }

    // The two rows with no variant: one has no promotion and one has no concept
    // at all.
    assert_eq!(
        EvidenceKind::DependencyPresenceOnly.ceiling(),
        EvidenceCeiling::NoPromotion
    );
    assert_eq!(
        EvidenceKind::CourseGrade.ceiling(),
        EvidenceCeiling::NoPromotion
    );
    Ok(())
}

/// One `ConceptEvidence` for each of the seven rows that has a variant.
fn sample_evidence(
    lecture: &Lecture,
    concept: EntityId,
) -> Result<Vec<(EvidenceKind, ConceptEvidence)>, Box<dyn Error>> {
    let repetition = TransferRepetition::across(vec![
        TransferContext::of("service-a", evidence_id("ctx-a"), true),
        TransferContext::of("service-b", evidence_id("ctx-b"), true),
    ])
    .ok_or("two distinct independent contexts are a repetition")?;
    let _ = concept;
    Ok(vec![
        (
            EvidenceKind::MeaningfulTeaching,
            ConceptEvidence::MeaningfulTeaching(teaching(lecture)?),
        ),
        (
            EvidenceKind::SelfExplanationConfirmed,
            ConceptEvidence::SelfExplanation(SelfExplanation::confirmed_by(
                evidence_id("explanation"),
                &confirmation(entity("transaction"), MasteryLevel::Understood)?,
            )),
        ),
        (
            EvidenceKind::ConceptSpecificExercise,
            ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id("exercise"))),
        ),
        (
            EvidenceKind::AuthoredProjectCode,
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
        ),
        (
            EvidenceKind::IncidentDebugging,
            ConceptEvidence::IncidentDebugging(IncidentRepair::of(
                evidence_id("incident"),
                evidence_id("cause"),
                evidence_id("fix"),
                evidence_id("verification"),
            )),
        ),
        (
            EvidenceKind::RepeatedIndependentTransfer,
            ConceptEvidence::RepeatedTransfer(repetition),
        ),
        (
            EvidenceKind::DependencyPresenceOnly,
            ConceptEvidence::DependencyPresence(installed_only()?),
        ),
    ])
}

// ---------------------------------------------------------------------------
// 4. `course_attendance_only_ceiling_is_exposed`
// ---------------------------------------------------------------------------

#[test]
fn course_attendance_only_ceiling_is_exposed() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("attendance")?;
    let teaching_only = vec![admitted(
        ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
        "teaching",
        &dossier,
    )?];
    let projection = project(&teaching_only, &[])?;

    // The ceiling *is* `EXPOSED`: section 13.1's `UNDERSTOOD` row says
    // `강의 수강만으로 자동 승격 금지`, and section 13.2's first row says
    // `Exposed`.
    assert_eq!(projection.level(), MasteryLevel::Exposed);
    assert_eq!(
        projection.ceiling().ceiling(),
        EvidenceCeiling::UpTo(MasteryLevel::Exposed)
    );

    // And the ceiling *is exposed*: a reader is told which row fixed it and what
    // that row's own cell says, not just a level.
    assert_eq!(
        projection.ceiling().from(),
        Some(EvidenceKind::MeaningfulTeaching)
    );
    assert_eq!(projection.ceiling().cell(), "Exposed");

    // The control: the same fixture with one further row of evidence rises, so
    // the ceiling above is the teaching row's doing and not a floor this
    // projection would report for anything.
    let with_explanation = vec![
        teaching_only[0].clone(),
        admitted(
            ConceptEvidence::SelfExplanation(SelfExplanation::confirmed_by(
                evidence_id("explanation"),
                &confirmation(concept, MasteryLevel::Understood)?,
            )),
            "explanation",
            &dossier,
        )?,
    ];
    let risen = project(&with_explanation, &[])?;
    assert_eq!(risen.level(), MasteryLevel::Understood);
    assert_eq!(
        risen.ceiling().from(),
        Some(EvidenceKind::SelfExplanationConfirmed)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `dependency_only_creates_no_promotion`
// ---------------------------------------------------------------------------

#[test]
fn dependency_only_creates_no_promotion() -> TestResult {
    let concept = entity("caching");
    let dossier = full_dossier(concept);

    // `P2-R4` produced no `ObservedProof` for the manifest-only corpus, and
    // produced one for the corpus that calls the subject. The two constructors
    // are complements over the same input, so exactly one of them answers.
    let observed_corpus = common::project::built(&common::project::OBSERVED_REDIS)?;
    let observed_set =
        common::project::classified(&observed_corpus, &common::project::goal("cache-warmup", 1)?)?;
    let observed_stance = common::project::stance_of(&observed_set)?;
    assert!(ProjectUse::of_stance(&observed_stance).is_some());
    assert!(DependencyOnly::of_stance(&observed_stance).is_none());

    // The same two trees differ only in whether anything uses the package, and
    // the two corpora are otherwise built by the same four crates. So the
    // absence below is `P2-R2`'s rung and not a fixture difference.
    let installed_corpus = common::project::built(&common::project::INSTALLED_ONLY)?;
    let installed_set = common::project::classified_with(
        &installed_corpus,
        &common::project::goal("cache-warmup", 1)?,
        std::slice::from_ref(&common::project::benefit_contract()?),
    )?;
    let installed_stance = common::project::stance_of(&installed_set)?;
    assert!(installed_stance.observed().is_none());
    assert!(ProjectUse::of_stance(&installed_stance).is_none());

    let installed = installed_only()?;
    let evidence = vec![admitted(
        ConceptEvidence::DependencyPresence(installed),
        "dependency",
        &dossier,
    )?];
    let projection = project(&evidence, &[])?;

    // Admitted, retained and shown — and it promoted nothing.
    assert_eq!(projection.level(), MasteryLevel::Unseen);
    assert_eq!(projection.supporting().len(), 1);
    assert_eq!(
        projection.unseen_basis(),
        Some(UnseenBasis::EvidenceRecordedWithoutPromotion)
    );
    assert_eq!(
        projection.ceiling().ceiling(),
        EvidenceCeiling::NoPromotion,
        "an installed dependency licenses no level"
    );

    // The control: the same concept observed in the same repository rises to
    // `APPLIED`, so the refusal above is about the evidence and not about the
    // fixture.
    let used = vec![admitted(
        ConceptEvidence::AuthoredProjectCode(observed_use()?),
        "project",
        &dossier,
    )?];
    assert_eq!(project(&used, &[])?.level(), MasteryLevel::Applied);
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `grade_creates_no_concept_promotion`
// ---------------------------------------------------------------------------

#[test]
fn grade_creates_no_concept_promotion() -> TestResult {
    let concept = entity("transaction");
    let grade = CourseGradeSignal::recorded("M1522.000100", "2027-1", "A+", evidence_id("grade"));

    // The grade is retained and linked, and it names no concept: there is
    // nowhere on the value to write one.
    let signal = BroadSignal::of_grade(grade.clone());
    assert_eq!(signal.grade().course(), "M1522.000100");
    assert_eq!(signal.grade().kind(), EvidenceKind::CourseGrade);

    // No `ConceptEvidence` variant carries it, so it cannot reach a projection
    // at all. This is the whole-set half: every variant's kind is one of the
    // seven, and `CourseGrade` is not among them.
    let lecture = lecture_document("grade")?;
    let kinds: Vec<EvidenceKind> = sample_evidence(&lecture, concept)?
        .iter()
        .map(|(kind, evidence)| {
            assert_eq!(*kind, evidence.kind());
            *kind
        })
        .collect();
    assert!(!kinds.contains(&EvidenceKind::CourseGrade));
    assert_eq!(kinds.len() + 1, EvidenceKind::ALL.len());

    // An assertion carries the grade and still projects `UNSEEN`, so the signal
    // is visible and inert at the same time.
    let projection = project(&[], &[])?;
    let assertion = KnowledgeStateAssertion::open(
        concept,
        TimestampMillis::new(i64::try_from(NOW)?),
        &projection,
        facets(),
        FreshnessBand::Unknown,
        ConfidencePermille::new(0)?,
        vec![signal],
    )?;
    assert_eq!(assertion.mastery_level(), MasteryLevel::Unseen);
    assert_eq!(assertion.broad_signals().len(), 1);
    assert_eq!(assertion.evidence().len(), 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `unseen_is_not_a_failed_test`
// ---------------------------------------------------------------------------

#[test]
fn unseen_is_not_a_failed_test() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);

    let nothing = project(&[], &[])?;
    let failed = vec![admitted(
        ConceptEvidence::ConceptExercise(ExerciseOutcome::failed(evidence_id("attempt"))),
        "attempt",
        &dossier,
    )?];
    let attempted = project(&failed, &[])?;

    // Both are `UNSEEN`, and they are not the same value.
    assert_eq!(nothing.level(), MasteryLevel::Unseen);
    assert_eq!(attempted.level(), MasteryLevel::Unseen);
    assert_ne!(
        nothing, attempted,
        "no evidence and a failed attempt must not be one projection"
    );

    // The distinction is readable rather than implicit.
    assert_eq!(nothing.unseen_basis(), Some(UnseenBasis::NoEvidenceRecorded));
    assert_eq!(
        attempted.unseen_basis(),
        Some(UnseenBasis::EvidenceRecordedWithoutPromotion)
    );
    assert!(nothing.contradicting().is_empty());
    assert_eq!(attempted.contradicting().len(), 1);
    assert!(
        attempted
            .sufficiency()
            .gaps()
            .contains(&SufficiencyGap::Contradicted)
    );

    // The copy the product shows is section 13.1's own sentence, and it does not
    // spell a verdict about the person.
    assert_eq!(nothing.unseen_meaning(), Some(UNSEEN_MEANING));
    assert_eq!(attempted.unseen_meaning(), Some(UNSEEN_MEANING));
    assert!(UNSEEN_MEANING.contains("evidence 없음"));
    let page = specification()?;
    assert!(
        page.contains(UNSEEN_MEANING),
        "the copy is not the design document's own"
    );

    // A promoted state has no basis at all, so the field is not a value every
    // projection carries.
    let lecture = lecture_document("unseen")?;
    let seen = vec![admitted(
        ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
        "teaching",
        &dossier,
    )?];
    assert_eq!(project(&seen, &[])?.unseen_basis(), None);
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. `eligibility_four_checks_block_with_reason_codes`
// ---------------------------------------------------------------------------

#[test]
fn eligibility_four_checks_block_with_reason_codes() -> TestResult {
    let page = specification()?;
    let block = block_after(&page, "### 13.4 상태 갱신", "```text")?;
    let designed: Vec<String> = block
        .iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("├─ ")
                .or_else(|| trimmed.strip_prefix("└─ "))
                .map(str::to_owned)
        })
        .collect();
    let held: Vec<String> = EligibilityCheck::ALL
        .iter()
        .map(|check| check.question().to_owned())
        .collect();
    assert_eq!(designed, held, "section 13.4's four checks disagree");

    let concept = entity("transaction");
    let evidence =
        ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id("exercise")));

    // One check false at a time, each with its own code and each blocking.
    let cases: [(EligibilityCheck, EvidenceDossier, EligibilityReasonCode); 4] = [
        (
            EligibilityCheck::ExactConceptLink,
            EvidenceDossier::of(
                ConceptLink::Ambiguous,
                Participation::Authored,
                Outcome::Succeeded,
                SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
            ),
            EligibilityReasonCode::ConceptLinkAmbiguous,
        ),
        (
            EligibilityCheck::AuthorshipOrParticipation,
            EvidenceDossier::of(
                ConceptLink::Exact(concept, EntityKind::Concept),
                Participation::Unknown,
                Outcome::Succeeded,
                SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
            ),
            EligibilityReasonCode::AuthorshipUnknown,
        ),
        (
            EligibilityCheck::Outcome,
            EvidenceDossier::of(
                ConceptLink::Exact(concept, EntityKind::Concept),
                Participation::Authored,
                Outcome::Unknown,
                SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
            ),
            EligibilityReasonCode::OutcomeUnknown,
        ),
        (
            EligibilityCheck::SourceIntegrity,
            EvidenceDossier::of(
                ConceptLink::Exact(concept, EntityKind::Concept),
                Participation::Authored,
                Outcome::Succeeded,
                SourceIntegrity::Broken,
            ),
            EligibilityReasonCode::SourceIntegrityBroken,
        ),
    ];
    for (check, dossier, code) in &cases {
        let outcome = EligibilityOutcome::admit(evidence.clone(), evidence_id("item"), dossier);
        let blocked = outcome
            .blocked()
            .ok_or_else(|| format!("{check:?} false was admitted"))?;
        assert_eq!(blocked.reasons(), [*code]);
        assert_eq!(blocked.failed_checks(), vec![*check]);
        assert_eq!(code.check(), *check);
        // The evidence itself is not discarded.
        assert_eq!(blocked.evidence().kind(), EvidenceKind::ConceptSpecificExercise);
        // And it cannot reach a projection.
        assert!(outcome.admitted().is_none());
    }

    // Every check false at once reports all four codes rather than the first.
    let all_wrong = EvidenceDossier::of(
        ConceptLink::Absent,
        Participation::ThirdParty,
        Outcome::Unknown,
        SourceIntegrity::Unknown,
    );
    let blocked = EligibilityOutcome::admit(evidence.clone(), evidence_id("item"), &all_wrong);
    let blocked = blocked.blocked().ok_or("a fully wrong dossier was admitted")?;
    assert_eq!(blocked.reasons().len(), EligibilityCheck::ALL.len());
    assert_eq!(blocked.failed_checks(), EligibilityCheck::ALL.to_vec());

    // A `FIELD` is not a thing a person holds a mastery of: section 7.4's own
    // answer, reached through `P2-N1`'s tier rather than through a name list.
    let broad = EvidenceDossier::of(
        ConceptLink::Exact(entity("database-systems"), EntityKind::Field),
        Participation::Authored,
        Outcome::Succeeded,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    );
    let blocked = EligibilityOutcome::admit(evidence.clone(), evidence_id("item"), &broad);
    assert_eq!(
        blocked.blocked().map(|item| item.reasons().to_vec()),
        Some(vec![EligibilityReasonCode::ConceptLinkTierNotLearnable])
    );

    // The control: all four answered admits, so the refusals above are the
    // answers and not the fixture.
    let admitted = EligibilityOutcome::admit(evidence, evidence_id("item"), &full_dossier(concept));
    assert!(admitted.admitted().is_some());

    // A known failure is a known outcome: section 13.4 asks whether the outcome
    // is known, not whether it succeeded.
    let failed_but_known = EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Authored,
        Outcome::Failed,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    );
    assert!(failed_but_known.blocking_reasons().is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. `fluent_requires_repetition_and_user_confirmation`
// ---------------------------------------------------------------------------

#[test]
fn fluent_requires_repetition_and_user_confirmation() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);

    // The automatic path cannot express `FLUENT` at all.
    assert!(!AutomaticLevel::ALL.iter().any(|level| level.level() == MasteryLevel::Fluent));
    assert_eq!(AutomaticLevel::of(MasteryLevel::Fluent), None);
    for level in LADDER {
        if level != MasteryLevel::Fluent {
            assert!(AutomaticLevel::of(level).is_some(), "{level:?} is automatic");
        }
    }

    // Even the row whose ceiling is `Fluent candidate` projects `APPLIED`.
    let repetition = TransferRepetition::across(vec![
        TransferContext::of("service-a", evidence_id("ctx-a"), true),
        TransferContext::of("service-b", evidence_id("ctx-b"), true),
    ])
    .ok_or("two distinct independent contexts are a repetition")?;
    let transfer = vec![admitted(
        ConceptEvidence::RepeatedTransfer(repetition.clone()),
        "transfer",
        &dossier,
    )?];
    let projection = project(&transfer, &[])?;
    assert_eq!(projection.level(), MasteryLevel::Applied);
    assert_eq!(
        EvidenceKind::RepeatedIndependentTransfer.ceiling(),
        EvidenceCeiling::UpTo(MasteryLevel::Fluent),
        "the row's ceiling admits FLUENT; the automatic path still does not reach it"
    );

    // Repetition alone is not enough — but neither is it satisfied by one
    // context repeated, nor by contexts whose work was not independent.
    assert!(
        TransferRepetition::across(vec![
            TransferContext::of("service-a", evidence_id("ctx-a"), true),
            TransferContext::of("service-a", evidence_id("ctx-a2"), true),
        ])
        .is_none(),
        "one context named twice is not repetition"
    );
    assert!(
        TransferRepetition::across(vec![
            TransferContext::of("service-a", evidence_id("ctx-a"), false),
            TransferContext::of("service-b", evidence_id("ctx-b"), false),
        ])
        .is_none(),
        "work that was not independent is not independent performance"
    );

    // A model run cannot mint the confirmation half. Its own valid pairing is
    // refused, and the user pairing it would need fails ADR-003's matrix.
    let evidence = evidence_item("confirmation");
    let native_model = confirmation_claim(
        concept,
        MasteryLevel::Fluent,
        &evidence,
        AuthorityClass::ModelInference,
        EpistemicStatus::AiInferred,
        academic_knowledge_state::STATE_CONFIRMATION_PREDICATE,
    )?;
    assert!(matches!(
        UserConfirmation::verify(
            &model_actor(),
            &native_model,
            &evidence,
            concept,
            MasteryLevel::Fluent,
            TimestampMillis::new(i64::try_from(NOW)?),
        ),
        Err(KnowledgeStateError::InvalidConfirmationAction)
    ));
    let forged_user = confirmation_claim(
        concept,
        MasteryLevel::Fluent,
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        academic_knowledge_state::STATE_CONFIRMATION_PREDICATE,
    )?;
    assert!(matches!(
        UserConfirmation::verify(
            &model_actor(),
            &forged_user,
            &evidence,
            concept,
            MasteryLevel::Fluent,
            TimestampMillis::new(i64::try_from(NOW)?),
        ),
        Err(KnowledgeStateError::Domain(_))
    ));

    // The wrong predicate is refused even from the user.
    let wrong_predicate = confirmation_claim(
        concept,
        MasteryLevel::Fluent,
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        "knowledge.state.noted",
    )?;
    assert!(matches!(
        UserConfirmation::verify(
            &user_actor(),
            &wrong_predicate,
            &evidence,
            concept,
            MasteryLevel::Fluent,
            TimestampMillis::new(i64::try_from(NOW)?),
        ),
        Err(KnowledgeStateError::InvalidConfirmationAction)
    ));

    // An authorization for another level is refused.
    assert!(matches!(
        FluentAuthorization::granted(
            repetition.clone(),
            confirmation(concept, MasteryLevel::Applied)?,
            concept,
        ),
        Err(KnowledgeStateError::ConfirmationLevelMismatch)
    ));

    // Both halves together, and only then, reach `FLUENT`.
    let authorization =
        FluentAuthorization::granted(repetition, confirmation(concept, MasteryLevel::Fluent)?, concept)?;
    assert_eq!(authorization.distinct_contexts(), 2);
    let fluent = projection.with_fluency(authorization, concept)?;
    assert_eq!(fluent.level(), MasteryLevel::Fluent);
    assert_eq!(fluent.automatic(), AutomaticLevel::Applied);
    assert_eq!(fluent.fluency_contexts(), Some(2));

    // And a serialized `FLUENT` cannot come back without its record.
    let history = KnowledgeStateHistory::open(
        concept,
        transfer,
        Vec::new(),
        Vec::new(),
        facets(),
        freshness()?,
        TimestampMillis::new(i64::try_from(NOW)?),
    )?;
    let repetition = TransferRepetition::across(vec![
        TransferContext::of("service-a", evidence_id("ctx-a"), true),
        TransferContext::of("service-b", evidence_id("ctx-b"), true),
    ])
    .ok_or("two distinct independent contexts are a repetition")?;
    let promoted = history.promote_to_fluent(
        FluentAuthorization::granted(
            repetition,
            confirmation(concept, MasteryLevel::Fluent)?,
            concept,
        )?,
        &confirmation(concept, MasteryLevel::Fluent)?,
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;
    let current = promoted.current().ok_or("no current version")?;
    assert_eq!(current.mastery_level(), MasteryLevel::Fluent);
    let json = serde_json::to_string(current)?;
    assert!(
        serde_json::from_str::<KnowledgeStateAssertion>(&json).is_ok(),
        "the promoted assertion did not round-trip"
    );

    let mut stripped: serde_json::Value = serde_json::from_str(&json)?;
    let object = stripped
        .as_object_mut()
        .ok_or("the wire form is not an object")?;
    assert!(
        object.remove("fluency").is_some(),
        "the promoted assertion carries no fluency record"
    );
    assert!(
        serde_json::from_str::<KnowledgeStateAssertion>(&serde_json::to_string(&stripped)?).is_err(),
        "a FLUENT assertion deserialized without its record"
    );

    // And the reverse: a record on a level that is not `FLUENT` is refused too,
    // so the check is a pairing rather than a one-way presence test.
    let applied = KnowledgeStateAssertion::open(
        concept,
        TimestampMillis::new(i64::try_from(NOW)?),
        &project(&[], &[])?,
        facets(),
        FreshnessBand::Unknown,
        ConfidencePermille::new(0)?,
        Vec::new(),
    )?;
    let mut smuggled: serde_json::Value = serde_json::from_str(&serde_json::to_string(&applied)?)?;
    let record = serde_json::from_str::<serde_json::Value>(&json)?
        .get("fluency")
        .cloned()
        .ok_or("the promoted assertion carries no fluency record")?;
    smuggled
        .as_object_mut()
        .ok_or("the wire form is not an object")?
        .insert("fluency".to_owned(), record);
    assert!(
        serde_json::from_str::<KnowledgeStateAssertion>(&serde_json::to_string(&smuggled)?).is_err(),
        "an UNSEEN assertion carried a fluency record"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. `assertion_is_never_mutated_in_place`
// ---------------------------------------------------------------------------

#[test]
fn assertion_is_never_mutated_in_place() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("versions")?;
    let first = vec![admitted(
        ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
        "teaching",
        &dossier,
    )?];
    let projection = project(&first, &[])?;
    let one = KnowledgeStateAssertion::open(
        concept,
        TimestampMillis::new(i64::try_from(NOW)?),
        &projection,
        facets(),
        FreshnessBand::VeryHigh,
        ConfidencePermille::new(920)?,
        Vec::new(),
    )?;
    let before = serde_json::to_string(&one)?;

    let mut wider = first.clone();
    wider.push(admitted(
        ConceptEvidence::AuthoredProjectCode(observed_use()?),
        "project",
        &dossier,
    )?);
    let two = one.revise(
        TimestampMillis::new(i64::try_from(LATER)?),
        &project(&wider, &[])?,
        facets(),
        FreshnessBand::VeryHigh,
        ConfidencePermille::new(920)?,
        Vec::new(),
    )?;

    // The old value is unchanged, byte for byte.
    assert_eq!(serde_json::to_string(&one)?, before);
    assert_eq!(one.mastery_level(), MasteryLevel::Exposed);
    assert_eq!(one.version(), 1);
    assert_eq!(one.supersedes(), None);

    // The new one is a different version with a different identity that names
    // the old.
    assert_eq!(two.version(), 2);
    assert_eq!(two.supersedes(), Some(one.id()));
    assert_ne!(two.id(), one.id());
    assert_eq!(two.mastery_level(), MasteryLevel::Applied);

    // Identity is a hash chain, so the same content under a different
    // predecessor is a different identity. Two assertions built from the same
    // projection at the same instant with no predecessor are the same identity,
    // which is what makes the previous line a statement about the chain rather
    // than about a nonce.
    let again = KnowledgeStateAssertion::open(
        concept,
        TimestampMillis::new(i64::try_from(NOW)?),
        &projection,
        facets(),
        FreshnessBand::VeryHigh,
        ConfidencePermille::new(920)?,
        Vec::new(),
    )?;
    assert_eq!(again.id(), one.id());
    let third = two.revise(
        TimestampMillis::new(i64::try_from(LATER)?),
        &project(&wider, &[])?,
        facets(),
        FreshnessBand::VeryHigh,
        ConfidencePermille::new(920)?,
        Vec::new(),
    )?;
    assert_ne!(third.id(), two.id(), "a later version binds its predecessor");

    // A tampered wire value does not come back: the identity is recomputed.
    let json = serde_json::to_string(&two)?;
    let tampered = json.replace("\"APPLIED\"", "\"FLUENT\"");
    assert!(serde_json::from_str::<KnowledgeStateAssertion>(&tampered).is_err());
    let relabelled = json.replace("\"version\":2", "\"version\":7");
    assert!(
        serde_json::from_str::<KnowledgeStateAssertion>(&relabelled).is_err(),
        "a renumbered version kept its identity"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 11. `retraction_is_append_only_and_recomputes_projection`
// ---------------------------------------------------------------------------

#[test]
fn retraction_is_append_only_and_recomputes_projection() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("retraction")?;
    let teaching_item = admitted(
        ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
        "teaching",
        &dossier,
    )?;
    let copied_exercise = admitted(
        ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id("assignment"))),
        "assignment",
        &dossier,
    )?;
    let history = KnowledgeStateHistory::open(
        concept,
        vec![teaching_item, copied_exercise],
        Vec::new(),
        Vec::new(),
        facets(),
        freshness()?,
        TimestampMillis::new(i64::try_from(NOW)?),
    )?;
    let before = history.current().ok_or("no version")?.clone();
    assert_eq!(before.mastery_level(), MasteryLevel::Practiced);
    assert_eq!(before.evidence().len(), 2);

    // Section 13.2's own case: the assignment turns out to be somebody else's
    // work, which is the authorship check answered again and differently.
    let retracted = history.retract(
        EvidenceRetraction::of(
            evidence_id("assignment"),
            EligibilityCheck::AuthorshipOrParticipation,
            TimestampMillis::new(i64::try_from(LATER)?),
        ),
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;

    // `철회 event도 역사에 남고`: the row is in the history and so is every
    // earlier version.
    assert_eq!(retracted.retractions().len(), 1);
    assert!(matches!(
        retracted.entries().get(1),
        Some(HistoryEntry::Retracted(_))
    ));
    assert_eq!(retracted.versions().len(), 2);
    let earlier = retracted
        .version_at(before.id())
        .ok_or("the earlier version is not readable")?;
    assert_eq!(earlier, &before, "the earlier projection changed");
    assert_eq!(earlier.mastery_level(), MasteryLevel::Practiced);

    // `projection만 다시 계산한다`: the current version drops to the level the
    // surviving evidence supports and no longer lists the retracted item.
    let current = retracted.current().ok_or("no current version")?;
    assert_eq!(current.version(), 2);
    assert_eq!(current.mastery_level(), MasteryLevel::Exposed);
    assert!(!current.evidence().contains(&evidence_id("assignment")));
    assert_eq!(retracted.surviving_evidence().len(), 1);

    // Nothing was deleted: the history still holds both entries and the
    // retraction names which check failed.
    assert_eq!(
        retracted.retractions()[0].failed_check(),
        EligibilityCheck::AuthorshipOrParticipation
    );

    // The control: a retraction naming evidence this history never admitted is
    // refused rather than silently recorded.
    assert!(matches!(
        retracted.clone().retract(
            EvidenceRetraction::of(
                evidence_id("never-admitted"),
                EligibilityCheck::Outcome,
                TimestampMillis::new(i64::try_from(LATER)?),
            ),
            freshness()?,
            TimestampMillis::new(i64::try_from(LATER)?),
        ),
        Err(KnowledgeStateError::RetractionNamesUnknownEvidence)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// 12. `confirmed_state_rejects_ai_adjustment`
// ---------------------------------------------------------------------------

#[test]
fn confirmed_state_rejects_ai_adjustment() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("confirmed")?;
    let evidence = vec![
        admitted(
            ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
            "teaching",
            &dossier,
        )?,
        admitted(
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
            "project",
            &dossier,
        )?,
    ];
    let history = KnowledgeStateHistory::open(
        concept,
        evidence,
        Vec::new(),
        Vec::new(),
        facets(),
        freshness()?,
        TimestampMillis::new(i64::try_from(NOW)?),
    )?;
    let confirmed = history.confirm(
        &confirmation(concept, MasteryLevel::Applied)?,
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;
    let standing = confirmed.current().ok_or("no version")?.clone();
    assert!(standing.user_confirmed());
    assert_eq!(standing.mastery_level(), MasteryLevel::Applied);

    // Both directions, one at a time, each rejected and each leaving the
    // assertion exactly as it was.
    for proposed in [MasteryLevel::Fluent, MasteryLevel::Practiced] {
        let applied = confirmed.clone().propose(
            AiProposal::of(
                model_run_id("run"),
                concept,
                proposed,
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
            freshness()?,
            TimestampMillis::new(i64::try_from(LATER)?),
        )?;
        let after = applied.history().current().ok_or("no version")?;
        assert_eq!(after, &standing, "a confirmed state was adjusted");
        assert!(matches!(applied.outcome(), ProposalOutcome::Conflict(_)));
    }

    // The direction is recorded, and it is the one the proposal moved in.
    let raised = confirmed.clone().propose(
        AiProposal::of(
            model_run_id("run"),
            concept,
            MasteryLevel::Fluent,
            Vec::new(),
        ),
        Vec::new(),
        Vec::new(),
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;
    let ProposalOutcome::Conflict(card) = raised.outcome() else {
        return Err("a raise on a confirmed state did not conflict".into());
    };
    assert_eq!(card.direction(), AdjustmentDirection::Raise);

    // The control: the same proposal on the same evidence before confirmation
    // supersedes rather than conflicts, so the refusal above is the
    // confirmation's doing.
    let unconfirmed = KnowledgeStateHistory::open(
        concept,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        facets(),
        freshness()?,
        TimestampMillis::new(i64::try_from(NOW)?),
    )?;
    let applied = unconfirmed.propose(
        AiProposal::of(
            model_run_id("run"),
            concept,
            MasteryLevel::Exposed,
            Vec::new(),
        ),
        vec![admitted(
            ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
            "teaching",
            &dossier,
        )?],
        Vec::new(),
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;
    assert!(matches!(
        applied.outcome(),
        ProposalOutcome::Superseded(_)
    ));
    assert_eq!(
        applied
            .history()
            .current()
            .ok_or("no version")?
            .mastery_level(),
        MasteryLevel::Exposed
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 13. `conflict_card_instead_of_auto_change`
// ---------------------------------------------------------------------------

#[test]
fn conflict_card_instead_of_auto_change() -> TestResult {
    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("conflict")?;
    let evidence = vec![
        admitted(
            ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
            "teaching",
            &dossier,
        )?,
        admitted(
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
            "project",
            &dossier,
        )?,
    ];
    let history = KnowledgeStateHistory::open(
        concept,
        evidence,
        Vec::new(),
        Vec::new(),
        facets(),
        freshness()?,
        TimestampMillis::new(i64::try_from(NOW)?),
    )?;
    let confirmed = history.confirm(
        &confirmation(concept, MasteryLevel::Applied)?,
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;
    let standing = confirmed.current().ok_or("no version")?.clone();

    let contrary = vec![ConceptEvidence::ConceptExercise(ExerciseOutcome::failed(
        evidence_id("recall-failure"),
    ))];
    let proposal = AiProposal::of(
        model_run_id("run"),
        concept,
        MasteryLevel::Understood,
        contrary.clone(),
    );
    let applied = confirmed.propose(
        proposal.clone(),
        Vec::new(),
        Vec::new(),
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;

    let ProposalOutcome::Conflict(card) = applied.outcome() else {
        return Err("contrary evidence did not open a card".into());
    };

    // Both sides, and neither rewritten.
    assert_eq!(card.standing(), &standing);
    assert_eq!(card.standing_level(), MasteryLevel::Applied);
    assert_eq!(card.proposed(), &proposal);
    assert_eq!(card.proposed_level(), MasteryLevel::Understood);
    assert_eq!(card.proposed().evidence(), contrary.as_slice());
    assert_eq!(card.direction(), AdjustmentDirection::Lower);

    // The token is `P2-M3`'s and not a second vocabulary.
    assert_eq!(card.reason_token(), academic_ledger::NEW_EVIDENCE_CONFLICT);
    assert_eq!(
        academic_ledger::ConflictReason::from_token(card.reason_token()),
        Some(card.reason())
    );

    // The card is in the history beside both versions, and no version was
    // added: an automatic change would have appended one.
    let history = applied.history();
    assert_eq!(history.conflicts().len(), 1);
    assert_eq!(history.versions().len(), 2);
    assert_eq!(history.current().ok_or("no version")?, &standing);

    // A proposal naming the level the state already holds is not an
    // adjustment, so it opens no card and changes nothing either.
    let unchanged = history.clone().propose(
        AiProposal::of(
            model_run_id("run"),
            concept,
            MasteryLevel::Applied,
            Vec::new(),
        ),
        Vec::new(),
        Vec::new(),
        freshness()?,
        TimestampMillis::new(i64::try_from(LATER)?),
    )?;
    assert!(matches!(unchanged.outcome(), ProposalOutcome::NoAdjustment));
    assert_eq!(unchanged.history().conflicts().len(), 1);

    // A proposal about another concept is refused rather than filed here.
    assert!(matches!(
        history.clone().propose(
            AiProposal::of(
                model_run_id("run"),
                entity("another-concept"),
                MasteryLevel::Fluent,
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
            freshness()?,
            TimestampMillis::new(i64::try_from(LATER)?),
        ),
        Err(KnowledgeStateError::ProposalNamesAnotherConcept)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// Beside the thirteen: what `estimateConfidence` is and is not.
// ---------------------------------------------------------------------------

#[test]
fn estimate_confidence_is_evidence_sufficiency_and_not_a_score() -> TestResult {
    let page = specification()?;
    assert!(
        page.contains("`estimateConfidence`는 사용자의 실력 점수가 아니다"),
        "section 13.1 no longer says what this test is about"
    );

    let concept = entity("transaction");
    let dossier = full_dossier(concept);
    let lecture = lecture_document("sufficiency")?;
    let supporting = vec![
        admitted(
            ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
            "teaching",
            &dossier,
        )?,
        admitted(
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
            "project",
            &dossier,
        )?,
    ];

    // Section 13.1's own illustration: an `APPLIED` projection with applied
    // candidates whose authorship and outcome are unclear reads 0.45.
    let unclear_authorship = EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Unknown,
        Outcome::Succeeded,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    );
    let unclear_outcome = EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Authored,
        Outcome::Unknown,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    );
    let blocked: Vec<_> = [
        EligibilityOutcome::admit(
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
            evidence_id("candidate-a"),
            &unclear_authorship,
        ),
        EligibilityOutcome::admit(
            ConceptEvidence::AuthoredProjectCode(observed_use()?),
            evidence_id("candidate-b"),
            &unclear_outcome,
        ),
    ]
    .into_iter()
    .filter_map(|outcome| outcome.blocked().cloned())
    .collect();
    assert_eq!(blocked.len(), 2);

    let projection = project(&supporting, &blocked)?;
    assert_eq!(projection.level(), MasteryLevel::Applied);
    assert_eq!(
        projection.sufficiency().permille(),
        ConfidencePermille::new(450)?,
        "section 13.1's own `mastery 4, confidence 0.45`"
    );
    assert_eq!(
        projection.sufficiency().gaps(),
        [
            SufficiencyGap::AuthorshipUnresolved,
            SufficiencyGap::OutcomeUnresolved
        ]
    );

    // Sufficient evidence with nothing unresolved reads 1000, so the number
    // above is the gaps' doing and not a constant.
    let clean = project(&supporting, &[])?;
    assert_eq!(clean.sufficiency().permille(), ConfidencePermille::new(1000)?);
    assert!(clean.sufficiency().gaps().is_empty());

    // And the level did not move: sufficiency and mastery are two fields.
    assert_eq!(clean.level(), projection.level());
    Ok(())
}

