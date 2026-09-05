//! What `academic-blind-spot` may reach, hold and hand out.
//!
//! ## The claims this file exists for
//!
//! Four of `P2-N7`'s ten acceptance rows drive behaviour, and behaviour is a
//! statement about the paths a test walked. The statements this task actually
//! makes are stronger, and a behavioural test cannot make them:
//!
//! * `coverage_never_becomes_mastery` counts admitted evidence and shows the
//!   reading is not a level. **The claim is that no code path could make it
//!   one**, because this crate has no name for a mastery at all —
//!   [`the_blind_spot_crate_cannot_name_a_mastery`].
//! * `not_relevant_survives_ai_rerun` drives five reruns.
//!   **The claim is that there is nothing else to drive**: `detect` is this
//!   crate's only producer of a finding —
//!   [`the_finding_has_exactly_one_producer`].
//! * `low_relevance_uses_neutral_tokens` compares the whole set of renderable
//!   copy against the design document. **The claim is that there is no other
//!   place text could enter**, because the presentation types carry no owned
//!   string and this crate has no warning vocabulary —
//!   [`the_presentation_carries_no_free_text`].
//! * `no_equalize_all_goal_is_generated` reads one skewed distribution.
//!   **The claim is that a goal is not a value this crate could build**, because
//!   it cannot name one and has no edge to the crate that owns them —
//!   [`no_goal_vocabulary_reaches_this_crate`].
//!
//! Each is three or four whole-set comparisons in both directions, so a reach
//! for a forbidden name appears as an **extra key** rather than as a token
//! nobody listed — the shape `docs/contracts/policy-source-scans.md` records
//! seven spellings defeating a list.
//!
//! The extractors are `crates/freshness/tests/freshness_scans.rs`'s, restated
//! the way that file restates `P2-N2`'s: a test module is not a library target.
//! [`the_helpers_are_not_vacuous`] re-exercises each of them here against a
//! sample it must match, because an extractor that always answered the empty set
//! would satisfy every comparison below.
//!
//! ## It reads no clock
//!
//! Section 23's `HIDE_UNTIL` expiry is a comparison between two arguments.
//! [`REACHED_PATHS`] holds no `std::time`, [`USE_ITEMS`] imports no clock, and
//! every instant this crate holds arrived as a `TimestampMillis` parameter — so
//! *this engine cannot ask what time it is* is a property of the whole crate
//! rather than a convention.

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

/// One `const` or `static` declaration's text, from its name to the `;`.
fn const_declaration(source: &str, name: &str) -> Result<String, Box<dyn Error>> {
    let needle = format!("const {name}");
    let start = source
        .find(&needle)
        .ok_or_else(|| format!("{name} is not declared in the source"))?;
    // The `;` this stops at is the statement's own, not the one inside
    // `[T; N]`: an array length is a semicolon at bracket depth one, and an
    // earlier version of this helper truncated `LOW_RECENCY_BANDS` there.
    let mut depth = 0_i32;
    let mut end = None;
    for (offset, character) in source[start..].char_indices() {
        match character {
            '[' | '(' | '<' | '{' => depth += 1,
            ']' | ')' | '>' | '}' => depth -= 1,
            ';' if depth == 0 => {
                end = Some(offset);
                break;
            }
            _ => {}
        }
    }
    let end = end.ok_or_else(|| format!("{name}'s declaration does not end"))?;
    Ok(collapse(&source[start..start + end + 1]))
}

/// One type declaration's text, from its `pub struct`/`pub enum` line to the
/// matching `}` at column zero.
fn type_declaration(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{header} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
}

fn read_module(name: &str) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(
        crate_root().join("src").join(name),
    )?))
}

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------

/// Every `use` item of this crate's product code that is not a re-export.
///
/// Compared in both directions. A mastery name, a goal vocabulary, a filesystem,
/// clock, process or transport import appears here as an **extra key** whatever
/// it is called, and a listed import that is removed appears as a missing one.
const USE_ITEMS: [&str; 49] = [
    "academic_domain::Actor",
    "academic_domain::Claim",
    "academic_domain::ClaimObject",
    "academic_domain::EntityId",
    "academic_domain::EpistemicStatus",
    "academic_domain::EvidenceId",
    "academic_domain::EvidenceItem",
    "academic_domain::FreshnessBand",
    "academic_domain::ScopeId",
    "academic_domain::TimestampMillis",
    "academic_domain::entity_registry::EntityKind",
    "academic_domain::ontology::TaxonomyNode",
    "academic_domain::ontology::TaxonomyVersionIdentity",
    "academic_domain::ontology::VersionedTaxonomyImport",
    "academic_knowledge_state::EligibleEvidence",
    "academic_knowledge_state::Outcome",
    "crate::BlindSpotError",
    "crate::coverage::EvidenceDiversity",
    "crate::coverage::ExposureItem",
    "crate::coverage::ExposureSource",
    "crate::coverage::FieldCoverage",
    "crate::disposition::DispositionLedger",
    "crate::disposition::UserDisposition",
    "crate::disposition::UserDispositionChoice",
    "crate::explanation::SkewExplanation",
    "crate::finding::BlindSpotFinding",
    "crate::presentation::FindingPresentation",
    "crate::presentation::NeutralPresentation",
    "crate::reading::KeyReading",
    "crate::relevance::GoalRelevance",
    "crate::resolution::FieldResolver",
    "crate::scope::BlindSpotScope",
    "crate::scope::TaxonomyGranularity",
    "crate::state::BLIND_SPOT_STATES",
    "crate::state::BelowMinimum",
    "crate::state::BlindSpotState",
    "crate::state::GoalBlock",
    "crate::state::LOW_RECENCY_BANDS",
    "crate::state::LowRecency",
    "crate::state::ObservedDifficulty",
    "crate::state::ScopeExclusion",
    "crate::state::StateBasis",
    "crate::state::state_of",
    "crate::taste::TastePath",
    "crate::taste::TasteStep",
    "serde::Deserialize",
    "serde::Serialize",
    "std::collections::BTreeMap",
    "std::collections::BTreeSet",
];

/// The modules `lib.rs` re-exports from.
///
/// `pub use` is a different act from `use`: it hands a name out rather than
/// reaching for one, so it is inventoried separately and by module.
const RE_EXPORT_MODULES: [&str; 12] = [
    "coverage",
    "detector",
    "disposition",
    "explanation",
    "finding",
    "presentation",
    "reading",
    "relevance",
    "resolution",
    "scope",
    "state",
    "taste",
];

/// Every two-segment path this crate's product code reaches through a crate
/// root, `use` items excluded.
const REACHED_PATHS: [&str; 5] = [
    "academic_domain::DomainError",
    "academic_domain::EntityId",
    "academic_domain::EvidenceId",
    "academic_domain::FreshnessBand",
    "thiserror::Error",
];

/// Every macro this crate's product code invokes.
const MACROS_SPELLED: [&str; 1] = ["format"];

/// Every name in this workspace that means a mastery level.
///
/// `academic-knowledge-state` is a product edge of this crate and hands all of
/// these out; `academic_domain::MasteryLevel` is one `use` away. This crate
/// reaches for none of them, which is what makes *a coverage reading cannot
/// become a mastery score* a fact about the graph rather than a rule inside one
/// function. The list is `P2-N3`'s, unchanged, so its control reads the same
/// file.
const MASTERY_NAMES: [&str; 8] = [
    "MasteryLevel",
    "AutomaticLevel",
    "MasteryProjection",
    "MasteryFacet",
    "LADDER",
    "rung",
    "level_token",
    "automatic_contribution",
];

/// Every spelling that would fold a coverage reading into one number.
///
/// `coverage_never_becomes_mastery` can only observe the two readings it built.
const SCORE_NAMES: [&str; 12] = [
    "score",
    "percent",
    "percentage",
    "ratio",
    "mean",
    "average",
    "weight",
    "weighted",
    "normalise",
    "normalize",
    "proficiency",
    "rank",
];

/// Every spelling that would make a blind spot a warning.
///
/// Section 23: `warning red가 아니라 중립 outline`. Section 34.5's prevention
/// column for the whole failure mode is `neutral UI`.
const WARNING_NAMES: [&str; 8] = [
    "WarningRed",
    "Severity",
    "Alert",
    "Danger",
    "Critical",
    "Urgent",
    "Priority",
    "Pressure",
];

/// Every spelling that would let this engine name a goal it could then emit.
///
/// `P2-N5` owns them and there is no edge to it.
const GOAL_NAMES: [&str; 8] = [
    "ActiveGoal",
    "GoalCriteria",
    "SuccessCriterion",
    "GapCase",
    "academic_gap",
    "Objective",
    "Remediation",
    "Recommendation",
];

/// The manifest edges this crate must not have.
const FORBIDDEN_EDGES: [&str; 6] = [
    "academic-gap",
    "academic-freshness",
    "academic-store",
    "academic-worker",
    "academic-egress-boundary",
    "academic-vault",
];

/// The shapes a reader expects to see refused, kept as the third and weakest
/// layer.
const FORBIDDEN_CONSTRUCTS: [&str; 15] = [
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "std::time",
    "SystemTime",
    "Instant",
    "now",
    "File",
    "Path",
    "PathBuf",
    "TcpStream",
    "Command",
    "include_str",
    "include_bytes",
];

/// The whole classification order, whitespace-collapsed.
///
/// Every one of its steps is a decision a later edit could reorder without any
/// behavioural test noticing, and the first one is the whole of section 25.12's
/// `NOT_RELEVANT is respected`. A whole-text pin refuses every edit to the
/// function rather than the edits somebody thought of.
const DETECT_ORDER: &str = "pub fn detect( scope: &BlindSpotScope, resolver: &FieldResolver, ledger: \
     &DispositionLedger, readings: &[KeyReading], as_of: TimestampMillis, ) -> \
     Result<Vec<BlindSpotFinding>, BlindSpotError> { let mut coverage: BTreeMap<_, \
     FieldCoverage> = BTreeMap::new(); for reading in readings { let counted = \
     FieldCoverage::of(reading.key(), scope, resolver, reading.items())?; \
     coverage.insert(reading.key(), counted); } let all: Vec<FieldCoverage> = \
     coverage.values().cloned().collect(); let cause = SkewExplanation::of(scope, &all); \
     let mut findings = Vec::new(); for reading in readings { let Some(counted) = \
     coverage.get(&reading.key()) else { continue; }; let standing = \
     ledger.standing(reading.key()); if let Some(choice) = standing && choice.field() != \
     reading.key() { return Err(BlindSpotError::DispositionIsAboutAnotherField); } let \
     disposition = standing.map(|choice| choice.disposition()); let basis = if let \
     Some(choice) = standing.filter(|held| held.disposition() == \
     UserDisposition::NotRelevant) { \
     StateBasis::UserExcluded(ScopeExclusion::of(choice)?) } else if let Some(block) = \
     reading.goal_block() { StateBasis::ActiveGoalBlocked(block) } else if \
     counted.evidence_count() < scope.minimum_exposure() { \
     StateBasis::CoverageBelowMinimum(BelowMinimum::of( counted.evidence_count(), \
     scope.minimum_exposure(), )?) } else if !counted.failed_attempts().is_empty() { \
     StateBasis::DifficultyObserved(ObservedDifficulty::of( \
     counted.failed_attempts().to_vec(), )?) } else if let Some(band) = reading .band() \
     .filter(|held| LOW_RECENCY_BANDS.contains(held)) { \
     StateBasis::RecencyLow(LowRecency::of(band)?) } else { continue; }; let relevance = \
     reading.relevance(); let copy = NeutralPresentation::of(state_of(&basis), \
     relevance.clone()); let presentation = if disposition == \
     Some(UserDisposition::Explore) { let Some(choice) = standing else { return \
     Err(BlindSpotError::ExploreWithoutAStep); }; let Some(step) = reading.taste_step() \
     else { return Err(BlindSpotError::ExploreWithoutAStep); }; \
     FindingPresentation::Explore { presentation: copy, path: \
     TastePath::for_explore(choice, reading.key(), step)?, } } else { \
     FindingPresentation::Neutral { presentation: copy } }; let warns = \
     !standing.is_some_and(|choice| choice.suppresses_warning_at(as_of)); \
     findings.push(BlindSpotFinding::assemble( reading.key(), scope.label(), \
     counted.evidence_count(), counted.diversity(), basis, relevance.clone(), \
     cause.clone(), disposition, presentation, warns, )); } Ok(findings) }";

/// Every public function of this crate's product code, as its module, its
/// name and its return type.
///
/// Compared in both directions, which is the closure a name list cannot
/// give. A method that folded a coverage reading into a number, a second
/// producer of a finding, a shipped scope, a copy string reachable outside
/// `renderable_copy`, or a ledger operation that drops a standing choice each
/// appear here as an **extra key** whatever they are called — and each was
/// injected past the earlier name lists before this comparison existed.
const PUBLIC_SIGNATURES: [&str; 114] = [
    "coverage.rs as_str -> &'static str",
    "coverage.rs as_str -> &'static str",
    "coverage.rs by_source -> &BTreeMap<ExposureSource, u32>",
    "coverage.rs concept -> EntityId",
    "coverage.rs design_token -> &'static str",
    "coverage.rs diversity -> EvidenceDiversity",
    "coverage.rs evidence -> &EligibleEvidence",
    "coverage.rs evidence_count -> u32",
    "coverage.rs evidence_id -> EvidenceId",
    "coverage.rs failed_attempts -> &[EvidenceId]",
    "coverage.rs key -> EntityId",
    "coverage.rs newest -> Option<TimestampMillis>",
    "coverage.rs observed_at -> TimestampMillis",
    "coverage.rs of -> Result<Self, BlindSpotError>",
    "coverage.rs of -> Self",
    "coverage.rs of_distinct_sources -> Self",
    "coverage.rs records_difficulty -> bool",
    "coverage.rs source -> ExposureSource",
    "coverage.rs sources -> BTreeSet<ExposureSource>",
    "detector.rs detect -> Result<Vec<BlindSpotFinding>, BlindSpotError>",
    "disposition.rs as_str -> &'static str",
    "disposition.rs chosen_at -> TimestampMillis",
    "disposition.rs disposition -> UserDisposition",
    "disposition.rs field -> EntityId",
    "disposition.rs fields -> Vec<EntityId>",
    "disposition.rs hidden_until -> Option<TimestampMillis>",
    "disposition.rs is_empty -> bool",
    "disposition.rs len -> usize",
    "disposition.rs needs_deadline -> bool",
    "disposition.rs new -> Self",
    "disposition.rs record -> Result<Self, BlindSpotError>",
    "disposition.rs scope_id -> ScopeId",
    "disposition.rs standing -> Option<&UserDispositionChoice>",
    "disposition.rs suppresses_warning_at -> bool",
    "disposition.rs user_id -> EntityId",
    "disposition.rs verify -> Result<Self, BlindSpotError>",
    "explanation.rs concentrated -> &[EntityId]",
    "explanation.rs drivers -> &[ExposureDriver]",
    "explanation.rs of -> Self",
    "explanation.rs sparse -> &[EntityId]",
    "finding.rs basis -> &StateBasis",
    "finding.rs classification -> BlindSpotState",
    "finding.rs evidence_diversity -> EvidenceDiversity",
    "finding.rs exposure_evidence_count -> u32",
    "finding.rs field -> EntityId",
    "finding.rs likely_cause -> &SkewExplanation",
    "finding.rs presentation -> &FindingPresentation",
    "finding.rs relevance_to_active_goals -> &GoalRelevance",
    "finding.rs scope -> &str",
    "finding.rs to_wire -> BlindSpotFindingWire",
    "finding.rs user_disposition -> Option<UserDisposition>",
    "finding.rs warns -> bool",
    "presentation.rs emphasis -> &'static str",
    "presentation.rs headline -> &'static str",
    "presentation.rs headline -> &'static str",
    "presentation.rs of -> Self",
    "presentation.rs path -> Option<&TastePath>",
    "presentation.rs presentation -> &NeutralPresentation",
    "presentation.rs relevance -> &GoalRelevance",
    "presentation.rs renderable_copy -> Vec<&'static str>",
    "presentation.rs state -> BlindSpotState",
    "presentation.rs uncertainty -> &'static str",
    "reading.rs band -> Option<FreshnessBand>",
    "reading.rs goal_block -> Option<GoalBlock>",
    "reading.rs items -> &[ExposureItem]",
    "reading.rs key -> EntityId",
    "reading.rs of -> Self",
    "reading.rs relevance -> &GoalRelevance",
    "reading.rs taste_step -> Option<TasteStep>",
    "reading.rs with_band -> Self",
    "reading.rs with_goal_block -> Self",
    "reading.rs with_relevance -> Self",
    "reading.rs with_taste_step -> Self",
    "relevance.rs as_str -> &'static str",
    "relevance.rs citing_goals -> &BTreeSet<EntityId>",
    "relevance.rs is_low -> bool",
    "relevance.rs none -> Self",
    "relevance.rs of -> Self",
    "resolution.rs granularity -> TaxonomyGranularity",
    "resolution.rs keys -> Vec<EntityId>",
    "resolution.rs of -> Self",
    "resolution.rs resolve -> Option<EntityId>",
    "scope.rs as_str -> &'static str",
    "scope.rs as_str -> &'static str",
    "scope.rs between -> Result<Self, BlindSpotError>",
    "scope.rs granularity -> TaxonomyGranularity",
    "scope.rs holds -> bool",
    "scope.rs label -> String",
    "scope.rs minimum_exposure -> u32",
    "scope.rs select -> Result<Self, BlindSpotError>",
    "scope.rs taxonomy -> &TaxonomyVersionIdentity",
    "scope.rs tier -> EntityKind",
    "scope.rs window -> ObservationWindow",
    "state.rs as_str -> &'static str",
    "state.rs attempts -> &[EvidenceId]",
    "state.rs band -> FreshnessBand",
    "state.rs blocking_concept -> EntityId",
    "state.rs goal -> EntityId",
    "state.rs meaning -> &'static str",
    "state.rs minimum -> u32",
    "state.rs observed -> u32",
    "state.rs of -> Result<Self, BlindSpotError>",
    "state.rs of -> Result<Self, BlindSpotError>",
    "state.rs of -> Result<Self, BlindSpotError>",
    "state.rs of -> Result<Self, BlindSpotError>",
    "state.rs of -> Result<Self, BlindSpotError>",
    "state.rs scope_id -> ScopeId",
    "state.rs state_of -> BlindSpotState",
    "state.rs user_id -> EntityId",
    "taste.rs as_str -> &'static str",
    "taste.rs design_token -> &'static str",
    "taste.rs for_explore -> Result<Self, BlindSpotError>",
    "taste.rs key -> EntityId",
    "taste.rs step -> TasteStep",
];

/// The one public function of this crate that produces a finding.
const FINDING_PRODUCERS: [&str; 1] = ["detect"];

/// The product files permitted to name `BlindSpotFinding::assemble`.
const ASSEMBLE_SITES: [&str; 2] = [
    "crates/blind-spot/src/detector.rs",
    "crates/blind-spot/src/finding.rs",
];

/// The product files permitted to name `Default` at all.
///
/// `DispositionLedger`'s default is the empty ledger. `BlindSpotScope` has none,
/// and section 23's `사용자가 선택한다` is what that absence holds.
const DEFAULT_SITES: [&str; 1] = ["crates/blind-spot/src/disposition.rs"];

/// The files of this package permitted to read anything at all.
const READERS: [&str; 2] = [
    "crates/blind-spot/tests/blind_spot.rs",
    "crates/blind-spot/tests/blind_spot_scans.rs",
];

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let declared = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    let mut modules: Vec<String> = declared
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .map(|rest| format!("{}.rs", rest.trim_end_matches(';')))
        .collect();
    modules.push("lib.rs".to_owned());
    modules.sort();

    let mut walked: Vec<String> = crate_product_sources()?
        .iter()
        .map(|path| module_of(&relative(path)))
        .collect();
    walked.sort();

    // Both directions. A module added to `src/` without a `pub mod` line, and a
    // `pub mod` line without a file, are each a failure here.
    assert_eq!(walked, modules, "the walk and lib.rs disagree");
    assert!(
        walked.len() >= 13,
        "the walk found only {} product files",
        walked.len()
    );

    // Every `mod` and `#[path]` target in the package is a file the walk read.
    for (path, code) in product_code()? {
        for line in code.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
            else {
                continue;
            };
            let name = format!("{}.rs", rest.trim_end_matches(';').trim());
            assert!(
                walked.contains(&name),
                "{path} declares {name} and the walk did not read it"
            );
        }
    }
    Ok(())
}

#[test]
fn the_blind_spot_crate_cannot_name_a_mastery() -> TestResult {
    let files = product_code()?;
    assert!(files.len() >= 13, "the product walk found {}", files.len());

    // 1. Every `use` item, in both directions.
    let mut items: Vec<String> = Vec::new();
    for (_, code) in &files {
        items.extend(use_items(code));
    }
    items.sort();
    items.dedup();
    assert_eq!(
        items,
        USE_ITEMS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "this crate's use items and USE_ITEMS disagree"
    );

    // 2. Every two-segment path reached through a crate root, in both
    //    directions. `use` items are removed first, so a re-export is not
    //    counted as a reach.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    for (_, code) in &files {
        reached.extend(absolute_paths(&without_use_items(code)));
    }
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate's reached paths and REACHED_PATHS disagree"
    );

    // 3. Every macro invoked, in both directions. `include_str!` spells no
    //    listed token and adds no `use` item, which is why this set is compared
    //    rather than searched.
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in &files {
        macros.extend(macros_spelled(code));
    }
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate's macros and MACROS_SPELLED disagree"
    );

    // 4. The modules `lib.rs` hands names out of.
    let lib = strip_non_code(&fs::read_to_string(crate_root().join("src/lib.rs"))?);
    assert_eq!(
        re_export_modules(&lib),
        RE_EXPORT_MODULES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "lib.rs re-exports from other modules than RE_EXPORT_MODULES"
    );

    // 5. And the rule the four sets exist to hold: no product file of this
    //    crate names anything that means a mastery level, and neither does any
    //    public signature.
    for (path, code) in &files {
        for name in MASTERY_NAMES {
            assert_eq!(
                uses_of(code, name),
                0,
                "{path} names {name}, so a coverage reading could become one"
            );
        }
        for (function, signature) in public_signatures(code) {
            for name in MASTERY_NAMES {
                assert_eq!(
                    uses_of(&signature, name),
                    0,
                    "{path}'s {function} takes or returns {name}"
                );
            }
        }
    }

    // The control. The same reader is required to find most of the eight in
    // `P2-N2`'s own ladder, so the zero it reports above is a measurement rather
    // than a reader that always answers zero.
    let ladder = strip_non_code(&fs::read_to_string(
        workspace_root().join("crates/knowledge-state/src/ladder.rs"),
    )?);
    let found = MASTERY_NAMES
        .iter()
        .filter(|name| uses_of(&ladder, name) > 0)
        .count();
    assert!(
        found >= 5,
        "the mastery reader found only {found} of the eight names in P2-N2's own ladder"
    );

    // 6. And the folding half: no product file spells a way to turn a coverage
    //    reading into one number.
    for (path, code) in &files {
        for name in SCORE_NAMES {
            assert_eq!(
                uses_of(code, name),
                0,
                "{path} names {name}, so coverage could be folded into a score"
            );
        }
    }
    // `FieldCoverage` and `EvidenceDiversity` are not orderable, read out of the
    // derive attribute above each declaration rather than assumed.
    let coverage = read_module("coverage.rs")?;
    for header in ["pub struct FieldCoverage", "pub enum EvidenceDiversity"] {
        let at = coverage
            .find(header)
            .ok_or_else(|| format!("{header} is not in coverage.rs"))?;
        let derive_start = coverage[..at]
            .rfind("#[derive(")
            .ok_or_else(|| format!("{header} has no derive attribute"))?;
        let derive = &coverage[derive_start..at];
        for trait_name in ["PartialOrd", "Ord"] {
            assert_eq!(
                uses_of(derive, trait_name),
                0,
                "{header} derives {trait_name}, so two readings can be ranked"
            );
        }
    }
    Ok(())
}

#[test]
fn the_finding_has_exactly_one_producer() -> TestResult {
    let files = product_code()?;
    let mut producers: Vec<String> = Vec::new();
    for (_, code) in &files {
        for (function, signature) in public_signatures(code) {
            if uses_of(&signature, "BlindSpotFinding") > 0 {
                producers.push(function);
            }
        }
    }
    producers.sort();
    producers.dedup();
    assert_eq!(
        producers,
        FINDING_PRODUCERS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "this crate's finding producers and FINDING_PRODUCERS disagree"
    );

    // The control: the same reader finds the accessors it is not counting, so a
    // filter that matched nothing would not pass here.
    let finding = read_module("finding.rs")?;
    let names: Vec<String> = public_signatures(&finding)
        .into_iter()
        .map(|(function, _)| function)
        .collect();
    assert!(
        names.len() >= 10,
        "the signature reader found only {} public functions in finding.rs",
        names.len()
    );
    assert!(names.contains(&"to_wire".to_owned()));

    // And the private constructor is reachable from exactly two files, so a
    // second producer cannot be built without appearing here.
    let mut sites: Vec<String> = Vec::new();
    for (path, code) in &files {
        if uses_of(code, "assemble") > 0 {
            sites.push(path.clone());
        }
    }
    sites.sort();
    assert_eq!(
        sites,
        ASSEMBLE_SITES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "BlindSpotFinding::assemble is reached from other files than ASSEMBLE_SITES"
    );

    // The ledger cannot be edited: no removal, no clearing, no `&mut self`.
    let disposition = read_module("disposition.rs")?;
    for removal in ["remove", "clear", "retain", "drain", "take", "delete"] {
        assert_eq!(
            uses_of(&disposition, removal),
            0,
            "the ledger names {removal}, so a rerun could drop a standing choice"
        );
    }
    Ok(())
}

#[test]
fn the_presentation_carries_no_free_text() -> TestResult {
    let files = product_code()?;

    // No product file of this crate names a warning.
    for (path, code) in &files {
        for name in WARNING_NAMES {
            assert_eq!(
                uses_of(code, name),
                0,
                "{path} names {name}, so a blind spot could be shown as a warning"
            );
        }
    }

    // The two presentation types are pinned whole, so a free-text field added to
    // either is a failure here rather than a slot a demand arrives through.
    let presentation = read_module("presentation.rs")?;
    assert_eq!(
        type_declaration(&presentation, "pub struct NeutralPresentation")?,
        "pub struct NeutralPresentation { state: BlindSpotState, relevance: GoalRelevance, }"
    );
    assert_eq!(
        type_declaration(&presentation, "pub enum FindingPresentation")?,
        "pub enum FindingPresentation { Neutral { presentation: NeutralPresentation, }, \
         Explore { presentation: NeutralPresentation, path: TastePath, }, }"
    );
    assert_eq!(
        free_function(
            &presentation,
            "pub const fn headline(state: BlindSpotState)"
        )?,
        "pub const fn headline(state: BlindSpotState) -> &'static str { match state { \
         BlindSpotState::Unobserved => CANNOT_INFER_ABILITY, BlindSpotState::Weak \
         | BlindSpotState::Stale | BlindSpotState::OutOfScope | BlindSpotState::Gap \
         => state.meaning(), } }"
    );

    // And neither presentation type owns a string.
    for header in [
        "pub struct NeutralPresentation",
        "pub enum FindingPresentation",
    ] {
        let declaration = type_declaration(&presentation, header)?;
        for owned in ["String", "Cow"] {
            assert_eq!(
                uses_of(&declaration, owned),
                0,
                "{header} carries a {owned}"
            );
        }
    }
    Ok(())
}

#[test]
fn no_goal_vocabulary_reaches_this_crate() -> TestResult {
    for (path, code) in product_code()? {
        for name in GOAL_NAMES {
            assert_eq!(
                uses_of(&code, name),
                0,
                "{path} names {name}, so this engine could emit a goal"
            );
        }
    }

    // The manifest, with comment lines stripped first, so the reasons a comment
    // gives for an edge it does not have are not read as the edge.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let declared: String = manifest
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for edge in FORBIDDEN_EDGES {
        assert!(
            !declared.contains(edge),
            "the manifest declares a {edge} edge"
        );
    }
    // The control: the reader does see the edges this crate does have.
    for edge in ["academic-domain", "academic-knowledge-state"] {
        assert!(
            declared.contains(edge),
            "the manifest reader cannot see the {edge} edge, so its refusals are vacuous"
        );
    }
    Ok(())
}

#[test]
fn no_clock_socket_or_file_reaches_this_crate() -> TestResult {
    let files = product_code()?;
    let mut scanned = 0_usize;
    for (path, code) in &files {
        scanned += 1;
        for construct in FORBIDDEN_CONSTRUCTS {
            let spelled = if construct.contains("::") {
                absolute_paths(code).contains(construct)
            } else {
                uses_of(code, construct) > 0
            };
            assert!(!spelled, "{path} spells {construct}");
        }
    }
    assert!(scanned >= 13, "only {scanned} product files were scanned");
    Ok(())
}

#[test]
fn only_the_named_test_files_read_anything() -> TestResult {
    let mut readers: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let reads = uses_of(&code, "read_to_string") > 0
            || uses_of(&code, "include_str") > 0
            || uses_of(&code, "File") > 0
            || uses_of(&code, "read_dir") > 0;
        if reads {
            readers.push(relative(&path));
        }
    }
    readers.sort();
    assert_eq!(
        readers,
        READERS
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "the files of this package that read something are not READERS"
    );
    Ok(())
}

#[test]
fn the_blind_spot_decisions_are_pinned() -> TestResult {
    let state = read_module("state.rs")?;
    let coverage = read_module("coverage.rs")?;
    let disposition = read_module("disposition.rs")?;
    let detector = read_module("detector.rs")?;
    let taste = read_module("taste.rs")?;

    // The bands `최근성 낮음` may be read off. `UNKNOWN` is not one of them, and
    // no behavioural test can say that a fourth band was not added.
    assert_eq!(
        const_declaration(&state, "LOW_RECENCY_BANDS")?,
        "const LOW_RECENCY_BANDS: [FreshnessBand; 2] = [FreshnessBand::Stale, FreshnessBand::Low];"
    );

    // The basis-to-state map, whole. A second basis landing on one state would
    // make the five names four.
    assert_eq!(
        free_function(&state, "pub const fn state_of(basis: &StateBasis)")?,
        "pub const fn state_of(basis: &StateBasis) -> BlindSpotState { match basis { \
         StateBasis::CoverageBelowMinimum(_) => BlindSpotState::Unobserved, \
         StateBasis::DifficultyObserved(_) => BlindSpotState::Weak, \
         StateBasis::RecencyLow(_) => BlindSpotState::Stale, \
         StateBasis::UserExcluded(_) => BlindSpotState::OutOfScope, \
         StateBasis::ActiveGoalBlocked(_) => BlindSpotState::Gap, } }"
    );

    // The diversity split point, whole. Section 23 exhibits one token and the
    // acceptance case pins where it holds; this pins the whole rule.
    assert_eq!(
        whole_block(
            &coverage,
            "pub const fn of_distinct_sources(distinct: usize)"
        )?,
        "pub const fn of_distinct_sources(distinct: usize) -> Self { \
         if distinct > 1 { Self::Mixed } else { Self::Low } }"
    );

    // What suppresses a warning and for how long. `NOT_RELEVANT` has no expiry
    // and `HIDE_UNTIL` has exactly one.
    assert_eq!(
        whole_block(
            &disposition,
            "pub fn suppresses_warning_at(self, as_of: TimestampMillis)"
        )?,
        "pub fn suppresses_warning_at(self, as_of: TimestampMillis) -> bool { \
         match self.disposition { UserDisposition::NotRelevant => true, \
         UserDisposition::HideUntil => self .hidden_until \
         .is_some_and(|until| as_of.value() < until.value()), \
         UserDisposition::Explore | UserDisposition::Later => false, } }"
    );

    // The classification order, whole.
    assert_eq!(free_function(&detector, "pub fn detect(")?, DETECT_ORDER);
    assert!(
        DETECT_ORDER.len() > 100,
        "the detect pin is only {} characters",
        DETECT_ORDER.len()
    );

    // The misattribution guard, which `P2-N2` found one layer up and `P2-N3`
    // one hop out.
    let counted = whole_block(
        &coverage,
        "pub fn of(
        key: EntityId,",
    )?;
    for guard in [
        "let Some(resolved) = resolver.resolve(item.concept()) else { return Err(BlindSpotError::ItemIsOutsideTheTaxonomy(item.evidence_id())); };",
        "if resolved != key { return Err(BlindSpotError::ItemIsAboutAnotherKey { expected: key, found: resolved, }); }",
        "if !scope.window().holds(item.observed_at()) { continue; }",
    ] {
        assert!(
            counted.contains(guard),
            "FieldCoverage::of no longer contains {guard:?}"
        );
    }

    // And the taste path's two refusals.
    let path = whole_block(&taste, "pub fn for_explore(")?;
    for guard in [
        "if choice.disposition() != UserDisposition::Explore { return Err(BlindSpotError::TastePathNeedsExplore); }",
        "if choice.field() != key { return Err(BlindSpotError::TastePathIsAboutAnotherField); }",
    ] {
        assert!(
            path.contains(guard),
            "TastePath::for_explore no longer contains {guard:?}"
        );
    }
    Ok(())
}

#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    for (path, code) in product_code()? {
        for (function, signature) in public_signatures(&code) {
            assert!(
                !signature.contains("&mut self"),
                "{path}'s {function} takes &mut self"
            );
        }
    }

    // And the shipped-value half: the only product file that may name `Default`
    // is the ledger's, so a `Default` for the scope the user is supposed to
    // choose is an extra key here.
    let mut sites: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        if uses_of(&code, "Default") > 0 {
            sites.push(path);
        }
    }
    sites.sort();
    assert_eq!(
        sites,
        DEFAULT_SITES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "the files naming Default are not DEFAULT_SITES"
    );
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    // Every extractor, against a sample it must match.
    let sample = "use a::b::{c, d as e};\npub use crate::f::G;\nfn h() { std::fs::read(\"x\"); format!(\"y\"); }\n";
    let stripped = strip_non_code(sample);
    // The flattener drops the whitespace inside an alias, and this records that
    // rather than assuming otherwise: what matters is that both leaves appear.
    assert_eq!(use_items(&stripped), vec!["a::b::c", "a::b::dase"]);
    assert_eq!(re_export_modules(&stripped), vec!["crate"]);
    assert!(absolute_paths(&without_use_items(&stripped)).contains("std::fs"));
    assert!(macros_spelled(&stripped).contains("format"));
    assert_eq!(uses_of("alpha beta alphabet", "alpha"), 1);
    assert_eq!(uses_of("alpha beta alphabet", "gamma"), 0);
    assert_eq!(
        strip_non_code("let x = \"std::fs\"; // std::net\n").trim(),
        "let x =  ;"
    );
    assert_eq!(tighten("std :: env :: var"), "std::env::var");
    assert_eq!(
        public_signatures("pub fn a(&mut self) -> u8 {\n}\n")
            .into_iter()
            .map(|(name, signature)| (name, signature.contains("&mut self")))
            .collect::<Vec<_>>(),
        vec![("a".to_owned(), true)]
    );
    assert!(free_function("pub fn z(v: u8) -> u8 {\n    v\n}\n", "pub fn z(v: u8)")?.contains('v'));
    assert!(whole_block("impl A { pub fn z() -> u8 { 1 } }", "pub fn z()")?.contains('1'));
    assert!(type_declaration("pub struct S {\n    a: u8,\n}\n", "pub struct S")?.contains("a: u8"));
    assert!(const_declaration("const K: u8 = 3;\n", "K")?.contains('3'));
    assert!(free_function("fn a() {\n}\n", "fn missing(").is_err());
    assert!(const_declaration("const K: u8 = 3;\n", "MISSING").is_err());
    assert!(type_declaration("pub struct S {\n}\n", "pub struct MISSING").is_err());

    // Each forbidden-name list is required to be found in a sample that spells
    // it, so a list nothing could ever match is a failure here.
    for name in MASTERY_NAMES {
        assert_eq!(
            uses_of(&format!("x {name} y"), name),
            1,
            "{name} is unmatchable"
        );
    }
    for name in SCORE_NAMES {
        assert_eq!(
            uses_of(&format!("x {name} y"), name),
            1,
            "{name} is unmatchable"
        );
    }
    for name in WARNING_NAMES {
        assert_eq!(
            uses_of(&format!("x {name} y"), name),
            1,
            "{name} is unmatchable"
        );
    }
    for name in GOAL_NAMES {
        assert_eq!(
            uses_of(&format!("x {name} y"), name),
            1,
            "{name} is unmatchable"
        );
    }

    // The whole-text pins are required to extract something substantial, so a
    // pin against an empty string is a failure here.
    let state = read_module("state.rs")?;
    assert!(free_function(&state, "pub const fn state_of(basis: &StateBasis)")?.len() > 100);
    let presentation = read_module("presentation.rs")?;
    assert!(type_declaration(&presentation, "pub enum FindingPresentation")?.len() > 100);
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
fn every_public_signature_is_in_the_inventory() -> TestResult {
    let mut found: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        let module = module_of(&path);
        for (name, signature) in public_signatures(&code) {
            let tail = signature
                .split_once("->")
                .map_or("()", |(_, rest)| rest)
                .trim();
            let tail = tail.split_whitespace().collect::<Vec<_>>().join(" ");
            found.push(format!("{module} {name} -> {tail}"));
        }
    }
    found.sort();
    assert_eq!(
        found,
        PUBLIC_SIGNATURES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "this crate's public signatures and PUBLIC_SIGNATURES disagree"
    );
    assert!(
        found.len() >= 110,
        "the signature reader found only {} public functions",
        found.len()
    );

    // The control: the same reader is required to see a return type it is being
    // asked to notice, so an extractor that always answered `()` would not pass.
    assert!(
        found.iter().any(|entry| entry.ends_with("-> u32")),
        "the reader reports no numeric return type at all"
    );
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    // Every scan in this file, read out of this file rather than listed here, so
    // a scan added later without a row is a failure. The committed row was
    // already stale by the time this crate had twelve scans, and the
    // re-derivation control found it; the list this reads is the file's own.
    let source = fs::read_to_string(crate_root().join("tests/blind_spot_scans.rs"))?;
    let mut declared: Vec<String> = Vec::new();
    let mut previous_is_test = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if previous_is_test
            && let Some(rest) = trimmed.strip_prefix("fn ")
            && let Some(name) = rest.split('(').next()
        {
            declared.push(name.to_owned());
        }
        previous_is_test = trimmed == "#[test]";
    }
    declared.sort();
    assert!(
        declared.len() >= 12,
        "the scan reader found only {} tests in this file",
        declared.len()
    );
    assert!(
        page.contains("crates/blind-spot/tests/blind_spot_scans.rs"),
        "the inventory has no row naming this file"
    );
    for name in &declared {
        assert!(
            page.contains(name),
            "the inventory has no row naming {name}"
        );
    }
    Ok(())
}
