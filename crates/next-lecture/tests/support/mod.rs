#![allow(
    dead_code,
    unused_imports,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]
//! Fixtures and source-reading helpers shared by the two `P2-L6` suites.
//!
//! # Nothing here invents a state
//!
//! `P2-N5`'s own fixture module is included by `#[path]` rather than restated,
//! the way that module includes `P2-N2`'s. So section 36.4's chain --
//! `Buffer Pool` requires `Disk Page` requires `Storage Hierarchy` -- is built
//! by driving a real `P2-L2` capture, a real `P2-L3` run, a real `P2-L4`
//! document, `P2-N2`'s own eligibility checks and `P2-N3`'s own `project`, and
//! this file adds only what section 12.7 needs on top: an ingested material, an
//! adjudicated `P2-G5` proposal over it, and the claim extracted from that.
//!
//! Section 36.4 is section 12.7's own example. `Tomorrow: Database / Buffer
//! Management`, `Expected: Disk Page, Buffer Pool, Replacement`, and
//! `Disk Page mastery: Exposed, freshness: Low` are the same three concepts and
//! the same reading, so the acceptance suite runs on the design document's own
//! scenario rather than on one this file made up.
//!
//! # The extractors
//!
//! They are `crates/integrations/tests/support/mod.rs`'s, restated because a
//! test module is not a library target. `the_helpers_are_not_vacuous`
//! re-exercises each of them against a sample it must match, because an
//! extractor that always answered the empty set would satisfy every whole-set
//! comparison in these suites.

#[path = "../../../gap/tests/common/mod.rs"]
pub mod gapfix;

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_domain::{
    ConfidencePermille, ContentDigest, EntityId, MasteryLevel, entity_registry::EntityKind,
    predicates::PrerequisiteStrength,
};
use academic_gap::{
    ActiveGoal, ConceptReading, ConceptState, GapCase, GoalCriteria, PrerequisiteEdge,
    PrerequisiteGraph, SuccessCriterion,
};
use academic_ingestion::dating::Date;
use academic_lecture_document::NodeId;
use academic_next_lecture::{ExpectedConceptClaim, ExpectedConceptSource, MaterialReference};
use academic_untrusted_content::{
    PROPOSAL_FORMAT, Proposal, SourceId, SourceIndex, SourceKind, adjudicate, ingest,
    ingest_model_output,
};

pub use gapfix::{
    TestResult, at, band_from, buffer_pool, disk_page, entity, evidence_id, exercise_evidence,
    exposure_evidence, full_dossier, offered, random_io, reading, requires, scope,
    storage_hierarchy, unknown_band,
};

// ---------------------------------------------------------------------------
// Source-reading helpers
// ---------------------------------------------------------------------------

pub fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(|crates| crates.parent())
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

pub fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
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

pub fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
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
pub fn strip_non_code(source: &str) -> String {
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

/// One brace-balanced block's text, from `header` to its matching `}`.
pub fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
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
pub fn collapse(body: &str) -> String {
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
pub fn uses_of(code: &str, name: &str) -> usize {
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

/// The relative path of `path` under the workspace, with forward slashes.
pub fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// This crate's product files, as code with comments and literals removed.
pub fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

/// Joins a path or a macro name that whitespace was inserted into.
pub fn tighten(code: &str) -> String {
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
pub fn absolute_paths(code: &str) -> BTreeSet<String> {
    let roots = ["std", "core", "alloc", "crate", "thiserror", "sha2"];
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
pub fn macros_spelled(code: &str) -> BTreeSet<String> {
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

/// Every `pub fn` in `code`, as its name, its signature up to the body, and the
/// byte offset it starts at.
pub fn public_signature_sites(code: &str) -> Vec<(String, String, usize)> {
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
            found.push((name, collapse(&code[at..end]), at));
            cursor = after;
        }
    }
    found.sort_by_key(|(_, _, at)| *at);
    found
}

/// Every `pub fn` in `code`, as its name and its signature up to the body.
pub fn public_signatures(code: &str) -> Vec<(String, String)> {
    public_signature_sites(code)
        .into_iter()
        .map(|(name, signature, _)| (name, signature))
        .collect()
}

/// Every `pub fn` in `code`, with the type whose `impl` block it sits in.
///
/// The owner matters because an accessor is a way *out* of a value that
/// already holds something, and a constructor is a way to make one. A
/// classification that could not tell them apart would have to admit both or
/// refuse both.
pub fn public_signatures_with_owner(code: &str) -> Vec<(String, String, String)> {
    public_signature_sites(code)
        .into_iter()
        .map(|(name, signature, at)| (owner_before(code, at), name, signature))
        .collect()
}

/// The type named by the last `impl` header before `at`.
fn owner_before(code: &str, at: usize) -> String {
    // `impl<'a, T> Name<'a, T>` has no space after `impl`, so the anchor is the
    // keyword alone. An earlier version anchored on `impl ` and silently
    // reported an empty owner for every generic block.
    //
    // **The block has to still be open.** Taking the last `impl` header before
    // the offset reports the type of a block that already closed, so a free
    // function declared after one is attributed to it. `P2-L6` measured that:
    // the reader answered `MinimalityDefect` for `minimality_defects`, a free
    // function of the same module, and a classification that reads owners could
    // not have told a constructor from an accessor for any module shaped that
    // way.
    let mut search = at;
    let start = loop {
        let Some(found) = code[..search].rfind("\nimpl") else {
            return String::new();
        };
        if block_contains(code, found, at) {
            break found;
        }
        search = found;
    };
    let header: String = code[start + 5..]
        .chars()
        .take_while(|character| *character != '{')
        .collect();
    let mut rest = header.trim();
    if rest.starts_with('<') {
        let mut depth = 0_usize;
        let mut end = 0;
        for (offset, character) in rest.char_indices() {
            match character {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        end = offset + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = rest[end..].trim();
    }
    if let Some((_, after)) = rest.split_once(" for ") {
        rest = after.trim();
    }
    rest.split(['<', ' ', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// Whether the block opened by the `impl` header at `start` still holds `at`.
fn block_contains(code: &str, start: usize, at: usize) -> bool {
    let bytes = code.as_bytes();
    let Some(open) = code[start..].find('{').map(|offset| start + offset) else {
        return false;
    };
    if open > at {
        return false;
    }
    let mut depth = 0_usize;
    let mut cursor = open;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return cursor > at;
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    true
}

/// The text between `code`'s first `{` and its matching `}`, exclusive.
pub fn balanced(code: &str) -> Option<&str> {
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
pub fn top_level_items(body: &str) -> Vec<String> {
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
pub fn use_items(code: &str) -> Vec<String> {
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
            // Whitespace is squeezed rather than removed, so `Digest as _`
            // stays two words. Removing it outright rendered that import as
            // `Digestas_`, which is a spelling nothing could be compared with.
            let mut item = rest[at + 4..at + end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            for (spaced, tight) in [
                (" ::", "::"),
                (":: ", "::"),
                (" {", "{"),
                ("{ ", "{"),
                (" }", "}"),
                ("} ", "}"),
                (" ,", ","),
                (", ", ","),
            ] {
                item = item.replace(spaced, tight);
            }
            flatten(&item, String::new(), &mut found);
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

pub fn module_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// One type declaration's text, from its header to the matching `}` at column
/// zero.
pub fn type_declaration(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{header} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
}

/// The `(name, type)` pairs of one `struct` declaration, in declaration order.
pub fn struct_fields(source: &str, header: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} declares no fields"))?;
    let body = balanced(&source[open..]).ok_or_else(|| format!("{header} is unbalanced"))?;
    let mut found = Vec::new();
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (name, ty) = item
            .split_once(':')
            .ok_or_else(|| format!("{header} has a field with no type: {item}"))?;
        found.push((
            name.split_whitespace().collect::<Vec<_>>().join(" "),
            ty.split_whitespace().collect::<Vec<_>>().join(" "),
        ));
    }
    Ok(found)
}

/// The variant names of one `enum` declaration, in declaration order.
pub fn enum_variants(source: &str, header: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} declares no variants"))?;
    let body = balanced(&source[open..]).ok_or_else(|| format!("{header} is unbalanced"))?;
    let mut found = Vec::new();
    for item in top_level_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let name: String = item
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
    }
    Ok(found)
}

/// The `Self::` entries of one type's `pub const ALL` array, in order.
///
/// `None` when the type declares no `ALL`, which is what tells the caller to
/// skip it rather than to fail.
pub fn all_array(source: &str, type_name: &str) -> Option<Vec<String>> {
    // The search is bounded to this type's own `impl` block. An earlier version
    // searched forward from the header to the end of the file, so a type with
    // no `ALL` picked up the next type's -- `ConnectorError` reported
    // `HttpMethod`'s, and the scan reported a disagreement that was its own.
    let header = format!("impl {type_name} {{");
    let start = source.find(&header)?;
    let block = balanced(&source[start + header.len() - 1..])?;
    let at = block.find("pub const ALL:")?;
    // The declaration is `[Self; N] = [ ... ];`; the entries are in the second
    // bracket group, so the first is skipped.
    let after_type = block[at..].find(']')? + at + 1;
    let entries_open = block[after_type..].find('[')? + after_type;
    let entries_close = block[entries_open..].find(']')? + entries_open;
    Some(
        block[entries_open + 1..entries_close]
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.trim_start_matches("Self::").to_owned())
            .collect(),
    )
}

/// Every `pub enum` this code declares, by name, in declaration order.
pub fn public_enums(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(at) = rest.find("pub enum ") {
        let name: String = rest[at + 9..]
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            found.push(name);
        }
        rest = &rest[at + 9..];
    }
    found
}

/// The method names one `trait` block declares, in declaration order.
pub fn trait_methods(source: &str, header: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let block = whole_block(source, header)?;
    let mut found = Vec::new();
    let mut rest = block.as_str();
    while let Some(at) = rest.find("fn ") {
        let after = &rest[at + 3..];
        let name: String = after
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if !name.is_empty() {
            let end = after
                .find(';')
                .or_else(|| after.find('{'))
                .unwrap_or(after.len());
            found.push(format!("{name} {}", collapse(&after[..end])));
        }
        rest = after;
    }
    Ok(found)
}

pub fn read_module(name: &str) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(
        crate_root().join("src").join(name),
    )?))
}

// ---------------------------------------------------------------------------
// The design document
// ---------------------------------------------------------------------------

/// The design document's whole text.
pub fn specification() -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(workspace_root().join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// One numbered subsection's body, from its own heading to the next heading at
/// the same level or above.
///
/// The end anchor is `\n### ` or `\n## `, whichever comes first, so a
/// subsection cannot silently absorb the one after it.
pub fn section(specification: &str, heading: &str) -> Result<String, Box<dyn Error>> {
    bounded(specification, heading, &["\n### ", "\n## "])
}

/// One top-level section's body, subsections included.
///
/// `## 4.` opens with prose and then declares `### 하루`, so a reader that
/// stopped at the first `###` would answer with the two lines before it and
/// report the morning missing. This one stops only at the next `## `.
pub fn top_section(specification: &str, heading: &str) -> Result<String, Box<dyn Error>> {
    bounded(specification, heading, &["\n## "])
}

fn bounded(specification: &str, heading: &str, anchors: &[&str]) -> Result<String, Box<dyn Error>> {
    let start = specification
        .find(heading)
        .ok_or_else(|| format!("{heading} is not in the design document"))?;
    let rest = &specification[start + heading.len()..];
    let end = anchors
        .iter()
        .filter_map(|anchor| rest.find(anchor))
        .min()
        .unwrap_or(rest.len());
    Ok(rest[..end].to_owned())
}

// ---------------------------------------------------------------------------
// `P2-G5` materials
// ---------------------------------------------------------------------------

/// The first 32 hexadecimal characters of the SHA-256 of `bytes`.
///
/// That is `SPAN_DIGEST_HEX_LEN` of `P2-G5`'s own truncation, computed here
/// from `P2-C1`'s digest so this file links no hash crate of its own.
#[must_use]
pub fn span_digest(bytes: &[u8]) -> String {
    let digest = ContentDigest::sha256(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex.truncate(academic_untrusted_content::SPAN_DIGEST_HEX_LEN);
    hex
}

/// One material, ingested through `P2-G5`'s own boundary.
#[derive(Debug)]
pub struct Material {
    pub index: SourceIndex,
    pub id: SourceId,
    pub text: String,
}

/// Ingests one document and puts it in an index of its own.
pub fn ingest_material(
    identifier: &str,
    kind: SourceKind,
    text: &str,
) -> Result<Material, Box<dyn Error>> {
    let id = SourceId::new(identifier)?;
    let mut index = SourceIndex::new();
    index.insert(ingest(id.clone(), kind, 1, text.as_bytes())?)?;
    Ok(Material {
        index,
        id,
        text: text.to_owned(),
    })
}

/// A well-formed proposal citing `[start, end)` of `material`, adjudicated.
///
/// The whole path runs: bytes are ingested, a model output is tagged, and
/// `adjudicate` resolves the span. Nothing here constructs a `Proposal`
/// directly, because there is no way to.
pub fn proposal_over(
    material: &Material,
    start: usize,
    end: usize,
    summary: &str,
) -> Result<Proposal, Box<dyn Error>> {
    let slice = material
        .text
        .get(start..end)
        .ok_or("the fixture span is out of range")?;
    let digest = span_digest(slice.as_bytes());
    let identifier = material.id.as_str();
    let record = format!(
        "{PROPOSAL_FORMAT}\nkind: TOPIC_SUMMARY\nsummary: {summary}\nsupport: {identifier} {start} {end} {digest}\n"
    );
    let output = ingest_model_output(SourceId::new("model-output-1")?, 2, record.as_bytes())?;
    adjudicate(&material.index, &output).map_err(|quarantined| {
        format!("the fixture output was quarantined: {quarantined:?}").into()
    })
}

/// A fixed calendar date. Never a clock reading; `P2-U6`'s `Date` validates it.
pub fn material_date() -> Result<Date, Box<dyn Error>> {
    Ok(Date::new(2026, 3, 9)?)
}

/// The text every fixture material carries, one per place.
///
/// Seven different documents, so one place's citations cannot accidentally
/// resolve into another place's bytes.
#[must_use]
pub fn material_text(place: ExpectedConceptSource) -> String {
    format!(
        "Week 6 covers buffer management. {} names buffer pool as the unit under study.",
        place.as_str()
    )
}

/// A `P2-G5` document kind for one of section 12.7's seven places.
///
/// The two vocabularies are not the same and no product file maps either onto
/// the other, so this fixture picks a plausible document kind per place and the
/// choice reaches no product code. What matters is that the boundary was
/// crossed at all.
#[must_use]
pub fn document_kind(place: ExpectedConceptSource) -> SourceKind {
    match place {
        ExpectedConceptSource::Syllabus => SourceKind::Syllabus,
        ExpectedConceptSource::NextTitleOrSlide
        | ExpectedConceptSource::TextbookChapter
        | ExpectedConceptSource::LmsMaterial
        | ExpectedConceptSource::Assignment
        | ExpectedConceptSource::Notice
        | ExpectedConceptSource::PriorLectureEnding => SourceKind::Readme,
    }
}

/// A `P2-L4` node identifier, for the one place that has one.
pub fn ending_node() -> Result<NodeId, Box<dyn Error>> {
    Ok(NodeId::new("lecture-05-section-09-paragraph-04")?)
}

/// The material reference for one of section 12.7's seven places.
pub fn material_reference(
    place: ExpectedConceptSource,
    id: SourceId,
) -> Result<MaterialReference, Box<dyn Error>> {
    let node = if place.is_recorded_by_this_system() {
        Some(ending_node()?)
    } else {
        None
    };
    Ok(MaterialReference::of(place, id, material_date()?, node)?)
}

/// One claim about `Buffer Pool`, extracted from one of the seven places.
pub fn claim_from(place: ExpectedConceptSource) -> Result<ExpectedConceptClaim, Box<dyn Error>> {
    claim_about(place, buffer_pool())
}

/// One claim about `concept`, extracted from one of the seven places.
pub fn claim_about(
    place: ExpectedConceptSource,
    concept: EntityId,
) -> Result<ExpectedConceptClaim, Box<dyn Error>> {
    let text = material_text(place);
    let material = ingest_material(
        &format!("material-{}", place.as_str().to_lowercase()),
        document_kind(place),
        &text,
    )?;
    let proposal = proposal_over(&material, 0, 31, "week 6 covers buffer management")?;
    let reference = material_reference(place, material.id.clone())?;
    Ok(ExpectedConceptClaim::extract(
        concept,
        EntityKind::Concept,
        reference,
        &proposal,
        ConfidencePermille::new(720)?,
    )?)
}

// ---------------------------------------------------------------------------
// The `P2-N5` descent section 12.7 compares against
// ---------------------------------------------------------------------------

/// Section 12.7's own blocking evidence, as `P2-N5` readings.
///
/// `Buffer Pool` and `Disk Page` each carry one exposure item, which `P2-N2`
/// admits at `Exposed`; `Storage Hierarchy` carries nothing at all. The bands
/// come from `P2-N3`'s `project` over no dated evidence, which that crate reads
/// as `UNKNOWN`, and `unknown_band` verifies that rather than asserting it.
pub fn section_12_7_readings() -> Result<Vec<ConceptReading>, Box<dyn Error>> {
    let mut buffer = reading(buffer_pool(), unknown_band(buffer_pool())?);
    buffer.offered = vec![offered(
        exposure_evidence("lecture-buffer-pool")?,
        "evidence-buffer-pool-exposure",
        full_dossier(buffer_pool()),
    )];
    let mut page = reading(disk_page(), unknown_band(disk_page())?);
    page.offered = vec![offered(
        exposure_evidence("lecture-disk-page")?,
        "evidence-disk-page-exposure",
        full_dossier(disk_page()),
    )];
    let mut hierarchy = reading(storage_hierarchy(), unknown_band(storage_hierarchy())?);
    hierarchy.offered = vec![offered(
        exposure_evidence("lecture-storage-hierarchy")?,
        "evidence-storage-hierarchy-exposure",
        full_dossier(storage_hierarchy()),
    )];
    Ok(vec![buffer, page, hierarchy])
}

/// The overlay `P2-N5` reads at one concept, rebuilt from the same reading.
pub fn overlay_of(reading: &ConceptReading) -> Result<ConceptState, Box<dyn Error>> {
    Ok(ConceptState::overlay(
        reading.concept,
        reading.kind,
        reading.identity.clone(),
        &reading.offered,
        &reading.freshness,
        &reading.spillover,
    )?)
}

/// Every overlay for a set of readings.
pub fn overlays(readings: &[ConceptReading]) -> Result<Vec<ConceptState>, Box<dyn Error>> {
    readings.iter().map(overlay_of).collect()
}

/// Section 36.4's descent from `Buffer Pool`, run for real.
pub fn case_over(
    goal: &ActiveGoal,
    graph: &PrerequisiteGraph,
    readings: &[ConceptReading],
) -> Result<GapCase, Box<dyn Error>> {
    academic_gap::search(goal, graph, readings, None)?
        .ok_or_else(|| "the fixture descent found no gap".into())
}

/// A goal whose surface concept is `concept`.
pub fn goal_at(concept: EntityId, tag: &str) -> Result<ActiveGoal, Box<dyn Error>> {
    let criteria = GoalCriteria::of(vec![SuccessCriterion::concept(
        concept,
        EntityKind::Concept,
        MasteryLevel::Practiced,
    )?])
    .ok_or("the fixture criteria are empty")?;
    Ok(ActiveGoal::declare(
        entity(tag),
        scope(),
        concept,
        EntityKind::Concept,
        criteria,
    )?)
}

/// A hard `REQUIRES` edge, the strength section 36.4's own chain opens with.
pub fn hard_edge(
    advanced: EntityId,
    prerequisite: EntityId,
    tag: &str,
) -> Result<PrerequisiteEdge, Box<dyn Error>> {
    requires(advanced, prerequisite, PrerequisiteStrength::Hard, tag)
}

/// A reading `P2-N5` routes to `MASTERY_GAP` without building a lecture.
///
/// One succeeded attempt and one failed one. Section 15.2's fourth overlay
/// dimension is `a recorded failure is a deficit at any rung`, so this is a
/// blocking foundation whatever the band and whatever the floor -- which is
/// what the count fixtures need, and they need it without a `tempfile` and a
/// transcription run each.
pub fn blocked_reading(concept: EntityId, tag: &str) -> Result<ConceptReading, Box<dyn Error>> {
    let mut value = reading(concept, unknown_band(concept)?);
    value.offered = vec![
        offered(
            exercise_evidence(&format!("{tag}-succeeded")),
            &format!("{tag}-succeeded"),
            full_dossier(concept),
        ),
        offered(
            gapfix::failed_exercise_evidence(&format!("{tag}-failed")),
            &format!("{tag}-failed"),
            full_dossier(concept),
        ),
    ];
    Ok(value)
}

/// The confidence axis two carries. See `crates/next-lecture/src/uncertainty.rs`
/// for why this one is supplied and the other two are not.
pub fn edge_confidence() -> Result<ConfidencePermille, Box<dyn Error>> {
    Ok(ConfidencePermille::new(880)?)
}
