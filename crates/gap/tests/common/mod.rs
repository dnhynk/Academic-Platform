//! Fixtures for the `P2-N5` acceptance suite.
//!
//! Section 36.4's own scenario, because it is the design document's worked
//! example of exactly this engine: `Buffer Pool` is the surface concept of an
//! active goal, `Disk Page` is the root the engine finds one hop below it,
//! `Storage Hierarchy` sits below that, and section 15.1's `GapCase` block lists
//! the last two as its two root candidates.
//!
//! Every state input is produced by the crate that owns it. Mastery evidence
//! goes through `P2-N2`'s `EligibilityOutcome::admit`, and the exposure rows are
//! built from a `P2-L4` document that a real `P2-L2` capture and a real `P2-L3`
//! run produced — `crates/knowledge-state/tests/common/mod.rs` is included by
//! `#[path]` for that, the way `academic-freshness`'s suite includes it. Bands
//! are produced by `P2-N3`'s own `project`, never asserted into existence.
//!
//! Nothing here reads a clock, opens a socket or records anything: every instant
//! is an offset from [`ORIGIN`], every identifier is a SHA-256 of its own name
//! with the UUIDv7 nibbles set, and the one directory that is opened is a
//! `tempfile` the lecture fixture writes its capture journal into.

#![allow(dead_code, clippy::missing_panics_doc, clippy::missing_errors_doc)]

#[path = "../../../knowledge-state/tests/common/mod.rs"]
pub mod ks;

use std::error::Error;

use academic_domain::{
    ContentDigest, EntityId, EvidenceId, FreshnessBand, MasteryLevel, ScopeId, TimestampMillis,
    entity_registry::EntityKind,
    predicates::{PredicateName, PrerequisiteStrength},
};
use academic_freshness::{
    DatedEvidence, FreshnessInputs, FreshnessProjection, RetentionPrior, Spillover,
    UNCALIBRATED_PRIOR_V1,
};
use academic_gap::{
    ActiveGoal, ConceptReading, GoalCriteria, IdentityStanding, LinkedContext, OfferedEvidence,
    PrerequisiteEdge, PrerequisiteGraph, SuccessCriterion,
};
use academic_knowledge_state::{
    ConceptEvidence, ConceptLink, EvidenceDossier, ExerciseOutcome, IncidentRepair, Outcome,
    Participation, SourceIntegrity, TeachingSite,
};
use academic_lecture_document::{LectureDocument, NodeId};

pub type TestResult = Result<(), Box<dyn Error>>;

/// Milliseconds in a day.
pub const DAY: i64 = 86_400_000;

/// The instant every fixture is dated from. `2025-07-12` as Unix milliseconds,
/// which is section 13.3's own `Last strong evidence:` line and the instant
/// `P2-N3`'s suite uses.
pub const ORIGIN: i64 = 1_752_278_400_000;

// ---------------------------------------------------------------------------
// Identities.
// ---------------------------------------------------------------------------

#[must_use]
pub fn uuid_of(tag: &str) -> uuid::Uuid {
    let digest = ContentDigest::sha256(tag.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

#[must_use]
pub fn entity(tag: &str) -> EntityId {
    EntityId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

#[must_use]
pub fn evidence_id(tag: &str) -> EvidenceId {
    EvidenceId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

#[must_use]
pub fn scope() -> ScopeId {
    ScopeId::try_from_uuid(uuid_of("scope-gap")).unwrap_or_else(|error| unreachable!("{error}"))
}

#[must_use]
pub fn at(days_after: i64) -> TimestampMillis {
    TimestampMillis::new(ORIGIN + days_after * DAY)
}

/// Section 36.4's concepts.
#[must_use]
pub fn buffer_pool() -> EntityId {
    entity("BUFFER_POOL")
}

#[must_use]
pub fn disk_page() -> EntityId {
    entity("DISK_PAGE")
}

#[must_use]
pub fn storage_hierarchy() -> EntityId {
    entity("STORAGE_HIERARCHY")
}

#[must_use]
pub fn random_io() -> EntityId {
    entity("RANDOM_IO")
}

#[must_use]
pub fn fan_out() -> EntityId {
    entity("FAN_OUT")
}

// ---------------------------------------------------------------------------
// `P2-N2` evidence.
// ---------------------------------------------------------------------------

#[must_use]
pub fn full_dossier(concept: EntityId) -> EvidenceDossier {
    EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Authored,
        Outcome::Succeeded,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    )
}

/// A dossier whose authorship check does not resolve, so `P2-N2` blocks it.
#[must_use]
pub fn unresolved_authorship(concept: EntityId) -> EvidenceDossier {
    EvidenceDossier::of(
        ConceptLink::Exact(concept, EntityKind::Concept),
        Participation::Unknown,
        Outcome::Succeeded,
        SourceIntegrity::Verified(ContentDigest::sha256(b"artifact")),
    )
}

/// A `P2-L4` document, built by driving a real `P2-L2` capture and `P2-L3` run.
pub struct Lecture {
    _directory: tempfile::TempDir,
    pub document: LectureDocument,
}

pub fn lecture_document(tag: &str) -> Result<Lecture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let capture = ks::lecture::clean_capture(&directory, tag)?;
    let manifest = ks::lecture::full_manifest(&capture)?;
    let transcribed = ks::lecture::transcribe(&manifest)?;
    let capture_seq = ks::lecture::capture_frame_seq(&capture)
        .ok_or("the fixture capture holds no board photograph")?;
    let document = ks::lecture::whole_document(transcribed.lineage(), &manifest, capture_seq)?;
    Ok(Lecture {
        _directory: directory,
        document,
    })
}

pub fn teaching(lecture: &Lecture) -> Result<TeachingSite, Box<dyn Error>> {
    let node: &NodeId = lecture
        .document
        .nodes()
        .first()
        .ok_or("the fixture document has no node")?
        .id();
    Ok(TeachingSite::in_document(&lecture.document, node)?)
}

/// Section 13.2's first row, from a real `P2-L4` document.
pub fn exposure_evidence(tag: &str) -> Result<ConceptEvidence, Box<dyn Error>> {
    let lecture = lecture_document(tag)?;
    Ok(ConceptEvidence::MeaningfulTeaching(teaching(&lecture)?))
}

/// Section 13.2's third row, succeeded.
#[must_use]
pub fn exercise_evidence(tag: &str) -> ConceptEvidence {
    ConceptEvidence::ConceptExercise(ExerciseOutcome::succeeded(evidence_id(tag)))
}

/// Section 13.2's third row, failed. `P2-N2` keeps it as contradicting evidence.
#[must_use]
pub fn failed_exercise_evidence(tag: &str) -> ConceptEvidence {
    ConceptEvidence::ConceptExercise(ExerciseOutcome::failed(evidence_id(tag)))
}

/// Section 13.2's fifth row.
#[must_use]
pub fn debugging_evidence(tag: &str) -> ConceptEvidence {
    ConceptEvidence::IncidentDebugging(IncidentRepair::of(
        evidence_id(&format!("{tag}-incident")),
        evidence_id(&format!("{tag}-cause")),
        evidence_id(&format!("{tag}-fix")),
        evidence_id(&format!("{tag}-verified")),
    ))
}

#[must_use]
pub fn offered(evidence: ConceptEvidence, tag: &str, dossier: EvidenceDossier) -> OfferedEvidence {
    OfferedEvidence::of(evidence, evidence_id(tag), dossier)
}

// ---------------------------------------------------------------------------
// `P2-N3` bands.
// ---------------------------------------------------------------------------

#[must_use]
pub fn prior() -> &'static RetentionPrior {
    &UNCALIBRATED_PRIOR_V1
}

/// A projection over no input at all, which `P2-N3` reads as `UNKNOWN`.
pub fn unknown_band(concept: EntityId) -> Result<FreshnessProjection, Box<dyn Error>> {
    let projection = academic_freshness::project(
        concept,
        FreshnessInputs {
            dated: &[],
            spillover: &[],
            statements: &[],
            contrary: &[],
        },
        prior(),
        at(0),
    )?;
    assert_eq!(
        projection.band(),
        FreshnessBand::Unknown,
        "the fixture meant to build an UNKNOWN band"
    );
    Ok(projection)
}

/// A projection over one dated item, checked against the band the fixture meant.
///
/// The band is produced by `P2-N3`'s `project` and then *verified*, so a fixture
/// that stops producing what it says it produces fails here rather than making
/// some later assertion vacuous.
pub fn band_from(
    concept: EntityId,
    dated: &[DatedEvidence],
    spillover: &[Spillover],
    as_of: TimestampMillis,
    expected: FreshnessBand,
) -> Result<FreshnessProjection, Box<dyn Error>> {
    let projection = academic_freshness::project(
        concept,
        FreshnessInputs {
            dated,
            spillover,
            statements: &[],
            contrary: &[],
        },
        prior(),
        as_of,
    )?;
    assert_eq!(
        projection.band(),
        expected,
        "the fixture meant to build {expected:?} for {concept}"
    );
    Ok(projection)
}

// ---------------------------------------------------------------------------
// The graph and the goal.
// ---------------------------------------------------------------------------

pub fn requires(
    advanced: EntityId,
    prerequisite: EntityId,
    strength: PrerequisiteStrength,
    tag: &str,
) -> Result<PrerequisiteEdge, Box<dyn Error>> {
    Ok(PrerequisiteEdge::admit(
        PredicateName::Requires,
        strength,
        advanced,
        prerequisite,
        vec![evidence_id(tag)],
    )?)
}

pub fn builds_on(
    advanced: EntityId,
    foundation: EntityId,
    strength: PrerequisiteStrength,
    tag: &str,
) -> Result<PrerequisiteEdge, Box<dyn Error>> {
    Ok(PrerequisiteEdge::admit(
        PredicateName::BuildsOn,
        strength,
        advanced,
        foundation,
        vec![evidence_id(tag)],
    )?)
}

/// Section 36.4's chain: `Buffer Pool` requires `Disk Page` hard, and
/// `Disk Page` requires `Storage Hierarchy` near-hard.
pub fn section_36_4_graph() -> Result<PrerequisiteGraph, Box<dyn Error>> {
    Ok(PrerequisiteGraph::new()
        .with(requires(
            buffer_pool(),
            disk_page(),
            PrerequisiteStrength::Hard,
            "edge-buffer-pool-disk-page",
        )?)
        .with(requires(
            disk_page(),
            storage_hierarchy(),
            PrerequisiteStrength::Strong,
            "edge-disk-page-storage-hierarchy",
        )?))
}

/// Section 36.3's goal, with the success criteria step 1 requires.
pub fn understand_buffer_pool() -> Result<ActiveGoal, Box<dyn Error>> {
    let criteria = GoalCriteria::of(vec![SuccessCriterion::concept(
        buffer_pool(),
        EntityKind::Concept,
        MasteryLevel::Practiced,
    )?])
    .ok_or("the fixture criteria are empty")?;
    Ok(ActiveGoal::declare(
        entity("goal-understand-buffer-pool"),
        scope(),
        buffer_pool(),
        EntityKind::Concept,
        criteria,
    )?)
}

/// One reading with everything section 15.3 requires already filled in, so a
/// test that wants to observe one field can change one field.
pub fn reading(concept: EntityId, freshness: FreshnessProjection) -> ConceptReading {
    ConceptReading {
        concept,
        kind: EntityKind::Concept,
        identity: IdentityStanding::Settled,
        offered: Vec::new(),
        freshness,
        spillover: Vec::new(),
        reason: "page가 I/O와 buffer frame의 교환 단위이기 때문".to_owned(),
        remediation_minutes: 25,
        remediation_description: "page layout을 직접 재는 25분짜리 실험".to_owned(),
        remediation_sources: vec![evidence_id("lecture-segment-page-layout")],
        linked: LinkedContext {
            lectures: vec![entity("lecture-storage-04")],
            projects: Vec::new(),
        },
        alternative_routes: Vec::new(),
    }
}
