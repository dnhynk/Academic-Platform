//! `P2-N3`'s nine named acceptance rows.
//!
//! Two of them read `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and
//! compare what is in this crate against what is in the document, in both
//! directions: section 13.3's band sentence and its seven-bullet input list are
//! **measurements** rather than counts restated in a test, and section 13.3's
//! `노출·복습보다 … 더 긴 지속성` is read for its own word order rather than
//! assumed. Section 34.2's `불확실성 표시` cell and section 13.3's example block
//! are read the same way for the copy a stale concept shows.
//!
//! The lecture and project evidence comes from `P2-N2`'s own fixture module,
//! included by `#[path]`: the capture is written by the real
//! `academic_capture::begin`, the transcript by the real
//! `academic_transcription::run`, and the document by the real `P2-L4` builder,
//! so a `TeachingSite` here names a node of a document `P2-L4` produced rather
//! than a string this suite invented.

#[path = "../../knowledge-state/tests/common/mod.rs"]
mod common;

use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, ConfidencePermille, ContentDigest,
    EntityId, EpistemicStatus, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole,
    EvidenceStrength, FreshnessBand, MasteryLevel, ModelRunId, PredicateId, ScopeId,
    TimestampMillis, ValidInterval, entity_registry::EntityKind, predicates::PredicateName,
};
use academic_freshness::{
    BANDS, CitedEdge, ConfidenceGap, ContraryEvent, ContraryKind, DatedEvidence,
    FRESHNESS_GAP_PERMILLE, FreshnessError, FreshnessInputs, FreshnessSignal, JUDGEMENT_TOKENS,
    NeighborUse, PersistenceClass, PersonalizationSpeed, PriorBasis, PriorName, RecallCheck,
    RecallStatement, Repetition, SPILLOVER_CEILING, SPILLOVER_EDGES, STALE_ACTION,
    STALE_DISCLOSURE, STALE_MEANING, Spillover, StaleDisclosure, UNCALIBRATED_PRIOR_V1, UserRecall,
    band_token, decay, persistence_class, project, rank,
};
use academic_knowledge_state::{
    AiProposal, ConceptEvidence, ConceptLink, EligibilityOutcome, EligibleEvidence,
    EvidenceDossier, EvidenceKind, ExerciseOutcome, FacetProfile, FacetStrength, FreshnessInput,
    IncidentRepair, KnowledgeStateHistory, Outcome, Participation, ProposalOutcome,
    SelfExplanation, SourceIntegrity, TeachingSite, UserConfirmation,
};
use academic_lecture_document::{LectureDocument, NodeId};

type TestResult = Result<(), Box<dyn Error>>;

/// Milliseconds in a day.
const DAY: i64 = 86_400_000;

/// The instant every fixture is dated from. `2025-07-12`, which is section
/// 13.3's own `Last strong evidence:` line, as Unix milliseconds.
const LAST_STRONG_EVIDENCE: i64 = 1_752_278_400_000;

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

/// The body of section 13.3, from its heading to the next one.
fn section_13_3(page: &str) -> Result<String, Box<dyn Error>> {
    let start = page
        .find("### 13.3 Freshness는")
        .ok_or("the design document has no section 13.3")?;
    let rest = &page[start..];
    let end = rest[1..]
        .find("\n### ")
        .map_or(rest.len(), |offset| offset + 1);
    Ok(rest[..end].to_owned())
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
    Ok(page[body_start..body_start + closed]
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect())
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
        if cells
            .iter()
            .all(|cell| cell.chars().all(|c| c == '-' || c == ':'))
        {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err(format!("no table rows after {heading}").into());
    }
    Ok(rows)
}

/// The `- ` bullets of section 13.3's `계산 입력` list, in the document's order.
fn designed_signals(page: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let body = section_13_3(page)?;
    let start = body
        .find("계산 입력은 다음과 같다.")
        .ok_or("section 13.3 has no input list")?;
    let bullets: Vec<String> = body[start..]
        .lines()
        .skip(1)
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| line.starts_with("- "))
        .map(|line| line[2..].trim().to_owned())
        .collect();
    if bullets.is_empty() {
        return Err("section 13.3's input list is empty".into());
    }
    Ok(bullets)
}

/// Every back-quoted spelling of section 13.3's band sentence, in its order.
fn designed_bands(page: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let body = section_13_3(page)?;
    let sentence = body
        .lines()
        .find(|line| line.starts_with("Freshness는") && line.contains("band로 표시한다"))
        .ok_or("section 13.3 has no band sentence")?;
    let head = sentence
        .split_once("band로 표시한다")
        .map(|(head, _)| head)
        .ok_or("the band sentence has no tail")?;
    let names: Vec<String> = head
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    if names.is_empty() {
        return Err("the band sentence names nothing".into());
    }
    Ok(names)
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

fn model_run_id(tag: &str) -> ModelRunId {
    ModelRunId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn scope() -> ScopeId {
    ScopeId::try_from_uuid(uuid_of("scope-freshness"))
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

fn at(days_after: i64) -> TimestampMillis {
    TimestampMillis::new(LAST_STRONG_EVIDENCE + days_after * DAY)
}

fn full_dossier(concept: EntityId) -> EvidenceDossier {
    EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Authored,
        Outcome::Succeeded,
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

/// Section 13.2's first row — the `노출` side — about `concept`.
fn exposure(concept: EntityId, tag: &str) -> Result<EligibleEvidence, Box<dyn Error>> {
    let lecture = lecture_document(tag)?;
    admitted(
        ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?),
        tag,
        &full_dossier(concept),
    )
}

/// Section 13.2's fifth row — the `debugging` side — about `concept`.
fn debugging(concept: EntityId, tag: &str) -> Result<EligibleEvidence, Box<dyn Error>> {
    admitted(
        ConceptEvidence::IncidentDebugging(IncidentRepair::of(
            evidence_id(&format!("{tag}-incident")),
            evidence_id(&format!("{tag}-cause")),
            evidence_id(&format!("{tag}-fix")),
            evidence_id(&format!("{tag}-verified")),
        )),
        tag,
        &full_dossier(concept),
    )
}

/// Section 13.2's third row about `concept`.
fn exercise(concept: EntityId, tag: &str) -> Result<EligibleEvidence, Box<dyn Error>> {
    admitted(
        ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id(tag))),
        tag,
        &full_dossier(concept),
    )
}

fn recall_claim(
    concept: EntityId,
    band: FreshnessBand,
    evidence: &EvidenceItem,
    authority: AuthorityClass,
    status: EpistemicStatus,
    predicate: &str,
) -> Result<Claim, Box<dyn Error>> {
    Ok(Claim {
        id: claim_id("recall-claim"),
        subject_entity_id: concept,
        predicate_id: PredicateId::parse(predicate)?,
        object: ClaimObject::Freshness(band),
        scope_id: scope(),
        authority_class: authority,
        epistemic_status: status,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::new(TimestampMillis::new(0), None)?,
        evidence_ids: vec![evidence.id],
    })
}

fn statement(
    concept: EntityId,
    said: UserRecall,
    when: TimestampMillis,
) -> Result<RecallStatement, Box<dyn Error>> {
    let evidence = evidence_item("recall");
    let claim = recall_claim(
        concept,
        said.band(),
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        academic_freshness::RECALL_STATEMENT_PREDICATE,
    )?;
    Ok(RecallStatement::verify(
        &user_actor(),
        &claim,
        &evidence,
        concept,
        said,
        when,
    )?)
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

const NO_SPILLOVER: &[Spillover] = &[];
const NO_STATEMENTS: &[RecallStatement] = &[];
const NO_CONTRARY: &[ContraryEvent] = &[];

fn inputs<'a>(dated: &'a [DatedEvidence]) -> FreshnessInputs<'a> {
    FreshnessInputs {
        dated,
        spillover: NO_SPILLOVER,
        statements: NO_STATEMENTS,
        contrary: NO_CONTRARY,
    }
}

// ---------------------------------------------------------------------------
// 1. `freshness_bands_are_exactly_six`
// ---------------------------------------------------------------------------

#[test]
fn freshness_bands_are_exactly_six() -> TestResult {
    let page = specification()?;

    // Both directions against section 13.3's own sentence. Six is what the
    // document names, not a number written here.
    let designed = designed_bands(&page)?;
    let held: Vec<String> = BANDS
        .iter()
        .map(|band| band_token(*band).to_owned())
        .collect();
    assert_eq!(designed, held, "section 13.3's bands and BANDS disagree");
    assert_eq!(BANDS.len(), designed.len());

    // The array is the sentence's order — best first — and the domain
    // enumeration's `Ord` runs the other way. Requiring the array to be strictly
    // decreasing under `rank` is what keeps the two from drifting apart.
    for pair in BANDS.windows(2) {
        assert!(
            rank(pair[0]) > rank(pair[1]),
            "{pair:?} is not the sentence's order"
        );
        assert!(
            pair[0] > pair[1],
            "{pair:?} disagrees with FreshnessBand's Ord"
        );
        assert_eq!(rank(pair[1]) + 1, rank(pair[0]));
    }

    // Every band the domain enumeration has is one of these, so a seventh added
    // there appears here as a missing key rather than as a value nothing lists.
    let distinct: BTreeSet<FreshnessBand> = BANDS.iter().copied().collect();
    assert_eq!(distinct.len(), BANDS.len(), "BANDS repeats a band");

    // Section 13.3's seven inputs, the same way.
    let signals = designed_signals(&page)?;
    let bullets: Vec<String> = FreshnessSignal::ALL
        .iter()
        .map(|signal| signal.bullet().to_owned())
        .collect();
    assert_eq!(
        signals, bullets,
        "section 13.3's input list and FreshnessSignal disagree"
    );
    assert_eq!(FreshnessSignal::ALL.len(), signals.len());

    // And a control on the reader: the two extractors must not both answer with
    // whatever they are given. A band name the document does not carry is not
    // found in it, and a bullet it does not carry is not either.
    assert!(!designed.iter().any(|name| name == "VERY_LOW"));
    assert!(
        !signals
            .iter()
            .any(|bullet| bullet.contains("mastery 자동 강등"))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. `stale_does_not_demote`
// ---------------------------------------------------------------------------

#[test]
fn stale_does_not_demote() -> TestResult {
    let concept = entity("concept-virtual-memory");
    let evidence = vec![
        exposure(concept, "vm-lecture")?,
        exercise(concept, "vm-exercise")?,
    ];
    let dated: Vec<DatedEvidence> = evidence
        .iter()
        .map(|item| DatedEvidence::at(item.clone(), at(0)))
        .collect();

    // Day zero: the evidence is new, so the band is at the top.
    let fresh = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(0))?;
    assert_eq!(fresh.band(), FreshnessBand::VeryHigh);

    let history = KnowledgeStateHistory::open(
        concept,
        evidence.clone(),
        Vec::new(),
        Vec::new(),
        facets(),
        fresh.input(),
        at(0),
    )?;
    let before = history
        .current()
        .ok_or("the history holds no assertion")?
        .clone();

    // Two years later, with nothing else changed. Section 4's own case.
    let later = at(730);
    let stale = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, later)?;
    assert_eq!(stale.band(), FreshnessBand::Stale);

    let applied = history.propose(
        AiProposal::of(
            model_run_id("proposal"),
            concept,
            before.mastery_level(),
            Vec::new(),
        ),
        Vec::new(),
        Vec::new(),
        stale.input(),
        later,
    )?;
    let after = applied
        .history()
        .current()
        .ok_or("the history lost its assertion")?;

    // The band moved all the way down and the level did not move at all.
    assert_eq!(before.freshness_band(), FreshnessBand::VeryHigh);
    assert_eq!(after.freshness_band(), FreshnessBand::Stale);
    assert_eq!(
        after.mastery_level(),
        before.mastery_level(),
        "elapsed time changed the mastery level"
    );
    assert!(matches!(applied.outcome(), ProposalOutcome::Superseded(_)));

    // The earlier version is still readable at its own identity with its own
    // band, so the demotion did not happen to the history either.
    let earlier = applied
        .history()
        .version_at(before.id())
        .ok_or("the earlier version is gone")?;
    assert_eq!(earlier, &before);
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. `time_decay_touches_freshness_only`
// ---------------------------------------------------------------------------

#[test]
fn time_decay_touches_freshness_only() -> TestResult {
    let concept = entity("concept-serializability");
    let evidence = vec![debugging(concept, "ser-debug")?];
    let dated: Vec<DatedEvidence> = evidence
        .iter()
        .map(|item| DatedEvidence::at(item.clone(), at(0)))
        .collect();

    let opening = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(0))?;
    let mut history = KnowledgeStateHistory::open(
        concept,
        evidence.clone(),
        Vec::new(),
        Vec::new(),
        facets(),
        opening.input(),
        at(0),
    )?;
    let level = history
        .current()
        .ok_or("the history holds no assertion")?
        .mastery_level();

    // Sweep the clock. The only thing that changes between steps is `as_of`.
    let mut seen: Vec<FreshnessBand> = Vec::new();
    for days in [0_i64, 200, 400, 800, 1600, 3200] {
        let projection = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(days))?;
        seen.push(projection.band());
        let applied = history.propose(
            AiProposal::of(model_run_id("sweep"), concept, level, Vec::new()),
            Vec::new(),
            Vec::new(),
            projection.input(),
            at(days),
        )?;
        history = applied.into_history();
        assert_eq!(
            history
                .current()
                .ok_or("the history lost its assertion")?
                .mastery_level(),
            level,
            "the clock moved the mastery level at day {days}"
        );
    }

    // The sweep has to actually move the band, or the equality above is a
    // measurement of nothing.
    assert_eq!(seen.first(), Some(&FreshnessBand::VeryHigh));
    assert_eq!(seen.last(), Some(&FreshnessBand::Stale));
    for pair in seen.windows(2) {
        assert!(
            rank(pair[0]) >= rank(pair[1]),
            "the band rose with time: {seen:?}"
        );
    }
    assert!(
        seen.windows(2).any(|pair| rank(pair[0]) > rank(pair[1])),
        "the band never moved"
    );

    // Every version in the history carries the same level and they are not all
    // the same assertion: the freshness is what changed.
    let versions = history.versions();
    assert_eq!(versions.len(), 7);
    assert!(
        versions
            .iter()
            .all(|version| version.mastery_level() == level)
    );
    let bands: BTreeSet<FreshnessBand> = versions
        .iter()
        .map(|version| version.freshness_band())
        .collect();
    assert!(bands.len() > 1, "no version's band differs");

    // The decay function's own signature: a span and a window, and the value it
    // returns is a band. There is no mastery in either position, and
    // `the_freshness_crate_cannot_name_a_mastery` measures the whole crate.
    let window = UNCALIBRATED_PRIOR_V1.window_of(PersistenceClass::ExposureOrReview);
    assert_eq!(decay(0, window), FreshnessBand::VeryHigh);
    assert_eq!(decay(window.millis() * 4, window), FreshnessBand::Stale);
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. `spillover_is_one_hop_and_cited`
// ---------------------------------------------------------------------------

#[test]
fn spillover_is_one_hop_and_cited() -> TestResult {
    let page = specification()?;
    let a = entity("concept-a-buffer-pool");
    let b = entity("concept-b-disk-page");
    let c = entity("concept-c-page-replacement");

    // The four admitted predicates are section 7.2's, and the sixteen that are
    // not are refused. Both directions over the registry, so a twenty-first
    // predicate is an extra key rather than a silent admission.
    let rows = table_after(&page, "### 7.2 Edge 방향과 엄밀한 의미")?;
    let designed: BTreeSet<String> = rows
        .iter()
        .filter_map(|cells| Some(cells.first()?.trim_matches('`').to_owned()))
        .collect();
    assert_eq!(designed.len(), PredicateName::ALL.len());
    for predicate in SPILLOVER_EDGES {
        assert!(
            designed.contains(predicate.as_str()),
            "{} is not a section 7.2 edge",
            predicate.as_str()
        );
    }
    let admitted_names: BTreeSet<&str> = SPILLOVER_EDGES.iter().map(|one| one.as_str()).collect();
    let mut refused = 0_usize;
    for predicate in PredicateName::ALL {
        let edge = CitedEdge::of(predicate, a, b, vec![evidence_id("edge")]);
        if admitted_names.contains(predicate.as_str()) {
            assert!(edge.is_some(), "{} was refused", predicate.as_str());
        } else {
            assert!(edge.is_none(), "{} was admitted", predicate.as_str());
            refused += 1;
        }
    }
    assert_eq!(refused + SPILLOVER_EDGES.len(), PredicateName::ALL.len());

    // `명시적 근거`: an edge with no evidence and a self-edge are both refused.
    assert!(CitedEdge::of(PredicateName::RelatedTo, a, b, Vec::new()).is_none());
    assert!(CitedEdge::of(PredicateName::RelatedTo, a, a, vec![evidence_id("edge")]).is_none());

    // `A` was used today. `B` has an edge to `A` and nothing of its own.
    let a_evidence = vec![DatedEvidence::at(debugging(a, "a-debug")?, at(0))];
    let edge_ab = CitedEdge::of(PredicateName::BuildsOn, b, a, vec![evidence_id("edge-ab")])
        .ok_or("the A-B edge was refused")?;
    let a_use = NeighborUse::direct(edge_ab, a, &a_evidence, &UNCALIBRATED_PRIOR_V1, at(0))
        .ok_or("A's own use was not read")?;
    let to_b = Spillover::toward(b, a_use).ok_or("A contributed nothing to B")?;

    // `낮은 weight`, as a band comparison rather than a coefficient.
    assert_eq!(to_b.neighbor_band(), FreshnessBand::VeryHigh);
    assert!(
        rank(to_b.band()) < rank(to_b.neighbor_band()),
        "the contribution is not below the neighbour's own band"
    );
    // The step down alone would leave a neighbour at the top contributing
    // `HIGH`, which is a band this concept's own evidence reaches. The ceiling
    // is what keeps a neighbour out of the top two, and it is observed here
    // rather than inferred from the step.
    assert!(
        rank(to_b.band()) <= rank(SPILLOVER_CEILING),
        "the contribution is above the spillover ceiling"
    );
    assert!(rank(SPILLOVER_CEILING) < rank(FreshnessBand::High));

    let b_from_spill = project(
        b,
        FreshnessInputs {
            dated: &[],
            spillover: std::slice::from_ref(&to_b),
            statements: NO_STATEMENTS,
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(0),
    )?;
    assert_eq!(b_from_spill.band(), to_b.band());
    assert!(
        rank(b_from_spill.band()) < rank(FreshnessBand::VeryHigh),
        "B reached A's own band"
    );

    // The trace cites the edge and the neighbour.
    let cited = b_from_spill
        .trace()
        .of(FreshnessSignal::RelatedConceptSpillover);
    assert_eq!(cited.len(), 1);
    assert!(cited[0].detail().contains(PredicateName::BuildsOn.as_str()));
    assert_eq!(to_b.edge().evidence(), &[evidence_id("edge-ab")]);

    // `한 단계`: C has a real edge to B, and B has no evidence of its own, so C
    // gets nothing. This is `REQ-13-034`'s own A → B → C case.
    let edge_bc = CitedEdge::of(PredicateName::BuildsOn, c, b, vec![evidence_id("edge-bc")])
        .ok_or("the B-C edge was refused")?;
    assert!(
        NeighborUse::direct(edge_bc.clone(), b, &[], &UNCALIBRATED_PRIOR_V1, at(0)).is_none(),
        "B offered a use it does not have"
    );

    // And a use is refused on an edge that does not join the concept it claims
    // to be about. `Spillover::toward` refuses the same shape one step later,
    // so this is observed here rather than left to that: a `NeighborUse` whose
    // edge does not name its own neighbour is a value a caller can hold.
    let a_evidence_for_c = vec![DatedEvidence::at(debugging(c, "c-debug")?, at(0))];
    let edge_ab_again = CitedEdge::of(PredicateName::BuildsOn, b, a, vec![evidence_id("edge-ab")])
        .ok_or("the A-B edge was refused")?;
    assert!(
        NeighborUse::direct(
            edge_ab_again,
            c,
            &a_evidence_for_c,
            &UNCALIBRATED_PRIOR_V1,
            at(0)
        )
        .is_none(),
        "a use was read across an edge that does not join it"
    );

    // And the route that survives every other limit: cite the real B-C edge and
    // hand it **A's** evidence as B's. That is two hops through one well-formed
    // edge, and it is refused.
    assert!(
        NeighborUse::direct(edge_bc, b, &a_evidence, &UNCALIBRATED_PRIOR_V1, at(0)).is_none(),
        "A's evidence was read as B's recent use"
    );

    // A neighbour below MODERATE is not `최근 사용` and contributes nothing; a
    // whole sweep over the six bands, so the two rules cover all of them.
    let mut contributed = 0_usize;
    for days in [0_i64, 60, 200, 500, 1500, 4000] {
        let edge = CitedEdge::of(PredicateName::RelatedTo, b, a, vec![evidence_id("edge-ab")])
            .ok_or("the A-B edge was refused")?;
        let Some(use_) =
            NeighborUse::direct(edge, a, &a_evidence, &UNCALIBRATED_PRIOR_V1, at(days))
        else {
            continue;
        };
        let spill = Spillover::toward(b, use_).ok_or("a read use contributed nothing")?;
        assert!(
            rank(spill.band()) < rank(spill.neighbor_band()),
            "a contribution at day {days} is not below its source"
        );
        contributed += 1;
    }
    assert!(contributed >= 2, "only {contributed} bands contributed");

    // Contributions do not accumulate: ten neighbours give what one gives.
    let many: Vec<Spillover> = std::iter::repeat_n(to_b.clone(), 10).collect();
    let piled = project(
        b,
        FreshnessInputs {
            dated: &[],
            spillover: &many,
            statements: NO_STATEMENTS,
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(0),
    )?;
    assert_eq!(piled.band(), b_from_spill.band(), "spillover accumulated");

    // A contribution computed toward B is refused under C's projection.
    let wrong = project(
        c,
        FreshnessInputs {
            dated: &[],
            spillover: std::slice::from_ref(&to_b),
            statements: NO_STATEMENTS,
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(0),
    );
    assert_eq!(wrong, Err(FreshnessError::SpilloverNamesAnotherConcept));

    // And the zero-hop form of the same misattribution: A's dated evidence
    // offered directly under B's projection, with no edge at all.
    let direct = project(b, inputs(&a_evidence), &UNCALIBRATED_PRIOR_V1, at(0));
    assert_eq!(direct, Err(FreshnessError::EvidenceNamesAnotherConcept));
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. `debugging_evidence_persists_longer_than_exposure`
// ---------------------------------------------------------------------------

#[test]
fn debugging_evidence_persists_longer_than_exposure() -> TestResult {
    let page = specification()?;

    // The direction is read out of section 13.3's own sentence: the phrase
    // before `보다` is the shorter-lived side and the phrase after it is the
    // longer-lived one, and the relation word is `더 긴`.
    let bullet = designed_signals(&page)?
        .into_iter()
        .find(|line| line.contains("보다") && line.contains("지속성"))
        .ok_or("section 13.3 has no persistence bullet")?;
    let (shorter, rest) = bullet
        .split_once("보다")
        .ok_or("the persistence bullet has no 보다")?;
    assert_eq!(shorter.trim(), PersistenceClass::ExposureOrReview.phrase());
    let (longer, relation) = rest
        .trim()
        .split_once('에')
        .ok_or("the persistence bullet has no 에")?;
    assert_eq!(
        longer.trim(),
        PersistenceClass::ApplicationOrDesign.phrase()
    );
    assert!(
        relation.trim_start().starts_with("더 긴"),
        "the bullet does not say the second side lasts longer: {relation}"
    );

    // Every section 13.2 row is on one side or has no window at all, and the two
    // windows are ordered the way the sentence says.
    let short = UNCALIBRATED_PRIOR_V1.window_of(PersistenceClass::ExposureOrReview);
    let long = UNCALIBRATED_PRIOR_V1.window_of(PersistenceClass::ApplicationOrDesign);
    assert!(
        long.days() > short.days(),
        "the windows contradict the bullet"
    );

    let mut classified = 0_usize;
    for kind in EvidenceKind::ALL {
        match persistence_class(kind) {
            Some(class) => {
                classified += 1;
                assert_eq!(
                    UNCALIBRATED_PRIOR_V1.window_of(class),
                    UNCALIBRATED_PRIOR_V1
                        .window_for(kind)
                        .ok_or("a classified row has no window")?
                );
            }
            None => assert_eq!(kind, EvidenceKind::CourseGrade),
        }
    }
    assert_eq!(classified + 1, EvidenceKind::ALL.len());

    // Two concepts, one exposure item and one debugging item, on the same day.
    let taught = entity("concept-taught-only");
    let debugged = entity("concept-debugged");
    let exposure_dated = vec![DatedEvidence::at(
        exposure(taught, "persist-lecture")?,
        at(0),
    )];
    let debug_dated = vec![DatedEvidence::at(
        debugging(debugged, "persist-debug")?,
        at(0),
    )];

    let mut strictly_higher = 0_usize;
    for days in [0_i64, 45, 100, 200, 400, 800, 1600] {
        let exposed = project(
            taught,
            inputs(&exposure_dated),
            &UNCALIBRATED_PRIOR_V1,
            at(days),
        )?;
        let repaired = project(
            debugged,
            inputs(&debug_dated),
            &UNCALIBRATED_PRIOR_V1,
            at(days),
        )?;
        assert!(
            rank(repaired.band()) >= rank(exposed.band()),
            "debugging fell below exposure at day {days}: {:?} vs {:?}",
            repaired.band(),
            exposed.band()
        );
        if rank(repaired.band()) > rank(exposed.band()) {
            strictly_higher += 1;
        }
    }
    assert!(
        strictly_higher > 0,
        "the two kinds never differed, so the comparison measured nothing"
    );

    // The trace names the input that made the difference.
    let repaired = project(
        debugged,
        inputs(&debug_dated),
        &UNCALIBRATED_PRIOR_V1,
        at(200),
    )?;
    let persistence = repaired
        .trace()
        .of(FreshnessSignal::EvidenceTypePersistence);
    assert_eq!(persistence.len(), 1);
    assert!(persistence[0].detail().contains(&long.days().to_string()));
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. `user_recall_confirmation_is_reflected`
// ---------------------------------------------------------------------------

#[test]
fn user_recall_confirmation_is_reflected() -> TestResult {
    let page = specification()?;

    // Both statements are the design document's own two phrases.
    let bullet = designed_signals(&page)?
        .into_iter()
        .find(|line| line.contains("사용자 직접"))
        .ok_or("section 13.3 has no user statement bullet")?;
    for said in UserRecall::ALL {
        assert!(
            bullet.contains(said.phrase()),
            "section 13.3 does not say {}",
            said.phrase()
        );
    }
    assert!(!bullet.contains("아직 배우지 않음"));

    let concept = entity("concept-recall");
    let dated = vec![DatedEvidence::at(
        exposure(concept, "recall-lecture")?,
        at(0),
    )];

    // The same evidence history, long enough after that it alone is LOW.
    let bare = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(300))?;
    assert_eq!(bare.band(), FreshnessBand::Low);

    // Opposite confirmations, and each one is reflected.
    let can_use = statement(concept, UserRecall::CanUseNow, at(300))?;
    let raised = project(
        concept,
        FreshnessInputs {
            dated: &dated,
            spillover: NO_SPILLOVER,
            statements: std::slice::from_ref(&can_use),
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(300),
    )?;
    assert_eq!(raised.band(), FreshnessBand::VeryHigh);
    assert!(raised.trace().names(FreshnessSignal::UserRecallStatement));

    let needs_review = statement(concept, UserRecall::NeedsReview, at(300))?;
    let lowered = project(
        concept,
        FreshnessInputs {
            dated: &dated,
            spillover: NO_SPILLOVER,
            statements: std::slice::from_ref(&needs_review),
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(300),
    )?;
    assert_eq!(lowered.band(), FreshnessBand::Low);

    // The two are different answers to the same evidence, which is
    // `REQ-13-028`'s own case.
    assert_ne!(raised.band(), lowered.band());

    // A `복습 필요` caps a band that would otherwise be at the top.
    let recent = vec![DatedEvidence::at(
        debugging(concept, "recall-debug")?,
        at(300),
    )];
    let capped = project(
        concept,
        FreshnessInputs {
            dated: &recent,
            spillover: NO_SPILLOVER,
            statements: std::slice::from_ref(&needs_review),
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(300),
    )?;
    assert_eq!(capped.band(), FreshnessBand::Low);

    // A statement made long ago raises nothing today: it decays like every other
    // input rather than standing forever.
    let stale_statement = project(
        concept,
        FreshnessInputs {
            dated: &dated,
            spillover: NO_SPILLOVER,
            statements: std::slice::from_ref(&can_use),
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(3000),
    )?;
    assert_eq!(stale_statement.band(), FreshnessBand::Stale);

    // A model run cannot mint one. ADR-003's matrix refuses the actor, and the
    // wrong predicate, the wrong band and another concept are each refused too.
    let evidence = evidence_item("recall");
    let claim = recall_claim(
        concept,
        UserRecall::CanUseNow.band(),
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        academic_freshness::RECALL_STATEMENT_PREDICATE,
    )?;
    // ADR-003's matrix, in both pairings, and by the **exact** error rather than
    // by `is_err`. Injection `I17` deleted `validate_for_actor` from `verify`
    // and an `is_err` assertion did not notice: the `Actor::User` destructure at
    // the end of `verify` refuses a model run on its own, so the matrix could
    // have gone away with nothing observing it.
    assert!(matches!(
        RecallStatement::verify(
            &model_actor(),
            &claim,
            &evidence,
            concept,
            UserRecall::CanUseNow,
            at(300)
        ),
        Err(FreshnessError::Domain(_))
    ));

    // The pairing only the matrix refuses: the user's own actor on a claim
    // carrying a model's authority. Every other check in `verify` passes it.
    let model_authority = recall_claim(
        concept,
        UserRecall::CanUseNow.band(),
        &evidence,
        AuthorityClass::ModelInference,
        EpistemicStatus::UserConfirmed,
        academic_freshness::RECALL_STATEMENT_PREDICATE,
    )?;
    assert!(matches!(
        RecallStatement::verify(
            &user_actor(),
            &model_authority,
            &evidence,
            concept,
            UserRecall::CanUseNow,
            at(300)
        ),
        Err(FreshnessError::Domain(_))
    ));
    let mismatched = recall_claim(
        concept,
        UserRecall::NeedsReview.band(),
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        academic_freshness::RECALL_STATEMENT_PREDICATE,
    )?;
    assert_eq!(
        RecallStatement::verify(
            &user_actor(),
            &mismatched,
            &evidence,
            concept,
            UserRecall::CanUseNow,
            at(300)
        ),
        Err(FreshnessError::RecallBandMismatch)
    );
    assert_eq!(
        RecallStatement::verify(
            &user_actor(),
            &claim,
            &evidence,
            entity("some-other-concept"),
            UserRecall::CanUseNow,
            at(300)
        ),
        Err(FreshnessError::RecallSubjectMismatch)
    );
    let wrong_predicate = recall_claim(
        concept,
        UserRecall::CanUseNow.band(),
        &evidence,
        AuthorityClass::UserExplicit,
        EpistemicStatus::UserConfirmed,
        "knowledge.state.confirmed",
    )?;
    assert_eq!(
        RecallStatement::verify(
            &user_actor(),
            &wrong_predicate,
            &evidence,
            concept,
            UserRecall::CanUseNow,
            at(300)
        ),
        Err(FreshnessError::NotARecallStatement)
    );

    // Another concept's statement is refused by the projection.
    let elsewhere = statement(entity("some-other-concept"), UserRecall::CanUseNow, at(300))?;
    assert_eq!(
        project(
            concept,
            FreshnessInputs {
                dated: &dated,
                spillover: NO_SPILLOVER,
                statements: std::slice::from_ref(&elsewhere),
                contrary: NO_CONTRARY,
            },
            &UNCALIBRATED_PRIOR_V1,
            at(300),
        ),
        Err(FreshnessError::EvidenceNamesAnotherConcept)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. `recall_failure_prevents_freshness_increase`
// ---------------------------------------------------------------------------

#[test]
fn recall_failure_prevents_freshness_increase() -> TestResult {
    let page = specification()?;

    // All three contrary events are the design document's own phrases.
    let bullet = designed_signals(&page)?
        .into_iter()
        .find(|line| line.starts_with("반대 evidence"))
        .ok_or("section 13.3 has no contrary evidence bullet")?;
    for kind in ContraryKind::ALL {
        assert!(
            bullet.contains(kind.phrase()),
            "section 13.3 does not say {}",
            kind.phrase()
        );
    }
    let listed: Vec<&str> = bullet
        .split_once(':')
        .ok_or("the contrary bullet has no list")?
        .1
        .split(',')
        .map(str::trim)
        .collect();
    assert_eq!(
        listed.len(),
        ContraryKind::ALL.len(),
        "the bullet lists {listed:?}"
    );

    let concept = entity("concept-contrary");

    // `REQ-13-030`: recent positive evidence, then a recall failure.
    let strong = vec![DatedEvidence::at(
        debugging(concept, "contrary-debug")?,
        at(0),
    )];
    let without = project(concept, inputs(&strong), &UNCALIBRATED_PRIOR_V1, at(10))?;
    assert_eq!(without.band(), FreshnessBand::VeryHigh);

    let failure = ContraryEvent::of(
        ContraryKind::NoMemoryMarked,
        concept,
        evidence_id("no-memory"),
        at(10),
    );
    let with = project(
        concept,
        FreshnessInputs {
            dated: &strong,
            spillover: NO_SPILLOVER,
            statements: NO_STATEMENTS,
            contrary: std::slice::from_ref(&failure),
        },
        &UNCALIBRATED_PRIOR_V1,
        at(10),
    )?;
    assert_eq!(with.band(), ContraryKind::NoMemoryMarked.ceiling());
    assert!(rank(with.band()) < rank(without.band()));
    assert!(with.trace().names(FreshnessSignal::ContraryEvidence));
    assert!(
        with.gaps()
            .contains(&ConfidenceGap::ContraryEvidenceStanding)
    );

    // **Prevents increase**: piling on every raising input this engine has, all
    // dated at or before the failure, changes nothing.
    let neighbour = entity("concept-contrary-neighbour");
    let neighbour_evidence = vec![DatedEvidence::at(
        debugging(neighbour, "contrary-nb")?,
        at(0),
    )];
    let edge = CitedEdge::of(
        PredicateName::RelatedTo,
        concept,
        neighbour,
        vec![evidence_id("contrary-edge")],
    )
    .ok_or("the edge was refused")?;
    let spill = Spillover::toward(
        concept,
        NeighborUse::direct(
            edge,
            neighbour,
            &neighbour_evidence,
            &UNCALIBRATED_PRIOR_V1,
            at(10),
        )
        .ok_or("the neighbour offered nothing")?,
    )
    .ok_or("the neighbour contributed nothing")?;
    let said = statement(concept, UserRecall::CanUseNow, at(9))?;
    // One of these is dated **at** the failure rather than before it, so the
    // boundary the cap rule is written on carries a case.
    let piled: Vec<DatedEvidence> = [0_i64, 2, 4, 6, 8, 10]
        .into_iter()
        .map(|day| -> Result<DatedEvidence, Box<dyn Error>> {
            Ok(DatedEvidence::at(
                debugging(concept, &format!("contrary-extra-{day}"))?,
                at(day),
            ))
        })
        .collect::<Result<_, _>>()?;

    let loaded = project(
        concept,
        FreshnessInputs {
            dated: &piled,
            spillover: std::slice::from_ref(&spill),
            statements: std::slice::from_ref(&said),
            contrary: std::slice::from_ref(&failure),
        },
        &UNCALIBRATED_PRIOR_V1,
        at(10),
    )?;
    assert_eq!(
        loaded.band(),
        with.band(),
        "an input dated before the failure raised the band"
    );

    // Every one of those raisers does move the band when the failure is not
    // there, so the equality above is not a comparison of two zeroes.
    let unblocked = project(
        concept,
        FreshnessInputs {
            dated: &piled,
            spillover: std::slice::from_ref(&spill),
            statements: std::slice::from_ref(&said),
            contrary: NO_CONTRARY,
        },
        &UNCALIBRATED_PRIOR_V1,
        at(10),
    )?;
    assert!(rank(unblocked.band()) > rank(loaded.band()));

    // Relearning still works: an input dated **after** the failure lifts it.
    let relearned = vec![DatedEvidence::at(
        debugging(concept, "contrary-relearn")?,
        at(11),
    )];
    let after = project(
        concept,
        FreshnessInputs {
            dated: &relearned,
            spillover: NO_SPILLOVER,
            statements: NO_STATEMENTS,
            contrary: std::slice::from_ref(&failure),
        },
        &UNCALIBRATED_PRIOR_V1,
        at(11),
    )?;
    assert_eq!(after.band(), FreshnessBand::VeryHigh);

    // The three kinds cap at their own ceilings and none of them reaches
    // `UNKNOWN`: a recall failure is a record, and `UNKNOWN` is the absence of
    // one.
    for kind in ContraryKind::ALL {
        let event = ContraryEvent::of(kind, concept, evidence_id("contrary"), at(10));
        let capped = project(
            concept,
            FreshnessInputs {
                dated: &strong,
                spillover: NO_SPILLOVER,
                statements: NO_STATEMENTS,
                contrary: std::slice::from_ref(&event),
            },
            &UNCALIBRATED_PRIOR_V1,
            at(10),
        )?;
        assert_eq!(capped.band(), kind.ceiling());
        assert_ne!(kind.ceiling(), FreshnessBand::Unknown);
    }

    // And a contrary event about another concept is refused rather than applied.
    let elsewhere = ContraryEvent::of(
        ContraryKind::NoMemoryMarked,
        entity("some-other-concept"),
        evidence_id("no-memory"),
        at(10),
    );
    assert_eq!(
        project(
            concept,
            FreshnessInputs {
                dated: &strong,
                spillover: NO_SPILLOVER,
                statements: NO_STATEMENTS,
                contrary: std::slice::from_ref(&elsewhere),
            },
            &UNCALIBRATED_PRIOR_V1,
            at(10),
        ),
        Err(FreshnessError::EvidenceNamesAnotherConcept)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. `prior_is_versioned_and_identifiable_after_calibration`
// ---------------------------------------------------------------------------

#[test]
fn prior_is_versioned_and_identifiable_after_calibration() -> TestResult {
    // The shipped default says what it is, in its own name and in a value.
    assert_eq!(
        UNCALIBRATED_PRIOR_V1.identity().name_str(),
        "UNCALIBRATED_PRIOR_V1"
    );
    assert_eq!(
        UNCALIBRATED_PRIOR_V1.identity().name(),
        PriorName::UncalibratedV1
    );
    assert_eq!(UNCALIBRATED_PRIOR_V1.identity().generation(), 1);
    assert_eq!(
        UNCALIBRATED_PRIOR_V1.basis(),
        PriorBasis::NoEvidenceBasisEstablished
    );
    assert!(UNCALIBRATED_PRIOR_V1.is_uncalibrated());

    let concept = entity("concept-calibrated");
    let dated = vec![DatedEvidence::at(exposure(concept, "cal-lecture")?, at(0))];

    // And every band read through it is visibly labelled uncalibrated.
    let shipped = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(10))?;
    assert!(shipped.prior_is_uncalibrated());
    assert!(shipped.gaps().contains(&ConfidenceGap::PriorUncalibrated));
    let trace = shipped
        .trace()
        .of(FreshnessSignal::RetentionPriorAndCalibration);
    assert_eq!(trace.len(), 1);
    assert!(trace[0].detail().contains("UNCALIBRATED"));

    // Cold start: below the minimum sample count, the prior is returned
    // unchanged and is still the shipped one.
    let speed = PersonalizationSpeed::of(3, 5).ok_or("the speed was refused")?;
    let one = [RecallCheck::from_statement(&statement(
        concept,
        UserRecall::CanUseNow,
        at(1),
    )?)];
    let cold = UNCALIBRATED_PRIOR_V1.calibrate(&one, speed);
    assert_eq!(cold, UNCALIBRATED_PRIOR_V1);
    assert!(cold.is_uncalibrated());

    // The user's own recall record moves it.
    let record: Vec<RecallCheck> = [1_i64, 20, 40, 60]
        .into_iter()
        .map(|day| -> Result<RecallCheck, Box<dyn Error>> {
            Ok(RecallCheck::from_statement(&statement(
                concept,
                UserRecall::CanUseNow,
                at(day),
            )?))
        })
        .collect::<Result<_, _>>()?;
    let calibrated = UNCALIBRATED_PRIOR_V1.calibrate(&record, speed);

    // `differs from initial prior`: the windows and the identity both moved.
    assert_ne!(calibrated, UNCALIBRATED_PRIOR_V1);
    assert_ne!(calibrated.identity(), UNCALIBRATED_PRIOR_V1.identity());
    assert!(
        calibrated
            .window_of(PersistenceClass::ExposureOrReview)
            .days()
            > UNCALIBRATED_PRIOR_V1
                .window_of(PersistenceClass::ExposureOrReview)
                .days()
    );
    assert!(!calibrated.is_uncalibrated());
    assert_eq!(calibrated.basis(), PriorBasis::UserRecallRecord);

    // `prior remains identifiable`: the shipped default is still nameable from
    // the calibrated one, in both the value and the projection.
    assert_eq!(calibrated.origin(), UNCALIBRATED_PRIOR_V1.identity());
    assert_eq!(calibrated.origin().name_str(), "UNCALIBRATED_PRIOR_V1");
    let personal = project(concept, inputs(&dated), &calibrated, at(10))?;
    assert!(!personal.prior_is_uncalibrated());
    assert_eq!(personal.prior_origin(), UNCALIBRATED_PRIOR_V1.identity());
    assert!(!personal.gaps().contains(&ConfidenceGap::PriorUncalibrated));

    // Versioned: a second calibration is a further generation, still tracing
    // back to the shipped default.
    let again = calibrated.calibrate(&record, speed);
    assert_eq!(
        again.identity().generation(),
        calibrated.identity().generation() + 1
    );
    assert_eq!(again.origin(), UNCALIBRATED_PRIOR_V1.identity());

    // Failures move it the other way, so the direction is not a fixed offset.
    let failures: Vec<RecallCheck> = [1_i64, 2, 3, 4]
        .into_iter()
        .map(|day| {
            RecallCheck::from_contrary(&ContraryEvent::of(
                ContraryKind::NoMemoryMarked,
                concept,
                evidence_id("cal-failure"),
                at(day),
            ))
        })
        .collect();
    let shrunk = UNCALIBRATED_PRIOR_V1.calibrate(&failures, speed);
    assert!(
        shrunk.window_of(PersistenceClass::ExposureOrReview).days()
            < UNCALIBRATED_PRIOR_V1
                .window_of(PersistenceClass::ExposureOrReview)
                .days()
    );
    assert_eq!(shrunk.origin(), UNCALIBRATED_PRIOR_V1.identity());

    // `GATE-38-024`'s other half has no shipped value at all: a speed has to be
    // named, and neither degenerate one is a speed.
    assert!(PersonalizationSpeed::of(0, 5).is_none());
    assert!(PersonalizationSpeed::of(3, 0).is_none());
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. `stale_copy_says_past_mastery_remains`
// ---------------------------------------------------------------------------

#[test]
fn stale_copy_says_past_mastery_remains() -> TestResult {
    let page = specification()?;

    // Section 13.3's own example block, line for line.
    let block = block_after(&page, "### 13.3 Freshness는", "```text")?;
    let line_of = |label: &str| -> Result<String, Box<dyn Error>> {
        Ok(block
            .iter()
            .find_map(|line| line.strip_prefix(label))
            .ok_or_else(|| format!("section 13.3's block has no {label}"))?
            .trim()
            .to_owned())
    };
    assert_eq!(line_of("Meaning:")?, STALE_MEANING);
    assert_eq!(line_of("Action:")?, STALE_ACTION);
    assert_eq!(line_of("Recent use:")?, academic_freshness::NO_RECENT_USE);
    assert_eq!(line_of("Freshness:")?, band_token(FreshnessBand::Stale));

    // Section 34.2's `불확실성 표시` cell for this exact failure.
    let rows = table_after(&page, "### 34.2 Knowledge graph와 state")?;
    let row = rows
        .iter()
        .find(|cells| {
            cells
                .first()
                .is_some_and(|failure| failure.contains("Freshness를 실력 저하로 오인"))
        })
        .ok_or("section 34.2 has no freshness-confusion row")?;
    let disclosure = row.last().ok_or("the row has no 불확실성 표시 cell")?;
    assert!(
        disclosure.contains(STALE_DISCLOSURE),
        "section 34.2 says {disclosure}, not {STALE_DISCLOSURE}"
    );

    // The copy exists only on a stale band.
    let concept = entity("concept-stale-copy");
    let dated = vec![DatedEvidence::at(
        exposure(concept, "stale-lecture")?,
        at(0),
    )];
    let fresh = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(0))?;
    assert_eq!(fresh.band(), FreshnessBand::VeryHigh);
    assert!(StaleDisclosure::of(&fresh).is_none());

    let stale = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(730))?;
    assert_eq!(stale.band(), FreshnessBand::Stale);
    let copy = StaleDisclosure::of(&stale).ok_or("a stale band produced no copy")?;

    // It says the past evidence is retained, and it says the recent-use evidence
    // is absent. Both halves of `REQ-34-045`.
    assert!(copy.meaning().contains("유지되지만"));
    assert!(copy.disclosure().contains("유지"));
    assert!(copy.disclosure().contains("근거 없음"));
    assert_eq!(copy.recent_use(), "none");

    // And it says nothing about the person. The reader is exercised against a
    // string that does contain a forbidden spelling, so the zero below is a
    // measurement rather than a reader that always answers zero.
    let judgement = |text: &str| -> Vec<&'static str> {
        JUDGEMENT_TOKENS
            .into_iter()
            .filter(|token| text.contains(token))
            .collect()
    };
    assert_eq!(
        judgement("이 개념은 모름으로 표시됩니다"),
        vec!["모름"],
        "the reader does not find a spelling that is there"
    );
    assert_eq!(judgement("the user has forgotten this"), vec!["forgotten"]);
    for line in copy.lines() {
        if line == copy.action() {
            // The design document's own `Action` line quotes the forbidden word
            // in order to forbid it: `“모름”으로 표시하지 않음`. The quotation is
            // the instruction, so it is checked for the instruction rather than
            // for the absence of the word it names.
            assert!(line.contains("“모름”으로 표시하지 않음"));
            continue;
        }
        assert_eq!(
            judgement(line),
            Vec::<&str>::new(),
            "the copy judges: {line}"
        );
    }

    // The band the copy belongs to still leaves the mastery alone: the same
    // projection handed to `P2-N2` carries a band and a confidence and nothing
    // else.
    let history = KnowledgeStateHistory::open(
        concept,
        vec![exposure(concept, "stale-lecture")?],
        Vec::new(),
        Vec::new(),
        facets(),
        stale.input(),
        at(730),
    )?;
    let assertion = history.current().ok_or("the history holds no assertion")?;
    assert_eq!(assertion.freshness_band(), FreshnessBand::Stale);
    assert_eq!(assertion.mastery_level(), MasteryLevel::Exposed);
    Ok(())
}

// ---------------------------------------------------------------------------
// Beside the nine.
// ---------------------------------------------------------------------------

/// The permille scale is section 13.1's schema example rather than a tuning
/// constant.
#[test]
fn the_confidence_scale_is_the_schema_examples() -> TestResult {
    let page = specification()?;
    let block = block_after(&page, "### 13.1 Mastery는 학습 깊이", "```yaml")?;
    let designed: Vec<String> = block
        .iter()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once(':')?;
            (key.trim() == "freshnessConfidence").then(|| value.trim().to_owned())
        })
        .collect();
    assert_eq!(designed.len(), 1, "section 13.1 shows {designed:?}");
    let permille: u16 = designed[0]
        .trim_start_matches("0.")
        .parse::<u16>()?
        .saturating_mul(10);
    assert_eq!(permille, 920);
    assert_eq!(
        1000 - u32::from(FRESHNESS_GAP_PERMILLE),
        u32::from(permille)
    );

    // The example's own state: direct evidence, no contradiction, no recall
    // history, spaced repetition — one open gap, the calibration section 13.3
    // says the prior is waiting for.
    let concept = entity("concept-transaction");
    let dated = vec![
        DatedEvidence::at(exposure(concept, "conf-lecture")?, at(0)),
        DatedEvidence::at(exercise(concept, "conf-exercise")?, at(20)),
        DatedEvidence::at(debugging(concept, "conf-debug")?, at(40)),
    ];
    let projection = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(45))?;
    assert_eq!(projection.band(), FreshnessBand::VeryHigh);
    assert_eq!(projection.gaps(), &[ConfidenceGap::PriorUncalibrated]);
    assert_eq!(projection.confidence(), ConfidencePermille::new(920)?);

    // And the projection is the only thing the knowledge state receives.
    let input: FreshnessInput = projection.input();
    assert_eq!(input.band(), FreshnessBand::VeryHigh);
    assert_eq!(input.confidence(), ConfidencePermille::new(920)?);
    Ok(())
}

/// Section 13.2's eighth row has no dated form, because it has no concept.
#[test]
fn no_dated_evidence_can_carry_a_grade() -> TestResult {
    let concept = entity("concept-grade");
    let dossier = full_dossier(concept);

    // Every `ConceptEvidence` variant is one of the seven rows that have one,
    // and none of them is the grade row.
    let items = vec![
        exposure(concept, "grade-lecture")?,
        exercise(concept, "grade-exercise")?,
        debugging(concept, "grade-debug")?,
        admitted(
            ConceptEvidence::SelfExplanation(SelfExplanation::confirmed_by(
                evidence_id("grade-explain"),
                &UserConfirmation::verify(
                    &user_actor(),
                    &Claim {
                        id: claim_id("grade-confirm"),
                        subject_entity_id: concept,
                        predicate_id: PredicateId::parse(
                            academic_knowledge_state::STATE_CONFIRMATION_PREDICATE,
                        )?,
                        object: ClaimObject::Mastery(MasteryLevel::Understood),
                        scope_id: scope(),
                        authority_class: AuthorityClass::UserExplicit,
                        epistemic_status: EpistemicStatus::UserConfirmed,
                        confidence: None,
                        prediction_metadata: None,
                        valid_time: ValidInterval::new(TimestampMillis::new(0), None)?,
                        evidence_ids: vec![evidence_item("grade-explain-item").id],
                    },
                    &evidence_item("grade-explain-item"),
                    concept,
                    MasteryLevel::Understood,
                    at(0),
                )?,
            )),
            "grade-explain",
            &dossier,
        )?,
    ];
    for item in &items {
        let kind = item.kind();
        assert_ne!(kind, EvidenceKind::CourseGrade);
        assert!(
            persistence_class(kind).is_some(),
            "{kind:?} has no persistence class"
        );
        let dated = DatedEvidence::at(item.clone(), at(0));
        assert_eq!(
            dated.window(&UNCALIBRATED_PRIOR_V1),
            UNCALIBRATED_PRIOR_V1
                .window_for(kind)
                .ok_or("a dated item has no window")?
        );
    }

    // The only kind with no window is the one no dated value can carry, and the
    // fallback exists for that unreachable branch alone.
    let without: Vec<EvidenceKind> = EvidenceKind::ALL
        .into_iter()
        .filter(|kind| persistence_class(*kind).is_none())
        .collect();
    assert_eq!(without, vec![EvidenceKind::CourseGrade]);
    Ok(())
}

/// The shipped numbers are not evidence-based, and they do not contradict the
/// two cases the design document does work out.
#[test]
fn the_shipped_prior_does_not_contradict_the_document() -> TestResult {
    let page = specification()?;

    // Section 4: `2년 전 배운 Virtual Memory는 mastery가 유지된 채 freshness가
    // STALE로 보일 수 있고`.
    let sentence = page
        .lines()
        .find(|line| line.contains("2년 전 배운 Virtual Memory"))
        .ok_or("the design document has no two-year case")?;
    assert!(sentence.contains("freshness가 `STALE`"));

    let concept = entity("concept-virtual-memory-two-years");
    let dated = vec![DatedEvidence::at(exposure(concept, "vm-two-years")?, at(0))];
    let two_years = project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(730))?;
    assert_eq!(two_years.band(), FreshnessBand::Stale);

    // Section 4's same sentence: `최근 성능 debugging으로 다시 높아진`.
    assert!(sentence.contains("최근 성능 debugging으로 다시 높아진"));
    let relearned = vec![
        DatedEvidence::at(exposure(concept, "vm-two-years")?, at(0)),
        DatedEvidence::at(debugging(concept, "vm-recent-debug")?, at(725)),
    ];
    let raised = project(concept, inputs(&relearned), &UNCALIBRATED_PRIOR_V1, at(730))?;
    assert!(rank(raised.band()) > rank(two_years.band()));

    // A window is a whole number of days and never zero.
    for class in PersistenceClass::ALL {
        assert!(UNCALIBRATED_PRIOR_V1.window_of(class).days() > 0);
    }
    Ok(())
}

/// Section 13.3's second input distinguishes clustered from spaced repetition.
#[test]
fn repetition_counts_occasions_and_not_items() -> TestResult {
    let concept = entity("concept-repetition");
    let window = UNCALIBRATED_PRIOR_V1.window_of(PersistenceClass::ApplicationOrDesign);

    let clustered: Vec<DatedEvidence> = (0..4)
        .map(|n| -> Result<DatedEvidence, Box<dyn Error>> {
            Ok(DatedEvidence::at(
                exercise(concept, &format!("rep-clustered-{n}"))?,
                TimestampMillis::new(LAST_STRONG_EVIDENCE + n * 3_600_000),
            ))
        })
        .collect::<Result<_, _>>()?;
    let spaced: Vec<DatedEvidence> = (0..4)
        .map(|n| -> Result<DatedEvidence, Box<dyn Error>> {
            Ok(DatedEvidence::at(
                exercise(concept, &format!("rep-spaced-{n}"))?,
                at(n * 30),
            ))
        })
        .collect::<Result<_, _>>()?;

    let now = at(120);
    let clustered_repeats = Repetition::over(&clustered, window, now);
    let spaced_repeats = Repetition::over(&spaced, window, now);
    assert_eq!(clustered_repeats.occasions(), 1);
    assert_eq!(clustered_repeats.repeats(), 0);
    assert_eq!(spaced_repeats.occasions(), 4);
    assert_eq!(spaced_repeats.repeats(), 3);
    assert!(spaced_repeats.span_days() > clustered_repeats.span_days());

    // Same item count, different bands — which is `REQ-13-025`'s own case.
    let from_clustered = project(concept, inputs(&clustered), &UNCALIBRATED_PRIOR_V1, now)?;
    let from_spaced = project(concept, inputs(&spaced), &UNCALIBRATED_PRIOR_V1, now)?;
    assert_eq!(clustered.len(), spaced.len());
    assert!(rank(from_spaced.band()) > rank(from_clustered.band()));
    assert!(
        from_spaced
            .trace()
            .names(FreshnessSignal::RepetitionAndInterval)
    );
    assert!(
        !from_clustered
            .trace()
            .names(FreshnessSignal::RepetitionAndInterval)
    );
    assert!(
        from_clustered
            .gaps()
            .contains(&ConfidenceGap::NoRepetitionInterval)
    );
    Ok(())
}

/// A concept nothing was recorded about is `UNKNOWN`, which is not `STALE`.
#[test]
fn unknown_is_the_absence_of_a_record() -> TestResult {
    let concept = entity("concept-untouched");
    let nothing = project(concept, inputs(&[]), &UNCALIBRATED_PRIOR_V1, at(0))?;
    assert_eq!(nothing.band(), FreshnessBand::Unknown);
    assert_eq!(nothing.last_strong_evidence(), None);
    assert!(nothing.gaps().contains(&ConfidenceGap::NoDirectEvidence));
    assert!(StaleDisclosure::of(&nothing).is_none());

    // A concept with old evidence is `STALE` and carries the instant, which is
    // the difference the copy rests on.
    let known = entity("concept-once-taught");
    let dated = vec![DatedEvidence::at(
        exposure(known, "unknown-lecture")?,
        at(0),
    )];
    let stale = project(known, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(730))?;
    assert_eq!(stale.band(), FreshnessBand::Stale);
    assert_eq!(stale.last_strong_evidence(), Some(at(0)));
    assert_ne!(nothing.band(), stale.band());
    Ok(())
}

/// An input dated after the instant being asked about is a caller error rather
/// than a very fresh input.
#[test]
fn an_input_from_the_future_is_refused() -> TestResult {
    let concept = entity("concept-future");
    let dated = vec![DatedEvidence::at(exercise(concept, "future")?, at(10))];
    assert_eq!(
        project(concept, inputs(&dated), &UNCALIBRATED_PRIOR_V1, at(0)),
        Err(FreshnessError::InputAfterAsOf)
    );
    assert_eq!(
        academic_freshness::elapsed_millis(at(10), at(0)),
        None,
        "a negative elapsed span was answered"
    );
    assert_eq!(
        academic_freshness::elapsed_millis(at(0), at(10)),
        Some(10 * DAY)
    );
    Ok(())
}
