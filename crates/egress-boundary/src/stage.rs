//! Staging: the one place a transmittable byte comes into existence.
//!
//! `stage` is a total function from a source document to either a
//! [`StagedPayload`] or an [`EgressDenial`]. Every refusal returns the denial
//! arm, and the denial arm carries no bytes, so a caller holding a denial has
//! nothing it could send. That is what "every failure mode emits zero bytes"
//! means physically here: the bytes are not withheld from the caller, they were
//! never constructed.
//!
//! The staged bytes are built exactly once, by [`substitute_identifiers`], and
//! stored in the [`Preview`] that `StagedPayload` owns. The preview the user
//! reads and the payload the transport writes are the same buffer, not two
//! computations of one intent.

use std::collections::BTreeMap;
use std::fmt;

use academic_policy::{ContentDigest, ObjectRange, ReasonCode};

use crate::minimize::{self, ClassificationError, SourceRange};
use crate::rulepack::{Finding, Rulepack, RulepackId, ScanError};

/// A private document a request wants part of.
pub struct SourceDocument {
    object_id: String,
    payload: Vec<u8>,
}

impl SourceDocument {
    /// Wraps bytes with the object identifier the broker's ranges use.
    #[must_use]
    pub fn new(object_id: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            object_id: object_id.into(),
            payload: payload.into(),
        }
    }

    /// Stable object identifier.
    #[must_use]
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Exact source length.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.payload.len()
    }
}

impl fmt::Debug for SourceDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceDocument")
            .field("object_id", &self.object_id)
            .field(
                "payload",
                &format_args!("<redacted:{} bytes>", self.payload.len()),
            )
            .finish()
    }
}

/// One identifier the redaction policy replaced, with both ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Substitution {
    original: String,
    placeholder: String,
    source_range: SourceRange,
    staged_start: usize,
    staged_end: usize,
}

impl Substitution {
    /// The private identifier that was replaced.
    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }

    /// What the provider sees in its place.
    #[must_use]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Where the original sat in the source document.
    #[must_use]
    pub const fn source_range(&self) -> SourceRange {
        self.source_range
    }

    /// Inclusive start of the placeholder in the staged bytes.
    #[must_use]
    pub const fn staged_start(&self) -> usize {
        self.staged_start
    }

    /// Exclusive end of the placeholder in the staged bytes.
    #[must_use]
    pub const fn staged_end(&self) -> usize {
        self.staged_end
    }
}

/// The byte-accurate preview, and the only copy of the staged bytes.
pub struct Preview {
    payload: Vec<u8>,
    source_ranges: Vec<SourceRange>,
    substitutions: Vec<Substitution>,
    rulepack: RulepackId,
}

impl Preview {
    /// The exact bytes that will be transmitted.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.payload
    }

    /// Length of the staged bytes.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.payload.len()
    }

    /// Digest of the staged bytes.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::of(&self.payload)
    }

    /// Source ranges the staged bytes were built from, in document order.
    #[must_use]
    pub fn source_ranges(&self) -> &[SourceRange] {
        &self.source_ranges
    }

    /// Every identifier substitution, in staged order.
    #[must_use]
    pub fn substitutions(&self) -> &[Substitution] {
        &self.substitutions
    }

    /// Rulepack that produced this preview.
    #[must_use]
    pub const fn rulepack(&self) -> &RulepackId {
        &self.rulepack
    }

    /// A human rendering: exact ranges, exact length, and every substitution.
    ///
    /// The staged bytes themselves are not rendered. The caller already holds
    /// them through [`Preview::bytes`]; a second textual copy inside a report
    /// is what ends up in a log.
    #[must_use]
    pub fn render(&self) -> String {
        let mut report = format!(
            "rulepack {} staged {} bytes digest {}\n",
            self.rulepack.label(),
            self.payload.len(),
            self.digest().as_str()
        );
        for range in &self.source_ranges {
            report.push_str(&format!(
                "source [{}, {}) {} bytes\n",
                range.start(),
                range.end(),
                range.len()
            ));
        }
        for substitution in &self.substitutions {
            report.push_str(&format!(
                "substituted {} -> {} source [{}, {}) staged [{}, {})\n",
                substitution.original,
                substitution.placeholder,
                substitution.source_range.start(),
                substitution.source_range.end(),
                substitution.staged_start,
                substitution.staged_end
            ));
        }
        report
    }
}

impl fmt::Debug for Preview {
    /// Redacting, and deliberately partial.
    ///
    /// The rulepack identity is omitted rather than printed: it holds a digest,
    /// and `tools/secret-debug-policy.test.mjs` allows a redacting formatter to
    /// reach a field of a secret-bearing type only through a length. What a
    /// reader wants from a preview is [`Preview::render`], which names the
    /// rulepack, the ranges, and every substitution.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Preview")
            .field(
                "payload",
                &format_args!("<redacted:{} bytes>", self.payload.len()),
            )
            .field("source_ranges", &self.source_ranges)
            .field("substitution_count", &self.substitutions.len())
            .finish_non_exhaustive()
    }
}

/// A staged payload: a preview, and the grant scope it must be sent under.
#[derive(Debug)]
pub struct StagedPayload {
    staged_object_id: String,
    preview: Preview,
}

impl StagedPayload {
    /// The byte-accurate preview. The transport reads its bytes and no others.
    #[must_use]
    pub const fn preview(&self) -> &Preview {
        &self.preview
    }

    /// Identifier the grant's byte range names.
    #[must_use]
    pub fn staged_object_id(&self) -> &str {
        &self.staged_object_id
    }

    /// The exact range a grant for this payload must cover.
    ///
    /// The digest is the preview's, so a grant minted from this range is a
    /// grant over the previewed bytes. The broker recomputes it at the
    /// capability boundary, which is the second, independent reason a
    /// transmission that is not the preview cannot leave.
    pub fn object_range(&self) -> Result<ObjectRange, academic_policy::BrokerError> {
        ObjectRange::new(
            self.staged_object_id.clone(),
            0,
            u64::try_from(self.preview.payload.len()).unwrap_or(u64::MAX),
            self.preview.digest(),
        )
    }
}

/// Which identifiers the redaction policy replaces, and how much loss is too much.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierPolicy {
    substitute: Vec<String>,
    max_substituted_percent: u32,
}

impl IdentifierPolicy {
    /// A policy replacing exactly the named identifiers.
    ///
    /// `max_substituted_percent` bounds how much of the slice's non-whitespace
    /// bytes may be covered by substitutions before the slice is judged to have
    /// lost its meaning.
    #[must_use]
    pub fn new(substitute: Vec<String>, max_substituted_percent: u32) -> Self {
        let mut substitute = substitute;
        substitute.sort();
        substitute.dedup();
        Self {
            substitute,
            max_substituted_percent,
        }
    }

    /// A policy that substitutes nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            substitute: Vec::new(),
            max_substituted_percent: 100,
        }
    }

    /// Identifiers this policy replaces, sorted and deduplicated.
    #[must_use]
    pub fn substituted(&self) -> &[String] {
        &self.substitute
    }
}

/// Everything one staging decision reads.
#[derive(Debug)]
pub struct StagingRequest<'a> {
    /// The private document.
    pub document: &'a SourceDocument,
    /// Symbols the request needs. An empty list is refused.
    pub focus: &'a [String],
    /// Which identifiers to substitute, and the meaning-loss bound.
    pub identifier_policy: &'a IdentifierPolicy,
    /// Largest payload this destination accepts, from the provider registry.
    pub max_bytes: u64,
}

/// A refusal, with the closed reason code and the evidence behind it.
///
/// It holds no bytes. That is deliberate: a denial that carried the payload it
/// refused would be a byte-emitting failure path wearing an error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EgressDenial {
    reason: ReasonCode,
    detail: String,
    findings: Vec<Finding>,
    bytes_transmitted: usize,
}

impl EgressDenial {
    pub(crate) fn new(reason: ReasonCode, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
            findings: Vec::new(),
            bytes_transmitted: 0,
        }
    }

    pub(crate) fn with_findings(
        reason: ReasonCode,
        detail: impl Into<String>,
        findings: Vec<Finding>,
    ) -> Self {
        Self {
            reason,
            detail: detail.into(),
            findings,
            bytes_transmitted: 0,
        }
    }

    pub(crate) fn aborted(reason: ReasonCode, detail: impl Into<String>, sent: usize) -> Self {
        Self {
            reason,
            detail: detail.into(),
            findings: Vec::new(),
            bytes_transmitted: sent,
        }
    }

    /// Bytes already handed to the transport before the refusal.
    ///
    /// Zero for every staging refusal, because staging refuses before a byte
    /// exists. Non-zero only for a transfer aborted after the capability
    /// boundary, which is fault `EG04`.
    #[must_use]
    pub const fn bytes_transmitted(&self) -> usize {
        self.bytes_transmitted
    }

    /// Where the work goes after this refusal.
    ///
    /// Every refusal routes the same way. There is no reason code that routes
    /// to a retry, a downgrade, or a different provider, so this is a constant
    /// of the denial rather than a decision made from one.
    #[must_use]
    pub const fn route(&self) -> crate::Route {
        crate::Route::LocalOnlyOrStop
    }

    /// The closed section 3.5 reason code.
    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }

    /// Why, in words, for the local user.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Scanner findings behind the refusal, when the refusal came from a scan.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

impl fmt::Display for EgressDenial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason.as_str(), self.detail)
    }
}

impl std::error::Error for EgressDenial {}

/// One redaction pass: the bytes it produced and what it replaced.
///
/// Private, and deliberately without a `Debug`: it holds the staged bytes
/// before they reach the preview that redacts them in its own formatter, and a
/// derived `Debug` here would print them.
struct Redaction {
    payload: Vec<u8>,
    substitutions: Vec<Substitution>,
    substituted_source_bytes: usize,
}

/// Builds the staged bytes. This is the only construction site.
///
/// The pass is a single left-to-right walk of the selected source ranges. Each
/// whole-word occurrence of a policy identifier becomes a stable placeholder
/// numbered in first-appearance order, so the same slice and the same policy
/// always produce the same bytes.
fn substitute_identifiers(
    text: &str,
    ranges: &[SourceRange],
    policy: &IdentifierPolicy,
) -> Option<Redaction> {
    let mut payload: Vec<u8> = Vec::new();
    let mut substitutions = Vec::new();
    let mut assigned: BTreeMap<String, String> = BTreeMap::new();
    let mut substituted_source_bytes = 0_usize;
    for (index, range) in ranges.iter().enumerate() {
        if index > 0 {
            payload.push(b'\n');
        }
        let slice = text.get(range.start()..range.end())?;
        let bytes = slice.as_bytes();
        let mut cursor = 0_usize;
        while cursor < bytes.len() {
            let word_end = identifier_end(bytes, cursor);
            if word_end == cursor {
                payload.push(bytes[cursor]);
                cursor = cursor.saturating_add(1);
                continue;
            }
            let word = slice.get(cursor..word_end)?;
            if policy.substitute.iter().any(|name| name == word) {
                let next = assigned.len().saturating_add(1);
                let placeholder = assigned
                    .entry(word.to_owned())
                    .or_insert_with(|| format!("IDENT_{next}"))
                    .clone();
                let staged_start = payload.len();
                payload.extend_from_slice(placeholder.as_bytes());
                substitutions.push(Substitution {
                    original: word.to_owned(),
                    placeholder,
                    source_range: SourceRange::new(
                        range.start().saturating_add(cursor),
                        range.start().saturating_add(word_end),
                    ),
                    staged_start,
                    staged_end: payload.len(),
                });
                substituted_source_bytes =
                    substituted_source_bytes.saturating_add(word_end.saturating_sub(cursor));
            } else {
                payload.extend_from_slice(&bytes[cursor..word_end]);
            }
            cursor = word_end;
        }
    }
    Some(Redaction {
        payload,
        substitutions,
        substituted_source_bytes,
    })
}

fn identifier_end(bytes: &[u8], from: usize) -> usize {
    let starts = bytes
        .get(from)
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_');
    if !starts {
        return from;
    }
    let mut end = from;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end = end.saturating_add(1);
    }
    end
}

/// Runs the whole staging pipeline.
///
/// The order is fixed and each step's refusal is final:
///
/// 1. source size, so an oversize container is refused as oversize rather than
///    as an unreadable one;
/// 2. classification, because a scanner cannot answer for bytes it cannot read;
/// 3. minimization, so the scan and the payload cover only what was asked for;
/// 4. the scan, which denies on any finding and on any scanner failure;
/// 5. redaction, which is refused when it destroys the slice's meaning;
/// 6. a second scan of the redacted bytes, so a substitution can neither
///    introduce nor preserve a finding; and
/// 7. the staged size, because a substitution can make a payload longer.
pub(crate) fn stage(
    rulepack: &Rulepack,
    request: &StagingRequest<'_>,
) -> Result<StagedPayload, EgressDenial> {
    let source_len = u64::try_from(request.document.payload.len()).unwrap_or(u64::MAX);
    if source_len > request.max_bytes {
        return Err(EgressDenial::new(
            ReasonCode::Oversize,
            format!(
                "source is {source_len} bytes, over the {} byte destination bound",
                request.max_bytes
            ),
        ));
    }
    let text = minimize::classify(&request.document.payload).map_err(|error| {
        let detail = match error {
            ClassificationError::NotUtf8 => "payload is not UTF-8 text".to_owned(),
            ClassificationError::ContainerMagic(name) => format!("payload is a {name}"),
            ClassificationError::ControlByte => {
                "payload holds a control byte no source text uses".to_owned()
            }
        };
        EgressDenial::new(ReasonCode::UnknownBinary, detail)
    })?;
    let ranges = minimize::minimal_ranges(text, request.focus).ok_or_else(|| {
        EgressDenial::new(
            ReasonCode::ScopeMismatch,
            "the document declares no item for a requested symbol",
        )
    })?;
    let selected = concatenate(text, &ranges).ok_or_else(range_off_document)?;
    deny_on_findings(rulepack, &selected, "source slice")?;

    let redaction = substitute_identifiers(text, &ranges, request.identifier_policy)
        .ok_or_else(range_off_document)?;
    meaning_check(&selected, &redaction, request)?;

    let staged_text = String::from_utf8(redaction.payload).map_err(|_| {
        EgressDenial::new(
            ReasonCode::UnknownBinary,
            "redaction produced bytes that are not UTF-8 text",
        )
    })?;
    deny_on_findings(rulepack, &staged_text, "redacted slice")?;

    let staged_len = u64::try_from(staged_text.len()).unwrap_or(u64::MAX);
    if staged_len > request.max_bytes {
        return Err(EgressDenial::new(
            ReasonCode::Oversize,
            format!(
                "redacted payload is {staged_len} bytes, over the {} byte destination bound",
                request.max_bytes
            ),
        ));
    }

    let rulepack_id = rulepack.id();
    let preview = Preview {
        payload: staged_text.into_bytes(),
        source_ranges: ranges,
        substitutions: redaction.substitutions,
        rulepack: rulepack_id.clone(),
    };
    let staged_object_id = staged_object_id(request.document.object_id(), &preview, &rulepack_id);
    Ok(StagedPayload {
        staged_object_id,
        preview,
    })
}

/// The one refusal for a range that is not on the document it came from.
fn range_off_document() -> EgressDenial {
    EgressDenial::new(
        ReasonCode::ScopeMismatch,
        "a minimized range does not lie on the document",
    )
}

/// Joins the selected ranges. `None` when a range is not on the text.
///
/// The empty-string fallback this used to have was a fail-open shape: the scan
/// reads this join and the payload is built from the same ranges, so a silently
/// dropped range would have been scanned as absent and staged as present.
fn concatenate(text: &str, ranges: &[SourceRange]) -> Option<String> {
    let mut joined = String::new();
    for (index, range) in ranges.iter().enumerate() {
        if index > 0 {
            joined.push('\n');
        }
        joined.push_str(text.get(range.start()..range.end())?);
    }
    Some(joined)
}

/// Denies on a scanner failure and on every finding.
///
/// Both arms return `Err`. There is no arm that logs and continues, and no arm
/// that treats an empty finding list produced by a failed rule as clean,
/// because `scan` has no partial result to return.
fn deny_on_findings(rulepack: &Rulepack, text: &str, stage: &str) -> Result<(), EgressDenial> {
    let findings = rulepack.scan(text).map_err(|error: ScanError| {
        EgressDenial::new(
            ReasonCode::ScannerError,
            format!(
                "{stage}: rule {} could not complete: {}",
                error.rule_id, error.detail
            ),
        )
    })?;
    let Some(first) = findings.first() else {
        return Ok(());
    };
    Err(EgressDenial::with_findings(
        first.reason(),
        format!(
            "{stage}: rule {} matched bytes [{}, {}) in a {} span",
            first.rule_id(),
            first.start(),
            first.end(),
            first.span_kind().as_str()
        ),
        findings,
    ))
}

/// Refuses a redaction that removed what the request was about.
///
/// Two conditions, both exact. Substituting a focus symbol renames the thing
/// the question is about, and a slice whose substituted share passes the policy
/// bound is no longer the slice that was reviewed.
fn meaning_check(
    selected: &str,
    redaction: &Redaction,
    request: &StagingRequest<'_>,
) -> Result<(), EgressDenial> {
    for symbol in request.focus {
        if request
            .identifier_policy
            .substitute
            .iter()
            .any(|name| name == symbol)
        {
            return Err(EgressDenial::new(
                ReasonCode::RedactionDestroysMeaning,
                format!("the redaction policy substitutes the requested symbol {symbol}"),
            ));
        }
    }
    let significant = selected
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .count();
    if significant == 0 {
        return Err(EgressDenial::new(
            ReasonCode::RedactionDestroysMeaning,
            "the selected slice holds no significant bytes",
        ));
    }
    let percent = redaction
        .substituted_source_bytes
        .saturating_mul(100)
        .checked_div(significant)
        .unwrap_or(100);
    // A bound that does not fit `usize` becomes zero rather than a hundred: an
    // unrepresentable bound must refuse, not admit everything.
    let bound = usize::try_from(request.identifier_policy.max_substituted_percent).unwrap_or(0);
    if percent > bound {
        return Err(EgressDenial::new(
            ReasonCode::RedactionDestroysMeaning,
            format!("{percent}% of the slice was substituted, over the {bound}% bound"),
        ));
    }
    Ok(())
}

/// Names the staged artifact so a grant range cannot be reused for another one.
fn staged_object_id(source_object_id: &str, preview: &Preview, rulepack: &RulepackId) -> String {
    let mut bytes = b"academic-egress-boundary-staged-object-v1\0".to_vec();
    bytes.extend_from_slice(source_object_id.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(rulepack.redaction_policy_hash().as_str().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(preview.digest().as_str().as_bytes());
    format!("staged:{}", ContentDigest::of(&bytes).as_str())
}
