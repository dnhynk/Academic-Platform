//! What comes back, and why it is not trusted.
//!
//! A provider response is scanned with the same rulepack that guarded the
//! outbound payload, plus a canary corpus the caller registers. A hit
//! quarantines the response: the bytes are dropped inside this function and the
//! caller receives an [`Incident`] holding digests, ranges, and rule names.
//! Nothing that could be persisted as a claim survives the refusal, which is
//! the point of returning the incident instead of the bytes.

use std::fmt;

use academic_policy::{ContentDigest, ReasonCode};

use crate::rulepack::Rulepack;

/// Synthetic tokens that must never come back from a provider.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanaryCorpus {
    tokens: Vec<String>,
}

impl CanaryCorpus {
    /// A corpus of exact tokens, sorted and deduplicated.
    #[must_use]
    pub fn new(tokens: Vec<String>) -> Self {
        let mut tokens = tokens;
        tokens.retain(|token| !token.is_empty());
        tokens.sort();
        tokens.dedup();
        Self { tokens }
    }

    /// Number of registered canaries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Whether the corpus is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

/// What matched inside a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitSource {
    /// A registered canary token, named by digest rather than by value.
    Canary {
        /// Digest of the canary that matched.
        canary_digest: String,
    },
    /// A rulepack rule.
    Rule {
        /// Rule identifier.
        rule_id: &'static str,
    },
}

/// One hit inside a quarantined response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanaryHit {
    /// What matched.
    pub source: HitSource,
    /// Inclusive start offset in the response.
    pub start: usize,
    /// Exclusive end offset in the response.
    pub end: usize,
}

/// Severity of a quarantine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentSeverity {
    /// A secret-shaped token or a registered canary came back from a provider.
    High,
}

/// A quarantined response. It carries no response byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incident {
    reason: ReasonCode,
    severity: IncidentSeverity,
    response_digest: String,
    response_byte_count: usize,
    hits: Vec<CanaryHit>,
    detail: String,
}

impl Incident {
    /// The closed section 3.5 reason code.
    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }

    /// Severity. A provider response canary is always high.
    #[must_use]
    pub const fn severity(&self) -> IncidentSeverity {
        self.severity
    }

    /// Digest of the quarantined response.
    #[must_use]
    pub fn response_digest(&self) -> &str {
        &self.response_digest
    }

    /// Exact length of the quarantined response.
    #[must_use]
    pub const fn response_byte_count(&self) -> usize {
        self.response_byte_count
    }

    /// Every hit, in offset order.
    #[must_use]
    pub fn hits(&self) -> &[CanaryHit] {
        &self.hits
    }

    /// Why, in words.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for Incident {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} ({} hits)",
            self.reason.as_str(),
            self.detail,
            self.hits.len()
        )
    }
}

impl std::error::Error for Incident {}

/// A response that passed the canary and rulepack scan.
pub struct AcceptedResponse {
    payload: Vec<u8>,
    digest: String,
}

impl AcceptedResponse {
    /// The response bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.payload
    }

    /// Digest of the response bytes.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl fmt::Debug for AcceptedResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceptedResponse")
            .field(
                "payload",
                &format_args!("<redacted:{} bytes>", self.payload.len()),
            )
            .field("digest", &self.digest)
            .finish()
    }
}

/// Scans a provider response and either accepts it or quarantines it.
///
/// The canary corpus is matched first so a registered token is reported as a
/// canary rather than as whichever rule happens to also cover it. A scanner
/// failure quarantines too: an unscanned response is not a clean one.
pub(crate) fn accept_response(
    rulepack: &Rulepack,
    corpus: &CanaryCorpus,
    response: &[u8],
) -> Result<AcceptedResponse, Incident> {
    let digest = ContentDigest::of(response).as_str().to_owned();
    let quarantine = |reason: ReasonCode, detail: String, hits: Vec<CanaryHit>| Incident {
        reason,
        severity: IncidentSeverity::High,
        response_digest: digest.clone(),
        response_byte_count: response.len(),
        hits,
        detail,
    };

    let Ok(text) = core::str::from_utf8(response) else {
        return Err(quarantine(
            ReasonCode::UnknownBinary,
            "the provider response is not UTF-8 text".to_owned(),
            Vec::new(),
        ));
    };

    let mut hits = Vec::new();
    for token in &corpus.tokens {
        let mut base = 0_usize;
        while let Some(offset) = text.get(base..).and_then(|tail| tail.find(token.as_str())) {
            let start = base.saturating_add(offset);
            let end = start.saturating_add(token.len());
            hits.push(CanaryHit {
                source: HitSource::Canary {
                    canary_digest: ContentDigest::of(token.as_bytes()).as_str().to_owned(),
                },
                start,
                end,
            });
            base = end;
        }
    }

    match rulepack.scan(text) {
        Err(error) => {
            return Err(quarantine(
                ReasonCode::ScannerError,
                format!(
                    "response rule {} could not complete: {}",
                    error.rule_id, error.detail
                ),
                hits,
            ));
        }
        Ok(findings) => {
            for finding in findings {
                hits.push(CanaryHit {
                    source: HitSource::Rule {
                        rule_id: finding.rule_id(),
                    },
                    start: finding.start(),
                    end: finding.end(),
                });
            }
        }
    }

    if hits.is_empty() {
        return Ok(AcceptedResponse {
            payload: response.to_vec(),
            digest,
        });
    }
    hits.sort_by_key(|hit| (hit.start, hit.end));
    let count = hits.len();
    Err(quarantine(
        ReasonCode::CanaryInResponse,
        format!("the provider response matched {count} canary or rule patterns"),
        hits,
    ))
}
