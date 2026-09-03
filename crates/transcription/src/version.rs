//! Corrections as versions, and the lineage that keeps every one of them.
//!
//! # A correction never reaches a raw token
//!
//! [`TranscriptLineage`] owns one [`crate::RawTranscript`] and hands out
//! `&RawTranscript`. Every version after the first is the same raw transcript
//! plus a list of [`AppliedCorrection`]s, so reading a transcript at version
//! *n* is an overlay computed on the way out, and the raw token at every
//! version is the same value. `token_correction_new_version` observes the raw
//! token digest before and after, and `raw_token_write_protection` is the type
//! half of the same claim.
//!
//! # Which of the three dispositions appends a version
//!
//! `academic-domain`'s `DecisionAction` has exactly three arms and `P2-M2` is
//! where they are the queue's vocabulary. [`LineageEffect::of`] is a total
//! `match` over that closed enum, so a fourth disposition stops this crate
//! compiling until it says what it does to a lineage -- which is how "do not
//! invent a fourth" is held by the compiler rather than by this sentence.
//!
//! | `DecisionAction` | Section 3 | Effect |
//! |---|---|---|
//! | `Confirm` | 승인 | the model's candidate becomes version *n+1* |
//! | `Replace` | 수정 | the user's own text becomes version *n+1* |
//! | `Reject` | 거절 | nothing is appended, and the proposal is retained |
//!
//! [`SettledCorrection`] has exactly two producers, one per appending arm.
//! `Reject` has none, so a rejected correction is not a value that can be
//! handed to [`TranscriptLineage::append_correction`] at all.

use academic_domain::{Actor, ContentDigest, DecisionAction};
use academic_proposal::{Approved, DispositionRecord, ProposalId};

use crate::{
    annotation::AnnotationLayer,
    authorize::be_len,
    fault::VersionFault,
    transcript::{RawSegment, RawToken, RawTranscript},
};

/// Where one token sits in a transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress {
    segment: usize,
    position: usize,
}

impl TokenAddress {
    /// Names a token by segment index and position inside it.
    #[must_use]
    pub const fn new(segment: usize, position: usize) -> Self {
        Self { segment, position }
    }

    /// Which segment.
    #[must_use]
    pub const fn segment(self) -> usize {
        self.segment
    }

    /// Which token inside it.
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }
}

/// What somebody proposes one token should read instead.
///
/// This is the payload a caller wraps in `academic_proposal::Proposed<T>`. It
/// holds no raw token and no reference to one -- only an address and the text
/// the correction proposes -- so a correction that is never settled leaves the
/// transcript exactly as it was.
#[derive(Clone, PartialEq, Eq)]
pub struct CorrectionCandidate {
    address: TokenAddress,
    replacement_text: String,
}

impl CorrectionCandidate {
    /// Proposes `replacement_text` at `address`.
    ///
    /// # Errors
    ///
    /// [`VersionFault::ReplacementText`] for empty text or text holding a
    /// control character.
    pub fn proposing(
        address: TokenAddress,
        replacement_text: impl Into<String>,
    ) -> Result<Self, VersionFault> {
        let replacement_text = replacement_text.into();
        if replacement_text.is_empty() || replacement_text.chars().any(char::is_control) {
            return Err(VersionFault::ReplacementText);
        }
        Ok(Self {
            address,
            replacement_text,
        })
    }

    /// Which token it addresses.
    #[must_use]
    pub const fn address(&self) -> TokenAddress {
        self.address
    }

    /// What it proposes the token should read.
    #[must_use]
    pub fn replacement_text(&self) -> &str {
        &self.replacement_text
    }
}

// A correction carries a word from the lecture, so it is redacted for the same
// reason `RawToken` is.
impl core::fmt::Debug for CorrectionCandidate {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CorrectionCandidate")
            .field("address", &self.address)
            .field("replacement_byte_len", &self.replacement_text.len())
            .finish()
    }
}

/// What one disposition does to a lineage.
///
/// A total `match` over `academic-domain`'s closed `DecisionAction`. Section 3
/// of the specification names exactly three things a user does with an AI
/// proposal, ADR-003 froze them, and `P2-M2` is where they are already the
/// queue's vocabulary; this crate adds none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineageEffect {
    /// A new version is appended over the annotation layer.
    AppendsVersion,
    /// Nothing is appended. The proposal itself is retained by the queue.
    AppendsNothing,
}

impl LineageEffect {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::AppendsVersion, Self::AppendsNothing];

    /// What `disposition` does here.
    #[must_use]
    pub const fn of(disposition: &DecisionAction) -> Self {
        match disposition {
            DecisionAction::Confirm | DecisionAction::Replace { .. } => Self::AppendsVersion,
            DecisionAction::Reject => Self::AppendsNothing,
        }
    }

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AppendsVersion => "APPENDS_VERSION",
            Self::AppendsNothing => "APPENDS_NOTHING",
        }
    }
}

/// Who authored the text a version carries.
///
/// Two arms, one per appending disposition. It is not a fourth disposition: it
/// records which of the three that were already recorded produced the version,
/// so a reader of the lineage can tell a confirmed model candidate from the
/// user's own replacement without going back to the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CorrectionAuthor {
    /// The model proposed the text and the user confirmed that exact
    /// candidate.
    ConfirmedModelCandidate,
    /// The user rejected the candidate and supplied their own text.
    UserReplacement,
}

impl CorrectionAuthor {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::ConfirmedModelCandidate, Self::UserReplacement];

    /// Which disposition produced it.
    #[must_use]
    pub const fn disposition_token(self) -> &'static str {
        match self {
            Self::ConfirmedModelCandidate => "CONFIRM",
            Self::UserReplacement => "REPLACE",
        }
    }
}

/// A correction a user has settled, and the only thing a version is built from.
///
/// Private fields and two producers, one per appending disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledCorrection {
    proposal: ProposalId,
    candidate: CorrectionCandidate,
    author: CorrectionAuthor,
}

impl SettledCorrection {
    /// The user confirmed the model's own candidate.
    ///
    /// The argument is `academic_proposal::Approved<CorrectionCandidate>`,
    /// which `ReviewQueue::commit` produces only after a user `CONFIRM` for
    /// that exact proposal is already in its append-only history. So a
    /// confirmation nobody recorded is not a value this constructor can be
    /// handed.
    #[must_use]
    pub fn confirmed(approved: Approved<CorrectionCandidate>) -> Self {
        let proposal = approved.id();
        Self {
            proposal,
            candidate: approved.into_inner(),
            author: CorrectionAuthor::ConfirmedModelCandidate,
        }
    }

    /// The user replaced the model's candidate with their own text.
    ///
    /// ADR-003 has a replacement reject the target and select a different
    /// object, so `P2-M2`'s `commit` refuses to release the model's payload for
    /// a `Replace`. The user's own candidate is passed in instead, and the
    /// disposition record that names this proposal is what proves the
    /// replacement was recorded.
    ///
    /// # Errors
    ///
    /// [`VersionFault::NotSettled`] when the record does not carry a `Replace`
    /// disposition.
    pub fn replaced(
        proposal: ProposalId,
        record: &DispositionRecord,
        own_text: CorrectionCandidate,
    ) -> Result<Self, VersionFault> {
        if !matches!(record.disposition(), DecisionAction::Replace { .. }) {
            return Err(VersionFault::NotSettled);
        }
        Ok(Self {
            proposal,
            candidate: own_text,
            author: CorrectionAuthor::UserReplacement,
        })
    }

    /// Which proposal was settled.
    #[must_use]
    pub const fn proposal(&self) -> ProposalId {
        self.proposal
    }

    /// The text that was settled on.
    #[must_use]
    pub const fn candidate(&self) -> &CorrectionCandidate {
        &self.candidate
    }

    /// Which of the two appending dispositions produced it.
    #[must_use]
    pub const fn author(&self) -> CorrectionAuthor {
        self.author
    }
}

/// One correction as a version records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedCorrection {
    settled: SettledCorrection,
    previous_text: String,
}

impl AppliedCorrection {
    /// The settled correction.
    #[must_use]
    pub const fn settled(&self) -> &SettledCorrection {
        &self.settled
    }

    /// Which token it addresses.
    #[must_use]
    pub const fn address(&self) -> TokenAddress {
        self.settled.candidate.address
    }

    /// What the token read at the version before this one.
    #[must_use]
    pub fn previous_text(&self) -> &str {
        &self.previous_text
    }

    /// What it reads from this version on.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.settled.candidate.replacement_text
    }
}

/// One version of a transcript.
///
/// Version 1 is what the provider returned: no correction, and an empty
/// annotation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptVersion {
    number: u32,
    supersedes: Option<u32>,
    corrections: Vec<AppliedCorrection>,
    annotations: AnnotationLayer,
}

impl TranscriptVersion {
    /// Its number, from one.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// The version it supersedes, which is `None` only for version one.
    #[must_use]
    pub const fn supersedes(&self) -> Option<u32> {
        self.supersedes
    }

    /// Every correction that applies at this version, oldest first.
    #[must_use]
    pub fn corrections(&self) -> &[AppliedCorrection] {
        &self.corrections
    }

    /// The annotation layer this version reads through.
    #[must_use]
    pub const fn annotations(&self) -> &AnnotationLayer {
        &self.annotations
    }

    /// A digest over the version's own content: its number, what it
    /// supersedes, its corrections and its annotation layer.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-transcription-version-v1\0");
        material.extend_from_slice(&self.number.to_be_bytes());
        material.extend_from_slice(&self.supersedes.unwrap_or(0).to_be_bytes());
        material.extend_from_slice(&be_len(self.corrections.len()));
        for correction in &self.corrections {
            material.extend_from_slice(&be_len(correction.address().segment()));
            material.extend_from_slice(&be_len(correction.address().position()));
            material.extend_from_slice(correction.settled.author.disposition_token().as_bytes());
            material.push(0);
            material.extend_from_slice(&be_len(correction.previous_text.len()));
            material.extend_from_slice(correction.previous_text.as_bytes());
            material.extend_from_slice(&be_len(correction.text().len()));
            material.extend_from_slice(correction.text().as_bytes());
        }
        material.extend_from_slice(self.annotations.digest().as_bytes());
        ContentDigest::sha256(&material)
    }
}

/// What section 12.4 calls a segment's `correctionStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CorrectionStatus {
    /// Nothing is proposed and nothing is corrected.
    Uncorrected,
    /// A correction is open against this segment and nobody has settled it.
    NeedsReview,
    /// A settled correction applies at this version.
    Corrected,
}

impl CorrectionStatus {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::Uncorrected, Self::NeedsReview, Self::Corrected];

    /// Section 12.4's spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncorrected => "UNCORRECTED",
            Self::NeedsReview => "NEEDS_REVIEW",
            Self::Corrected => "CORRECTED",
        }
    }
}

/// One token as a version reads it, beside the raw token it came from.
///
/// Both halves are always present. A reader that wants what the provider said
/// has it, at every version, without going anywhere else.
#[derive(Clone, PartialEq, Eq)]
pub struct EffectiveToken<'a> {
    raw: &'a RawToken,
    effective_text: &'a str,
    corrected_at: Option<u32>,
}

impl<'a> EffectiveToken<'a> {
    /// The raw token, unchanged at every version.
    #[must_use]
    pub const fn raw(&self) -> &'a RawToken {
        self.raw
    }

    /// What this version reads.
    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.effective_text
    }

    /// The version that changed it, if any has.
    #[must_use]
    pub const fn corrected_at(&self) -> Option<u32> {
        self.corrected_at
    }
}

// It borrows a word of the lecture. Redacted for the reason `RawToken` is.
impl core::fmt::Debug for EffectiveToken<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EffectiveToken")
            .field("effective_byte_len", &self.effective_text.len())
            .field("corrected_at", &self.corrected_at)
            .finish()
    }
}

/// Section 12.4's record, read at one version.
///
/// It is a view rather than a stored row: the raw half is borrowed from the
/// raw transcript and the corrected half is computed from the version's
/// correction list, so there is no second copy of a token anywhere that could
/// drift from the first.
#[derive(Debug, Clone)]
pub struct TranscriptSegment<'a> {
    lecture: academic_domain::LectureSessionId,
    version: u32,
    index: usize,
    raw: &'a RawSegment,
    tokens: Vec<EffectiveToken<'a>>,
    correction_status: CorrectionStatus,
    versions: Vec<u32>,
}

impl<'a> TranscriptSegment<'a> {
    /// The provider's own identifier for the segment.
    #[must_use]
    pub fn id(&self) -> &'a str {
        self.raw.id()
    }

    /// Which lecture session.
    #[must_use]
    pub const fn lecture(&self) -> academic_domain::LectureSessionId {
        self.lecture
    }

    /// Which version this view was read at.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Its index in the transcript.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// When it starts.
    #[must_use]
    pub const fn start_nanos(&self) -> u64 {
        self.raw.start_nanos()
    }

    /// When it ends, exclusive.
    #[must_use]
    pub const fn end_nanos(&self) -> u64 {
        self.raw.end_nanos()
    }

    /// Who spoke.
    #[must_use]
    pub const fn speaker(&self) -> crate::transcript::Speaker {
        self.raw.speaker()
    }

    /// The provider's verbatim text for the whole segment, unchanged at every
    /// version.
    #[must_use]
    pub fn verbatim_text(&self) -> &'a str {
        self.raw.verbatim_text()
    }

    /// Its tokens as this version reads them, each beside its raw token.
    #[must_use]
    pub fn tokens(&self) -> &[EffectiveToken<'a>] {
        &self.tokens
    }

    /// The journal frames it was transcribed from.
    #[must_use]
    pub fn source_audio_chunks(&self) -> &'a [u32] {
        self.raw.source_audio_chunks()
    }

    /// Section 12.4's `correctionStatus`.
    #[must_use]
    pub const fn correction_status(&self) -> CorrectionStatus {
        self.correction_status
    }

    /// Every version in which a token of this segment changed.
    #[must_use]
    pub fn versions(&self) -> &[u32] {
        &self.versions
    }
}

/// One transcript and every version of it.
///
/// The raw transcript is owned here and handed out by shared reference only.
#[derive(Debug)]
pub struct TranscriptLineage {
    raw: RawTranscript,
    versions: Vec<TranscriptVersion>,
    open_reviews: Vec<TokenAddress>,
}

impl TranscriptLineage {
    /// Opens a lineage at version one: what the provider returned.
    #[must_use]
    pub fn open(raw: RawTranscript) -> Self {
        Self {
            raw,
            versions: vec![TranscriptVersion {
                number: 1,
                supersedes: None,
                corrections: Vec::new(),
                annotations: AnnotationLayer::new(),
            }],
            open_reviews: Vec::new(),
        }
    }

    /// The raw transcript, which is the same value at every version.
    #[must_use]
    pub const fn raw(&self) -> &RawTranscript {
        &self.raw
    }

    /// Every version, oldest first.
    #[must_use]
    pub fn versions(&self) -> &[TranscriptVersion] {
        &self.versions
    }

    /// The newest version.
    ///
    /// # Panics
    ///
    /// Never: [`TranscriptLineage::open`] pushes version one and nothing
    /// removes it, so the vector is non-empty for the lifetime of the value.
    #[must_use]
    pub fn current(&self) -> &TranscriptVersion {
        match self.versions.last() {
            Some(version) => version,
            // Unreachable: `open` pushes version one and there is no removal.
            None => unreachable!("a lineage always holds version one"),
        }
    }

    /// Records that a correction is open against one token.
    ///
    /// The canonical record of a proposal is `academic-proposal`'s append-only
    /// history; this is the projection of it that section 12.4's
    /// `correctionStatus` reads. What decides that a token needs review is the
    /// caller's: a provider's raw confidence is an
    /// `academic_model_run::RawScore` with no readable units, and turning one
    /// into a comparable number needs that crate's `CalibrationRegistry`, which
    /// this crate does not carry.
    ///
    /// # Errors
    ///
    /// [`VersionFault::NoSuchToken`] when no raw token sits at that address.
    pub fn open_review(&mut self, address: TokenAddress) -> Result<(), VersionFault> {
        self.token_at(address)?;
        if !self.open_reviews.contains(&address) {
            self.open_reviews.push(address);
        }
        Ok(())
    }

    /// Every address with an open correction.
    #[must_use]
    pub fn open_reviews(&self) -> &[TokenAddress] {
        &self.open_reviews
    }

    /// Appends a new version carrying one settled correction.
    ///
    /// The raw transcript is untouched. The new version carries every
    /// correction the version before it carried, plus this one, and a
    /// `supersedes` naming that version.
    ///
    /// # Errors
    ///
    /// [`VersionFault::NoSuchToken`] when no raw token sits at the address, and
    /// [`VersionFault::NoChange`] when the replacement equals what the newest
    /// version already reads there.
    pub fn append_correction(&mut self, settled: SettledCorrection) -> Result<u32, VersionFault> {
        let address = settled.candidate.address;
        self.token_at(address)?;
        let previous_text = self.effective_text_at(address)?.to_owned();
        if previous_text == settled.candidate.replacement_text {
            return Err(VersionFault::NoChange);
        }
        let current = self.current();
        let number = current.number.saturating_add(1);
        let mut corrections = current.corrections.clone();
        let annotations = current.annotations.clone();
        corrections.push(AppliedCorrection {
            settled,
            previous_text,
        });
        self.versions.push(TranscriptVersion {
            number,
            supersedes: Some(current.number),
            corrections,
            annotations,
        });
        self.open_reviews.retain(|open| *open != address);
        Ok(number)
    }

    /// Appends a new version carrying a changed annotation layer.
    ///
    /// Formatting is a version too, because a reader has to be able to say
    /// which rendering they saw. The raw transcript is untouched here as well:
    /// the layer holds no token.
    #[must_use]
    pub fn append_annotations(&mut self, annotations: AnnotationLayer) -> u32 {
        let current = self.current();
        let number = current.number.saturating_add(1);
        let corrections = current.corrections.clone();
        self.versions.push(TranscriptVersion {
            number,
            supersedes: Some(current.number),
            corrections,
            annotations,
        });
        number
    }

    /// Reads one segment at one version.
    #[must_use]
    pub fn segment_at(&self, version: u32, index: usize) -> Option<TranscriptSegment<'_>> {
        let version = self.versions.iter().find(|entry| entry.number == version)?;
        let raw = self.raw.segments().get(index)?;
        let mut tokens = Vec::with_capacity(raw.tokens().len());
        for (position, token) in raw.tokens().iter().enumerate() {
            let address = TokenAddress::new(index, position);
            let applied = version
                .corrections
                .iter()
                .rev()
                .find(|correction| correction.address() == address);
            let (effective_text, corrected_at) = match applied {
                Some(correction) => (
                    correction.text(),
                    self.versions
                        .iter()
                        .find(|entry| {
                            entry
                                .corrections
                                .iter()
                                .any(|candidate| candidate == correction)
                        })
                        .map(|entry| entry.number),
                ),
                None => (token.text(), None),
            };
            tokens.push(EffectiveToken {
                raw: token,
                effective_text,
                corrected_at,
            });
        }
        let corrected = version
            .corrections
            .iter()
            .any(|correction| correction.address().segment() == index);
        let under_review = self
            .open_reviews
            .iter()
            .any(|address| address.segment() == index);
        let correction_status = if corrected {
            CorrectionStatus::Corrected
        } else if under_review {
            CorrectionStatus::NeedsReview
        } else {
            CorrectionStatus::Uncorrected
        };
        let versions = self
            .versions
            .iter()
            .filter(|entry| {
                entry
                    .corrections
                    .iter()
                    .any(|correction| correction.address().segment() == index)
            })
            .map(|entry| entry.number)
            .collect();
        Some(TranscriptSegment {
            lecture: self.raw.lecture(),
            version: version.number,
            index,
            raw,
            tokens,
            correction_status,
            versions,
        })
    }

    /// The raw token at an address.
    fn token_at(&self, address: TokenAddress) -> Result<&RawToken, VersionFault> {
        self.raw
            .segments()
            .get(address.segment())
            .and_then(|segment| segment.tokens().get(address.position()))
            .ok_or(VersionFault::NoSuchToken {
                segment: address.segment(),
                position: address.position(),
            })
    }

    /// What the newest version reads at an address.
    fn effective_text_at(&self, address: TokenAddress) -> Result<&str, VersionFault> {
        let current = self.current();
        match current
            .corrections
            .iter()
            .rev()
            .find(|correction| correction.address() == address)
        {
            Some(correction) => Ok(correction.text()),
            None => Ok(self.token_at(address)?.text()),
        }
    }
}

/// The one actor class that may settle a correction.
///
/// `academic_proposal::UserDecision::by` already refuses every automatic actor,
/// so this is the same rule read from the reporting side rather than a second
/// comparison: a caller asking who may settle gets the same answer the queue
/// enforces.
#[must_use]
pub const fn settles_corrections(actor: &Actor) -> bool {
    match actor {
        Actor::User { .. } => true,
        Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
            false
        }
    }
}
