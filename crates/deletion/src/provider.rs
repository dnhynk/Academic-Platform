//! Provider erasure requests, and the receipts a deletion keeps against them.
//!
//! Section 32.10: *cloud provider 삭제 receipt를 가능한 경우 보존.* Three layers
//! already exist and this module is the fourth, which is the one that was
//! missing:
//!
//! 1. `academic-policy` persists the receipt row and links it to the grant and
//!    the exact allow-audit row of the transmission it deletes.
//! 2. `academic-evidence-center` carries that row's columns, less its two
//!    identifier strings, as a `DeletionReceiptRef` and shows it beside the
//!    transmission on the correction centre's sixth section.
//! 3. `academic-policy`'s `EG07` says a provider that offers no receipt is a
//!    state, not an absence.
//! 4. **Nothing linked either of those to the artifact deletion that caused the
//!    request.** A user who deletes a lecture whose bytes were sent to a
//!    provider is owed the provider's half of that deletion, and a local
//!    `COMPLETE` that did not mention an outstanding provider copy would read
//!    as "it is gone".
//!
//! So a [`ProviderErasureLog`] is keyed by [`DeletionTarget`] — artifact and
//! locator, never locator alone — and a receipt can only be recorded against a
//! request this deletion actually made. The receipt reference itself is
//! `P2-X7`'s type and the two identifier strings stay in `P2-G3`'s row, which
//! is why nothing here can name a transmitted byte.
//!
//! # What an outstanding request does to the result
//!
//! It does **not** become a fifth result word: `P2-K5`'s vocabulary is
//! `PLANNED`, `COMPLETE`, `PARTIAL`, `REPAIR_REQUIRED` over the seven local
//! derivative classes, and a provider copy is not one of them. What it does is
//! stop [`crate::ArtifactDeletionReceipt::is_fully_erased`] from being true, and
//! it is named exactly in [`ProviderErasureLog::outstanding`]. A local
//! `COMPLETE` beside an outstanding provider erasure is an honest pair of facts;
//! collapsing them into one word would lose one of them.

use std::collections::BTreeMap;

use academic_domain::{EgressDecisionId, TimestampMillis};
use academic_evidence_center::{DeletionReceiptRef, ReceiptState};

use crate::{error::DeletionFlowError, target::DeletionTarget};

/// One provider transmission this deletion has to reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderErasureRequest {
    target: DeletionTarget,
    decision: EgressDecisionId,
    requested_at: TimestampMillis,
}

impl ProviderErasureRequest {
    /// Records that erasure of one artifact's transmission was requested.
    #[must_use]
    pub const fn new(
        target: DeletionTarget,
        decision: EgressDecisionId,
        requested_at: TimestampMillis,
    ) -> Self {
        Self {
            target,
            decision,
            requested_at,
        }
    }

    /// Which artifact's copy this is about.
    #[must_use]
    pub const fn target(&self) -> &DeletionTarget {
        &self.target
    }

    /// The `P2-G1` egress decision whose payload is being erased.
    #[must_use]
    pub const fn decision(&self) -> EgressDecisionId {
        self.decision
    }

    /// When erasure was requested.
    #[must_use]
    pub const fn requested_at(&self) -> TimestampMillis {
        self.requested_at
    }

    /// The row a report shows.
    #[must_use]
    pub fn to_row(&self) -> String {
        format!("{} -> {}", self.target.to_row(), self.decision)
    }
}

/// One provider erasure and where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderErasureEntry {
    request: ProviderErasureRequest,
    state: ReceiptState,
}

impl ProviderErasureEntry {
    /// What was requested.
    #[must_use]
    pub const fn request(&self) -> &ProviderErasureRequest {
        &self.request
    }

    /// Where the receipt stands, in `P2-X7`'s own vocabulary.
    #[must_use]
    pub const fn state(&self) -> ReceiptState {
        self.state
    }

    /// The receipt, when one arrived.
    #[must_use]
    pub const fn receipt(&self) -> Option<&DeletionReceiptRef> {
        self.state.receipt()
    }

    /// Whether the provider has confirmed erasure.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        matches!(self.state, ReceiptState::Received(_))
    }
}

/// Every provider erasure one deletion asked for, with its receipt.
///
/// Keyed by the artifact **and** the egress decision, so one artifact sent to
/// two providers is two entries, and two registrations of the same bytes sent
/// to one provider are two entries as well.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderErasureLog {
    entries: BTreeMap<(DeletionTarget, EgressDecisionId), ProviderErasureEntry>,
}

impl ProviderErasureLog {
    /// An empty log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Records an erasure request, in the state the provider's contract allows.
    ///
    /// `NotOffered` is fault `EG07` and is recorded as a request all the same:
    /// a provider that will never confirm is a fact the user is owed, and a
    /// request that was never filed because it could not succeed would be
    /// missing from the report instead.
    pub fn request(&mut self, request: ProviderErasureRequest, state: ReceiptState) {
        self.entries.insert(
            (*request.target(), request.decision()),
            ProviderErasureEntry { request, state },
        );
    }

    /// Records the receipt a provider returned.
    ///
    /// # Errors
    ///
    /// [`DeletionFlowError::ReceiptWithoutRequest`] when no request names this
    /// artifact and this decision. A receipt that arrived for something nobody
    /// asked about is not evidence that this deletion reached a provider.
    pub fn record_receipt(
        &mut self,
        target: DeletionTarget,
        decision: EgressDecisionId,
        receipt: DeletionReceiptRef,
    ) -> Result<(), DeletionFlowError> {
        let entry = self
            .entries
            .get_mut(&(target, decision))
            .ok_or(DeletionFlowError::ReceiptWithoutRequest)?;
        entry.state = ReceiptState::Received(receipt);
        Ok(())
    }

    /// Every entry, ordered by artifact and then by decision.
    #[must_use]
    pub fn entries(&self) -> Vec<&ProviderErasureEntry> {
        self.entries.values().collect()
    }

    /// The entry for one artifact at one provider decision.
    #[must_use]
    pub fn entry(
        &self,
        target: &DeletionTarget,
        decision: EgressDecisionId,
    ) -> Option<&ProviderErasureEntry> {
        self.entries.get(&(*target, decision))
    }

    /// Every request no receipt has settled, in the same order.
    ///
    /// This is the exact list, not a count: `PARTIAL` names what is left and so
    /// does this.
    #[must_use]
    pub fn outstanding(&self) -> Vec<&ProviderErasureEntry> {
        self.entries
            .values()
            .filter(|entry| !entry.is_settled())
            .collect()
    }

    /// The rendered rows of every outstanding request.
    #[must_use]
    pub fn outstanding_rows(&self) -> Vec<String> {
        self.outstanding()
            .iter()
            .map(|entry| {
                let stance = match entry.state() {
                    ReceiptState::NotOffered => "NO_RECEIPT_OFFERED",
                    ReceiptState::Requested { .. } => "AWAITING_RECEIPT",
                    ReceiptState::Received(_) => "RECEIVED",
                };
                format!("{}: {stance}", entry.request().to_row())
            })
            .collect()
    }
}
