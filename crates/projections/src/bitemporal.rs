//! Disposable materialized time-travel snapshots and change-origin labelling.
//!
//! # What this is and is not
//!
//! The canonical store is opened read-only and every row written here goes to a
//! third sidecar database with its own `application_id`, beside the Phase 1
//! graph and search sidecar. A snapshot is a cache of what the ledger already
//! says at one exact coordinate pair; it is never the truth, it is never backed
//! up or exported as truth, and [`TimelineStore::discard`] deletes the whole
//! file. `snapshot_deletion_and_rebuild_preserves_ledger` is the executable
//! form of that claim.
//!
//! # Both coordinates, always
//!
//! Every entry point takes [`ProjectionCoordinates`], which carries
//! `known_at_accept_seq` and `valid_at` together. There is no `current()`, no
//! `latest()`, and no single-coordinate overload, so a caller cannot ask a
//! question whose answer would silently mix "what was known then" with "what
//! applies now".
//!
//! # Two lanes
//!
//! A snapshot reads two canonical lanes at the same coordinates: the eighteen
//! Phase 2 aggregate closure tables, and the resolved claim lane the Phase 1
//! store already projects. A profile that carries no aggregate tables records
//! [`AggregateLane::Absent`] rather than zero aggregate rows, because zero rows
//! would read as "nothing was registered by this coordinate".

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use academic_domain::{
    ClaimId, ClaimObject, ContentDigest, DomainId, EntityId, PredicateId, ScopeId, TimestampMillis,
    temporal::{
        ChangeOrigin, DimensionCarrier, DimensionStep, ExplainedTransition, TemporalError,
        TimeTravelDimension, TransitionCause, explain_transition,
    },
};
use academic_store::{
    connection::ReaderConnection,
    queries::{AuthorityPolicy, ProjectionSnapshotRequest, QueryError, projection_source_snapshot},
    timeline::{
        AggregateTimelineRequest, AggregateTimelineRow, OriginMarks, aggregate_timeline_snapshot,
    },
};
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior, params};

use crate::{
    generation::ProjectionCoordinates,
    resolution::{
        PredicatePolicies, authority_name, authority_policy_name, epistemic_name,
        parse_authority_policy,
    },
    runner::{ProjectionError, ProjectionResult, fixed_bytes, id_from_bytes},
};

/// Application identity for the disposable time-travel sidecar (`ACTL`).
pub const TIMELINE_APPLICATION_ID: u32 = 0x4143_544C;
/// Physical sidecar version.
pub const TIMELINE_DATABASE_VERSION: u32 = 1;
/// The sidecar's only migration, embedded byte-for-byte.
pub const TIMELINE_MIGRATION_SQL: &str =
    include_str!("../../../migrations/store/0001_phase2_bitemporal.sql");
/// Identifier of the projector this build materializes snapshots with.
pub const TIMELINE_PROJECTOR_VERSION: &str = "phase2-aggregate-and-claim-timeline-v1";

/// Whether the canonical profile carries the Phase 2 aggregate closure tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateLane {
    /// The eighteen closure tables exist and were read.
    Present,
    /// The profile carries none of them; this is not "no aggregate registered".
    Absent,
}

impl AggregateLane {
    /// Returns the stable persisted spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "PRESENT",
            Self::Absent => "ABSENT",
        }
    }

    fn parse(value: &str) -> ProjectionResult<Self> {
        match value {
            "PRESENT" => Ok(Self::Present),
            "ABSENT" => Ok(Self::Absent),
            _ => Err(ProjectionError::Corrupt(
                "timeline aggregate lane is invalid".to_owned(),
            )),
        }
    }
}

/// Exactly which projector produced a snapshot.
///
/// Recorded per snapshot so that a recomputation whose result differs while the
/// source digests are equal is attributable to the code and to nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectorIdentity {
    /// Stable projector version string.
    pub version: String,
    /// Digest of the binary that ran it.
    pub binary_digest: ContentDigest,
    /// Digest of the effective configuration it ran with.
    pub config_hash: ContentDigest,
}

impl ProjectorIdentity {
    /// Constructs a projector identity.
    #[must_use]
    pub fn new(
        version: impl Into<String>,
        binary_digest: ContentDigest,
        config_hash: ContentDigest,
    ) -> Self {
        Self {
            version: version.into(),
            binary_digest,
            config_hash,
        }
    }
}

/// One aggregate registration held by a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotAggregateRow {
    /// Event schema v3 arm that registered the aggregate.
    pub kind: String,
    /// Opaque aggregate identifier.
    pub aggregate_id: [u8; 16],
    /// Identifier of the event that registered it.
    pub registered_event_id: [u8; 16],
    /// Replica-local acceptance order of that event.
    pub accept_seq: u64,
    /// Scope the aggregate is registered under.
    pub scope_id: ScopeId,
    /// Optional provenance digest recorded with the registration.
    pub source_digest: Option<ContentDigest>,
    /// Inclusive start of the aggregate's half-open valid interval.
    pub valid_from: TimestampMillis,
    /// Exclusive end of that interval, absent when still open.
    pub valid_to: Option<TimestampMillis>,
}

/// One resolved active claim held by a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotClaimRow {
    /// Identifier of the resolved claim.
    pub claim_id: ClaimId,
    /// Subject the claim is about.
    pub subject_entity_id: EntityId,
    /// Predicate the claim asserts.
    pub predicate_id: PredicateId,
    /// Scope the claim is confined to.
    pub scope_id: ScopeId,
    /// Replica-local acceptance order of the asserting event.
    pub accept_seq: u64,
    /// Authority class carried by the claim.
    pub authority_class: String,
    /// Epistemic status carried by the claim.
    pub epistemic_status: String,
    /// Predicate policy that selected it.
    pub applied_policy: AuthorityPolicy,
    /// Object kind, from the closed nine-value canonical enum.
    pub object_kind: String,
    /// Inclusive start of the claim's half-open valid interval.
    pub valid_from: TimestampMillis,
    /// Exclusive end of that interval, absent when still open.
    pub valid_to: Option<TimestampMillis>,
}

/// One materialized reading of both canonical lanes at exact coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedSnapshot {
    /// Deterministic identifier of this snapshot.
    pub snapshot_id: [u8; 16],
    /// Security domain the reading is confined to.
    pub security_domain: DomainId,
    /// The two mandatory coordinates.
    pub coordinates: ProjectionCoordinates,
    /// Exactly which projector produced it.
    pub projector: ProjectorIdentity,
    /// Canonical source-ledger authority digest at the known-at coordinate.
    pub source_ledger_digest: ContentDigest,
    /// Digest over the canonical input this reading was bound to.
    ///
    /// It commits to the coordinates, the aggregate rows visible at them, and
    /// the source-ledger authority at the known-at coordinate. It deliberately
    /// commits to no projector output, so two readings that agree on it were
    /// bound to identical canonical input and any difference in what they
    /// produced is the projector's.
    pub source_row_digest: ContentDigest,
    /// Whether the profile carries the aggregate closure tables at all.
    pub aggregate_lane: AggregateLane,
    /// Highest acceptance sequence the canonical store held.
    pub latest_accept_seq: u64,
    /// Aggregate registrations visible at the coordinates.
    pub aggregates: Vec<SnapshotAggregateRow>,
    /// Resolved active claims visible at the coordinates.
    pub claims: Vec<SnapshotClaimRow>,
}

impl MaterializedSnapshot {
    /// Returns the aggregate rows one named dimension reads.
    ///
    /// # Errors
    ///
    /// Returns [`TemporalError::DimensionNotCarried`] for a dimension with no
    /// landed canonical carrier, and [`TemporalError::AggregateLaneAbsent`]
    /// when the carrier exists but this profile holds no aggregate tables.
    /// Both refusals are deliberate: an empty vector would read as "this
    /// dimension had no activity", which is a different statement from either.
    pub fn dimension(
        &self,
        dimension: TimeTravelDimension,
    ) -> Result<Vec<&SnapshotAggregateRow>, TemporalError> {
        match dimension.carrier() {
            DimensionCarrier::Aggregate(kind) if self.aggregate_lane == AggregateLane::Absent => {
                Err(TemporalError::AggregateLaneAbsent {
                    dimension: dimension.as_str(),
                    carrier: kind,
                })
            }
            DimensionCarrier::Aggregate(kind) => Ok(self
                .aggregates
                .iter()
                .filter(|row| row.kind == kind)
                .collect()),
            DimensionCarrier::NotYetCarried => Err(TemporalError::DimensionNotCarried {
                dimension: dimension.as_str(),
            }),
        }
    }

    /// Digest over everything a projector produced, in persisted order.
    ///
    /// Two snapshots of the same canonical bytes whose content digests differ
    /// differ because the projector changed.
    #[must_use]
    pub fn content_digest(&self) -> ContentDigest {
        let mut material = Vec::new();
        material.extend_from_slice(b"ACADEMIC_TIMELINE_SNAPSHOT_CONTENT_V1\0");
        material.extend_from_slice(&self.aggregate_lane.as_str().len().to_be_bytes());
        material.extend_from_slice(self.aggregate_lane.as_str().as_bytes());
        for row in &self.aggregates {
            material.extend_from_slice(row.kind.as_bytes());
            material.extend_from_slice(&row.aggregate_id);
            material.extend_from_slice(&row.accept_seq.to_be_bytes());
            material.extend_from_slice(&row.valid_from.value().to_be_bytes());
        }
        for row in &self.claims {
            material.extend_from_slice(row.claim_id.as_bytes());
            material.extend_from_slice(&row.accept_seq.to_be_bytes());
            material.extend_from_slice(row.object_kind.as_bytes());
            // The applied predicate policy is projector output, not canonical
            // input, so a registry change moves this digest while the source
            // digests stay equal. That is what makes such a difference
            // attributable to the algorithm.
            material.extend_from_slice(authority_policy_name(row.applied_policy).as_bytes());
            material.extend_from_slice(&row.valid_from.value().to_be_bytes());
        }
        ContentDigest::sha256(&material)
    }
}

/// Disposable sidecar holding materialized time-travel snapshots.
#[derive(Debug, Clone)]
pub struct TimelineStore {
    database_path: PathBuf,
}

impl TimelineStore {
    /// Opens or creates the disposable time-travel sidecar.
    ///
    /// # Errors
    ///
    /// Fails when the path holds a database that is not this sidecar format.
    pub fn open(database_path: impl AsRef<Path>) -> ProjectionResult<Self> {
        let database_path = database_path.as_ref().to_path_buf();
        migrate_timeline_database(&database_path)?;
        Ok(Self { database_path })
    }

    /// Returns the sidecar path.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Reads both canonical lanes at exact coordinates and materializes them.
    ///
    /// Re-materializing the same coordinates with the same projector replaces
    /// the previous snapshot, which is what makes a snapshot disposable rather
    /// than a second history.
    ///
    /// # Errors
    ///
    /// Propagates canonical read failures and sidecar write failures.
    pub fn materialize(
        &self,
        canonical: &mut ReaderConnection,
        security_domain: DomainId,
        coordinates: ProjectionCoordinates,
        policies: &PredicatePolicies,
        projector: &ProjectorIdentity,
    ) -> ProjectionResult<MaterializedSnapshot> {
        let snapshot =
            read_canonical(canonical, security_domain, coordinates, policies, projector)?;
        self.persist(&snapshot)?;
        Ok(snapshot)
    }

    /// Returns the persisted snapshot for exact coordinates and projector version.
    ///
    /// # Errors
    ///
    /// Propagates sidecar read failures and refuses corrupt persisted rows.
    pub fn snapshot(
        &self,
        security_domain: DomainId,
        coordinates: ProjectionCoordinates,
        projector_version: &str,
    ) -> ProjectionResult<Option<MaterializedSnapshot>> {
        let connection = self.open_reader()?;
        read_persisted(&connection, security_domain, coordinates, projector_version)
    }

    /// Returns how many snapshots the sidecar holds.
    ///
    /// # Errors
    ///
    /// Propagates sidecar read failures.
    pub fn snapshot_count(&self) -> ProjectionResult<u64> {
        let connection = self.open_reader()?;
        let count: i64 =
            connection.query_row("SELECT count(*) FROM timeline_snapshot", [], |row| {
                row.get(0)
            })?;
        u64::try_from(count)
            .map_err(|_| ProjectionError::Corrupt("timeline snapshot count is negative".to_owned()))
    }

    /// Deletes the whole sidecar, which is always safe.
    ///
    /// Nothing here is canonical: every row can be recomputed from the ledger,
    /// so discarding the file loses no history. Reopening recreates an empty
    /// sidecar.
    ///
    /// # Errors
    ///
    /// Propagates filesystem failures other than "already gone".
    pub fn discard(&self) -> ProjectionResult<()> {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.database_path.clone().into_os_string();
            path.push(suffix);
            match fs::remove_file(PathBuf::from(path)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn open_reader(&self) -> ProjectionResult<Connection> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = Connection::open_with_flags(&self.database_path, flags)?;
        verify_timeline_format(&connection)?;
        Ok(connection)
    }

    fn persist(&self, snapshot: &MaterializedSnapshot) -> ProjectionResult<()> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut connection = Connection::open_with_flags(&self.database_path, flags)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        verify_timeline_format(&connection)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "DELETE FROM timeline_snapshot WHERE snapshot_id = ?1",
            params![snapshot.snapshot_id.as_slice()],
        )?;
        transaction.execute(
            concat!(
                "INSERT INTO timeline_snapshot (snapshot_id, security_domain, ",
                "known_at_accept_seq, valid_at_unix_ms, projector_version, ",
                "projector_binary_digest, projector_config_hash, source_ledger_digest, ",
                "source_row_digest, aggregate_lane, latest_accept_seq, built_at_unix_ms, ",
                "aggregate_row_count, claim_row_count) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                snapshot.snapshot_id.as_slice(),
                snapshot.security_domain.as_bytes().as_slice(),
                checked_i64(snapshot.coordinates.known_at_accept_seq)?,
                snapshot.coordinates.valid_at.value(),
                snapshot.projector.version.as_str(),
                snapshot.projector.binary_digest.as_bytes().as_slice(),
                snapshot.projector.config_hash.as_bytes().as_slice(),
                snapshot.source_ledger_digest.as_bytes().as_slice(),
                snapshot.source_row_digest.as_bytes().as_slice(),
                snapshot.aggregate_lane.as_str(),
                checked_i64(snapshot.latest_accept_seq)?,
                unix_time_millis()?,
                i64::try_from(snapshot.aggregates.len())
                    .map_err(|_| ProjectionError::IntegerOverflow(0))?,
                i64::try_from(snapshot.claims.len())
                    .map_err(|_| ProjectionError::IntegerOverflow(0))?,
            ],
        )?;
        for row in &snapshot.aggregates {
            transaction.execute(
                concat!(
                    "INSERT INTO timeline_snapshot_aggregate (snapshot_id, aggregate_kind, ",
                    "aggregate_id, registered_event_id, accept_seq, scope_id, source_digest, ",
                    "valid_from_unix_ms, valid_to_unix_ms) ",
                    "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
                ),
                params![
                    snapshot.snapshot_id.as_slice(),
                    row.kind.as_str(),
                    row.aggregate_id.as_slice(),
                    row.registered_event_id.as_slice(),
                    checked_i64(row.accept_seq)?,
                    row.scope_id.as_bytes().as_slice(),
                    row.source_digest.map(|digest| digest.as_bytes().to_vec()),
                    row.valid_from.value(),
                    row.valid_to.map(TimestampMillis::value),
                ],
            )?;
        }
        for row in &snapshot.claims {
            transaction.execute(
                concat!(
                    "INSERT INTO timeline_snapshot_claim (snapshot_id, claim_id, ",
                    "subject_entity_id, predicate_id, scope_id, accept_seq, authority_class, ",
                    "epistemic_status, applied_policy, object_kind, valid_from_unix_ms, ",
                    "valid_to_unix_ms) ",
                    "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
                ),
                params![
                    snapshot.snapshot_id.as_slice(),
                    row.claim_id.as_bytes().as_slice(),
                    row.subject_entity_id.as_bytes().as_slice(),
                    row.predicate_id.as_str(),
                    row.scope_id.as_bytes().as_slice(),
                    checked_i64(row.accept_seq)?,
                    row.authority_class.as_str(),
                    row.epistemic_status.as_str(),
                    authority_policy_name(row.applied_policy),
                    row.object_kind.as_str(),
                    row.valid_from.value(),
                    row.valid_to.map(TimestampMillis::value),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

/// Reads both canonical lanes at exact coordinates without writing anything.
fn read_canonical(
    canonical: &mut ReaderConnection,
    security_domain: DomainId,
    coordinates: ProjectionCoordinates,
    policies: &PredicatePolicies,
    projector: &ProjectorIdentity,
) -> ProjectionResult<MaterializedSnapshot> {
    let aggregate_request = AggregateTimelineRequest {
        domain_id: security_domain,
        coordinates,
    };
    let (aggregate_lane, aggregates, aggregate_row_digest, aggregate_head) =
        match aggregate_timeline_snapshot(canonical, &aggregate_request) {
            Ok(reading) => (
                AggregateLane::Present,
                reading
                    .rows
                    .iter()
                    .map(aggregate_row)
                    .collect::<Vec<SnapshotAggregateRow>>(),
                reading.source_row_digest,
                reading.latest_accept_seq,
            ),
            Err(QueryError::AggregatesAbsent { .. }) => (
                AggregateLane::Absent,
                Vec::new(),
                ContentDigest::sha256(b"ACADEMIC_TIMELINE_AGGREGATE_LANE_ABSENT_V1"),
                0,
            ),
            Err(error) => return Err(ProjectionError::CanonicalQuery(error)),
        };

    let claim_reading = projection_source_snapshot(
        canonical,
        &ProjectionSnapshotRequest {
            domain_id: security_domain,
            valid_at: coordinates.valid_at,
            known_at_accept_seq: coordinates.known_at_accept_seq,
            predicate_policies: policies.entries(),
        },
    )
    .map_err(ProjectionError::CanonicalQuery)?;

    let mut claims: Vec<SnapshotClaimRow> = claim_reading
        .resolved_claims
        .iter()
        .map(|resolved| SnapshotClaimRow {
            claim_id: resolved.claim.id,
            subject_entity_id: resolved.claim.subject_entity_id,
            predicate_id: resolved.claim.predicate_id.clone(),
            scope_id: resolved.claim.scope_id,
            accept_seq: resolved.accept_seq,
            authority_class: authority_name(resolved.claim.authority_class).to_owned(),
            epistemic_status: epistemic_name(resolved.claim.epistemic_status).to_owned(),
            applied_policy: resolved.applied_policy,
            object_kind: object_kind(&resolved.claim.object).to_owned(),
            valid_from: resolved.claim.valid_time.from(),
            valid_to: resolved.claim.valid_time.to(),
        })
        .collect();
    claims.sort_by(|left, right| {
        (left.accept_seq, left.claim_id).cmp(&(right.accept_seq, right.claim_id))
    });

    let latest_accept_seq = aggregate_head.max(claim_reading.latest_accept_seq);
    let snapshot_id = snapshot_identity(security_domain, coordinates, &projector.version);
    Ok(MaterializedSnapshot {
        snapshot_id,
        security_domain,
        coordinates,
        projector: projector.clone(),
        source_ledger_digest: claim_reading.source_ledger_digest,
        source_row_digest: canonical_input_digest(
            coordinates,
            aggregate_row_digest,
            claim_reading.source_ledger_digest,
        ),
        aggregate_lane,
        latest_accept_seq,
        aggregates,
        claims,
    })
}

fn aggregate_row(row: &AggregateTimelineRow) -> SnapshotAggregateRow {
    SnapshotAggregateRow {
        kind: row.kind.to_owned(),
        aggregate_id: row.aggregate_id,
        registered_event_id: row.registered_event_id,
        accept_seq: row.accept_seq,
        scope_id: row.scope_id,
        source_digest: row.source_digest,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
    }
}

/// Names the closed nine-value canonical object kind.
const fn object_kind(object: &ClaimObject) -> &'static str {
    match object {
        ClaimObject::Entity(_) => "ENTITY",
        ClaimObject::Text(_) => "TEXT",
        ClaimObject::Integer(_) => "INTEGER",
        ClaimObject::Boolean(_) => "BOOLEAN",
        ClaimObject::Decimal(_) => "DECIMAL",
        ClaimObject::Instant(_) => "INSTANT",
        ClaimObject::Interval(_) => "INTERVAL",
        ClaimObject::Mastery(_) => "MASTERY",
        ClaimObject::Freshness(_) => "FRESHNESS",
    }
}

/// Derives a snapshot identifier from what the snapshot is of.
///
/// Deterministic rather than random, so rebuilding a discarded snapshot at the
/// same coordinates with the same projector produces the same identity and the
/// rebuild can be compared byte for byte.
fn snapshot_identity(
    security_domain: DomainId,
    coordinates: ProjectionCoordinates,
    projector_version: &str,
) -> [u8; 16] {
    let mut material = Vec::new();
    material.extend_from_slice(b"ACADEMIC_TIMELINE_SNAPSHOT_ID_V1\0");
    material.extend_from_slice(security_domain.as_bytes());
    material.extend_from_slice(&coordinates.known_at_accept_seq.to_be_bytes());
    material.extend_from_slice(&coordinates.valid_at.value().to_be_bytes());
    material.extend_from_slice(projector_version.as_bytes());
    let digest = ContentDigest::sha256(&material);
    let mut identity = [0_u8; 16];
    identity.copy_from_slice(&digest.as_bytes()[..16]);
    identity
}

/// Commits to the canonical input one reading was bound to.
///
/// The coordinates are part of it because they are what selected the input: two
/// readings at different coordinates were bound to different canonical input
/// even when the ledger prefix behind them is the same. No projector output is
/// mixed in, which is what lets `explain_recomputation` read an equal digest as
/// "the same canonical bytes" rather than "the same answer".
fn canonical_input_digest(
    coordinates: ProjectionCoordinates,
    aggregate: ContentDigest,
    ledger: ContentDigest,
) -> ContentDigest {
    let mut material = Vec::with_capacity(112);
    material.extend_from_slice(b"ACADEMIC_TIMELINE_CANONICAL_INPUT_V1\0");
    material.extend_from_slice(&coordinates.known_at_accept_seq.to_be_bytes());
    material.extend_from_slice(&coordinates.valid_at.value().to_be_bytes());
    material.extend_from_slice(aggregate.as_bytes());
    material.extend_from_slice(ledger.as_bytes());
    ContentDigest::sha256(&material)
}

/// Turns the ledger's origin-bearing acceptances into origin-pure steps.
///
/// One acceptance carries one payload arm, so each mark contributes exactly one
/// step with exactly one cause. That is what makes the known-time interval
/// splittable without a precedence rule: an interval holding an identity
/// change, an official correction, and other evidence becomes three steps
/// rather than one step that has to pick a winner.
///
/// `projector_change_at` appends the step a projector version change
/// contributes, which is the one axis the ledger cannot record.
///
/// # Errors
///
/// Refuses a mark whose origin discriminant is not one of the canonical three.
pub fn origin_pure_steps(
    marks: &OriginMarks,
    valid_at: TimestampMillis,
    projector_change_at: Option<ProjectionCoordinates>,
) -> ProjectionResult<Vec<DimensionStep>> {
    let mut steps: Vec<DimensionStep> = marks
        .ordered()
        .into_iter()
        .map(|(accept_seq, origin)| {
            let cause = match origin {
                "ONTOLOGY_CHANGE" => TransitionCause::identity(),
                "OFFICIAL_SOURCE_CORRECTION" => TransitionCause::official(),
                "EVIDENCE_CHANGE" => TransitionCause::evidence(),
                _ => {
                    return Err(ProjectionError::Corrupt(
                        "origin mark discriminant is invalid".to_owned(),
                    ));
                }
            };
            Ok(DimensionStep {
                at: ProjectionCoordinates::new(accept_seq, valid_at),
                cause,
            })
        })
        .collect::<ProjectionResult<Vec<DimensionStep>>>()?;
    if let Some(at) = projector_change_at {
        steps.push(DimensionStep {
            at,
            cause: TransitionCause::projector(),
        });
    }
    Ok(steps)
}

/// Explains how one dimension moved between successive readings.
///
/// The caller supplies steps that are already origin-pure. Splitting a
/// known-time interval at each origin-bearing acceptance, and holding the
/// projector fixed across a recomputation, is what makes them so; this function
/// refuses rather than guessing when that was not done.
///
/// # Errors
///
/// Propagates [`explain_transition`]'s refusals.
pub fn explain_dimension_transition(
    dimension: TimeTravelDimension,
    origin_at: ProjectionCoordinates,
    steps: &[DimensionStep],
) -> Result<ExplainedTransition, TemporalError> {
    explain_transition(dimension, origin_at, steps)
}

/// Derives what differed between two snapshots of the same dimension.
///
/// Only the projector axis can be decided from the snapshots alone; the three
/// canonical axes need the acceptance-order marks the caller collected from the
/// ledger, which is why they are arguments rather than guesses.
#[must_use]
pub fn transition_cause(
    before: &MaterializedSnapshot,
    after: &MaterializedSnapshot,
    identity_changed: bool,
    official_correction: bool,
) -> TransitionCause {
    let projector_changed = before.projector.version != after.projector.version;
    let source_changed = before.source_row_digest != after.source_row_digest;
    TransitionCause {
        projector_changed,
        identity_changed,
        official_correction,
        other_evidence: source_changed && !identity_changed && !official_correction,
    }
}

/// Names the origin a recomputation difference is attributable to.
///
/// When two readings share their source digests and differ in content, nothing
/// canonical moved, so the difference is the projector's.
///
/// # Errors
///
/// Returns [`TemporalError::UnexplainedTransition`] when the content is equal
/// and [`TemporalError::AmbiguousOrigin`] when the canonical source moved too,
/// because that difference is not the projector's alone.
pub fn explain_recomputation(
    before: &MaterializedSnapshot,
    after: &MaterializedSnapshot,
) -> Result<ChangeOrigin, TemporalError> {
    let source_changed = before.source_row_digest != after.source_row_digest;
    let content_changed = before.content_digest() != after.content_digest();
    let projector_changed = before.projector.version != after.projector.version;
    if !content_changed {
        return Err(TemporalError::UnexplainedTransition);
    }
    TransitionCause {
        projector_changed,
        identity_changed: false,
        official_correction: false,
        other_evidence: source_changed,
    }
    .label()
}

// ---------------------------------------------------------------------------
// Sidecar format
// ---------------------------------------------------------------------------

fn migrate_timeline_database(path: &Path) -> ProjectionResult<()> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = Connection::open_with_flags(path, flags)?;
    let application_id = pragma_i64(&connection, "application_id")?;
    let user_version = pragma_i64(&connection, "user_version")?;
    if application_id == 0 && user_version == 0 && user_object_count(&connection)? == 0 {
        connection.execute_batch(TIMELINE_MIGRATION_SQL)?;
        return verify_timeline_format(&connection);
    }
    verify_timeline_format(&connection)
}

fn verify_timeline_format(connection: &Connection) -> ProjectionResult<()> {
    let application_id = pragma_i64(connection, "application_id")?;
    let user_version = pragma_i64(connection, "user_version")?;
    if application_id != i64::from(TIMELINE_APPLICATION_ID)
        || user_version != i64::from(TIMELINE_DATABASE_VERSION)
    {
        return Err(ProjectionError::UnsupportedProjectionFormat {
            application_id,
            user_version,
            reason: "time-travel sidecar identity does not match this build",
        });
    }
    Ok(())
}

fn pragma_i64(connection: &Connection, name: &str) -> ProjectionResult<i64> {
    Ok(connection.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?)
}

fn user_object_count(connection: &Connection) -> ProjectionResult<i64> {
    Ok(connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
        [],
        |row| row.get(0),
    )?)
}

type RawSnapshot = (
    Vec<u8>,
    i64,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    String,
    i64,
);

fn read_persisted(
    connection: &Connection,
    security_domain: DomainId,
    coordinates: ProjectionCoordinates,
    projector_version: &str,
) -> ProjectionResult<Option<MaterializedSnapshot>> {
    let raw: Option<RawSnapshot> = connection
        .query_row(
            concat!(
                "SELECT snapshot_id, latest_accept_seq, projector_binary_digest, ",
                "projector_config_hash, source_ledger_digest, source_row_digest, ",
                "aggregate_lane, aggregate_row_count FROM timeline_snapshot ",
                "WHERE security_domain = ?1 AND known_at_accept_seq = ?2 ",
                "AND valid_at_unix_ms = ?3 AND projector_version = ?4"
            ),
            params![
                security_domain.as_bytes().as_slice(),
                checked_i64(coordinates.known_at_accept_seq)?,
                coordinates.valid_at.value(),
                projector_version,
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        // Only "no such snapshot" is absence. Any other SQLite failure is a
        // corrupt or unreadable sidecar and must not read as "not materialized
        // yet", which would silently rebuild over a database that is broken.
        .optional()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let snapshot_id = fixed_bytes::<16>(raw.0, "timeline snapshot identifier")?;
    let aggregates = read_persisted_aggregates(connection, &snapshot_id)?;
    let claims = read_persisted_claims(connection, &snapshot_id)?;
    if i64::try_from(aggregates.len()).map_err(|_| ProjectionError::IntegerOverflow(0))? != raw.7 {
        return Err(ProjectionError::Corrupt(
            "timeline snapshot aggregate row count does not match its rows".to_owned(),
        ));
    }
    Ok(Some(MaterializedSnapshot {
        snapshot_id,
        security_domain,
        coordinates,
        projector: ProjectorIdentity {
            version: projector_version.to_owned(),
            binary_digest: ContentDigest::from_sha256_bytes(fixed_bytes::<32>(
                raw.2,
                "timeline projector binary digest",
            )?),
            config_hash: ContentDigest::from_sha256_bytes(fixed_bytes::<32>(
                raw.3,
                "timeline projector config hash",
            )?),
        },
        source_ledger_digest: ContentDigest::from_sha256_bytes(fixed_bytes::<32>(
            raw.4,
            "timeline source ledger digest",
        )?),
        source_row_digest: ContentDigest::from_sha256_bytes(fixed_bytes::<32>(
            raw.5,
            "timeline source row digest",
        )?),
        aggregate_lane: AggregateLane::parse(&raw.6)?,
        latest_accept_seq: u64::try_from(raw.1).map_err(|_| {
            ProjectionError::Corrupt("timeline latest acceptance sequence is negative".to_owned())
        })?,
        aggregates,
        claims,
    }))
}

type RawAggregate = (
    String,
    Vec<u8>,
    Vec<u8>,
    i64,
    Vec<u8>,
    Option<Vec<u8>>,
    i64,
    Option<i64>,
);

fn read_persisted_aggregates(
    connection: &Connection,
    snapshot_id: &[u8; 16],
) -> ProjectionResult<Vec<SnapshotAggregateRow>> {
    let mut statement = connection.prepare(concat!(
        "SELECT aggregate_kind, aggregate_id, registered_event_id, accept_seq, scope_id, ",
        "source_digest, valid_from_unix_ms, valid_to_unix_ms ",
        "FROM timeline_snapshot_aggregate WHERE snapshot_id = ?1 ",
        "ORDER BY accept_seq, aggregate_id"
    ))?;
    let rows = statement.query_map(params![snapshot_id.as_slice()], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
        ))
    })?;
    rows.collect::<rusqlite::Result<Vec<RawAggregate>>>()?
        .into_iter()
        .map(|row| {
            Ok(SnapshotAggregateRow {
                kind: row.0,
                aggregate_id: fixed_bytes::<16>(row.1, "timeline aggregate identifier")?,
                registered_event_id: fixed_bytes::<16>(row.2, "timeline aggregate event")?,
                accept_seq: u64::try_from(row.3).map_err(|_| {
                    ProjectionError::Corrupt("timeline aggregate acceptance is negative".to_owned())
                })?,
                scope_id: id_from_bytes(row.4, "timeline aggregate scope")?,
                source_digest: row
                    .5
                    .map(|bytes| {
                        fixed_bytes::<32>(bytes, "timeline aggregate source digest")
                            .map(ContentDigest::from_sha256_bytes)
                    })
                    .transpose()?,
                valid_from: TimestampMillis::new(row.6),
                valid_to: row.7.map(TimestampMillis::new),
            })
        })
        .collect()
}

type RawClaim = (
    Vec<u8>,
    Vec<u8>,
    String,
    Vec<u8>,
    i64,
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
);

fn read_persisted_claims(
    connection: &Connection,
    snapshot_id: &[u8; 16],
) -> ProjectionResult<Vec<SnapshotClaimRow>> {
    let mut statement = connection.prepare(concat!(
        "SELECT claim_id, subject_entity_id, predicate_id, scope_id, accept_seq, ",
        "authority_class, epistemic_status, applied_policy, object_kind, ",
        "valid_from_unix_ms, valid_to_unix_ms FROM timeline_snapshot_claim ",
        "WHERE snapshot_id = ?1 ORDER BY accept_seq, claim_id"
    ))?;
    let rows = statement.query_map(params![snapshot_id.as_slice()], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
        ))
    })?;
    rows.collect::<rusqlite::Result<Vec<RawClaim>>>()?
        .into_iter()
        .map(|row| {
            Ok(SnapshotClaimRow {
                claim_id: id_from_bytes(row.0, "timeline claim identifier")?,
                subject_entity_id: id_from_bytes(row.1, "timeline claim subject")?,
                predicate_id: PredicateId::parse(row.2).map_err(ProjectionError::Domain)?,
                scope_id: id_from_bytes(row.3, "timeline claim scope")?,
                accept_seq: u64::try_from(row.4).map_err(|_| {
                    ProjectionError::Corrupt("timeline claim acceptance is negative".to_owned())
                })?,
                authority_class: row.5,
                epistemic_status: row.6,
                applied_policy: parse_authority_policy(&row.7)?,
                object_kind: row.8,
                valid_from: TimestampMillis::new(row.9),
                valid_to: row.10.map(TimestampMillis::new),
            })
        })
        .collect()
}

fn checked_i64(value: u64) -> ProjectionResult<i64> {
    i64::try_from(value).map_err(|_| ProjectionError::IntegerOverflow(value))
}

fn unix_time_millis() -> ProjectionResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProjectionError::SystemClock)?;
    i64::try_from(duration.as_millis()).map_err(|_| ProjectionError::SystemClock)
}
