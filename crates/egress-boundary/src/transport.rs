//! The outbound seam, and the journal that survives a kill across it.
//!
//! [`OutboundTransport`] is the only trait in this workspace whose method hands
//! bytes to something outside the process. Nothing in this crate implements it:
//! a transport is supplied by the caller, and `only_egress_crate_has_a_socket`
//! refuses a socket outside the two egress crates. Today no implementation
//! ships, and the guard is what keeps that exception by crate rather than
//! global.
//!
//! Two functions carry the byte path and both are pinned as whole text by
//! `preview_bytes_equal_transmitted_bytes`. [`staged_runtime_call`] is the only
//! place a payload argument is built, and it reads
//! `staged.preview().bytes()`. [`write_authorized_bytes`] is the only place a
//! transport is written to, and it reads `authorized.payload()` — the buffer
//! the broker has just verified against the grant's payload digest. Neither
//! recomputes anything, so there is no second derivation to drift from the
//! first.

use academic_policy::{AuthorizedToolCall, BrokerError, ProcessClass, ReasonCode, RuntimeToolCall};

use crate::stage::{EgressDenial, StagedPayload};

/// A transport that could not write.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// The destination refused or dropped the write.
    #[error("transport write failed after {sent} bytes: {detail}")]
    WriteFailed {
        /// Bytes handed to the transport before it failed.
        sent: usize,
        /// What the transport reported.
        detail: String,
    },
    /// The grant's expiry passed while the transfer was in flight.
    #[error("grant expired mid-transfer after {sent} bytes")]
    GrantExpiredMidTransfer {
        /// Bytes handed to the transport before the abort.
        sent: usize,
    },
}

impl TransportError {
    /// Bytes handed to the transport before it stopped.
    #[must_use]
    pub const fn sent(&self) -> usize {
        match self {
            Self::WriteFailed { sent, .. } | Self::GrantExpiredMidTransfer { sent } => *sent,
        }
    }
}

/// The sole outbound seam.
pub trait OutboundTransport {
    /// Writes one chunk of an already-authorized staged payload.
    fn send_chunk(&mut self, chunk: &[u8]) -> Result<(), TransportError>;
}

/// The tuple fields a runtime call needs beyond the staged bytes.
#[derive(Debug, Clone)]
pub struct TransmissionPlan<'a> {
    /// Grant identifier from the broker's decision receipt.
    pub grant_id: &'a str,
    /// Actor identity, matching the capability.
    pub actor_id: &'a str,
    /// Typed process boundary. `P2-G7` admits only `EgressProxy` for an
    /// outbound socket capability, and `RuntimeToolCall::new` refuses any
    /// other, so a caller cannot build this call from the wrong process.
    pub process_class: ProcessClass,
    /// Operation, matching the capability.
    pub operation: &'a str,
    /// Purpose, matching the capability.
    pub purpose_id: &'a str,
    /// Destination, matching the capability.
    pub destination_id: &'a str,
    /// Exclusive expiry the transfer must finish before.
    pub expires_at: u64,
    /// Bytes per transport write.
    pub chunk_bytes: usize,
}

/// What one completed transmission handed to the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transmission {
    bytes_sent: usize,
    payload_digest: String,
}

impl Transmission {
    /// Bytes written to the transport.
    #[must_use]
    pub const fn bytes_sent(&self) -> usize {
        self.bytes_sent
    }

    /// Digest of the bytes written, which is the preview's digest.
    #[must_use]
    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }
}

/// One append-only journal record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalEntry {
    /// Written before the capability boundary is crossed.
    SendIntent {
        /// Grant this transfer consumes.
        grant_id: String,
        /// Staged artifact identifier.
        staged_object_id: String,
        /// Digest of the staged bytes.
        payload_digest: String,
        /// Exact staged byte count.
        byte_count: usize,
        /// Destination the bytes go to.
        destination_id: String,
        /// When the intent was recorded.
        at: u64,
    },
    /// Written after the transport returns, whether it finished or aborted.
    SendOutcome {
        /// Grant this transfer consumed.
        grant_id: String,
        /// Bytes actually handed to the transport.
        bytes_sent: usize,
        /// Whether every staged byte was written.
        complete: bool,
        /// When the outcome was recorded.
        at: u64,
    },
}

/// An intent with no outcome: what a kill after the send leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconstructedAudit {
    /// Grant the interrupted transfer consumed.
    pub grant_id: String,
    /// Staged artifact identifier.
    pub staged_object_id: String,
    /// Digest of the staged bytes.
    pub payload_digest: String,
    /// Exact staged byte count.
    pub byte_count: usize,
    /// Destination the bytes went to.
    pub destination_id: String,
    /// When the intent was recorded.
    pub at: u64,
}

/// Append-only record of every transfer this process began.
///
/// It holds identifiers, digests, and counts. It holds no payload byte, for the
/// same reason `egress_audit` does not: a journal that recorded what it was
/// protecting would be the leak.
#[derive(Debug, Clone, Default)]
pub struct StagedGrantJournal {
    entries: Vec<JournalEntry>,
}

impl StagedGrantJournal {
    /// An empty journal.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Every record, in append order.
    #[must_use]
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    pub(crate) fn append(&mut self, entry: JournalEntry) {
        self.entries.push(entry);
    }

    /// Intents with no outcome, in append order.
    ///
    /// A kill between the provider send and the outcome record leaves exactly
    /// this. The broker's allow audit and its `consumed_at` are already
    /// committed at that point, so what the journal adds is the transfer's own
    /// unresolved state, and a replay of the grant is refused as consumed.
    #[must_use]
    pub fn reconstruct(&self) -> Vec<ReconstructedAudit> {
        let resolved: Vec<&str> = self
            .entries
            .iter()
            .filter_map(|entry| match entry {
                JournalEntry::SendOutcome { grant_id, .. } => Some(grant_id.as_str()),
                JournalEntry::SendIntent { .. } => None,
            })
            .collect();
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                JournalEntry::SendIntent {
                    grant_id,
                    staged_object_id,
                    payload_digest,
                    byte_count,
                    destination_id,
                    at,
                } if !resolved.contains(&grant_id.as_str()) => Some(ReconstructedAudit {
                    grant_id: grant_id.clone(),
                    staged_object_id: staged_object_id.clone(),
                    payload_digest: payload_digest.clone(),
                    byte_count: *byte_count,
                    destination_id: destination_id.clone(),
                    at: *at,
                }),
                _ => None,
            })
            .collect()
    }
}

/// Builds the runtime call. The payload argument is read from the preview.
pub(crate) fn staged_runtime_call<'a>(
    staged: &'a StagedPayload,
    plan: &TransmissionPlan<'_>,
) -> Result<RuntimeToolCall<'a>, BrokerError> {
    RuntimeToolCall::new(
        plan.actor_id,
        plan.process_class,
        plan.operation,
        plan.purpose_id,
        plan.destination_id,
        vec![staged.object_range()?],
        staged.preview().bytes(),
    )
}

/// Writes the authorized bytes, in chunks, refusing to continue past expiry.
pub(crate) fn write_authorized_bytes<T: OutboundTransport>(
    authorized: &AuthorizedToolCall<'_>,
    transport: &mut T,
    chunk_bytes: usize,
    now: &dyn Fn() -> u64,
    expires_at: u64,
) -> Result<usize, TransportError> {
    let bytes = authorized.payload();
    let mut sent = 0_usize;
    for chunk in bytes.chunks(chunk_bytes.max(1)) {
        if now() >= expires_at {
            return Err(TransportError::GrantExpiredMidTransfer { sent });
        }
        transport.send_chunk(chunk)?;
        sent = sent.saturating_add(chunk.len());
    }
    Ok(sent)
}

pub(crate) fn transport_denial(error: &TransportError) -> EgressDenial {
    match error {
        TransportError::GrantExpiredMidTransfer { sent } => EgressDenial::aborted(
            ReasonCode::GrantExpired,
            format!("grant expired mid-transfer after {sent} bytes"),
            *sent,
        ),
        TransportError::WriteFailed { sent, detail } => EgressDenial::aborted(
            ReasonCode::ScopeMismatch,
            format!("transport refused the write after {sent} bytes: {detail}"),
            *sent,
        ),
    }
}

pub(crate) fn transmission(bytes_sent: usize, payload_digest: String) -> Transmission {
    Transmission {
        bytes_sent,
        payload_digest,
    }
}
