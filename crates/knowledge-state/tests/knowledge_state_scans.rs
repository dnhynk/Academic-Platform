//! What `academic-knowledge-state` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, and this repository's own measurement
//!
//! `P2-R2` shipped a crate with a forbidden-token list as its only net, and
//! `docs/contracts/policy-source-scans.md` records what that measured: seven
//! spellings of a filesystem or environment reach — `std::path::Path::new(p)
//! .metadata()`, its leading-`::` form, its whitespace-inside-the-path form and
//! `include_str!` among them — compile, spell none of the listed tokens, add no
//! `use` item, and passed. The repair was three **whole-set** comparisons, in
//! both directions:
//!
//! * every `use` item ([`USE_ITEMS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! A reach nobody predicted appears as an **extra key** rather than as a token
//! nobody listed. [`FORBIDDEN_CONSTRUCTS`] is kept as the third and weakest
//! layer, because it names the shapes a reader expects to see refused.
//!
//! ## What a field of this crate may hold, and what it may not
//!
//! [`FIELDS`] is every field of every type this crate declares, compared in
//! both directions, each carrying what it holds. Six things and no seventh:
//!
//! * a **closed vocabulary value** — one of this crate's or a reviewed crate's
//!   enumerations;
//! * a **system identifier** — an `EntityId`, `EvidenceId`, `ModelRunId`,
//!   `ScopeId` or a digest;
//! * a **value of a reviewed crate** — an `ObservedProof`, a `GoalScope`, a
//!   `ConfidencePermille`, a `FreshnessBand`, a `ConflictReason`;
//! * a **value of this crate**;
//! * a **caller-supplied identifier** — a document, node, concept, course,
//!   term, grade or context name; and
//! * a **count, flag or timestamp**.
//!
//! There is no seventh and in particular no byte buffer: no field of this crate
//! is declared `Vec<u8>` or `[u8; N]` under any name, which is what the
//! inventory says without consulting a list of names.
//! `tools/secret-debug-policy.test.mjs` matches a field's **name** against a
//! fixed alternation, so a field holding bytes under a name outside it is
//! invisible to that tool; that tool passing this crate is therefore not
//! evidence about this crate, and the inventory is.
//!
//! ## It reads no clock
//!
//! Section 13.3's `시간 decay는 freshness projection에만 적용한다. mastery를
//! 자동 내리지 않는다` is a whole-crate property here rather than a rule inside
//! one function: [`REACHED_PATHS`] holds no `std::time`, [`USE_ITEMS`] imports
//! no clock, and every instant this crate holds arrived as a
//! `TimestampMillis` argument. `the_state_crate_reads_no_clock_and_opens_nothing`
//! observes all three.

#![allow(clippy::items_after_statements)]

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root().join("src"), &mut found)?;
    walk(&crate_root().join("tests"), &mut found)?;
    found.sort();
    Ok(found)
}

fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    let mut found = Vec::new();
    walk(&root, &mut found)?;
    found.retain(|path| {
        !path
            .strip_prefix(&root)
            .unwrap_or(path)
            .starts_with("tests")
    });
    found.sort();
    Ok(found)
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Removes comments, string literals and character literals.
fn strip_non_code(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            out.push('\n');
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index] == '/' && bytes.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if bytes[index] == '*' && bytes.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            out.push(' ');
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut probe = index + 1;
            let mut hashes = 0_usize;
            while bytes.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if bytes.get(probe) == Some(&'"') {
                let terminator: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = bytes[probe + 1..].iter().collect();
                let end = rest.find(&terminator).map_or(bytes.len(), |offset| {
                    probe + 1 + rest[..offset].chars().count() + terminator.chars().count()
                });
                index = end;
                out.push(' ');
                continue;
            }
        }
        if current == '"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == '\\' {
                    index += 2;
                    continue;
                }
                if bytes[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            out.push(' ');
            continue;
        }
        if current == '\'' {
            let closes = if next == Some('\\') {
                bytes
                    .iter()
                    .skip(index + 2)
                    .position(|character| *character == '\'')
                    .map(|offset| index + 2 + offset)
            } else {
                (bytes.get(index + 2) == Some(&'\'')).then_some(index + 2)
            };
            if let Some(end) = closes {
                index = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// One free function's text, from its signature to the `}` at column zero.
fn free_function(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
}

/// One brace-balanced block's text, from `header` to its matching `}`.
fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} opens no block"))?;
    let mut depth = 0_usize;
    let bytes = source.as_bytes();
    let mut at = open;
    while at < bytes.len() {
        match bytes[at] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        at += 1;
    }
    Ok(collapse(&source[start..=at]))
}

/// Drops comment lines and squeezes runs of whitespace to one space.
fn collapse(body: &str) -> String {
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    kept.join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
fn uses_of(code: &str, name: &str) -> usize {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
            before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
        })
        .count()
}

/// Drops every `use` item, so a re-export is not counted as a reach.
fn without_use_items(code: &str) -> String {
    let mut kept = String::with_capacity(code.len());
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        let opens = trimmed.starts_with("use ")
            || (trimmed.starts_with("pub") && trimmed.contains(" use "));
        if inside || opens {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// The relative path of `path` under the workspace, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The extractors.
// ---------------------------------------------------------------------------

/// Joins a path or a macro name that whitespace was inserted into.
///
/// `std :: env :: var(k)` and `std::env::var(k)` are one reach, and an earlier
/// guard `P2-R2` shipped saw only the second.
fn tighten(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(char::is_whitespace) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let stop = tail
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(tail.len());
        let after = &tail[stop..];
        let joins = out.ends_with("::")
            || out.ends_with('!')
            || after.starts_with("::")
            || after.starts_with('!');
        if !joins {
            out.push(' ');
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Every two-segment path `code` spells through a crate root.
fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "crate", "thiserror"];
    let code = &tighten(code);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    let mut taken = 0_usize;
    for (at, _) in code.match_indices("::") {
        let mut start = at;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start == at {
            continue;
        }
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            continue;
        }
        // A middle segment of a longer path -- the `b` of `a::b::c` -- is not a
        // crate root, and skipping it is what stops one path yielding two keys.
        // What decides it is whether this segment already sits inside a key
        // this pass took, not the byte three positions back. `tighten` glues
        // `as ::std` shut, so that byte is the `s` of a keyword and the leading
        // `::` of a qualified path read as a middle one: `P2-A5` measured
        // `<str as ::std::net::ToSocketAddrs>::to_socket_addrs(host)` resolving
        // a name from a live function while this pass reported nothing. Every
        // segment outside a key already taken is a root, and a root nobody
        // admits fails as an extra key rather than passing.
        if start < taken {
            continue;
        }
        let root = &code[start..at];
        if !roots.contains(&root) && !root.starts_with("academic_") {
            continue;
        }
        let mut end = at + 2;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end > at + 2 {
            found.insert(code[start..end].to_owned());
            taken = end;
        }
    }
    found
}

/// Every macro `code` invokes, by name.
fn macros_spelled(code: &str) -> BTreeSet<String> {
    let code = &tighten(code);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    for (at, _) in code.match_indices('!') {
        let opens = bytes
            .get(at + 1)
            .is_some_and(|byte| matches!(byte, b'(' | b'[' | b'{'));
        if !opens {
            continue;
        }
        let mut start = at;
        while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
            start -= 1;
        }
        if start == at {
            continue;
        }
        let name = &code[start..at];
        let keyword = matches!(
            name,
            "if" | "while" | "for" | "match" | "return" | "else" | "let" | "in"
        );
        if !keyword && (bytes[start].is_ascii_lowercase() || bytes[start] == b'_') {
            found.insert(name.to_owned());
        }
    }
    found
}

/// Every `pub fn` in `code`, as its name and its signature up to the body.
fn public_signatures(code: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for marker in ["pub fn ", "pub const fn "] {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find(marker).map(|at| at + cursor) {
            let after = at + marker.len();
            let name: String = code[after..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            let end = code[at..]
                .find(" {\n")
                .or_else(|| code[at..].find(";\n"))
                .map_or(code.len(), |offset| at + offset);
            found.push((name, code[at..end].to_owned()));
            cursor = after;
        }
    }
    found
}

/// Counts declarations of a function whose name is exactly `name`.
fn declarations_of(code: &str, name: &str) -> usize {
    let needle = format!("fn {name}");
    let bytes = code.as_bytes();
    code.match_indices(&needle)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + needle.len()).copied().unwrap_or(b' ');
            before_ok && (after == b'(' || after == b'<')
        })
        .count()
}

/// The use count of `name` less its declarations, which cannot go negative.
fn calls_of(code: &str, name: &str) -> usize {
    let uses = uses_of(code, name);
    let declarations = declarations_of(code, name);
    assert!(
        uses >= declarations,
        "{name} is declared {declarations} times and named {uses}; the two counts disagree"
    );
    uses - declarations
}

/// Every named field of every `struct` and `enum` `code` declares.
///
/// A struct-variant of an enumeration is reported as `Enum::Variant`, so a
/// field added inside one is a key here rather than a shape this extractor has
/// no name for. A tuple struct has no named field at all, and
/// [`the_helpers_are_not_vacuous`] requires this crate to declare none, so the
/// whole-set claim is not quietly narrowed by one.
fn type_fields(code: &str) -> Vec<(String, String, String)> {
    let mut found = Vec::new();
    for keyword in ["struct ", "enum "] {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find(keyword).map(|at| at + cursor) {
            cursor = at + keyword.len();
            let before = code[..at].chars().next_back().unwrap_or(' ');
            if before.is_alphanumeric() || before == '_' {
                continue;
            }
            let name: String = code[cursor..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let rest = &code[cursor + name.len()..];
            // Skip a lifetime or generic parameter list, then require a body.
            let opens = rest.find(['{', ';', '(']);
            let Some(offset) = opens else {
                continue;
            };
            if rest.as_bytes()[offset] != b'{' {
                continue;
            }
            let body_start = cursor + name.len() + offset;
            let Some(body) = balanced(&code[body_start..]) else {
                continue;
            };
            collect_fields(&name, body, &mut found);
        }
    }
    found.sort();
    found
}

/// The text between `code`'s first `{` and its matching `}`, exclusive.
fn balanced(code: &str) -> Option<&str> {
    let bytes = code.as_bytes();
    let mut depth = 0_usize;
    for (at, byte) in bytes.iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&code[1..at]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Splits one declaration body into its top-level items and reads each.
fn collect_fields(owner: &str, body: &str, found: &mut Vec<(String, String, String)>) {
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some(open) = item.find('{') {
            // An enum struct-variant. Its name is what comes before the brace,
            // less any attribute or doc line the stripper left behind.
            let variant = item[..open]
                .split_whitespace()
                .next_back()
                .unwrap_or("")
                .to_owned();
            if let Some(inner) = balanced(&item[open..]) {
                collect_fields(&format!("{owner}::{variant}"), inner, found);
            }
            continue;
        }
        let Some(colon) = item.find(':') else {
            continue;
        };
        if item.as_bytes().get(colon + 1) == Some(&b':') {
            continue;
        }
        let name = item[..colon]
            .split_whitespace()
            .next_back()
            .unwrap_or("")
            .to_owned();
        if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
            continue;
        }
        let declared: String = item[colon + 1..].split_whitespace().collect();
        found.push((owner.to_owned(), name, declared));
    }
}

/// Splits `body` on the commas that sit at nesting depth zero.
fn top_level_items(body: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut braces = 0_usize;
    let mut parens = 0_usize;
    let mut angles = 0_usize;
    for character in body.chars() {
        match character {
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '(' | '[' => parens += 1,
            ')' | ']' => parens = parens.saturating_sub(1),
            '<' => angles += 1,
            '>' => angles = angles.saturating_sub(1),
            ',' if braces == 0 && parens == 0 && angles == 0 => {
                items.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(character);
    }
    items.push(current);
    items
}

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------

/// Every `use` item of this crate's product code that is not a re-export.
///
/// Compared in both directions. A filesystem, clock, process or transport
/// import appears here as an **extra key** whatever it is called, and a listed
/// import that is removed appears as a missing one.
const USE_ITEMS: [(&str, &str); 94] = [
    ("academic_domain::Actor", "confirmation.rs"),
    ("academic_domain::Claim", "confirmation.rs"),
    ("academic_domain::ClaimObject", "confirmation.rs"),
    ("academic_domain::ConfidencePermille", "assertion.rs"),
    ("academic_domain::ConfidencePermille", "history.rs"),
    ("academic_domain::ConfidencePermille", "projection.rs"),
    ("academic_domain::ContentDigest", "assertion.rs"),
    ("academic_domain::ContentDigest", "eligibility.rs"),
    ("academic_domain::ContentDigest", "evidence.rs"),
    ("academic_domain::EntityId", "assertion.rs"),
    ("academic_domain::EntityId", "confirmation.rs"),
    ("academic_domain::EntityId", "conflict.rs"),
    ("academic_domain::EntityId", "eligibility.rs"),
    ("academic_domain::EntityId", "history.rs"),
    ("academic_domain::EpistemicStatus", "confirmation.rs"),
    ("academic_domain::EvidenceId", "assertion.rs"),
    ("academic_domain::EvidenceId", "confirmation.rs"),
    ("academic_domain::EvidenceId", "eligibility.rs"),
    ("academic_domain::EvidenceId", "evidence.rs"),
    ("academic_domain::EvidenceId", "history.rs"),
    ("academic_domain::EvidenceId", "projection.rs"),
    ("academic_domain::EvidenceItem", "confirmation.rs"),
    ("academic_domain::FreshnessBand", "assertion.rs"),
    ("academic_domain::FreshnessBand", "history.rs"),
    ("academic_domain::MasteryLevel", "assertion.rs"),
    ("academic_domain::MasteryLevel", "confirmation.rs"),
    ("academic_domain::MasteryLevel", "conflict.rs"),
    ("academic_domain::MasteryLevel", "evidence.rs"),
    ("academic_domain::MasteryLevel", "ladder.rs"),
    ("academic_domain::MasteryLevel", "projection.rs"),
    ("academic_domain::ModelRunId", "confirmation.rs"),
    ("academic_domain::ScopeId", "confirmation.rs"),
    ("academic_domain::TimestampMillis", "assertion.rs"),
    ("academic_domain::TimestampMillis", "confirmation.rs"),
    ("academic_domain::TimestampMillis", "evidence.rs"),
    ("academic_domain::TimestampMillis", "history.rs"),
    (
        "academic_domain::entity_registry::EntityKind",
        "eligibility.rs",
    ),
    ("academic_lecture_document::LectureDocument", "evidence.rs"),
    ("academic_lecture_document::NodeId", "evidence.rs"),
    ("academic_ledger::ConflictReason", "conflict.rs"),
    (
        "academic_repository_classification::ConceptStance",
        "evidence.rs",
    ),
    (
        "academic_repository_classification::GoalScope",
        "evidence.rs",
    ),
    (
        "academic_repository_classification::ObservedProof",
        "evidence.rs",
    ),
    ("crate::KnowledgeStateError", "assertion.rs"),
    ("crate::KnowledgeStateError", "confirmation.rs"),
    ("crate::KnowledgeStateError", "evidence.rs"),
    ("crate::KnowledgeStateError", "history.rs"),
    ("crate::assertion::AssertionId", "history.rs"),
    ("crate::assertion::KnowledgeStateAssertion", "conflict.rs"),
    ("crate::assertion::KnowledgeStateAssertion", "history.rs"),
    ("crate::confirmation::AdjustmentDirection", "conflict.rs"),
    ("crate::confirmation::AdjustmentDirection", "history.rs"),
    ("crate::confirmation::AiProposal", "conflict.rs"),
    ("crate::confirmation::AiProposal", "history.rs"),
    ("crate::confirmation::FluentAuthorization", "projection.rs"),
    ("crate::confirmation::UserConfirmation", "assertion.rs"),
    ("crate::confirmation::UserConfirmation", "evidence.rs"),
    ("crate::confirmation::UserConfirmation", "history.rs"),
    ("crate::conflict::KnowledgeStateConflict", "history.rs"),
    ("crate::eligibility::BlockedEvidence", "history.rs"),
    ("crate::eligibility::BlockedEvidence", "projection.rs"),
    ("crate::eligibility::EligibilityCheck", "history.rs"),
    ("crate::eligibility::EligibilityCheck", "projection.rs"),
    ("crate::eligibility::EligibleEvidence", "history.rs"),
    ("crate::eligibility::EligibleEvidence", "projection.rs"),
    ("crate::evidence::BroadSignal", "assertion.rs"),
    ("crate::evidence::BroadSignal", "history.rs"),
    ("crate::evidence::CEILINGS", "projection.rs"),
    ("crate::evidence::ConceptEvidence", "confirmation.rs"),
    ("crate::evidence::ConceptEvidence", "eligibility.rs"),
    ("crate::evidence::EvidenceCeiling", "projection.rs"),
    ("crate::evidence::EvidenceKind", "eligibility.rs"),
    ("crate::evidence::EvidenceKind", "projection.rs"),
    ("crate::ladder::AutomaticLevel", "projection.rs"),
    ("crate::ladder::FacetProfile", "assertion.rs"),
    ("crate::ladder::FacetProfile", "history.rs"),
    ("crate::ladder::FacetStrength", "evidence.rs"),
    ("crate::ladder::level_token", "assertion.rs"),
    ("crate::projection::EvidenceSufficiency", "assertion.rs"),
    ("crate::projection::MasteryProjection", "assertion.rs"),
    ("crate::projection::MasteryProjection", "history.rs"),
    ("crate::projection::UnseenBasis", "assertion.rs"),
    ("crate::projection::project", "history.rs"),
    ("serde::Deserialize", "assertion.rs"),
    ("serde::Deserialize", "eligibility.rs"),
    ("serde::Deserialize", "evidence.rs"),
    ("serde::Deserialize", "ladder.rs"),
    ("serde::Deserialize", "projection.rs"),
    ("serde::Serialize", "assertion.rs"),
    ("serde::Serialize", "eligibility.rs"),
    ("serde::Serialize", "evidence.rs"),
    ("serde::Serialize", "ladder.rs"),
    ("serde::Serialize", "projection.rs"),
    ("std::collections::BTreeSet", "confirmation.rs"),
];

/// The modules `lib.rs` re-exports from.
///
/// `pub use` is a different act from `use`: it hands a name out rather than
/// reaching for one, so it is inventoried separately and by module. Eight
/// modules and no ninth.
const RE_EXPORT_MODULES: [&str; 8] = [
    "assertion",
    "confirmation",
    "conflict",
    "eligibility",
    "evidence",
    "history",
    "ladder",
    "projection",
];

/// Every two-segment path this crate's product code reaches through a crate
/// root, `use` items excluded.
///
/// `absolute_paths` counts a leading `::` form and a whitespace-inside-the-path
/// form as the same reach, which are two of the shapes `P2-R2` measured passing
/// a token list.
const REACHED_PATHS: [(&str, &str); 9] = [
    ("academic_domain::DomainError", "lib.rs"),
    ("academic_domain::EntityId", "projection.rs"),
    ("crate::KnowledgeStateError", "projection.rs"),
    ("crate::confirmation", "evidence.rs"),
    ("crate::confirmation", "history.rs"),
    ("crate::ladder", "assertion.rs"),
    ("crate::ladder", "confirmation.rs"),
    ("crate::ladder", "evidence.rs"),
    ("thiserror::Error", "lib.rs"),
];

/// Every macro this crate's product code invokes.
///
/// Three, and neither of the two file-reading macros is among them:
/// `include_str!` and `include_bytes!` spell no listed token and add no `use`
/// item, which is why this set is compared rather than searched.
const MACROS_SPELLED: [(&str, &str); 3] = [
    ("matches", "evidence.rs"),
    ("vec", "eligibility.rs"),
    ("vec", "history.rs"),
];

/// The shapes a reader expects to see refused, kept as the third and weakest
/// layer.
const FORBIDDEN_CONSTRUCTS: [&str; 14] = [
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "SystemTime",
    "Instant",
    "File::open",
    "read_to_string",
    "include_str",
    "include_bytes",
    "TcpStream",
    "UdpSocket",
    "Command::new",
];

/// Every field of every type this crate declares, and what it holds.
const FIELDS: [(&str, &str, &str, &str); 67] = [
    ("AiProposal", "concept", "EntityId", "system identifier"),
    (
        "AiProposal",
        "evidence",
        "Vec<ConceptEvidence>",
        "value of this crate",
    ),
    (
        "AiProposal",
        "proposed",
        "MasteryLevel",
        "value of a reviewed crate",
    ),
    ("AiProposal", "run_id", "ModelRunId", "system identifier"),
    (
        "AssertionWire",
        "as_of",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "AssertionWire",
        "broad_signals",
        "Vec<BroadSignal>",
        "value of this crate",
    ),
    ("AssertionWire", "concept", "EntityId", "system identifier"),
    (
        "AssertionWire",
        "confirmation",
        "Option<ConfirmationRecord>",
        "value of this crate",
    ),
    (
        "AssertionWire",
        "contradicting_evidence",
        "Vec<EvidenceId>",
        "system identifier",
    ),
    (
        "AssertionWire",
        "estimate_confidence",
        "EvidenceSufficiency",
        "value of this crate",
    ),
    (
        "AssertionWire",
        "evidence",
        "Vec<EvidenceId>",
        "system identifier",
    ),
    (
        "AssertionWire",
        "facets",
        "FacetProfile",
        "value of this crate",
    ),
    (
        "AssertionWire",
        "fluency",
        "Option<FluencyRecord>",
        "value of this crate",
    ),
    (
        "AssertionWire",
        "freshness_band",
        "FreshnessBand",
        "value of a reviewed crate",
    ),
    (
        "AssertionWire",
        "freshness_confidence",
        "ConfidencePermille",
        "value of a reviewed crate",
    ),
    ("AssertionWire", "id", "AssertionId", "system identifier"),
    (
        "AssertionWire",
        "mastery_level",
        "MasteryLevel",
        "value of a reviewed crate",
    ),
    (
        "AssertionWire",
        "supersedes",
        "Option<AssertionId>",
        "system identifier",
    ),
    (
        "AssertionWire",
        "unseen_basis",
        "Option<UnseenBasis>",
        "closed vocabulary value",
    ),
    (
        "AssertionWire",
        "version",
        "u32",
        "count, flag or timestamp",
    ),
    (
        "BlockedEvidence",
        "evidence",
        "ConceptEvidence",
        "value of this crate",
    ),
    (
        "BlockedEvidence",
        "evidence_id",
        "EvidenceId",
        "system identifier",
    ),
    (
        "BlockedEvidence",
        "reasons",
        "Vec<EligibilityReasonCode>",
        "closed vocabulary value",
    ),
    (
        "BroadSignal",
        "signal",
        "CourseGradeSignal",
        "value of this crate",
    ),
    (
        "CeilingDisclosure",
        "ceiling",
        "EvidenceCeiling",
        "closed vocabulary value",
    ),
    (
        "CeilingDisclosure",
        "cell",
        "&'staticstr",
        "a design-document cell, verbatim",
    ),
    (
        "CeilingDisclosure",
        "from",
        "Option<EvidenceKind>",
        "closed vocabulary value",
    ),
    (
        "CeilingRow",
        "ceiling",
        "EvidenceCeiling",
        "closed vocabulary value",
    ),
    (
        "CeilingRow",
        "ceiling_cell",
        "&'staticstr",
        "a design-document cell, verbatim",
    ),
    (
        "CeilingRow",
        "interpretation",
        "&'staticstr",
        "a design-document cell, verbatim",
    ),
    (
        "CeilingRow",
        "kind",
        "EvidenceKind",
        "closed vocabulary value",
    ),
    (
        "ConfirmationRecord",
        "confirmed_at",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "ConfirmationRecord",
        "level",
        "MasteryLevel",
        "value of a reviewed crate",
    ),
    (
        "ConfirmationRecord",
        "user_id",
        "EntityId",
        "system identifier",
    ),
    (
        "CourseGradeSignal",
        "artifact",
        "EvidenceId",
        "system identifier",
    ),
    (
        "CourseGradeSignal",
        "course",
        "String",
        "caller-supplied identifier",
    ),
    (
        "CourseGradeSignal",
        "grade",
        "String",
        "caller-supplied identifier",
    ),
    (
        "CourseGradeSignal",
        "term",
        "String",
        "caller-supplied identifier",
    ),
    (
        "DependencyOnly",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "DependencyOnly",
        "goal",
        "GoalScope",
        "value of a reviewed crate",
    ),
    (
        "DependencyOnly",
        "snapshot",
        "String",
        "caller-supplied identifier",
    ),
    (
        "EligibleEvidence",
        "concept",
        "EntityId",
        "system identifier",
    ),
    (
        "EligibleEvidence",
        "evidence",
        "ConceptEvidence",
        "value of this crate",
    ),
    (
        "EligibleEvidence",
        "evidence_id",
        "EvidenceId",
        "system identifier",
    ),
    (
        "EligibleEvidence",
        "outcome",
        "Outcome",
        "closed vocabulary value",
    ),
    (
        "EligibleEvidence",
        "tier",
        "EntityKind",
        "value of a reviewed crate",
    ),
    (
        "EvidenceDossier",
        "concept_link",
        "ConceptLink",
        "closed vocabulary value",
    ),
    (
        "EvidenceDossier",
        "integrity",
        "SourceIntegrity",
        "closed vocabulary value",
    ),
    (
        "EvidenceDossier",
        "outcome",
        "Outcome",
        "closed vocabulary value",
    ),
    (
        "EvidenceDossier",
        "participation",
        "Participation",
        "closed vocabulary value",
    ),
    (
        "EvidenceRetraction",
        "evidence_id",
        "EvidenceId",
        "system identifier",
    ),
    (
        "EvidenceRetraction",
        "failed_check",
        "EligibilityCheck",
        "closed vocabulary value",
    ),
    (
        "EvidenceRetraction",
        "retracted_at",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "EvidenceSufficiency",
        "gaps",
        "Vec<SufficiencyGap>",
        "closed vocabulary value",
    ),
    (
        "EvidenceSufficiency",
        "permille",
        "ConfidencePermille",
        "value of a reviewed crate",
    ),
    (
        "ExerciseOutcome",
        "artifact",
        "EvidenceId",
        "system identifier",
    ),
    (
        "ExerciseOutcome",
        "succeeded",
        "bool",
        "count, flag or timestamp",
    ),
    (
        "FacetProfile",
        "explain",
        "FacetStrength",
        "closed vocabulary value",
    ),
    (
        "FacetProfile",
        "implement_or_operate",
        "FacetStrength",
        "closed vocabulary value",
    ),
    (
        "FacetProfile",
        "recognize",
        "FacetStrength",
        "closed vocabulary value",
    ),
    (
        "FacetProfile",
        "solve_structured_problem",
        "FacetStrength",
        "closed vocabulary value",
    ),
    (
        "FacetProfile",
        "transfer_to_novel_situation",
        "FacetStrength",
        "closed vocabulary value",
    ),
    (
        "FluencyRecord",
        "confirmed_at",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "FluencyRecord",
        "distinct_contexts",
        "usize",
        "count, flag or timestamp",
    ),
    ("FluencyRecord", "user_id", "EntityId", "system identifier"),
    (
        "FluentAuthorization",
        "concept",
        "EntityId",
        "system identifier",
    ),
    (
        "FluentAuthorization",
        "confirmed_at",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
];

/// The rest of the same inventory, split only because one array of 134 rows
/// is harder to read than two.
const MORE_FIELDS: [(&str, &str, &str, &str); 67] = [
    (
        "FluentAuthorization",
        "distinct_contexts",
        "usize",
        "count, flag or timestamp",
    ),
    (
        "FluentAuthorization",
        "scope_id",
        "ScopeId",
        "system identifier",
    ),
    (
        "FluentAuthorization",
        "user_id",
        "EntityId",
        "system identifier",
    ),
    (
        "FreshnessInput",
        "band",
        "FreshnessBand",
        "value of a reviewed crate",
    ),
    (
        "FreshnessInput",
        "confidence",
        "ConfidencePermille",
        "value of a reviewed crate",
    ),
    ("IncidentRepair", "fix", "EvidenceId", "system identifier"),
    (
        "IncidentRepair",
        "incident",
        "EvidenceId",
        "system identifier",
    ),
    (
        "IncidentRepair",
        "root_cause",
        "EvidenceId",
        "system identifier",
    ),
    (
        "IncidentRepair",
        "verification",
        "EvidenceId",
        "system identifier",
    ),
    (
        "KnowledgeStateAssertion",
        "as_of",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "KnowledgeStateAssertion",
        "broad_signals",
        "Vec<BroadSignal>",
        "value of this crate",
    ),
    (
        "KnowledgeStateAssertion",
        "concept",
        "EntityId",
        "system identifier",
    ),
    (
        "KnowledgeStateAssertion",
        "confirmation",
        "Option<ConfirmationRecord>",
        "value of this crate",
    ),
    (
        "KnowledgeStateAssertion",
        "contradicting_evidence",
        "Vec<EvidenceId>",
        "system identifier",
    ),
    (
        "KnowledgeStateAssertion",
        "estimate_confidence",
        "EvidenceSufficiency",
        "value of this crate",
    ),
    (
        "KnowledgeStateAssertion",
        "evidence",
        "Vec<EvidenceId>",
        "system identifier",
    ),
    (
        "KnowledgeStateAssertion",
        "facets",
        "FacetProfile",
        "value of this crate",
    ),
    (
        "KnowledgeStateAssertion",
        "fluency",
        "Option<FluencyRecord>",
        "value of this crate",
    ),
    (
        "KnowledgeStateAssertion",
        "freshness_band",
        "FreshnessBand",
        "value of a reviewed crate",
    ),
    (
        "KnowledgeStateAssertion",
        "freshness_confidence",
        "ConfidencePermille",
        "value of a reviewed crate",
    ),
    (
        "KnowledgeStateAssertion",
        "id",
        "AssertionId",
        "system identifier",
    ),
    (
        "KnowledgeStateAssertion",
        "mastery_level",
        "MasteryLevel",
        "value of a reviewed crate",
    ),
    (
        "KnowledgeStateAssertion",
        "supersedes",
        "Option<AssertionId>",
        "system identifier",
    ),
    (
        "KnowledgeStateAssertion",
        "unseen_basis",
        "Option<UnseenBasis>",
        "closed vocabulary value",
    ),
    (
        "KnowledgeStateAssertion",
        "version",
        "u32",
        "count, flag or timestamp",
    ),
    (
        "KnowledgeStateConflict",
        "concept",
        "EntityId",
        "system identifier",
    ),
    (
        "KnowledgeStateConflict",
        "direction",
        "AdjustmentDirection",
        "closed vocabulary value",
    ),
    (
        "KnowledgeStateConflict",
        "proposed",
        "AiProposal",
        "value of this crate",
    ),
    (
        "KnowledgeStateConflict",
        "reason",
        "ConflictReason",
        "value of a reviewed crate",
    ),
    (
        "KnowledgeStateConflict",
        "standing",
        "KnowledgeStateAssertion",
        "value of this crate",
    ),
    (
        "KnowledgeStateError::TeachingSiteNotInDocument",
        "document",
        "String",
        "caller-supplied identifier",
    ),
    (
        "KnowledgeStateError::TeachingSiteNotInDocument",
        "node",
        "String",
        "caller-supplied identifier",
    ),
    (
        "KnowledgeStateHistory",
        "admitted",
        "Vec<EligibleEvidence>",
        "value of this crate",
    ),
    (
        "KnowledgeStateHistory",
        "blocked",
        "Vec<BlockedEvidence>",
        "value of this crate",
    ),
    (
        "KnowledgeStateHistory",
        "broad_signals",
        "Vec<BroadSignal>",
        "value of this crate",
    ),
    (
        "KnowledgeStateHistory",
        "concept",
        "EntityId",
        "system identifier",
    ),
    (
        "KnowledgeStateHistory",
        "entries",
        "Vec<HistoryEntry>",
        "value of this crate",
    ),
    (
        "KnowledgeStateHistory",
        "facets",
        "FacetProfile",
        "value of this crate",
    ),
    (
        "KnowledgeStateHistory",
        "retracted",
        "Vec<EvidenceRetraction>",
        "value of this crate",
    ),
    (
        "MasteryProjection",
        "automatic",
        "AutomaticLevel",
        "closed vocabulary value",
    ),
    (
        "MasteryProjection",
        "contradicting",
        "Vec<EvidenceId>",
        "system identifier",
    ),
    (
        "MasteryProjection",
        "disclosure",
        "CeilingDisclosure",
        "value of this crate",
    ),
    (
        "MasteryProjection",
        "fluency_contexts",
        "Option<usize>",
        "count, flag or timestamp",
    ),
    (
        "MasteryProjection",
        "level",
        "MasteryLevel",
        "value of a reviewed crate",
    ),
    (
        "MasteryProjection",
        "sufficiency",
        "EvidenceSufficiency",
        "value of this crate",
    ),
    (
        "MasteryProjection",
        "supporting",
        "Vec<EvidenceId>",
        "system identifier",
    ),
    (
        "MasteryProjection",
        "unseen_basis",
        "Option<UnseenBasis>",
        "closed vocabulary value",
    ),
    (
        "ProjectUse",
        "concept",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ProjectUse",
        "goal",
        "GoalScope",
        "value of a reviewed crate",
    ),
    (
        "ProjectUse",
        "proof",
        "ObservedProof",
        "value of a reviewed crate",
    ),
    (
        "ProjectUse",
        "snapshot",
        "String",
        "caller-supplied identifier",
    ),
    (
        "ProposalApplication",
        "history",
        "KnowledgeStateHistory",
        "value of this crate",
    ),
    (
        "ProposalApplication",
        "outcome",
        "ProposalOutcome",
        "value of this crate",
    ),
    (
        "SelfExplanation",
        "artifact",
        "EvidenceId",
        "system identifier",
    ),
    (
        "SelfExplanation",
        "confirmed_at",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "TeachingSite",
        "document",
        "String",
        "caller-supplied identifier",
    ),
    (
        "TeachingSite",
        "document_digest",
        "ContentDigest",
        "system identifier",
    ),
    (
        "TeachingSite",
        "node",
        "String",
        "caller-supplied identifier",
    ),
    (
        "TransferContext",
        "context",
        "String",
        "caller-supplied identifier",
    ),
    (
        "TransferContext",
        "evidence",
        "EvidenceId",
        "system identifier",
    ),
    (
        "TransferContext",
        "independent",
        "bool",
        "count, flag or timestamp",
    ),
    (
        "TransferRepetition",
        "contexts",
        "Vec<TransferContext>",
        "value of this crate",
    ),
    (
        "UserConfirmation",
        "concept",
        "EntityId",
        "system identifier",
    ),
    (
        "UserConfirmation",
        "confirmed_at",
        "TimestampMillis",
        "count, flag or timestamp",
    ),
    (
        "UserConfirmation",
        "level",
        "MasteryLevel",
        "value of a reviewed crate",
    ),
    (
        "UserConfirmation",
        "scope_id",
        "ScopeId",
        "system identifier",
    ),
    (
        "UserConfirmation",
        "user_id",
        "EntityId",
        "system identifier",
    ),
];

/// The six things a field of this crate may hold.
const REASONS: [&str; 6] = [
    "closed vocabulary value",
    "system identifier",
    "value of a reviewed crate",
    "value of this crate",
    "caller-supplied identifier",
    "count, flag or timestamp",
];

/// The seventh, kept apart because it is the only one that is text.
const CELL_REASON: &str = "a design-document cell, verbatim";

/// The two inventory arrays, as one list.
fn inventory() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    FIELDS.into_iter().chain(MORE_FIELDS).collect()
}

/// Every `use` item, flattened to one leaf per line, `pub use` excluded.
fn use_items(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("use ") {
        let before = rest[..at].trim_end();
        let re_export = before.ends_with("pub");
        let boundary = rest[..at].chars().next_back().unwrap_or('\n');
        let joined = boundary.is_alphanumeric() || boundary == '_';
        let Some(end) = rest[at..].find(';') else {
            break;
        };
        if !re_export && !joined {
            let item: String = rest[at + 4..at + end].split_whitespace().collect();
            flatten(&item, String::new(), &mut found);
        }
        rest = &rest[at + end + 1..];
    }
    found.sort();
    found.dedup();
    found
}

/// The modules a `pub use` hands names out of.
fn re_export_modules(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("pub use ") {
        let Some(end) = rest[at..].find(';') else {
            break;
        };
        let item: String = rest[at + 8..at + end].split_whitespace().collect();
        let module = item.split("::").next().unwrap_or("").to_owned();
        if !module.is_empty() {
            found.push(module);
        }
        rest = &rest[at + end + 1..];
    }
    found.sort();
    found.dedup();
    found
}

fn flatten(item: &str, prefix: String, found: &mut Vec<String>) {
    if let Some(open) = item.find('{') {
        let head = format!("{prefix}{}", &item[..open]);
        let Some(inner) = balanced(&item[open..]) else {
            return;
        };
        for part in top_level_items(inner) {
            flatten(part.trim(), head.clone(), found);
        }
        return;
    }
    if item.is_empty() {
        return;
    }
    found.push(format!("{prefix}{item}"));
}

fn module_of(path: &str) -> String {
    let stem = path.rsplit('/').next().unwrap_or(path);
    stem.to_owned()
}

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let declared = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    let mut modules: Vec<String> = declared
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .map(|name| format!("{name}.rs"))
        .collect();
    modules.push("lib.rs".to_owned());
    modules.sort();

    let mut walked: Vec<String> = crate_product_sources()?
        .iter()
        .map(|path| module_of(&relative(path)))
        .collect();
    walked.sort();

    // Both directions: a module declared and not walked, or walked and not
    // declared, is a hole in every whole-set claim below.
    assert_eq!(walked, modules, "the walk and lib.rs disagree");
    assert!(walked.len() >= 8);
    Ok(())
}

#[test]
fn the_state_crate_reads_no_clock_and_opens_nothing() -> TestResult {
    let mut uses: Vec<(String, String)> = Vec::new();
    let mut paths: Vec<(String, String)> = Vec::new();
    let mut macros: Vec<(String, String)> = Vec::new();
    let mut re_exports: Vec<String> = Vec::new();

    for (path, code) in product_code()? {
        let file = module_of(&path);
        for item in use_items(&code) {
            uses.push((item, file.clone()));
        }
        re_exports.extend(re_export_modules(&code));
        let body = without_use_items(&code);
        for reached in absolute_paths(&body) {
            paths.push((reached, file.clone()));
        }
        for spelled in macros_spelled(&body) {
            macros.push((spelled, file.clone()));
        }
    }
    uses.sort();
    paths.sort();
    macros.sort();
    re_exports.sort();
    re_exports.dedup();

    let expected_uses: Vec<(String, String)> = USE_ITEMS
        .iter()
        .map(|(item, file)| ((*item).to_owned(), (*file).to_owned()))
        .collect();
    let expected_paths: Vec<(String, String)> = REACHED_PATHS
        .iter()
        .map(|(item, file)| ((*item).to_owned(), (*file).to_owned()))
        .collect();
    let expected_macros: Vec<(String, String)> = MACROS_SPELLED
        .iter()
        .map(|(item, file)| ((*item).to_owned(), (*file).to_owned()))
        .collect();

    // Three whole sets, each compared in both directions.
    assert_eq!(uses, expected_uses, "the use-item inventory disagrees");
    assert_eq!(
        paths, expected_paths,
        "the reached-path inventory disagrees"
    );
    assert_eq!(macros, expected_macros, "the macro inventory disagrees");
    assert_eq!(
        re_exports,
        RE_EXPORT_MODULES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    );

    // The third and weakest layer, over every product file.
    for (path, code) in product_code()? {
        for construct in FORBIDDEN_CONSTRUCTS {
            assert_eq!(uses_of(&code, construct), 0, "{path} spells {construct}");
        }
    }
    Ok(())
}

/// The files of this package that read or write anything, pinned by name.
const READERS: [&str; 3] = [
    "crates/knowledge-state/tests/common/lecture.rs",
    "crates/knowledge-state/tests/knowledge_state.rs",
    "crates/knowledge-state/tests/knowledge_state_scans.rs",
];

#[test]
fn only_the_named_test_files_read_anything() -> TestResult {
    let mut readers: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let reads = ["fs::", "tempfile", "read_to_string", "include_str"]
            .iter()
            .any(|token| uses_of(&code, token) > 0);
        if reads {
            readers.push(relative(&path));
        }
    }
    readers.sort();
    let mut expected: Vec<String> = READERS.iter().map(|name| (*name).to_owned()).collect();
    expected.sort();
    assert_eq!(readers, expected, "a file of this package reads something");

    // And none of them is a product file: the four are the acceptance suite,
    // this scan, and the two fixture modules.
    for reader in &readers {
        assert!(
            reader.contains("/tests/"),
            "{reader} is a product file that reads"
        );
    }
    Ok(())
}

#[test]
fn every_field_of_this_crate_is_in_the_inventory() -> TestResult {
    let mut declared: Vec<(String, String, String)> = Vec::new();
    for (_, code) in product_code()? {
        declared.extend(type_fields(&code));
    }
    declared.sort();

    let mut inventoried: Vec<(String, String, String)> = inventory()
        .iter()
        .map(|(owner, field, kind, _)| {
            ((*owner).to_owned(), (*field).to_owned(), (*kind).to_owned())
        })
        .collect();
    inventoried.sort();

    // Both directions. A field added under any name at all is an extra key.
    assert_eq!(declared, inventoried, "the field inventory disagrees");

    // Every row's reason is one of the seven, and there is no byte buffer.
    for (owner, field, kind, reason) in inventory() {
        assert!(
            REASONS.contains(&reason) || reason == CELL_REASON,
            "{owner}.{field} claims the reason {reason}"
        );
        assert!(
            !kind.contains("u8"),
            "{owner}.{field} is declared {kind}, which holds bytes"
        );
    }

    // A tuple struct has no named field, so a whole-set claim over named fields
    // would silently miss one. This crate declares exactly one, `AssertionId`,
    // and its single element is a digest.
    let mut tuple_structs = 0_usize;
    for (_, code) in product_code()? {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find("struct ").map(|at| at + cursor) {
            cursor = at + 7;
            let rest = &code[cursor..];
            let name: String = rest
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            let after = &rest[name.len()..];
            if after.trim_start().starts_with('(') {
                assert_eq!(
                    name, "AssertionId",
                    "{name} is an uninventoried tuple struct"
                );
                tuple_structs += 1;
            }
        }
    }
    assert_eq!(tuple_structs, 1);
    Ok(())
}

#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    for (path, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            assert!(
                !signature.contains("&mut self"),
                "{path}::{name} takes &mut self"
            );
        }
    }
    // The control: the extractor does find a `&mut self` when one is there.
    assert!(
        public_signatures("pub fn set(&mut self, v: u16) {\n")
            .iter()
            .any(|(_, signature)| signature.contains("&mut self")),
        "the extractor cannot see a &mut self at all"
    );
    Ok(())
}

/// The decisions this crate makes, pinned as whole bodies.
///
/// A ceiling silently raised, an automatic contribution silently promoted, or a
/// sixth automatic level added is a change to one of these strings.
const WHOLE_CEILING: &str = "pub const fn ceiling(self) -> EvidenceCeiling { match self { Self::MeaningfulTeaching => EvidenceCeiling::UpTo(MasteryLevel::Exposed), Self::SelfExplanationConfirmed => EvidenceCeiling::UpTo(MasteryLevel::Understood), Self::ConceptSpecificExercise => EvidenceCeiling::UpTo(MasteryLevel::Practiced), Self::AuthoredProjectCode | Self::IncidentDebugging => { EvidenceCeiling::UpTo(MasteryLevel::Applied) } Self::RepeatedIndependentTransfer => EvidenceCeiling::UpTo(MasteryLevel::Fluent), Self::DependencyPresenceOnly | Self::CourseGrade => EvidenceCeiling::NoPromotion, } }";

const WHOLE_AUTOMATIC: &str = "pub const fn automatic_contribution(kind: EvidenceKind) -> AutomaticLevel { match kind { EvidenceKind::MeaningfulTeaching => AutomaticLevel::Exposed, EvidenceKind::SelfExplanationConfirmed => AutomaticLevel::Understood, EvidenceKind::ConceptSpecificExercise => AutomaticLevel::Practiced, EvidenceKind::AuthoredProjectCode | EvidenceKind::IncidentDebugging | EvidenceKind::RepeatedIndependentTransfer => AutomaticLevel::Applied, EvidenceKind::DependencyPresenceOnly | EvidenceKind::CourseGrade => AutomaticLevel::Unseen, } }";

#[test]
fn the_state_decisions_are_pinned() -> TestResult {
    let evidence = strip_non_code(&fs::read_to_string(crate_root().join("src/evidence.rs"))?);
    let projection = strip_non_code(&fs::read_to_string(crate_root().join("src/projection.rs"))?);
    let ladder = strip_non_code(&fs::read_to_string(crate_root().join("src/ladder.rs"))?);

    assert_eq!(
        whole_block(&evidence, "pub const fn ceiling(self) -> EvidenceCeiling")?,
        WHOLE_CEILING
    );
    assert_eq!(
        free_function(
            &projection,
            "pub const fn automatic_contribution(kind: EvidenceKind) -> AutomaticLevel"
        )?,
        WHOLE_AUTOMATIC
    );

    // `AutomaticLevel`'s variant list is the whole of "an automatic projection
    // cannot reach FLUENT". A sixth variant is a change to this block, and the
    // name itself must not appear in it.
    let declared = whole_block(&ladder, "pub enum AutomaticLevel")?;
    assert_eq!(
        declared,
        "pub enum AutomaticLevel { Unseen, Exposed, Understood, Practiced, Applied, }"
    );
    assert_eq!(uses_of(&declared, "Fluent"), 0);

    // The control: the same extractor does see `Fluent` in the ladder that has
    // it, so the previous line is a measurement and not an extractor that
    // always answers zero.
    let full = whole_block(&ladder, "pub const fn rung(level: MasteryLevel) -> u8")?;
    assert_eq!(uses_of(&full, "Fluent"), 1);
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(declarations_of("fn projected(", "project"), 0);
    assert_eq!(declarations_of("fn project(x)", "project"), 1);
    assert_eq!(calls_of("fn project(){} project(a);", "project"), 1);
    assert_eq!(uses_of("reprojected projected project", "project"), 1);
    assert_eq!(
        without_use_items("use a::b;\nlet x = b();\n").trim(),
        "let x = b();"
    );
    assert_eq!(collapse("// gone\n  a   b\n"), "a b");

    // The stripper is what makes the forbidden-token pass a statement about
    // code: a `std::fs` inside a string literal or a comment is prose about the
    // rule, and this file writes both.
    assert_eq!(
        strip_non_code("let a = \"std::fs\"; // std::fs\n"),
        "let a =  ; \n\n"
    );

    // The three reach extractors, each on a shape `P2-R2` measured passing an
    // earlier token list.
    assert_eq!(
        absolute_paths("let _ = std::path::Path::new(p).metadata();"),
        BTreeSet::from(["std::path".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = ::std::time::SystemTime::now();"),
        BTreeSet::from(["std::time".to_owned()])
    );
    assert_eq!(
        absolute_paths("let _ = std :: env :: var(k);"),
        BTreeSet::from(["std::env".to_owned()])
    );
    assert_eq!(
        absolute_paths("Self::Variant and self.field"),
        BTreeSet::new()
    );
    assert_eq!(
        macros_spelled("let a = include_str!(\"x\");"),
        BTreeSet::from(["include_str".to_owned()])
    );

    // The use-item flattener, on the shapes this crate writes.
    assert_eq!(
        use_items("use a::{b, c::{d, e}};\n"),
        vec![
            "a::b".to_owned(),
            "a::c::d".to_owned(),
            "a::c::e".to_owned()
        ]
    );
    assert!(
        use_items("pub use a::b;\n").is_empty(),
        "a re-export is not a reach"
    );
    assert_eq!(
        re_export_modules("pub use a::{b, c};\n"),
        vec!["a".to_owned()]
    );

    // The field extractor sees a field of a struct, of an enum struct-variant,
    // and a byte buffer under a harmless name.
    assert_eq!(
        type_fields("struct A { b: u16 }"),
        vec![("A".to_owned(), "b".to_owned(), "u16".to_owned())]
    );
    assert_eq!(
        type_fields("enum A { B { c: Vec<u8> } }"),
        vec![("A::B".to_owned(), "c".to_owned(), "Vec<u8>".to_owned())]
    );
    // A qualified path is a leading `::` however it is spelled. `tighten` glues
    // the space in `<T as ::std::net::X>` shut, and deciding on the byte before
    // the `::` then read the crate root as a middle segment: `P2-A5` measured a
    // name resolved from a live function with this pass reporting nothing.
    assert!(
        absolute_paths("let _ = <str as ::std::net::ToSocketAddrs>::to_socket_addrs(h);")
            .contains("std::net")
    );
    assert!(absolute_paths("let _: &dyn ::core::fmt::Debug = &v;").contains("core::fmt"));
    // The other direction, so the repair is not "every segment is a root": a
    // real middle segment still yields no second key.
    assert!(!absolute_paths("std::alloc::Layout::new::<u8>()").contains("alloc::Layout"));
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let all: Vec<String> = crate_all_sources()?.iter().map(|p| relative(p)).collect();
    assert!(
        all.iter()
            .any(|path| path.ends_with("tests/knowledge_state_scans.rs")),
        "this file is not in the walk it performs"
    );
    assert!(
        READERS.contains(&"crates/knowledge-state/tests/knowledge_state_scans.rs"),
        "this file reads and is not in READERS"
    );
    Ok(())
}
