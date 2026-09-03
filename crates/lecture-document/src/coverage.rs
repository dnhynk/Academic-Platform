//! Section 12.6's deterministic coverage validator.
//!
//! # Exactly one status, held by the type
//!
//! [`SegmentStatus`] has four variants and a [`SegmentAccount`] has **one**
//! field of that type. Two statuses is not a value: there is no set, no vector
//! and no `Option`. Zero statuses is not a value either, for the same reason.
//! An unknown status is not a value because the enum is closed and carries no
//! `#[non_exhaustive]`. And a non-mapped status without its evidence is not a
//! value because the evidence is a field of the variant.
//!
//! What remains — a caller mapping a segment in the document *and* declaring a
//! disposition for it — is genuinely a property of two inputs, so it is decided
//! by a total `match` over `(mapped, disposition)` whose four arms the compiler
//! enumerates. Three of them produce an outcome and the fourth is
//! [`CoverageFault::SegmentHasTwoStatuses`]. A report that exists therefore has
//! a clean partition, and `segment_status_exhaustive` proves the partition
//! reconciles by counting rather than by trusting it.
//!
//! # `UNMAPPED` is the absence of a status, not a fifth one
//!
//! Section 12.6 lists four statuses and then says a single `UNMAPPED` segment
//! makes the document `INCOMPLETE`. Those are two different sentences and this
//! module keeps them apart: [`CoverageReport::accounts`] holds the segments
//! that have one of the four, [`CoverageReport::unmapped`] holds the ones that
//! have none, and the completeness witness is minted only when the second list
//! is empty. Incomplete is what a report is unless something proves otherwise.
//!
//! # Nothing here shrinks the denominator
//!
//! The eligible segment set is `0..` over the lineage at the document's
//! version, walked until the lineage returns `None`. It is not an argument, so
//! there is no parameter a caller could pass to leave a segment out of the
//! count — which is half of `no_low_importance_deletion`, the half a scan
//! cannot express.

use academic_capture::{JournalRecovery, RecordBody, SessionClockDomain};
use academic_domain::{ContentDigest, LectureSessionId};
use academic_transcription::{InputManifest, TranscriptLineage};

use crate::{
    config::CoverageConfig,
    disposition::{
        CaptureExclusionLedger, CaptureExclusionReason, NonSpeechEvidence, RedactionPolicyRef,
        TranscriptionFailure,
    },
    document::{CrossReferenceReason, LectureDocument, NodeId, be_len, push_str, token_spans},
    fault::CoverageFault,
};

/// The status of one segment. Exactly the four section 12.6 names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentStatus {
    /// The document renders it. Derived, never declared.
    Mapped {
        /// The nodes that map it, in document order.
        nodes: Vec<NodeId>,
    },
    /// It holds no speech.
    ExcludedNonSpeech {
        /// Why, and who decided.
        evidence: NonSpeechEvidence,
    },
    /// A policy removed it.
    RedactedWithPolicy {
        /// Which policy.
        policy: RedactionPolicyRef,
    },
    /// The recording failed over it.
    UntranscribedFailure {
        /// The journal frame that says so.
        failure: TranscriptionFailure,
    },
}

impl SegmentStatus {
    /// The contract spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Mapped { .. } => "MAPPED",
            Self::ExcludedNonSpeech { .. } => "EXCLUDED_NON_SPEECH",
            Self::RedactedWithPolicy { .. } => "REDACTED_WITH_POLICY",
            Self::UntranscribedFailure { .. } => "UNTRANSCRIBED_FAILURE",
        }
    }

    /// The four spellings, in the order section 12.6 lists them.
    ///
    /// A list of names, used only where a name is what is needed — the engine's
    /// frozen inputs and the report's canonical bytes. It is not a guard: the
    /// enum is.
    pub const SPELLINGS: [&'static str; 4] = [
        "MAPPED",
        "EXCLUDED_NON_SPEECH",
        "REDACTED_WITH_POLICY",
        "UNTRANSCRIBED_FAILURE",
    ];

    /// Whether the document renders this segment.
    #[must_use]
    pub const fn is_mapped(&self) -> bool {
        matches!(self, Self::Mapped { .. })
    }
}

/// One segment and its one status.
///
/// Private fields, one field of status type, one producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentAccount {
    segment_index: usize,
    segment_id: String,
    token_count: usize,
    status: SegmentStatus,
}

impl SegmentAccount {
    /// Which segment, by index at the document's version.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// The provider's identifier for it.
    #[must_use]
    pub fn segment_id(&self) -> &str {
        &self.segment_id
    }

    /// How many tokens it has at that version.
    #[must_use]
    pub const fn token_count(&self) -> usize {
        self.token_count
    }

    /// Its one status.
    #[must_use]
    pub const fn status(&self) -> &SegmentStatus {
        &self.status
    }
}

/// One eligible segment with no status at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmappedSegment {
    segment_index: usize,
    segment_id: String,
    token_count: usize,
}

impl UnmappedSegment {
    /// Which segment.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// The provider's identifier for it.
    #[must_use]
    pub fn segment_id(&self) -> &str {
        &self.segment_id
    }

    /// How many tokens it has.
    #[must_use]
    pub const fn token_count(&self) -> usize {
        self.token_count
    }
}

/// An exact ratio. No floating point reaches a coverage number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio {
    numerator: u64,
    denominator: u64,
}

impl Ratio {
    /// The numerator.
    #[must_use]
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    /// The denominator.
    #[must_use]
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Whether every countable thing is counted.
    ///
    /// A zero denominator is *not* complete. A transcript with no eligible
    /// segment has nothing to be complete about, and returning `true` there
    /// would hand a completeness witness to an empty document.
    #[must_use]
    pub const fn is_whole(self) -> bool {
        self.denominator > 0 && self.numerator == self.denominator
    }
}

/// One node that renders an earlier segment than the node before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingFinding {
    node: NodeId,
    previous_segment_index: usize,
    segment_index: usize,
}

impl OrderingFinding {
    /// Which node.
    #[must_use]
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// The segment the previous node reached.
    #[must_use]
    pub const fn previous_segment_index(&self) -> usize {
        self.previous_segment_index
    }

    /// The segment this node went back to.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }
}

/// One node that goes back and says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingException {
    node: NodeId,
    segment_index: usize,
    reason: CrossReferenceReason,
}

impl OrderingException {
    /// Which node.
    #[must_use]
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Which segment it went back to.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// The reason it declared.
    #[must_use]
    pub const fn reason(&self) -> CrossReferenceReason {
        self.reason
    }
}

/// One authorized capture that is neither placed nor excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnaccountedCapture {
    frame_seq: u32,
}

impl UnaccountedCapture {
    /// Which capture.
    #[must_use]
    pub const fn frame_seq(self) -> u32 {
        self.frame_seq
    }
}

/// One hole in the audio timeline.
///
/// A hole between two frames of the *same* session clock has a length, and it
/// is a finding when that length is above the configured threshold. A hole
/// across a clock change has **no** length: `P2-L2`'s `SessionTick::offset_from`
/// refuses a distance between two clocks, and inventing one here would be the
/// same error one layer up. An unmeasurable hole is always a finding, because
/// unknown is not below a threshold — folding it into a pass would manufacture
/// a verdict, which is the rule `InputValue::Unknown` already states for the
/// engine harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapFinding {
    from_frame_seq: u32,
    to_frame_seq: u32,
    length_nanos: Option<u64>,
    explained: bool,
}

impl GapFinding {
    /// The frame before the hole.
    #[must_use]
    pub const fn from_frame_seq(self) -> u32 {
        self.from_frame_seq
    }

    /// The frame after it.
    #[must_use]
    pub const fn to_frame_seq(self) -> u32 {
        self.to_frame_seq
    }

    /// How long it is, when the two frames share a session clock.
    ///
    /// Section 34.1's display wants the length; `None` is the honest answer
    /// across a clock change and the display says so rather than showing a zero.
    #[must_use]
    pub const fn length_nanos(self) -> Option<u64> {
        self.length_nanos
    }

    /// Whether a journal gap frame explains it. Section 34.1's display wants
    /// the cause, and an explained hole carries one.
    #[must_use]
    pub const fn explained(self) -> bool {
        self.explained
    }
}

/// The witness a document needs to be called complete.
///
/// Private fields and one producer, [`CoverageReport::completeness_witness`].
/// It is what makes "incomplete unless proven otherwise" structural rather than
/// a default value somebody could change: there is no other way to obtain one,
/// and `PdfArtifact` cannot be `COMPLETE` without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletenessWitness {
    report_digest: ContentDigest,
}

impl CompletenessWitness {
    /// The digest of the report that minted it.
    #[must_use]
    pub const fn report_digest(&self) -> &ContentDigest {
        &self.report_digest
    }
}

/// Everything a coverage run reads.
#[derive(Debug, Clone, Copy)]
pub struct CoverageInputs<'a> {
    /// The transcript.
    pub lineage: &'a TranscriptLineage,
    /// Which version of it the document renders.
    pub version: u32,
    /// The document.
    pub document: &'a LectureDocument,
    /// The inputs the transcription run was authorized to read.
    pub manifest: &'a InputManifest,
    /// The journal the capture wrote.
    pub journal: &'a JournalRecovery,
    /// The three declared statuses.
    pub dispositions: &'a DispositionLedger,
    /// The captures the document does not place.
    pub capture_exclusions: &'a CaptureExclusionLedger,
    /// The thresholds this run is evaluated under.
    pub config: CoverageConfig,
}

/// One segment's declared status.
///
/// There is no `mapped` constructor. `MAPPED` is derived from the document and
/// a caller cannot declare it, which is what stops a coverage number being
/// asserted rather than measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentDisposition {
    segment_index: usize,
    status: SegmentStatus,
}

impl SegmentDisposition {
    /// Declares that a segment holds no speech.
    #[must_use]
    pub const fn excluded_non_speech(segment_index: usize, evidence: NonSpeechEvidence) -> Self {
        Self {
            segment_index,
            status: SegmentStatus::ExcludedNonSpeech { evidence },
        }
    }

    /// Declares that a policy removed a segment.
    #[must_use]
    pub const fn redacted_with_policy(segment_index: usize, policy: RedactionPolicyRef) -> Self {
        Self {
            segment_index,
            status: SegmentStatus::RedactedWithPolicy { policy },
        }
    }

    /// Declares that the recording failed over a segment.
    #[must_use]
    pub const fn untranscribed_failure(
        segment_index: usize,
        failure: TranscriptionFailure,
    ) -> Self {
        Self {
            segment_index,
            status: SegmentStatus::UntranscribedFailure { failure },
        }
    }

    /// Which segment.
    #[must_use]
    pub const fn segment_index(&self) -> usize {
        self.segment_index
    }

    /// The status it declares.
    #[must_use]
    pub const fn status(&self) -> &SegmentStatus {
        &self.status
    }
}

/// Every declared status for one document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispositionLedger {
    entries: Vec<SegmentDisposition>,
}

impl DispositionLedger {
    /// An empty ledger.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Records one declaration.
    ///
    /// # Errors
    ///
    /// [`CoverageFault::DuplicateDisposition`] when the segment already has
    /// one. Two declarations for one segment would be two statuses, and the
    /// ledger refuses the second rather than picking between them.
    pub fn record(&mut self, disposition: SegmentDisposition) -> Result<(), CoverageFault> {
        if self
            .entries
            .iter()
            .any(|entry| entry.segment_index == disposition.segment_index)
        {
            return Err(CoverageFault::DuplicateDisposition(
                disposition.segment_index,
            ));
        }
        self.entries.push(disposition);
        Ok(())
    }

    /// Every declaration, in record order.
    #[must_use]
    pub fn entries(&self) -> &[SegmentDisposition] {
        &self.entries
    }

    /// The declaration for one segment, if there is one.
    #[must_use]
    pub fn for_segment(&self, segment_index: usize) -> Option<&SegmentDisposition> {
        self.entries
            .iter()
            .find(|entry| entry.segment_index == segment_index)
    }
}

/// Section 12.6's report.
///
/// Private fields and one producer. A number in here was measured by
/// [`CoverageValidator::validate`] over a transcript, a document and a journal;
/// none of them is a value a caller set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageReport {
    lecture: LectureSessionId,
    version: u32,
    config: CoverageConfig,
    accounts: Vec<SegmentAccount>,
    unmapped: Vec<UnmappedSegment>,
    segment_coverage: Ratio,
    token_coverage: Ratio,
    ordering_findings: Vec<OrderingFinding>,
    ordering_exceptions: Vec<OrderingException>,
    placed_captures: Vec<u32>,
    excluded_captures: Vec<(u32, CaptureExclusionReason)>,
    unaccounted_captures: Vec<UnaccountedCapture>,
    gaps: Vec<GapFinding>,
    document_digest: ContentDigest,
    transcript_token_digest: ContentDigest,
}

impl CoverageReport {
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

    /// The thresholds this run used.
    #[must_use]
    pub const fn config(&self) -> CoverageConfig {
        self.config
    }

    /// Every segment that has one of the four statuses, by ascending index.
    #[must_use]
    pub fn accounts(&self) -> &[SegmentAccount] {
        &self.accounts
    }

    /// Every eligible segment with no status, by ascending index.
    #[must_use]
    pub fn unmapped(&self) -> &[UnmappedSegment] {
        &self.unmapped
    }

    /// Mapped non-silence segments over all eligible segments.
    #[must_use]
    pub const fn segment_coverage(&self) -> Ratio {
        self.segment_coverage
    }

    /// Mapped tokens over all tokens.
    #[must_use]
    pub const fn token_coverage(&self) -> Ratio {
        self.token_coverage
    }

    /// Every node that goes back without saying why.
    #[must_use]
    pub fn ordering_findings(&self) -> &[OrderingFinding] {
        &self.ordering_findings
    }

    /// Every node that goes back and says why.
    #[must_use]
    pub fn ordering_exceptions(&self) -> &[OrderingException] {
        &self.ordering_exceptions
    }

    /// Every authorized capture the document places.
    #[must_use]
    pub fn placed_captures(&self) -> &[u32] {
        &self.placed_captures
    }

    /// Every authorized capture excluded with a reason.
    #[must_use]
    pub fn excluded_captures(&self) -> &[(u32, CaptureExclusionReason)] {
        &self.excluded_captures
    }

    /// Every authorized capture that is neither.
    #[must_use]
    pub fn unaccounted_captures(&self) -> &[UnaccountedCapture] {
        &self.unaccounted_captures
    }

    /// Every hole above the threshold, explained or not.
    #[must_use]
    pub fn gaps(&self) -> &[GapFinding] {
        &self.gaps
    }

    /// Holes above the threshold that nothing explains.
    #[must_use]
    pub fn unexplained_gaps(&self) -> Vec<GapFinding> {
        self.gaps
            .iter()
            .filter(|gap| !gap.explained)
            .copied()
            .collect()
    }

    /// The document this report is about.
    #[must_use]
    pub const fn document_digest(&self) -> &ContentDigest {
        &self.document_digest
    }

    /// The raw token digest of the transcript underneath it.
    #[must_use]
    pub const fn transcript_token_digest(&self) -> &ContentDigest {
        &self.transcript_token_digest
    }

    /// How many eligible segments have no status.
    ///
    /// Section 34.1's `INCOMPLETE` banner wants this number beside it.
    #[must_use]
    pub fn unmapped_count(&self) -> usize {
        self.unmapped.len()
    }

    /// Whether every eligible segment has exactly one status.
    ///
    /// This is the reconciliation `segment_status_exhaustive` checks by
    /// counting: the accounts and the unmapped list are disjoint and together
    /// they are the whole eligible set.
    #[must_use]
    pub fn reconciles(&self) -> bool {
        let eligible = self.accounts.len().saturating_add(self.unmapped.len());
        let mut seen: Vec<usize> = self
            .accounts
            .iter()
            .map(SegmentAccount::segment_index)
            .chain(self.unmapped.iter().map(UnmappedSegment::segment_index))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len() == eligible && seen == (0..eligible).collect::<Vec<_>>()
    }

    /// The witness a complete document needs, when there is one.
    ///
    /// Five things have to hold, and every one of them is section 12.6's own:
    /// no unmapped segment, whole segment coverage, whole token coverage, no
    /// ordering finding, no unaccounted capture, and no unexplained hole. There
    /// is no argument that relaxes any of them.
    #[must_use]
    pub fn completeness_witness(&self) -> Option<CompletenessWitness> {
        if !self.unmapped.is_empty()
            || !self.segment_coverage.is_whole()
            || !self.token_coverage.is_whole()
            || !self.ordering_findings.is_empty()
            || !self.unaccounted_captures.is_empty()
            || !self.unexplained_gaps().is_empty()
            || !self.reconciles()
        {
            return None;
        }
        Some(CompletenessWitness {
            report_digest: self.digest(),
        })
    }

    /// One digest over the whole report.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::sha256(&self.canonical_material())
    }

    fn canonical_material(&self) -> Vec<u8> {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-coverage-report-v1\0");
        material.extend_from_slice(self.lecture.to_string().as_bytes());
        material.extend_from_slice(&self.version.to_be_bytes());
        material.extend_from_slice(&self.config.version().to_be_bytes());
        material.extend_from_slice(&self.config.gap_threshold_nanos().to_be_bytes());
        material.extend_from_slice(
            &self
                .config
                .low_confidence_at_or_below_permille()
                .to_be_bytes(),
        );
        material.extend_from_slice(self.document_digest.to_string().as_bytes());
        material.extend_from_slice(self.transcript_token_digest.to_string().as_bytes());
        material.extend_from_slice(&be_len(self.accounts.len()));
        for account in &self.accounts {
            material.extend_from_slice(&be_len(account.segment_index));
            push_str(&mut material, &account.segment_id);
            material.extend_from_slice(&be_len(account.token_count));
            push_str(&mut material, account.status.as_str());
            match &account.status {
                SegmentStatus::Mapped { nodes } => {
                    material.extend_from_slice(&be_len(nodes.len()));
                    for node in nodes {
                        push_str(&mut material, node.as_str());
                    }
                }
                SegmentStatus::ExcludedNonSpeech { evidence } => {
                    push_str(&mut material, evidence.reason().as_str());
                }
                SegmentStatus::RedactedWithPolicy { policy } => {
                    push_str(&mut material, policy.basis().as_str());
                    push_str(&mut material, &policy.policy_digest().to_string());
                }
                SegmentStatus::UntranscribedFailure { failure } => {
                    material.extend_from_slice(&failure.frame_seq().to_be_bytes());
                    push_str(&mut material, failure.cause().as_str());
                }
            }
        }
        material.extend_from_slice(&be_len(self.unmapped.len()));
        for segment in &self.unmapped {
            material.extend_from_slice(&be_len(segment.segment_index));
            push_str(&mut material, &segment.segment_id);
            material.extend_from_slice(&be_len(segment.token_count));
        }
        material.extend_from_slice(&self.segment_coverage.numerator.to_be_bytes());
        material.extend_from_slice(&self.segment_coverage.denominator.to_be_bytes());
        material.extend_from_slice(&self.token_coverage.numerator.to_be_bytes());
        material.extend_from_slice(&self.token_coverage.denominator.to_be_bytes());
        material.extend_from_slice(&be_len(self.ordering_findings.len()));
        for finding in &self.ordering_findings {
            push_str(&mut material, finding.node.as_str());
            material.extend_from_slice(&be_len(finding.previous_segment_index));
            material.extend_from_slice(&be_len(finding.segment_index));
        }
        material.extend_from_slice(&be_len(self.ordering_exceptions.len()));
        for exception in &self.ordering_exceptions {
            push_str(&mut material, exception.node.as_str());
            material.extend_from_slice(&be_len(exception.segment_index));
            push_str(&mut material, exception.reason.as_str());
        }
        material.extend_from_slice(&be_len(self.placed_captures.len()));
        for frame_seq in &self.placed_captures {
            material.extend_from_slice(&frame_seq.to_be_bytes());
        }
        material.extend_from_slice(&be_len(self.excluded_captures.len()));
        for (frame_seq, reason) in &self.excluded_captures {
            material.extend_from_slice(&frame_seq.to_be_bytes());
            push_str(&mut material, reason.as_str());
        }
        material.extend_from_slice(&be_len(self.unaccounted_captures.len()));
        for capture in &self.unaccounted_captures {
            material.extend_from_slice(&capture.frame_seq.to_be_bytes());
        }
        material.extend_from_slice(&be_len(self.gaps.len()));
        for gap in &self.gaps {
            material.extend_from_slice(&gap.from_frame_seq.to_be_bytes());
            material.extend_from_slice(&gap.to_frame_seq.to_be_bytes());
            match gap.length_nanos {
                Some(length) => {
                    material.push(1);
                    material.extend_from_slice(&length.to_be_bytes());
                }
                None => material.push(0),
            }
            material.push(u8::from(gap.explained));
        }
        material
    }

    /// The report's canonical bytes.
    ///
    /// `coverage_determinism` compares these. They are a total function of the
    /// report and of nothing else — no instant, no host, no iteration order of
    /// a hash map.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.canonical_material()
    }
}

/// Section 12.6's validator.
///
/// A unit type with one associated function. It holds no state, so two runs
/// cannot differ by what a validator remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageValidator;

impl CoverageValidator {
    /// Measures one document against the transcript it renders.
    ///
    /// # Errors
    ///
    /// A [`CoverageFault`] naming the first rule the inputs broke. The one
    /// that is not a malformed input is
    /// [`CoverageFault::SegmentHasTwoStatuses`], which is refused rather than
    /// resolved: a report that exists partitions its segments.
    pub fn validate(inputs: &CoverageInputs<'_>) -> Result<CoverageReport, CoverageFault> {
        let lineage = inputs.lineage;
        let version = inputs.version;
        if inputs.document.version() != version {
            return Err(CoverageFault::DocumentIsForAnotherVersion {
                document: inputs.document.version(),
                requested: version,
            });
        }
        if inputs.document.lecture() != lineage.raw().lecture() {
            return Err(CoverageFault::LectureMismatch);
        }
        if inputs.document.transcript_token_digest() != &lineage.raw().token_sequence_digest() {
            return Err(CoverageFault::DocumentIsForAnotherTranscript);
        }

        // The eligible set. Not an argument: `0..` over the lineage until it
        // says there is no more.
        let mut eligible = Vec::new();
        let mut index = 0_usize;
        while let Some(segment) = lineage.segment_at(version, index) {
            eligible.push((
                index,
                segment.id().to_owned(),
                segment.tokens().len(),
                token_spans(&segment)?.len(),
            ));
            index = index.saturating_add(1);
        }
        if eligible.is_empty() {
            return Err(CoverageFault::NoSuchVersion(version));
        }

        let mut accounts = Vec::new();
        let mut unmapped = Vec::new();
        let mut mapped_segments = 0_u64;
        let mut mapped_tokens = 0_u64;
        let mut total_tokens = 0_u64;
        for (segment_index, segment_id, token_count, _aligned) in &eligible {
            total_tokens = total_tokens.saturating_add(*token_count as u64);
            let nodes: Vec<NodeId> = inputs
                .document
                .nodes()
                .iter()
                .filter(|node| {
                    node.mappings()
                        .iter()
                        .any(|mapping| mapping.segment_index() == *segment_index)
                })
                .map(|node| node.id().clone())
                .collect();
            let declared = inputs.dispositions.for_segment(*segment_index);
            // Four arms, and the compiler enumerates them.
            match (nodes.is_empty(), declared) {
                (false, None) => {
                    let mut covered: Vec<usize> = inputs
                        .document
                        .nodes()
                        .iter()
                        .flat_map(|node| node.mappings())
                        .filter(|mapping| mapping.segment_index() == *segment_index)
                        .flat_map(|mapping| mapping.covered_tokens().iter().copied())
                        .collect();
                    covered.sort_unstable();
                    covered.dedup();
                    mapped_segments = mapped_segments.saturating_add(1);
                    mapped_tokens = mapped_tokens.saturating_add(covered.len() as u64);
                    accounts.push(SegmentAccount {
                        segment_index: *segment_index,
                        segment_id: segment_id.clone(),
                        token_count: *token_count,
                        status: SegmentStatus::Mapped { nodes },
                    });
                }
                (true, Some(disposition)) => {
                    accounts.push(SegmentAccount {
                        segment_index: *segment_index,
                        segment_id: segment_id.clone(),
                        token_count: *token_count,
                        status: disposition.status().clone(),
                    });
                }
                (false, Some(_)) => {
                    return Err(CoverageFault::SegmentHasTwoStatuses {
                        segment_index: *segment_index,
                    });
                }
                (true, None) => {
                    unmapped.push(UnmappedSegment {
                        segment_index: *segment_index,
                        segment_id: segment_id.clone(),
                        token_count: *token_count,
                    });
                }
            }
        }
        for disposition in inputs.dispositions.entries() {
            if disposition.segment_index() >= eligible.len() {
                return Err(CoverageFault::DispositionForNoSuchSegment(
                    disposition.segment_index(),
                ));
            }
        }

        // Section 12.6's segment coverage: mapped non-silence segments over all
        // eligible segments. `EXCLUDED_NON_SPEECH` is the silence, so it leaves
        // the denominator; the other two declared statuses do not, because a
        // redaction and a recording failure are content that is missing rather
        // than content that was never there.
        let non_speech = accounts
            .iter()
            .filter(|account| matches!(account.status, SegmentStatus::ExcludedNonSpeech { .. }))
            .count() as u64;
        let eligible_segments = (eligible.len() as u64).saturating_sub(non_speech);
        let non_speech_tokens: u64 = accounts
            .iter()
            .filter(|account| matches!(account.status, SegmentStatus::ExcludedNonSpeech { .. }))
            .map(|account| account.token_count as u64)
            .sum();
        let segment_coverage = Ratio {
            numerator: mapped_segments,
            denominator: eligible_segments,
        };
        let token_coverage = Ratio {
            numerator: mapped_tokens,
            denominator: total_tokens.saturating_sub(non_speech_tokens),
        };

        let (ordering_findings, ordering_exceptions) = check_ordering(inputs.document);
        let (placed_captures, excluded_captures, unaccounted_captures) =
            check_captures(inputs.document, inputs.manifest, inputs.capture_exclusions)?;
        let gaps = check_gaps(inputs.journal, inputs.config.gap_threshold_nanos());

        Ok(CoverageReport {
            lecture: lineage.raw().lecture(),
            version,
            config: inputs.config,
            accounts,
            unmapped,
            segment_coverage,
            token_coverage,
            ordering_findings,
            ordering_exceptions,
            placed_captures,
            excluded_captures,
            unaccounted_captures,
            gaps,
            document_digest: inputs.document.digest(),
            transcript_token_digest: lineage.raw().token_sequence_digest(),
        })
    }
}

/// Section 12.6's ordering check.
///
/// Monotonic over the lowest segment each node maps, unless the node carries a
/// cross-reference naming the segment it goes back to. A cross-reference does
/// not turn the check off: the exception is recorded with its reason, and a
/// cross-reference that names a different segment than the node maps is not an
/// exception at all.
fn check_ordering(document: &LectureDocument) -> (Vec<OrderingFinding>, Vec<OrderingException>) {
    let mut findings = Vec::new();
    let mut exceptions = Vec::new();
    let mut highest: Option<usize> = None;
    for node in document.nodes() {
        let Some(first) = node.first_segment_index() else {
            continue;
        };
        if let Some(previous) = highest
            && first < previous
        {
            match node.cross_reference() {
                Some(reference) if reference.to_segment_index() == first => {
                    exceptions.push(OrderingException {
                        node: node.id().clone(),
                        segment_index: first,
                        reason: reference.reason(),
                    });
                }
                Some(_) | None => findings.push(OrderingFinding {
                    node: node.id().clone(),
                    previous_segment_index: previous,
                    segment_index: first,
                }),
            }
        }
        highest = Some(match highest {
            Some(previous) => previous.max(first),
            None => first,
        });
    }
    (findings, exceptions)
}

/// Section 12.6's capture check.
type CaptureCheck = (
    Vec<u32>,
    Vec<(u32, CaptureExclusionReason)>,
    Vec<UnaccountedCapture>,
);

fn check_captures(
    document: &LectureDocument,
    manifest: &InputManifest,
    exclusions: &CaptureExclusionLedger,
) -> Result<CaptureCheck, CoverageFault> {
    let mut placed = Vec::new();
    let mut excluded = Vec::new();
    let mut unaccounted = Vec::new();
    for capture in manifest.captures() {
        let frame_seq = capture.frame_seq();
        let is_placed = document
            .nodes()
            .iter()
            .any(|node| node.nearby_captures().contains(&frame_seq));
        match (is_placed, exclusions.for_frame(frame_seq)) {
            (true, None) => placed.push(frame_seq),
            (false, Some(exclusion)) => excluded.push((frame_seq, exclusion.reason())),
            (true, Some(_)) => {
                return Err(CoverageFault::CaptureIsPlacedAndExcluded(frame_seq));
            }
            (false, None) => unaccounted.push(UnaccountedCapture { frame_seq }),
        }
    }
    for exclusion in exclusions.entries() {
        if !manifest
            .captures()
            .iter()
            .any(|capture| capture.frame_seq() == exclusion.frame_seq())
        {
            return Err(CoverageFault::ExclusionForNoSuchCapture(
                exclusion.frame_seq(),
            ));
        }
    }
    Ok((placed, excluded, unaccounted))
}

/// Section 12.6's gap check.
///
/// `P2-L2` records a frame's session instant and not its duration, so what is
/// measurable is the elapsed distance between two consecutive audio frames. A
/// hole is explained when a `GAP` frame sits between the two, which is the
/// journal's own account of why the recording stopped rather than a caller's.
fn check_gaps(journal: &JournalRecovery, threshold_nanos: u64) -> Vec<GapFinding> {
    let mut audio: Vec<(u32, SessionClockDomain, u64)> = Vec::new();
    let mut gap_frames: Vec<u32> = Vec::new();
    for record in journal.records() {
        match record.body() {
            RecordBody::AudioChunk { .. } => {
                audio.push((
                    record.seq(),
                    record.at().domain(),
                    record.at().elapsed_nanos(),
                ));
            }
            RecordBody::Gap { .. } => gap_frames.push(record.seq()),
            _ => {}
        }
    }
    let mut findings = Vec::new();
    for pair in audio.windows(2) {
        let [
            (from_seq, from_domain, from_nanos),
            (to_seq, to_domain, to_nanos),
        ] = pair
        else {
            continue;
        };
        let length_nanos = if from_domain == to_domain {
            let length = to_nanos.saturating_sub(*from_nanos);
            if length <= threshold_nanos {
                continue;
            }
            Some(length)
        } else {
            None
        };
        let explained = gap_frames.iter().any(|seq| seq > from_seq && seq < to_seq);
        findings.push(GapFinding {
            from_frame_seq: *from_seq,
            to_frame_seq: *to_seq,
            length_nanos,
            explained,
        });
    }
    findings
}
