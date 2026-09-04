//! What `academic-gap` may reach, hold and hand out.
//!
//! ## The claim this file exists for
//!
//! `generic_advice_fails_validation` drives the behaviour: a broad, fluent,
//! entirely reasonable-sounding recommendation is refused, and two rewordings of
//! it are refused identically. That is a statement about three sentences.
//! **The statement this task actually makes is stronger** — that the validator
//! *cannot* be lexical, because there is no text in it to compare against — and a
//! behavioural test cannot say that. [`the_gap_crate_holds_no_phrase_list`] says
//! it, as a whole-set comparison of every non-ASCII string literal in the
//! package against the design document's own cells, plus a whole-text pin on
//! `GapExplanation::defects` showing it reads no text at all.
//!
//! The other three pins are the decisions a later edit could move without any
//! test noticing: which rung each prerequisite strength needs
//! ([`WHOLE_BLOCKING_FLOOR`]), which band still counts as retrievable
//! ([`WHOLE_RETRIEVAL_FLOOR`]), which of the five kinds is a strong deficit
//! ([`WHOLE_STRONG_DEFICIT`]), and the two guards that keep one concept's
//! evidence out of another's judgement
//! ([`WHOLE_EVIDENCE_GUARD`] and [`WHOLE_PATH_SPILLOVER_GUARD`]).
//!
//! The extractors are `crates/freshness/tests/freshness_scans.rs`'s, restated
//! the way that file restates `P2-N2`'s and that file restates `P2-R2`'s: a test
//! module is not a library target. [`the_helpers_are_not_vacuous`] re-exercises
//! each of them here against a sample it must match, because an extractor that
//! always answered the empty set would satisfy every comparison below — and it
//! carries a **control**: the same reader is required to find most of a set of
//! names in a crate that does spell them, so a zero reported here is a
//! measurement rather than a reader that always answers zero.
//!
//! ## It reads no clock
//!
//! Every instant this crate holds arrived as a `TimestampMillis` inside a
//! `P2-N3` value. [`REACHED_PATHS`] holds no `std::time`, [`USE_ITEMS`] imports
//! no clock, and nothing here opens a file or a socket.

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
        // crate root. A **leading** `::` is not a middle segment:
        // `::std::path::Path::new(p)` is the absolute form of the same reach,
        // and `P2-R2` measured an earlier version of this function skipping it.
        let after_segment = start >= 3
            && &code[start - 2..start] == "::"
            && (bytes[start - 3].is_ascii_alphanumeric() || bytes[start - 3] == b'_');
        if after_segment {
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

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------
/// Every `use` item this crate's product code spells, in both directions.
///
/// Fifty-nine, and the shape of the list is the point: `academic_freshness` and
/// `academic_knowledge_state` are product edges, so a reach for anything they
/// hand out that is not below appears here as an **extra key** rather than as a
/// token nobody listed.
const USE_ITEMS: [&str; 59] = [
    "academic_domain::ConfidencePermille",
    "academic_domain::EntityId",
    "academic_domain::EvidenceId",
    "academic_domain::FreshnessBand",
    "academic_domain::MasteryLevel",
    "academic_domain::ScopeId",
    "academic_domain::entity_registry::EntityKind",
    "academic_domain::predicates::PredicateName",
    "academic_domain::predicates::PrerequisiteStrength",
    "academic_domain::predicates::prerequisite_descriptor",
    "academic_domain::question::QuestionStatus",
    "academic_freshness::FreshnessProjection",
    "academic_freshness::FreshnessSignal",
    "academic_freshness::Spillover",
    "academic_freshness::rank",
    "academic_knowledge_state::ConceptEvidence",
    "academic_knowledge_state::ConceptLink",
    "academic_knowledge_state::EligibilityOutcome",
    "academic_knowledge_state::EligibleEvidence",
    "academic_knowledge_state::EvidenceDossier",
    "academic_knowledge_state::EvidenceSufficiency",
    "academic_knowledge_state::SufficiencyGap",
    "academic_knowledge_state::UnseenBasis",
    "academic_knowledge_state::project",
    "academic_knowledge_state::rung",
    "crate::GapError",
    "crate::case::GapCase",
    "crate::case::RootCandidate",
    "crate::case::TieDiagnostic",
    "crate::case::roots_of",
    "crate::explanation::AlternativePath",
    "crate::explanation::ExplanationParts",
    "crate::explanation::GapExplanation",
    "crate::explanation::LinkedContext",
    "crate::explanation::MinimumRemediation",
    "crate::explanation::NoAlternativeReason",
    "crate::explanation::RemediationActivity",
    "crate::goal::ActiveGoal",
    "crate::graph::PrerequisiteEdge",
    "crate::graph::PrerequisiteGraph",
    "crate::kind::GapKind",
    "crate::node::IdentityStanding",
    "crate::node::gap_bearing",
    "crate::path::AncestorImpact",
    "crate::path::BlockingPath",
    "crate::path::PathStep",
    "crate::routing::BranchStanding",
    "crate::routing::route",
    "crate::state::ConceptState",
    "crate::state::OfferedEvidence",
    "crate::state::StateSnapshot",
    "serde::Deserialize",
    "serde::Deserializer",
    "serde::Serialize",
    "serde::Serializer",
    "serde::de",
    "std::collections::BTreeMap",
    "std::collections::BTreeSet",
    "std::collections::VecDeque",
];
const RE_EXPORT_MODULES: [&str; 10] = [
    "case",
    "engine",
    "explanation",
    "goal",
    "graph",
    "kind",
    "node",
    "path",
    "routing",
    "state",
];
const REACHED_PATHS: [&str; 7] = [
    "academic_domain::DomainError",
    "academic_domain::EntityId",
    "academic_domain::entity_registry",
    "academic_domain::predicates",
    "academic_knowledge_state::KnowledgeStateError",
    "crate::node",
    "thiserror::Error",
];
const MACROS_SPELLED: [&str; 2] = ["matches", "vec"];

/// Every name this crate spells that means a section 15.2 gap kind, a section
/// 15.3 field, or one of the two decisions the descent turns on.
///
/// Used as the control in [`the_helpers_are_not_vacuous`]: the same reader that
/// reports zero occurrences of these outside `crates/gap/src` is required to
/// find most of them inside it first.
const GAP_NAMES: [&str; 8] = [
    "GapKind",
    "GAP_KINDS",
    "StateDimension",
    "STATE_DIMENSIONS",
    "EXPLANATION_FIELDS",
    "SpecificityDefect",
    "RETRIEVAL_FLOOR",
    "blocking_floor",
];

/// The shapes a reader expects to see refused, kept as the weakest layer.
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

/// The files of this package permitted to read anything at all.
const READERS: [&str; 3] = [
    "crates/gap/tests/gap.rs",
    "crates/gap/tests/gap_scans.rs",
    "crates/gap/tests/common/mod.rs",
];

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    // A scan over a file list that quietly lost a module would report nothing
    // about that module, so the list is compared against `lib.rs`'s own
    // declarations in both directions before anything else runs.
    let lib = fs::read_to_string(crate_root().join("src/lib.rs"))?;
    let declared: BTreeSet<String> = lib
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .map(|rest| rest.trim_end_matches(';').to_owned())
        .collect();
    let walked: BTreeSet<String> = crate_product_sources()?
        .iter()
        .filter_map(|path| path.file_stem().and_then(|stem| stem.to_str()))
        .filter(|stem| *stem != "lib")
        .map(str::to_owned)
        .collect();
    assert_eq!(
        declared, walked,
        "lib.rs declares {declared:?} and the walk found {walked:?}"
    );
    assert!(
        declared.len() >= 9,
        "only {} modules were declared",
        declared.len()
    );

    // And the re-export modules `lib.rs` names are the same set.
    let re_exported: BTreeSet<String> = re_export_modules(&lib).into_iter().collect();
    assert_eq!(
        re_exported,
        RE_EXPORT_MODULES.into_iter().map(str::to_owned).collect(),
        "lib.rs re-exports from a different set of modules than the pin"
    );
    Ok(())
}

#[test]
fn the_gap_crate_holds_no_phrase_list() -> TestResult {
    // The whole `use` inventory, in both directions.
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        found.extend(use_items(&code));
    }
    let pinned: BTreeSet<String> = USE_ITEMS.into_iter().map(str::to_owned).collect();
    assert_eq!(found, pinned, "the crate's use items moved");

    // The whole set of two-segment paths reached through a crate root, in both
    // directions, over code with the `use` items removed so a re-export is not
    // counted as a reach.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        reached.extend(absolute_paths(&without_use_items(&code)));
    }
    let pinned: BTreeSet<String> = REACHED_PATHS.into_iter().map(str::to_owned).collect();
    assert_eq!(reached, pinned, "the crate's reached paths moved");

    // Every macro, in both directions.
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        macros.extend(macros_spelled(&code));
    }
    let pinned: BTreeSet<String> = MACROS_SPELLED.into_iter().map(str::to_owned).collect();
    assert_eq!(macros, pinned, "the crate's macros moved");

    // And the pin that carries the claim: `defects` names no text operation at
    // all, so the refusal cannot be a comparison against a phrase.
    let explanation = strip_non_code(&fs::read_to_string(
        crate_root().join("src/explanation.rs"),
    )?);
    let defects = whole_block(
        &explanation,
        "pub fn defects(&self) -> Vec<SpecificityDefect>",
    )?;
    assert_eq!(defects, WHOLE_DEFECTS, "the specificity validator moved");
    // `matches!` is a pattern macro and is not on this list; every entry below
    // is a way of reading text.
    for operation in [
        "contains",
        "starts_with",
        "ends_with",
        "find",
        "split",
        "chars",
        "trim",
        "to_lowercase",
        "eq_ignore_ascii_case",
        "description",
    ] {
        assert_eq!(
            uses_of(&defects, operation),
            0,
            "the specificity validator names {operation}, so it reads text"
        );
    }
    Ok(())
}

#[test]
fn no_clock_socket_or_file_reaches_this_crate() -> TestResult {
    for (path, code) in product_code()? {
        let tightened = tighten(&code);
        for construct in FORBIDDEN_CONSTRUCTS {
            assert_eq!(
                uses_of(&tightened, construct),
                0,
                "{path} names {construct}"
            );
        }
    }
    Ok(())
}

#[test]
fn only_the_named_test_files_read_anything() -> TestResult {
    for path in crate_all_sources()? {
        let relative = relative(&path);
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let reads = calls_of(&code, "read_to_string") + calls_of(&code, "read_dir");
        if reads == 0 {
            continue;
        }
        assert!(
            READERS.contains(&relative.as_str()),
            "{relative} reads from the filesystem and is not a named reader"
        );
    }
    Ok(())
}

#[test]
fn the_gap_decisions_are_pinned() -> TestResult {
    let graph = strip_non_code(&fs::read_to_string(crate_root().join("src/graph.rs"))?);
    assert_eq!(
        free_function(
            &graph,
            "pub const fn blocking_floor(strength: PrerequisiteStrength)"
        )?,
        WHOLE_BLOCKING_FLOOR,
        "the rung each strength needs moved"
    );

    let routing = strip_non_code(&fs::read_to_string(crate_root().join("src/routing.rs"))?);
    assert!(
        routing.contains(WHOLE_RETRIEVAL_FLOOR),
        "the retrieval floor moved"
    );
    assert_eq!(
        whole_block(&routing, "pub const fn is_strong_deficit(self) -> bool")?,
        WHOLE_STRONG_DEFICIT,
        "which of the five kinds is a strong deficit moved"
    );
    assert_eq!(
        free_function(
            &routing,
            "pub fn route(\n    state: &ConceptState,\n    floor: MasteryLevel,"
        )
        .or_else(|_| free_function(&routing, "pub fn route("))?,
        WHOLE_ROUTE,
        "the routing order moved"
    );

    let state = strip_non_code(&fs::read_to_string(crate_root().join("src/state.rs"))?);
    assert!(
        collapse(&state).contains(WHOLE_ADMITTED_GUARD),
        "the guard on the admitted half moved"
    );
    assert!(
        collapse(&state).contains(WHOLE_BLOCKED_GUARD),
        "the guard on the blocked half moved"
    );
    assert!(
        collapse(&state).contains(WHOLE_TRACE_GUARD),
        "the guard that stops a projection hiding a contribution moved"
    );

    let engine = strip_non_code(&fs::read_to_string(crate_root().join("src/engine.rs"))?);
    assert_eq!(
        free_function(&engine, "fn require_band_is_not_from_the_path(")?,
        WHOLE_PATH_SPILLOVER_GUARD,
        "the guard that refuses a band raised from the blocking path moved"
    );
    Ok(())
}

/// `blocking_floor`'s whole text: the rung each strength needs.
const WHOLE_BLOCKING_FLOOR: &str = "pub const fn blocking_floor(strength: PrerequisiteStrength) -> Option<MasteryLevel> { match strength { PrerequisiteStrength::Hard => Some(MasteryLevel::Practiced), PrerequisiteStrength::Strong => Some(MasteryLevel::Understood), PrerequisiteStrength::Helpful => None, } }";

/// The band below which section 15.2 calls immediate use uncertain.
const WHOLE_RETRIEVAL_FLOOR: &str =
    "pub const RETRIEVAL_FLOOR: FreshnessBand = FreshnessBand::Moderate;";

/// Which of section 15.2's five kinds is step 4's `강한 부족`.
const WHOLE_STRONG_DEFICIT: &str = "pub const fn is_strong_deficit(self) -> bool { match self { Self::MasteryGap => true, Self::FreshnessGap | Self::EvidenceGap | Self::OntologyGap | Self::ContextGap => false, } }";

/// The routing order. Moving a rule past another changes which kind a state
/// reports without changing any single rule.
const WHOLE_ROUTE: &str = "pub fn route( state: &ConceptState, floor: MasteryLevel, branch: &BranchStanding, ) -> Option<GapKind> { if !state.identity().is_settled() { return Some(GapKind::OntologyGap); } if !branch.is_settled() { return Some(GapKind::ContextGap); } if !state.contradicting().is_empty() { return Some(GapKind::MasteryGap); } if rung(state.mastery()) >= rung(floor) { if rank(state.freshness()) < rank(RETRIEVAL_FLOOR) { return Some(GapKind::FreshnessGap); } return None; } if state.unseen_basis() == Some(UnseenBasis::NoEvidenceRecorded) || state.sufficiency_gaps().iter().any(is_admission_gap) { return Some(GapKind::EvidenceGap); } Some(GapKind::MasteryGap) }";

/// The specificity validator's whole text. Eight structural rules and no text
/// operation among them.
const WHOLE_DEFECTS: &str = "pub fn defects(&self) -> Vec<SpecificityDefect> { let mut found = Vec::new(); if !gap_bearing(self.subject_kind) { found.push(SpecificityDefect::SubjectCarriesNoPrerequisite); } if self.blocks.tip() != self.subject || self.blocks.steps().is_empty() { found.push(SpecificityDefect::BlockingPathDoesNotReachSubject); } if self.evidence.is_empty() { found.push(SpecificityDefect::NoEvidenceCited); } if self.remediation.minutes() == 0 { found.push(SpecificityDefect::RemediationUnbounded); } if self.remediation.sources().is_empty() { found.push(SpecificityDefect::RemediationUncited); } if self.remediation.activity() != RemediationActivity::for_kind(self.kind) { found.push(SpecificityDefect::RemediationDoesNotMatchKind); } if matches!(&self.alternative, AlternativePath::Routes { routes } if routes.is_empty()) { found.push(SpecificityDefect::AlternativeIsEmpty); } if self.linked.is_empty() { found.push(SpecificityDefect::NoLinkedContext); } found }";

/// The guard on the **admitted** half: an item `P2-N2` admitted carries the
/// concept check one resolved, so `EligibleEvidence::concept` is the answer.
const WHOLE_ADMITTED_GUARD: &str =
    "if value.concept() != concept { return Err(GapError::EvidenceNamesAnotherConcept); }";

/// The guard on the **blocked** half: `BlockedEvidence` keeps the failing codes
/// and drops the link, so the dossier is the only place that answer survives.
/// Two guards over disjoint halves; neither can stand in for the other, which is
/// what `N5-I1` and `N5-I2` each observe by removing one.
const WHOLE_BLOCKED_GUARD: &str = "if item .linked_concept() .is_some_and(|linked| linked != concept) { return Err(GapError::EvidenceNamesAnotherConcept); }";

/// The guard that refuses a projection whose trace names contributions the
/// caller did not declare.
const WHOLE_TRACE_GUARD: &str =
    "if traced == declared { Ok(()) } else { Err(GapError::SpilloverNotDeclared) }";

/// The guard this task had to find: a band raised by a neighbour that lies on
/// the node's own blocking path.
const WHOLE_PATH_SPILLOVER_GUARD: &str = "fn require_band_is_not_from_the_path( state: &ConceptState, path: &BlockingPath, ) -> Result<(), GapError> { for source in state.spillover_sources() { if path.holds(source.neighbor) { return Err(GapError::FreshnessRestsOnPathSpillover { concept: state.concept(), neighbor: source.neighbor, predicate: source.predicate.as_str(), }); } } Ok(()) }";

#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let mut signatures = 0_usize;
    for (path, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            signatures += 1;
            assert!(
                !signature.contains("&mut self"),
                "{path}::{name} takes &mut self"
            );
        }
    }
    assert!(
        signatures >= 60,
        "only {signatures} public signatures were read"
    );
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    // Every extractor is exercised against a sample it must match. An extractor
    // that always answered the empty set would satisfy every comparison above.
    assert_eq!(
        use_items("use academic_domain::{EntityId, MasteryLevel};"),
        vec![
            "academic_domain::EntityId".to_owned(),
            "academic_domain::MasteryLevel".to_owned()
        ]
    );
    assert!(use_items("pub use crate::kind::GAP_KINDS;").is_empty());
    assert_eq!(re_export_modules("pub use kind::GAP_KINDS;"), vec!["kind"]);
    assert!(absolute_paths("std::time::SystemTime::now()").contains("std::time"));
    assert!(absolute_paths("::std :: env :: vars_os()").contains("std::env"));
    assert!(macros_spelled("let s = include_str!(\"x\");").contains("include_str"));
    assert_eq!(uses_of("GapKind::MasteryGap", "GapKind"), 1);
    assert_eq!(uses_of("NotGapKindHere", "GapKind"), 0);
    assert_eq!(
        public_signatures("pub fn f(&mut self) {\n}\n")
            .first()
            .map(|(name, _)| name.clone()),
        Some("f".to_owned())
    );
    assert_eq!(calls_of("fn g() {} let x = g();", "g"), 1);
    assert_eq!(
        strip_non_code("let s = \"std::fs\"; // std::net\n").trim(),
        "let s =  ;"
    );

    // The whole-text pins are compared against a constant, so a pin whose
    // extractor found nothing would compare two empty strings. Each extractor is
    // required to find something in this crate's own source first.
    let graph = strip_non_code(&fs::read_to_string(crate_root().join("src/graph.rs"))?);
    assert!(
        free_function(&graph, "pub const fn blocking_floor(")?.len() > 100,
        "the free-function reader found nothing to pin"
    );
    let routing = strip_non_code(&fs::read_to_string(crate_root().join("src/routing.rs"))?);
    assert!(
        whole_block(&routing, "pub const fn is_strong_deficit(")?.len() > 100,
        "the block reader found nothing to pin"
    );
    assert!(free_function(&graph, "pub fn no_such_function(").is_err());

    // And the control on the phrase rule. The same readers that report this
    // crate's use items, reached paths and gap names are required to find most
    // of those names in a package that does spell them, so the sets reported
    // above are measurements rather than a reader that always answers nothing.
    let neighbour = strip_non_code(&fs::read_to_string(
        workspace_root().join("crates/gap/src/lib.rs"),
    )?);
    let found: Vec<&str> = GAP_NAMES
        .into_iter()
        .filter(|name| uses_of(&neighbour, name) > 0)
        .collect();
    assert!(
        found.len() >= 6,
        "the reader found only {found:?} in this crate's own lib.rs, so what it \
         reports elsewhere proves nothing"
    );
    // The same reader over a crate that spells none of them answers zero, which
    // is the other half of the control.
    let unrelated = strip_non_code(&fs::read_to_string(
        workspace_root().join("crates/freshness/src/decay.rs"),
    )?);
    let leaked: Vec<&str> = GAP_NAMES
        .into_iter()
        .filter(|name| uses_of(&unrelated, name) > 0)
        .collect();
    assert!(
        leaked.is_empty(),
        "the reader found {leaked:?} in P2-N3's decay module"
    );
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    assert!(
        page.contains("crates/gap/tests/gap_scans.rs"),
        "this scan is not named in the inventory page"
    );
    for name in [
        "the_walk_reads_every_module_in_this_package",
        "the_gap_crate_holds_no_phrase_list",
        "no_clock_socket_or_file_reaches_this_crate",
        "only_the_named_test_files_read_anything",
        "the_gap_decisions_are_pinned",
        "no_public_function_mutates_in_place",
        "the_helpers_are_not_vacuous",
        "this_scan_is_in_the_inventory",
    ] {
        assert!(
            page.contains(name),
            "the inventory page does not name {name}"
        );
    }
    Ok(())
}
