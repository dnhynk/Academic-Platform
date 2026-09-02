//! Model output, schema validation, provenance resolution, and quarantine.
//!
//! A model output is bytes. It becomes a [`Proposal`] only by passing two
//! checks, in this order:
//!
//! 1. **Schema.** The exact record below, no unknown key, no missing key, no
//!    trailing content. Every refusal is a [`SchemaError`] variant.
//! 2. **Provenance.** Every `support` line names an indexed document and a
//!    half-open byte range whose SHA-256 the line already carries. A range that
//!    does not resolve, or resolves to different bytes, is a [`SpanError`].
//!
//! Either refusal produces a [`QuarantinedOutput`], which holds no bytes and no
//! proposal. Quarantine is a state and not a log line: there is no method on
//! `QuarantinedOutput` that returns a `Proposal`, no `From` between them, and
//! [`ReviewQueue`] keeps the two in separate typed collections.
//!
//! ```compile_fail
//! # use academic_untrusted_content::{Proposal, QuarantinedOutput};
//! fn release(quarantined: QuarantinedOutput) -> Proposal {
//!     quarantined.into()
//! }
//! ```
//!
//! ```compile_fail
//! # use academic_untrusted_content::{Proposal, ProposalKind};
//! fn forge() -> Proposal {
//!     Proposal { kind: ProposalKind::ConceptLink, summary: todo!(), support: Vec::new() }
//! }
//! ```
//!
//! # The record
//!
//! ```text
//! academic-proposal/1
//! kind: CONCEPT_LINK
//! summary: <one line, at most 512 bytes, no control character>
//! support: <source_id> <start> <end> <truncated sha256 of [start, end)>
//! ```
//!
//! `support` repeats, at least once and at most [`MAX_SUPPORT_SPANS`] times.
//! Every other line, and any unknown key, is refused.
//!
//! # What the record cannot say
//!
//! [`ProposalKind`] is closed and has no variant naming a tool, a capability, a
//! process class, a destination, or a socket, and no other field of the record
//! is free-form except `summary`, which stays [`crate::Untrusted`]. A model
//! output therefore has nowhere to put a tool call: not because a filter removes
//! one, but because the schema has no field it would parse into.
//! `provider_response_cannot_request_a_tool_call` reads the enum through a
//! compiler-checked witness `match`, so a variant added later stops that suite
//! compiling rather than widening what a proposal may say.

use core::fmt;

use academic_egress_boundary::{AcceptedResponse, Incident};

use crate::{
    ingest::{IngestError, MAX_SOURCE_BYTES, SourceIndex},
    label::{Provenance, SourceId, SourceKind, Untrusted, digest_of},
};

/// The format identifier a model output opens with.
pub const PROPOSAL_FORMAT: &str = "academic-proposal/1";

/// The largest number of support spans one proposal may carry.
pub const MAX_SUPPORT_SPANS: usize = 32;

/// The largest summary a proposal may carry, in bytes.
pub const MAX_SUMMARY_BYTES: usize = 512;

/// How many hexadecimal characters a support line's digest carries.
///
/// This is SHA-256 truncated to its first 128 bits, and the truncation is not
/// cosmetic. `P2-G2`'s shipped rulepack refuses a run of 40 or more hexadecimal
/// characters at 3.40 bits per character as `SECRET_ENTROPY`, so a full-length
/// digest inside a provider response is quarantined by the DLP scan before this
/// boundary ever sees the record -- measured, and it is why this constant
/// exists. A 32-character run is below that rule's minimum length, and its
/// Shannon entropy cannot exceed 4.00 bits per character in any case, which is
/// below the base64url entropy rule's 4.20.
///
/// What the truncation costs is collision resistance: 128 bits rather than 256.
/// A support line also names its document and its offsets, and finding a
/// 128-bit collision is not a shape this boundary is defending against.
pub const SPAN_DIGEST_HEX_LEN: usize = 32;

/// What a proposal is about.
///
/// Closed, and deliberately narrow. No variant names a tool, a capability, a
/// process class, a destination, or a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProposalKind {
    /// Two concepts are related.
    ConceptLink,
    /// One concept is a prerequisite of another.
    PrerequisiteEdge,
    /// A course is mentioned in the cited span.
    CourseMention,
    /// The cited spans are about one topic.
    TopicSummary,
    /// The cited spans support an existing claim.
    EvidenceCitation,
}

impl ProposalKind {
    /// Exhaustive order.
    pub const ALL: [Self; 5] = [
        Self::ConceptLink,
        Self::PrerequisiteEdge,
        Self::CourseMention,
        Self::TopicSummary,
        Self::EvidenceCitation,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConceptLink => "CONCEPT_LINK",
            Self::PrerequisiteEdge => "PREREQUISITE_EDGE",
            Self::CourseMention => "COURSE_MENTION",
            Self::TopicSummary => "TOPIC_SUMMARY",
            Self::EvidenceCitation => "EVIDENCE_CITATION",
        }
    }

    /// Parses the stable spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// Why a model output failed schema validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// The first line was not [`PROPOSAL_FORMAT`].
    #[error("the output does not open with the proposal format line")]
    MissingFormatLine,
    /// The second line was not a `kind` line.
    #[error("the output has no kind line")]
    MissingKind,
    /// The `kind` value is not a [`ProposalKind`].
    #[error("the output names a kind that does not exist")]
    UnknownKind,
    /// The third line was not a `summary` line.
    #[error("the output has no summary line")]
    MissingSummary,
    /// The summary was longer than [`MAX_SUMMARY_BYTES`].
    #[error("the summary is longer than the bound")]
    SummaryTooLong,
    /// The summary held a control character.
    #[error("the summary holds a control character")]
    SummaryHasControlCharacter,
    /// No `support` line was present.
    #[error("the output cites no span")]
    NoSupport,
    /// More than [`MAX_SUPPORT_SPANS`] `support` lines were present.
    #[error("the output cites more spans than the bound")]
    TooManySupport,
    /// A `support` line was not four whitespace-separated fields, or a field
    /// was not the shape its position requires.
    #[error("a support line is malformed")]
    MalformedSupport,
    /// A line used a key the record does not define.
    #[error("the output uses a key the record does not define")]
    UnknownKey,
    /// Bytes followed the last record line.
    #[error("the output has content after its last line")]
    TrailingContent,
}

/// Why a support span did not resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SpanError {
    /// The span names a document the index does not hold.
    #[error("the span names a source that was never ingested")]
    UnknownSource,
    /// `start` was not below `end`.
    #[error("the span is empty or inverted")]
    EmptySpan,
    /// The range reached past the end of the document.
    #[error("the span reaches past the end of the source")]
    OutOfRange,
    /// An offset fell inside a UTF-8 sequence.
    #[error("a span offset is not a character boundary")]
    NotACharBoundary,
    /// The recorded digest is not the digest of the source bytes in the range.
    #[error("the span digest is not the digest of the source bytes it names")]
    DigestMismatch,
}

/// Why an output is quarantined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineReason {
    /// The output failed schema validation.
    Schema(SchemaError),
    /// The output failed provenance resolution.
    Provenance(SpanError),
    /// `P2-G2`'s provider-response scan refused the bytes. The reason code and
    /// the hit count are copied out of its `Incident`; the bytes are not.
    ProviderIncident {
        /// The closed section 3.5 reason code the incident carried.
        reason_code: String,
        /// How many canary or rule hits the incident recorded.
        hit_count: usize,
    },
}

impl fmt::Display for QuarantineReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(error) => write!(formatter, "SCHEMA: {error}"),
            Self::Provenance(error) => write!(formatter, "PROVENANCE: {error}"),
            Self::ProviderIncident {
                reason_code,
                hit_count,
            } => write!(
                formatter,
                "PROVIDER_INCIDENT: {reason_code} ({hit_count} hits)"
            ),
        }
    }
}

/// A model output that will not become a proposal.
///
/// It holds the identity of what was refused and why. It holds no output byte
/// and no [`Proposal`], and there is no method or conversion that produces one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedOutput {
    output_id: SourceId,
    digest: String,
    byte_len: usize,
    reason: QuarantineReason,
}

impl QuarantinedOutput {
    /// Which output.
    #[must_use]
    pub const fn output_id(&self) -> &SourceId {
        &self.output_id
    }

    /// Digest of the refused bytes.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Length of the refused bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Why it was refused.
    #[must_use]
    pub const fn reason(&self) -> &QuarantineReason {
        &self.reason
    }
}

/// A model output, tagged at parse time like any other ingested byte.
///
/// The field is named `source_bytes` so `tools/secret-debug-policy.test.mjs`'s
/// existing discovery net refuses a derived `Debug` over it.
pub struct ModelOutput {
    source_bytes: String,
}

impl fmt::Debug for ModelOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelOutput")
            .field(
                "source_bytes",
                &format_args!("<untrusted:{} bytes>", self.source_bytes.len()),
            )
            .finish()
    }
}

/// Tags bytes a locally run model produced.
///
/// # Errors
///
/// [`IngestError`] when the bytes are not UTF-8 or exceed the ingest bound.
pub fn ingest_model_output(
    output_id: SourceId,
    ingest_seq: u64,
    bytes: &[u8],
) -> Result<Untrusted<ModelOutput>, IngestError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(IngestError::Oversize);
    }
    let Ok(text) = core::str::from_utf8(bytes) else {
        return Err(IngestError::NotUtf8);
    };
    Ok(Untrusted::seal(
        ModelOutput {
            source_bytes: text.to_owned(),
        },
        Provenance::new(output_id, SourceKind::ProviderResponse, ingest_seq),
        bytes,
    ))
}

/// Tags a provider response `P2-G2`'s scan already accepted.
///
/// # Errors
///
/// As [`ingest_model_output`].
pub fn ingest_provider_model_output(
    output_id: SourceId,
    ingest_seq: u64,
    accepted: &AcceptedResponse,
) -> Result<Untrusted<ModelOutput>, IngestError> {
    ingest_model_output(output_id, ingest_seq, accepted.bytes())
}

/// Records `P2-G2`'s refusal of a provider response as a quarantine state.
///
/// The incident's reason code and hit count are copied; its bytes were already
/// dropped inside `accept_response`, and this crate never held them.
#[must_use]
pub fn quarantine_incident(output_id: SourceId, incident: &Incident) -> QuarantinedOutput {
    QuarantinedOutput {
        output_id,
        digest: incident.response_digest().to_owned(),
        byte_len: incident.response_byte_count(),
        reason: QuarantineReason::ProviderIncident {
            reason_code: incident.reason().as_str().to_owned(),
            hit_count: incident.hits().len(),
        },
    }
}

/// One resolved citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpan {
    source_id: SourceId,
    kind: SourceKind,
    start: usize,
    end: usize,
    digest: String,
}

impl ResolvedSpan {
    /// Which document.
    #[must_use]
    pub const fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Which kind of source.
    #[must_use]
    pub const fn kind(&self) -> SourceKind {
        self.kind
    }

    /// Inclusive start offset in the source document.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Exclusive end offset in the source document.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Digest of the source bytes in the range.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// A model output that passed schema validation and provenance resolution.
///
/// Private fields, one producer -- [`crate::adjudicate`] -- and no `Default`.
/// The summary stays [`Untrusted`]: it is still model-authored text.
#[derive(Debug)]
pub struct Proposal {
    kind: ProposalKind,
    summary: Untrusted<String>,
    support: Vec<ResolvedSpan>,
}

impl Proposal {
    /// What the proposal is about.
    #[must_use]
    pub const fn kind(&self) -> ProposalKind {
        self.kind
    }

    /// The model's own summary, still labelled.
    #[must_use]
    pub const fn summary(&self) -> &Untrusted<String> {
        &self.summary
    }

    /// Every resolved citation, in the order the output listed them.
    #[must_use]
    pub fn support(&self) -> &[ResolvedSpan] {
        &self.support
    }
}

/// A support line as the schema read it, before provenance resolution.
struct ParsedSpan {
    source_id: SourceId,
    start: usize,
    end: usize,
    digest: String,
}

/// A model output as the schema read it.
struct ParsedOutput {
    kind: ProposalKind,
    summary: String,
    support: Vec<ParsedSpan>,
}

/// Applies the record grammar to `text`.
fn parse_schema(text: &str) -> Result<ParsedOutput, SchemaError> {
    let Some(body) = text.strip_suffix('\n') else {
        return Err(SchemaError::TrailingContent);
    };
    if body.contains('\r') {
        return Err(SchemaError::TrailingContent);
    }
    let mut lines = body.split('\n');
    if lines.next() != Some(PROPOSAL_FORMAT) {
        return Err(SchemaError::MissingFormatLine);
    }
    let Some(kind_line) = lines.next() else {
        return Err(SchemaError::MissingKind);
    };
    let Some(kind_value) = kind_line.strip_prefix("kind: ") else {
        return Err(
            if kind_line.starts_with("summary: ") || kind_line.starts_with("support: ") {
                SchemaError::MissingKind
            } else {
                SchemaError::UnknownKey
            },
        );
    };
    let Some(kind) = ProposalKind::parse(kind_value) else {
        return Err(SchemaError::UnknownKind);
    };
    let Some(summary_line) = lines.next() else {
        return Err(SchemaError::MissingSummary);
    };
    let Some(summary) = summary_line.strip_prefix("summary: ") else {
        return Err(if summary_line.starts_with("support: ") {
            SchemaError::MissingSummary
        } else {
            SchemaError::UnknownKey
        });
    };
    if summary.len() > MAX_SUMMARY_BYTES {
        return Err(SchemaError::SummaryTooLong);
    }
    if summary.chars().any(char::is_control) {
        return Err(SchemaError::SummaryHasControlCharacter);
    }

    let mut support = Vec::new();
    for line in lines {
        let Some(value) = line.strip_prefix("support: ") else {
            return Err(SchemaError::UnknownKey);
        };
        support.push(parse_span(value)?);
        if support.len() > MAX_SUPPORT_SPANS {
            return Err(SchemaError::TooManySupport);
        }
    }
    if support.is_empty() {
        return Err(SchemaError::NoSupport);
    }
    Ok(ParsedOutput {
        kind,
        summary: summary.to_owned(),
        support,
    })
}

/// Applies the support-line grammar to `value`.
fn parse_span(value: &str) -> Result<ParsedSpan, SchemaError> {
    let mut fields = value.split(' ');
    let (Some(source), Some(start), Some(end), Some(digest), None) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(SchemaError::MalformedSupport);
    };
    let Ok(source_id) = SourceId::new(source) else {
        return Err(SchemaError::MalformedSupport);
    };
    let (Ok(start), Ok(end)) = (start.parse::<usize>(), end.parse::<usize>()) else {
        return Err(SchemaError::MalformedSupport);
    };
    if digest.len() != SPAN_DIGEST_HEX_LEN || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SchemaError::MalformedSupport);
    }
    if digest.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(SchemaError::MalformedSupport);
    }
    Ok(ParsedSpan {
        source_id,
        start,
        end,
        digest: digest.to_owned(),
    })
}

/// Resolves one parsed span against the index.
///
/// This is the crate's second exposure site. It reads the indexed document's
/// text so it can slice the named range and hash it. Nothing it read is
/// returned: the value it produces is a [`ResolvedSpan`], which holds offsets
/// and a digest.
fn resolve_span(index: &SourceIndex, span: &ParsedSpan) -> Result<ResolvedSpan, SpanError> {
    let Some(document) = index.get(&span.source_id) else {
        return Err(SpanError::UnknownSource);
    };
    if span.start >= span.end {
        return Err(SpanError::EmptySpan);
    }
    let text = document.expose().text();
    if span.end > text.len() {
        return Err(SpanError::OutOfRange);
    }
    if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
        return Err(SpanError::NotACharBoundary);
    }
    let Some(slice) = text.get(span.start..span.end) else {
        return Err(SpanError::OutOfRange);
    };
    let expected = digest_of(slice.as_bytes());
    if expected.get(..SPAN_DIGEST_HEX_LEN) != Some(span.digest.as_str()) {
        return Err(SpanError::DigestMismatch);
    }
    Ok(ResolvedSpan {
        source_id: span.source_id.clone(),
        kind: document.provenance().kind(),
        start: span.start,
        end: span.end,
        digest: span.digest.clone(),
    })
}

/// Turns a model output into a proposal, or quarantines it.
///
/// This is the whole path from bytes to a proposal. It takes an index and an
/// output and nothing else: no broker, no capability token, no transport, no
/// filesystem path, no ledger. `the_adjudicator_receives_no_capability` pins
/// this function and its one caller as whole text.
///
/// # Errors
///
/// A [`QuarantinedOutput`] whose reason is the first check that refused.
pub fn adjudicate(
    index: &SourceIndex,
    output: &Untrusted<ModelOutput>,
) -> Result<Proposal, QuarantinedOutput> {
    let quarantine = |reason: QuarantineReason| QuarantinedOutput {
        output_id: output.provenance().source_id().clone(),
        digest: output.digest().to_owned(),
        byte_len: output.byte_len(),
        reason,
    };
    // The third and last exposure site: the schema has to read the bytes it
    // validates. What leaves this function is a `ProposalKind`, a set of
    // offsets and digests, and a summary that is sealed again below.
    let parsed = match parse_schema(output.expose().source_bytes.as_str()) {
        Ok(parsed) => parsed,
        Err(error) => return Err(quarantine(QuarantineReason::Schema(error))),
    };
    let mut support = Vec::with_capacity(parsed.support.len());
    for span in &parsed.support {
        match resolve_span(index, span) {
            Ok(resolved) => support.push(resolved),
            Err(error) => return Err(quarantine(QuarantineReason::Provenance(error))),
        }
    }
    let summary_bytes = parsed.summary.clone().into_bytes();
    Ok(Proposal {
        kind: parsed.kind,
        summary: Untrusted::seal(parsed.summary, output.provenance().clone(), &summary_bytes),
        support,
    })
}

/// Where an adjudicated output goes.
///
/// The two outcomes are separate typed collections. Nothing moves between them:
/// [`ReviewQueue`] has no method that takes a [`QuarantinedOutput`] and produces
/// a [`Proposal`], and the collections are private.
#[derive(Debug, Default)]
pub struct ReviewQueue {
    proposals: Vec<Proposal>,
    quarantined: Vec<QuarantinedOutput>,
}

impl ReviewQueue {
    /// An empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            proposals: Vec::new(),
            quarantined: Vec::new(),
        }
    }

    /// Routes one adjudication result.
    pub fn admit(&mut self, outcome: Result<Proposal, QuarantinedOutput>) {
        match outcome {
            Ok(proposal) => self.proposals.push(proposal),
            Err(quarantined) => self.quarantined.push(quarantined),
        }
    }

    /// The proposals, in admission order.
    #[must_use]
    pub fn proposals(&self) -> &[Proposal] {
        &self.proposals
    }

    /// The quarantined outputs, in admission order.
    #[must_use]
    pub fn quarantined(&self) -> &[QuarantinedOutput] {
        &self.quarantined
    }
}
