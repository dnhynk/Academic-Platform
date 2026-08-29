//! Bitemporal reads over the eighteen migration 0004 aggregate closure tables.
//!
//! # Both coordinates, always
//!
//! [`AggregateTimelineRequest`] carries [`TimeCoordinates`], which holds
//! `known_at_accept_seq` and `valid_at` together. There is no single-coordinate
//! entry point and no "current" one: ADR-003 forbids an ambiguous mutable
//! current-state query, and a caller that has not decided both coordinates
//! cannot construct a request.
//!
//! `known_at_accept_seq` selects by the replica-local acceptance order of the
//! event that registered the aggregate, so a row accepted later is invisible at
//! an earlier coordinate no matter what its valid interval says. `valid_at`
//! selects by the aggregate's own half-open `[valid_from, valid_to)` interval,
//! so moving it re-reads history with whatever is known at the other
//! coordinate. The two are independent, which is the whole point: origin order,
//! local acceptance order, and valid time stay separate.
//!
//! # Absence is not emptiness
//!
//! A schema-1 profile carries no aggregate tables at all. Reading one returns
//! [`QueryError::AggregatesAbsent`] rather than an empty snapshot, because an
//! empty snapshot reads as "no aggregate was registered by this coordinate",
//! which is a different statement from "this profile cannot hold aggregates".

use rusqlite::Connection;

use academic_domain::{
    ContentDigest, DomainId, ScopeId, TimestampMillis, temporal::TimeCoordinates,
};

use crate::{
    connection::ReaderConnection,
    error::StoreError,
    queries::{QueryError, checked_i64, fixed_bytes, nonnegative_u64, positive_u64, query_collect},
    repository::id_from_blob,
};

/// One migration 0004 aggregate closure table and how to read it.
///
/// The wire discriminant is the event schema v3 arm that registers the
/// aggregate, so this registry and `V3_EVENT_KINDS` are the same eighteen
/// names. `aggregate_timeline_registry_matches_the_writer` checks that against
/// the writer's own table list and against a live migrated schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregateTable {
    /// Event schema v3 arm that registers rows in this table.
    pub kind: &'static str,
    /// SQL table migration 0004 creates for the arm.
    pub table: &'static str,
    /// Primary-key column carrying the aggregate identifier.
    pub primary_key_column: &'static str,
}

/// The eighteen aggregate closure tables, in Proto tag order.
pub const AGGREGATE_TABLES: [AggregateTable; 18] = [
    table("CURRICULUM_VERSION_PUBLISHED", "curriculum_version"),
    table("COURSE_REVISION_PUBLISHED", "course_revision"),
    table("OFFERING_OBSERVED", "offering"),
    table("ATTEMPT_RECORDED", "attempt"),
    table("REQUIREMENT_SET_PUBLISHED", "requirement_set"),
    table("AUDIT_COMPUTED", "audit"),
    table("CAPTURE_PERMISSION_RECORDED", "capture_permission"),
    table("LECTURE_SESSION_RECORDED", "lecture_session"),
    table("TRANSCRIPT_VERSION_ADDED", "transcript_version"),
    table("LECTURE_DOCUMENT_PUBLISHED", "lecture_document"),
    table("SNAPSHOT_REGISTERED", "snapshot"),
    table("FINDING_PUBLISHED", "finding"),
    table("MODEL_RUN_RECORDED", "model_run"),
    AggregateTable {
        kind: "PROPOSAL_DISPOSED",
        table: "proposal_disposition",
        primary_key_column: "proposal_id",
    },
    table("EGRESS_DECIDED", "egress_decision"),
    table("CONSENT_RECORDED", "consent"),
    table("ENTITY_IDENTITY_CHANGED", "entity_identity_change"),
    table("RETENTION_ACTION_RECORDED", "retention_action"),
];

/// Declares a table whose primary key is its own name plus `_id`.
const fn table(kind: &'static str, name: &'static str) -> AggregateTable {
    AggregateTable {
        kind,
        table: name,
        primary_key_column: "",
    }
}

/// Store-owned request for one aggregate-set reading at exact coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregateTimelineRequest {
    /// Security domain the reading is confined to.
    pub domain_id: DomainId,
    /// The two mandatory bitemporal coordinates.
    pub coordinates: TimeCoordinates,
}

/// One aggregate registration visible at the requested coordinates.
///
/// These are exactly migration 0004's registration frame columns. Typed
/// aggregate attributes are deliberately absent: each aggregate owner adds its
/// own columns in a later migration, and none of them are readable here yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateTimelineRow {
    /// Event schema v3 arm that registered the aggregate.
    pub kind: &'static str,
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
    /// Exclusive end of that interval, absent when it is still open.
    pub valid_to: Option<TimestampMillis>,
}

/// One reading of the whole Phase 2 aggregate set at exact coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateTimelineSnapshot {
    /// Coordinates this reading was taken at.
    pub coordinates: TimeCoordinates,
    /// Highest acceptance sequence the canonical store holds.
    pub latest_accept_seq: u64,
    /// Visible rows, ordered by acceptance sequence then aggregate identifier.
    pub rows: Vec<AggregateTimelineRow>,
    /// Digest over the visible rows, in that order.
    ///
    /// Two readings that agree on this digest read the same canonical bytes, so
    /// a recomputation whose result differs while this digest is equal differs
    /// because the projector changed and for no other reason.
    pub source_row_digest: ContentDigest,
}

impl AggregateTimelineSnapshot {
    /// Returns the visible rows for one aggregate arm.
    #[must_use]
    pub fn rows_of(&self, kind: &str) -> Vec<&AggregateTimelineRow> {
        self.rows.iter().filter(|row| row.kind == kind).collect()
    }
}

/// Reads the whole Phase 2 aggregate set at one exact coordinate pair.
///
/// # Errors
///
/// Returns [`QueryError::AggregatesAbsent`] when the profile carries no
/// aggregate tables, [`QueryError::KnownAtBeyondHead`] when the requested
/// acceptance coordinate is ahead of the canonical head, and the usual
/// corruption refusals for malformed normalized rows.
pub fn aggregate_timeline_snapshot(
    reader: &mut ReaderConnection,
    request: &AggregateTimelineRequest,
) -> Result<AggregateTimelineSnapshot, QueryError> {
    let transaction = reader.begin_deferred()?;
    let snapshot = aggregate_timeline_from_connection(&transaction, request)?;
    transaction.commit().map_err(StoreError::from)?;
    Ok(snapshot)
}

/// The one reading implementation, held inside a single canonical transaction.
pub(crate) fn aggregate_timeline_from_connection(
    connection: &Connection,
    request: &AggregateTimelineRequest,
) -> Result<AggregateTimelineSnapshot, QueryError> {
    let missing = missing_aggregate_tables(connection)?;
    if !missing.is_empty() {
        return Err(QueryError::AggregatesAbsent {
            missing: missing.len(),
            first: missing[0],
        });
    }
    let latest_accept_seq = nonnegative_u64(
        connection
            .query_row(
                "SELECT coalesce(max(accept_seq), 0) FROM ledger_event",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::from)?,
        "latest acceptance sequence",
    )?;
    let known_at = request.coordinates.known_at_accept_seq;
    if known_at > latest_accept_seq {
        return Err(QueryError::KnownAtBeyondHead {
            requested: known_at,
            latest: latest_accept_seq,
        });
    }

    let mut rows = Vec::new();
    for descriptor in AGGREGATE_TABLES {
        rows.extend(read_aggregate_table(connection, request, descriptor)?);
    }
    rows.sort_by(|left, right| {
        (left.accept_seq, left.aggregate_id).cmp(&(right.accept_seq, right.aggregate_id))
    });

    Ok(AggregateTimelineSnapshot {
        coordinates: request.coordinates,
        latest_accept_seq,
        source_row_digest: row_digest(request, &rows)?,
        rows,
    })
}

/// Returns the aggregate tables this database does not carry, in registry order.
fn missing_aggregate_tables(connection: &Connection) -> Result<Vec<&'static str>, QueryError> {
    let present: Vec<String> = query_collect(
        connection,
        "SELECT name FROM sqlite_schema WHERE type = 'table'",
        [],
        |row| row.get::<_, String>(0),
    )?;
    Ok(AGGREGATE_TABLES
        .iter()
        .filter(|descriptor| !present.iter().any(|name| name.as_str() == descriptor.table))
        .map(|descriptor| descriptor.table)
        .collect())
}

type RawAggregateRow = (
    Vec<u8>,
    Vec<u8>,
    i64,
    Vec<u8>,
    Option<Vec<u8>>,
    i64,
    Option<i64>,
);

/// Reads one aggregate table at both coordinates.
///
/// `e.accept_seq <= ?2` is the as-known-at half and the valid-interval
/// comparison is the valid-at half. They are separate predicates on separate
/// columns, so neither can stand in for the other.
fn read_aggregate_table(
    connection: &Connection,
    request: &AggregateTimelineRequest,
    descriptor: AggregateTable,
) -> Result<Vec<AggregateTimelineRow>, QueryError> {
    let primary_key = descriptor.resolved_primary_key_column();
    let sql = format!(
        "SELECT a.{primary_key}, a.registered_event_id, e.accept_seq, a.scope_id, \
         a.source_digest, a.valid_from, a.valid_to \
         FROM {table} a JOIN ledger_event e ON e.event_id = a.registered_event_id \
         WHERE a.domain_id = ?1 AND e.accept_seq <= ?2 \
         AND a.valid_from <= ?3 AND (a.valid_to IS NULL OR a.valid_to > ?3) \
         ORDER BY e.accept_seq, a.{primary_key}",
        table = descriptor.table,
    );
    let raw: Vec<RawAggregateRow> = query_collect(
        connection,
        &sql,
        rusqlite::params![
            request.domain_id.as_bytes().as_slice(),
            checked_i64(request.coordinates.known_at_accept_seq)?,
            request.coordinates.valid_at.value(),
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
            ))
        },
    )?;
    raw.into_iter()
        .map(|row| {
            Ok(AggregateTimelineRow {
                kind: descriptor.kind,
                aggregate_id: fixed_bytes::<16>(row.0, "aggregate identifier")?,
                registered_event_id: fixed_bytes::<16>(row.1, "aggregate registration event")?,
                accept_seq: positive_u64(row.2, "aggregate acceptance sequence")?,
                scope_id: id_from_blob::<ScopeId>(row.3)?,
                source_digest: row
                    .4
                    .map(|bytes| {
                        fixed_bytes::<32>(bytes, "aggregate source digest")
                            .map(ContentDigest::from_sha256_bytes)
                    })
                    .transpose()?,
                valid_from: TimestampMillis::new(row.5),
                valid_to: row.6.map(TimestampMillis::new),
            })
        })
        .collect()
}

impl AggregateTable {
    /// Returns the primary-key column, defaulting to the table name plus `_id`.
    #[must_use]
    pub fn resolved_primary_key_column(&self) -> String {
        if self.primary_key_column.is_empty() {
            format!("{}_id", self.table)
        } else {
            self.primary_key_column.to_owned()
        }
    }
}

/// Commits to the exact ordered rows a reading saw, and to its coordinates.
fn row_digest(
    request: &AggregateTimelineRequest,
    rows: &[AggregateTimelineRow],
) -> Result<ContentDigest, QueryError> {
    let row_count = u64::try_from(rows.len())
        .map_err(|_| QueryError::Corrupt("aggregate timeline row count overflow"))?;
    let mut material = Vec::with_capacity(64_usize.saturating_add(rows.len().saturating_mul(120)));
    material.extend_from_slice(b"ACADEMIC_AGGREGATE_TIMELINE_V1\0");
    material.extend_from_slice(request.domain_id.as_bytes());
    material.extend_from_slice(&request.coordinates.known_at_accept_seq.to_be_bytes());
    material.extend_from_slice(&request.coordinates.valid_at.value().to_be_bytes());
    material.extend_from_slice(&row_count.to_be_bytes());
    for row in rows {
        let kind_length = u32::try_from(row.kind.len())
            .map_err(|_| QueryError::Corrupt("aggregate kind length overflow"))?;
        material.extend_from_slice(&kind_length.to_be_bytes());
        material.extend_from_slice(row.kind.as_bytes());
        material.extend_from_slice(&row.aggregate_id);
        material.extend_from_slice(&row.registered_event_id);
        material.extend_from_slice(&row.accept_seq.to_be_bytes());
        material.extend_from_slice(row.scope_id.as_bytes());
        match row.source_digest.as_ref() {
            Some(digest) => {
                material.push(1);
                material.extend_from_slice(digest.as_bytes());
            }
            None => material.push(0),
        }
        material.extend_from_slice(&row.valid_from.value().to_be_bytes());
        match row.valid_to {
            Some(instant) => {
                material.push(1);
                material.extend_from_slice(&instant.value().to_be_bytes());
            }
            None => material.push(0),
        }
    }
    Ok(ContentDigest::sha256(&material))
}

/// Origin-bearing canonical acceptances inside a half-open known-time interval.
///
/// The three sets are disjoint by construction: one event carries one payload
/// arm, so an acceptance cannot be both an identity change and a claim
/// relation. That is what lets a caller split `(after, through]` into steps that
/// each carry exactly one change origin instead of guessing a precedence order
/// when several kinds of change land in the same interval.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OriginMarks {
    /// Acceptances that registered an entity identity change (merge or split).
    pub identity_changes: Vec<u64>,
    /// Acceptances where an official source superseded an earlier official claim.
    pub official_corrections: Vec<u64>,
    /// Every other acceptance in the interval.
    pub other_acceptances: Vec<u64>,
    /// Whether the profile carries the entity identity change table at all.
    ///
    /// A schema-1 profile does not. An empty `identity_changes` on such a
    /// profile means "this profile cannot record one", not "none happened".
    pub identity_lane_present: bool,
}

impl OriginMarks {
    /// Returns every mark in acceptance order, paired with its origin.
    ///
    /// The pairs are what a reader turns into origin-pure steps.
    #[must_use]
    pub fn ordered(&self) -> Vec<(u64, &'static str)> {
        let mut marks: Vec<(u64, &'static str)> = self
            .identity_changes
            .iter()
            .map(|seq| (*seq, "ONTOLOGY_CHANGE"))
            .chain(
                self.official_corrections
                    .iter()
                    .map(|seq| (*seq, "OFFICIAL_SOURCE_CORRECTION")),
            )
            .chain(
                self.other_acceptances
                    .iter()
                    .map(|seq| (*seq, "EVIDENCE_CHANGE")),
            )
            .collect();
        marks.sort_by_key(|(seq, _)| *seq);
        marks
    }
}

/// Collects the origin-bearing acceptances in `(after_accept_seq, through_accept_seq]`.
///
/// # Errors
///
/// Propagates canonical read failures and refuses malformed acceptance rows.
pub fn origin_marks(
    reader: &mut ReaderConnection,
    domain_id: DomainId,
    after_accept_seq: u64,
    through_accept_seq: u64,
) -> Result<OriginMarks, QueryError> {
    let transaction = reader.begin_deferred()?;
    let marks = origin_marks_from_connection(
        &transaction,
        domain_id,
        after_accept_seq,
        through_accept_seq,
    )?;
    transaction.commit().map_err(StoreError::from)?;
    Ok(marks)
}

pub(crate) fn origin_marks_from_connection(
    connection: &Connection,
    domain_id: DomainId,
    after_accept_seq: u64,
    through_accept_seq: u64,
) -> Result<OriginMarks, QueryError> {
    let parameters = rusqlite::params![
        domain_id.as_bytes().as_slice(),
        checked_i64(after_accept_seq)?,
        checked_i64(through_accept_seq)?,
    ];
    let identity_lane_present = table_exists(connection, "entity_identity_change")?;
    let identity_changes = if identity_lane_present {
        accept_sequences(
            connection,
            concat!(
                "SELECT e.accept_seq FROM entity_identity_change a ",
                "JOIN ledger_event e ON e.event_id = a.registered_event_id ",
                "WHERE a.domain_id = ?1 AND e.accept_seq > ?2 AND e.accept_seq <= ?3 ",
                "ORDER BY e.accept_seq"
            ),
            parameters,
        )?
    } else {
        Vec::new()
    };
    let official_corrections = accept_sequences(
        connection,
        concat!(
            "SELECT e.accept_seq FROM claim_relation r ",
            "JOIN ledger_event e ON e.event_id = r.relation_event_id ",
            "JOIN claim s ON s.claim_id = r.source_claim_id ",
            "JOIN claim t ON t.claim_id = r.target_claim_id ",
            "WHERE e.domain_id = ?1 AND e.accept_seq > ?2 AND e.accept_seq <= ?3 ",
            "AND r.relation_kind = 'SUPERSEDES' AND r.actor_kind = 'IMPORTER' ",
            "AND s.epistemic_status = 'OFFICIAL_CONFIRMED' ",
            "AND t.epistemic_status = 'OFFICIAL_CONFIRMED' ",
            "ORDER BY e.accept_seq"
        ),
        parameters,
    )?;
    let all = accept_sequences(
        connection,
        concat!(
            "SELECT accept_seq FROM ledger_event ",
            "WHERE domain_id = ?1 AND accept_seq > ?2 AND accept_seq <= ?3 ",
            "ORDER BY accept_seq"
        ),
        parameters,
    )?;
    let other_acceptances = all
        .into_iter()
        .filter(|seq| !identity_changes.contains(seq) && !official_corrections.contains(seq))
        .collect();
    Ok(OriginMarks {
        identity_changes,
        official_corrections,
        other_acceptances,
        identity_lane_present,
    })
}

fn accept_sequences(
    connection: &Connection,
    sql: &str,
    parameters: &[&dyn rusqlite::ToSql],
) -> Result<Vec<u64>, QueryError> {
    query_collect(connection, sql, parameters, |row| row.get::<_, i64>(0))?
        .into_iter()
        .map(|value| positive_u64(value, "origin mark acceptance sequence"))
        .collect()
}

fn table_exists(connection: &Connection, name: &str) -> Result<bool, QueryError> {
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get(0),
        )
        .map_err(StoreError::from)?;
    Ok(count > 0)
}
