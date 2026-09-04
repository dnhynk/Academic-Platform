//! Synthetic fixtures for the `P2-X5` suite.
//!
//! `CONTRIBUTING.md` rule 1: only synthetic fixtures. Every identity below is a
//! SHA-256 of a tag typed into this file, reshaped into a UUIDv7-shaped value —
//! the helper `P2-N2`'s, `P2-Y1`'s and `P2-Y3`'s suites use — so no clock is
//! read, no record is copied and every run produces the same map.
//!
//! Two fixtures, because they measure different things:
//!
//! * [`small`] is sixteen named nodes whose focus results can be written out by
//!   hand and compared in both directions. Nothing in it is generated.
//! * [`atlas_of`] is the five-thousand-node fixture. It is generated, so it is
//!   generated from a rule stated here rather than from a file.
//!
//! [`timeline`] is transcribed a second time, in another language, by
//! `tools/cs-map-scrubber-oracle.mjs`. If a row here moves, that transcription
//! still says the old answer and `scrubber_matches_the_temporal_oracle` fails —
//! which is the whole reason it is a second transcription and not a render of
//! this one.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use academic_cs_map::{
    CsMapError, GlyphMark, MapEdge, MapGraph, MapNode, NodeReading,
    scrubber::{Appearance, MapEvent, MapTransition, Timeline},
};
use academic_domain::{
    ConfidencePermille, ContentDigest, EntityId, EpistemicStatus, FreshnessBand, MasteryLevel,
    TimestampMillis,
    predicates::{NodeType, PredicateName},
    temporal::TimeCoordinates,
};

/// The suite's result type.
pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A UUIDv7-shaped identity derived from a tag, with no clock read.
pub fn uuid_of(tag: &str) -> uuid::Uuid {
    let digest = ContentDigest::sha256(tag.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

/// The entity identity of a tag.
#[must_use]
pub fn entity(tag: &str) -> EntityId {
    EntityId::try_from_uuid(uuid_of(tag)).unwrap_or_else(|error| unreachable!("{error}"))
}

/// A confidence value, or a panic if the fixture wrote one over 1000.
#[must_use]
pub fn permille(value: u16) -> ConfidencePermille {
    ConfidencePermille::new(value).unwrap_or_else(|error| unreachable!("{error}"))
}

/// A bitemporal reading.
#[must_use]
pub const fn coordinates(known_at: u64, valid_at: i64) -> TimeCoordinates {
    TimeCoordinates::new(known_at, TimestampMillis::new(valid_at))
}

// ---------------------------------------------------------------------------
// The hand-written graph
// ---------------------------------------------------------------------------

/// Every tag in [`small`], so a test can name one without spelling a string
/// twice and so the whole set is enumerable.
pub const SMALL_TAGS: [(&str, NodeType, &str, &str); 16] = [
    ("field.systems", NodeType::Field, "Systems", "field.systems"),
    ("field.theory", NodeType::Field, "Theory", "field.theory"),
    (
        "concept.transaction",
        NodeType::Concept,
        "Transaction",
        "field.systems",
    ),
    (
        "concept.isolation",
        NodeType::Concept,
        "Isolation",
        "field.systems",
    ),
    (
        "concept.locking",
        NodeType::Concept,
        "Locking",
        "field.systems",
    ),
    (
        "concept.logging",
        NodeType::Concept,
        "Logging",
        "field.systems",
    ),
    (
        "concept.serializability",
        NodeType::Concept,
        "Serializability",
        "field.theory",
    ),
    (
        "concept.ordering",
        NodeType::Concept,
        "Ordering",
        "field.theory",
    ),
    (
        "sense.transaction.db",
        NodeType::ConceptSense,
        "Transaction (database sense)",
        "field.systems",
    ),
    (
        "claim.isolation",
        NodeType::Claim,
        "Claim about isolation",
        "field.systems",
    ),
    (
        "evidence.lecture12",
        NodeType::EvidenceItem,
        "Lecture 12 segment",
        "field.systems",
    ),
    (
        "evidence.commit",
        NodeType::EvidenceItem,
        "Commit abc1234",
        "field.systems",
    ),
    (
        "lecture.12",
        NodeType::Lecture,
        "Lecture 12",
        "field.systems",
    ),
    (
        "code.txn-manager",
        NodeType::CodeComponent,
        "TransactionManager",
        "field.systems",
    ),
    (
        "revision.db-2026",
        NodeType::CourseRevision,
        "Databases 2026 revision",
        "field.systems",
    ),
    (
        "goal.reliable-collaboration",
        NodeType::LearningGoal,
        "Reliable collaboration",
        "field.theory",
    ),
];

/// Every edge of [`small`], as tags.
pub const SMALL_EDGES: [(&str, &str, PredicateName); 13] = [
    (
        "concept.transaction",
        "concept.isolation",
        PredicateName::Requires,
    ),
    (
        "concept.isolation",
        "concept.locking",
        PredicateName::Requires,
    ),
    (
        "concept.locking",
        "concept.logging",
        PredicateName::Requires,
    ),
    (
        "concept.serializability",
        "concept.transaction",
        PredicateName::Requires,
    ),
    (
        "concept.ordering",
        "concept.transaction",
        PredicateName::Requires,
    ),
    // Deliberately `BUILDS_ON` and deliberately between two nodes a goal focus
    // already holds: a focus that walked section 7.2's optional edge as a
    // prerequisite would return the same node set and a bigger edge set, and
    // only the edge comparison would catch it.
    (
        "concept.transaction",
        "concept.ordering",
        PredicateName::BuildsOn,
    ),
    (
        "concept.isolation",
        "evidence.lecture12",
        PredicateName::EvidencedBy,
    ),
    (
        "concept.isolation",
        "evidence.commit",
        PredicateName::EvidencedBy,
    ),
    (
        "concept.locking",
        "evidence.commit",
        PredicateName::EvidencedBy,
    ),
    ("concept.transaction", "lecture.12", PredicateName::TaughtIn),
    ("concept.locking", "lecture.12", PredicateName::TaughtIn),
    (
        "revision.db-2026",
        "concept.transaction",
        PredicateName::DesignedToTeach,
    ),
    (
        "revision.db-2026",
        "concept.isolation",
        PredicateName::DesignedToTeach,
    ),
];

/// The sixteen-node graph the focus, search and encoding cases run over.
///
/// # Errors
///
/// Propagates whatever `MapGraph::declare` refuses, so a fixture that broke a
/// declaration rule fails rather than being silently repaired.
pub fn small() -> Result<MapGraph, CsMapError> {
    let mut nodes = Vec::new();
    for (tag, node_type, label, cluster) in SMALL_TAGS {
        nodes.push(MapNode::declare(
            entity(tag),
            node_type,
            label,
            cluster_id(cluster),
        )?);
    }
    let edges = SMALL_EDGES
        .into_iter()
        .map(|(from, to, predicate)| MapEdge {
            from: entity(from),
            to: entity(to),
            predicate,
        })
        .collect();
    MapGraph::declare(nodes, edges)
}

/// The cluster identity of a field tag.
fn cluster_id(tag: &str) -> academic_cs_map::ClusterId {
    academic_cs_map::ClusterId::of_field(entity(tag))
}

// ---------------------------------------------------------------------------
// The generated atlas
// ---------------------------------------------------------------------------

/// How many field clusters the generated fixture declares.
pub const ATLAS_CLUSTERS: usize = 16;

/// The tag of one generated cluster.
#[must_use]
pub fn atlas_cluster_tag(index: usize) -> String {
    format!("cs-map.field.{index:02}")
}

/// The tag of one generated concept.
#[must_use]
pub fn atlas_concept_tag(index: usize) -> String {
    format!("cs-map.concept.{index:05}")
}

/// The label of one generated concept, which is what a search matches on.
#[must_use]
pub fn atlas_concept_label(index: usize) -> String {
    format!("Concept {index:05}")
}

/// A synthetic atlas of exactly `nodes` nodes.
///
/// The rule, stated here rather than stored in a file:
///
/// * the first [`ATLAS_CLUSTERS`] nodes are `FIELD`s, each its own cluster;
/// * concept `i` belongs to cluster `i % ATLAS_CLUSTERS`;
/// * concept `i` `REQUIRES` concept `i - ATLAS_CLUSTERS`, which makes each
///   cluster one long prerequisite chain rather than a hairball;
/// * field `f` is `RELATED_TO` concept `f`, so every cluster is connected to its
///   own chain and the graph has one component per cluster plus the join.
///
/// # Errors
///
/// * [`CsMapError::ClusterCountOutOfRange`] is *not* raised here — it is raised
///   by `Atlas::initial_view`, which is where section 25.3's rule lives.
/// * Propagates `MapGraph::declare`'s refusals.
///
/// # Panics
///
/// When `nodes` is below [`ATLAS_CLUSTERS`], which would be a fixture with no
/// concepts at all.
pub fn atlas_of(nodes: usize) -> Result<MapGraph, CsMapError> {
    assert!(
        nodes > ATLAS_CLUSTERS,
        "a fixture of {nodes} nodes has no concept in it"
    );
    let mut declared = Vec::with_capacity(nodes);
    let mut clusters = Vec::with_capacity(ATLAS_CLUSTERS);
    for index in 0..ATLAS_CLUSTERS {
        let id = entity(&atlas_cluster_tag(index));
        clusters.push(academic_cs_map::ClusterId::of_field(id));
        declared.push(MapNode::declare(
            id,
            NodeType::Field,
            format!("Field {index:02}"),
            clusters[index],
        )?);
    }
    for index in 0..(nodes - ATLAS_CLUSTERS) {
        declared.push(MapNode::declare(
            entity(&atlas_concept_tag(index)),
            NodeType::Concept,
            atlas_concept_label(index),
            clusters[index % ATLAS_CLUSTERS],
        )?);
    }
    let mut edges = Vec::new();
    for index in ATLAS_CLUSTERS..(nodes - ATLAS_CLUSTERS) {
        edges.push(MapEdge {
            from: entity(&atlas_concept_tag(index)),
            to: entity(&atlas_concept_tag(index - ATLAS_CLUSTERS)),
            predicate: PredicateName::Requires,
        });
    }
    for index in 0..ATLAS_CLUSTERS {
        edges.push(MapEdge {
            from: entity(&atlas_cluster_tag(index)),
            to: entity(&atlas_concept_tag(index)),
            predicate: PredicateName::RelatedTo,
        });
    }
    MapGraph::declare(declared, edges)
}

// ---------------------------------------------------------------------------
// Readings
// ---------------------------------------------------------------------------

/// A reading with nothing marked, so a case varies one thing and no more.
#[must_use]
pub fn plain_reading(node: EntityId) -> NodeReading {
    NodeReading {
        node,
        mastery: MasteryLevel::Practiced,
        freshness: FreshnessBand::Moderate,
        status: EpistemicStatus::DeterministicDerived,
        confidence: Some(permille(800)),
        marks: BTreeSet::new(),
        on_active_critical_path: false,
    }
}

/// The readings the uncertainty focus is measured against.
///
/// Five of [`small`]'s nodes carry one and the rest carry none, which is what
/// makes "a node with no reading is not uncertain" observable rather than
/// asserted.
#[must_use]
pub fn small_readings() -> BTreeMap<EntityId, NodeReading> {
    let mut readings = BTreeMap::new();
    let mut put = |tag: &str, status: EpistemicStatus, confidence: Option<u16>| {
        let node = entity(tag);
        let mut reading = plain_reading(node);
        reading.status = status;
        reading.confidence = confidence.map(permille);
        readings.insert(node, reading);
    };
    put(
        "concept.transaction",
        EpistemicStatus::UserConfirmed,
        Some(900),
    );
    put("concept.isolation", EpistemicStatus::AiInferred, Some(700));
    put(
        "concept.locking",
        EpistemicStatus::DeterministicDerived,
        Some(400),
    );
    put("concept.logging", EpistemicStatus::Disputed, None);
    put(
        "concept.serializability",
        EpistemicStatus::OfficialConfirmed,
        Some(1000),
    );
    readings
}

/// Every glyph mark, so a case can vary channel four.
#[must_use]
pub fn every_mark() -> BTreeSet<GlyphMark> {
    academic_cs_map::GLYPH_MARKS.into_iter().collect()
}

// ---------------------------------------------------------------------------
// The timeline
// ---------------------------------------------------------------------------

/// The scrubber fixture, as `(known_at, valid_at, tag, appears, transition)`.
///
/// Typed out rather than generated, because
/// `tools/cs-map-scrubber-oracle.mjs` holds a second transcription of exactly
/// these rows and the two are only useful while they are independent.
pub const TIMELINE_ROWS: [(u64, i64, &str, bool, MapTransition); 12] = [
    (
        10,
        1_000,
        "concept.transaction",
        true,
        MapTransition::EvidenceChange,
    ),
    (
        10,
        1_000,
        "concept.isolation",
        true,
        MapTransition::EvidenceChange,
    ),
    (
        20,
        2_000,
        "concept.locking",
        true,
        MapTransition::EvidenceChange,
    ),
    (
        20,
        5_000,
        "concept.logging",
        true,
        MapTransition::UserScopeChange,
    ),
    (
        30,
        3_000,
        "concept.isolation",
        false,
        MapTransition::OntologyChange,
    ),
    (
        30,
        3_000,
        "sense.transaction.db",
        true,
        MapTransition::OntologyChange,
    ),
    (
        40,
        4_000,
        "concept.serializability",
        true,
        MapTransition::AnalyzerUpgrade,
    ),
    (
        40,
        9_000,
        "concept.ordering",
        true,
        MapTransition::AnalyzerUpgrade,
    ),
    (
        50,
        5_000,
        "concept.locking",
        false,
        MapTransition::OfficialSourceCorrection,
    ),
    (
        50,
        5_000,
        "code.txn-manager",
        true,
        MapTransition::OfficialSourceCorrection,
    ),
    (
        60,
        6_000,
        "concept.isolation",
        true,
        MapTransition::EvidenceChange,
    ),
    (
        70,
        7_000,
        "concept.logging",
        false,
        MapTransition::UserScopeChange,
    ),
];

/// The scrubber positions the oracle and the suite both read.
pub const TIMELINE_READINGS: [(u64, i64); 8] = [
    (5, 500),
    (10, 1_000),
    (20, 2_000),
    (25, 5_000),
    (30, 3_000),
    (40, 9_000),
    (50, 5_000),
    (70, 7_000),
];

/// The timeline itself.
///
/// # Errors
///
/// Propagates `Timeline::declare`'s refusal of an empty list.
pub fn timeline() -> Result<Timeline, CsMapError> {
    Timeline::declare(
        TIMELINE_ROWS
            .into_iter()
            .map(|(known_at, valid_at, tag, appears, transition)| MapEvent {
                at: coordinates(known_at, valid_at),
                subject: entity(tag),
                appearance: if appears {
                    Appearance::Appears
                } else {
                    Appearance::Disappears
                },
                transition,
            })
            .collect(),
    )
}
