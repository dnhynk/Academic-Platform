//! Request idempotency and byte-exact durable acceptance receipts.

use std::{error::Error, fmt, str::FromStr};

use academic_domain::{BatchId, ContentDigest, DomainError, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, params};

const RECEIPT_MAGIC: &[u8; 4] = b"ASR1";
const RECEIPT_VERSION: u16 = 1;
const RECEIPT_LENGTH: usize = 4 + 2 + 16 + 32 + 8 + 8 + 8;

/// Exact mutable-command identity and source bytes supplied to S2.
#[derive(Debug, Clone, Copy)]
pub struct AcceptanceCommand<'a> {
    pub request_id: [u8; 16],
    pub client_instance_id: [u8; 16],
    pub idempotency_key: [u8; 32],
    pub expected_revision: Option<u64>,
    pub envelope_bytes: &'a [u8],
}

impl AcceptanceCommand<'_> {
    /// Hashes every semantic request field except the idempotency address itself.
    #[must_use]
    pub fn request_hash(&self) -> ContentDigest {
        let mut bytes = Vec::with_capacity(64 + self.envelope_bytes.len());
        bytes.extend_from_slice(b"academic.accept-request.v1\0");
        bytes.extend_from_slice(&self.request_id);
        match self.expected_revision {
            Some(revision) => {
                bytes.push(1);
                bytes.extend_from_slice(&revision.to_be_bytes());
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(
            &u64::try_from(self.envelope_bytes.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        bytes.extend_from_slice(self.envelope_bytes);
        ContentDigest::sha256(&bytes)
    }
}

/// Immutable response persisted before the canonical transaction commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableAcceptanceReceipt {
    pub batch_id: BatchId,
    pub envelope_hash: ContentDigest,
    pub accept_seq_start: u64,
    pub accept_seq_end: u64,
    pub committed_revision: u64,
    response_bytes: Vec<u8>,
    response_hash: ContentDigest,
}

impl DurableAcceptanceReceipt {
    pub(crate) fn new(
        batch_id: BatchId,
        envelope_hash: ContentDigest,
        accept_seq_start: u64,
        accept_seq_end: u64,
        committed_revision: u64,
    ) -> Self {
        let mut response_bytes = Vec::with_capacity(RECEIPT_LENGTH);
        response_bytes.extend_from_slice(RECEIPT_MAGIC);
        response_bytes.extend_from_slice(&RECEIPT_VERSION.to_be_bytes());
        response_bytes.extend_from_slice(batch_id.as_bytes());
        response_bytes.extend_from_slice(envelope_hash.as_bytes());
        response_bytes.extend_from_slice(&accept_seq_start.to_be_bytes());
        response_bytes.extend_from_slice(&accept_seq_end.to_be_bytes());
        response_bytes.extend_from_slice(&committed_revision.to_be_bytes());
        let response_hash = ContentDigest::sha256(&response_bytes);
        Self {
            batch_id,
            envelope_hash,
            accept_seq_start,
            accept_seq_end,
            committed_revision,
            response_bytes,
            response_hash,
        }
    }

    /// Returns the byte-for-byte response replayed after a lost ACK.
    #[must_use]
    pub fn response_bytes(&self) -> &[u8] {
        &self.response_bytes
    }

    /// Returns the digest covering the exact response bytes.
    #[must_use]
    pub const fn response_hash(&self) -> ContentDigest {
        self.response_hash
    }

    fn decode(response_bytes: Vec<u8>, response_hash: [u8; 32]) -> Result<Self, IdempotencyError> {
        if response_bytes.len() != RECEIPT_LENGTH
            || response_bytes.get(..4) != Some(RECEIPT_MAGIC.as_slice())
            || response_bytes.get(4..6) != Some(RECEIPT_VERSION.to_be_bytes().as_slice())
        {
            return Err(IdempotencyError::CorruptReceipt(
                "receipt framing is not canonical",
            ));
        }
        let actual_hash = ContentDigest::sha256(&response_bytes);
        if actual_hash.as_bytes() != &response_hash {
            return Err(IdempotencyError::CorruptReceipt(
                "receipt digest does not match stored response bytes",
            ));
        }
        let batch_bytes: [u8; 16] = response_bytes[6..22]
            .try_into()
            .map_err(|_| IdempotencyError::CorruptReceipt("batch id length is invalid"))?;
        let envelope_hash: [u8; 32] = response_bytes[22..54]
            .try_into()
            .map_err(|_| IdempotencyError::CorruptReceipt("envelope hash length is invalid"))?;
        let accept_seq_start = u64::from_be_bytes(
            response_bytes[54..62]
                .try_into()
                .map_err(|_| IdempotencyError::CorruptReceipt("acceptance start is invalid"))?,
        );
        let accept_seq_end = u64::from_be_bytes(
            response_bytes[62..70]
                .try_into()
                .map_err(|_| IdempotencyError::CorruptReceipt("acceptance end is invalid"))?,
        );
        let committed_revision = u64::from_be_bytes(
            response_bytes[70..78]
                .try_into()
                .map_err(|_| IdempotencyError::CorruptReceipt("revision is invalid"))?,
        );
        if accept_seq_start == 0 || accept_seq_end < accept_seq_start || committed_revision == 0 {
            return Err(IdempotencyError::CorruptReceipt(
                "receipt numeric invariants are invalid",
            ));
        }
        Ok(Self {
            batch_id: id_from_bytes(batch_bytes)?,
            envelope_hash: ContentDigest::from_sha256_bytes(envelope_hash),
            accept_seq_start,
            accept_seq_end,
            committed_revision,
            response_bytes,
            response_hash: actual_hash,
        })
    }
}

/// Idempotency lookup/receipt integrity failure.
#[derive(Debug)]
pub enum IdempotencyError {
    Sqlite(rusqlite::Error),
    Domain(DomainError),
    KeyCollision,
    CorruptReceipt(&'static str),
    IntegerOverflow(u64),
}

impl fmt::Display for IdempotencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "SQLite idempotency error: {error}"),
            Self::Domain(error) => write!(formatter, "invalid stored receipt identity: {error}"),
            Self::KeyCollision => {
                write!(formatter, "idempotency key was reused for another request")
            }
            Self::CorruptReceipt(reason) => {
                write!(formatter, "stored receipt is corrupt: {reason}")
            }
            Self::IntegerOverflow(value) => {
                write!(
                    formatter,
                    "idempotency value {value} exceeds signed 64-bit storage"
                )
            }
        }
    }
}

impl Error for IdempotencyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Domain(error) => Some(error),
            Self::KeyCollision | Self::CorruptReceipt(_) | Self::IntegerOverflow(_) => None,
        }
    }
}

impl From<rusqlite::Error> for IdempotencyError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<DomainError> for IdempotencyError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

pub(crate) fn lookup_command_receipt(
    transaction: &Transaction<'_>,
    command: &AcceptanceCommand<'_>,
) -> Result<Option<DurableAcceptanceReceipt>, IdempotencyError> {
    let stored = transaction
        .query_row(
            concat!(
                "SELECT request_hash, committed_revision, response_bytes, response_hash ",
                "FROM command_receipt WHERE client_instance_id = ?1 AND idempotency_key = ?2"
            ),
            params![
                command.client_instance_id.as_slice(),
                command.idempotency_key.as_slice()
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((request_hash, committed_revision, response_bytes, response_hash)) = stored else {
        return Ok(None);
    };
    if request_hash.as_slice() != command.request_hash().as_bytes() {
        return Err(IdempotencyError::KeyCollision);
    }
    let response_hash: [u8; 32] = response_hash
        .try_into()
        .map_err(|_| IdempotencyError::CorruptReceipt("response hash length is invalid"))?;
    let receipt = DurableAcceptanceReceipt::decode(response_bytes, response_hash)?;
    if i64::try_from(receipt.committed_revision).ok() != Some(committed_revision) {
        return Err(IdempotencyError::CorruptReceipt(
            "receipt revision disagrees with its row",
        ));
    }
    Ok(Some(receipt))
}

pub(crate) fn insert_command_receipt(
    transaction: &Transaction<'_>,
    command: &AcceptanceCommand<'_>,
    receipt: &DurableAcceptanceReceipt,
    created_at: TimestampMillis,
) -> Result<(), IdempotencyError> {
    let expected_revision = command.expected_revision.map(checked_i64).transpose()?;
    transaction.execute(
        concat!(
            "INSERT INTO command_receipt (client_instance_id, idempotency_key, request_hash, ",
            "expected_revision, committed_revision, response_bytes, response_hash, created_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        ),
        params![
            command.client_instance_id.as_slice(),
            command.idempotency_key.as_slice(),
            command.request_hash().as_bytes().as_slice(),
            expected_revision,
            checked_i64(receipt.committed_revision)?,
            receipt.response_bytes(),
            receipt.response_hash().as_bytes().as_slice(),
            created_at.value(),
        ],
    )?;
    Ok(())
}

fn checked_i64(value: u64) -> Result<i64, IdempotencyError> {
    i64::try_from(value).map_err(|_| IdempotencyError::IntegerOverflow(value))
}

fn id_from_bytes<T>(bytes: [u8; 16]) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
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
    text.parse()
}
