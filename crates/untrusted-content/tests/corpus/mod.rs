//! The injection corpus loader.
//!
//! `testdata/injection-corpus/corpus.txt` holds the payloads; this parses them
//! and refuses a corpus that has quietly shrunk or fallen out of coverage.
//!
//! The kind and action rules iterate `SourceKind::ALL` and
//! `PrivilegedAction::ALL` rather than a written count. Those arrays carry their
//! length in their type, so a variant added to either enum without extending its
//! array does not compile, and one added with it fails the corpus here until a
//! record covers it. The family rule is a written list, because an injection
//! family is a property of the payloads and not of the code; it is compared
//! against the corpus in both directions instead.

use academic_untrusted_content::{PrivilegedAction, SourceKind};

/// The corpus text, compiled in so the test needs no path at run time.
const CORPUS: &str = include_str!("../../../../testdata/injection-corpus/corpus.txt");

/// The `PJ04` sentinels, one per line after the comment header.
const RESPONSE_CANARIES: &str =
    include_str!("../../../../testdata/injection-corpus/response-canary.txt");

/// The floor. A walk that returns less than this is a corpus that stopped
/// being read, which would satisfy every assertion made over its result.
pub const MIN_ENTRIES: usize = 48;

/// Every injection family the corpus must cover.
///
/// This is a written list rather than an enum because the families are a
/// property of the payloads and not of the code. What keeps it honest is that
/// every family here must have a record and every record's family must be here:
/// the two sets are compared, not searched.
pub const VECTORS: [&str; 13] = [
    "imperative-override",
    "role-reassignment",
    "fake-system-delimiter",
    "fake-tool-call",
    "markdown-comment",
    "code-comment",
    "encoded-directive",
    "unicode-bidi",
    "zero-width",
    "homoglyph",
    "fence-break",
    "nested-quote",
    "authority-claim",
];

/// The families above plus the ones that are about intent rather than encoding.
pub const EXTRA_VECTORS: [&str; 5] = [
    "conditional-trigger",
    "exfiltration-request",
    "self-reference",
    "control-char",
    "multilingual",
];

/// One corpus record.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Unique identifier.
    pub id: String,
    /// Which channel the payload arrives on.
    pub kind: SourceKind,
    /// Injection family.
    pub vector: String,
    /// The privileged action the payload asks for.
    pub targets: PrivilegedAction,
    /// The sentinel this record carries into the data channel.
    pub canary: String,
    /// The adversarial text, with the canary appended.
    pub payload: String,
}

/// Applies the payload escapes.
fn unescape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).unwrap_or(0xfffd);
                out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Parses the corpus, or returns why it could not be trusted.
pub fn load() -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    for block in CORPUS.split("\n\n") {
        let lines: Vec<&str> = block
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        let field = |prefix: &str, index: usize| -> Result<String, String> {
            lines
                .get(index)
                .and_then(|line| line.strip_prefix(prefix))
                .map(str::to_owned)
                .ok_or_else(|| format!("record {} has no {prefix}line", entries.len()))
        };
        let id = field("id: ", 0)?;
        let kind_text = field("kind: ", 1)?;
        let vector = field("vector: ", 2)?;
        let targets_text = field("targets: ", 3)?;
        let canary = field("canary: ", 4)?;
        let kind =
            SourceKind::parse(&kind_text).ok_or(format!("{id}: unknown kind {kind_text}"))?;
        let targets = PrivilegedAction::parse(&targets_text)
            .ok_or(format!("{id}: unknown action {targets_text}"))?;
        let mut payload_lines = Vec::new();
        for line in lines.iter().skip(5) {
            let Some(value) = line.strip_prefix("payload: ") else {
                return Err(format!("{id}: {line} is not a payload line"));
            };
            payload_lines.push(unescape(value));
        }
        if payload_lines.is_empty() {
            return Err(format!("{id}: no payload"));
        }
        payload_lines.push(canary.clone());
        entries.push(Entry {
            id,
            kind,
            vector,
            targets,
            canary,
            payload: payload_lines.join("\n"),
        });
    }
    check(&entries)?;
    Ok(entries)
}

/// The corpus completeness rules.
fn check(entries: &[Entry]) -> Result<(), String> {
    if entries.len() < MIN_ENTRIES {
        return Err(format!(
            "the corpus holds {} records, below the floor of {MIN_ENTRIES}",
            entries.len()
        ));
    }
    let mut ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    if ids.len() != unique {
        return Err("the corpus repeats an identifier".to_owned());
    }
    let mut canaries: Vec<&str> = entries.iter().map(|entry| entry.canary.as_str()).collect();
    canaries.sort_unstable();
    let unique = canaries.len();
    canaries.dedup();
    if canaries.len() != unique {
        return Err("the corpus repeats a canary".to_owned());
    }

    for kind in SourceKind::ALL {
        if !entries.iter().any(|entry| entry.kind == kind) {
            return Err(format!("no record has kind {}", kind.as_str()));
        }
    }
    for action in PrivilegedAction::ALL {
        if !entries.iter().any(|entry| entry.targets == action) {
            return Err(format!("no record targets {}", action.as_str()));
        }
    }
    let expected: Vec<&str> = VECTORS.into_iter().chain(EXTRA_VECTORS).collect();
    for vector in &expected {
        if !entries.iter().any(|entry| entry.vector == *vector) {
            return Err(format!("no record uses vector {vector}"));
        }
    }
    for entry in entries {
        if !expected.contains(&entry.vector.as_str()) {
            return Err(format!(
                "{}: vector {} is not listed",
                entry.id, entry.vector
            ));
        }
        if !entry.payload.contains(&entry.canary) {
            return Err(format!("{}: the payload lost its canary", entry.id));
        }
    }
    Ok(())
}

/// The `PJ04` sentinels.
pub fn response_canaries() -> Vec<String> {
    RESPONSE_CANARIES
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(str::to_owned)
        .collect()
}
