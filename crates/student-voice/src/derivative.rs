//! The redacted derivative, the original it leaves alone, and the retention
//! that can only narrow.
//!
//! # The derivative excludes and the original retains
//!
//! `REQ-12-031` is two sentences: a sensitive utterance is hidden in the
//! display and redacted projections, *and* deletion of the original follows
//! retention policy. So one redaction produces two values.
//! [`RedactedDerivative`] holds what survived and, for what did not, the
//! speaker and the span but **no text**. [`RestrictedOriginal`] holds the text,
//! has no accessor for it, and hands it out only against a grant it consumes.
//!
//! The two carry different retention terms on purpose. The derivative's are the
//! parent's narrowed by whatever it asked for; the original's are the parent's,
//! because redacting a copy is not a reason to keep the original longer or
//! shorter than the permission said.
//!
//! # A redaction that redacts nothing is refused
//!
//! [`RedactionFault::NothingExcluded`] is the guard against the vacuous case:
//! a plan whose exclusion set is empty would satisfy "the derivative contains
//! no targeted speaker" by containing everything.
//!
//! There are **two** sites, and until `P2-RF20` the suite drove one of them.
//! [`RedactionPlan::manual`] refuses an empty list, and [`redact`] refuses a
//! plan whose exclusion set comes out empty after the policy is applied — which
//! is the only reachable arm for an *automatic* plan, because that arm has no
//! manual list to be empty. `P2-A4`'s F11 deleted the second and every row
//! stayed green while `redact` returned a derivative holding every utterance in
//! the lecture, students included, labelled `REDACTED`.
//! `a_redaction_that_removes_nothing_is_refused_on_both_paths` drives it now.
//!
//! # Automatic needs a witness, manual needs a person, and there is no third
//!
//! [`RedactionMode`] has two variants. `Automatic` carries an
//! [`AccuracyWitness`] **by value**, whose one producer is a measurement over a
//! corpus, so there is no automatic redaction claim without a measured number.
//! `Manual` carries no witness and cannot: every span it excludes is a
//! [`ManualExclusion`] a person decided, one at a time, and an automatic actor
//! is refused by an exhaustive `match`.
//!
//! # The retention rule is `P2-G6`'s, called rather than copied
//!
//! `RetentionTerms::inherit` takes the stricter of two bounds on each axis and
//! there is one of it. This module calls it at exactly one place --
//! [`inherit_terms`] -- and every derivative in this crate goes through that
//! function, so the direction of the comparison is one edit in one file and
//! `derivative_expiry_is_equal_or_stricter` walks a grid rather than a case.

use std::fmt;

use academic_consent::{DerivativeClass, RetentionTerms};
use academic_domain::{Actor, ContentDigest, LectureSessionId};
use academic_lecture_document::RedactionPolicyRef;
use academic_transcription::{Speaker, TranscriptLineage};

use crate::{
    fault::{AccessRefusal, RedactionFault},
    measure::AccuracyWitness,
    policy::RedactionPolicy,
};

/// One utterance of the source, as the transcript records it.
///
/// `Debug` is hand-written and reaches the verbatim text through a length
/// only. `P2-L3` and `P2-L4` made the same decision for every type of theirs
/// that holds the lecture in words, in the strengthening direction, and this
/// crate follows it: a panic message or a log line must not print what a
/// student said.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SourceUtterance<'a> {
    index: usize,
    speaker: Speaker,
    start_nanos: u64,
    end_nanos: u64,
    verbatim: &'a str,
}

impl fmt::Debug for SourceUtterance<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceUtterance")
            .field("index", &self.index)
            .field("speaker", &self.speaker)
            .field("verbatim_len", &self.verbatim.len())
            .finish_non_exhaustive()
    }
}

impl<'a> SourceUtterance<'a> {
    /// Its index in the transcript.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Who spoke.
    #[must_use]
    pub const fn speaker(&self) -> Speaker {
        self.speaker
    }

    /// When it starts.
    #[must_use]
    pub const fn start_nanos(&self) -> u64 {
        self.start_nanos
    }

    /// When it ends, exclusive.
    #[must_use]
    pub const fn end_nanos(&self) -> u64 {
        self.end_nanos
    }

    /// What was said.
    #[must_use]
    pub const fn verbatim(&self) -> &'a str {
        self.verbatim
    }
}

/// A lecture read at one transcript version, with the terms that govern it.
///
/// [`LectureSource::of`] takes a `&TranscriptLineage`, whose one producer is
/// `academic_transcription::run`, so a source cannot be assembled out of text
/// somebody wrote here: what this crate redacts is what the pipeline produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LectureSource<'a> {
    lecture: LectureSessionId,
    version: u32,
    terms: RetentionTerms,
    utterances: Vec<SourceUtterance<'a>>,
}

impl<'a> LectureSource<'a> {
    /// Reads a lineage at one version.
    ///
    /// # Errors
    ///
    /// [`RedactionFault::NoSuchVersion`] when the lineage has no such version,
    /// which is also what an empty transcript produces.
    pub fn of(
        lineage: &'a TranscriptLineage,
        version: u32,
        terms: RetentionTerms,
    ) -> Result<Self, RedactionFault> {
        let mut utterances = Vec::new();
        let mut index = 0;
        let mut lecture = None;
        while let Some(segment) = lineage.segment_at(version, index) {
            lecture = Some(segment.lecture());
            utterances.push(SourceUtterance {
                index,
                speaker: segment.speaker(),
                start_nanos: segment.start_nanos(),
                end_nanos: segment.end_nanos(),
                verbatim: segment.verbatim_text(),
            });
            index += 1;
        }
        let lecture = lecture.ok_or(RedactionFault::NoSuchVersion { version })?;
        Ok(Self {
            lecture,
            version,
            terms,
            utterances,
        })
    }

    /// Which lecture.
    #[must_use]
    pub const fn lecture(&self) -> LectureSessionId {
        self.lecture
    }

    /// Which transcript version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// The terms a derivative of it inherits from.
    #[must_use]
    pub const fn terms(&self) -> RetentionTerms {
        self.terms
    }

    /// Its utterances, in transcript order.
    #[must_use]
    pub fn utterances(&self) -> &[SourceUtterance<'a>] {
        &self.utterances
    }
}

/// One exclusion a person decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualExclusion {
    index: usize,
    decided_by: Actor,
}

impl ManualExclusion {
    /// Records one.
    ///
    /// # Errors
    ///
    /// [`RedactionFault::AutomaticActorCannotRedact`] for every automatic
    /// actor, by an exhaustive `match` over `academic-domain`'s closed `Actor`.
    pub fn decided(index: usize, decided_by: Actor) -> Result<Self, RedactionFault> {
        match &decided_by {
            Actor::User { .. } => Ok(Self { index, decided_by }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(RedactionFault::AutomaticActorCannotRedact)
            }
        }
    }

    /// Which utterance.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Who decided.
    #[must_use]
    pub const fn decided_by(&self) -> &Actor {
        &self.decided_by
    }
}

/// How the spans a redaction removes were chosen.
///
/// Two variants and no third. The automatic one carries its measurement by
/// value; there is no `Automatic` without one and no `Default` for this enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedactionMode {
    /// A person chose every span.
    Manual,
    /// The diarizer chose them, and this is the measurement that let it.
    Automatic(AccuracyWitness),
}

impl RedactionMode {
    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "MANUAL",
            Self::Automatic(_) => "AUTOMATIC",
        }
    }

    /// The measurement, when there is one.
    #[must_use]
    pub const fn witness(&self) -> Option<&AccuracyWitness> {
        match self {
            Self::Manual => None,
            Self::Automatic(witness) => Some(witness),
        }
    }
}

/// A policy plus the way its spans were chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionPlan {
    policy: RedactionPolicy,
    mode: RedactionMode,
    manual: Vec<ManualExclusion>,
}

impl RedactionPlan {
    /// A plan whose spans a person chose.
    ///
    /// # Errors
    ///
    /// [`RedactionFault::NothingExcluded`] for an empty list.
    pub fn manual(
        policy: RedactionPolicy,
        exclusions: Vec<ManualExclusion>,
    ) -> Result<Self, RedactionFault> {
        if exclusions.is_empty() {
            return Err(RedactionFault::NothingExcluded);
        }
        Ok(Self {
            policy,
            mode: RedactionMode::Manual,
            manual: exclusions,
        })
    }

    /// A plan whose spans the diarizer chose.
    ///
    /// The witness is taken by value and there is no other way to build this
    /// arm, which is what "below-threshold accuracy forbids an automatic
    /// redaction claim" means when it is a type rather than a check.
    #[must_use]
    pub fn automatic(policy: RedactionPolicy, witness: AccuracyWitness) -> Self {
        Self {
            policy,
            mode: RedactionMode::Automatic(witness),
            manual: Vec::new(),
        }
    }

    /// The policy.
    #[must_use]
    pub const fn policy(&self) -> &RedactionPolicy {
        &self.policy
    }

    /// How the spans were chosen.
    #[must_use]
    pub const fn mode(&self) -> &RedactionMode {
        &self.mode
    }

    /// The exclusions a person decided, empty on the automatic arm.
    #[must_use]
    pub fn manual_exclusions(&self) -> &[ManualExclusion] {
        &self.manual
    }
}

/// One utterance the derivative kept.
///
/// Kept text is still the lecture, so its `Debug` is redacting for the same
/// reason [`SourceUtterance`]'s is.
#[derive(Clone, PartialEq, Eq)]
pub struct KeptUtterance {
    index: usize,
    speaker: Speaker,
    start_nanos: u64,
    end_nanos: u64,
    text: String,
}

impl fmt::Debug for KeptUtterance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeptUtterance")
            .field("index", &self.index)
            .field("speaker", &self.speaker)
            .field("text_len", &self.text.len())
            .finish_non_exhaustive()
    }
}

impl KeptUtterance {
    /// Its index in the source transcript.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Who spoke.
    #[must_use]
    pub const fn speaker(&self) -> Speaker {
        self.speaker
    }

    /// When it starts.
    #[must_use]
    pub const fn start_nanos(&self) -> u64 {
        self.start_nanos
    }

    /// When it ends, exclusive.
    #[must_use]
    pub const fn end_nanos(&self) -> u64 {
        self.end_nanos
    }

    /// What was said.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// One utterance the derivative removed, without its text.
///
/// The span and the speaker are here because a reader has to be able to see
/// that something was removed and how much -- `P2-L4`'s coverage report reads
/// exactly this and refuses to call such a document complete. The text is not
/// here and there is no field for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExclusionRecord {
    index: usize,
    speaker: Speaker,
    start_nanos: u64,
    end_nanos: u64,
}

impl ExclusionRecord {
    /// Its index in the source transcript.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Who spoke.
    #[must_use]
    pub const fn speaker(&self) -> Speaker {
        self.speaker
    }

    /// When it starts.
    #[must_use]
    pub const fn start_nanos(&self) -> u64 {
        self.start_nanos
    }

    /// When it ends, exclusive.
    #[must_use]
    pub const fn end_nanos(&self) -> u64 {
        self.end_nanos
    }

    /// How long it is.
    #[must_use]
    pub const fn duration_nanos(&self) -> u64 {
        self.end_nanos.saturating_sub(self.start_nanos)
    }
}

/// The derivative a redaction produces.
///
/// It holds the utterances that survived and, for the ones that did not, a
/// record with no text. There is no accessor returning a removed utterance and
/// no field holding one, so "the derivative excludes the targeted speakers" is
/// the absence of a place to put them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedDerivative {
    lecture: LectureSessionId,
    source_version: u32,
    policy_digest: ContentDigest,
    mode: RedactionMode,
    terms: RetentionTerms,
    kept: Vec<KeptUtterance>,
    excluded: Vec<ExclusionRecord>,
}

impl RedactedDerivative {
    /// Which lecture.
    #[must_use]
    pub const fn lecture(&self) -> LectureSessionId {
        self.lecture
    }

    /// Which transcript version it was taken from.
    #[must_use]
    pub const fn source_version(&self) -> u32 {
        self.source_version
    }

    /// The policy it was taken under.
    #[must_use]
    pub const fn policy_digest(&self) -> &ContentDigest {
        &self.policy_digest
    }

    /// How the spans were chosen.
    #[must_use]
    pub const fn mode(&self) -> &RedactionMode {
        &self.mode
    }

    /// Its retention terms, which are no wider than its parent's on either
    /// axis.
    #[must_use]
    pub const fn terms(&self) -> RetentionTerms {
        self.terms
    }

    /// What it kept.
    #[must_use]
    pub fn kept(&self) -> &[KeptUtterance] {
        &self.kept
    }

    /// What it removed, without the text.
    #[must_use]
    pub fn excluded(&self) -> &[ExclusionRecord] {
        &self.excluded
    }

    /// Whether any utterance it kept is spoken by a speaker `policy` targets.
    ///
    /// Stated separately from the construction so a test can assert it against
    /// a value this module did not produce, which is `RetentionTerms::is_no_wider_than`'s
    /// shape in `P2-G6`.
    #[must_use]
    pub fn keeps_a_targeted_speaker(&self, policy: &RedactionPolicy) -> bool {
        self.kept
            .iter()
            .any(|utterance| policy.targets(utterance.speaker))
    }

    /// The terms a further derivative of this one inherits.
    ///
    /// Calls the same [`inherit_terms`] every other derivative goes through, so
    /// a chain cannot widen at any link.
    #[must_use]
    pub fn inherit_for_child(&self, requested: RetentionTerms) -> RetentionTerms {
        inherit_terms(self.terms, requested)
    }

    /// The derivative's canonical bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut text = format!(
            "academic-redacted-derivative/1 {} {} {} {}\n",
            self.lecture,
            self.source_version,
            self.policy_digest,
            self.mode.as_str()
        );
        for kept in &self.kept {
            text.push_str("kept=");
            text.push_str(&kept.index.to_string());
            text.push(' ');
            text.push_str(&kept.speaker.spelling());
            text.push(' ');
            text.push_str(&kept.text);
            text.push('\n');
        }
        for excluded in &self.excluded {
            text.push_str("excluded=");
            text.push_str(&excluded.index.to_string());
            text.push(' ');
            text.push_str(&excluded.speaker.spelling());
            text.push(' ');
            text.push_str(&excluded.start_nanos.to_string());
            text.push(' ');
            text.push_str(&excluded.end_nanos.to_string());
            text.push('\n');
        }
        text.into_bytes()
    }

    /// The digest a deletion preview names it by.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(&self.canonical_bytes())
    }
}

/// One utterance the original still holds.
///
/// Private fields with no public accessor. A [`DisclosedOriginal`] is the one
/// route to the text and it exists only for the length of an authorized read.
#[derive(Clone, PartialEq, Eq)]
pub struct RemovedUtterance {
    index: usize,
    speaker: Speaker,
    start_nanos: u64,
    end_nanos: u64,
    verbatim: String,
}

impl fmt::Debug for RemovedUtterance {
    /// Redacting, and this is the one that matters most on this page: the whole
    /// point of a restricted original is that its text has no route out except
    /// an authorized read, and a derived `Debug` would be one.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemovedUtterance")
            .field("index", &self.index)
            .field("speaker", &self.speaker)
            .field("verbatim_len", &self.verbatim.len())
            .finish_non_exhaustive()
    }
}

/// What the original still is after a derivative was taken from it.
///
/// The classification is `RESTRICTED` and there is no method that changes it.
/// An authorized read does not lift it: it produces one disclosure and one
/// audit row, and leaves this value exactly as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictedOriginal {
    lecture: LectureSessionId,
    source_version: u32,
    terms: RetentionTerms,
    removed: Vec<RemovedUtterance>,
    digest: ContentDigest,
}

/// The classification an original carries, always.
pub const ORIGINAL_CLASSIFICATION: &str = "RESTRICTED";

impl RestrictedOriginal {
    /// Which lecture.
    #[must_use]
    pub const fn lecture(&self) -> LectureSessionId {
        self.lecture
    }

    /// Which transcript version.
    #[must_use]
    pub const fn source_version(&self) -> u32 {
        self.source_version
    }

    /// Its retention terms, which are the permission's rather than the
    /// derivative's.
    #[must_use]
    pub const fn terms(&self) -> RetentionTerms {
        self.terms
    }

    /// How many utterances the derivative removed.
    ///
    /// A count, not the content. A reader that has no grant can see that
    /// something was removed and how much, which is what makes a redaction
    /// visible without making it readable.
    #[must_use]
    pub fn removed_count(&self) -> usize {
        self.removed.len()
    }

    /// Its classification, which is the same before and after any access.
    #[must_use]
    pub const fn classification(&self) -> &'static str {
        ORIGINAL_CLASSIFICATION
    }

    /// The identity a grant is bound to.
    #[must_use]
    pub const fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Opens the original against a grant, and records that it was opened.
    ///
    /// The grant is taken **by value**, so a second read needs a second grant:
    /// one-use is the move rather than a flag. The log is taken by mutable
    /// reference and appended before the disclosure is returned, so there is no
    /// authorized read with no audit row -- the shape `preview_expiry` uses in
    /// `academic-consent`.
    ///
    /// # Errors
    ///
    /// [`AccessRefusal::GrantIsForAnotherOriginal`] when the grant names a
    /// different original.
    pub fn open(
        &self,
        grant: RawAccessGrant,
        log: &mut RawAccessLog,
    ) -> Result<DisclosedOriginal<'_>, AccessRefusal> {
        if grant.original_digest != self.digest {
            return Err(AccessRefusal::GrantIsForAnotherOriginal);
        }
        log.entries.push(RawAccessRecord {
            original_digest: self.digest,
            opened_by: grant.requested_by.clone(),
            purpose: grant.purpose.clone(),
            at: grant.at,
            utterances_disclosed: self.removed.len(),
        });
        Ok(DisclosedOriginal {
            removed: &self.removed,
        })
    }
}

/// A one-use authorization to read one original.
///
/// It is consumed by [`RestrictedOriginal::open`], which takes it by value. It
/// implements no `Clone` and no `Copy`, so there is no way to spend it twice.
#[derive(Debug, PartialEq, Eq)]
pub struct RawAccessGrant {
    original_digest: ContentDigest,
    requested_by: Actor,
    purpose: String,
    at: u64,
}

impl RawAccessGrant {
    /// Issues a grant for one original.
    ///
    /// # Errors
    ///
    /// [`AccessRefusal::AutomaticActorCannotOpen`] for every automatic actor.
    /// Reading the speech a redaction removed is a disclosure of somebody
    /// else's words, and section 27.2 does not let a model authorize one.
    pub fn issued(
        original: &RestrictedOriginal,
        requested_by: Actor,
        purpose: &str,
        at: u64,
    ) -> Result<Self, AccessRefusal> {
        match &requested_by {
            Actor::User { .. } => Ok(Self {
                original_digest: original.digest,
                requested_by,
                purpose: purpose.to_owned(),
                at,
            }),
            Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => {
                Err(AccessRefusal::AutomaticActorCannotOpen)
            }
        }
    }

    /// Which original.
    #[must_use]
    pub const fn original_digest(&self) -> &ContentDigest {
        &self.original_digest
    }

    /// Who asked.
    #[must_use]
    pub const fn requested_by(&self) -> &Actor {
        &self.requested_by
    }

    /// Why.
    #[must_use]
    pub fn purpose(&self) -> &str {
        &self.purpose
    }
}

/// One authorized read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAccessRecord {
    original_digest: ContentDigest,
    opened_by: Actor,
    purpose: String,
    at: u64,
    utterances_disclosed: usize,
}

impl RawAccessRecord {
    /// Which original.
    #[must_use]
    pub const fn original_digest(&self) -> &ContentDigest {
        &self.original_digest
    }

    /// Who read it.
    #[must_use]
    pub const fn opened_by(&self) -> &Actor {
        &self.opened_by
    }

    /// Why.
    #[must_use]
    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    /// When.
    #[must_use]
    pub const fn at(&self) -> u64 {
        self.at
    }

    /// How many utterances the read reached.
    #[must_use]
    pub const fn utterances_disclosed(&self) -> usize {
        self.utterances_disclosed
    }
}

/// Every authorized read, append-only.
///
/// One `&mut self` method and it pushes: ADR-003's shape rather than a second
/// mechanism. There is no removal, no replacement and no mutable accessor into
/// an entry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawAccessLog {
    entries: Vec<RawAccessRecord>,
}

impl RawAccessLog {
    /// An empty log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Every read, in the order they happened.
    #[must_use]
    pub fn entries(&self) -> &[RawAccessRecord] {
        &self.entries
    }
}

/// The removed speech, for the length of one authorized read.
///
/// It borrows the original, implements no `Clone`, and has no owned form. What
/// a holder may do with the text it reads is outside this crate; what this
/// crate does not do is route it back -- no function here takes a
/// [`DisclosedOriginal`] and returns a [`RedactedDerivative`], a
/// [`KeptUtterance`] or a [`LectureSource`], and `no_disclosure_reaches_a_derivative`
/// asserts that over every package in `crates/`.
#[derive(Debug, PartialEq, Eq)]
pub struct DisclosedOriginal<'a> {
    removed: &'a [RemovedUtterance],
}

impl DisclosedOriginal<'_> {
    /// How many utterances this read reached.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.removed.len()
    }

    /// Whether it reached none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.removed.is_empty()
    }

    /// The text of the utterance at `position`, if there is one.
    #[must_use]
    pub fn verbatim(&self, position: usize) -> Option<&str> {
        self.removed
            .get(position)
            .map(|utterance| utterance.verbatim.as_str())
    }

    /// Who spoke it.
    #[must_use]
    pub fn speaker(&self, position: usize) -> Option<Speaker> {
        self.removed
            .get(position)
            .map(|utterance| utterance.speaker)
    }

    /// Which source index it had.
    #[must_use]
    pub fn source_index(&self, position: usize) -> Option<usize> {
        self.removed.get(position).map(|utterance| utterance.index)
    }
}

/// A redaction: the derivative and the original it left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redaction {
    derivative: RedactedDerivative,
    original: RestrictedOriginal,
}

impl Redaction {
    /// What a reader gets.
    #[must_use]
    pub const fn derivative(&self) -> &RedactedDerivative {
        &self.derivative
    }

    /// What stays restricted.
    #[must_use]
    pub const fn original(&self) -> &RestrictedOriginal {
        &self.original
    }
}

/// The one place a derivative's retention terms come from.
///
/// `P2-G6` owns the rule and there is one call to it in this crate.
/// `derivative_terms_have_one_producer` counts this function's callers, so a
/// second inheritance path is an extra key rather than a `max` nobody notices.
#[must_use]
pub fn inherit_terms(parent: RetentionTerms, requested: RetentionTerms) -> RetentionTerms {
    parent.inherit(requested)
}

/// Any artifact derived from a redaction, in one of `P2-K5`'s classes.
///
/// It exists so that a chain -- derivative, transcript of the derivative,
/// summary of that -- is representable and every link goes through
/// [`inherit_terms`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedArtifact {
    class: DerivativeClass,
    parent_digest: ContentDigest,
    terms: RetentionTerms,
}

impl DerivedArtifact {
    /// An artifact derived directly from a redacted derivative.
    #[must_use]
    pub fn of_derivative(
        parent: &RedactedDerivative,
        class: DerivativeClass,
        requested: RetentionTerms,
    ) -> Self {
        Self {
            class,
            parent_digest: parent.digest(),
            terms: parent.inherit_for_child(requested),
        }
    }

    /// An artifact derived from another artifact.
    #[must_use]
    pub fn of_artifact(parent: &Self, class: DerivativeClass, requested: RetentionTerms) -> Self {
        Self {
            class,
            parent_digest: parent.digest(),
            terms: inherit_terms(parent.terms, requested),
        }
    }

    /// Which class.
    #[must_use]
    pub const fn class(&self) -> DerivativeClass {
        self.class
    }

    /// What it was derived from.
    #[must_use]
    pub const fn parent_digest(&self) -> &ContentDigest {
        &self.parent_digest
    }

    /// Its terms.
    #[must_use]
    pub const fn terms(&self) -> RetentionTerms {
        self.terms
    }

    /// The identity a preview names it by.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let mut material = b"academic-derived-artifact-v1\0".to_vec();
        material.extend_from_slice(self.class.as_str().as_bytes());
        material.extend_from_slice(self.parent_digest.as_bytes());
        ContentDigest::sha256(&material)
    }
}

/// Applies a plan to a source.
///
/// The one producer of a [`RedactedDerivative`] and a [`RestrictedOriginal`].
/// It takes the policy reference the `P2-L4` disposition cites, so a derivative
/// whose reference resolves to a different policy is refused here rather than
/// left for a reader to notice.
///
/// # Errors
///
/// [`RedactionFault::PolicyReferenceDoesNotResolve`] when the cited digest is
/// not this policy's; [`RedactionFault::NoSuchSegment`] and
/// [`RedactionFault::SegmentIsNotTargeted`] for a manual exclusion that names
/// an utterance the source does not have or the policy does not target; and
/// [`RedactionFault::NothingExcluded`] when the plan would remove nothing.
pub fn redact(
    plan: &RedactionPlan,
    reference: &RedactionPolicyRef,
    source: &LectureSource<'_>,
    requested: RetentionTerms,
) -> Result<Redaction, RedactionFault> {
    plan.policy.resolve(reference)?;
    let excluded_indices = excluded_indices(plan, source)?;
    if excluded_indices.is_empty() {
        return Err(RedactionFault::NothingExcluded);
    }
    let mut kept = Vec::new();
    let mut excluded = Vec::new();
    let mut removed = Vec::new();
    for utterance in &source.utterances {
        if excluded_indices.contains(&utterance.index) {
            excluded.push(ExclusionRecord {
                index: utterance.index,
                speaker: utterance.speaker,
                start_nanos: utterance.start_nanos,
                end_nanos: utterance.end_nanos,
            });
            removed.push(RemovedUtterance {
                index: utterance.index,
                speaker: utterance.speaker,
                start_nanos: utterance.start_nanos,
                end_nanos: utterance.end_nanos,
                verbatim: utterance.verbatim.to_owned(),
            });
        } else {
            kept.push(KeptUtterance {
                index: utterance.index,
                speaker: utterance.speaker,
                start_nanos: utterance.start_nanos,
                end_nanos: utterance.end_nanos,
                text: utterance.verbatim.to_owned(),
            });
        }
    }
    let derivative = RedactedDerivative {
        lecture: source.lecture,
        source_version: source.version,
        policy_digest: plan.policy.digest(),
        mode: plan.mode.clone(),
        terms: inherit_terms(source.terms, requested),
        kept,
        excluded,
    };
    let original = RestrictedOriginal {
        lecture: source.lecture,
        source_version: source.version,
        terms: source.terms,
        digest: original_digest(source),
        removed,
    };
    Ok(Redaction {
        derivative,
        original,
    })
}

/// Which utterances a plan removes.
///
/// A total `match` over the two modes. The automatic arm reads the transcript's
/// own speaker attribution -- which is the diarizer's output -- against the
/// policy's targeting; the manual arm reads the person's list and checks each
/// entry against the same targeting, so a manual plan cannot remove an
/// utterance the policy does not cover either.
fn excluded_indices(
    plan: &RedactionPlan,
    source: &LectureSource<'_>,
) -> Result<Vec<usize>, RedactionFault> {
    match &plan.mode {
        RedactionMode::Automatic(_) => Ok(source
            .utterances
            .iter()
            .filter(|utterance| plan.policy.targets(utterance.speaker))
            .map(|utterance| utterance.index)
            .collect()),
        RedactionMode::Manual => {
            let mut indices = Vec::new();
            for exclusion in &plan.manual {
                let utterance = source
                    .utterances
                    .iter()
                    .find(|candidate| candidate.index == exclusion.index)
                    .ok_or(RedactionFault::NoSuchSegment {
                        index: exclusion.index,
                    })?;
                if !plan.policy.targets(utterance.speaker) {
                    return Err(RedactionFault::SegmentIsNotTargeted {
                        index: exclusion.index,
                    });
                }
                indices.push(exclusion.index);
            }
            Ok(indices)
        }
    }
}

fn original_digest(source: &LectureSource<'_>) -> ContentDigest {
    let mut material = b"academic-restricted-original-v1\0".to_vec();
    material.extend_from_slice(source.lecture.to_string().as_bytes());
    material.extend_from_slice(&source.version.to_be_bytes());
    for utterance in &source.utterances {
        material.extend_from_slice(&utterance.index.to_be_bytes());
        material.extend_from_slice(utterance.speaker.spelling().as_bytes());
        material.extend_from_slice(&utterance.start_nanos.to_be_bytes());
        material.extend_from_slice(&utterance.end_nanos.to_be_bytes());
        material.extend_from_slice(utterance.verbatim.as_bytes());
    }
    ContentDigest::sha256(&material)
}
