//! Fixtures for the `P2-R6` acceptance suite.
//!
//! The scenario is section 20.1's own: `실시간 협업 편집기를 만들고 싶다`, with
//! that section's three success criteria, its two constraints and its two
//! unresolved decisions. The overlays the readiness comparison reads are **real**
//! `P2-N5` overlays over real `P2-N2` eligibility and real `P2-N3` bands — the
//! `academic-gap` fixture module is included by `#[path]` rather than restated,
//! the way `academic-critical-path`'s own suite includes it.
//!
//! Nothing here reads a clock or opens a socket. Every instant is an offset from
//! `P2-N5`'s `ORIGIN`, every identifier is a SHA-256 of its own name with the
//! UUIDv7 nibbles set, and the one directory opened is the `tempfile` the lecture
//! fixture writes its capture journal into.

// Three targets include this module by `#[path]`, and each uses a different
// subset of what it re-exports, so an unused item here is a property of the
// caller rather than of this file.
#![allow(
    dead_code,
    unused_imports,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]

#[path = "../../../gap/tests/common/mod.rs"]
pub mod gap;

use std::{collections::BTreeMap, error::Error};

use academic_build_learn::{
    ActualCoverage, Alternative, ArchitectureBranch, BranchGroup, BuildLearnError,
    ConceptRequirement, Constraint, Constraints, CoverageEvidenceKind, EvidenceTask,
    ExplainedByHand, GoalInput, LearningItem, MotivationEdge, NonEmptyText, ObservableCriterion,
    ObservableResponsibility, PartId, PlanStep, ProjectGoal, ReadingDone,
    ResponsibilityDecomposition, ReturnCheckpoint, SelectionApproved, SimulationPassed,
    SuccessCriteria, UnresolvedDecision, UnresolvedDecisions, normalize,
};
use academic_critical_path::EdgeStanding;
use academic_domain::{
    EntityId, EvidenceId, FreshnessBand,
    entity_registry::EntityKind,
    predicates::{PredicateName, PrerequisiteStrength},
};
use academic_freshness::{DatedEvidence, FreshnessProjection};
use academic_gap::{ConceptState, IdentityStanding, OfferedEvidence, PrerequisiteEdge};
use academic_knowledge_state::EligibilityOutcome;

pub use gap::{
    DAY, ORIGIN, TestResult, at, entity, evidence_id, exercise_evidence, exposure_evidence,
    full_dossier, offered, scope, unknown_band, uuid_of,
};

// ---------------------------------------------------------------------------
// The five concepts section 20.1's example is about.
// ---------------------------------------------------------------------------

/// `shared-state semantics`: the conjunction's first member.
#[must_use]
pub fn shared_state() -> EntityId {
    entity("concept-shared-state-semantics")
}

/// `failure model`: the conjunction's second member.
#[must_use]
pub fn failure_model() -> EntityId {
    entity("concept-failure-model")
}

/// `OT fundamentals`: the first branch of the `OT vs CRDT` decision.
#[must_use]
pub fn ot_fundamentals() -> EntityId {
    entity("concept-ot-fundamentals")
}

/// `CRDT fundamentals`: the second branch of the same decision.
#[must_use]
pub fn crdt_fundamentals() -> EntityId {
    entity("concept-crdt-fundamentals")
}

/// `central server ordering`: the first branch of the ordering decision.
#[must_use]
pub fn central_ordering() -> EntityId {
    entity("concept-central-server-ordering")
}

/// `peer/offline merge`: the second branch of the ordering decision.
#[must_use]
pub fn peer_merge() -> EntityId {
    entity("concept-peer-offline-merge")
}

/// The capability node every requirement is stated about.
#[must_use]
pub fn realtime_collaboration() -> EntityId {
    entity("capability-realtime-collaboration")
}

/// A concept `P2-R4` published a `WOULD_BENEFIT_FROM` contract for.
#[must_use]
pub fn replication() -> EntityId {
    entity("concept-replication")
}

/// Every concept the fixture goal reaches, in a fixed order.
#[must_use]
pub fn all_concepts() -> Vec<EntityId> {
    vec![
        shared_state(),
        failure_model(),
        ot_fundamentals(),
        crdt_fundamentals(),
        central_ordering(),
        peer_merge(),
        replication(),
    ]
}

// ---------------------------------------------------------------------------
// Small constructors.
// ---------------------------------------------------------------------------

pub fn text(value: &str) -> Result<NonEmptyText, Box<dyn Error>> {
    Ok(NonEmptyText::new(value)?)
}

pub fn id(value: &str) -> Result<PartId, Box<dyn Error>> {
    Ok(PartId::new(value)?)
}

// ---------------------------------------------------------------------------
// Section 20.1's goal, built through the whole chain.
// ---------------------------------------------------------------------------

/// The natural-language input section 20.1's `text` field holds.
pub fn editor_input() -> Result<GoalInput, Box<dyn Error>> {
    Ok(GoalInput::NaturalLanguageFeature {
        sentence: text("실시간 협업 편집기를 만들고 싶다")?,
    })
}

/// Section 20.1's three `successCriteria`, verbatim, each with its observation.
pub fn editor_criteria() -> Result<SuccessCriteria, Box<dyn Error>> {
    let criteria = vec![
        ObservableCriterion::state(
            id("converge")?,
            text("concurrent edits converge according to chosen semantics")?,
            text("two clients apply an interleaved edit log and compare documents")?,
        ),
        ObservableCriterion::state(
            id("reconnect")?,
            text("reconnect does not silently lose acknowledged edits")?,
            text("acknowledged edits are replayed after a forced disconnect")?,
        ),
        ObservableCriterion::state(
            id("latency")?,
            text("user-visible latency target is stated")?,
            text("the stated target is read back off the recorded budget")?,
        ),
    ];
    SuccessCriteria::of(criteria).ok_or_else(|| "the fixture criteria were empty".into())
}

/// Section 20.1's two `constraints`, verbatim.
pub fn editor_constraints() -> Result<Constraints, Box<dyn Error>> {
    Ok(Constraints::of(vec![
        Constraint::fixed(id("web-client")?, text("web client")?),
        Constraint::fixed(
            id("single-region")?,
            text("current single-region deployment")?,
        ),
    ]))
}

/// Section 20.1's two `unresolvedDecisions`, verbatim.
pub fn editor_decisions() -> Result<UnresolvedDecisions, Box<dyn Error>> {
    Ok(UnresolvedDecisions::of(vec![
        UnresolvedDecision::open(
            id("ordering")?,
            text("central ordering vs peer/offline merge")?,
            vec![
                Alternative::named(id("central")?, text("central ordering")?),
                Alternative::named(id("peer")?, text("peer/offline merge")?),
            ],
        )?,
        UnresolvedDecision::open(
            id("merge-algorithm")?,
            text("OT vs CRDT conditional branch")?,
            vec![
                Alternative::named(id("ot")?, text("OT")?),
                Alternative::named(id("crdt")?, text("CRDT")?),
            ],
        )?,
    ]))
}

/// Section 20.1's `ProjectGoal`, stated through `normalize` and `state`.
pub fn editor_goal() -> Result<ProjectGoal, Box<dyn Error>> {
    let input = editor_input()?;
    let intent = normalize(&input)?;
    Ok(ProjectGoal::state(
        &intent,
        editor_criteria()?,
        editor_constraints()?,
        editor_decisions()?,
    )?)
}

/// One observable responsibility per criterion, plus a second for the first.
pub fn editor_responsibilities() -> Result<Vec<ObservableResponsibility>, Box<dyn Error>> {
    Ok(vec![
        ObservableResponsibility::of(
            id("merge-order")?,
            id("converge")?,
            text("apply concurrent operations in an order both clients agree on")?,
            text("two clients that applied the same operations show different text")?,
        ),
        ObservableResponsibility::of(
            id("ack-durability")?,
            id("reconnect")?,
            text("hold acknowledged operations until the peer confirms receipt")?,
            text("an acknowledged edit is missing after a reconnect")?,
        ),
        ObservableResponsibility::of(
            id("budget")?,
            id("latency")?,
            text("record the round-trip budget the interaction is designed against")?,
            text("nobody can say whether an observed delay is within target")?,
        ),
    ])
}

/// Section 20.2's first stage over the fixture goal.
pub fn editor_decomposition() -> Result<ResponsibilityDecomposition, Box<dyn Error>> {
    Ok(ResponsibilityDecomposition::decompose(
        editor_goal()?,
        editor_responsibilities()?,
    )?)
}

/// Section 20.2's second stage: two mandatory members and two open decisions.
///
/// ```text
/// realtime collaboration
///   REQUIRES ALL [shared-state semantics, failure model]
///   REQUIRES ONE OF (ordering)
///     ├─ [central server ordering]
///     └─ [peer/offline merge]
///   REQUIRES ONE OF (merge-algorithm)
///     ├─ [OT fundamentals]
///     └─ [CRDT fundamentals]
/// ```
pub fn editor_branch() -> Result<ArchitectureBranch, Box<dyn Error>> {
    let conjunction = vec![
        ConceptRequirement::always(shared_state(), EntityKind::Concept, id("merge-order")?)?,
        ConceptRequirement::always(failure_model(), EntityKind::Concept, id("ack-durability")?)?,
    ];
    let groups = vec![
        BranchGroup::of(
            id("ordering")?,
            id("central")?,
            vec![(central_ordering(), EntityKind::Concept, id("merge-order")?)],
        )?,
        BranchGroup::of(
            id("ordering")?,
            id("peer")?,
            vec![(peer_merge(), EntityKind::Concept, id("ack-durability")?)],
        )?,
        BranchGroup::of(
            id("merge-algorithm")?,
            id("ot")?,
            vec![(ot_fundamentals(), EntityKind::Concept, id("merge-order")?)],
        )?,
        BranchGroup::of(
            id("merge-algorithm")?,
            id("crdt")?,
            vec![(crdt_fundamentals(), EntityKind::Concept, id("merge-order")?)],
        )?,
    ];
    Ok(ArchitectureBranch::of(
        editor_decomposition()?,
        realtime_collaboration(),
        conjunction,
        groups,
    )?)
}

// ---------------------------------------------------------------------------
// `P2-N5` edges, so the branch can be turned into a `P2-N6` hypergraph.
// ---------------------------------------------------------------------------

/// One admitted `REQUIRES` edge from the capability to each concept.
pub fn editor_edges() -> Result<BTreeMap<EntityId, PrerequisiteEdge>, Box<dyn Error>> {
    let mut found = BTreeMap::new();
    for concept in all_concepts() {
        found.insert(
            concept,
            PrerequisiteEdge::admit(
                PredicateName::Requires,
                PrerequisiteStrength::Hard,
                realtime_collaboration(),
                concept,
                vec![evidence_id(&format!("{concept}-edge"))],
            )?,
        );
    }
    Ok(found)
}

/// The standing every fixture edge carries.
#[must_use]
pub const fn settled() -> EdgeStanding {
    EdgeStanding::Settled
}

// ---------------------------------------------------------------------------
// Real `P2-N5` overlays.
// ---------------------------------------------------------------------------

fn dated(concept: EntityId, tag: &str, days_after: i64) -> Result<DatedEvidence, Box<dyn Error>> {
    let admitted = match EligibilityOutcome::admit(
        exercise_evidence(&format!("{concept}-{tag}-dated")),
        evidence_id(&format!("{concept}-{tag}-dated")),
        &full_dossier(concept),
    ) {
        EligibilityOutcome::Admitted(value) => value,
        EligibilityOutcome::Blocked(blocked) => {
            return Err(format!("the fixture was blocked: {:?}", blocked.reasons()).into());
        }
    };
    Ok(DatedEvidence::at(admitted, at(days_after)))
}

fn band(
    concept: EntityId,
    dated: &[DatedEvidence],
    expected: FreshnessBand,
) -> Result<FreshnessProjection, Box<dyn Error>> {
    gap::band_from(concept, dated, &[], at(0), expected)
}

fn overlay(
    concept: EntityId,
    offered_items: Vec<OfferedEvidence>,
    freshness: &FreshnessProjection,
) -> Result<ConceptState, Box<dyn Error>> {
    Ok(ConceptState::overlay(
        concept,
        EntityKind::Concept,
        IdentityStanding::Settled,
        &offered_items,
        freshness,
        &[],
    )?)
}

/// `충분하고 최근인 evidence`: two eligible items and a `VERY_HIGH` band.
pub fn ready_state(concept: EntityId) -> Result<ConceptState, Box<dyn Error>> {
    let recent = dated(concept, "ready", 0)?;
    let band = band(concept, &[recent], FreshnessBand::VeryHigh)?;
    let offered_items = vec![
        offered(
            exercise_evidence(&format!("{concept}-ready-a")),
            &format!("{concept}-ready-a"),
            full_dossier(concept),
        ),
        offered(
            exercise_evidence(&format!("{concept}-ready-b")),
            &format!("{concept}-ready-b"),
            full_dossier(concept),
        ),
    ];
    overlay(concept, offered_items, &band)
}

/// `mastery evidence는 있으나 stale`: the same two items, a `STALE` band.
pub fn stale_state(concept: EntityId) -> Result<ConceptState, Box<dyn Error>> {
    let old = dated(concept, "stale", -400)?;
    let band = band(concept, &[old], FreshnessBand::Stale)?;
    let offered_items = vec![
        offered(
            exercise_evidence(&format!("{concept}-ready-a")),
            &format!("{concept}-ready-a"),
            full_dossier(concept),
        ),
        offered(
            exercise_evidence(&format!("{concept}-ready-b")),
            &format!("{concept}-ready-b"),
            full_dossier(concept),
        ),
    ];
    overlay(concept, offered_items, &band)
}

/// `evidence 부족`: one eligible item, which `P2-N2` reports as a gap.
pub fn thin_state(concept: EntityId) -> Result<ConceptState, Box<dyn Error>> {
    let recent = dated(concept, "thin", 0)?;
    let band = band(concept, &[recent], FreshnessBand::VeryHigh)?;
    let offered_items = vec![offered(
        exposure_evidence(&format!("{concept}-thin"))?,
        &format!("{concept}-thin"),
        full_dossier(concept),
    )];
    overlay(concept, offered_items, &band)
}

// ---------------------------------------------------------------------------
// Learning items and plan steps.
// ---------------------------------------------------------------------------

/// A four-stage approval, built through the chain that requires all four.
pub fn approval(concept: EntityId, tag: &str) -> Result<SelectionApproved, Box<dyn Error>> {
    let reading = ReadingDone::of(text(&format!("the {tag} chapter on {concept}"))?);
    let explained = ExplainedByHand::after(
        reading,
        text("two clients merge the same pair of operations to the same document")?,
    );
    let simulated = SimulationPassed::after(
        explained,
        text("a twelve-line harness that replays both orders")?,
    );
    Ok(SelectionApproved::after(
        simulated,
        id("merge-algorithm")?,
        id("crdt")?,
    ))
}

/// A learning item that returns to `returns_to`.
pub fn learning(
    item_id: &str,
    concept: EntityId,
    returns_to: &str,
) -> Result<LearningItem, Box<dyn Error>> {
    Ok(LearningItem::plan(
        id(item_id)?,
        concept,
        EvidenceTask::of(
            text("run the twelve-line replay harness over both operation orders")?,
            text("that the two orders reach one document")?,
        ),
        ReturnCheckpoint::of(approval(concept, item_id)?, id(returns_to)?),
    ))
}

/// An implementation step toward one criterion.
pub fn implementation(step_id: &str, criterion: &str) -> Result<PlanStep, Box<dyn Error>> {
    Ok(PlanStep::Implementation {
        id: id(step_id)?,
        satisfies: id(criterion)?,
        builds: text("the merge path, behind the flag the harness drives")?,
    })
}

/// An experiment step answering one open decision.
pub fn experiment(step_id: &str, decision: &str) -> Result<PlanStep, Box<dyn Error>> {
    Ok(PlanStep::Experiment {
        id: id(step_id)?,
        answers: id(decision)?,
        runs: text("a one-afternoon spike over both alternatives")?,
    })
}

// ---------------------------------------------------------------------------
// Motivation edges.
// ---------------------------------------------------------------------------

/// The three motivation edges section 20.3's own sentence gives, on one concept.
pub fn three_motivations(concept: EntityId) -> Result<Vec<MotivationEdge>, Box<dyn Error>> {
    Ok(vec![
        MotivationEdge::of(
            academic_build_learn::Motivation::Project,
            concept,
            text("이번 주 project 때문에")?,
        ),
        MotivationEdge::of(
            academic_build_learn::Motivation::School,
            concept,
            text("다음 강의 prerequisite라서")?,
        ),
        MotivationEdge::of(
            academic_build_learn::Motivation::Role,
            concept,
            text("장기 systems path에서 재사용")?,
        ),
    ])
}

// ---------------------------------------------------------------------------
// Section 21 fixtures: `P2-U1` revisions and offerings, built through that
// crate's own drafts.
// ---------------------------------------------------------------------------

use academic_curriculum::{
    CourseCode, CourseOffering, CourseOfferingDraft, CourseRevision, CourseRevisionDraft,
    CourseTitle, Credits, GradingMode, OfferingStatus, SectionCode, TermCode,
};
use academic_domain::{
    ContentDigest, CourseId, CourseRevisionId, CurriculumVersionId, OfferingId, TimestampMillis,
    ValidInterval,
};

fn course_id(tag: &str) -> CourseId {
    CourseId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn revision_id(tag: &str) -> CourseRevisionId {
    CourseRevisionId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn version_id(tag: &str) -> CurriculumVersionId {
    CurriculumVersionId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn offering_id(tag: &str) -> OfferingId {
    OfferingId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

fn interval() -> ValidInterval {
    ValidInterval::open_ended(TimestampMillis::new(ORIGIN))
}

/// A revision whose title reads like the subject and whose designed coverage is
/// empty. Section 21.1's `“데이터베이스” 과목 이름만 보고` case.
pub fn title_only_revision() -> Result<CourseRevision, Box<dyn Error>> {
    Ok(CourseRevisionDraft::new(
        revision_id("revision-title-only"),
        course_id("course-title-only"),
        version_id("curriculum-2027"),
        CourseCode::parse("M1522.001800")?,
        interval(),
    )
    .title(CourseTitle::parse("데이터베이스")?)
    .credits(Credits::new(3)?)
    .build()?)
}

/// A revision that names `subject` in its designed coverage.
pub fn covering_revision(subject: EntityId) -> Result<CourseRevision, Box<dyn Error>> {
    Ok(CourseRevisionDraft::new(
        revision_id("revision-covering"),
        course_id("course-covering"),
        version_id("curriculum-2027"),
        CourseCode::parse("M1522.001800")?,
        interval(),
    )
    .title(CourseTitle::parse("데이터베이스")?)
    .credits(Credits::new(3)?)
    .designed_concept(subject)
    .build()?)
}

/// One offering of `revision`, confirmed and observed at the fixture origin.
pub fn offering_for(revision: &CourseRevision) -> Result<CourseOffering, Box<dyn Error>> {
    Ok(CourseOfferingDraft::new(
        offering_id("offering-2027-1"),
        revision.id(),
        TermCode::parse("T20271")?,
        SectionCode::parse("SEC001")?,
        OfferingStatus::Confirmed,
        TimestampMillis::new(ORIGIN),
    )
    .grading_mode(GradingMode::Letter)
    .build())
}

/// A measured `P2-N6` estimate on the effort axis, for the channel comparison.
pub fn estimate(
    low: u32,
    high: u32,
) -> Result<academic_critical_path::CostEstimate, Box<dyn Error>> {
    use academic_critical_path::{BasisFamily, CostBasis, CostComponent, CostEstimate};
    Ok(CostEstimate::of(
        low,
        high,
        CostComponent::LearningEffort.unit(),
        CostBasis::measured(&[
            BasisFamily::StateAndFreshness,
            BasisFamily::ConceptGranularity,
        ])?,
    )?)
}
