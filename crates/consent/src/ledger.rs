//! The append-only consent ledger and the recheck queue.
//!
//! # Append-only in the same sense as the attempt history
//!
//! [`ConsentLedger`] has one push and no removal. A correction is a new
//! [`PermissionRecord`] at the next `permission_seq`, which is why section 3.7
//! keys the aggregate on `(offering_id, permission_seq)` rather than on the
//! offering alone, and why [`permission_for`](ConsentLedger::permission_for)
//! resolves to the highest sequence rather than to the only one.
//!
//! # The event kinds
//!
//! [`ConsentEventKind`] is the closed vocabulary this ledger records with. Two
//! of the arms are the whole point of the type separation in
//! [`crate::evidence`]: [`EvidenceRecorded`](ConsentEventKind::EvidenceRecorded)
//! is what filing an attestation produces, and it is a different arm from
//! [`PermissionGranted`](ConsentEventKind::PermissionGranted), which only a
//! written authority reaches. Filing evidence appends an entry and changes no
//! status, which is `oral_attestation_cannot_create_permission` stated as a
//! property of the ledger rather than only of the types.
//!
//! # The recheck queue is a consequence, not a schedule
//!
//! There is no timer here. A scope reaches [`RecheckItem`] because a capture
//! was refused for it with a status of `UNKNOWN` or `EXPIRED` -- the two states
//! a user can clear by confirming the offering again. `PROHIBITED` does not
//! queue: an authority answered, and asking again is the user's decision to
//! make, not the system's to prompt for.

use academic_domain::{ContentDigest, LectureSessionId, OfferingId};

use crate::{
    ConsentError,
    capability::{BoundPermission, CaptureDenial, CaptureRequest},
    evidence::{AttestationRecord, EvidenceArtifact},
    external::ExternalReviewTask,
    permission::{PermissionRecord, TermKey},
    status::{CaptureStatus, status_of},
};

/// The closed set of things this ledger records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ConsentEventKind {
    /// A written artifact was filed as evidence. Changes no status.
    EvidenceRecorded,
    /// A user filed their own account of events. Changes no status.
    AttestationRecorded,
    /// A written authority granted.
    PermissionGranted,
    /// A written authority refused.
    PermissionProhibited,
    /// A legal question was referred outside this system.
    ExternalReviewOpened,
    /// A scope was put back in the recheck queue.
    RecheckQueued,
    /// A capture capability was minted.
    CaptureCapabilityMinted,
    /// A capture capability was refused.
    CaptureCapabilityDenied,
    /// A deletion impact was previewed.
    ExpiryPreviewed,
    /// A previewed deletion was applied.
    ExpiryApplied,
}

impl ConsentEventKind {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceRecorded => "EVIDENCE_RECORDED",
            Self::AttestationRecorded => "ATTESTATION_RECORDED",
            Self::PermissionGranted => "PERMISSION_GRANTED",
            Self::PermissionProhibited => "PERMISSION_PROHIBITED",
            Self::ExternalReviewOpened => "EXTERNAL_REVIEW_OPENED",
            Self::RecheckQueued => "RECHECK_QUEUED",
            Self::CaptureCapabilityMinted => "CAPTURE_CAPABILITY_MINTED",
            Self::CaptureCapabilityDenied => "CAPTURE_CAPABILITY_DENIED",
            Self::ExpiryPreviewed => "EXPIRY_PREVIEWED",
            Self::ExpiryApplied => "EXPIRY_APPLIED",
        }
    }
}

/// One appended entry.
///
/// It carries identifiers, a digest, and a time. It carries no bytes of
/// anything it describes, for the reason [`crate::evidence`] gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    kind: ConsentEventKind,
    offering_id: Option<OfferingId>,
    term: Option<TermKey>,
    subject_digest: Option<ContentDigest>,
    status: CaptureStatus,
    recorded_at: u64,
}

impl LedgerEntry {
    /// What happened.
    #[must_use]
    pub const fn kind(&self) -> ConsentEventKind {
        self.kind
    }

    /// Which offering it happened to, when there is one.
    #[must_use]
    pub const fn offering_id(&self) -> Option<OfferingId> {
        self.offering_id
    }

    /// Which term, when there is one.
    #[must_use]
    pub const fn term(&self) -> Option<TermKey> {
        self.term
    }

    /// The digest of whatever the entry is about.
    #[must_use]
    pub const fn subject_digest(&self) -> Option<&ContentDigest> {
        self.subject_digest.as_ref()
    }

    /// The status at the moment the entry was appended.
    #[must_use]
    pub const fn status(&self) -> CaptureStatus {
        self.status
    }

    /// When it was appended.
    #[must_use]
    pub const fn recorded_at(&self) -> u64 {
        self.recorded_at
    }
}

/// One scope the user has to confirm again before a capture is possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecheckItem {
    offering_id: OfferingId,
    term: TermKey,
    status: CaptureStatus,
    queued_at: u64,
}

impl RecheckItem {
    /// Which offering needs confirming.
    #[must_use]
    pub const fn offering_id(&self) -> OfferingId {
        self.offering_id
    }

    /// For which term.
    #[must_use]
    pub const fn term(&self) -> TermKey {
        self.term
    }

    /// The status that put it here.
    #[must_use]
    pub const fn status(&self) -> CaptureStatus {
        self.status
    }

    /// When it was queued.
    #[must_use]
    pub const fn queued_at(&self) -> u64 {
        self.queued_at
    }
}

/// The append-only record of everything consent-shaped that has happened.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConsentLedger {
    entries: Vec<LedgerEntry>,
    records: Vec<PermissionRecord>,
    rechecks: Vec<RecheckItem>,
}

impl ConsentLedger {
    /// A ledger with nothing in it.
    ///
    /// This is what a new profile has, and it is the whole of
    /// `new_offering_permission_defaults_unknown`: there is no seeded row, no
    /// template, and no permissive base to override.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            records: Vec::new(),
            rechecks: Vec::new(),
        }
    }

    /// Every entry, in append order.
    #[must_use]
    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Every recorded permission, in append order.
    #[must_use]
    pub fn records(&self) -> &[PermissionRecord] {
        &self.records
    }

    /// The scopes waiting to be confirmed again.
    #[must_use]
    pub fn rechecks(&self) -> &[RecheckItem] {
        &self.rechecks
    }

    /// Files a written artifact as evidence.
    ///
    /// Returns the entry's digest. It appends and returns nothing that could be
    /// mistaken for a permission.
    pub fn record_evidence(
        &mut self,
        offering_id: OfferingId,
        term: TermKey,
        artifact: &EvidenceArtifact,
        at: u64,
    ) -> ContentDigest {
        let digest = *artifact.digest();
        self.append(LedgerEntry {
            kind: ConsentEventKind::EvidenceRecorded,
            offering_id: Some(offering_id),
            term: Some(term),
            subject_digest: Some(digest),
            status: self.status(offering_id, term, at),
            recorded_at: at,
        });
        digest
    }

    /// Files a user's own account of events.
    ///
    /// Section 3.7: an attestation is an evidence kind and never a status
    /// transition. This method appends one entry and touches neither
    /// [`records`](Self::records) nor [`rechecks`](Self::rechecks), so the
    /// status it reports afterwards is the status it reported before.
    pub fn record_attestation(
        &mut self,
        offering_id: OfferingId,
        term: TermKey,
        attestation: &AttestationRecord,
        at: u64,
    ) -> ContentDigest {
        let digest = *attestation.conditions_digest();
        self.append(LedgerEntry {
            kind: ConsentEventKind::AttestationRecorded,
            offering_id: Some(offering_id),
            term: Some(term),
            subject_digest: Some(digest),
            status: self.status(offering_id, term, at),
            recorded_at: at,
        });
        digest
    }

    /// Appends what a written authority said about one scope.
    ///
    /// The argument is a whole [`PermissionRecord`], which cannot be built
    /// without a [`Disposition`](crate::permission::Disposition), which cannot
    /// be built without either a written grant or a written refusal. There is
    /// no overload taking an attestation.
    pub fn record_permission(
        &mut self,
        record: PermissionRecord,
        at: u64,
    ) -> Result<(), ConsentError> {
        let scope = *record.scope();
        if self.records.iter().any(|existing| {
            existing.permission_seq() == record.permission_seq()
                && existing.scope().offering_id() == scope.offering_id()
                && existing.scope().term() == scope.term()
        }) {
            return Err(ConsentError::ScopeAlreadyRecorded);
        }
        let kind = if record.grant().is_some() {
            ConsentEventKind::PermissionGranted
        } else {
            ConsentEventKind::PermissionProhibited
        };
        let digest = *record.verification_source_digest();
        let status = status_of(&record, at);
        self.records.push(record);
        self.append(LedgerEntry {
            kind,
            offering_id: Some(scope.offering_id()),
            term: Some(scope.term()),
            subject_digest: Some(digest),
            status,
            recorded_at: at,
        });
        Ok(())
    }

    /// The highest-sequence record answering one offering, term, and session.
    #[must_use]
    pub fn permission_for(
        &self,
        offering_id: OfferingId,
        term: TermKey,
        lecture_id: LectureSessionId,
    ) -> Option<&PermissionRecord> {
        self.records
            .iter()
            .filter(|record| record.scope().answers(offering_id, term, lecture_id))
            .max_by_key(|record| record.permission_seq())
    }

    /// The section 3.7 status of one scope at one instant.
    ///
    /// A scope with no record is `UNKNOWN`. That is not a branch this method
    /// takes on the way to a default; it is what having nothing to hand
    /// [`status_of`] means.
    #[must_use]
    pub fn status(&self, offering_id: OfferingId, term: TermKey, at: u64) -> CaptureStatus {
        self.records
            .iter()
            .filter(|record| {
                record.scope().offering_id() == offering_id && record.scope().term() == term
            })
            .max_by_key(|record| record.permission_seq())
            .map_or(CaptureStatus::Unknown, |record| status_of(record, at))
    }

    /// Records that a legal question was referred outside this system.
    ///
    /// The task is the return value of
    /// [`open_external_review`](crate::external::open_external_review) and this
    /// method takes it by reference and reads two enum fields off it. Nothing
    /// here reads a conclusion, because the type has none.
    pub fn record_external_review(&mut self, task: &ExternalReviewTask, at: u64) {
        self.append(LedgerEntry {
            kind: ConsentEventKind::ExternalReviewOpened,
            offering_id: Some(task.offering_id()),
            term: Some(task.term()),
            subject_digest: None,
            status: self.status(task.offering_id(), task.term(), at),
            recorded_at: at,
        });
    }

    /// Appends the allow row for a minted capability.
    pub(crate) fn record_capture_mint(
        &mut self,
        bound: &BoundPermission,
        token_id: &ContentDigest,
        at: u64,
    ) {
        let record = self
            .records
            .iter()
            .find(|record| record.permission_id() == bound.permission_id());
        let (offering_id, term) = record.map_or((None, None), |record| {
            (
                Some(record.scope().offering_id()),
                Some(record.scope().term()),
            )
        });
        self.append(LedgerEntry {
            kind: ConsentEventKind::CaptureCapabilityMinted,
            offering_id,
            term,
            subject_digest: Some(*token_id),
            status: bound.status(),
            recorded_at: at,
        });
    }

    /// Appends the deny row, queues a recheck when the status is one a user can
    /// clear, and hands the denial back unchanged.
    ///
    /// It returns its argument so a caller cannot append the row and then
    /// return a different refusal, and so no early return in
    /// [`mint_capture_capability`](crate::mint_capture_capability) or
    /// [`continue_capture`](crate::capability::continue_capture) can skip the
    /// row on its way out.
    pub(crate) fn record_capture_denial(
        &mut self,
        request: &CaptureRequest,
        denial: CaptureDenial,
        at: u64,
    ) -> CaptureDenial {
        self.append(LedgerEntry {
            kind: ConsentEventKind::CaptureCapabilityDenied,
            offering_id: request.offering_id,
            term: request.term,
            subject_digest: None,
            status: denial.status(),
            recorded_at: at,
        });
        if let (Some(offering_id), Some(term)) = (request.offering_id, request.term)
            && denial.queues_recheck()
        {
            self.queue_recheck(offering_id, term, denial.status(), at);
        }
        denial
    }

    /// Appends an expiry entry.
    pub(crate) fn record_expiry(
        &mut self,
        kind: ConsentEventKind,
        offering_id: OfferingId,
        term: TermKey,
        digest: ContentDigest,
        at: u64,
    ) {
        self.append(LedgerEntry {
            kind,
            offering_id: Some(offering_id),
            term: Some(term),
            subject_digest: Some(digest),
            status: self.status(offering_id, term, at),
            recorded_at: at,
        });
    }

    /// Puts one scope in the recheck queue, once.
    fn queue_recheck(
        &mut self,
        offering_id: OfferingId,
        term: TermKey,
        status: CaptureStatus,
        at: u64,
    ) {
        if self
            .rechecks
            .iter()
            .any(|item| item.offering_id == offering_id && item.term == term)
        {
            return;
        }
        self.rechecks.push(RecheckItem {
            offering_id,
            term,
            status,
            queued_at: at,
        });
        self.append(LedgerEntry {
            kind: ConsentEventKind::RecheckQueued,
            offering_id: Some(offering_id),
            term: Some(term),
            subject_digest: None,
            status,
            recorded_at: at,
        });
    }

    /// The one mutator. There is no removal path and no in-place edit.
    fn append(&mut self, entry: LedgerEntry) {
        self.entries.push(entry);
    }
}
