//! Normalized append-only repository operations used by the S2 transaction.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    str::FromStr,
};

use academic_contracts::{
    ContractError, VerifiedBatch, decode_canonical_evidence_locator, encode_canonical_actor,
    encode_canonical_claim_object, encode_canonical_event_payload, encode_canonical_evidence_ids,
    encode_canonical_evidence_locator,
};
use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, AuthorityClass, BatchId, Claim, ClaimId, ClaimObject,
    ClaimRelationKind, Confidentiality, ContentDigest, Decimal, DecisionAction, DeviceId,
    DomainError, DomainId, EntityId, EpistemicStatus, Event, EventPayload, EvidenceId,
    EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength, FreshnessBand, MasteryLevel,
    MediaType, PredicateId, PredictionMetadata, PredictionObservationWindow, RetentionClass,
    ScopeDescriptor, ScopeId, TimestampMillis, UnsignedBatch, UserDecision, ValidInterval,
};
use academic_ledger::{LedgerError, ResolverActorKind, relation_effect_is_authorized_for_kind};
use academic_vault::SealedObjectCapability;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::connection::WriterConnection;

/// Current singleton counters read under `BEGIN IMMEDIATE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReplicaState {
    pub next_accept_seq: u64,
    pub profile_revision: u64,
}

/// Existing immutable batch material needed for batch-id replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredBatch {
    pub signed_envelope: Vec<u8>,
    pub envelope_hash: ContentDigest,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
    pub committed_revision: u64,
}

/// Existing per-device chain head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StoredDeviceHead {
    pub next_origin_seq: u64,
    pub envelope_hash: ContentDigest,
}

/// SQL/normalization failure before the transaction can commit.
#[derive(Debug)]
pub enum RepositoryError {
    Sqlite(rusqlite::Error),
    Domain(DomainError),
    Contract(ContractError),
    Ledger(LedgerError),
    MissingSealedReceipt(ArtifactId),
    MismatchedSealedReceipt(ArtifactId),
    Corrupt(&'static str),
    IntegerOverflow(u64),
    UnstorableEventKind(&'static str),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite repository error: {error}"),
            Self::Domain(error) => write!(formatter, "stored domain value is invalid: {error}"),
            Self::Contract(error) => write!(formatter, "canonical payload error: {error}"),
            Self::Ledger(error) => write!(formatter, "ledger closure rejected: {error}"),
            Self::MissingSealedReceipt(artifact_id) => {
                write!(
                    formatter,
                    "artifact {artifact_id} has no retained sealed receipt"
                )
            }
            Self::MismatchedSealedReceipt(artifact_id) => write!(
                formatter,
                "artifact {artifact_id} has a mismatched retained sealed receipt"
            ),
            Self::Corrupt(reason) => write!(formatter, "normalized store is corrupt: {reason}"),
            Self::UnstorableEventKind(kind) => write!(
                formatter,
                "event kind {kind} has no canonical table in this store schema"
            ),
            Self::IntegerOverflow(value) => {
                write!(
                    formatter,
                    "repository value {value} exceeds signed 64-bit storage"
                )
            }
        }
    }
}

impl Error for RepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::Ledger(error) => Some(error),
            Self::MissingSealedReceipt(_)
            | Self::MismatchedSealedReceipt(_)
            | Self::Corrupt(_)
            | Self::IntegerOverflow(_)
            | Self::UnstorableEventKind(_) => None,
        }
    }
}

impl From<rusqlite::Error> for RepositoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<DomainError> for RepositoryError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<ContractError> for RepositoryError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<LedgerError> for RepositoryError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

pub(crate) fn read_replica_state(
    transaction: &Transaction<'_>,
) -> Result<ReplicaState, RepositoryError> {
    let (next_accept_seq, profile_revision) = transaction.query_row(
        "SELECT next_accept_seq, profile_revision FROM replica_state WHERE singleton = 1",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(ReplicaState {
        next_accept_seq: nonnegative_u64(next_accept_seq, "replica next_accept_seq")?,
        profile_revision: nonnegative_u64(profile_revision, "profile revision")?,
    })
}

pub(crate) fn find_batch(
    transaction: &Transaction<'_>,
    batch_id: BatchId,
) -> Result<Option<StoredBatch>, RepositoryError> {
    let row = transaction
        .query_row(
            concat!(
                "SELECT b.signed_envelope, b.envelope_hash, b.accept_seq_start, b.accept_seq_end, ",
                "o.canonical_revision FROM ledger_batch b ",
                "JOIN projection_outbox o ON o.accepted_batch_id = b.batch_id ",
                "WHERE b.batch_id = ?1"
            ),
            [batch_id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(signed_envelope, envelope_hash, accept_seq_start, accept_seq_end, revision)| {
            Ok(StoredBatch {
                signed_envelope,
                envelope_hash: digest_from_blob(envelope_hash)?,
                accept_seq_start: positive_u64(accept_seq_start, "batch accept_seq_start")?,
                accept_seq_end: positive_u64(accept_seq_end, "batch accept_seq_end")?,
                committed_revision: positive_u64(revision, "batch canonical revision")?,
            })
        },
    )
    .transpose()
}

pub(crate) fn read_device_head(
    transaction: &Transaction<'_>,
    device_id: DeviceId,
) -> Result<Option<StoredDeviceHead>, RepositoryError> {
    let row = transaction
        .query_row(
            "SELECT next_origin_seq, head_envelope_hash FROM device_head WHERE device_id = ?1",
            [device_id.as_bytes().as_slice()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()?;
    row.map(|(next_origin_seq, envelope_hash)| {
        Ok(StoredDeviceHead {
            next_origin_seq: positive_u64(next_origin_seq, "device next_origin_seq")?,
            envelope_hash: digest_from_blob(envelope_hash)?,
        })
    })
    .transpose()
}

pub(crate) fn insert_batch(
    transaction: &Transaction<'_>,
    verified: &VerifiedBatch,
    accept_seq_start: u64,
    accept_seq_end: u64,
    accepted_at: TimestampMillis,
) -> Result<(), RepositoryError> {
    let batch = verified.batch();
    transaction.execute(
        concat!(
            "INSERT INTO ledger_batch (batch_id, signed_envelope, envelope_hash, ",
            "deterministic_payload, deterministic_payload_hash, signing_public_key, signature, ",
            "device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
            "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, accepted_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"
        ),
        params![
            batch.batch_id.as_bytes().as_slice(),
            verified.source_envelope(),
            verified.envelope_hash().as_bytes().as_slice(),
            verified.source_payload(),
            verified.payload_hash().as_bytes().as_slice(),
            verified.public_key().as_bytes().as_slice(),
            verified.signature_bytes(),
            batch.device_id.as_bytes().as_slice(),
            checked_i64(batch.origin_seq_start)?,
            checked_i64(batch.origin_seq_end)?,
            batch
                .previous_batch_hash
                .as_ref()
                .map(|digest| digest.as_bytes().as_slice()),
            batch.origin_created_at.value(),
            i64::from(verified.source_schema_version()),
            checked_i64(accept_seq_start)?,
            checked_i64(accept_seq_end)?,
            accepted_at.value(),
        ],
    )?;
    Ok(())
}

pub(crate) fn update_heads(
    transaction: &Transaction<'_>,
    batch: &UnsignedBatch,
    envelope_hash: ContentDigest,
    next_accept_seq: u64,
    committed_revision: u64,
    updated_at: TimestampMillis,
    had_device_head: bool,
) -> Result<(), RepositoryError> {
    let next_origin_seq = batch
        .origin_seq_end
        .checked_add(1)
        .ok_or(RepositoryError::IntegerOverflow(batch.origin_seq_end))?;
    let changed = if had_device_head {
        transaction.execute(
            concat!(
                "UPDATE device_head SET next_origin_seq = ?2, head_batch_id = ?3, ",
                "head_envelope_hash = ?4, updated_at = ?5 WHERE device_id = ?1"
            ),
            params![
                batch.device_id.as_bytes().as_slice(),
                checked_i64(next_origin_seq)?,
                batch.batch_id.as_bytes().as_slice(),
                envelope_hash.as_bytes().as_slice(),
                updated_at.value(),
            ],
        )?
    } else {
        transaction.execute(
            concat!(
                "INSERT INTO device_head (device_id, next_origin_seq, head_batch_id, ",
                "head_envelope_hash, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)"
            ),
            params![
                batch.device_id.as_bytes().as_slice(),
                checked_i64(next_origin_seq)?,
                batch.batch_id.as_bytes().as_slice(),
                envelope_hash.as_bytes().as_slice(),
                updated_at.value(),
            ],
        )?
    };
    if changed != 1 {
        return Err(RepositoryError::Corrupt(
            "device head update did not affect exactly one row",
        ));
    }
    let changed = transaction.execute(
        "UPDATE replica_state SET next_accept_seq = ?1, profile_revision = ?2 WHERE singleton = 1",
        params![
            checked_i64(next_accept_seq)?,
            checked_i64(committed_revision)?
        ],
    )?;
    if changed != 1 {
        return Err(RepositoryError::Corrupt(
            "replica state update did not affect exactly one row",
        ));
    }
    Ok(())
}

/// Loads an immutable descriptor during the pre-transaction sealing closure.
pub(crate) fn preflight_artifact_descriptor(
    writer: &WriterConnection,
    artifact_id: ArtifactId,
) -> Result<Option<ArtifactDescriptor>, RepositoryError> {
    writer.with_preflight_reader(|connection| load_artifact(connection, artifact_id))
}

/// Resolves an already-normalized evidence row to its immutable artifact.
pub(crate) fn preflight_evidence_artifact(
    writer: &WriterConnection,
    evidence_id: EvidenceId,
) -> Result<Option<ArtifactId>, RepositoryError> {
    writer.with_preflight_reader(|connection| {
        connection
            .query_row(
                "SELECT artifact_id FROM evidence_item WHERE evidence_id = ?1",
                [evidence_id.as_bytes().as_slice()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .map(id_from_blob)
            .transpose()
    })
}

/// Resolves every evidence edge of an already-normalized claim.
pub(crate) fn preflight_claim_evidence(
    writer: &WriterConnection,
    claim_id: ClaimId,
) -> Result<Option<Vec<EvidenceId>>, RepositoryError> {
    writer.with_preflight_reader(|connection| {
        let exists = connection
            .query_row(
                "SELECT 1 FROM claim WHERE claim_id = ?1",
                [claim_id.as_bytes().as_slice()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Ok(None);
        }
        let mut statement = connection.prepare(
            "SELECT evidence_id FROM claim_evidence WHERE claim_id = ?1 ORDER BY evidence_ordinal",
        )?;
        let rows = statement.query_map([claim_id.as_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?;
        let mut evidence_ids = Vec::new();
        for row in rows {
            evidence_ids.push(id_from_blob(row?)?);
        }
        Ok(Some(evidence_ids))
    })
}

#[derive(Debug, Clone)]
struct AcceptedEvidence {
    artifact_id: ArtifactId,
    domain_id: DomainId,
}

#[derive(Debug, Clone)]
struct AcceptedClaim {
    claim: Claim,
    domain_id: DomainId,
    artifact_ids: BTreeSet<ArtifactId>,
}

/// Stages closure knowledge while normalized rows are appended in event order.
pub(crate) struct ClosureWriter<'transaction, 'connection, 'receipts> {
    transaction: &'transaction Transaction<'connection>,
    batch: &'transaction UnsignedBatch,
    sealed_receipts: &'receipts BTreeMap<ArtifactId, SealedObjectCapability>,
    scopes: BTreeMap<ScopeId, ScopeDescriptor>,
    artifacts: BTreeMap<ArtifactId, ArtifactDescriptor>,
    evidence: BTreeMap<EvidenceId, AcceptedEvidence>,
    claims: BTreeMap<ClaimId, AcceptedClaim>,
    registrations: BTreeSet<(&'static str, [u8; 16])>,
}

/// One aggregate closure row in the exact shape migration 0004 gives every event
/// schema v3 registration arm.
///
/// The fields are the whole of the v3 registration frame. Typed aggregate
/// attributes are deliberately absent here and in the schema; each aggregate
/// owner adds its own columns later, and none of them may be smuggled into
/// `claim.object_text`.
struct AggregateClosureRow<'payload> {
    table: &'static str,
    primary_key_column: &'static str,
    aggregate_id: &'payload [u8; 16],
    /// The same identifier in its typed spelling, used only to name a duplicate.
    aggregate_id_display: &'payload dyn fmt::Display,
    parent: Option<(&'static str, &'payload [u8; 16])>,
    domain_id: &'payload DomainId,
    scope_id: &'payload ScopeId,
    source_digest: Option<&'payload ContentDigest>,
    valid_time: ValidInterval,
}

/// Maps an event schema v3 registration arm onto its migration 0004 table.
///
/// Returns `None` for the six v1/v2 arms, which have their own typed writers.
/// The match is exhaustive over `EventPayload`, so a nineteenth arm cannot be
/// added without deciding where its closure row lives.
fn aggregate_closure_row(payload: &EventPayload) -> Option<AggregateClosureRow<'_>> {
    macro_rules! row {
        ($record:expr, $table:literal, $key:literal) => {
            AggregateClosureRow {
                table: $table,
                primary_key_column: $key,
                aggregate_id: $record.id.as_bytes(),
                aggregate_id_display: &$record.id,
                parent: None,
                domain_id: &$record.domain_id,
                scope_id: &$record.scope_id,
                source_digest: $record.source_digest.as_ref(),
                valid_time: $record.valid_time,
            }
        };
        ($record:expr, $table:literal, $key:literal, $parent_column:literal, $parent:ident) => {
            AggregateClosureRow {
                parent: Some(($parent_column, $record.$parent.as_bytes())),
                ..row!($record, $table, $key)
            }
        };
    }

    Some(match payload {
        EventPayload::ScopeRegistered(_)
        | EventPayload::ArtifactRegistered(_)
        | EventPayload::EvidenceRegistered(_)
        | EventPayload::ClaimAsserted(_)
        | EventPayload::ClaimRelated(_)
        | EventPayload::DecisionRecorded(_) => return None,
        EventPayload::CurriculumVersionPublished(record) => {
            row!(record, "curriculum_version", "curriculum_version_id")
        }
        EventPayload::CourseRevisionPublished(record) => row!(
            record,
            "course_revision",
            "course_revision_id",
            "curriculum_version_id",
            curriculum_version_id
        ),
        EventPayload::OfferingObserved(record) => row!(
            record,
            "offering",
            "offering_id",
            "course_revision_id",
            course_revision_id
        ),
        EventPayload::AttemptRecorded(record) => {
            row!(record, "attempt", "attempt_id", "offering_id", offering_id)
        }
        EventPayload::RequirementSetPublished(record) => row!(
            record,
            "requirement_set",
            "requirement_set_id",
            "curriculum_version_id",
            curriculum_version_id
        ),
        EventPayload::AuditComputed(record) => row!(
            record,
            "audit",
            "audit_id",
            "requirement_set_id",
            requirement_set_id
        ),
        EventPayload::CapturePermissionRecorded(record) => row!(
            record,
            "capture_permission",
            "capture_permission_id",
            "offering_id",
            offering_id
        ),
        EventPayload::LectureSessionRecorded(record) => row!(
            record,
            "lecture_session",
            "lecture_session_id",
            "offering_id",
            offering_id
        ),
        EventPayload::TranscriptVersionAdded(record) => row!(
            record,
            "transcript_version",
            "transcript_version_id",
            "lecture_session_id",
            lecture_session_id
        ),
        EventPayload::LectureDocumentPublished(record) => row!(
            record,
            "lecture_document",
            "lecture_document_id",
            "lecture_session_id",
            lecture_session_id
        ),
        EventPayload::SnapshotRegistered(record) => row!(
            record,
            "snapshot",
            "snapshot_id",
            "repository_id",
            repository_id
        ),
        EventPayload::FindingPublished(record) => {
            row!(record, "finding", "finding_id", "snapshot_id", snapshot_id)
        }
        EventPayload::ModelRunRecorded(record) => row!(record, "model_run", "model_run_id"),
        EventPayload::ProposalDisposed(record) => row!(
            record,
            "proposal_disposition",
            "proposal_id",
            "model_run_id",
            model_run_id
        ),
        EventPayload::EgressDecided(record) => {
            row!(record, "egress_decision", "egress_decision_id")
        }
        EventPayload::ConsentRecorded(record) => row!(record, "consent", "consent_id"),
        EventPayload::EntityIdentityChanged(record) => row!(
            record,
            "entity_identity_change",
            "entity_identity_change_id",
            "entity_id",
            entity_id
        ),
        EventPayload::RetentionActionRecorded(record) => {
            row!(record, "retention_action", "retention_action_id")
        }
    })
}

impl<'transaction, 'connection, 'receipts> ClosureWriter<'transaction, 'connection, 'receipts> {
    pub(crate) fn new(
        transaction: &'transaction Transaction<'connection>,
        batch: &'transaction UnsignedBatch,
        sealed_receipts: &'receipts BTreeMap<ArtifactId, SealedObjectCapability>,
    ) -> Self {
        Self {
            transaction,
            batch,
            sealed_receipts,
            scopes: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            evidence: BTreeMap::new(),
            claims: BTreeMap::new(),
            registrations: BTreeSet::new(),
        }
    }

    pub(crate) fn append_event(
        &mut self,
        event: &Event,
        accept_seq: u64,
    ) -> Result<(), RepositoryError> {
        event.validate()?;
        if row_exists(
            self.transaction,
            "ledger_event",
            "event_id",
            event.id.as_bytes(),
        )? {
            return Err(LedgerError::DuplicateId {
                kind: "event",
                id: event.id.to_string(),
            }
            .into());
        }
        let canonical_payload = encode_canonical_event_payload(event)?;
        let actor_canonical = encode_canonical_actor(&event.actor)?;
        self.transaction.execute(
            concat!(
                "INSERT INTO ledger_event (event_id, batch_id, origin_seq, origin_observed_at, ",
                "accept_seq, actor_kind, actor_canonical, domain_id, event_kind, ",
                "canonical_payload, payload_hash) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            ),
            params![
                event.id.as_bytes().as_slice(),
                self.batch.batch_id.as_bytes().as_slice(),
                checked_i64(event.origin_seq)?,
                event.origin_observed_at.value(),
                checked_i64(accept_seq)?,
                actor_kind(&event.actor),
                actor_canonical,
                event.domain_id.as_bytes().as_slice(),
                event_kind(&event.payload),
                canonical_payload.as_slice(),
                ContentDigest::sha256(&canonical_payload)
                    .as_bytes()
                    .as_slice(),
            ],
        )?;

        match &event.payload {
            EventPayload::ScopeRegistered(scope) => self.append_scope(event, scope),
            EventPayload::ArtifactRegistered(descriptor) => self.append_artifact(event, descriptor),
            EventPayload::EvidenceRegistered(item) => self.append_evidence(event, item),
            EventPayload::ClaimAsserted(claim) => self.append_claim(event, claim, accept_seq),
            EventPayload::ClaimRelated(relation) => {
                self.append_relation(event, relation, accept_seq)
            }
            EventPayload::DecisionRecorded(decision) => {
                self.append_decision(event, decision, accept_seq)
            }
            payload => {
                let row = aggregate_closure_row(payload)
                    .ok_or(RepositoryError::UnstorableEventKind(payload.kind()))?;
                self.append_registration(event, &row)
            }
        }
    }

    /// Writes one migration 0004 closure row inside the acceptance transaction.
    ///
    /// The row lands in the same transaction as its own `ledger_event` insert, so
    /// an aggregate can never outlive or precede the event that registered it.
    /// `registered_event_id` is UNIQUE within each closure table, the same
    /// per-table shape `scope.created_event_id` and `claim.assertion_event_id`
    /// carry. Across tables the binding is structural instead: an event holds one
    /// payload arm and [`aggregate_closure_row`] maps each arm to one table.
    fn append_registration(
        &mut self,
        event: &Event,
        row: &AggregateClosureRow<'_>,
    ) -> Result<(), RepositoryError> {
        if !self.registrations.insert((row.table, *row.aggregate_id))
            || row_exists(
                self.transaction,
                row.table,
                row.primary_key_column,
                row.aggregate_id,
            )?
        {
            return Err(LedgerError::DuplicateId {
                kind: row.table,
                id: row.aggregate_id_display.to_string(),
            }
            .into());
        }
        // Every identifier interpolated below is a `&'static str` chosen by
        // `aggregate_closure_row`, never caller input; every value is bound.
        let valid_to = row.valid_time.to().map(TimestampMillis::value);
        match row.parent {
            Some((parent_column, parent_id)) => self.transaction.execute(
                &format!(
                    "INSERT INTO {} ({}, registered_event_id, {parent_column}, domain_id, \
                     scope_id, source_digest, valid_from, valid_to) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    row.table, row.primary_key_column
                ),
                params![
                    row.aggregate_id.as_slice(),
                    event.id.as_bytes().as_slice(),
                    parent_id.as_slice(),
                    row.domain_id.as_bytes().as_slice(),
                    row.scope_id.as_bytes().as_slice(),
                    row.source_digest.map(|digest| digest.as_bytes().as_slice()),
                    row.valid_time.from().value(),
                    valid_to,
                ],
            )?,
            None => self.transaction.execute(
                &format!(
                    "INSERT INTO {} ({}, registered_event_id, domain_id, scope_id, \
                     source_digest, valid_from, valid_to) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    row.table, row.primary_key_column
                ),
                params![
                    row.aggregate_id.as_slice(),
                    event.id.as_bytes().as_slice(),
                    row.domain_id.as_bytes().as_slice(),
                    row.scope_id.as_bytes().as_slice(),
                    row.source_digest.map(|digest| digest.as_bytes().as_slice()),
                    row.valid_time.from().value(),
                    valid_to,
                ],
            )?,
        };
        Ok(())
    }

    fn append_scope(
        &mut self,
        event: &Event,
        scope: &ScopeDescriptor,
    ) -> Result<(), RepositoryError> {
        if self.scopes.contains_key(&scope.id)
            || row_exists(self.transaction, "scope", "scope_id", scope.id.as_bytes())?
        {
            return Err(LedgerError::DuplicateId {
                kind: "scope",
                id: scope.id.to_string(),
            }
            .into());
        }
        if scope.domain_id != event.domain_id {
            return Err(LedgerError::CrossDomain("scope registration").into());
        }
        self.transaction.execute(
            "INSERT INTO scope (scope_id, created_event_id, domain_id, label) VALUES (?1, ?2, ?3, ?4)",
            params![
                scope.id.as_bytes().as_slice(),
                event.id.as_bytes().as_slice(),
                scope.domain_id.as_bytes().as_slice(),
                scope.label,
            ],
        )?;
        self.scopes.insert(scope.id, scope.clone());
        Ok(())
    }

    fn append_artifact(
        &mut self,
        event: &Event,
        descriptor: &ArtifactDescriptor,
    ) -> Result<(), RepositoryError> {
        self.require_sealed_descriptor(descriptor)?;
        if self.artifacts.contains_key(&descriptor.id)
            || row_exists(
                self.transaction,
                "artifact_descriptor",
                "artifact_id",
                descriptor.id.as_bytes(),
            )?
        {
            return Err(LedgerError::DuplicateId {
                kind: "artifact",
                id: descriptor.id.to_string(),
            }
            .into());
        }
        self.transaction.execute(
            concat!(
                "INSERT INTO artifact_descriptor (artifact_id, registered_event_id, content_digest, ",
                "media_type, byte_length, domain_id, confidentiality, retention_class, ",
                "permission_lineage_id, format_version, vault_locator) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            ),
            params![
                descriptor.id.as_bytes().as_slice(),
                event.id.as_bytes().as_slice(),
                descriptor.content_digest.as_bytes().as_slice(),
                descriptor.media_type.as_str(),
                checked_i64(descriptor.byte_length)?,
                descriptor.domain_id.as_bytes().as_slice(),
                confidentiality(descriptor.confidentiality),
                retention_class(descriptor.retention_class),
                descriptor.permission_lineage_id.as_bytes().as_slice(),
                i64::from(descriptor.format_version),
                descriptor.vault_locator.as_bytes().as_slice(),
            ],
        )?;
        for (index, representation) in descriptor.evidence_representations.iter().enumerate() {
            let (kind, payload) = encode_locator(&representation.locator)?;
            self.transaction.execute(
                concat!(
                    "INSERT INTO artifact_representation (artifact_id, representation_index, ",
                    "locator_kind, locator_payload, content_digest, byte_length) ",
                    "VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ),
                params![
                    descriptor.id.as_bytes().as_slice(),
                    checked_i64(u64::try_from(index).map_err(|_| {
                        RepositoryError::Corrupt("representation index does not fit u64")
                    })?)?,
                    kind,
                    payload,
                    representation.content_digest.as_bytes().as_slice(),
                    checked_i64(representation.byte_length)?,
                ],
            )?;
        }
        self.artifacts.insert(descriptor.id, descriptor.clone());
        Ok(())
    }

    fn append_evidence(
        &mut self,
        event: &Event,
        item: &EvidenceItem,
    ) -> Result<(), RepositoryError> {
        let descriptor = self.artifact(item.artifact_id)?;
        self.require_sealed_descriptor(&descriptor)?;
        if descriptor.domain_id != event.domain_id {
            return Err(LedgerError::CrossDomain("evidence artifact").into());
        }
        let Some((representation_index, representation)) = descriptor
            .evidence_representations
            .iter()
            .enumerate()
            .find(|(_, representation)| representation.locator == item.locator)
        else {
            return Err(LedgerError::UnprovenEvidenceRepresentation(item.id).into());
        };
        if !descriptor.is_artifact_digest_bound(representation)
            || item.excerpt_digest != descriptor.content_digest
        {
            return Err(LedgerError::UnprovenEvidenceRepresentation(item.id).into());
        }
        if self.evidence.contains_key(&item.id)
            || row_exists(
                self.transaction,
                "evidence_item",
                "evidence_id",
                item.id.as_bytes(),
            )?
        {
            return Err(LedgerError::DuplicateId {
                kind: "evidence",
                id: item.id.to_string(),
            }
            .into());
        }
        self.transaction.execute(
            concat!(
                "INSERT INTO evidence_item (evidence_id, registered_event_id, artifact_id, ",
                "representation_index, excerpt_digest, evidence_role, evidence_strength, ",
                "extraction_method, extractor_version) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            ),
            params![
                item.id.as_bytes().as_slice(),
                event.id.as_bytes().as_slice(),
                item.artifact_id.as_bytes().as_slice(),
                checked_i64(u64::try_from(representation_index).map_err(|_| {
                    RepositoryError::Corrupt("evidence representation index does not fit u64")
                })?)?,
                item.excerpt_digest.as_bytes().as_slice(),
                evidence_role(item.role),
                evidence_strength(item.strength),
                item.extraction_method,
                item.extractor_version,
            ],
        )?;
        self.evidence.insert(
            item.id,
            AcceptedEvidence {
                artifact_id: item.artifact_id,
                domain_id: event.domain_id,
            },
        );
        Ok(())
    }

    fn append_claim(
        &mut self,
        event: &Event,
        claim: &Claim,
        _accept_seq: u64,
    ) -> Result<(), RepositoryError> {
        let scope = self.scope(claim.scope_id)?;
        if scope.domain_id != event.domain_id {
            return Err(LedgerError::CrossDomain("claim scope").into());
        }
        let mut artifact_ids = BTreeSet::new();
        for evidence_id in &claim.evidence_ids {
            let evidence = self.evidence(*evidence_id)?;
            if evidence.domain_id != event.domain_id {
                return Err(LedgerError::CrossDomain("claim evidence").into());
            }
            let descriptor = self.artifact(evidence.artifact_id)?;
            self.require_sealed_descriptor(&descriptor)?;
            artifact_ids.insert(evidence.artifact_id);
        }
        if self.claims.contains_key(&claim.id)
            || row_exists(self.transaction, "claim", "claim_id", claim.id.as_bytes())?
        {
            return Err(LedgerError::DuplicateId {
                kind: "claim",
                id: claim.id.to_string(),
            }
            .into());
        }
        let object = claim_object_columns(&claim.object);
        let prediction = claim.prediction_metadata;
        self.transaction.execute(
            concat!(
                "INSERT INTO claim (claim_id, assertion_event_id, subject_entity_id, predicate_id, ",
                "scope_id, object_kind, object_entity_id, object_text, object_integer, ",
                "object_decimal_coefficient, object_decimal_scale, object_interval_from, ",
                "object_interval_to, authority_class, epistemic_status, confidence_permille, ",
                "prediction_metadata_version, prediction_observation_from, prediction_observation_to, ",
                "prediction_sample_count, valid_from, valid_to) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ",
                "?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"
            ),
            params![
                claim.id.as_bytes().as_slice(),
                event.id.as_bytes().as_slice(),
                claim.subject_entity_id.as_bytes().as_slice(),
                claim.predicate_id.as_str(),
                claim.scope_id.as_bytes().as_slice(),
                object.kind,
                object.entity_id.as_ref().map(|id| id.as_bytes().as_slice()),
                object.text,
                object.integer,
                object.decimal_coefficient,
                object.decimal_scale,
                object.interval_from,
                object.interval_to,
                authority_class(claim.authority_class),
                epistemic_status(claim.epistemic_status),
                claim.confidence.map(|confidence| i64::from(confidence.value())),
                prediction.map(|metadata| i64::from(metadata.version())),
                prediction.map(|metadata| metadata.observation_window().from().value()),
                prediction.map(|metadata| metadata.observation_window().to().value()),
                prediction.map(|metadata| i64::from(metadata.positive_sample_count())),
                claim.valid_time.from().value(),
                claim.valid_time.to().map(TimestampMillis::value),
            ],
        )?;
        for (ordinal, evidence_id) in claim.evidence_ids.iter().enumerate() {
            self.transaction.execute(
                "INSERT INTO claim_evidence (claim_id, evidence_id, evidence_ordinal) VALUES (?1, ?2, ?3)",
                params![
                    claim.id.as_bytes().as_slice(),
                    evidence_id.as_bytes().as_slice(),
                    checked_i64(u64::try_from(ordinal).map_err(|_| {
                        RepositoryError::Corrupt("claim evidence ordinal does not fit u64")
                    })?)?,
                ],
            )?;
        }
        self.claims.insert(
            claim.id,
            AcceptedClaim {
                claim: claim.clone(),
                domain_id: event.domain_id,
                artifact_ids,
            },
        );
        Ok(())
    }

    fn append_relation(
        &mut self,
        event: &Event,
        relation: &academic_domain::ClaimRelation,
        _accept_seq: u64,
    ) -> Result<(), RepositoryError> {
        let source = self.claim(relation.source_claim_id)?;
        let target = self.claim(relation.target_claim_id)?;
        if source.claim.scope_id != relation.scope_id || target.claim.scope_id != relation.scope_id
        {
            return Err(LedgerError::CrossScope("claim relation").into());
        }
        if source.domain_id != event.domain_id || target.domain_id != event.domain_id {
            return Err(LedgerError::CrossDomain("claim relation").into());
        }
        for artifact_id in source.artifact_ids.union(&target.artifact_ids) {
            let descriptor = self.artifact(*artifact_id)?;
            self.require_sealed_descriptor(&descriptor)?;
        }
        if !relation_effect_is_authorized_for_kind(
            ResolverActorKind::from(&event.actor),
            relation.kind,
            &source.claim,
            &target.claim,
        ) {
            return Err(LedgerError::UnauthorizedRelationEffect {
                actor: event.actor.kind_name(),
                kind: relation.kind,
            }
            .into());
        }
        self.transaction.execute(
            concat!(
                "INSERT INTO claim_relation (relation_event_id, source_claim_id, target_claim_id, ",
                "scope_id, relation_kind, actor_kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ),
            params![
                event.id.as_bytes().as_slice(),
                relation.source_claim_id.as_bytes().as_slice(),
                relation.target_claim_id.as_bytes().as_slice(),
                relation.scope_id.as_bytes().as_slice(),
                relation_kind(relation.kind),
                actor_kind(&event.actor),
            ],
        )?;
        Ok(())
    }

    fn append_decision(
        &mut self,
        event: &Event,
        decision: &UserDecision,
        _accept_seq: u64,
    ) -> Result<(), RepositoryError> {
        let target = self.claim(decision.target_claim_id)?;
        if target.claim.scope_id != decision.resolution_slot.scope_id {
            return Err(LedgerError::CrossScope("user decision").into());
        }
        if target.domain_id != event.domain_id {
            return Err(LedgerError::CrossDomain("user decision").into());
        }
        for artifact_id in &target.artifact_ids {
            let descriptor = self.artifact(*artifact_id)?;
            self.require_sealed_descriptor(&descriptor)?;
        }
        if target.claim.subject_entity_id != decision.resolution_slot.subject_entity_id
            || target.claim.predicate_id != decision.resolution_slot.predicate_id
            || target.claim.object != decision.target_object
        {
            return Err(LedgerError::DecisionSemanticMismatch.into());
        }
        if row_exists(
            self.transaction,
            "user_decision",
            "decision_id",
            decision.id.as_bytes(),
        )? {
            return Err(LedgerError::DuplicateId {
                kind: "decision",
                id: decision.id.to_string(),
            }
            .into());
        }
        for evidence_id in &decision.rationale_evidence_ids {
            let evidence = self.evidence(*evidence_id)?;
            if evidence.domain_id != event.domain_id {
                return Err(LedgerError::CrossDomain("decision evidence").into());
            }
            let descriptor = self.artifact(evidence.artifact_id)?;
            self.require_sealed_descriptor(&descriptor)?;
        }
        let (action, replacement_claim_id) = match decision.action {
            DecisionAction::Confirm => ("CONFIRM", None),
            DecisionAction::Reject => ("REJECT", None),
            DecisionAction::Replace {
                replacement_claim_id,
            } => {
                let replacement = self.claim(replacement_claim_id)?;
                if replacement.claim.scope_id != decision.resolution_slot.scope_id {
                    return Err(LedgerError::CrossScope("replacement decision").into());
                }
                if replacement.domain_id != event.domain_id {
                    return Err(LedgerError::CrossDomain("replacement decision").into());
                }
                for artifact_id in &replacement.artifact_ids {
                    let descriptor = self.artifact(*artifact_id)?;
                    self.require_sealed_descriptor(&descriptor)?;
                }
                if replacement.claim.subject_entity_id != decision.resolution_slot.subject_entity_id
                    || replacement.claim.predicate_id != decision.resolution_slot.predicate_id
                    || replacement.claim.object == decision.target_object
                {
                    return Err(LedgerError::DecisionSemanticMismatch.into());
                }
                ("REPLACE", Some(replacement_claim_id))
            }
        };
        self.transaction.execute(
            concat!(
                "INSERT INTO user_decision (decision_id, decision_event_id, target_claim_id, ",
                "target_object_canonical, resolution_subject_entity_id, resolution_predicate_id, ",
                "resolution_scope_id, action, replacement_claim_id, valid_from, valid_to, ",
                "rationale_evidence_ids_canonical, decided_at, reversible_until) ",
                "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                decision.id.as_bytes().as_slice(),
                event.id.as_bytes().as_slice(),
                decision.target_claim_id.as_bytes().as_slice(),
                encode_canonical_claim_object(&decision.target_object)?,
                decision
                    .resolution_slot
                    .subject_entity_id
                    .as_bytes()
                    .as_slice(),
                decision.resolution_slot.predicate_id.as_str(),
                decision.resolution_slot.scope_id.as_bytes().as_slice(),
                action,
                replacement_claim_id
                    .as_ref()
                    .map(|id| id.as_bytes().as_slice()),
                decision.valid_time.from().value(),
                decision.valid_time.to().map(TimestampMillis::value),
                encode_canonical_evidence_ids(&decision.rationale_evidence_ids)?,
                decision.decided_at.value(),
                decision.reversible_until.map(TimestampMillis::value),
            ],
        )?;
        Ok(())
    }

    fn scope(&self, id: ScopeId) -> Result<ScopeDescriptor, RepositoryError> {
        if let Some(scope) = self.scopes.get(&id) {
            return Ok(scope.clone());
        }
        load_scope(self.transaction, id)?.ok_or_else(|| LedgerError::UnknownScope(id).into())
    }

    fn artifact(&self, id: ArtifactId) -> Result<ArtifactDescriptor, RepositoryError> {
        if let Some(artifact) = self.artifacts.get(&id) {
            return Ok(artifact.clone());
        }
        load_artifact(self.transaction, id)?.ok_or_else(|| LedgerError::UnknownArtifact(id).into())
    }

    fn evidence(&self, id: EvidenceId) -> Result<AcceptedEvidence, RepositoryError> {
        if let Some(evidence) = self.evidence.get(&id) {
            return Ok(evidence.clone());
        }
        load_evidence(self.transaction, id)?.ok_or_else(|| LedgerError::UnknownEvidence(id).into())
    }

    fn claim(&self, id: ClaimId) -> Result<AcceptedClaim, RepositoryError> {
        if let Some(claim) = self.claims.get(&id) {
            return Ok(claim.clone());
        }
        load_claim(self.transaction, id)?.ok_or_else(|| LedgerError::UnknownClaim(id).into())
    }

    fn require_sealed_descriptor(
        &self,
        descriptor: &ArtifactDescriptor,
    ) -> Result<(), RepositoryError> {
        let Some(receipt) = self.sealed_receipts.get(&descriptor.id) else {
            return Err(RepositoryError::MissingSealedReceipt(descriptor.id));
        };
        if receipt.descriptor() != descriptor {
            return Err(RepositoryError::MismatchedSealedReceipt(descriptor.id));
        }
        Ok(())
    }
}

fn load_scope(
    transaction: &Transaction<'_>,
    id: ScopeId,
) -> Result<Option<ScopeDescriptor>, RepositoryError> {
    transaction
        .query_row(
            "SELECT domain_id, label FROM scope WHERE scope_id = ?1",
            [id.as_bytes().as_slice()],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(domain, label)| {
            Ok(ScopeDescriptor {
                id,
                domain_id: id_from_blob(domain)?,
                label,
            })
        })
        .transpose()
}

fn load_artifact(
    transaction: &Connection,
    id: ArtifactId,
) -> Result<Option<ArtifactDescriptor>, RepositoryError> {
    let row = transaction
        .query_row(
            concat!(
                "SELECT content_digest, media_type, byte_length, domain_id, confidentiality, ",
                "retention_class, permission_lineage_id, format_version, vault_locator ",
                "FROM artifact_descriptor WHERE artifact_id = ?1"
            ),
            [id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Vec<u8>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        digest,
        media_type,
        byte_length,
        domain,
        confidentiality_value,
        retention,
        permission,
        format_version,
        locator,
    )) = row
    else {
        return Ok(None);
    };
    let mut statement = transaction.prepare(concat!(
        "SELECT locator_kind, locator_payload, content_digest, byte_length ",
        "FROM artifact_representation WHERE artifact_id = ?1 ORDER BY representation_index"
    ))?;
    let rows = statement.query_map([id.as_bytes().as_slice()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    let mut representations = Vec::new();
    for row in rows {
        let (kind, payload, representation_digest, representation_length) = row?;
        representations.push(academic_domain::ArtifactRepresentation {
            locator: decode_locator(&kind, &payload)?,
            content_digest: digest_from_blob(representation_digest)?,
            byte_length: nonnegative_u64(representation_length, "representation byte length")?,
        });
    }
    let locator = digest_bytes(locator, "vault locator")?;
    let descriptor = ArtifactDescriptor {
        id,
        content_digest: digest_from_blob(digest)?,
        media_type: MediaType::parse(media_type)?,
        byte_length: nonnegative_u64(byte_length, "artifact byte length")?,
        domain_id: id_from_blob(domain)?,
        confidentiality: parse_confidentiality(&confidentiality_value)?,
        retention_class: parse_retention(&retention)?,
        permission_lineage_id: id_from_blob(permission)?,
        format_version: u16::try_from(format_version)
            .map_err(|_| RepositoryError::Corrupt("artifact format version is invalid"))?,
        vault_locator: format!("locator:v1:{}", hex_lower(&locator)).parse()?,
        evidence_representations: representations,
    };
    descriptor.validate()?;
    Ok(Some(descriptor))
}

fn load_evidence(
    transaction: &Transaction<'_>,
    id: EvidenceId,
) -> Result<Option<AcceptedEvidence>, RepositoryError> {
    let row = transaction
        .query_row(
            concat!(
                "SELECT e.artifact_id, r.locator_kind, r.locator_payload, e.excerpt_digest, ",
                "e.evidence_role, e.evidence_strength, e.extraction_method, e.extractor_version, ",
                "l.domain_id FROM evidence_item e ",
                "JOIN artifact_representation r ON r.artifact_id = e.artifact_id ",
                "AND r.representation_index = e.representation_index ",
                "JOIN ledger_event l ON l.event_id = e.registered_event_id ",
                "WHERE e.evidence_id = ?1"
            ),
            [id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Vec<u8>>(8)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(
            artifact,
            locator_kind,
            locator_payload,
            excerpt,
            role,
            strength,
            method,
            version,
            domain,
        )| {
            let artifact_id = id_from_blob(artifact)?;
            let item = EvidenceItem {
                id,
                artifact_id,
                locator: decode_locator(&locator_kind, &locator_payload)?,
                excerpt_digest: digest_from_blob(excerpt)?,
                role: parse_evidence_role(&role)?,
                strength: parse_evidence_strength(&strength)?,
                extraction_method: method,
                extractor_version: version,
            };
            item.validate()?;
            Ok(AcceptedEvidence {
                artifact_id,
                domain_id: id_from_blob(domain)?,
            })
        },
    )
    .transpose()
}

fn load_claim(
    transaction: &Transaction<'_>,
    id: ClaimId,
) -> Result<Option<AcceptedClaim>, RepositoryError> {
    type ClaimRow = (
        Vec<u8>,
        String,
        Vec<u8>,
        String,
        Option<Vec<u8>>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        String,
        String,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        i64,
        Option<i64>,
        Vec<u8>,
    );
    let row: Option<ClaimRow> = transaction
        .query_row(
            concat!(
                "SELECT c.subject_entity_id, c.predicate_id, c.scope_id, c.object_kind, ",
                "c.object_entity_id, c.object_text, c.object_integer, c.object_decimal_coefficient, ",
                "c.object_decimal_scale, c.object_interval_from, c.object_interval_to, ",
                "c.authority_class, c.epistemic_status, c.confidence_permille, ",
                "c.prediction_metadata_version, c.prediction_observation_from, ",
                "c.prediction_observation_to, c.prediction_sample_count, c.valid_from, c.valid_to, ",
                "l.domain_id FROM claim c JOIN ledger_event l ON l.event_id = c.assertion_event_id ",
                "WHERE c.claim_id = ?1"
            ),
            [id.as_bytes().as_slice()],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                    row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
                    row.get(10)?, row.get(11)?, row.get(12)?, row.get(13)?, row.get(14)?,
                    row.get(15)?, row.get(16)?, row.get(17)?, row.get(18)?, row.get(19)?,
                    row.get(20)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let mut statement = transaction.prepare(
        "SELECT evidence_id FROM claim_evidence WHERE claim_id = ?1 ORDER BY evidence_ordinal",
    )?;
    let evidence_rows =
        statement.query_map([id.as_bytes().as_slice()], |row| row.get::<_, Vec<u8>>(0))?;
    let mut evidence_ids = Vec::new();
    for evidence in evidence_rows {
        evidence_ids.push(id_from_blob(evidence?)?);
    }
    let mut artifact_ids = BTreeSet::new();
    for evidence_id in &evidence_ids {
        let evidence = load_evidence(transaction, *evidence_id)?
            .ok_or(LedgerError::UnknownEvidence(*evidence_id))?;
        artifact_ids.insert(evidence.artifact_id);
    }
    let object = decode_claim_object(StoredClaimObject {
        kind: row.3,
        entity: row.4,
        text: row.5,
        integer: row.6,
        coefficient: row.7,
        scale: row.8,
        interval_from: row.9,
        interval_to: row.10,
    })?;
    let prediction_metadata = decode_prediction_metadata(row.14, row.15, row.16, row.17)?;
    let claim = Claim {
        id,
        subject_entity_id: id_from_blob(row.0)?,
        predicate_id: PredicateId::parse(row.1)?,
        object,
        scope_id: id_from_blob(row.2)?,
        authority_class: parse_authority(&row.11)?,
        epistemic_status: parse_epistemic(&row.12)?,
        confidence: row
            .13
            .map(|value| {
                u16::try_from(value)
                    .map_err(|_| RepositoryError::Corrupt("claim confidence is invalid"))
                    .and_then(|value| {
                        academic_domain::ConfidencePermille::new(value).map_err(Into::into)
                    })
            })
            .transpose()?,
        prediction_metadata,
        valid_time: ValidInterval::new(
            TimestampMillis::new(row.18),
            row.19.map(TimestampMillis::new),
        )?,
        evidence_ids,
    };
    claim.validate()?;
    Ok(Some(AcceptedClaim {
        claim,
        domain_id: id_from_blob(row.20)?,
        artifact_ids,
    }))
}

struct ClaimObjectColumns {
    kind: &'static str,
    entity_id: Option<EntityId>,
    text: Option<String>,
    integer: Option<i64>,
    decimal_coefficient: Option<String>,
    decimal_scale: Option<i64>,
    interval_from: Option<i64>,
    interval_to: Option<i64>,
}

fn claim_object_columns(object: &ClaimObject) -> ClaimObjectColumns {
    let empty = || ClaimObjectColumns {
        kind: "",
        entity_id: None,
        text: None,
        integer: None,
        decimal_coefficient: None,
        decimal_scale: None,
        interval_from: None,
        interval_to: None,
    };
    match object {
        ClaimObject::Entity(id) => ClaimObjectColumns {
            kind: "ENTITY",
            entity_id: Some(*id),
            ..empty()
        },
        ClaimObject::Text(text) => ClaimObjectColumns {
            kind: "TEXT",
            text: Some(text.clone()),
            ..empty()
        },
        ClaimObject::Integer(value) => ClaimObjectColumns {
            kind: "INTEGER",
            integer: Some(*value),
            ..empty()
        },
        ClaimObject::Boolean(value) => ClaimObjectColumns {
            kind: "BOOLEAN",
            integer: Some(i64::from(*value)),
            ..empty()
        },
        ClaimObject::Decimal(value) => ClaimObjectColumns {
            kind: "DECIMAL",
            decimal_coefficient: Some(value.coefficient().to_string()),
            decimal_scale: Some(i64::from(value.scale())),
            ..empty()
        },
        ClaimObject::Instant(value) => ClaimObjectColumns {
            kind: "INSTANT",
            integer: Some(value.value()),
            ..empty()
        },
        ClaimObject::Interval(value) => ClaimObjectColumns {
            kind: "INTERVAL",
            interval_from: Some(value.from().value()),
            interval_to: value.to().map(TimestampMillis::value),
            ..empty()
        },
        ClaimObject::Mastery(value) => ClaimObjectColumns {
            kind: "MASTERY",
            text: Some(mastery(*value).to_owned()),
            ..empty()
        },
        ClaimObject::Freshness(value) => ClaimObjectColumns {
            kind: "FRESHNESS",
            text: Some(freshness(*value).to_owned()),
            ..empty()
        },
    }
}

pub(crate) struct StoredClaimObject {
    pub kind: String,
    pub entity: Option<Vec<u8>>,
    pub text: Option<String>,
    pub integer: Option<i64>,
    pub coefficient: Option<String>,
    pub scale: Option<i64>,
    pub interval_from: Option<i64>,
    pub interval_to: Option<i64>,
}

pub(crate) fn decode_claim_object(
    columns: StoredClaimObject,
) -> Result<ClaimObject, RepositoryError> {
    match columns.kind.as_str() {
        "ENTITY" => Ok(ClaimObject::Entity(id_from_blob(required(
            columns.entity,
            "entity object",
        )?)?)),
        "TEXT" => Ok(ClaimObject::Text(required(columns.text, "text object")?)),
        "INTEGER" => Ok(ClaimObject::Integer(required(
            columns.integer,
            "integer object",
        )?)),
        "BOOLEAN" => match required(columns.integer, "boolean object")? {
            0 => Ok(ClaimObject::Boolean(false)),
            1 => Ok(ClaimObject::Boolean(true)),
            _ => Err(RepositoryError::Corrupt("boolean object is invalid")),
        },
        "DECIMAL" => Ok(ClaimObject::Decimal(Decimal::new(
            required(columns.coefficient, "decimal coefficient")?
                .parse()
                .map_err(|_| RepositoryError::Corrupt("decimal coefficient is invalid"))?,
            u8::try_from(required(columns.scale, "decimal scale")?)
                .map_err(|_| RepositoryError::Corrupt("decimal scale is invalid"))?,
        )?)),
        "INSTANT" => Ok(ClaimObject::Instant(TimestampMillis::new(required(
            columns.integer,
            "instant object",
        )?))),
        "INTERVAL" => Ok(ClaimObject::Interval(ValidInterval::new(
            TimestampMillis::new(required(columns.interval_from, "interval lower bound")?),
            columns.interval_to.map(TimestampMillis::new),
        )?)),
        "MASTERY" => Ok(ClaimObject::Mastery(parse_mastery(&required(
            columns.text,
            "mastery object",
        )?)?)),
        "FRESHNESS" => Ok(ClaimObject::Freshness(parse_freshness(&required(
            columns.text,
            "freshness object",
        )?)?)),
        _ => Err(RepositoryError::Corrupt("claim object kind is unknown")),
    }
}

pub(crate) fn decode_prediction_metadata(
    version: Option<i64>,
    from: Option<i64>,
    to: Option<i64>,
    samples: Option<i64>,
) -> Result<Option<PredictionMetadata>, RepositoryError> {
    match (version, from, to, samples) {
        (None, None, None, None) => Ok(None),
        (Some(version), Some(from), Some(to), Some(samples)) => {
            let version = u16::try_from(version)
                .map_err(|_| RepositoryError::Corrupt("prediction metadata version is invalid"))?;
            if version != academic_domain::PREDICTION_METADATA_VERSION_V1 {
                return Err(RepositoryError::Corrupt(
                    "prediction metadata version is unsupported",
                ));
            }
            let samples = u32::try_from(samples)
                .map_err(|_| RepositoryError::Corrupt("prediction sample count is invalid"))?;
            Ok(Some(PredictionMetadata::new(
                PredictionObservationWindow::new(
                    TimestampMillis::new(from),
                    TimestampMillis::new(to),
                )?,
                samples,
            )?))
        }
        _ => Err(RepositoryError::Corrupt(
            "prediction metadata columns are partially present",
        )),
    }
}

fn encode_locator(locator: &EvidenceLocator) -> Result<(&'static str, Vec<u8>), RepositoryError> {
    let kind = match locator {
        EvidenceLocator::TextBytes { .. } => "TEXT_BYTES",
        EvidenceLocator::Page { .. } => "PAGE",
        EvidenceLocator::TranscriptTime { .. } => "TRANSCRIPT_TIME",
        EvidenceLocator::RepositoryBytes { .. } => "REPOSITORY_BYTES",
    };
    Ok((kind, encode_canonical_evidence_locator(locator)?))
}

fn decode_locator(kind: &str, bytes: &[u8]) -> Result<EvidenceLocator, RepositoryError> {
    let locator = decode_canonical_evidence_locator(bytes)?;
    let actual_kind = match &locator {
        EvidenceLocator::TextBytes { .. } => "TEXT_BYTES",
        EvidenceLocator::Page { .. } => "PAGE",
        EvidenceLocator::TranscriptTime { .. } => "TRANSCRIPT_TIME",
        EvidenceLocator::RepositoryBytes { .. } => "REPOSITORY_BYTES",
    };
    if actual_kind != kind {
        return Err(RepositoryError::Corrupt(
            "evidence locator kind disagrees with canonical payload",
        ));
    }
    Ok(locator)
}

fn row_exists(
    transaction: &Transaction<'_>,
    table: &'static str,
    column: &'static str,
    id: &[u8; 16],
) -> Result<bool, RepositoryError> {
    let sql = format!("SELECT 1 FROM {table} WHERE {column} = ?1");
    Ok(transaction
        .query_row(&sql, [id.as_slice()], |row| row.get::<_, i64>(0))
        .optional()?
        .is_some())
}

fn required<T>(value: Option<T>, reason: &'static str) -> Result<T, RepositoryError> {
    value.ok_or(RepositoryError::Corrupt(reason))
}

fn checked_i64(value: u64) -> Result<i64, RepositoryError> {
    i64::try_from(value).map_err(|_| RepositoryError::IntegerOverflow(value))
}

fn nonnegative_u64(value: i64, reason: &'static str) -> Result<u64, RepositoryError> {
    u64::try_from(value).map_err(|_| RepositoryError::Corrupt(reason))
}

fn positive_u64(value: i64, reason: &'static str) -> Result<u64, RepositoryError> {
    let value = nonnegative_u64(value, reason)?;
    if value == 0 {
        Err(RepositoryError::Corrupt(reason))
    } else {
        Ok(value)
    }
}

fn digest_from_blob(bytes: Vec<u8>) -> Result<ContentDigest, RepositoryError> {
    Ok(ContentDigest::from_sha256_bytes(digest_bytes(
        bytes,
        "digest length is invalid",
    )?))
}

fn digest_bytes(bytes: Vec<u8>, reason: &'static str) -> Result<[u8; 32], RepositoryError> {
    bytes
        .try_into()
        .map_err(|_| RepositoryError::Corrupt(reason))
}

pub(crate) fn id_from_blob<T>(bytes: Vec<u8>) -> Result<T, RepositoryError>
where
    T: FromStr<Err = DomainError>,
{
    let bytes: [u8; 16] = bytes
        .try_into()
        .map_err(|_| RepositoryError::Corrupt("identifier length is invalid"))?;
    let text = format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    );
    Ok(text.parse()?)
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn actor_kind(actor: &Actor) -> &'static str {
    actor.kind_name()
}

fn event_kind(payload: &EventPayload) -> &'static str {
    payload.kind()
}

fn confidentiality(value: Confidentiality) -> &'static str {
    match value {
        Confidentiality::Public => "PUBLIC",
        Confidentiality::Personal => "PERSONAL",
        Confidentiality::Restricted => "RESTRICTED",
        Confidentiality::Secret => "SECRET",
    }
}

fn parse_confidentiality(value: &str) -> Result<Confidentiality, RepositoryError> {
    match value {
        "PUBLIC" => Ok(Confidentiality::Public),
        "PERSONAL" => Ok(Confidentiality::Personal),
        "RESTRICTED" => Ok(Confidentiality::Restricted),
        "SECRET" => Ok(Confidentiality::Secret),
        _ => Err(RepositoryError::Corrupt("confidentiality is invalid")),
    }
}

fn retention_class(value: RetentionClass) -> &'static str {
    match value {
        RetentionClass::Ephemeral => "EPHEMERAL",
        RetentionClass::CourseTerm => "COURSE_TERM",
        RetentionClass::UserManaged => "USER_MANAGED",
        RetentionClass::LegalHold => "LEGAL_HOLD",
    }
}

fn parse_retention(value: &str) -> Result<RetentionClass, RepositoryError> {
    match value {
        "EPHEMERAL" => Ok(RetentionClass::Ephemeral),
        "COURSE_TERM" => Ok(RetentionClass::CourseTerm),
        "USER_MANAGED" => Ok(RetentionClass::UserManaged),
        "LEGAL_HOLD" => Ok(RetentionClass::LegalHold),
        _ => Err(RepositoryError::Corrupt("retention class is invalid")),
    }
}

fn evidence_role(value: EvidenceRole) -> &'static str {
    match value {
        EvidenceRole::Supports => "SUPPORTS",
        EvidenceRole::Contradicts => "CONTRADICTS",
        EvidenceRole::ContextOnly => "CONTEXT_ONLY",
    }
}

fn parse_evidence_role(value: &str) -> Result<EvidenceRole, RepositoryError> {
    match value {
        "SUPPORTS" => Ok(EvidenceRole::Supports),
        "CONTRADICTS" => Ok(EvidenceRole::Contradicts),
        "CONTEXT_ONLY" => Ok(EvidenceRole::ContextOnly),
        _ => Err(RepositoryError::Corrupt("evidence role is invalid")),
    }
}

fn evidence_strength(value: EvidenceStrength) -> &'static str {
    match value {
        EvidenceStrength::Direct => "DIRECT",
        EvidenceStrength::Corroborating => "CORROBORATING",
        EvidenceStrength::Weak => "WEAK",
    }
}

fn parse_evidence_strength(value: &str) -> Result<EvidenceStrength, RepositoryError> {
    match value {
        "DIRECT" => Ok(EvidenceStrength::Direct),
        "CORROBORATING" => Ok(EvidenceStrength::Corroborating),
        "WEAK" => Ok(EvidenceStrength::Weak),
        _ => Err(RepositoryError::Corrupt("evidence strength is invalid")),
    }
}

fn authority_class(value: AuthorityClass) -> &'static str {
    match value {
        AuthorityClass::Official => "OFFICIAL",
        AuthorityClass::UserExplicit => "USER_EXPLICIT",
        AuthorityClass::DirectObservation => "DIRECT_OBSERVATION",
        AuthorityClass::Curated => "CURATED",
        AuthorityClass::DeterministicEngine => "DETERMINISTIC_ENGINE",
        AuthorityClass::ModelInference => "MODEL_INFERENCE",
        AuthorityClass::Prediction => "PREDICTION",
        AuthorityClass::Unknown => "UNKNOWN",
    }
}

pub(crate) fn parse_authority(value: &str) -> Result<AuthorityClass, RepositoryError> {
    match value {
        "OFFICIAL" => Ok(AuthorityClass::Official),
        "USER_EXPLICIT" => Ok(AuthorityClass::UserExplicit),
        "DIRECT_OBSERVATION" => Ok(AuthorityClass::DirectObservation),
        "CURATED" => Ok(AuthorityClass::Curated),
        "DETERMINISTIC_ENGINE" => Ok(AuthorityClass::DeterministicEngine),
        "MODEL_INFERENCE" => Ok(AuthorityClass::ModelInference),
        "PREDICTION" => Ok(AuthorityClass::Prediction),
        "UNKNOWN" => Ok(AuthorityClass::Unknown),
        _ => Err(RepositoryError::Corrupt("authority class is invalid")),
    }
}

fn epistemic_status(value: EpistemicStatus) -> &'static str {
    match value {
        EpistemicStatus::OfficialConfirmed => "OFFICIAL_CONFIRMED",
        EpistemicStatus::UserConfirmed => "USER_CONFIRMED",
        EpistemicStatus::CodeObserved => "CODE_OBSERVED",
        EpistemicStatus::DeterministicDerived => "DETERMINISTIC_DERIVED",
        EpistemicStatus::AiInferred => "AI_INFERRED",
        EpistemicStatus::Prediction => "PREDICTION",
        EpistemicStatus::Disputed => "DISPUTED",
        EpistemicStatus::Superseded => "SUPERSEDED",
        EpistemicStatus::Unknown => "UNKNOWN",
    }
}

pub(crate) fn parse_epistemic(value: &str) -> Result<EpistemicStatus, RepositoryError> {
    match value {
        "OFFICIAL_CONFIRMED" => Ok(EpistemicStatus::OfficialConfirmed),
        "USER_CONFIRMED" => Ok(EpistemicStatus::UserConfirmed),
        "CODE_OBSERVED" => Ok(EpistemicStatus::CodeObserved),
        "DETERMINISTIC_DERIVED" => Ok(EpistemicStatus::DeterministicDerived),
        "AI_INFERRED" => Ok(EpistemicStatus::AiInferred),
        "PREDICTION" => Ok(EpistemicStatus::Prediction),
        "DISPUTED" => Ok(EpistemicStatus::Disputed),
        "SUPERSEDED" => Ok(EpistemicStatus::Superseded),
        "UNKNOWN" => Ok(EpistemicStatus::Unknown),
        _ => Err(RepositoryError::Corrupt("epistemic status is invalid")),
    }
}

fn relation_kind(value: ClaimRelationKind) -> &'static str {
    match value {
        ClaimRelationKind::Supports => "SUPPORTS",
        ClaimRelationKind::Contradicts => "CONTRADICTS",
        ClaimRelationKind::Supersedes => "SUPERSEDES",
        ClaimRelationKind::Retracts => "RETRACTS",
        ClaimRelationKind::Duplicates => "DUPLICATES",
    }
}

fn mastery(value: MasteryLevel) -> &'static str {
    match value {
        MasteryLevel::Unseen => "UNSEEN",
        MasteryLevel::Exposed => "EXPOSED",
        MasteryLevel::Understood => "UNDERSTOOD",
        MasteryLevel::Practiced => "PRACTICED",
        MasteryLevel::Applied => "APPLIED",
        MasteryLevel::Fluent => "FLUENT",
    }
}

fn parse_mastery(value: &str) -> Result<MasteryLevel, RepositoryError> {
    match value {
        "UNSEEN" => Ok(MasteryLevel::Unseen),
        "EXPOSED" => Ok(MasteryLevel::Exposed),
        "UNDERSTOOD" => Ok(MasteryLevel::Understood),
        "PRACTICED" => Ok(MasteryLevel::Practiced),
        "APPLIED" => Ok(MasteryLevel::Applied),
        "FLUENT" => Ok(MasteryLevel::Fluent),
        _ => Err(RepositoryError::Corrupt("mastery value is invalid")),
    }
}

fn freshness(value: FreshnessBand) -> &'static str {
    match value {
        FreshnessBand::Unknown => "UNKNOWN",
        FreshnessBand::Stale => "STALE",
        FreshnessBand::Low => "LOW",
        FreshnessBand::Moderate => "MODERATE",
        FreshnessBand::High => "HIGH",
        FreshnessBand::VeryHigh => "VERY_HIGH",
    }
}

fn parse_freshness(value: &str) -> Result<FreshnessBand, RepositoryError> {
    match value {
        "UNKNOWN" => Ok(FreshnessBand::Unknown),
        "STALE" => Ok(FreshnessBand::Stale),
        "LOW" => Ok(FreshnessBand::Low),
        "MODERATE" => Ok(FreshnessBand::Moderate),
        "HIGH" => Ok(FreshnessBand::High),
        "VERY_HIGH" => Ok(FreshnessBand::VeryHigh),
        _ => Err(RepositoryError::Corrupt("freshness value is invalid")),
    }
}
