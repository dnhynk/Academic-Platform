//! `P2-N7`'s ten named acceptance rows.
//!
//! Six of them read `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compare what is in this crate against what is in the document, in both
//! directions: section 23's five-state block, its coverage sentence, its four
//! back-quoted disposition names, its three taste-path steps, its eight schema
//! keys and section 34.5's uncertainty cell are **measurements** rather than
//! counts restated in a test.
//!
//! The lecture evidence comes from `P2-N2`'s own fixture module, included by
//! `#[path]`: the capture is written by the real `academic_capture::begin`, the
//! transcript by the real `academic_transcription::run`, and the document by the
//! real `P2-L4` builder, so section 23's first exposure source names a node of a
//! document `P2-L4` produced rather than a string this suite invented.

#[path = "../../knowledge-state/tests/common/lecture.rs"]
mod lecture_fixture;

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_blind_spot::{
    BLIND_SPOT_STATES, BelowMinimum, BlindSpotError, BlindSpotFinding, BlindSpotFindingWire,
    BlindSpotScope, BlindSpotState, CANNOT_INFER_ABILITY, CLAIM_ABOUT_THE_PERSON,
    DISPOSITION_PREDICATE, DISPOSITIONS, DispositionLedger, EMPHASIS, EXPOSURE_CLASSES,
    EXPOSURE_SOURCES, EvidenceDiversity, ExposureItem, ExposureSource, FINDING_FIELDS,
    FieldCoverage, FieldResolver, FindingPresentation, GRANULARITIES, GoalBlock, GoalRelevance,
    KeyReading, LOW_RECENCY_BANDS, LowRecency, NOT_A_JUDGEMENT_OF_ABILITY, ObservationWindow,
    ObservedDifficulty, SCHEMA_EXAMPLE_DISPOSITION, ScopeExclusion, StateBasis, TASTE_STEPS,
    TastePath, TasteStep, TaxonomyGranularity, UserDisposition, UserDispositionChoice, detect,
    headline, renderable_copy, state_of,
};
use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ContentDigest, EntityId, EpistemicStatus,
    EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength, FreshnessBand,
    MasteryLevel, PredicateId, ScopeId, TimestampMillis, ValidInterval,
    entity_registry::EntityKind,
    ontology::{Concept, Field, Operation, TaxonomyNode, TaxonomySource, VersionedTaxonomyImport},
};
use academic_knowledge_state::{
    ConceptEvidence, ConceptLink, EligibilityOutcome, EligibleEvidence, EvidenceDossier,
    ExerciseOutcome, IncidentRepair, Outcome, Participation, SelfExplanation, SourceIntegrity,
    TeachingSite, UserConfirmation,
};
use academic_lecture_document::{LectureDocument, NodeId};

type TestResult = Result<(), Box<dyn Error>>;

/// Milliseconds in a day.
const DAY: i64 = 86_400_000;

/// The instant every fixture is dated from, as Unix milliseconds.
const EPOCH: i64 = 1_752_278_400_000;

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
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// The body of section 23, from its heading to the next top-level one.
fn section_23(page: &str) -> Result<String, Box<dyn Error>> {
    let start = page
        .find("## 23. Blind Spot Detector")
        .ok_or("the design document has no section 23")?;
    let rest = &page[start..];
    let end = rest[1..]
        .find("\n## ")
        .map_or(rest.len(), |offset| offset + 1);
    Ok(rest[..end].to_owned())
}

/// The body of section 34.5.
fn section_34_5(page: &str) -> Result<String, Box<dyn Error>> {
    let start = page
        .find("### 34.5 Planning, Blind Spot, Career")
        .ok_or("the design document has no section 34.5")?;
    let rest = &page[start..];
    let end = rest[1..]
        .find("\n### ")
        .map_or(rest.len(), |offset| offset + 1);
    Ok(rest[..end].to_owned())
}

/// The first fenced block of `language` inside `body`.
fn fenced_block(body: &str, language: &str) -> Result<String, Box<dyn Error>> {
    let opener = format!("```{language}\n");
    let start = body
        .find(&opener)
        .ok_or_else(|| format!("no ```{language} block"))?
        + opener.len();
    let end = body[start..]
        .find("```")
        .ok_or_else(|| format!("the ```{language} block does not close"))?;
    Ok(body[start..start + end].to_owned())
}

/// The one line of `body` that begins with `- ` and contains `needle`.
fn bullet_naming(body: &str, needle: &str) -> Result<String, Box<dyn Error>> {
    body.lines()
        .find(|line| line.trim_start().starts_with("- ") && line.contains(needle))
        .map(|line| line.trim().to_owned())
        .ok_or_else(|| format!("no bullet names {needle}").into())
}

/// Every back-quoted run inside `text`.
fn back_quoted(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

// ---------------------------------------------------------------------------
// Domain fixtures.
// ---------------------------------------------------------------------------

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

fn scope_id() -> ScopeId {
    ScopeId::try_from_uuid(uuid_of("scope-blind-spot"))
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

fn engine_actor() -> Actor {
    Actor::DeterministicEngine {
        name: "blind-spot-fixture".to_owned(),
        version: "1".to_owned(),
    }
}

fn importer_actor() -> Actor {
    Actor::Importer {
        name: "blind-spot-fixture".to_owned(),
        version: "1".to_owned(),
    }
}

fn at(days_after: i64) -> TimestampMillis {
    TimestampMillis::new(EPOCH + days_after * DAY)
}

/// The error a refusal returned.
///
/// `unwrap_err` panics on `Ok`, which the workspace lints refuse; this reports
/// the unrefused value instead, so a constructor that stopped refusing shows the
/// value it admitted rather than a bare panic.
fn refused<T: std::fmt::Debug>(
    outcome: Result<T, BlindSpotError>,
) -> Result<BlindSpotError, Box<dyn Error>> {
    match outcome {
        Ok(value) => Err(format!("expected a refusal, got {value:?}").into()),
        Err(error) => Ok(error),
    }
}

/// The same, for a fixture helper that boxes its error.
fn refused_text<T: std::fmt::Debug>(
    outcome: Result<T, Box<dyn Error>>,
) -> Result<String, Box<dyn Error>> {
    match outcome {
        Ok(value) => Err(format!("expected a refusal, got {value:?}").into()),
        Err(error) => Ok(error.to_string()),
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

// -- The taxonomy -----------------------------------------------------------

/// Section 23's example names two crowded areas and two empty ones. The import
/// carries all four as fields, with one concept each and one operation, so the
/// resolver has something to resolve at every granularity.
struct Taxonomy {
    import: VersionedTaxonomyImport,
}

const BACKEND: &str = "Application/Backend";
const DATABASES: &str = "Database Systems";
const GRAPHICS: &str = "Graphics";
const FORMAL: &str = "Formal Methods";

fn taxonomy() -> Result<Taxonomy, Box<dyn Error>> {
    let mut nodes = Vec::new();
    for label in [BACKEND, DATABASES, GRAPHICS, FORMAL] {
        nodes.push(TaxonomyNode::Field(Field::new(entity(label), label)?));
        nodes.push(TaxonomyNode::Concept(Concept::new(
            entity(&format!("{label}/concept")),
            format!("{label} concept"),
            entity(label),
        )?));
    }
    nodes.push(TaxonomyNode::Operation(Operation::new(
        entity("Database Systems/operation"),
        "B+ Tree node split",
        entity(&format!("{DATABASES}/concept")),
    )?));
    Ok(Taxonomy {
        import: VersionedTaxonomyImport::from_nodes(
            entity("undergraduate-cs-breadth"),
            TaxonomySource::Curriculum,
            // Section 23's own example scope names this release.
            "undergraduate CS breadth v2",
            nodes,
        )?,
    })
}

fn concept_of(label: &str) -> EntityId {
    entity(&format!("{label}/concept"))
}

fn scope_over(taxonomy: &Taxonomy, minimum: u32) -> Result<BlindSpotScope, Box<dyn Error>> {
    Ok(BlindSpotScope::select(
        taxonomy.import.identity().clone(),
        TaxonomyGranularity::Field,
        ObservationWindow::AllTime,
        minimum,
    )?)
}

fn resolver_at(taxonomy: &Taxonomy, granularity: TaxonomyGranularity) -> FieldResolver {
    FieldResolver::of(&taxonomy.import, granularity)
}

// -- Admitted evidence ------------------------------------------------------

fn dossier(concept: EntityId, outcome: Outcome) -> EvidenceDossier {
    EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Authored,
        outcome,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    )
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

/// A `P2-L4` document, built by driving a real `P2-L2` capture and `P2-L3` run.
struct Lecture {
    _directory: tempfile::TempDir,
    document: LectureDocument,
}

fn lecture_document(tag: &str) -> Result<Lecture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let capture = lecture_fixture::clean_capture(&directory, tag)?;
    let manifest = lecture_fixture::full_manifest(&capture)?;
    let transcribed = lecture_fixture::transcribe(&manifest)?;
    let capture_seq = lecture_fixture::capture_frame_seq(&capture)
        .ok_or("the fixture capture holds no board photograph")?;
    let document = lecture_fixture::whole_document(transcribed.lineage(), &manifest, capture_seq)?;
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

/// Section 23's first source: a real `P2-L4` document node.
fn lecture_item(
    concept: EntityId,
    tag: &str,
    days_after: i64,
) -> Result<ExposureItem, Box<dyn Error>> {
    let lecture = lecture_document(tag)?;
    let evidence = admitted(
        ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
        tag,
        &dossier(concept, Outcome::Succeeded),
    )?;
    Ok(ExposureItem::of(
        evidence,
        ExposureSource::Lecture,
        at(days_after),
    ))
}

/// Section 23's second source: an exercise whose outcome is recorded.
fn assignment_item(
    concept: EntityId,
    tag: &str,
    outcome: Outcome,
    days_after: i64,
) -> Result<ExposureItem, Box<dyn Error>> {
    let attempt = match outcome {
        Outcome::Succeeded => ExerciseOutcome::succeeded(evidence_id(tag)),
        _ => ExerciseOutcome::failed(evidence_id(tag)),
    };
    let evidence = admitted(
        ConceptEvidence::ConceptExercise(attempt),
        tag,
        &dossier(concept, outcome),
    )?;
    Ok(ExposureItem::of(
        evidence,
        ExposureSource::Assignment,
        at(days_after),
    ))
}

/// Section 23's third source: section 13.2's incident-debugging row.
fn project_item(
    concept: EntityId,
    tag: &str,
    days_after: i64,
) -> Result<ExposureItem, Box<dyn Error>> {
    let evidence = admitted(
        ConceptEvidence::IncidentDebugging(IncidentRepair::of(
            evidence_id(&format!("{tag}-incident")),
            evidence_id(&format!("{tag}-cause")),
            evidence_id(&format!("{tag}-fix")),
            evidence_id(&format!("{tag}-verified")),
        )),
        tag,
        &dossier(concept, Outcome::Succeeded),
    )?;
    Ok(ExposureItem::of(
        evidence,
        ExposureSource::Project,
        at(days_after),
    ))
}

/// Section 23's fifth source: the user's own explanation and confirmation.
fn user_confirmation_item(
    concept: EntityId,
    tag: &str,
    days_after: i64,
) -> Result<ExposureItem, Box<dyn Error>> {
    let evidence = admitted(
        ConceptEvidence::SelfExplanation(SelfExplanation::confirmed_by(
            evidence_id(tag),
            &user_confirmation(concept, tag)?,
        )),
        tag,
        &dossier(concept, Outcome::Succeeded),
    )?;
    Ok(ExposureItem::of(
        evidence,
        ExposureSource::UserConfirmation,
        at(days_after),
    ))
}

/// The user's own confirmation, verified through ADR-003's matrix, so section
/// 23's fifth source is `P2-N2`'s own `사용자 자신의 설명 + 자기 확인` row.
fn user_confirmation(concept: EntityId, tag: &str) -> Result<UserConfirmation, Box<dyn Error>> {
    let evidence = evidence_item(&format!("{tag}-confirmation"));
    let claim = Claim {
        id: claim_id(&format!("{tag}-confirm")),
        subject_entity_id: concept,
        predicate_id: PredicateId::parse(academic_knowledge_state::STATE_CONFIRMATION_PREDICATE)?,
        object: ClaimObject::Mastery(MasteryLevel::Understood),
        scope_id: scope_id(),
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
        at(0),
    )?)
}

/// Section 23's fourth source, carried on an exercise the user's own question
/// produced. Section 23's five are provenance; section 13.2's eight are what the
/// evidence licenses, and the two axes are independent.
fn question_item(
    concept: EntityId,
    tag: &str,
    days_after: i64,
) -> Result<ExposureItem, Box<dyn Error>> {
    let evidence = admitted(
        ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id(tag))),
        tag,
        &dossier(concept, Outcome::Succeeded),
    )?;
    Ok(ExposureItem::of(
        evidence,
        ExposureSource::Question,
        at(days_after),
    ))
}

// -- Dispositions -----------------------------------------------------------

fn disposition_claim(
    field: EntityId,
    token: &str,
    evidence: &EvidenceItem,
    authority: AuthorityClass,
    status: EpistemicStatus,
    predicate: &str,
) -> Result<Claim, Box<dyn Error>> {
    Ok(Claim {
        id: claim_id(&format!("disposition-{token}-{field:?}")),
        subject_entity_id: field,
        predicate_id: PredicateId::parse(predicate)?,
        object: ClaimObject::Text(token.to_owned()),
        scope_id: scope_id(),
        authority_class: authority,
        epistemic_status: status,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::new(TimestampMillis::new(0), None)?,
        evidence_ids: vec![evidence.id],
    })
}

fn choice(
    field: EntityId,
    disposition: UserDisposition,
    hidden_until: Option<TimestampMillis>,
    chosen_at: TimestampMillis,
) -> Result<UserDispositionChoice, Box<dyn Error>> {
    let evidence = evidence_item(&format!("disposition-{}", disposition.as_str()));
    let claim = disposition_claim(
        field,
        disposition.as_str(),
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        DISPOSITION_PREDICATE,
    )?;
    Ok(UserDispositionChoice::verify(
        &user_actor(),
        &claim,
        &evidence,
        field,
        disposition,
        hidden_until,
        chosen_at,
    )?)
}

// ---------------------------------------------------------------------------
// 1. `five_states_are_semantically_distinct`
// ---------------------------------------------------------------------------

#[test]
fn five_states_are_semantically_distinct() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;

    // The five, and their meanings, read out of section 23's own `text` block.
    let block = fenced_block(&body, "text")?;
    let mut documented: Vec<(String, String)> = Vec::new();
    for line in block.lines().filter(|line| !line.trim().is_empty()) {
        let (name, meaning) = line
            .split_once(": ")
            .ok_or_else(|| format!("the state block line {line:?} has no `: ` separator"))?;
        documented.push((name.trim().to_owned(), meaning.trim().to_owned()));
    }
    assert_eq!(
        documented.len(),
        5,
        "section 23's state block has {} lines",
        documented.len()
    );

    // Both directions, name and meaning cell alike.
    let held: Vec<(String, String)> = BLIND_SPOT_STATES
        .iter()
        .map(|state| (state.as_str().to_owned(), state.meaning().to_owned()))
        .collect();
    assert_eq!(held, documented, "the states and section 23 disagree");

    // Distinct is a different precondition, not a different spelling: the five
    // bases map onto the five states as a bijection.
    let bases = every_basis()?;
    let mut reached: Vec<BlindSpotState> = bases.iter().map(state_of).collect();
    assert_eq!(
        reached.len(),
        BLIND_SPOT_STATES.len(),
        "there is not one basis per state"
    );
    reached.sort_unstable();
    reached.dedup();
    assert_eq!(
        reached,
        {
            let mut all = BLIND_SPOT_STATES.to_vec();
            all.sort_unstable();
            all
        },
        "the bases do not cover the five states injectively"
    );

    // Coverage may reach exactly three of them, and the other two are exactly
    // the complement.
    let from_coverage: BTreeSet<BlindSpotState> = EXPOSURE_CLASSES
        .iter()
        .map(|class| BlindSpotState::from(*class))
        .collect();
    let all: BTreeSet<BlindSpotState> = BLIND_SPOT_STATES.iter().copied().collect();
    assert!(
        from_coverage.is_subset(&all),
        "an exposure class is not one of section 23's five states"
    );
    let complement: BTreeSet<BlindSpotState> = all.difference(&from_coverage).copied().collect();
    assert_eq!(
        complement,
        BTreeSet::from([BlindSpotState::OutOfScope, BlindSpotState::Gap]),
        "coverage reaches the wrong subset of the five"
    );

    // Each basis payload refuses the facts its own state is not.
    assert_eq!(
        refused(BelowMinimum::of(3, 2))?,
        BlindSpotError::CoverageIsNotBelowMinimum {
            observed: 3,
            minimum: 2
        }
    );
    assert_eq!(
        refused(ObservedDifficulty::of(Vec::new()))?,
        BlindSpotError::DifficultyHasNoAttempt
    );
    for band in [
        FreshnessBand::Unknown,
        FreshnessBand::Moderate,
        FreshnessBand::High,
        FreshnessBand::VeryHigh,
    ] {
        assert_eq!(
            refused(LowRecency::of(band))?,
            BlindSpotError::BandIsNotLowRecency(band),
            "{band:?} was admitted as low recency"
        );
    }
    for band in LOW_RECENCY_BANDS {
        assert_eq!(LowRecency::of(band)?.band(), band);
    }
    for disposition in DISPOSITIONS
        .iter()
        .filter(|held| **held != UserDisposition::NotRelevant)
    {
        let deadline = disposition.needs_deadline().then(|| at(30)).or(None);
        let other = choice(entity(GRAPHICS), *disposition, deadline, at(0))?;
        assert_eq!(
            refused(ScopeExclusion::of(&other))?,
            BlindSpotError::ExclusionNeedsNotRelevant,
            "{disposition:?} produced a scope exclusion"
        );
    }
    assert_eq!(
        refused(GoalBlock::of(entity("goal"), entity("goal")))?,
        BlindSpotError::GoalBlocksItself
    );

    // Section 23's schema block, in both directions.
    let schema = fenced_block(&body, "yaml")?;
    let keys: Vec<String> = schema
        .lines()
        .filter_map(|line| {
            let indented = line.starts_with("  ") && !line.starts_with("   ");
            indented
                .then(|| line.trim().split_once(':').map(|(key, _)| key.to_owned()))
                .flatten()
        })
        .collect();
    assert_eq!(
        keys,
        FINDING_FIELDS
            .iter()
            .map(|field| (*field).to_owned())
            .collect::<Vec<_>>(),
        "the schema keys and FINDING_FIELDS disagree"
    );

    Ok(())
}

/// One value of each [`StateBasis`] variant, built through its own constructor.
fn every_basis() -> Result<Vec<StateBasis>, Box<dyn Error>> {
    let excluded = choice(entity(GRAPHICS), UserDisposition::NotRelevant, None, at(0))?;
    Ok(vec![
        StateBasis::CoverageBelowMinimum(BelowMinimum::of(1, 2)?),
        StateBasis::DifficultyObserved(ObservedDifficulty::of(vec![evidence_id("failed")])?),
        StateBasis::RecencyLow(LowRecency::of(FreshnessBand::Stale)?),
        StateBasis::UserExcluded(ScopeExclusion::of(&excluded)?),
        StateBasis::ActiveGoalBlocked(GoalBlock::of(entity("goal"), entity("blocker"))?),
    ])
}

// ---------------------------------------------------------------------------
// 2. `unobserved_says_cannot_infer_ability`
// ---------------------------------------------------------------------------

#[test]
fn unobserved_says_cannot_infer_ability() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;

    // Section 23's own bullet, read for both halves of its own sentence.
    let bullet = bullet_naming(&body, "대신")?;
    let quoted = quoted_runs(&bullet);
    assert_eq!(
        quoted,
        vec![
            CLAIM_ABOUT_THE_PERSON.to_owned(),
            CANNOT_INFER_ABILITY.to_owned()
        ],
        "the bullet's two halves and this crate's two constants disagree"
    );

    // The `UNOBSERVED` cell is a statement about the record, and the crate says
    // the replacement phrase rather than the cell.
    assert_eq!(headline(BlindSpotState::Unobserved), CANNOT_INFER_ABILITY);
    assert!(
        BlindSpotState::Unobserved
            .meaning()
            .contains("말할 수 없음"),
        "the UNOBSERVED cell stopped saying that ability cannot be stated"
    );

    // Nothing this crate can render is the claim it replaces.
    for copy in renderable_copy() {
        assert!(
            !copy.contains(CLAIM_ABOUT_THE_PERSON),
            "{copy:?} says {CLAIM_ABOUT_THE_PERSON}"
        );
    }

    // And the same is true of what a real `UNOBSERVED` finding presents.
    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let readings = vec![KeyReading::of(entity(GRAPHICS), Vec::new())];
    let findings = detect(
        &scope,
        &resolver_at(&taxonomy, TaxonomyGranularity::Field),
        &DispositionLedger::new(),
        &readings,
        at(1),
    )?;
    let finding = one(&findings)?;
    assert_eq!(finding.classification(), BlindSpotState::Unobserved);
    assert_eq!(
        finding.presentation().presentation().headline(),
        CANNOT_INFER_ABILITY
    );
    assert_eq!(
        finding.presentation().presentation().uncertainty(),
        NOT_A_JUDGEMENT_OF_ABILITY
    );
    assert!(matches!(
        finding.basis(),
        StateBasis::CoverageBelowMinimum(_)
    ));

    // Section 34.5's uncertainty cell is where the second phrase comes from.
    let failure = section_34_5(&page)?;
    let row = failure
        .lines()
        .find(|line| line.contains("Blind Spot을 공부 압박으로 변환"))
        .ok_or("section 34.5 has no blind-spot row")?;
    assert!(
        row.contains(NOT_A_JUDGEMENT_OF_ABILITY),
        "section 34.5's row no longer carries {NOT_A_JUDGEMENT_OF_ABILITY}"
    );

    Ok(())
}

/// Every run inside a pair of typographic double quotes.
fn quoted_runs(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('\u{201c}') {
        let after = &rest[open + '\u{201c}'.len_utf8()..];
        let Some(close) = after.find('\u{201d}') else {
            break;
        };
        found.push(after[..close].to_owned());
        rest = &after[close + '\u{201d}'.len_utf8()..];
    }
    found
}

fn one(findings: &[BlindSpotFinding]) -> Result<&BlindSpotFinding, Box<dyn Error>> {
    match findings {
        [only] => Ok(only),
        other => Err(format!("expected one finding, got {}", other.len()).into()),
    }
}

// ---------------------------------------------------------------------------
// 3. `coverage_never_becomes_mastery`
// ---------------------------------------------------------------------------

#[test]
fn coverage_never_becomes_mastery() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;

    // Section 23's own coverage sentence, in both directions.
    let sentence = body
        .lines()
        .find(|line| line.starts_with("Field별 coverage는"))
        .ok_or("section 23 has no coverage sentence")?;
    let head = sentence
        .split_once(" evidence의 존재와 다양성을 집계하되")
        .ok_or("the coverage sentence does not name what it aggregates")?
        .0;
    let documented: Vec<String> = head
        .trim_start_matches("Field별 coverage는")
        .trim()
        .split('·')
        .map(|name| name.trim().to_owned())
        .collect();
    assert_eq!(
        documented,
        EXPOSURE_SOURCES
            .iter()
            .map(|source| source.design_token().to_owned())
            .collect::<Vec<_>>(),
        "the sources and section 23's own sentence disagree"
    );
    assert!(
        sentence.contains("mastery 점수로 바꾸지 않는다"),
        "section 23 no longer forbids the conversion"
    );

    // A field with every one of the five sources present is still not a level.
    let taxonomy = taxonomy()?;
    let concept = concept_of(BACKEND);
    let items = vec![
        lecture_item(concept, "backend-lecture", 0)?,
        assignment_item(concept, "backend-assignment", Outcome::Succeeded, 1)?,
        project_item(concept, "backend-project", 2)?,
        question_item(concept, "backend-question", 3)?,
        user_confirmation_item(concept, "backend-confirmation", 4)?,
    ];
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let rich = FieldCoverage::of(entity(BACKEND), &scope, &resolver, &items)?;
    assert_eq!(rich.evidence_count(), 5);
    assert_eq!(
        rich.sources(),
        EXPOSURE_SOURCES.iter().copied().collect::<BTreeSet<_>>()
    );
    assert_eq!(rich.diversity(), EvidenceDiversity::Mixed);

    // Section 23's own example: one item, one source, LOW.
    let thin = FieldCoverage::of(
        concept_of(GRAPHICS),
        &BlindSpotScope::select(
            taxonomy.import.identity().clone(),
            TaxonomyGranularity::Concept,
            ObservationWindow::AllTime,
            2,
        )?,
        &resolver_at(&taxonomy, TaxonomyGranularity::Concept),
        &[assignment_item(
            concept_of(GRAPHICS),
            "graphics-assignment",
            Outcome::Succeeded,
            0,
        )?],
    )?;
    let schema = fenced_block(&body, "yaml")?;
    let example_count: u32 = schema
        .lines()
        .find_map(|line| line.trim().strip_prefix("exposureEvidenceCount:"))
        .ok_or("the schema example has no exposureEvidenceCount")?
        .trim()
        .parse()?;
    let example_diversity = schema
        .lines()
        .find_map(|line| line.trim().strip_prefix("evidenceDiversity:"))
        .ok_or("the schema example has no evidenceDiversity")?
        .trim()
        .to_owned();
    assert_eq!(thin.evidence_count(), example_count);
    assert_eq!(thin.diversity().as_str(), example_diversity);

    // The two readings differ in every way a coverage reading can differ, and
    // the type still refuses to rank them: `FieldCoverage` and
    // `EvidenceDiversity` implement neither comparison trait, which
    // `crates/blind-spot/tests/compile_fail/` holds as compiled evidence, and
    // `blind_spot_scans.rs` holds the stronger claim that this crate has no name
    // for a mastery level at all.
    assert_ne!(rich, thin);

    // An item about another key is refused rather than absorbed into the count.
    let other = assignment_item(
        concept_of(DATABASES),
        "database-assignment",
        Outcome::Succeeded,
        0,
    )?;
    assert_eq!(
        refused(FieldCoverage::of(
            entity(BACKEND),
            &scope,
            &resolver,
            &[other]
        ))?,
        BlindSpotError::ItemIsAboutAnotherKey {
            expected: entity(BACKEND),
            found: entity(DATABASES),
        }
    );

    // And so is an item about an entity this taxonomy release does not hold.
    let stranger = assignment_item(
        entity("not-in-this-release"),
        "stranger",
        Outcome::Succeeded,
        0,
    )?;
    assert_eq!(
        refused(FieldCoverage::of(
            entity(BACKEND),
            &scope,
            &resolver,
            &[stranger]
        ))?,
        BlindSpotError::ItemIsOutsideTheTaxonomy(evidence_id("stranger"))
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `granularity_and_window_are_user_selected`
// ---------------------------------------------------------------------------

#[test]
fn granularity_and_window_are_user_selected() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;
    assert!(
        body.contains("taxonomy granularity와 기간을 사용자가 선택한다"),
        "section 23 no longer says the user selects the granularity and the window"
    );

    let taxonomy = taxonomy()?;

    // The same evidence, read at three granularities, aggregates under three
    // different keys, so the choice is not cosmetic.
    let operation = entity("Database Systems/operation");
    let item = assignment_item(operation, "operation-assignment", Outcome::Succeeded, 0)?;
    let expected = [
        (TaxonomyGranularity::Field, entity(DATABASES)),
        (TaxonomyGranularity::Concept, concept_of(DATABASES)),
        (TaxonomyGranularity::Operation, operation),
    ];
    assert_eq!(
        GRANULARITIES.to_vec(),
        expected
            .iter()
            .map(|(granularity, _)| *granularity)
            .collect::<Vec<_>>(),
        "GRANULARITIES and the cases driven here disagree"
    );
    for (granularity, key) in expected {
        let resolver = resolver_at(&taxonomy, granularity);
        assert_eq!(resolver.resolve(operation), Some(key));
        let scope = BlindSpotScope::select(
            taxonomy.import.identity().clone(),
            granularity,
            ObservationWindow::AllTime,
            2,
        )?;
        let counted = FieldCoverage::of(key, &scope, &resolver, std::slice::from_ref(&item))?;
        assert_eq!(counted.evidence_count(), 1);
        assert_eq!(counted.key(), key);
    }
    // A node above the selected tier resolves to nothing at all.
    assert_eq!(
        resolver_at(&taxonomy, TaxonomyGranularity::Operation).resolve(concept_of(DATABASES)),
        None
    );
    assert_eq!(
        resolver_at(&taxonomy, TaxonomyGranularity::Concept).resolve(entity(DATABASES)),
        None
    );

    // The window is the user's too, and it excludes rather than reweights.
    let concept = concept_of(BACKEND);
    let items = vec![
        assignment_item(concept, "in-window", Outcome::Succeeded, 1)?,
        assignment_item(concept, "out-of-window", Outcome::Succeeded, 100)?,
    ];
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let all_time = scope_over(&taxonomy, 2)?;
    assert_eq!(
        FieldCoverage::of(entity(BACKEND), &all_time, &resolver, &items)?.evidence_count(),
        2
    );
    let bounded = BlindSpotScope::select(
        taxonomy.import.identity().clone(),
        TaxonomyGranularity::Field,
        ObservationWindow::between(at(0), at(10))?,
        2,
    )?;
    assert_eq!(
        FieldCoverage::of(entity(BACKEND), &bounded, &resolver, &items)?.evidence_count(),
        1
    );

    // Section 23's own example scope string is the two halves the user chose.
    let schema = fenced_block(&body, "yaml")?;
    let example_scope = schema
        .lines()
        .find_map(|line| line.trim().strip_prefix("scope:"))
        .ok_or("the schema example has no scope")?
        .trim()
        .trim_matches('"')
        .to_owned();
    assert_eq!(all_time.label(), example_scope);

    // The threshold is the user's as well, and zero is refused.
    assert_eq!(
        refused(BlindSpotScope::select(
            taxonomy.import.identity().clone(),
            TaxonomyGranularity::Field,
            ObservationWindow::AllTime,
            0,
        ))?,
        BlindSpotError::MinimumExposureIsZero
    );
    assert_eq!(
        refused(ObservationWindow::between(at(10), at(10)))?,
        BlindSpotError::WindowIsEmpty
    );

    // And the same field is `UNOBSERVED` under one minimum and not the other,
    // which is what makes the fourth choice a choice.
    let thin = vec![assignment_item(concept, "one-item", Outcome::Succeeded, 1)?];
    let readings = vec![KeyReading::of(entity(BACKEND), thin)];
    for (minimum, expected) in [(2_u32, Some(BlindSpotState::Unobserved)), (1, None)] {
        let scope = scope_over(&taxonomy, minimum)?;
        let findings = detect(
            &scope,
            &resolver,
            &DispositionLedger::new(),
            &readings,
            at(2),
        )?;
        assert_eq!(
            findings.first().map(BlindSpotFinding::classification),
            expected,
            "minimum {minimum} classified the field wrongly"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `four_dispositions_are_durable`
// ---------------------------------------------------------------------------

#[test]
fn four_dispositions_are_durable() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;

    // Section 23's own bullet, in both directions.
    let bullet = bullet_naming(&body, "선택한다")?;
    assert_eq!(
        back_quoted(&bullet),
        DISPOSITIONS
            .iter()
            .map(|held| held.as_str().to_owned())
            .collect::<Vec<_>>(),
        "the dispositions and section 23's bullet disagree"
    );

    // The schema example writes a spelling the bullet does not list, and this
    // crate keeps it as a measured discrepancy rather than a fifth disposition.
    let schema = fenced_block(&body, "yaml")?;
    let example = schema
        .lines()
        .find_map(|line| line.trim().strip_prefix("userDisposition:"))
        .ok_or("the schema example has no userDisposition")?
        .trim()
        .to_owned();
    assert_eq!(example, SCHEMA_EXAMPLE_DISPOSITION);
    assert!(
        !DISPOSITIONS
            .iter()
            .any(|held| held.as_str() == SCHEMA_EXAMPLE_DISPOSITION),
        "the example's spelling became a fifth disposition"
    );

    // A model run cannot mint any of the four, in either pairing.
    let field = entity(GRAPHICS);
    for disposition in DISPOSITIONS {
        let deadline = disposition.needs_deadline().then(|| at(30));
        let evidence = evidence_item("forged");
        for (actor, authority, status) in [
            (
                model_actor(),
                AuthorityClass::UserExplicit,
                EpistemicStatus::UserConfirmed,
            ),
            (
                model_actor(),
                AuthorityClass::ModelInference,
                EpistemicStatus::AiInferred,
            ),
            (
                engine_actor(),
                AuthorityClass::DeterministicEngine,
                EpistemicStatus::DeterministicDerived,
            ),
            (
                importer_actor(),
                AuthorityClass::UserExplicit,
                EpistemicStatus::UserConfirmed,
            ),
        ] {
            let claim = disposition_claim(
                field,
                disposition.as_str(),
                &evidence,
                authority,
                status,
                DISPOSITION_PREDICATE,
            )?;
            assert!(
                UserDispositionChoice::verify(
                    &actor,
                    &claim,
                    &evidence,
                    field,
                    disposition,
                    deadline,
                    at(0),
                )
                .is_err(),
                "{actor:?} minted {disposition:?}"
            );
        }
        // The clean user pairing is the only one that produces the value.
        let held = choice(field, disposition, deadline, at(0))?;
        assert_eq!(held.disposition(), disposition);
        assert_eq!(held.field(), field);
        assert_eq!(held.hidden_until(), deadline);
    }

    // The deadline rule runs in both directions.
    assert_eq!(
        refused_text(choice(field, UserDisposition::HideUntil, None, at(0)))?,
        BlindSpotError::DeadlineRequired.to_string()
    );
    assert_eq!(
        refused_text(choice(field, UserDisposition::Later, Some(at(30)), at(0)))?,
        BlindSpotError::DeadlineNotAllowed(UserDisposition::Later).to_string()
    );
    assert_eq!(
        refused_text(choice(
            field,
            UserDisposition::HideUntil,
            Some(at(0)),
            at(0)
        ))?,
        BlindSpotError::DeadlineIsNotInTheFuture.to_string()
    );

    // A claim about another field is refused.
    let evidence = evidence_item("elsewhere");
    let claim = disposition_claim(
        entity(FORMAL),
        UserDisposition::Later.as_str(),
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        DISPOSITION_PREDICATE,
    )?;
    assert_eq!(
        refused(UserDispositionChoice::verify(
            &user_actor(),
            &claim,
            &evidence,
            field,
            UserDisposition::Later,
            None,
            at(0),
        ))?,
        BlindSpotError::DispositionSubjectMismatch
    );

    // Durable: every one of the four survives a recomputation over new inputs,
    // and a replayed older claim cannot undo a later decision.
    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    for disposition in DISPOSITIONS {
        let deadline = disposition.needs_deadline().then(|| at(30));
        let ledger =
            DispositionLedger::new().record(choice(field, disposition, deadline, at(0))?)?;
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.fields(), vec![field]);
        let mut carried = ledger.clone();
        for run in 0..3 {
            let readings = vec![
                KeyReading::of(field, Vec::new()).with_taste_step(TasteStep::OneToyExperiment),
            ];
            let findings = detect(&scope, &resolver, &carried, &readings, at(run))?;
            assert_eq!(
                one(&findings)?.user_disposition(),
                Some(disposition),
                "{disposition:?} did not survive run {run}"
            );
            carried = carried.clone();
        }
        assert_eq!(
            refused(
                carried
                    .clone()
                    .record(choice(field, UserDisposition::Later, None, at(0))?),
            )?,
            BlindSpotError::DispositionIsOlderThanTheOneItReplaces
        );
        // The user may change their mind, forward in time.
        let changed = carried.record(choice(field, UserDisposition::Later, None, at(1))?)?;
        assert_eq!(
            changed.standing(field).map(|held| held.disposition()),
            Some(UserDisposition::Later)
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `hide_until_reappears_after_clock_advance`
// ---------------------------------------------------------------------------

#[test]
fn hide_until_reappears_after_clock_advance() -> TestResult {
    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let field = entity(GRAPHICS);
    let deadline = at(30);
    let ledger = DispositionLedger::new().record(choice(
        field,
        UserDisposition::HideUntil,
        Some(deadline),
        at(0),
    )?)?;
    let readings = vec![KeyReading::of(field, Vec::new())];

    // The clock arrives as an argument; this crate reads none.
    let swept = [
        (at(1), false),
        (at(29), false),
        (deadline, true),
        (at(31), true),
        (at(400), true),
    ];
    for (as_of, expected) in swept {
        let findings = detect(&scope, &resolver, &ledger, &readings, as_of)?;
        let finding = one(&findings)?;
        assert_eq!(
            finding.warns(),
            expected,
            "at {as_of:?} the warning state was wrong"
        );
        // The finding itself never went away, and neither did the disposition:
        // section 39 stores the evidence class and the disposition separately.
        assert_eq!(finding.classification(), BlindSpotState::Unobserved);
        assert_eq!(finding.user_disposition(), Some(UserDisposition::HideUntil));
    }

    // `NOT_RELEVANT` is the one that never comes back, over the same sweep.
    let permanent = DispositionLedger::new().record(choice(
        field,
        UserDisposition::NotRelevant,
        None,
        at(0),
    )?)?;
    for (as_of, _) in swept {
        let findings = detect(&scope, &resolver, &permanent, &readings, as_of)?;
        let finding = one(&findings)?;
        assert!(!finding.warns(), "NOT_RELEVANT warned at {as_of:?}");
        assert_eq!(finding.classification(), BlindSpotState::OutOfScope);
    }

    // And a field with no disposition warns at every one of those instants, so
    // the sweep above measures the deadline rather than a detector that never
    // warns.
    for (as_of, _) in swept {
        let findings = detect(
            &scope,
            &resolver,
            &DispositionLedger::new(),
            &readings,
            as_of,
        )?;
        assert!(
            one(&findings)?.warns(),
            "the control did not warn at {as_of:?}"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `not_relevant_survives_ai_rerun`
// ---------------------------------------------------------------------------

#[test]
fn not_relevant_survives_ai_rerun() -> TestResult {
    let page = specification()?;
    assert!(
        page.contains("`NOT_RELEVANT`는 존중되며 새로운 AI run이 경고를 되살리지 않는다"),
        "section 25.12 no longer says a new AI run must not resurrect the warning"
    );

    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let field = entity(GRAPHICS);
    let concept = concept_of(GRAPHICS);
    let ledger = DispositionLedger::new().record(choice(
        field,
        UserDisposition::NotRelevant,
        None,
        at(0),
    )?)?;

    // Every rerun this crate has. `blind_spot_scans.rs` holds the whole-set
    // claim that `detect` is the only producer of a finding; this drives it over
    // inputs a later AI run could plausibly hand it, including ones that would
    // change the classification if the disposition were ignored.
    let reruns: Vec<(&str, Vec<KeyReading>)> = vec![
        (
            "no evidence at all",
            vec![KeyReading::of(field, Vec::new())],
        ),
        (
            "new evidence that clears the minimum",
            vec![KeyReading::of(
                field,
                vec![
                    lecture_item(concept, "rerun-lecture", 1)?,
                    project_item(concept, "rerun-project", 2)?,
                    question_item(concept, "rerun-question", 3)?,
                ],
            )],
        ),
        (
            "a failed attempt, which would otherwise be WEAK",
            vec![KeyReading::of(
                field,
                vec![
                    assignment_item(concept, "rerun-failed-a", Outcome::Failed, 1)?,
                    assignment_item(concept, "rerun-failed-b", Outcome::Failed, 2)?,
                ],
            )],
        ),
        (
            "a stale band, which would otherwise be STALE",
            vec![
                KeyReading::of(
                    field,
                    vec![
                        lecture_item(concept, "rerun-stale-a", 1)?,
                        project_item(concept, "rerun-stale-b", 2)?,
                    ],
                )
                .with_band(FreshnessBand::Stale),
            ],
        ),
        (
            "a blocked active goal, which would otherwise be GAP",
            vec![
                KeyReading::of(field, Vec::new())
                    .with_goal_block(GoalBlock::of(entity("goal"), concept)?)
                    .with_relevance(GoalRelevance::of(BTreeSet::from([entity("goal")]))),
            ],
        ),
    ];
    for (what, readings) in &reruns {
        let findings = detect(&scope, &resolver, &ledger, readings, at(500))?;
        let finding = one(&findings)?;
        assert_eq!(
            finding.classification(),
            BlindSpotState::OutOfScope,
            "{what} changed the classification"
        );
        assert_eq!(
            finding.user_disposition(),
            Some(UserDisposition::NotRelevant),
            "{what} cleared the disposition"
        );
        assert!(!finding.warns(), "{what} resurrected the warning");
        assert!(
            finding.presentation().path().is_none(),
            "{what} produced an action"
        );
    }

    // The positive control: with no disposition recorded, four of those five
    // inputs classify differently, so the sweep above is measuring the ledger.
    let mut classified = Vec::new();
    for (_, readings) in &reruns {
        let findings = detect(
            &scope,
            &resolver,
            &DispositionLedger::new(),
            readings,
            at(500),
        )?;
        classified.push(findings.first().map(BlindSpotFinding::classification));
    }
    assert_eq!(
        classified,
        vec![
            Some(BlindSpotState::Unobserved),
            None,
            Some(BlindSpotState::Weak),
            Some(BlindSpotState::Stale),
            Some(BlindSpotState::Gap),
        ],
        "the control runs did not exercise the states they were built for"
    );

    // A ledger cannot be edited: the standing choice survives because there is
    // no operation that removes one, and a rerun holding an older claim is
    // refused rather than applied.
    assert_eq!(
        refused(
            ledger
                .clone()
                .record(choice(field, UserDisposition::Explore, None, at(0))?),
        )?,
        BlindSpotError::DispositionIsOlderThanTheOneItReplaces
    );

    // And a wire round-trip preserves section 23's eight fields.
    let findings = detect(&scope, &resolver, &ledger, &reruns[0].1, at(500))?;
    let wire = one(&findings)?.to_wire();
    let json = serde_json::to_string(&wire)?;
    let back: BlindSpotFindingWire = serde_json::from_str(&json)?;
    assert_eq!(back, wire);
    for key in FINDING_FIELDS {
        assert!(
            json.contains(&format!("\"{key}\"")),
            "the wire dropped {key}"
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// 8. `no_equalize_all_goal_is_generated`
// ---------------------------------------------------------------------------

#[test]
fn no_equalize_all_goal_is_generated() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;
    assert!(
        body.contains("모든 분야를 균등하게 채우라는 목표를 만들지 않는다"),
        "section 23 no longer forbids the equalise-everything goal"
    );
    let failure = section_34_5(&page)?;
    assert!(
        failure.contains("모든 taxonomy 영역의 균등 coverage 목표"),
        "section 34.5 no longer names the cause"
    );

    // The most skewed distribution this taxonomy can hold: one field carries
    // every source, three carry nothing.
    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let concept = concept_of(BACKEND);
    let readings = vec![
        KeyReading::of(
            entity(BACKEND),
            vec![
                lecture_item(concept, "skew-lecture", 0)?,
                assignment_item(concept, "skew-assignment", Outcome::Succeeded, 1)?,
                project_item(concept, "skew-project", 2)?,
            ],
        ),
        KeyReading::of(entity(DATABASES), Vec::new()),
        KeyReading::of(entity(GRAPHICS), Vec::new()),
        KeyReading::of(entity(FORMAL), Vec::new()),
    ];
    let findings = detect(
        &scope,
        &resolver,
        &DispositionLedger::new(),
        &readings,
        at(3),
    )?;

    // Three findings, and the crowded field is not one of them: a key that is
    // adequately covered produces nothing at all.
    assert_eq!(findings.len(), 3);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.field() == entity(BACKEND)),
        "the crowded field produced a finding"
    );

    // Nothing in the output is or proposes a goal. The finding's whole wire
    // surface is section 23's eight keys, compared in both directions, so a
    // goal, an objective or a target added later is an extra key here.
    for finding in &findings {
        let value = serde_json::to_value(finding.to_wire())?;
        let object = value
            .as_object()
            .ok_or("a finding did not serialise as an object")?;
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = FINDING_FIELDS.to_vec();
        expected.sort_unstable();
        assert_eq!(keys, expected, "the finding's wire keys are not the eight");
        assert!(finding.presentation().path().is_none());
    }

    // The skew is explained as a distribution: the crowded key, the empty ones,
    // and the counts that made it so. No sentence, and nothing telling the user
    // to fill anything in.
    let cause = findings
        .first()
        .ok_or("no finding to read the cause off")?
        .likely_cause();
    assert_eq!(cause.concentrated(), &[entity(BACKEND)]);
    let mut sparse = vec![entity(DATABASES), entity(GRAPHICS), entity(FORMAL)];
    sparse.sort_unstable();
    assert_eq!(cause.sparse(), sparse.as_slice());
    let drivers: Vec<(EntityId, ExposureSource, u32)> = cause
        .drivers()
        .iter()
        .map(|driver| (driver.key, driver.source, driver.count))
        .collect();
    assert_eq!(
        drivers,
        vec![
            (entity(BACKEND), ExposureSource::Lecture, 1),
            (entity(BACKEND), ExposureSource::Assignment, 1),
            (entity(BACKEND), ExposureSource::Project, 1),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>(),
        "the drivers are not the distribution that produced the skew"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 9. `low_relevance_uses_neutral_tokens`
// ---------------------------------------------------------------------------

#[test]
fn low_relevance_uses_neutral_tokens() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;
    assert!(
        body.contains("warning red가 아니라 중립 outline으로 표시한다"),
        "section 23 no longer forbids the red warning"
    );
    assert!(
        body.contains("진로 목표와 무관하면 행동 요구를 만들지 않는다"),
        "section 23 no longer forbids the action demand"
    );

    // The whole set of strings a finding can render, against the design document
    // itself, in both directions. This is the check that does not depend on
    // anybody having guessed which words a demand would be spelled with: a
    // sentence the document does not contain cannot be rendered, whatever it
    // says.
    let copy = renderable_copy();
    assert_eq!(
        copy.len(),
        6,
        "the renderable copy is {} values",
        copy.len()
    );
    for value in &copy {
        assert!(
            page.contains(value),
            "{value:?} is not a sentence the design document contains"
        );
    }
    let mut from_document: Vec<&'static str> = BLIND_SPOT_STATES
        .iter()
        .map(|state| headline(*state))
        .collect();
    from_document.push(NOT_A_JUDGEMENT_OF_ABILITY);
    from_document.sort_unstable();
    from_document.dedup();
    assert_eq!(copy, from_document);

    // The control: the reader that says the six are in the document says a
    // demanding sentence is not, and says so for a sentence spelled with none of
    // the words any list here would hold.
    for injected in [
        "Graphics 영역의 비중을 이번 학기에 반드시 올리십시오",
        "지금 이 분야를 채우기 위한 계획을 세우세요",
        "이 부분을 방치하면 나중에 곤란해집니다",
    ] {
        assert!(
            !page.contains(injected),
            "the design document contains {injected:?}, so this control is vacuous"
        );
        assert!(!copy.contains(&injected), "{injected:?} is renderable copy");
    }

    // A finding at low relevance shows the neutral emphasis, discloses that it
    // is not a judgement of ability, and has no value in which an action could
    // be carried.
    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let readings = vec![KeyReading::of(entity(FORMAL), Vec::new())];
    let findings = detect(
        &scope,
        &resolver,
        &DispositionLedger::new(),
        &readings,
        at(1),
    )?;
    let finding = one(&findings)?;
    assert!(finding.relevance_to_active_goals().is_low());
    assert_eq!(finding.relevance_to_active_goals().as_str(), "LOW");
    let presentation = finding.presentation();
    assert!(matches!(presentation, FindingPresentation::Neutral { .. }));
    assert!(presentation.path().is_none());
    assert_eq!(presentation.presentation().emphasis(), EMPHASIS);
    assert_eq!(EMPHASIS, "NEUTRAL_OUTLINE");
    assert!(copy.contains(&presentation.presentation().headline()));
    assert!(copy.contains(&presentation.presentation().uncertainty()));

    // And a related field discloses the other token, so `LOW` is a reading
    // rather than a constant.
    let related = vec![
        KeyReading::of(entity(FORMAL), Vec::new())
            .with_relevance(GoalRelevance::of(BTreeSet::from([entity("goal")]))),
    ];
    let findings = detect(
        &scope,
        &resolver,
        &DispositionLedger::new(),
        &related,
        at(1),
    )?;
    let finding = one(&findings)?;
    assert!(!finding.relevance_to_active_goals().is_low());
    assert_eq!(finding.relevance_to_active_goals().as_str(), "RELATED");
    // Relevance changes the disclosure and nothing else: still neutral, still
    // no action.
    assert!(matches!(
        finding.presentation(),
        FindingPresentation::Neutral { .. }
    ));
    assert_eq!(finding.presentation().presentation().emphasis(), EMPHASIS);

    Ok(())
}

// ---------------------------------------------------------------------------
// 10. `explore_creates_one_bounded_taste_path`
// ---------------------------------------------------------------------------

#[test]
fn explore_creates_one_bounded_taste_path() -> TestResult {
    let page = specification()?;
    let body = section_23(&page)?;

    // Section 23's own three, in both directions.
    let bullet = bullet_naming(&body, "taste path")?;
    let between = bullet
        .split('\u{2014}')
        .nth(1)
        .ok_or("the taste-path bullet has no em-dash run")?;
    let documented: Vec<String> = between
        .split(", ")
        .map(|step| step.trim().to_owned())
        .collect();
    assert_eq!(
        documented,
        TASTE_STEPS
            .iter()
            .map(|step| step.design_token().to_owned())
            .collect::<Vec<_>>(),
        "the taste steps and section 23's bullet disagree"
    );
    assert!(
        bullet.contains("탐색을 원할 때만"),
        "section 23 no longer says the path is offered only on request"
    );
    assert!(
        page.contains("`EXPLORE`를 누른 경우에만 작은 입문 path를 만든다"),
        "section 25.12 no longer says only EXPLORE creates one"
    );

    let field = entity(FORMAL);

    // Only `EXPLORE` opens one.
    for disposition in DISPOSITIONS
        .iter()
        .filter(|held| **held != UserDisposition::Explore)
    {
        let deadline = disposition.needs_deadline().then(|| at(30));
        let held = choice(field, *disposition, deadline, at(0))?;
        assert_eq!(
            refused(TastePath::for_explore(&held, field, TasteStep::OneChapter))?,
            BlindSpotError::TastePathNeedsExplore,
            "{disposition:?} opened a taste path"
        );
    }
    let explore = choice(field, UserDisposition::Explore, None, at(0))?;
    assert_eq!(
        refused(TastePath::for_explore(
            &explore,
            entity(GRAPHICS),
            TasteStep::OneChapter,
        ))?,
        BlindSpotError::TastePathIsAboutAnotherField
    );
    for step in TASTE_STEPS {
        let path = TastePath::for_explore(&explore, field, step)?;
        assert_eq!(path.step(), step);
        assert_eq!(path.key(), field);
    }

    // Through the engine: `EXPLORE` produces exactly one bounded step, and the
    // finding is otherwise the same neutral value.
    let taxonomy = taxonomy()?;
    let scope = scope_over(&taxonomy, 2)?;
    let resolver = resolver_at(&taxonomy, TaxonomyGranularity::Field);
    let ledger = DispositionLedger::new().record(explore)?;
    let readings =
        vec![KeyReading::of(field, Vec::new()).with_taste_step(TasteStep::OneToyExperiment)];
    let findings = detect(&scope, &resolver, &ledger, &readings, at(1))?;
    let finding = one(&findings)?;
    let FindingPresentation::Explore { path, .. } = finding.presentation() else {
        return Err("EXPLORE did not produce a taste path".into());
    };
    // Section 37's own case: a compiler/PL blind spot explored with one toy
    // project. One step, not a list, so a second is not a value that exists.
    assert_eq!(path.step(), TasteStep::OneToyExperiment);
    assert_eq!(path.key(), field);
    assert_eq!(finding.classification(), BlindSpotState::Unobserved);
    assert_eq!(finding.presentation().presentation().emphasis(), EMPHASIS);
    assert_eq!(
        finding.presentation().presentation().headline(),
        CANNOT_INFER_ABILITY
    );

    // `EXPLORE` with no step offered is refused rather than silently neutral.
    let bare = vec![KeyReading::of(field, Vec::new())];
    assert_eq!(
        refused(detect(&scope, &resolver, &ledger, &bare, at(1)))?,
        BlindSpotError::ExploreWithoutAStep
    );

    // And the control: without the choice, the same reading offering the same
    // step produces no path at all.
    let findings = detect(
        &scope,
        &resolver,
        &DispositionLedger::new(),
        &readings,
        at(1),
    )?;
    assert!(one(&findings)?.presentation().path().is_none());

    // Section 37's `다시 neutral 상태로 둘 수 있다`: the user can move off
    // `EXPLORE` and the path goes with it.
    let later = DispositionLedger::new()
        .record(choice(field, UserDisposition::Explore, None, at(0))?)?
        .record(choice(field, UserDisposition::Later, None, at(1))?)?;
    let findings = detect(&scope, &resolver, &later, &readings, at(2))?;
    assert!(one(&findings)?.presentation().path().is_none());

    Ok(())
}
