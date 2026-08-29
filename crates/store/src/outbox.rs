//! Transactional projection-outbox rows coupled to canonical acceptance.

use std::{error::Error, fmt};

use academic_contracts::VerifiedBatch;
use academic_domain::{BatchId, ContentDigest, DomainError, EventPayload, TimestampMillis};
use rusqlite::{Transaction, params};

use crate::{connection::ReaderConnection, error::StoreError};

type RawOutboxRow = (i64, Vec<u8>, i64, i64, i64, Vec<u8>, Vec<u8>, i64);

/// One durable projection notification. Its sequence equals the committed
/// canonical revision, so an outbox row can never lead canonical state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEntry {
    pub outbox_seq: u64,
    pub accepted_batch_id: BatchId,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
    pub canonical_revision: u64,
    pub event_kind_mask: [u8; 8],
    pub payload_digest: ContentDigest,
    pub created_at: TimestampMillis,
}

/// Outbox encoding, storage, or integrity failure.
#[derive(Debug)]
pub enum OutboxError {
    Sqlite(rusqlite::Error),
    Store(StoreError),
    Domain(DomainError),
    Corrupt(&'static str),
    IntegerOverflow(u64),
}

impl fmt::Display for OutboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite outbox error: {error}"),
            Self::Store(error) => write!(formatter, "store outbox error: {error}"),
            Self::Domain(error) => write!(formatter, "invalid outbox identity: {error}"),
            Self::Corrupt(reason) => write!(formatter, "outbox row is corrupt: {reason}"),
            Self::IntegerOverflow(value) => {
                write!(
                    formatter,
                    "outbox value {value} exceeds signed 64-bit storage"
                )
            }
        }
    }
}

impl Error for OutboxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::Corrupt(_) | Self::IntegerOverflow(_) => None,
        }
    }
}

impl From<rusqlite::Error> for OutboxError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<StoreError> for OutboxError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<DomainError> for OutboxError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

pub(crate) fn insert_outbox(
    transaction: &Transaction<'_>,
    verified: &VerifiedBatch,
    accept_seq_start: u64,
    accept_seq_end: u64,
    canonical_revision: u64,
    created_at: TimestampMillis,
) -> Result<(), OutboxError> {
    let mask = event_kind_mask(verified);
    transaction.execute(
        concat!(
            "INSERT INTO projection_outbox (outbox_seq, accepted_batch_id, accept_seq_start, ",
            "accept_seq_end, canonical_revision, event_kind_mask, payload_digest, created_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        ),
        params![
            checked_i64(canonical_revision)?,
            verified.batch().batch_id.as_bytes().as_slice(),
            checked_i64(accept_seq_start)?,
            checked_i64(accept_seq_end)?,
            checked_i64(canonical_revision)?,
            mask.as_slice(),
            verified.payload_hash().as_bytes().as_slice(),
            created_at.value(),
        ],
    )?;
    Ok(())
}

/// Reads all durable outbox rows in canonical order.
pub fn read_outbox(reader: &ReaderConnection) -> Result<Vec<OutboxEntry>, OutboxError> {
    let rows: Vec<RawOutboxRow> = reader.query_collect(
        concat!(
            "SELECT outbox_seq, accepted_batch_id, accept_seq_start, accept_seq_end, ",
            "canonical_revision, event_kind_mask, payload_digest, created_at ",
            "FROM projection_outbox ORDER BY outbox_seq"
        ),
        [],
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
    )?;
    rows.into_iter().map(decode_row).collect()
}

fn event_kind_mask(verified: &VerifiedBatch) -> [u8; 8] {
    let mut value = 0_u64;
    for event in &verified.batch().events {
        let bit = match &event.payload {
            EventPayload::ScopeRegistered(_) => 0,
            EventPayload::ArtifactRegistered(_) => 1,
            EventPayload::EvidenceRegistered(_) => 2,
            EventPayload::ClaimAsserted(_) => 3,
            EventPayload::ClaimRelated(_) => 4,
            EventPayload::DecisionRecorded(_) => 5,
            EventPayload::CurriculumVersionPublished(_) => 6,
            EventPayload::CourseRevisionPublished(_) => 7,
            EventPayload::OfferingObserved(_) => 8,
            EventPayload::AttemptRecorded(_) => 9,
            EventPayload::RequirementSetPublished(_) => 10,
            EventPayload::AuditComputed(_) => 11,
            EventPayload::CapturePermissionRecorded(_) => 12,
            EventPayload::LectureSessionRecorded(_) => 13,
            EventPayload::TranscriptVersionAdded(_) => 14,
            EventPayload::LectureDocumentPublished(_) => 15,
            EventPayload::SnapshotRegistered(_) => 16,
            EventPayload::FindingPublished(_) => 17,
            EventPayload::ModelRunRecorded(_) => 18,
            EventPayload::ProposalDisposed(_) => 19,
            EventPayload::EgressDecided(_) => 20,
            EventPayload::ConsentRecorded(_) => 21,
            EventPayload::EntityIdentityChanged(_) => 22,
            EventPayload::RetentionActionRecorded(_) => 23,
        };
        value |= 1_u64 << bit;
    }
    value.to_be_bytes()
}

fn decode_row(row: RawOutboxRow) -> Result<OutboxEntry, OutboxError> {
    let mask: [u8; 8] = row
        .5
        .try_into()
        .map_err(|_| OutboxError::Corrupt("event-kind mask length is invalid"))?;
    let digest: [u8; 32] = row
        .6
        .try_into()
        .map_err(|_| OutboxError::Corrupt("payload digest length is invalid"))?;
    Ok(OutboxEntry {
        outbox_seq: positive_u64(row.0, "outbox sequence")?,
        accepted_batch_id: id_from_blob(row.1)?,
        accept_seq_start: positive_u64(row.2, "accept sequence start")?,
        accept_seq_end: positive_u64(row.3, "accept sequence end")?,
        canonical_revision: positive_u64(row.4, "canonical revision")?,
        event_kind_mask: mask,
        payload_digest: ContentDigest::from_sha256_bytes(digest),
        created_at: TimestampMillis::new(row.7),
    })
}

fn checked_i64(value: u64) -> Result<i64, OutboxError> {
    i64::try_from(value).map_err(|_| OutboxError::IntegerOverflow(value))
}

fn positive_u64(value: i64, reason: &'static str) -> Result<u64, OutboxError> {
    let value = u64::try_from(value).map_err(|_| OutboxError::Corrupt(reason))?;
    if value == 0 {
        return Err(OutboxError::Corrupt(reason));
    }
    Ok(value)
}

fn id_from_blob<T>(bytes: Vec<u8>) -> Result<T, OutboxError>
where
    T: std::str::FromStr<Err = DomainError>,
{
    let bytes: [u8; 16] = bytes
        .try_into()
        .map_err(|_| OutboxError::Corrupt("identifier length is invalid"))?;
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
