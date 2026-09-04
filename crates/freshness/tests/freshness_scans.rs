//! What `academic-freshness` may reach, hold and hand out.
//!
//! ## The claim this file exists for
//!
//! `time_decay_touches_freshness_only` drives the behaviour: a clock swept
//! forward moves the band and leaves the level alone. That is a statement about
//! the code paths the test exercised. **The statement this task actually makes
//! is stronger** — that no code path *could* do otherwise, because this crate
//! has no name for a mastery level at all — and a behavioural test cannot say
//! that. `the_freshness_crate_cannot_name_a_mastery` says it, as three
//! whole-set comparisons in both directions:
//!
//! * every `use` item ([`USE_ITEMS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! `academic-knowledge-state` is a product edge and it re-exports `LADDER`,
//! `rung`, `level_token`, `AutomaticLevel` and `MasteryProjection`;
//! `academic_domain::MasteryLevel` is one `use` away. A reach for any of them
//! appears above as an **extra key** rather than as a token nobody listed, which
//! is the shape `P2-R2` shipped and `docs/contracts/policy-source-scans.md`
//! records seven spellings defeating.
//!
//! The extractors are `crates/knowledge-state/tests/knowledge_state_scans.rs`'s,
//! restated the way that file restates `P2-R2`'s: a test module is not a library
//! target. `the_helpers_are_not_vacuous` re-exercises each of them here against
//! a sample it must match, because an extractor that always answers the empty
//! set would satisfy every comparison below.
//!
//! ## It reads no clock
//!
//! Section 13.3's decay is arithmetic on an argument. [`REACHED_PATHS`] holds no
//! `std::time`, [`USE_ITEMS`] imports no clock, and every instant this crate
//! holds arrived as a `TimestampMillis` parameter — so *this engine cannot ask
//! what time it is* is a property of the whole crate rather than a convention.

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

fn module_of(path: &str) -> String {
    let stem = path.rsplit('/').next().unwrap_or(path);
    stem.to_owned()
}

// ---------------------------------------------------------------------------
// The inventories.
// ---------------------------------------------------------------------------

/// Every `use` item of this crate's product code that is not a re-export.
///
/// Compared in both directions. A mastery name, a filesystem, clock, process or
/// transport import appears here as an **extra key** whatever it is called, and
/// a listed import that is removed appears as a missing one.
const USE_ITEMS: [&str; 39] = [
    "academic_domain::Actor",
    "academic_domain::Claim",
    "academic_domain::ClaimObject",
    "academic_domain::ConfidencePermille",
    "academic_domain::EntityId",
    "academic_domain::EpistemicStatus",
    "academic_domain::EvidenceId",
    "academic_domain::EvidenceItem",
    "academic_domain::FreshnessBand",
    "academic_domain::ScopeId",
    "academic_domain::TimestampMillis",
    "academic_domain::predicates::PredicateName",
    "academic_knowledge_state::EligibleEvidence",
    "academic_knowledge_state::EvidenceKind",
    "academic_knowledge_state::FreshnessInput",
    "crate::FreshnessError",
    "crate::band::FreshnessSignal",
    "crate::band::ceiling_of",
    "crate::band::floor_of",
    "crate::band::rank",
    "crate::band::step_down",
    "crate::decay::decay",
    "crate::evidence::DatedEvidence",
    "crate::evidence::Repetition",
    "crate::persistence::DAY_MILLIS",
    "crate::persistence::PersistenceClass",
    "crate::persistence::PersistenceWindow",
    "crate::persistence::PriorIdentity",
    "crate::persistence::RetentionPrior",
    "crate::persistence::elapsed_millis",
    "crate::projection::FreshnessProjection",
    "crate::recall::ContraryEvent",
    "crate::recall::RecallCheck",
    "crate::recall::RecallDirection",
    "crate::recall::RecallStatement",
    "crate::recall::UserRecall",
    "crate::spillover::Spillover",
    "serde::Deserialize",
    "serde::Serialize",
];

/// The modules `lib.rs` re-exports from.
///
/// `pub use` is a different act from `use`: it hands a name out rather than
/// reaching for one, so it is inventoried separately and by module.
const RE_EXPORT_MODULES: [&str; 8] = [
    "band",
    "decay",
    "disclosure",
    "evidence",
    "persistence",
    "projection",
    "recall",
    "spillover",
];

/// Every two-segment path this crate's product code reaches through a crate
/// root, `use` items excluded.
const REACHED_PATHS: [&str; 4] = [
    "academic_domain::DomainError",
    "crate::band",
    "crate::persistence",
    "thiserror::Error",
];

/// Every macro this crate's product code invokes.
const MACROS_SPELLED: [&str; 2] = ["format", "matches"];

/// Every name in this workspace that means a mastery level.
///
/// `academic-knowledge-state` is a product edge of this crate and hands all of
/// these out; `academic_domain::MasteryLevel` is one `use` away. This crate
/// reaches for none of them, which is what makes *the decay function cannot take
/// a mastery* a fact about the graph rather than a rule inside one signature.
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

/// The files of this package permitted to read anything at all.
const READERS: [&str; 2] = [
    "crates/freshness/tests/freshness.rs",
    "crates/freshness/tests/freshness_scans.rs",
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
        walked.len() >= 8,
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
fn the_freshness_crate_cannot_name_a_mastery() -> TestResult {
    let files = product_code()?;
    assert!(files.len() >= 8, "the product walk found {}", files.len());

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
    //    crate names anything that means a mastery level.
    for (path, code) in &files {
        for name in MASTERY_NAMES {
            assert_eq!(
                uses_of(code, name),
                0,
                "{path} names {name}, so a decay could take a mastery"
            );
        }
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
                tighten(code).contains(construct)
            } else {
                uses_of(code, construct) > 0
            };
            assert!(!spelled, "{path} spells {construct}");
        }
    }
    assert!(scanned >= 8, "only {scanned} product files were swept");

    // The manifest has no writer, worker or boundary edge, so nothing in the
    // closure can persist, launch or transport. Comment lines are stripped
    // first: this crate's manifest explains its edges at length, and a rule that
    // read the prose would pass on any spelling appearing only there.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let declared: String = manifest
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "academic-store",
        "academic-worker",
        "academic-egress-boundary",
        "academic-vault",
    ] {
        assert!(
            !declared.contains(forbidden),
            "the manifest declares {forbidden}"
        );
    }
    assert!(declared.contains("academic-knowledge-state"));
    Ok(())
}

#[test]
fn only_the_named_test_files_read_anything() -> TestResult {
    let mut readers: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        let reads = uses_of(&code, "read_to_string") > 0
            || uses_of(&code, "read_dir") > 0
            || macros_spelled(&code).contains("include_str");
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
        "a file of this package reads something and is not in READERS"
    );
    Ok(())
}

#[test]
fn the_freshness_decisions_are_pinned() -> TestResult {
    let decay = strip_non_code(&fs::read_to_string(crate_root().join("src/decay.rs"))?);
    let spillover = strip_non_code(&fs::read_to_string(crate_root().join("src/spillover.rs"))?);
    let projection = strip_non_code(&fs::read_to_string(crate_root().join("src/projection.rs"))?);
    let persistence = strip_non_code(&fs::read_to_string(
        crate_root().join("src/persistence.rs"),
    )?);

    // The decay function's whole text. Its signature is the separation and its
    // body is the band boundaries; an edit to either has to be an edit here.
    assert_eq!(free_function(&decay, "pub fn decay(")?, WHOLE_DECAY);

    // The two guards that make spillover one hop, whole.
    let direct = whole_block(&spillover, "pub fn direct(")?;
    assert!(
        direct.contains("if dated.iter().any(|item| item.concept() != neighbor) { return None; }"),
        "the neighbour-evidence guard is gone: {direct}"
    );
    assert!(
        direct.contains("if !edge.joins(neighbor) || dated.is_empty() { return None; }"),
        "the edge guard is gone: {direct}"
    );
    assert_eq!(whole_block(&spillover, "pub fn toward(")?, WHOLE_TOWARD);

    // The concept guard on the projection, whole. `P2-N2` found the same shape
    // one layer up and none of its named tests would have caught it.
    let about = free_function(&projection, "fn require_about(")?;
    for guard in [
        "item.concept() != concept",
        "statement.concept() != concept",
        "event.concept() != concept",
        "contribution.subject() != concept",
    ] {
        assert!(about.contains(guard), "{guard} is gone: {about}");
    }

    // The cap rule, whole: a cap applies when no raiser is more recent.
    assert!(
        collapse(&projection).contains(WHOLE_CAP_RULE),
        "the contrary-evidence cap rule changed"
    );

    // The shipped prior, whole. `GATE-38-024` is these five lines.
    assert_eq!(
        whole_block(
            &persistence,
            "pub const UNCALIBRATED_PRIOR_V1: RetentionPrior ="
        )?,
        WHOLE_SHIPPED_PRIOR
    );

    // `PersonalizationSpeed` has no `Default` anywhere in the crate, and no
    // constant of the type exists: that is the half of `GATE-38-024` a caller
    // cannot skip.
    for (path, code) in product_code()? {
        assert_eq!(
            uses_of(&code, "Default"),
            0,
            "{path} names Default, so a personalization speed may have one"
        );
        assert!(
            !code.contains(": PersonalizationSpeed ="),
            "{path} declares a personalization speed constant"
        );
    }
    Ok(())
}

/// [`crate::decay`]'s whole text.
const WHOLE_DECAY: &str = "pub fn decay(elapsed_millis: i64, window: PersistenceWindow) -> FreshnessBand { match permille_of_window(elapsed_millis, window) { ..500 => FreshnessBand::VeryHigh, 500..1000 => FreshnessBand::High, 1000..2000 => FreshnessBand::Moderate, 2000..4000 => FreshnessBand::Low, 4000.. => FreshnessBand::Stale, } }";

/// `Spillover::toward`'s whole text.
const WHOLE_TOWARD: &str = "pub fn toward(subject: EntityId, use_: NeighborUse) -> Option<Self> { if subject == use_.neighbor() || use_.edge().other_end(subject) != Some(use_.neighbor()) { return None; } let stepped = step_down(use_.band())?; Some(Self { band: floor_of(stepped, SPILLOVER_CEILING), neighbor_band: use_.band(), subject, neighbor: use_.neighbor(), at: use_.last_use(), edge: use_.edge, }) }";

/// The statement that makes a recall failure a cap rather than one more vote.
///
/// Compared through `collapse` rather than as raw text: the statement spans
/// lines, and a line-spanning pin is a pin on the platform's newline as much as
/// on the rule.
const WHOLE_CAP_RULE: &str = "let capped = cap.filter(|(_, at, _)| latest_raiser.is_none_or(|raiser| at.value() >= raiser.value()));";

/// The shipped prior's whole declaration. `GATE-38-024`.
const WHOLE_SHIPPED_PRIOR: &str = "pub const UNCALIBRATED_PRIOR_V1: RetentionPrior = RetentionPrior { identity: PriorIdentity::of(PriorName::UncalibratedV1, 1), basis: PriorBasis::NoEvidenceBasisEstablished, calibration: Calibration::Uncalibrated, exposure: PersistenceWindow(90), application: PersistenceWindow(360), }";

#[test]
fn no_public_function_takes_a_mastery_or_mutates_in_place() -> TestResult {
    let mut signatures = 0_usize;
    for (path, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            signatures += 1;
            assert!(
                !signature.contains("&mut self"),
                "{path}::{name} takes &mut self"
            );
            for mastery in MASTERY_NAMES {
                assert!(
                    !signature.contains(mastery),
                    "{path}::{name} names {mastery} in its signature"
                );
            }
        }
    }
    assert!(
        signatures >= 40,
        "only {signatures} public signatures were read"
    );
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    // Every extractor is exercised against a sample it must match. An extractor
    // that always answered the empty set would satisfy every comparison above.
    assert_eq!(
        use_items("use academic_domain::{EntityId, FreshnessBand};"),
        vec![
            "academic_domain::EntityId".to_owned(),
            "academic_domain::FreshnessBand".to_owned()
        ]
    );
    assert!(use_items("pub use crate::band::BANDS;").is_empty());
    assert_eq!(re_export_modules("pub use band::BANDS;"), vec!["band"]);
    assert!(absolute_paths("std::time::SystemTime::now()").contains("std::time"));
    assert!(absolute_paths("::std :: env :: vars_os()").contains("std::env"));
    assert!(macros_spelled("let s = include_str!(\"x\");").contains("include_str"));
    assert_eq!(uses_of("MasteryLevel::Applied", "MasteryLevel"), 1);
    assert_eq!(uses_of("NotMasteryLevelHere", "MasteryLevel"), 0);
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
    let decay = strip_non_code(&fs::read_to_string(crate_root().join("src/decay.rs"))?);
    assert!(free_function(&decay, "pub fn decay(")?.len() > 100);
    let persistence = strip_non_code(&fs::read_to_string(
        crate_root().join("src/persistence.rs"),
    )?);
    assert!(
        whole_block(
            &persistence,
            "pub const UNCALIBRATED_PRIOR_V1: RetentionPrior ="
        )?
        .len()
            > 100
    );
    assert!(free_function(&decay, "pub fn no_such_function(").is_err());

    // And the control on the mastery rule: the same reader is required to find
    // most of those names in a crate that does hand them out, so the zero this
    // file reports is a measurement rather than a reader that always answers
    // zero.
    let neighbour =
        fs::read_to_string(workspace_root().join("crates/knowledge-state/src/ladder.rs"))?;
    let neighbour = strip_non_code(&neighbour);
    let found: Vec<&str> = MASTERY_NAMES
        .into_iter()
        .filter(|name| uses_of(&neighbour, name) > 0)
        .collect();
    assert!(
        found.len() >= 5,
        "the reader found only {found:?} in P2-N2's ladder, so its zero here proves nothing"
    );
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    assert!(
        page.contains("crates/freshness/tests/freshness_scans.rs"),
        "this scan is not named in the inventory page"
    );
    for name in [
        "the_walk_reads_every_module_in_this_package",
        "the_freshness_crate_cannot_name_a_mastery",
        "no_clock_socket_or_file_reaches_this_crate",
        "only_the_named_test_files_read_anything",
        "the_freshness_decisions_are_pinned",
        "no_public_function_takes_a_mastery_or_mutates_in_place",
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
