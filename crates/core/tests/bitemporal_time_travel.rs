//! Named acceptance evidence for the bitemporal query surface and time travel.
//!
//! # Which lane each test runs against
//!
//! These tests drive a real synthetic profile: a real canonical database, a
//! real acceptance path, and the real disposable time-travel sidecar. That
//! profile is store schema version 1, so it carries the claim lane and no
//! Phase 2 aggregate closure tables — those belong to schema version 2, which
//! only the encrypted lane creates.
//!
//! The aggregate lane's coordinate SQL is therefore proved in
//! `academic-store`'s `aggregate_timeline_tests` against a real schema-2 base
//! with real closure tables. What is proved here is the surface a caller uses:
//! both coordinates are required, a snapshot is disposable, a recomputation
//! difference is attributable, every transition carries one origin, and every
//! named dimension has a defined answer.

mod support;

use std::{error::Error, fs, path::PathBuf};

use academic_domain::{
    AuthorityClass, ClaimRelation, ClaimRelationKind, ContentDigest, EpistemicStatus, PredicateId,
    ScopeId, TimestampMillis,
    temporal::{
        ChangeOrigin, DimensionCarrier, DimensionStep, NAMED_TIME_TRAVEL_DIMENSIONS, TemporalError,
        TimeTravelDimension, TransitionCause, explain_transition,
    },
};
use academic_ledger::AuthorityPolicy;
use academic_projections::{
    bitemporal::{
        AggregateLane, MaterializedSnapshot, ProjectorIdentity, SnapshotAggregateRow,
        TIMELINE_PROJECTOR_VERSION, TimelineStore, explain_recomputation, origin_pure_steps,
    },
    generation::ProjectionCoordinates,
    resolution::PredicatePolicies,
};
use academic_store::{queries::canonical_snapshot, timeline::OriginMarks};

use support::{Fixture, TestResult, claim_id, entity, importer_actor, text_claim};

/// The design document is the authority for the named time-travel dimensions.
const DESIGN_DOCUMENT: &str =
    include_str!("../../../PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md");

const OFFICIAL_PREDICATE: &str = "official.deadline";
const OBSERVED_PREDICATE: &str = "observed.exercise";

/// One synthetic history carrying an evidence change and an official correction.
struct History {
    fixture: Fixture,
    domain_id: academic_domain::DomainId,
    first_official: u64,
    head: u64,
    correction_at: u64,
}

impl History {
    fn build(label: &str) -> Result<Self, Box<dyn Error>> {
        let mut fixture = Fixture::new(label)?;
        let base = fixture.register_scope_evidence(6, 1, b"timeline evidence one")?;
        let second = fixture.register_evidence(6, base.scope_id, 2, b"timeline evidence two")?;
        let third = fixture.register_evidence(6, base.scope_id, 3, b"timeline evidence three")?;

        // An official deadline, applying from the start of the record.
        let first_official = fixture.accept_claim(
            importer_actor(),
            base.domain_id,
            text_claim(
                claim_id(6_001)?,
                entity(6_001)?,
                OFFICIAL_PREDICATE,
                "2027-03-01",
                base.scope_id,
                base.evidence_id,
                AuthorityClass::Official,
                EpistemicStatus::OfficialConfirmed,
                0,
                None,
            )?,
        )?;

        // New evidence about a different subject, applying only from 200.
        fixture.accept_claim(
            importer_actor(),
            base.domain_id,
            text_claim(
                claim_id(6_002)?,
                entity(6_002)?,
                OBSERVED_PREDICATE,
                "practised",
                base.scope_id,
                second.evidence_id,
                AuthorityClass::DirectObservation,
                EpistemicStatus::CodeObserved,
                200,
                None,
            )?,
        )?;

        // The official source corrects itself: a new official claim in the same
        // slot, plus the explicit supersession relation that retires the first.
        fixture.accept_claim(
            importer_actor(),
            base.domain_id,
            text_claim(
                claim_id(6_003)?,
                entity(6_001)?,
                OFFICIAL_PREDICATE,
                "2027-04-01",
                base.scope_id,
                third.evidence_id,
                AuthorityClass::Official,
                EpistemicStatus::OfficialConfirmed,
                0,
                None,
            )?,
        )?;
        let correction_at = fixture.accept_relation(
            importer_actor(),
            base.domain_id,
            ClaimRelation {
                source_claim_id: claim_id(6_003)?,
                target_claim_id: claim_id(6_001)?,
                scope_id: base.scope_id,
                kind: ClaimRelationKind::Supersedes,
            },
        )?;

        Ok(Self {
            domain_id: base.domain_id,
            head: correction_at,
            first_official,
            correction_at,
            fixture,
        })
    }

    fn timeline_path(&self) -> PathBuf {
        self.fixture
            .sidecar_path()
            .with_file_name("timeline.sqlite3")
    }

    fn store(&self) -> Result<TimelineStore, Box<dyn Error>> {
        Ok(TimelineStore::open(self.timeline_path())?)
    }
}

fn registry(version: &str, policy: AuthorityPolicy) -> Result<PredicatePolicies, Box<dyn Error>> {
    Ok(PredicatePolicies::new(
        version,
        [
            (PredicateId::parse(OFFICIAL_PREDICATE)?, policy),
            (PredicateId::parse(OBSERVED_PREDICATE)?, policy),
        ],
    )?)
}

fn projector(policies: &PredicatePolicies) -> ProjectorIdentity {
    ProjectorIdentity::new(
        format!("{TIMELINE_PROJECTOR_VERSION}/{}", policies.version()),
        ContentDigest::sha256(b"timeline-acceptance-binary"),
        policies.canonical_hash(),
    )
}

fn at(known_at_accept_seq: u64, valid_at: i64) -> ProjectionCoordinates {
    ProjectionCoordinates::new(known_at_accept_seq, TimestampMillis::new(valid_at))
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// A reading bound to an earlier known-at coordinate cannot see later knowledge.
#[test]
fn as_known_at_excludes_later_knowledge() -> TestResult {
    let history = History::build("as-known-at")?;
    let policies = registry("timeline-policies-v1", AuthorityPolicy::OfficialFact)?;
    let store = history.store()?;
    let mut reader = history.fixture.store_reader()?;

    let early = store.materialize(
        &mut reader,
        history.domain_id,
        at(history.first_official, 300),
        &policies,
        &projector(&policies),
    )?;
    let late = store.materialize(
        &mut reader,
        history.domain_id,
        at(history.head, 300),
        &policies,
        &projector(&policies),
    )?;

    assert_eq!(
        early.claims.len(),
        1,
        "only the first official claim is known"
    );
    assert_eq!(early.claims[0].claim_id, claim_id(6_001)?);
    assert!(
        early
            .claims
            .iter()
            .all(|row| row.accept_seq <= history.first_official),
        "no row may carry an acceptance after the requested known-at coordinate"
    );

    let late_ids: Vec<_> = late.claims.iter().map(|row| row.claim_id).collect();
    assert!(late_ids.contains(&claim_id(6_002)?));
    assert!(
        late_ids.contains(&claim_id(6_003)?),
        "the correction is knowledge at the head coordinate"
    );
    assert!(
        !late_ids.contains(&claim_id(6_001)?),
        "the superseded official claim is no longer active at the head"
    );
    assert_ne!(early.content_digest(), late.content_digest());
    Ok(())
}

/// One known-at coordinate, two valid instants: history is re-read, not re-known.
#[test]
fn valid_at_reinterprets_history_with_current_knowledge() -> TestResult {
    let history = History::build("valid-at")?;
    let policies = registry("timeline-policies-v1", AuthorityPolicy::OfficialFact)?;
    let store = history.store()?;
    let mut reader = history.fixture.store_reader()?;

    let before_practice = store.materialize(
        &mut reader,
        history.domain_id,
        at(history.head, 100),
        &policies,
        &projector(&policies),
    )?;
    let after_practice = store.materialize(
        &mut reader,
        history.domain_id,
        at(history.head, 300),
        &policies,
        &projector(&policies),
    )?;

    assert_eq!(
        before_practice.coordinates.known_at_accept_seq,
        after_practice.coordinates.known_at_accept_seq,
        "both readings use the same knowledge"
    );
    let earlier: Vec<_> = before_practice
        .claims
        .iter()
        .map(|row| row.claim_id)
        .collect();
    let later: Vec<_> = after_practice
        .claims
        .iter()
        .map(|row| row.claim_id)
        .collect();
    assert!(
        !earlier.contains(&claim_id(6_002)?),
        "the observation does not apply before its valid interval opens"
    );
    assert!(
        later.contains(&claim_id(6_002)?),
        "the same knowledge shows the observation once its interval is reached"
    );
    assert!(
        earlier.contains(&claim_id(6_003)?) && later.contains(&claim_id(6_003)?),
        "the correction applies across both instants because it was known at both"
    );
    assert_ne!(
        before_practice.source_row_digest,
        after_practice.source_row_digest
    );
    Ok(())
}

/// Deleting every snapshot and rebuilding leaves the ledger byte-identical.
#[test]
fn snapshot_deletion_and_rebuild_preserves_ledger() -> TestResult {
    let history = History::build("delete-rebuild")?;
    let policies = registry("timeline-policies-v1", AuthorityPolicy::OfficialFact)?;
    let store = history.store()?;
    let mut reader = history.fixture.store_reader()?;

    let coordinates = [at(history.first_official, 300), at(history.head, 300)];
    let mut originals = Vec::new();
    for coordinate in coordinates {
        originals.push(store.materialize(
            &mut reader,
            history.domain_id,
            coordinate,
            &policies,
            &projector(&policies),
        )?);
    }
    assert_eq!(store.snapshot_count()?, 2);

    let canonical_path = history.fixture.canonical_path().to_path_buf();
    let ledger_before = ContentDigest::sha256(&fs::read(&canonical_path)?);
    let counts_before = canonical_snapshot(&reader)?;

    store.discard()?;
    assert!(
        !history.timeline_path().exists(),
        "discard must remove the sidecar file"
    );

    // Reopening recreates an empty sidecar, which is what "disposable" means.
    let rebuilt_store = TimelineStore::open(history.timeline_path())?;
    assert_eq!(rebuilt_store.snapshot_count()?, 0);

    let mut rebuilt = Vec::new();
    for coordinate in coordinates {
        rebuilt.push(rebuilt_store.materialize(
            &mut reader,
            history.domain_id,
            coordinate,
            &policies,
            &projector(&policies),
        )?);
    }
    assert_eq!(
        rebuilt, originals,
        "a rebuild reproduces the same snapshots"
    );

    let ledger_after = ContentDigest::sha256(&fs::read(&canonical_path)?);
    let counts_after = canonical_snapshot(&reader)?;
    assert_eq!(
        ledger_before, ledger_after,
        "the canonical database bytes must not move when a snapshot is discarded and rebuilt"
    );
    assert_eq!(counts_before, counts_after);
    Ok(())
}

/// Two readings of identical canonical bytes that differ are the projector's doing.
#[test]
fn recomputation_difference_is_explained_as_algorithm_change() -> TestResult {
    let history = History::build("recompute")?;
    let store = history.store()?;
    let mut reader = history.fixture.store_reader()?;
    let coordinates = at(history.head, 300);

    let first_registry = registry("timeline-policies-v1", AuthorityPolicy::OfficialFact)?;
    let second_registry = registry("timeline-policies-v2", AuthorityPolicy::UserOwned)?;
    let before = store.materialize(
        &mut reader,
        history.domain_id,
        coordinates,
        &first_registry,
        &projector(&first_registry),
    )?;
    let after = store.materialize(
        &mut reader,
        history.domain_id,
        coordinates,
        &second_registry,
        &projector(&second_registry),
    )?;

    assert_eq!(
        before.source_row_digest, after.source_row_digest,
        "both readings saw the same canonical bytes"
    );
    assert_ne!(
        before.content_digest(),
        after.content_digest(),
        "the two projectors produced different output"
    );
    assert_eq!(
        explain_recomputation(&before, &after)?,
        ChangeOrigin::AnalyzerUpgrade
    );

    // Two readings by the same projector over the same bytes have nothing to
    // explain, and say so rather than inventing a cause.
    let repeated = store.materialize(
        &mut reader,
        history.domain_id,
        coordinates,
        &second_registry,
        &projector(&second_registry),
    )?;
    assert_eq!(
        explain_recomputation(&after, &repeated),
        Err(TemporalError::UnexplainedTransition)
    );
    Ok(())
}

/// Every transition carries exactly one origin, and all four are reachable.
///
/// The evidence and official-correction marks are read out of the ledger by
/// `origin_marks`; the analyzer mark is the projector axis, which the ledger
/// cannot record. The ontology mark needs the `entity_identity_change` table,
/// which store schema version 2 adds — its canonical derivation is proved in
/// `academic-store`'s `origin_marks_report_an_identity_change_as_an_ontology_change`.
#[test]
fn change_origin_is_labelled_for_every_transition() -> TestResult {
    let history = History::build("change-origin")?;
    let mut reader = history.fixture.store_reader()?;
    let marks = academic_store::timeline::origin_marks(
        &mut reader,
        history.domain_id,
        history.first_official,
        history.head,
    )?;

    assert!(
        marks.official_corrections.contains(&history.correction_at),
        "the official supersession must be read out of the ledger, not assumed"
    );
    assert!(
        !marks.other_acceptances.is_empty(),
        "the interval also holds ordinary evidence"
    );
    assert!(
        !marks.identity_lane_present,
        "a schema-1 profile cannot record an identity change; that is not the same as none"
    );

    let analyzer_at = at(history.head, 300);
    let steps = origin_pure_steps(&marks, TimestampMillis::new(300), Some(analyzer_at))?;
    let explained = explain_transition(
        TimeTravelDimension::Freshness,
        at(history.first_official, 300),
        &steps,
    )?;
    assert_eq!(explained.segments.len(), steps.len());
    let mut seen: Vec<ChangeOrigin> = explained
        .segments
        .iter()
        .map(|segment| segment.origin)
        .collect();
    seen.sort_unstable();
    seen.dedup();
    assert!(seen.contains(&ChangeOrigin::EvidenceChange));
    assert!(seen.contains(&ChangeOrigin::OfficialSourceCorrection));
    assert!(seen.contains(&ChangeOrigin::AnalyzerUpgrade));

    // An identity change in the interval labels as an ontology change; the
    // canonical read that produces such a mark is proved in the store suite.
    let ontology = origin_pure_steps(
        &OriginMarks {
            identity_changes: vec![history.head],
            official_corrections: Vec::new(),
            other_acceptances: Vec::new(),
            identity_lane_present: true,
        },
        TimestampMillis::new(300),
        None,
    )?;
    let ontology = explain_transition(
        TimeTravelDimension::Freshness,
        at(history.first_official, 300),
        &ontology,
    )?;
    assert_eq!(
        ontology.segments.first().map(|segment| segment.origin),
        Some(ChangeOrigin::OntologyChange)
    );

    // A step that mixes two origins is refused instead of ranked.
    let mixed = explain_transition(
        TimeTravelDimension::Freshness,
        at(history.first_official, 300),
        &[DimensionStep {
            at: analyzer_at,
            cause: TransitionCause {
                projector_changed: true,
                identity_changed: false,
                official_correction: true,
                other_evidence: false,
            },
        }],
    );
    assert!(
        matches!(mixed, Err(TemporalError::AmbiguousOrigin { .. })),
        "a mixed step must refuse, not pick a precedence: {mixed:?}"
    );
    Ok(())
}

/// Every dimension the design document names is enumerated and answerable.
///
/// The list is checked against the design document itself: each dimension's
/// exact words are removed from the temporal model's target bullets, and what
/// is left must be punctuation. Dropping an arm leaves its words behind and
/// fails here.
#[test]
fn time_travel_covers_all_fifteen_named_dimensions() -> TestResult {
    let bullets = time_travel_target_bullets()?;
    assert_eq!(bullets.len(), 7, "the temporal model names seven bullets");
    assert_eq!(NAMED_TIME_TRAVEL_DIMENSIONS.len(), 15);

    let mut residue = bullets.clone();
    for dimension in NAMED_TIME_TRAVEL_DIMENSIONS {
        let index = usize::from(dimension.spec_bullet())
            .checked_sub(1)
            .ok_or("spec bullet numbering starts at one")?;
        let line = residue
            .get_mut(index)
            .ok_or_else(|| format!("{} names a bullet that does not exist", dimension.as_str()))?;
        let name = dimension.spec_name();
        let found = line
            .find(name)
            .ok_or_else(|| format!("bullet {index} does not name {name}"))?;
        line.replace_range(found..found + name.len(), "");
    }
    for (index, line) in residue.iter().enumerate() {
        assert!(
            line.chars()
                .all(|character| matches!(character, ',' | '.' | ' ' | '\u{c640}')),
            "bullet {index} still names a target no dimension covers: {line:?}"
        );
    }

    // Every dimension answers, and none answers with a silent empty page.
    let history = History::build("dimension-coverage")?;
    let policies = registry("timeline-policies-v1", AuthorityPolicy::OfficialFact)?;
    let store = history.store()?;
    let mut reader = history.fixture.store_reader()?;
    let snapshot = store.materialize(
        &mut reader,
        history.domain_id,
        at(history.head, 300),
        &policies,
        &projector(&policies),
    )?;
    assert_eq!(snapshot.aggregate_lane, AggregateLane::Absent);

    let mut carried = 0_usize;
    for dimension in NAMED_TIME_TRAVEL_DIMENSIONS {
        match dimension.carrier() {
            DimensionCarrier::Aggregate(kind) => {
                carried += 1;
                assert_eq!(
                    snapshot.dimension(dimension),
                    Err(TemporalError::AggregateLaneAbsent {
                        dimension: dimension.as_str(),
                        carrier: kind,
                    }),
                    "{} must say this profile holds no aggregates",
                    dimension.as_str()
                );
            }
            DimensionCarrier::NotYetCarried => assert_eq!(
                snapshot.dimension(dimension),
                Err(TemporalError::DimensionNotCarried {
                    dimension: dimension.as_str(),
                }),
                "{} must refuse rather than read empty",
                dimension.as_str()
            ),
        }
    }
    assert_eq!(carried, 4);

    // On a profile that does hold the aggregate lane, the same four dimensions
    // read their rows. The canonical read behind such a snapshot is proved in
    // `academic-store`'s `every_carried_dimension_resolves_to_a_projected_arm`.
    let present = MaterializedSnapshot {
        aggregate_lane: AggregateLane::Present,
        aggregates: NAMED_TIME_TRAVEL_DIMENSIONS
            .iter()
            .enumerate()
            .filter_map(|(index, dimension)| match dimension.carrier() {
                DimensionCarrier::Aggregate(kind) => Some(carried_row(kind, index)),
                DimensionCarrier::NotYetCarried => None,
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
        ..snapshot
    };
    for dimension in NAMED_TIME_TRAVEL_DIMENSIONS {
        match dimension.carrier() {
            DimensionCarrier::Aggregate(_) => assert_eq!(
                present.dimension(dimension).map(|rows| rows.len()),
                Ok(1),
                "{} must read its carrier's row",
                dimension.as_str()
            ),
            DimensionCarrier::NotYetCarried => assert!(present.dimension(dimension).is_err()),
        }
    }
    Ok(())
}

/// One aggregate row standing in for a carrier that a schema-2 profile holds.
fn carried_row(kind: &str, index: usize) -> Result<SnapshotAggregateRow, Box<dyn Error>> {
    let seed = u8::try_from(index)?;
    Ok(SnapshotAggregateRow {
        kind: kind.to_owned(),
        aggregate_id: [seed; 16],
        registered_event_id: [seed.wrapping_add(1); 16],
        accept_seq: 1,
        scope_id: support::scoped_id::<ScopeId>(0x10, 6, 1)?,
        source_digest: None,
        valid_from: TimestampMillis::new(0),
        valid_to: None,
    })
}

/// Returns the temporal model's time-travel target bullets, without their marker.
fn time_travel_target_bullets() -> Result<Vec<String>, Box<dyn Error>> {
    let start = DESIGN_DOCUMENT
        .find("### 31.3 Time travel")
        .ok_or("the design document has no time-travel target section")?;
    let body = &DESIGN_DOCUMENT[start..];
    let end = body
        .find("### 31.4")
        .ok_or("the time-travel target section is unterminated")?;
    Ok(body[..end]
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::to_owned)
        .collect())
}
