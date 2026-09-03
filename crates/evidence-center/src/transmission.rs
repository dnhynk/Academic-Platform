//! The provider transmission log and its deletion receipts.
//!
//! Section 25.13's sixth bullet is *`provider transmission log와 deletion
//! receipt`*. The execution plan fixes exactly what a row exposes: *purpose,
//! payload digest, ranges, provider, time, and receipt, without any payload
//! bytes*. [`TransmissionRecord`] has those six accessors and no seventh.
//!
//! # Why there is no payload byte here, and what says so
//!
//! Four layers, each blind to a different bypass. The order matters: the first
//! is the strongest and the last is the weakest.
//!
//! 1. **The closure.** `academic-egress-boundary` and `academic-policy` are
//!    absent from this crate's dependency closure at every feature setting, so
//!    `StagedPayload`, `Preview` and the broker's own rows are not nameable in
//!    this file. The type that owns the transmitted bytes cannot be written
//!    down here.
//! 2. **The field types.** `the_center_cannot_name_a_payload_byte` collects
//!    every named field of every struct and enum in this crate's product
//!    source, reads each field's *declared type*, and compares the whole set of
//!    declared types against a reviewed allowlist in both directions. A field
//!    holding `Vec<u8>` fails as an unreviewed type whatever it is named — the
//!    exact shape that passed `tools/secret-debug-policy.test.mjs` under the
//!    name `excerpt`, because that tool matches field *names*. The crate also
//!    declares no tuple struct, because a tuple field has no name to inventory.
//! 3. **The public surface.** The same scan reads every public signature in the
//!    crate and requires none of them to mention a byte-capable type, so bytes
//!    cannot cross the boundary in either direction even in a value nothing
//!    stores.
//! 4. **The spellings.** A short forbidden-token list, kept as the explicitly
//!    weakest layer, because a list is broken by the spelling nobody predicted.
//!
//! `Debug` is derived on every type here. That is safe *because of* layer two
//! rather than despite it: a derive prints fields, and no field of this crate
//! can hold a byte of a payload. `P2-G7`'s leak went the other way — an audit
//! side table grew a `transmitted_bytes` field, the canary guard was a token
//! list, and the derive printed it.

use academic_domain::{ContentDigest, EgressDecisionId, TimestampMillis};

/// Why a payload was sent.
///
/// Closed, because a purpose a caller can spell freely is a purpose nobody
/// reviewed. Each arm is a use section 32.6 or section 25.9 already names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransmissionPurpose {
    /// Section 25.9's `Analyze`: a read-only repository analysis.
    RepositoryAnalysis,
    /// Section 34.1's `multi-pass/provider comparison` for a transcript.
    TranscriptComparison,
    /// Section 29.1's stage seven: a proposal candidate from a source document.
    ProposalExtraction,
    /// Section 25.7's equation and code reconstruction from a source image.
    EquationOrCodeReconstruction,
}

impl TransmissionPurpose {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::RepositoryAnalysis,
        Self::TranscriptComparison,
        Self::ProposalExtraction,
        Self::EquationOrCodeReconstruction,
    ];
}

/// Which contract surface a provider was reached through.
///
/// Section 32.6 keeps *`enterprise/API와 consumer UI 정책 차이`* as a versioned
/// fact, so the surface is part of the identity a log row names rather than a
/// detail of the vendor. The two tokens are `academic-policy`'s own, and
/// `the_provider_surface_is_the_brokers_own` compares them against that crate
/// through a dev edge instead of trusting this restatement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProviderSurface {
    /// A contracted enterprise or API surface.
    EnterpriseApi,
    /// A consumer user interface.
    ConsumerUi,
}

impl ProviderSurface {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::EnterpriseApi, Self::ConsumerUi];

    /// The broker's own spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnterpriseApi => "ENTERPRISE_API",
            Self::ConsumerUi => "CONSUMER_UI",
        }
    }
}

/// Which provider a payload went to.
///
/// The vendor is named by the digest `academic-policy`'s
/// `ProviderIdentity::destination_id` derives, not by its name. That is a
/// deliberate cost and it is worth stating: this crate cannot render "which
/// provider" as words, and a surface that wants to must resolve the digest
/// against `P2-G3`'s registry. What it buys is that the centre holds no text a
/// caller supplied at all, which is what makes the field-type inventory a
/// closed set rather than a set with one exception in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderRef {
    destination: ContentDigest,
    surface: ProviderSurface,
}

impl ProviderRef {
    /// A provider reference.
    #[must_use]
    pub const fn new(destination: ContentDigest, surface: ProviderSurface) -> Self {
        Self {
            destination,
            surface,
        }
    }

    /// The broker's canonical destination digest.
    #[must_use]
    pub const fn destination(&self) -> ContentDigest {
        self.destination
    }

    /// Which contract surface.
    #[must_use]
    pub const fn surface(&self) -> ProviderSurface {
        self.surface
    }
}

/// One byte range of one object, by offset and length.
///
/// Section 25.9 requires the range to be previewed before an analysis is sent:
/// *`외부 provider로 보낼 byte 범위를 preview한다`*. What the log records is the
/// range that was authorised. It is two integers; the bytes inside it are the
/// staging pipeline's and never arrive here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRange {
    offset: u64,
    length: u64,
}

impl ObjectRange {
    /// A range.
    #[must_use]
    pub const fn new(offset: u64, length: u64) -> Self {
        Self { offset, length }
    }

    /// Where it starts.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// How long it is.
    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }
}

/// A provider deletion receipt, by digest.
///
/// Section 32.10: *`cloud provider 삭제 receipt를 가능한 경우 보존`*. The receipt
/// bytes are the provider's document and are `P2-G3`'s to hold; what the centre
/// carries is the digest of those bytes, the digest of the provider-policy
/// version the transmission happened under, and the two instants. Those are
/// `academic-policy`'s own `DeletionReceiptRow` columns less its two identifier
/// strings, and `the_receipt_reference_is_the_brokers_own_columns` compares the
/// two through a dev edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeletionReceiptRef {
    receipt_digest: ContentDigest,
    provider_policy_snapshot: ContentDigest,
    requested_at: TimestampMillis,
    received_at: TimestampMillis,
}

impl DeletionReceiptRef {
    /// A receipt reference.
    #[must_use]
    pub const fn new(
        receipt_digest: ContentDigest,
        provider_policy_snapshot: ContentDigest,
        requested_at: TimestampMillis,
        received_at: TimestampMillis,
    ) -> Self {
        Self {
            receipt_digest,
            provider_policy_snapshot,
            requested_at,
            received_at,
        }
    }

    /// The digest of the provider's receipt document.
    #[must_use]
    pub const fn receipt_digest(&self) -> ContentDigest {
        self.receipt_digest
    }

    /// The provider-policy version the transmission happened under.
    #[must_use]
    pub const fn provider_policy_snapshot(&self) -> ContentDigest {
        self.provider_policy_snapshot
    }

    /// When deletion was requested.
    #[must_use]
    pub const fn requested_at(&self) -> TimestampMillis {
        self.requested_at
    }

    /// When the receipt was observed.
    #[must_use]
    pub const fn received_at(&self) -> TimestampMillis {
        self.received_at
    }
}

/// Where a transmission stands with respect to deletion.
///
/// `NotOffered` is fault `EG07` — a provider that offers no deletion receipt.
/// It is an arm rather than an absent receipt so that "this provider will never
/// give one" and "we have not asked yet" are different states on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptState {
    /// `EG07`: the provider offers no deletion receipt.
    NotOffered,
    /// Deletion was requested and no receipt has arrived.
    Requested {
        /// When it was requested.
        requested_at: TimestampMillis,
    },
    /// A receipt arrived.
    Received(DeletionReceiptRef),
}

impl ReceiptState {
    /// The receipt, when one arrived.
    #[must_use]
    pub const fn receipt(&self) -> Option<&DeletionReceiptRef> {
        match self {
            Self::NotOffered | Self::Requested { .. } => None,
            Self::Received(receipt) => Some(receipt),
        }
    }
}

/// One row of the transmission log.
///
/// Six accessors, one per thing the contract fixes, and no seventh.
/// `transmission_log_and_deletion_receipts_are_discoverable` compares the whole
/// public method set of this type against those six plus the decision
/// identifier that links a row to `P2-G1`'s audit row, in both directions, so a
/// seventh accessor fails as an extra key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransmissionRecord {
    decision: EgressDecisionId,
    purpose: TransmissionPurpose,
    payload_digest: ContentDigest,
    ranges: Vec<ObjectRange>,
    provider: ProviderRef,
    transmitted_at: TimestampMillis,
    receipt: ReceiptState,
}

impl TransmissionRecord {
    /// One transmission, recorded.
    ///
    /// There is no argument that could carry a payload byte: the digest is a
    /// digest, the ranges are integers, and every remaining argument is an
    /// identifier or a closed enum.
    #[must_use]
    pub const fn new(
        decision: EgressDecisionId,
        purpose: TransmissionPurpose,
        payload_digest: ContentDigest,
        ranges: Vec<ObjectRange>,
        provider: ProviderRef,
        transmitted_at: TimestampMillis,
        receipt: ReceiptState,
    ) -> Self {
        Self {
            decision,
            purpose,
            payload_digest,
            ranges,
            provider,
            transmitted_at,
            receipt,
        }
    }

    /// The `P2-G1` egress decision this row belongs to.
    #[must_use]
    pub const fn decision(&self) -> EgressDecisionId {
        self.decision
    }

    /// Why it was sent.
    #[must_use]
    pub const fn purpose(&self) -> TransmissionPurpose {
        self.purpose
    }

    /// The digest of what was sent.
    #[must_use]
    pub const fn payload_digest(&self) -> ContentDigest {
        self.payload_digest
    }

    /// Which ranges of the object were sent.
    #[must_use]
    pub fn ranges(&self) -> &[ObjectRange] {
        &self.ranges
    }

    /// Which provider it went to.
    #[must_use]
    pub const fn provider(&self) -> ProviderRef {
        self.provider
    }

    /// When it was sent.
    #[must_use]
    pub const fn transmitted_at(&self) -> TimestampMillis {
        self.transmitted_at
    }

    /// Where deletion stands.
    #[must_use]
    pub const fn receipt(&self) -> ReceiptState {
        self.receipt
    }
}

/// Every recorded transmission, with its receipts.
#[derive(Debug, Clone, Default)]
pub struct TransmissionLog {
    records: Vec<TransmissionRecord>,
}

impl TransmissionLog {
    /// An empty log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Records one transmission.
    pub fn record(&mut self, record: TransmissionRecord) {
        self.records.push(record);
    }

    /// Every transmission, in recording order.
    #[must_use]
    pub fn records(&self) -> &[TransmissionRecord] {
        &self.records
    }

    /// Every deletion receipt the log holds, with the transmission it belongs
    /// to.
    ///
    /// The pair rather than the receipt alone: a receipt that does not say
    /// which transmission it deletes is not evidence of anything.
    #[must_use]
    pub fn deletion_receipts(&self) -> Vec<(EgressDecisionId, &DeletionReceiptRef)> {
        self.records
            .iter()
            .filter_map(|record| match &record.receipt {
                ReceiptState::NotOffered | ReceiptState::Requested { .. } => None,
                ReceiptState::Received(receipt) => Some((record.decision(), receipt)),
            })
            .collect()
    }

    /// Every transmission whose provider offers no deletion receipt.
    ///
    /// Fault `EG07` is `P2-G3`'s decision; what the centre owes the user is that
    /// the transmissions it applies to are findable rather than silently
    /// missing from the receipt list.
    #[must_use]
    pub fn without_offered_receipt(&self) -> Vec<&TransmissionRecord> {
        self.records
            .iter()
            .filter(|record| record.receipt() == ReceiptState::NotOffered)
            .collect()
    }
}
