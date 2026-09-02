//! The versioned DLP rulepack: what a scan looks for and how a finding is named.
//!
//! Every grant records the rulepack identity in its `redaction_policy_hash`
//! column, so the pack that produced a staged payload is recoverable from the
//! grant row alone. The digest is over the rules rather than over the version
//! number, so a rule edit that forgets the version still moves it.
//!
//! The scanner is span-aware. A secret or a personal identifier inside a
//! comment or a string literal is the case this pack exists for: a scanner that
//! reads only code misses the fixture file and the commented-out connection
//! string, which is where they actually appear.

use academic_policy::{ContentDigest, ReasonCode};

/// Where in the source text a finding sits.
///
/// The kind is recorded rather than used to skip. A finding in a comment or a
/// string literal blocks exactly as one in code does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpanKind {
    /// Ordinary source text.
    Code,
    /// A `//` line comment, including its marker.
    LineComment,
    /// A `/* */` block comment, including its markers.
    BlockComment,
    /// A double-quoted string literal, including its quotes.
    StringLiteral,
}

impl SpanKind {
    /// Stable spelling used by the preview and by finding reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Code => "CODE",
            Self::LineComment => "LINE_COMMENT",
            Self::BlockComment => "BLOCK_COMMENT",
            Self::StringLiteral => "STRING_LITERAL",
        }
    }
}

/// Character classes a token body may draw from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alphabet {
    /// `A-Z a-z 0-9`.
    Alphanumeric,
    /// `A-Z a-z 0-9 - _`, the base64url token shape.
    Base64Url,
    /// `0-9 A-F a-f`.
    Hex,
}

impl Alphabet {
    const fn admits(self, byte: u8) -> bool {
        match self {
            Self::Alphanumeric => byte.is_ascii_alphanumeric(),
            Self::Base64Url => byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_',
            Self::Hex => byte.is_ascii_hexdigit(),
        }
    }
}

/// What one rule looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleKind {
    /// A vendor token prefix followed by a body of at least `min_body` bytes.
    TokenPrefix {
        prefix: &'static str,
        min_body: usize,
        alphabet: Alphabet,
    },
    /// A literal marker that may not appear at all, such as a PEM header.
    Marker { marker: &'static str },
    /// `key = value` where the key ends with one of `needles`.
    Assignment {
        needles: &'static [&'static str],
        min_value: usize,
    },
    /// A URI whose authority carries `user:password@`.
    CredentialUri { schemes: &'static [&'static str] },
    /// A token whose Shannon entropy per character clears `min_centibits`.
    Entropy {
        min_len: usize,
        min_centibits: u32,
        alphabet: Alphabet,
    },
    /// A `local@domain.tld` shape.
    Email,
    /// Digit groups of the given sizes joined by `separator`.
    DigitGroups {
        groups: &'static [usize],
        separator: u8,
    },
}

/// One named rule and the closed reason code a hit produces.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    id: &'static str,
    kind: RuleKind,
    reason: ReasonCode,
}

impl Rule {
    /// Stable rule identifier recorded in every finding.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Closed section 3.5 reason code this rule denies with.
    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }
}

/// One scanner hit: the rule, the exact half-open byte range, and its span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    rule_id: &'static str,
    reason: ReasonCode,
    start: usize,
    end: usize,
    span_kind: SpanKind,
}

impl Finding {
    /// Rule that produced the hit.
    #[must_use]
    pub const fn rule_id(&self) -> &'static str {
        self.rule_id
    }

    /// Closed reason code the hit denies with.
    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }

    /// Inclusive start offset into the scanned bytes.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// Exclusive end offset into the scanned bytes.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Where the hit sits in the source text.
    #[must_use]
    pub const fn span_kind(&self) -> SpanKind {
        self.span_kind
    }
}

/// A scan that could not complete.
///
/// There is no partial result. A scanner that answered for part of a payload
/// would be a fail-open path, because the caller could not tell that answer
/// from a clean one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("DLP rule {rule_id} could not complete: {detail}")]
pub struct ScanError {
    /// Rule that failed.
    pub rule_id: &'static str,
    /// Why it failed.
    pub detail: &'static str,
}

/// Versioned identity of one rulepack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulepackId {
    name: &'static str,
    version: u32,
    digest: ContentDigest,
}

impl RulepackId {
    /// Human identifier, `name/version`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}/{}", self.name, self.version)
    }

    /// Pack name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Pack version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Digest over the pack's rules, recorded as a grant's redaction policy hash.
    #[must_use]
    pub const fn redaction_policy_hash(&self) -> &ContentDigest {
        &self.digest
    }
}

/// The rule set applied before staging, and again to a provider response.
#[derive(Debug, Clone)]
pub struct Rulepack {
    name: &'static str,
    version: u32,
    rules: &'static [Rule],
    token_budget: usize,
}

/// Name of the pack shipped with this crate.
const BUILTIN_NAME: &str = "academic-dlp-rulepack";
/// Version of the shipped pack. A rule change increments it.
const BUILTIN_VERSION: u32 = 1;
/// Tokens one scan may examine before it gives up.
///
/// A scanner with no bound is a scanner that can be made to run forever by a
/// payload, so it has one. Exhausting it is a scan failure, not a clean result:
/// `scan` returns `Err` and the staging pipeline denies with `SCANNER_ERROR`.
const BUILTIN_TOKEN_BUDGET: usize = 200_000;

/// Shipped rules, in scan order.
///
/// Order fixes only which rule reports an overlapping region first; it does not
/// change whether a region is reported, because a scan runs every rule.
const BUILTIN_RULES: &[Rule] = &[
    Rule {
        id: "aws-access-key-id",
        kind: RuleKind::TokenPrefix {
            prefix: "AKIA",
            min_body: 16,
            alphabet: Alphabet::Alphanumeric,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "aws-session-key-id",
        kind: RuleKind::TokenPrefix {
            prefix: "ASIA",
            min_body: 16,
            alphabet: Alphabet::Alphanumeric,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "github-token",
        kind: RuleKind::TokenPrefix {
            prefix: "ghp_",
            min_body: 20,
            alphabet: Alphabet::Alphanumeric,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "slack-bot-token",
        kind: RuleKind::TokenPrefix {
            prefix: "xoxb-",
            min_body: 20,
            alphabet: Alphabet::Base64Url,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "google-api-key",
        kind: RuleKind::TokenPrefix {
            prefix: "AIza",
            min_body: 30,
            alphabet: Alphabet::Base64Url,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "bearer-key-prefix",
        kind: RuleKind::TokenPrefix {
            prefix: "sk-",
            min_body: 24,
            alphabet: Alphabet::Base64Url,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "pem-private-key",
        kind: RuleKind::Marker {
            marker: "-----BEGIN",
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "json-web-token",
        kind: RuleKind::TokenPrefix {
            prefix: "eyJ",
            min_body: 24,
            alphabet: Alphabet::Base64Url,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "cloud-credential-assignment",
        kind: RuleKind::Assignment {
            needles: &[
                "aws_secret_access_key",
                "accountkey",
                "client_secret",
                "gcp_service_account_key",
                "private_key_id",
            ],
            min_value: 8,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "generic-credential-assignment",
        kind: RuleKind::Assignment {
            needles: &["password", "passwd", "api_key", "apikey", "secret", "token"],
            min_value: 8,
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "credential-connection-string",
        kind: RuleKind::CredentialUri {
            schemes: &[
                "postgres://",
                "postgresql://",
                "mysql://",
                "mongodb://",
                "mongodb+srv://",
                "redis://",
                "amqp://",
            ],
        },
        reason: ReasonCode::SecretPattern,
    },
    Rule {
        id: "high-entropy-token",
        kind: RuleKind::Entropy {
            min_len: 28,
            min_centibits: 420,
            alphabet: Alphabet::Base64Url,
        },
        reason: ReasonCode::SecretEntropy,
    },
    Rule {
        id: "high-entropy-hex-token",
        kind: RuleKind::Entropy {
            min_len: 40,
            min_centibits: 340,
            alphabet: Alphabet::Hex,
        },
        reason: ReasonCode::SecretEntropy,
    },
    Rule {
        id: "email-address",
        kind: RuleKind::Email,
        reason: ReasonCode::PiiDetected,
    },
    Rule {
        id: "resident-registration-number",
        kind: RuleKind::DigitGroups {
            groups: &[6, 7],
            separator: b'-',
        },
        reason: ReasonCode::PiiDetected,
    },
    Rule {
        id: "telephone-number",
        kind: RuleKind::DigitGroups {
            groups: &[3, 4, 4],
            separator: b'-',
        },
        reason: ReasonCode::PiiDetected,
    },
    Rule {
        id: "student-number",
        kind: RuleKind::DigitGroups {
            groups: &[4, 5],
            separator: b'-',
        },
        reason: ReasonCode::PiiDetected,
    },
];

impl Default for Rulepack {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Rulepack {
    /// The rulepack shipped with this crate.
    #[must_use]
    pub const fn builtin() -> Self {
        Self {
            name: BUILTIN_NAME,
            version: BUILTIN_VERSION,
            rules: BUILTIN_RULES,
            token_budget: BUILTIN_TOKEN_BUDGET,
        }
    }

    /// The same pack with a different scan budget.
    ///
    /// Lowering the budget makes the pack refuse sooner; it can never make it
    /// accept something it would otherwise have found, because exhausting the
    /// budget is a failure and a failure denies.
    #[must_use]
    pub const fn with_token_budget(self, token_budget: usize) -> Self {
        Self {
            token_budget,
            ..self
        }
    }

    /// Tokens one scan may examine.
    #[must_use]
    pub const fn token_budget(&self) -> usize {
        self.token_budget
    }

    /// Versioned identity, including the digest recorded in every grant.
    #[must_use]
    pub fn id(&self) -> RulepackId {
        let mut bytes = b"academic-dlp-rulepack-v1\0".to_vec();
        push_str(&mut bytes, self.name);
        bytes.extend_from_slice(&self.version.to_be_bytes());
        bytes.extend_from_slice(
            &u64::try_from(self.rules.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        for rule in self.rules {
            push_str(&mut bytes, rule.id);
            push_str(&mut bytes, rule.reason.as_str());
            push_str(&mut bytes, &format!("{:?}", rule.kind));
        }
        RulepackId {
            name: self.name,
            version: self.version,
            digest: ContentDigest::of(&bytes),
        }
    }

    /// Every rule in the pack, in scan order.
    #[must_use]
    pub const fn rules(&self) -> &'static [Rule] {
        self.rules
    }

    /// Runs every rule over `text` and returns every hit, or fails whole.
    ///
    /// A rule that cannot answer stops the scan. No path returns the hits found
    /// so far, because the caller could not distinguish that from a clean
    /// payload.
    pub fn scan(&self, text: &str) -> Result<Vec<Finding>, ScanError> {
        let spans = classify_spans(text);
        let mut findings = Vec::new();
        let mut budget = Budget {
            remaining: self.token_budget,
        };
        for rule in self.rules {
            findings.extend(run_rule(rule, text, &spans, &mut budget)?);
        }
        findings.sort_by_key(|finding| (finding.start, finding.end, finding.rule_id));
        findings.dedup();
        Ok(findings)
    }
}

fn push_str(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

/// Half-open span with its kind, covering the whole text without gaps.
#[derive(Debug, Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
    kind: SpanKind,
}

/// Splits `text` into code, comment, and string-literal spans.
///
/// The lexer is deliberately small: it recognizes `//`, `/* */`, and
/// double-quoted strings with backslash escapes. It is not a Rust parser and
/// does not claim to be one. What it must get right is that a comment or a
/// literal is still scanned, and that the kind reported for a hit is the kind
/// the reader will see.
fn classify_spans(text: &str) -> Vec<Span> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut cursor = 0_usize;
    let mut plain_start = 0_usize;
    while cursor < bytes.len() {
        let two = bytes.get(cursor..cursor.saturating_add(2));
        let kind = match (bytes[cursor], two) {
            (b'/', Some(b"//")) => SpanKind::LineComment,
            (b'/', Some(b"/*")) => SpanKind::BlockComment,
            (b'"', _) => SpanKind::StringLiteral,
            _ => {
                cursor = cursor.saturating_add(1);
                continue;
            }
        };
        let body_start = cursor;
        if plain_start < body_start {
            spans.push(Span {
                start: plain_start,
                end: body_start,
                kind: SpanKind::Code,
            });
        }
        let end = match kind {
            SpanKind::LineComment => text[body_start..]
                .find('\n')
                .map_or(bytes.len(), |offset| body_start.saturating_add(offset)),
            SpanKind::BlockComment => text[body_start.saturating_add(2)..]
                .find("*/")
                .map_or(bytes.len(), |offset| {
                    body_start.saturating_add(4).saturating_add(offset)
                }),
            SpanKind::StringLiteral => string_literal_end(bytes, body_start),
            SpanKind::Code => body_start,
        };
        spans.push(Span {
            start: body_start,
            end,
            kind,
        });
        cursor = end.max(body_start.saturating_add(1));
        plain_start = cursor;
    }
    if plain_start < bytes.len() {
        spans.push(Span {
            start: plain_start,
            end: bytes.len(),
            kind: SpanKind::Code,
        });
    }
    spans
}

fn string_literal_end(bytes: &[u8], open: usize) -> usize {
    let mut cursor = open.saturating_add(1);
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = cursor.saturating_add(2),
            b'"' => return cursor.saturating_add(1),
            b'\n' => return cursor,
            _ => cursor = cursor.saturating_add(1),
        }
    }
    bytes.len()
}

fn span_kind_at(spans: &[Span], offset: usize) -> SpanKind {
    spans
        .iter()
        .find(|span| span.start <= offset && offset < span.end)
        .map_or(SpanKind::Code, |span| span.kind)
}

/// Remaining tokens one scan may examine.
#[derive(Debug)]
struct Budget {
    remaining: usize,
}

impl Budget {
    /// Charges one token, or reports the scan as unfinishable.
    fn charge(&mut self, rule_id: &'static str) -> Result<(), ScanError> {
        match self.remaining.checked_sub(1) {
            Some(remaining) => {
                self.remaining = remaining;
                Ok(())
            }
            None => Err(ScanError {
                rule_id,
                detail: "scan token budget exhausted before every rule had answered",
            }),
        }
    }
}

fn run_rule(
    rule: &Rule,
    text: &str,
    spans: &[Span],
    budget: &mut Budget,
) -> Result<Vec<Finding>, ScanError> {
    budget.charge(rule.id)?;
    let bytes = text.as_bytes();
    let hits = match rule.kind {
        RuleKind::TokenPrefix {
            prefix,
            min_body,
            alphabet,
        } => token_prefix_hits(bytes, prefix, min_body, alphabet),
        RuleKind::Marker { marker } => marker_hits(text, marker),
        RuleKind::Assignment { needles, min_value } => assignment_hits(bytes, needles, min_value),
        RuleKind::CredentialUri { schemes } => credential_uri_hits(text, schemes),
        RuleKind::Entropy {
            min_len,
            min_centibits,
            alphabet,
        } => entropy_hits(bytes, min_len, min_centibits, alphabet, rule.id, budget)?,
        RuleKind::Email => email_hits(bytes),
        RuleKind::DigitGroups { groups, separator } => digit_group_hits(bytes, groups, separator),
    };
    Ok(hits
        .into_iter()
        .map(|(start, end)| Finding {
            rule_id: rule.id,
            reason: rule.reason,
            start,
            end,
            span_kind: span_kind_at(spans, start),
        })
        .collect())
}

fn token_body_end(bytes: &[u8], from: usize, alphabet: Alphabet) -> usize {
    let mut end = from;
    while end < bytes.len() && alphabet.admits(bytes[end]) {
        end = end.saturating_add(1);
    }
    end
}

fn token_prefix_hits(
    bytes: &[u8],
    prefix: &str,
    min_body: usize,
    alphabet: Alphabet,
) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    let needle = prefix.as_bytes();
    let mut cursor = 0_usize;
    while cursor.saturating_add(needle.len()) <= bytes.len() {
        if bytes.get(cursor..cursor.saturating_add(needle.len())) == Some(needle) {
            let body_start = cursor.saturating_add(needle.len());
            let end = token_body_end(bytes, body_start, alphabet);
            if end.saturating_sub(body_start) >= min_body {
                hits.push((cursor, end));
                cursor = end;
                continue;
            }
        }
        cursor = cursor.saturating_add(1);
    }
    hits
}

fn marker_hits(text: &str, marker: &str) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    let mut base = 0_usize;
    while let Some(offset) = text.get(base..).and_then(|tail| tail.find(marker)) {
        let start = base.saturating_add(offset);
        let end = start.saturating_add(marker.len());
        hits.push((start, end));
        base = end;
    }
    hits
}

fn assignment_hits(bytes: &[u8], needles: &[&str], min_value: usize) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'=' && *byte != b':' {
            continue;
        }
        let key_end = index;
        let mut key_start = key_end;
        while key_start > 0 {
            let candidate = key_start.saturating_sub(1);
            let previous = bytes[candidate];
            if previous.is_ascii_alphanumeric() || matches!(previous, b'_' | b'-' | b'.' | b' ') {
                key_start = candidate;
            } else {
                break;
            }
        }
        // The key is read byte-wise rather than through a UTF-8 conversion. A
        // conversion has a failure arm, and the only thing that arm could do
        // here is skip the assignment -- which is a rule declining to answer,
        // which is what this scanner may not do. Every needle is ASCII, so a
        // byte-wise lowercase compares exactly.
        let Some(raw_key) = bytes.get(key_start..key_end) else {
            continue;
        };
        let trimmed = raw_key
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(raw_key.len());
        let key: String = raw_key
            .get(trimmed..)
            .unwrap_or_default()
            .iter()
            .map(|byte| char::from(byte.to_ascii_lowercase()))
            .collect();
        let key = key.trim_end().to_owned();
        if key.is_empty() || !needles.iter().any(|needle| key.ends_with(needle)) {
            continue;
        }
        let mut value_start = index.saturating_add(1);
        while value_start < bytes.len() && matches!(bytes[value_start], b' ' | b'"' | b'\'' | b'\t')
        {
            value_start = value_start.saturating_add(1);
        }
        let mut value_end = value_start;
        while value_end < bytes.len()
            && !matches!(
                bytes[value_end],
                b'\n' | b'\r' | b'"' | b'\'' | b';' | b' ' | b','
            )
        {
            value_end = value_end.saturating_add(1);
        }
        if value_end.saturating_sub(value_start) >= min_value {
            hits.push((key_start.saturating_add(trimmed), value_end));
        }
    }
    hits
}

fn credential_uri_hits(text: &str, schemes: &[&str]) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    for scheme in schemes {
        for (start, _) in marker_hits(text, scheme) {
            let authority_start = start.saturating_add(scheme.len());
            let Some(tail) = text.get(authority_start..) else {
                continue;
            };
            let authority_end = tail
                .find(|character: char| character.is_whitespace() || character == '"')
                .map_or(text.len(), |offset| authority_start.saturating_add(offset));
            let Some(authority) = text.get(authority_start..authority_end) else {
                continue;
            };
            let Some(at) = authority.find('@') else {
                continue;
            };
            if authority.get(..at).is_some_and(|user| user.contains(':')) {
                hits.push((start, authority_end));
            }
        }
    }
    hits
}

/// Shannon entropy of `token`, in centibits per character.
fn entropy_centibits(token: &[u8], rule_id: &'static str) -> Result<u32, ScanError> {
    if token.is_empty() {
        return Err(ScanError {
            rule_id,
            detail: "entropy of an empty token is undefined",
        });
    }
    let mut counts = [0_u32; 256];
    for byte in token {
        let slot = usize::from(*byte);
        counts[slot] = counts[slot].saturating_add(1);
    }
    // A token too long to count exactly is a token this rule cannot answer for.
    // Saturating the length would drive every probability towards zero and
    // report high-entropy bytes as low-entropy ones, which is the one default
    // in this file that would let something through.
    let Ok(counted) = u32::try_from(token.len()) else {
        return Err(ScanError {
            rule_id,
            detail: "token is longer than the entropy rule can measure",
        });
    };
    let length = f64::from(counted);
    let mut bits = 0.0_f64;
    for count in counts {
        if count == 0 {
            continue;
        }
        let probability = f64::from(count) / length;
        bits -= probability * probability.log2();
    }
    let centibits = (bits * 100.0).round();
    if !centibits.is_finite() || centibits < 0.0 {
        return Err(ScanError {
            rule_id,
            detail: "entropy did not converge to a finite value",
        });
    }
    Ok(centibits.min(f64::from(u32::MAX)) as u32)
}

fn entropy_hits(
    bytes: &[u8],
    min_len: usize,
    min_centibits: u32,
    alphabet: Alphabet,
    rule_id: &'static str,
    budget: &mut Budget,
) -> Result<Vec<(usize, usize)>, ScanError> {
    let mut hits = Vec::new();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        if !alphabet.admits(bytes[cursor]) {
            cursor = cursor.saturating_add(1);
            continue;
        }
        budget.charge(rule_id)?;
        let end = token_body_end(bytes, cursor, alphabet);
        if end.saturating_sub(cursor) >= min_len {
            let Some(token) = bytes.get(cursor..end) else {
                return Err(ScanError {
                    rule_id,
                    detail: "token range fell outside the scanned bytes",
                });
            };
            if entropy_centibits(token, rule_id)? >= min_centibits {
                hits.push((cursor, end));
            }
        }
        cursor = end.max(cursor.saturating_add(1));
    }
    Ok(hits)
}

fn is_email_local(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn is_email_domain(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

fn email_hits(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'@' {
            continue;
        }
        let mut start = index;
        while start > 0 && is_email_local(bytes[start.saturating_sub(1)]) {
            start = start.saturating_sub(1);
        }
        let mut end = index.saturating_add(1);
        while end < bytes.len() && is_email_domain(bytes[end]) {
            end = end.saturating_add(1);
        }
        let Some(domain) = bytes.get(index.saturating_add(1)..end) else {
            continue;
        };
        let tld = domain.rsplit(|character| *character == b'.').next();
        let tld_ok =
            tld.is_some_and(|tld| tld.len() >= 2 && tld.iter().all(u8::is_ascii_alphabetic));
        if start < index && domain.contains(&b'.') && tld_ok {
            hits.push((start, end));
        }
    }
    hits
}

fn digit_group_hits(bytes: &[u8], groups: &[usize], separator: u8) -> Vec<(usize, usize)> {
    let width = groups
        .iter()
        .sum::<usize>()
        .saturating_add(groups.len().saturating_sub(1));
    let mut hits = Vec::new();
    let mut cursor = 0_usize;
    while cursor.saturating_add(width) <= bytes.len() {
        let boundary_ok = cursor == 0 || !is_token_byte(bytes[cursor.saturating_sub(1)]);
        let after = cursor.saturating_add(width);
        let tail_ok = bytes.get(after).is_none_or(|byte| !is_token_byte(*byte));
        let window = bytes.get(cursor..after);
        if boundary_ok
            && tail_ok
            && window.is_some_and(|window| matches_groups(window, groups, separator))
        {
            hits.push((cursor, after));
            cursor = after;
            continue;
        }
        cursor = cursor.saturating_add(1);
    }
    hits
}

const fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

fn matches_groups(window: &[u8], groups: &[usize], separator: u8) -> bool {
    let mut offset = 0_usize;
    for (index, size) in groups.iter().enumerate() {
        if index > 0 {
            match window.get(offset) {
                Some(byte) if *byte == separator => offset = offset.saturating_add(1),
                _ => return false,
            }
        }
        let Some(group) = window.get(offset..offset.saturating_add(*size)) else {
            return false;
        };
        if !group.iter().all(u8::is_ascii_digit) {
            return false;
        }
        offset = offset.saturating_add(*size);
    }
    offset == window.len()
}
