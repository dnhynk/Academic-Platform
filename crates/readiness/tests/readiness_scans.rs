//! What `academic-readiness` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, measured in this run
//!
//! `docs/contracts/policy-source-scans.md` records what a forbidden-token list
//! measured one crate over: seven spellings of a filesystem or environment
//! reach compile, spell none of the listed tokens, add no `use` item, and
//! passed. The repair was not a longer list. It was three **whole-set**
//! comparisons, in both directions:
//!
//! * every `use` item ([`USE_ITEMS`], [`CRATE_IMPORTS`], [`RE_EXPORTS`]);
//! * every two-segment path reached through a crate root ([`REACHED_PATHS`]);
//! * every macro invoked ([`MACROS_SPELLED`]).
//!
//! [`FORBIDDEN_CONSTRUCTS`] is kept as the third and weakest layer, because it
//! names the shapes a reader expects to see refused.
//!
//! ## What this crate is allowed to hold
//!
//! Six things, and the last column of [`FIELDS`] is which one:
//!
//! * a **caller-supplied identifier** — the text inside an evidence locator or
//!   a starting point, admitted by `identity::validated`;
//! * an **identifier a refusal echoes** — the copy a `ReadinessError` carries
//!   so its message can name what it refused;
//! * a **closed vocabulary value** — one of this crate's enumerations;
//! * a **value of a reviewed crate** — `P2-Y1`'s competency, criterion and
//!   stage values, `P2-Y2`'s bundle reference and importance, `P2-N3`'s band;
//! * a **value of this crate**; and
//! * **the user's own words** — a rubric line and a weight's reason. Section
//!   24.3 makes both a condition of a score existing, so they are inventoried
//!   rather than refused.
//!
//! There is no seventh, and in particular there is no byte buffer, no timestamp
//! and **no float**: no field of this crate is declared `u8`, `u64`, `f32` or
//! `f64` under any name. `no_primary_aggregate_percentage` in
//! `readiness_matrix.rs` is the other half of that — a ratio has no type to
//! arrive in, whatever the field is called.
//!
//! ## Four rules of this task that are pinned rather than described
//!
//! `the_only_producer_of_a_score_is_disclose` compares the whole set of public
//! functions that return an `AuxiliaryScore` **by value** against `disclose`,
//! and pins that its parameter list names all four disclosures and no number.
//!
//! `the_matrix_is_the_only_view_and_it_is_first` requires `render` to be the
//! one function producing the block sequence and pins its body whole, so a
//! block placed before the matrix is an edit to a constant here.
//!
//! `no_function_maps_a_stage_or_a_kind_to_an_axis` compares the whole set of
//! public signatures naming both a `P2-Y1` stage (or a `P2-N2` evidence kind)
//! and a `ReadinessAxis` against the empty set. `P2-Y1` recorded that a total
//! map between section 13.2's rows and section 24.3's stages would have to
//! invent three of its six answers; one layer up the same map would have to
//! invent the `설계 선택` column's.
//!
//! `a_cell_and_a_band_are_never_one_field` requires no declared type of this
//! crate to hold both an `AxisCell` and a `FreshnessCell`, which is section
//! 34.5's `missing/unknown과 freshness를 별도 표시` as a property of the
//! declarations rather than of a rendering.

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

/// One brace-balanced block's text, from `header` to its matching `}`.
///
/// The slice, with its line breaks intact, so a reader that needs them — like
/// [`public_signatures`] — can be run over one block rather than a whole file.
fn raw_block<'a>(source: &'a str, header: &str) -> Result<&'a str, Box<dyn Error>> {
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
    Ok(&source[start..=at.min(bytes.len() - 1)])
}

/// The same block, collapsed to one line so a rewrapped pin still matches.
fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    Ok(collapse(raw_block(source, header)?))
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

fn source_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?)
}

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

/// Collapses whitespace so a rewrapped signature still matches its pin.
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
    let roots = ["std", "core", "alloc", "thiserror"];
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

/// Every named field of every `struct` and `enum` `code` declares.
///
/// A struct-variant of an enumeration is reported as `Enum::Variant`, so a
/// field added inside one is a key here rather than a shape this extractor has
/// no name for. A tuple struct has no named field at all, and
/// [`the_unnamed_fields_are_pinned`] closes those separately.
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
/// Every `use` item of a product file that is not a `crate::` item.
///
/// The three crate roots among them are the whole of this crate's reach outside
/// itself: `P2-Y1`'s competency values, `P2-Y2`'s bundle reference and
/// importance, and `P2-C1`/`P2-N3`'s freshness band. There is no `std::fs`, no
/// `std::net`, no `std::env`, no `std::process` and no `std::time`; the only
/// `std` items are two collections.
const USE_ITEMS: [(&str, &str); 20] = [
    (
        "crates/readiness/src/axis.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/readiness/src/cell.rs",
        "use academic_competency::{Competency, CriterionId, EvidenceStage, StageEvidence};",
    ),
    (
        "crates/readiness/src/cell.rs",
        "use academic_domain::FreshnessBand;",
    ),
    ("crates/readiness/src/cell.rs", "use serde::Serialize;"),
    ("crates/readiness/src/history.rs", "use serde::Serialize;"),
    ("crates/readiness/src/identity.rs", "use core::fmt;"),
    (
        "crates/readiness/src/identity.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/readiness/src/lib.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/readiness/src/matrix.rs",
        "use academic_competency::{Competency, CompetencyId};",
    ),
    (
        "crates/readiness/src/matrix.rs",
        "use academic_domain::FreshnessBand;",
    ),
    (
        "crates/readiness/src/matrix.rs",
        "use academic_role_profile::{BundleImportance, RoleProfile, RoleProfileRef};",
    ),
    ("crates/readiness/src/matrix.rs", "use serde::Serialize;"),
    (
        "crates/readiness/src/navigate.rs",
        "use academic_competency::{CompetencyId, ConceptRef, CriterionId, EvidenceSource};",
    ),
    (
        "crates/readiness/src/navigate.rs",
        "use academic_role_profile::RoleProfileRef;",
    ),
    ("crates/readiness/src/navigate.rs", "use serde::Serialize;"),
    ("crates/readiness/src/notice.rs", "use core::fmt;"),
    ("crates/readiness/src/notice.rs", "use serde::Serialize;"),
    (
        "crates/readiness/src/score.rs",
        "use std::collections::BTreeSet;",
    ),
    (
        "crates/readiness/src/score.rs",
        "use academic_competency::{Competency, CompetencyId};",
    ),
    ("crates/readiness/src/score.rs", "use serde::Serialize;"),
];

/// Every `use crate::` item of a product file.
const CRATE_IMPORTS: [(&str, &str); 7] = [
    (
        "crates/readiness/src/cell.rs",
        "use crate::{ReadinessError, axis::ReadinessAxis, identity::EvidenceLocatorId};",
    ),
    (
        "crates/readiness/src/history.rs",
        "use crate::score::{ScoreValue, WeightDisclosure};",
    ),
    (
        "crates/readiness/src/identity.rs",
        "use crate::ReadinessError;",
    ),
    (
        "crates/readiness/src/matrix.rs",
        "use crate::{ axis::ReadinessAxis, cell::{AxisCell, AxisEvidence, FreshnessCell}, };",
    ),
    (
        "crates/readiness/src/navigate.rs",
        "use crate::{ axis::ReadinessAxis, cell::{AxisCell, AxisEvidence, UnknownBasis}, \
         identity::{EvidenceLocatorId, StartingPointId}, view::ReadinessView, };",
    ),
    (
        "crates/readiness/src/score.rs",
        "use crate::{ ReadinessError, axis::ReadinessAxis, cell::AxisCell, \
         identity::{EvidenceLocatorId, non_empty}, matrix::ReadinessMatrix, };",
    ),
    (
        "crates/readiness/src/view.rs",
        "use crate::{ ReadinessError, history::ReadinessEvent, matrix::ReadinessMatrix, \
         notice::NonGuaranteeNotice, score::{ AuxiliaryScore, MissingDataDisclosure, \
         RubricDisclosure, SourceDisclosure, WeightDisclosure, disclose, }, };",
    ),
];

/// Every `pub use` of `lib.rs`, and the two `std` items `view.rs` and
/// `score.rs` reach for a map and a set.
const RE_EXPORTS: [&str; 9] = [
    "pub use axis::ReadinessAxis;",
    "pub use cell::{ AxisCell, AxisEvidence, FreshnessCell, MISSING_CELL_MARK, RefusedPlacement, \
     UnknownBasis, };",
    "pub use history::ReadinessEvent;",
    "pub use identity::{EvidenceLocatorId, MAX_IDENTIFIER, StartingPointId};",
    "pub use matrix::{ColumnReading, CompetencyInput, ReadinessMatrix, ReadinessRow, take};",
    "pub use navigate::{ AbsenceState, NavigationDirection, StartingPoint, Termination, Terminus, \
     traverse, };",
    "pub use notice::{ ALLOWED_INSTEAD, NonGuaranteeNotice, REFUSAL_REASON, REFUSED_PRODUCT, \
     SPECIFICATION_PHRASE, };",
    "pub use score::{ AuxiliaryScore, AxisWeight, MissingDataDisclosure, MissingDatum, \
     RubricDisclosure, RubricLines, ScoreValue, SourceDisclosure, WeightDisclosure, disclose, };",
    "pub use view::{NOTICE_KEY, ReadinessView, ViewBlock, published_notice};",
];

/// The three remaining `use` items, all of them `view.rs`'s.
const STD_IMPORTS: [(&str, &str); 3] = [
    (
        "crates/readiness/src/view.rs",
        "use std::collections::BTreeMap;",
    ),
    (
        "crates/readiness/src/view.rs",
        "use academic_competency::{Competency, CompetencyId, CriterionId, PerformanceCriterion};",
    ),
    ("crates/readiness/src/view.rs", "use serde::Serialize;"),
];

/// Every two-segment path this crate reaches through a crate root.
const REACHED_PATHS: [(&str, &str); 2] = [
    (
        "academic_domain::predicates",
        "section 7.1's node hierarchy, read for `RowSubject::node_type` rather than declared again",
    ),
    (
        "thiserror::Error",
        "the derive on ReadinessError, which is the only attribute path in this crate",
    ),
];

/// Every macro this crate invokes.
///
/// Three. None of them reads anything: an `include_str!` or an `env!` would be
/// a fourth key here, and a `println!` would be a fifth.
const MACROS_SPELLED: [(&str, &str); 3] = [
    (
        "format",
        "the rendered notice, a rubric line, and the error strings a refusal echoes",
    ),
    (
        "matches",
        "the byte class in `validated`, and the evidenced arm in `disclose`",
    ),
    (
        "vec",
        "the block sequence `render` returns, and the first terminus of a `Termination`",
    ),
];

/// The files of this package permitted to read a file at all.
const READERS: [&str; 4] = [
    "readiness_scans.rs",
    "readiness_matrix.rs",
    "readiness_export.rs",
    "mod.rs",
];

/// The third and weakest layer.
const FORBIDDEN_CONSTRUCTS: [&str; 17] = [
    "File",
    "OpenOptions",
    "read_to_string",
    "fs::",
    "Path",
    "PathBuf",
    "env",
    "var",
    "Command",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "net",
    "reqwest",
    "Instant",
    "SystemTime",
    "now",
];

/// The seven things a field of this crate may hold.
const REASONS: [&str; 7] = [
    "a caller-supplied identifier",
    "an identifier a refusal echoes",
    "a closed vocabulary value",
    "a value of a reviewed crate",
    "a value of this crate",
    "the user's own words",
    "a disclosed weight in whole units",
];

/// Every named field of every type this crate declares.
const FIELDS: [(&str, &str, &str, &str); 69] = [
    (
        "AbsenceState::CellIsMissing",
        "axis",
        "ReadinessAxis",
        "a closed vocabulary value",
    ),
    (
        "AbsenceState::CellIsMissing",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "AbsenceState::CellIsMissing",
        "criterion",
        "CriterionId",
        "a value of a reviewed crate",
    ),
    (
        "AbsenceState::CellIsUnknown",
        "axis",
        "ReadinessAxis",
        "a closed vocabulary value",
    ),
    (
        "AbsenceState::CellIsUnknown",
        "basis",
        "UnknownBasis",
        "a closed vocabulary value",
    ),
    (
        "AbsenceState::CellIsUnknown",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "AbsenceState::CellIsUnknown",
        "criterion",
        "CriterionId",
        "a value of a reviewed crate",
    ),
    (
        "AbsenceState::NoRowReachesTheStartingPoint",
        "direction",
        "NavigationDirection",
        "a closed vocabulary value",
    ),
    (
        "AbsenceState::NoRowReachesTheStartingPoint",
        "start",
        "StartingPoint",
        "a value of this crate",
    ),
    (
        "AuxiliaryScore",
        "missing_data",
        "MissingDataDisclosure",
        "a value of this crate",
    ),
    (
        "AuxiliaryScore",
        "rubric",
        "RubricDisclosure",
        "a value of this crate",
    ),
    (
        "AuxiliaryScore",
        "sources",
        "SourceDisclosure",
        "a value of this crate",
    ),
    (
        "AuxiliaryScore",
        "value",
        "ScoreValue",
        "a value of this crate",
    ),
    (
        "AuxiliaryScore",
        "weights",
        "WeightDisclosure",
        "a value of this crate",
    ),
    (
        "AxisEvidence",
        "axis",
        "ReadinessAxis",
        "a closed vocabulary value",
    ),
    (
        "AxisEvidence",
        "criterion",
        "CriterionId",
        "a value of a reviewed crate",
    ),
    (
        "AxisEvidence",
        "locator",
        "EvidenceLocatorId",
        "a caller-supplied identifier",
    ),
    (
        "AxisEvidence",
        "record",
        "StageEvidence",
        "a value of a reviewed crate",
    ),
    (
        "AxisEvidence",
        "stage",
        "EvidenceStage",
        "a value of a reviewed crate",
    ),
    (
        "AxisWeight",
        "axis",
        "ReadinessAxis",
        "a closed vocabulary value",
    ),
    ("AxisWeight", "reason", "String", "the user's own words"),
    (
        "AxisWeight",
        "weight",
        "u32",
        "a disclosed weight in whole units",
    ),
    (
        "CompetencyInput",
        "competency",
        "&'aCompetency",
        "a value of a reviewed crate",
    ),
    (
        "CompetencyInput",
        "freshness",
        "FreshnessBand",
        "a value of a reviewed crate",
    ),
    (
        "CompetencyInput",
        "placements",
        "&'a[AxisEvidence]",
        "a value of this crate",
    ),
    (
        "FreshnessCell",
        "band",
        "FreshnessBand",
        "a value of a reviewed crate",
    ),
    (
        "MissingDataDisclosure",
        "entries",
        "Vec<MissingDatum>",
        "a value of this crate",
    ),
    (
        "MissingDatum",
        "axis",
        "ReadinessAxis",
        "a closed vocabulary value",
    ),
    (
        "MissingDatum",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "MissingDatum",
        "reading",
        "&'staticstr",
        "a closed vocabulary value",
    ),
    (
        "ReadinessEvent::ScoreHidden",
        "value",
        "ScoreValue",
        "a value of this crate",
    ),
    (
        "ReadinessEvent::ScoreHidden",
        "weights",
        "WeightDisclosure",
        "a value of this crate",
    ),
    (
        "ReadinessEvent::ScorePublished",
        "value",
        "ScoreValue",
        "a value of this crate",
    ),
    (
        "ReadinessEvent::ScorePublished",
        "weights",
        "WeightDisclosure",
        "a value of this crate",
    ),
    (
        "ReadinessEvent::WeightsReset",
        "from",
        "WeightDisclosure",
        "a value of this crate",
    ),
    (
        "ReadinessEvent::WeightsReset",
        "previous_value",
        "ScoreValue",
        "a value of this crate",
    ),
    (
        "ReadinessEvent::WeightsReset",
        "to",
        "WeightDisclosure",
        "a value of this crate",
    ),
    (
        "ReadinessMatrix",
        "bundle",
        "RoleProfileRef",
        "a value of a reviewed crate",
    ),
    (
        "ReadinessMatrix",
        "rows",
        "Vec<ReadinessRow>",
        "a value of this crate",
    ),
    (
        "ReadinessRow",
        "academic_learning",
        "AxisCell",
        "a value of this crate",
    ),
    (
        "ReadinessRow",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "ReadinessRow",
        "design_choice",
        "AxisCell",
        "a value of this crate",
    ),
    (
        "ReadinessRow",
        "freshness",
        "FreshnessCell",
        "a value of this crate",
    ),
    (
        "ReadinessRow",
        "importance",
        "BundleImportance",
        "a value of a reviewed crate",
    ),
    (
        "ReadinessRow",
        "incident_debugging",
        "AxisCell",
        "a value of this crate",
    ),
    (
        "ReadinessRow",
        "problem_and_assignment",
        "AxisCell",
        "a value of this crate",
    ),
    (
        "ReadinessRow",
        "project_application",
        "AxisCell",
        "a value of this crate",
    ),
    (
        "ReadinessView",
        "criteria",
        "BTreeMap<CompetencyId,Vec<CriterionId>>",
        "a value of a reviewed crate",
    ),
    (
        "ReadinessView",
        "history",
        "Vec<ReadinessEvent>",
        "a value of this crate",
    ),
    (
        "ReadinessView",
        "matrix",
        "ReadinessMatrix",
        "a value of this crate",
    ),
    (
        "ReadinessView",
        "non_guarantee_notice",
        "NonGuaranteeNotice",
        "a value of this crate",
    ),
    (
        "ReadinessView",
        "score",
        "Option<AuxiliaryScore>",
        "a value of this crate",
    ),
    (
        "RefusedPlacement",
        "basis",
        "UnknownBasis",
        "a closed vocabulary value",
    ),
    (
        "RefusedPlacement",
        "evidence",
        "AxisEvidence",
        "a value of this crate",
    ),
    (
        "RubricDisclosure",
        "entries",
        "Vec<RubricLines>",
        "a value of this crate",
    ),
    (
        "RubricLines",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "RubricLines",
        "lines",
        "Vec<String>",
        "the user's own words",
    ),
    (
        "ScoreValue",
        "evidenced_units",
        "u32",
        "a disclosed weight in whole units",
    ),
    (
        "ScoreValue",
        "weighted_units",
        "u32",
        "a disclosed weight in whole units",
    ),
    (
        "SourceDisclosure",
        "locators",
        "Vec<EvidenceLocatorId>",
        "a caller-supplied identifier",
    ),
    (
        "Termination",
        "direction",
        "NavigationDirection",
        "a closed vocabulary value",
    ),
    ("Termination", "first", "Terminus", "a value of this crate"),
    (
        "Termination",
        "rest",
        "Vec<Terminus>",
        "a value of this crate",
    ),
    (
        "Termination",
        "start",
        "StartingPoint",
        "a value of this crate",
    ),
    (
        "Terminus::CriterionAndEvidence",
        "axis",
        "ReadinessAxis",
        "a closed vocabulary value",
    ),
    (
        "Terminus::CriterionAndEvidence",
        "competency",
        "CompetencyId",
        "a value of a reviewed crate",
    ),
    (
        "Terminus::CriterionAndEvidence",
        "criterion",
        "CriterionId",
        "a value of a reviewed crate",
    ),
    (
        "Terminus::CriterionAndEvidence",
        "locator",
        "EvidenceLocatorId",
        "a caller-supplied identifier",
    ),
    (
        "WeightDisclosure",
        "weights",
        "Vec<AxisWeight>",
        "a value of this crate",
    ),
];

/// The tuple structs, whose fields have no names to inventory.
const TUPLE_STRUCTS: [(&str, &str, &str); 2] = [
    (
        "EvidenceLocatorId",
        "pub struct EvidenceLocatorId(String);",
        "a caller-supplied identifier, admitted by identity::validated",
    ),
    (
        "StartingPointId",
        "pub struct StartingPointId(String);",
        "a caller-supplied identifier, admitted by identity::validated",
    ),
];

/// The enumerations with unnamed variants, pinned whole.
const TUPLE_ENUMS: [(&str, &str); 5] = [
    ("src/cell.rs", "pub enum AxisCell {"),
    ("src/matrix.rs", "pub enum ColumnReading<'row> {"),
    ("src/navigate.rs", "pub enum StartingPoint {"),
    ("src/navigate.rs", "pub enum Terminus {"),
    ("src/view.rs", "pub enum ViewBlock<'view> {"),
];

/// Every `impl` header this crate writes.
///
/// **Measured because a `pub fn` sweep did not see one.** An injection that
/// added `impl From<FreshnessCell> for AxisCell` passed the whole suite: a
/// trait implementation's `fn from` is not a `pub fn`, so neither
/// `a_cell_and_a_band_are_never_one_field` nor
/// `missing_and_unknown_are_separate_from_freshness` could see it. A
/// conversion between two of this crate's types is exactly what those tests
/// exist to refuse, so the whole set of `impl` headers is pinned here in both
/// directions: any trait implementation added anywhere in the crate is a key
/// that is not in this list.
const IMPL_HEADERS: [&str; 36] = [
    "impl AuxiliaryScore {",
    "impl AxisCell {",
    "impl AxisEvidence {",
    "impl AxisWeight {",
    "impl EvidenceLocatorId {",
    "impl FreshnessCell {",
    "impl From<EvidenceLocatorId> for String {",
    "impl From<NonGuaranteeNotice> for String {",
    "impl From<StartingPointId> for String {",
    "impl MissingDataDisclosure {",
    "impl MissingDatum {",
    "impl NavigationDirection {",
    "impl NonGuaranteeNotice {",
    "impl ReadinessAxis {",
    "impl ReadinessEvent {",
    "impl ReadinessMatrix {",
    "impl ReadinessRow {",
    "impl ReadinessView {",
    "impl RefusedPlacement {",
    "impl RowSubject {",
    "impl RubricDisclosure {",
    "impl RubricLines {",
    "impl ScoreValue {",
    "impl SourceDisclosure {",
    "impl StartingPoint {",
    "impl StartingPointId {",
    "impl Termination {",
    "impl Terminus {",
    "impl TryFrom<String> for EvidenceLocatorId {",
    "impl TryFrom<String> for StartingPointId {",
    "impl UnknownBasis {",
    "impl ViewBlock<'_> {",
    "impl WeightDisclosure {",
    "impl fmt::Display for EvidenceLocatorId {",
    "impl fmt::Display for NonGuaranteeNotice {",
    "impl fmt::Display for StartingPointId {",
];

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

/// The walk reads every module this crate declares.
#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let lib = source_of(&crate_root().join("src/lib.rs"))?;
    let declared: BTreeSet<String> = strip_non_code(&lib)
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod ").map(str::to_owned))
        .map(|name| name.trim_end_matches(';').to_owned())
        .collect();
    let read: BTreeSet<String> = crate_product_sources()?
        .iter()
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .filter(|stem| stem != "lib")
        .collect();
    assert_eq!(
        read, declared,
        "the walk and lib.rs disagree about this crate's modules"
    );
    assert!(
        declared.len() >= 9,
        "this crate declares {} modules; the walk would be reading almost nothing",
        declared.len()
    );
    assert!(
        crate_product_sources()?.len() >= 10,
        "the product walk found too few files to be reading this package"
    );
    Ok(())
}

/// The three whole-set comparisons, and then the token pass.
#[test]
fn the_readiness_crate_touches_no_file_and_no_socket() -> TestResult {
    let mut found: Vec<(String, String)> = Vec::new();
    for (file, _) in product_code()? {
        let source = fs::read_to_string(workspace_root().join(&file))?;
        let mut inside = false;
        let mut buffer = String::new();
        for line in source.lines() {
            let trimmed = line.trim();
            let opens = trimmed.starts_with("use ")
                || (trimmed.starts_with("pub") && trimmed.contains(" use "));
            if inside || opens {
                if inside {
                    buffer.push(' ');
                } else {
                    buffer.clear();
                }
                buffer.push_str(trimmed);
                inside = !trimmed.ends_with(';');
                if !inside {
                    found.push((file.clone(), collapse(&buffer)));
                }
            }
        }
    }
    let lib = "crates/readiness/src/lib.rs";
    let expected: BTreeSet<(String, String)> = USE_ITEMS
        .iter()
        .chain(CRATE_IMPORTS.iter())
        .chain(STD_IMPORTS.iter())
        .map(|(file, item)| ((*file).to_owned(), collapse(item)))
        .chain(
            RE_EXPORTS
                .iter()
                .map(|item| (lib.to_owned(), collapse(item))),
        )
        .collect();
    let found_set: BTreeSet<(String, String)> = found.into_iter().collect();
    assert_eq!(
        found_set, expected,
        "this crate's `use` set changed; a filesystem, transport or feed import is an extra key \
         here"
    );

    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        let body = without_use_items(&code);
        reached.extend(absolute_paths(&body));
        macros.extend(macros_spelled(&body));
    }
    assert_eq!(
        reached,
        REACHED_PATHS
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate reaches a path outside its inventory; every entry needs a reason"
    );
    assert_eq!(
        macros,
        MACROS_SPELLED
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate invokes a macro outside its inventory; an include_ macro reads a file"
    );

    for path in crate_all_sources()? {
        let code = strip_non_code(&source_of(&path)?);
        for forbidden in FORBIDDEN_CONSTRUCTS {
            let named = if forbidden.ends_with("::") {
                code.contains(forbidden)
            } else {
                uses_of(&code, forbidden) > 0
            };
            let permitted = READERS
                .iter()
                .any(|reader| relative(&path).ends_with(reader));
            assert!(
                permitted || !named,
                "{} spells {forbidden}",
                relative(&path)
            );
        }
    }
    Ok(())
}

/// Every declared field, in both directions, each saying what it holds.
#[test]
fn every_field_of_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (_, code) in product_code()? {
        for (owner, field, declared) in type_fields(&code) {
            found.insert((owner, field, declared));
        }
    }
    let expected: BTreeSet<(String, String, String)> = FIELDS
        .iter()
        .map(|(owner, field, declared, _)| {
            (
                (*owner).to_owned(),
                (*field).to_owned(),
                (*declared).to_owned(),
            )
        })
        .collect();
    assert_eq!(
        found, expected,
        "a field of this crate is not in the inventory; every field needs a line saying what it \
         holds, and a field holding a clock reading, a byte buffer or a ratio does not get one"
    );

    let admitted: BTreeSet<&str> = REASONS.into_iter().collect();
    for (owner, field, declared, reason) in FIELDS {
        assert!(
            admitted.contains(reason),
            "{owner}.{field} is described as {reason:?}, which is not one of the seven"
        );
        assert!(
            !declared.contains("u8"),
            "{owner}.{field} is declared {declared}, which is a byte buffer"
        );
        assert!(
            !declared.contains("u64") && !declared.contains("Instant"),
            "{owner}.{field} is declared {declared}, which could be a clock reading"
        );
        assert!(
            !declared.contains("f32") && !declared.contains("f64"),
            "{owner}.{field} is declared {declared}, which is a ratio and section 34.5's \
             허위 정밀도"
        );
    }
    Ok(())
}

/// The unnamed fields, which [`type_fields`] has no name for, pinned whole.
#[test]
fn the_unnamed_fields_are_pinned() -> TestResult {
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        let mut cursor = 0;
        while let Some(at) = code[cursor..].find("struct ").map(|at| at + cursor) {
            cursor = at + "struct ".len();
            let name: String = code[cursor..]
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            if code[cursor + name.len()..].starts_with('(') {
                declared.insert(name);
            }
        }
    }
    assert_eq!(
        declared,
        TUPLE_STRUCTS
            .iter()
            .map(|(name, _, _)| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "this crate declares a tuple struct the inventory does not name"
    );

    let mut all = String::new();
    for (_, code) in product_code()? {
        all.push_str(&collapse(&code));
        all.push(' ');
    }
    for (name, pin, reason) in TUPLE_STRUCTS {
        assert!(
            all.contains(pin),
            "{name} is not declared as {pin:?}; it holds {reason}"
        );
    }

    for (file, header) in TUPLE_ENUMS {
        let source = strip_non_code(&source_of(&crate_root().join(file))?);
        let block = whole_block(&source, header)?;
        assert!(
            block.len() > 40,
            "the pin on {header} extracted {} characters, which is not a declaration",
            block.len()
        );
        for held in ["u8", "u64", "f32", "f64"] {
            assert!(
                !block.contains(held),
                "{header} carries a {held} in an unnamed field"
            );
        }
    }
    Ok(())
}

/// No public function edits a value in place.
#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let mut mutating: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            if signature.contains("&mut self") || signature.contains("&mut Self") {
                mutating.push(format!("{file}::{name}"));
            }
        }
    }
    assert!(
        mutating.is_empty(),
        "these public functions edit in place: {mutating:?}"
    );
    Ok(())
}

/// The one producer of a score takes all four disclosures and no number.
#[test]
fn the_only_producer_of_a_score_is_disclose() -> TestResult {
    let mut producers: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            let Some(arrow) = signature.find("->") else {
                continue;
            };
            let returned = &signature[arrow + 2..];
            if uses_of(returned, "AuxiliaryScore") > 0 && !returned.contains("&AuxiliaryScore") {
                producers.insert(name);
            }
        }
    }
    assert_eq!(
        producers,
        BTreeSet::from(["disclose".to_owned()]),
        "a public function producing a score arrived or left"
    );

    let source = strip_non_code(&source_of(&crate_root().join("src/score.rs"))?);
    let signature = tighten(
        raw_block(&source, "pub fn disclose(")?
            .split(" {")
            .next()
            .ok_or("disclose has no body")?,
    );
    for required in [
        "matrix: &ReadinessMatrix",
        "competencies: &[&Competency]",
        "rubric: RubricDisclosure",
        "sources: SourceDisclosure",
        "missing_data: MissingDataDisclosure",
        "weights: WeightDisclosure",
    ] {
        assert!(
            signature.contains(required),
            "disclose no longer takes {required}: {signature}"
        );
    }
    assert_eq!(
        uses_of(&signature, "ScoreValue"),
        0,
        "disclose takes a number, so a score would no longer be computed: {signature}"
    );
    Ok(())
}

/// The block sequence is produced in one place and the matrix is first.
#[test]
fn the_matrix_is_the_only_view_and_it_is_first() -> TestResult {
    let mut producers: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            let Some(arrow) = signature.find("->") else {
                continue;
            };
            if uses_of(&signature[arrow + 2..], "ViewBlock") > 0 {
                producers.insert(name);
            }
        }
    }
    assert_eq!(
        producers,
        BTreeSet::from(["render".to_owned()]),
        "a second public function produces the block sequence"
    );

    let source = strip_non_code(&source_of(&crate_root().join("src/view.rs"))?);
    let body = tighten(raw_block(&source, "pub fn render(&self)")?);
    let first = body
        .find("ViewBlock::")
        .ok_or("render names no block at all")?;
    assert!(
        body[first..].starts_with("ViewBlock::Matrix"),
        "the first block render names is not the matrix: {}",
        &body[first..first.saturating_add(60).min(body.len())]
    );
    Ok(())
}

/// No function maps a `P2-Y1` stage or a `P2-N2` evidence kind to a column.
#[test]
fn no_function_maps_a_stage_or_a_kind_to_an_axis() -> TestResult {
    let mut mapping: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            let names_axis = uses_of(&signature, "ReadinessAxis") > 0;
            let names_depth =
                uses_of(&signature, "EvidenceStage") > 0 || uses_of(&signature, "EvidenceKind") > 0;
            if names_axis && names_depth && signature.contains("->") {
                mapping.push(format!("{file}::{name}"));
            }
        }
    }
    assert!(
        mapping.is_empty(),
        "these functions map a depth to a column, which would invent the 설계 선택 answer: \
         {mapping:?}"
    );

    // The scanner is not vacuous. Both halves of the conjunction it looks for
    // are present in this crate separately -- functions that name a column, and
    // functions that name a depth -- so the empty result above is the absence of
    // the *map* rather than the absence of anything to scan.
    let mut naming_axis: Vec<String> = Vec::new();
    let mut naming_depth: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            if uses_of(&signature, "ReadinessAxis") > 0 {
                naming_axis.push(format!("{file}::{name}"));
            }
            if uses_of(&signature, "EvidenceStage") > 0 || uses_of(&signature, "EvidenceKind") > 0 {
                naming_depth.push(format!("{file}::{name}"));
            }
        }
    }
    assert!(
        !naming_axis.is_empty(),
        "no public signature names a column, so the scan above proves nothing"
    );
    assert!(
        !naming_depth.is_empty(),
        "no public signature names a depth, so the scan above proves nothing"
    );

    // And the scan does bite: the same predicate over a fragment that *does*
    // map one to the other finds it.
    const MAPPED: &str = "pub fn axis_of(stage: EvidenceStage) -> ReadinessAxis {";
    let caught = public_signatures(MAPPED)
        .into_iter()
        .filter(|(_, signature)| {
            uses_of(signature, "ReadinessAxis") > 0
                && uses_of(signature, "EvidenceStage") > 0
                && signature.contains("->")
        })
        .count();
    assert_eq!(
        caught, 1,
        "the scan does not recognise a map it should refuse"
    );
    Ok(())
}

/// No declared type holds both a cell and a band.
#[test]
fn a_cell_and_a_band_are_never_one_field() -> TestResult {
    for (owner, field, declared, _) in FIELDS {
        let both = (declared.contains("AxisCell") && declared.contains("Freshness"))
            || (declared.contains("FreshnessCell") && declared.contains("AxisCell"));
        assert!(
            !both,
            "{owner}.{field} is declared {declared}, which folds a reading into a band"
        );
    }

    // And no public function converts one into the other, in either direction.
    let mut converting: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            let Some(arrow) = signature.find("->") else {
                continue;
            };
            let (takes, gives) = signature.split_at(arrow);
            let cell_to_band = uses_of(takes, "AxisCell") > 0
                && (uses_of(gives, "FreshnessCell") > 0 || uses_of(gives, "FreshnessBand") > 0);
            let band_to_cell = (uses_of(takes, "FreshnessCell") > 0
                || uses_of(takes, "FreshnessBand") > 0)
                && uses_of(gives, "AxisCell") > 0;
            if cell_to_band || band_to_cell {
                converting.push(format!("{file}::{name}"));
            }
        }
    }
    assert!(
        converting.is_empty(),
        "these functions convert between a reading and a band: {converting:?}"
    );

    // A `pub fn` sweep cannot see a trait implementation: `fn from` inside an
    // `impl From<A> for B` is not public, and an injection that added
    // `impl From<FreshnessCell> for AxisCell` passed the whole suite. So the
    // whole set of `impl` headers is compared in both directions, which refuses
    // a conversion however it is spelled and whatever trait carries it.
    let mut found: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl ") {
                found.insert(collapse(trimmed));
            }
        }
    }
    assert_eq!(
        found,
        IMPL_HEADERS
            .iter()
            .map(|header| (*header).to_owned())
            .collect::<BTreeSet<_>>(),
        "an `impl` header arrived or left; a conversion between two of this crate's types is a \
         key here"
    );
    for header in IMPL_HEADERS {
        let converts_a_cell = header.contains("AxisCell")
            && (header.contains("Freshness") || header.contains("ReadinessAxis"));
        assert!(
            !converts_a_cell,
            "{header} implements a conversion between a reading and a band or a column"
        );
    }
    Ok(())
}

/// This scan file is in the policy-source-scan inventory.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for named in [
        "crates/readiness/tests/readiness_scans.rs",
        "crates/readiness/tests/readiness_matrix.rs",
    ] {
        assert!(
            page.contains(named),
            "{named} is not registered in the policy source scan inventory"
        );
    }
    Ok(())
}

/// The helpers this file rests on are not vacuous.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert!(
        !product_code()?.is_empty(),
        "the product walk found nothing, so every whole-set comparison above is empty"
    );
    assert!(
        crate_all_sources()?.len() > product_code()?.len(),
        "the all-sources walk does not reach the test tree"
    );
    let sample = "// a\nuse std::fs;\nlet path = \"fs::\";\n";
    let stripped = strip_non_code(sample);
    assert!(!stripped.contains("// a"), "comments survive stripping");
    assert_eq!(
        uses_of(&stripped, "fs"),
        1,
        "the literal was counted or the use item was not"
    );
    assert_eq!(uses_of("foo_bar baz", "bar"), 0, "a suffix was counted");
    assert_eq!(uses_of("bar baz", "bar"), 1, "a whole word was missed");
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
