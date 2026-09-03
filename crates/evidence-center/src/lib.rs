//! `P2-X7` — the evidence and correction centre.
//!
//! Section 25.13 of the authoritative specification calls this *`OS의 신뢰를
//! 만드는 핵심 화면`* and names six things it holds. [`CenterSection::ALL`] is
//! that list, and `the_six_sections_are_section_25_13s_own` reads the six
//! bullets out of the specification and requires each arm's words to be in its
//! own bullet, so this enumeration cannot drift from the document.
//!
//! # What this is not evidence for
//!
//! **No window opens.** `P2-X1` merged with no Tauri runtime linked, and the
//! decision to link one is still open. This crate is the content behind the
//! `Evidence & Settings` branch of the section 25.1 tree; it is a set of typed
//! records and the rules that hold between them, and nothing here observes a
//! rendered pixel. Every claim below is checked by compiling this crate,
//! running its tests, or reading its source — none of them needs a window, and
//! none of them is evidence that one exists.
//!
//! **Nothing persists.** There is no `academic-store` edge, this task claims no
//! migration number, and every value in every test is synthetic and built in
//! process, as `CONTRIBUTING.md` requires.
//!
//! # The six sections
//!
//! | Section | What holds it | The rule that makes it non-trivial |
//! |---|---|---|
//! | AI proposal inbox | [`ProposalInbox`] | four classes are four payload types, not a tag |
//! | official source change | [`SourceChangeLog`] | the rules and plans are `P2-U6`'s own answers |
//! | unresolved conflict | [`ConflictBoard`] | both sides shown; three choices offered; nothing settles it but a user |
//! | low-confidence queue | [`LowConfidenceQueue`] | three span kinds, each with a locator back to its source |
//! | permission and consent expiry | [`PermissionQueue`] | a lapsed permission blocks its dependents by failing to produce a value |
//! | transmission log | [`TransmissionLog`] | six fields, and no type in this crate can hold a payload byte |
//!
//! Correction markers are not a seventh section. Section 34.6 makes a
//! correction something that appears *on the screen it corrects*, so
//! [`CorrectionLedger`] is a surface every historical view reads rather than an
//! item in the centre's own list.

#![doc(test(attr(deny(warnings))))]

mod conflict;
mod correction;
mod error;
mod inbox;
mod low_confidence;
mod permission;
mod source_change;
mod transmission;

pub use conflict::{
    ConflictBoard, ConflictCase, ConflictClass, ConflictLane, ConflictSide, CorrectionChoice,
    CorrectionOutcome, CorrectionRecord, Resolution, user_receipt,
};
pub use correction::{
    CorrectionLedger, CorrectionMarker, CorrectionOrigin, HistoricalView, UsedClaim,
};
pub use error::CenterError;
pub use inbox::{
    ConceptMergeProposal, FindingClassification, InboxEntry, ProjectClassificationProposal,
    ProposalClass, ProposalHeader, ProposalInbox, RelationProposal, StateUpdateProposal,
};
pub use low_confidence::{
    DocumentRegionLocator, LowConfidenceQueue, LowConfidenceSpan, SpanKind, TranscriptLocator,
};
pub use permission::{
    DependentAction, DependentActionKind, ExpiringPermission, LivePermission, PermissionKind,
    PermissionQueue, PermissionRef,
};
pub use source_change::{SourceChangeEntry, SourceChangeLog};
pub use transmission::{
    DeletionReceiptRef, ObjectRange, ProviderRef, ProviderSurface, ReceiptState, TransmissionLog,
    TransmissionPurpose, TransmissionRecord,
};

/// The six things section 25.13 says this screen holds.
///
/// The order is the specification's reading order. [`Self::spec_words`] holds
/// each bullet's own words, so a section renamed here fails against the
/// document rather than against a second list written beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CenterSection {
    /// `AI 제안 inbox: relation, concept merge, project classification, state update.`
    ProposalInbox,
    /// `official source change: 영향받는 rule/plan.`
    OfficialSourceChange,
    /// `unresolved conflict: user override vs new evidence, code vs spec.`
    UnresolvedConflict,
    /// `low-confidence transcript/math/code.`
    LowConfidence,
    /// `permission/consent expiry.`
    PermissionExpiry,
    /// `provider transmission log와 deletion receipt.`
    TransmissionLog,
}

impl CenterSection {
    /// Exhaustive listing, in section 25.13's own reading order.
    pub const ALL: [Self; 6] = [
        Self::ProposalInbox,
        Self::OfficialSourceChange,
        Self::UnresolvedConflict,
        Self::LowConfidence,
        Self::PermissionExpiry,
        Self::TransmissionLog,
    ];

    /// The specification's own words for this section.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::ProposalInbox => "AI 제안 inbox",
            Self::OfficialSourceChange => "official source change",
            Self::UnresolvedConflict => "unresolved conflict",
            Self::LowConfidence => "low-confidence transcript/math/code",
            Self::PermissionExpiry => "permission/consent expiry",
            Self::TransmissionLog => "provider transmission log와 deletion receipt",
        }
    }
}

/// One thing a reader can open from the centre's index.
///
/// Every arm is a typed reference to a record the centre holds. There is no
/// arm carrying a free-form target, so an index entry cannot point at something
/// the centre does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterItem {
    /// A proposal in the inbox, by identity and class.
    Proposal(academic_proposal::ProposalId, ProposalClass),
    /// An official-source change, by the content digest that arrived.
    SourceChange(academic_domain::ContentDigest),
    /// A conflict, by class and by the claim its held side names.
    Conflict(ConflictClass, academic_domain::ClaimId),
    /// A low-confidence span, by kind and by the session it reaches back to.
    LowConfidenceSpan(SpanKind, academic_domain::LectureSessionId),
    /// A permission with an expiry.
    Permission(PermissionRef),
    /// A transmission, by the egress decision it belongs to.
    Transmission(academic_domain::EgressDecisionId),
    /// A deletion receipt, by the egress decision it deletes.
    DeletionReceipt(academic_domain::EgressDecisionId),
}

/// One section of the index, with everything it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionIndex {
    section: CenterSection,
    items: Vec<CenterItem>,
}

impl SectionIndex {
    /// Which section.
    #[must_use]
    pub const fn section(&self) -> CenterSection {
        self.section
    }

    /// Everything a reader can open from it.
    #[must_use]
    pub fn items(&self) -> &[CenterItem] {
        &self.items
    }
}

/// The evidence and correction centre.
///
/// One value holding all six sections, because section 25.13 is one screen.
/// Splitting it into six would let a record exist in a subsystem and be absent
/// from the centre without anything noticing, which is the failure this screen
/// is for.
#[derive(Debug, Clone, Default)]
pub struct EvidenceCenter {
    inbox: ProposalInbox,
    source_changes: SourceChangeLog,
    conflicts: ConflictBoard,
    low_confidence: LowConfidenceQueue,
    permissions: PermissionQueue,
    transmissions: TransmissionLog,
    corrections: CorrectionLedger,
}

impl EvidenceCenter {
    /// An empty centre.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inbox: ProposalInbox::new(),
            source_changes: SourceChangeLog::new(),
            conflicts: ConflictBoard::new(),
            low_confidence: LowConfidenceQueue::new(),
            permissions: PermissionQueue::new(),
            transmissions: TransmissionLog::new(),
            corrections: CorrectionLedger::new(),
        }
    }

    /// The proposal inbox.
    #[must_use]
    pub const fn inbox(&self) -> &ProposalInbox {
        &self.inbox
    }

    /// The proposal inbox, for admission.
    pub const fn inbox_mut(&mut self) -> &mut ProposalInbox {
        &mut self.inbox
    }

    /// The official-source change log.
    #[must_use]
    pub const fn source_changes(&self) -> &SourceChangeLog {
        &self.source_changes
    }

    /// The official-source change log, for recording.
    pub const fn source_changes_mut(&mut self) -> &mut SourceChangeLog {
        &mut self.source_changes
    }

    /// The conflict board.
    #[must_use]
    pub const fn conflicts(&self) -> &ConflictBoard {
        &self.conflicts
    }

    /// The conflict board, for opening and settling.
    pub const fn conflicts_mut(&mut self) -> &mut ConflictBoard {
        &mut self.conflicts
    }

    /// The low-confidence queue.
    #[must_use]
    pub const fn low_confidence(&self) -> &LowConfidenceQueue {
        &self.low_confidence
    }

    /// The low-confidence queue, for queueing.
    pub const fn low_confidence_mut(&mut self) -> &mut LowConfidenceQueue {
        &mut self.low_confidence
    }

    /// The permission and consent expiry queue.
    #[must_use]
    pub const fn permissions(&self) -> &PermissionQueue {
        &self.permissions
    }

    /// The permission and consent expiry queue, for recording.
    pub const fn permissions_mut(&mut self) -> &mut PermissionQueue {
        &mut self.permissions
    }

    /// The transmission log.
    #[must_use]
    pub const fn transmissions(&self) -> &TransmissionLog {
        &self.transmissions
    }

    /// The transmission log, for recording.
    pub const fn transmissions_mut(&mut self) -> &mut TransmissionLog {
        &mut self.transmissions
    }

    /// The correction ledger every historical view is read through.
    #[must_use]
    pub const fn corrections(&self) -> &CorrectionLedger {
        &self.corrections
    }

    /// The correction ledger, for recording.
    pub const fn corrections_mut(&mut self) -> &mut CorrectionLedger {
        &mut self.corrections
    }

    /// The whole index, one entry per section, each holding everything a reader
    /// can open from it.
    ///
    /// The match is total over [`CenterSection`], so a seventh section stops
    /// this crate compiling until it says what it holds. A section with nothing
    /// in it still appears, with an empty item list: a section that vanished
    /// when it was empty would make "there is nothing to review" and "this
    /// screen has no such section" the same thing on the screen.
    #[must_use]
    pub fn index(&self) -> Vec<SectionIndex> {
        CenterSection::ALL
            .into_iter()
            .map(|section| SectionIndex {
                section,
                items: self.items_of(section),
            })
            .collect()
    }

    /// Everything one section holds.
    #[must_use]
    pub fn items_of(&self, section: CenterSection) -> Vec<CenterItem> {
        match section {
            CenterSection::ProposalInbox => self
                .inbox
                .entries()
                .iter()
                .map(|entry| CenterItem::Proposal(entry.header().id(), entry.class()))
                .collect(),
            CenterSection::OfficialSourceChange => self
                .source_changes
                .entries()
                .iter()
                .map(|entry| CenterItem::SourceChange(entry.current_content()))
                .collect(),
            CenterSection::UnresolvedConflict => self
                .conflicts
                .cases()
                .iter()
                .map(|case| CenterItem::Conflict(case.class(), case.both_sides().0.claim()))
                .collect(),
            CenterSection::LowConfidence => self
                .low_confidence
                .spans()
                .iter()
                .map(|span| CenterItem::LowConfidenceSpan(span.kind(), span.session()))
                .collect(),
            CenterSection::PermissionExpiry => self
                .permissions
                .permissions()
                .iter()
                .map(|permission| CenterItem::Permission(permission.reference()))
                .collect(),
            CenterSection::TransmissionLog => self
                .transmissions
                .records()
                .iter()
                .map(|record| CenterItem::Transmission(record.decision()))
                .chain(
                    self.transmissions
                        .deletion_receipts()
                        .into_iter()
                        .map(|(decision, _)| CenterItem::DeletionReceipt(decision)),
                )
                .collect(),
        }
    }
}
