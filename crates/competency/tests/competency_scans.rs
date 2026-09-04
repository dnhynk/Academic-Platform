//! What `academic-competency` may reach, hold and hand out.
//!
//! ## Why a token list is not enough, measured four tasks ago
//!
//! `P2-R2` shipped this crate's great-great-grandparent with a forbidden-token
//! list as its only net, and `docs/contracts/policy-source-scans.md` records
//! what that measured: seven spellings of a filesystem or environment reach
//! compile, spell none of the listed tokens, add no `use` item, and passed. The
//! repair was not a longer list. It was three **whole-set** comparisons, in both
//! directions:
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
//! * a **caller-supplied identifier** --- the text inside a competency, a
//!   criterion or a record identity, admitted by `identity::validated`;
//! * an **identifier a refusal echoes** --- the copy a `CompetencyError`
//!   carries so its message can name what it refused;
//! * a **closed vocabulary value** --- one of this crate's enumerations, or one
//!   of the predicate registry's;
//! * a **value of a reviewed crate** --- `P2-N2`'s admitted evidence, `P2-N1`'s
//!   entity identity;
//! * a **value of this crate**; and
//! * **the user's own words** --- a context, a performance criterion, a rubric
//!   row. Section 24.1 asks for all three by name and a competency a reader
//!   cannot read is not observable, so they are inventoried rather than
//!   refused, and no `Debug` here reduces one to a length.
//!
//! There is no seventh, and in particular there is no byte buffer and no
//! timestamp: no field of this crate is declared `u8` under any name, which is
//! what the inventory says without consulting a list of names, and nothing here
//! records when anything happened, which is what makes *freshness is `P2-N3`'s*
//! a property of the whole crate.
//!
//! ## Two rules of this task that are pinned rather than described
//!
//! `the_statement_is_rendered_rather_than_stored` pins
//! `Competency::statement`'s whole text and requires `declare` to take five
//! arguments with no sentence among them, because section 7.1's refusal of
//! `knows X` is exactly the absence of a place to write one.
//!
//! `the_join_has_no_second_key` pins `fill`'s matching condition and
//! `PerformanceCriterion::is_about`, and requires `sheet.rs` to name
//! `enabled_by` **zero** times. `P2-R5` measured the defect this closes one
//! layer down: a join that fell back to a weaker key when the strong one was
//! absent credited a user with a library they had not used.
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

/// Collapses whitespace so a rewrapped signature still matches the pin that
/// it closes.
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

/// Every `use` item of a product file that is not a `crate::` item, with the
/// file it is in.
const USE_ITEMS: [(&str, &str); 10] = [
    (
        "crates/competency/src/criterion.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    ("crates/competency/src/enabling.rs", "use serde::Serialize;"),
    (
        "crates/competency/src/evidence.rs",
        "use academic_domain::{EntityId, EvidenceId};",
    ),
    (
        "crates/competency/src/evidence.rs",
        "use academic_knowledge_state::{EligibleEvidence, EvidenceCeiling, EvidenceKind};",
    ),
    (
        "crates/competency/src/evidence.rs",
        "use academic_repository_competency::{ClaimStanding, PersonalApplicationClaim};",
    ),
    ("crates/competency/src/evidence.rs", "use serde::Serialize;"),
    (
        "crates/competency/src/identity.rs",
        "use academic_domain::EntityId;",
    ),
    (
        "crates/competency/src/identity.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    ("crates/competency/src/sheet.rs", "use serde::Serialize;"),
    (
        "crates/competency/src/stage.rs",
        "use serde::{Deserialize, Serialize};",
    ),
];

/// The `use` items that reach inside this crate, and the two `lib.rs` items
/// that are not re-exports.
const CRATE_IMPORTS: [(&str, &str); 8] = [
    (
        "crates/competency/src/criterion.rs",
        "use crate::{ CompetencyError, identity::{ConceptRef, CriterionId, non_empty}, };",
    ),
    (
        "crates/competency/src/enabling.rs",
        "use crate::{ Competency, criterion::{ContributionImportance, Necessity}, \
         identity::{CompetencyId, ConceptRef}, };",
    ),
    (
        "crates/competency/src/evidence.rs",
        "use crate::{ CompetencyError, identity::{ConceptRef, RecordId}, stage::EvidenceStage, };",
    ),
    (
        "crates/competency/src/identity.rs",
        "use crate::CompetencyError;",
    ),
    (
        "crates/competency/src/rubric.rs",
        "use crate::{ CompetencyError, identity::{CriterionId, non_empty}, stage::EvidenceStage, };",
    ),
    (
        "crates/competency/src/sheet.rs",
        "use crate::{ Competency, evidence::StageEvidence, identity::{CompetencyId, CriterionId}, \
         stage::EvidenceStage, };",
    ),
    (
        "crates/competency/src/lib.rs",
        "use std::{collections::BTreeSet, fmt};",
    ),
    (
        "crates/competency/src/lib.rs",
        "use academic_domain::predicates::{NodeType, PredicateName};",
    ),
];

/// The `use` items of `rubric.rs` and `lib.rs` that are neither of the above.
const SERDE_IMPORTS: [(&str, &str); 2] = [
    (
        "crates/competency/src/rubric.rs",
        "use serde::{Deserialize, Serialize};",
    ),
    (
        "crates/competency/src/lib.rs",
        "use serde::{Deserialize, Serialize};",
    ),
];

/// Everything `lib.rs` hands out, from the seven modules it declares.
///
/// A `pub use` hands a name out rather than reaching for one, so it is
/// inventoried separately: a module added without a re-export, or a re-export
/// from a module that is not one of the seven, is a difference here.
const RE_EXPORTS: [&str; 7] = [
    "pub use criterion::{ ContributionImportance, EnablingConcept, Necessity, \
     PerformanceCriterion, Situation, };",
    "pub use enabling::{EnablingEdge, EnablingGraph};",
    "pub use evidence::{EvidenceOrigin, EvidenceSource, PromotingEvidence, StageEvidence};",
    "pub use identity::{CompetencyId, ConceptNamespace, ConceptRef, CriterionId, RecordId};",
    "pub use rubric::{EvidenceRubric, RubricRow};",
    "pub use sheet::{CellState, RubricCell, RubricSheet, fill};",
    "pub use stage::EvidenceStage;",
];

/// Every two-segment path this crate spells through a crate root, with why.
const REACHED_PATHS: [(&str, &str); 1] = [(
    "thiserror::Error",
    "the derive on CompetencyError, which is the workspace's error vocabulary",
)];

/// Every macro this crate invokes, with why.
const MACROS_SPELLED: [(&str, &str); 4] = [
    (
        "format",
        "the rendered statement's witness rows and the cell's rubric text",
    ),
    ("matches", "two total reads of a closed enumeration"),
    ("vec", "the fill pass's per-record matched flags"),
    ("write", "the Display of a rendered statement"),
];

/// The files of this package permitted to read anything.
///
/// Two: this scan, and the acceptance suite, which reads the design document so
/// that section 24.1's example and section 24.3's stage list are measured rather
/// than restated. Both are named on `docs/contracts/policy-source-scans.md`.
const READERS: [&str; 2] = ["competency_scans.rs", "competency_model.rs"];

/// The shapes a reader expects to see refused, as the third and weakest layer.
///
/// `Instant`, `SystemTime` and `now` are here beside the filesystem and
/// transport names because *this engine cannot ask what time it is* is what
/// keeps freshness `P2-N3`'s and mastery `P2-N2`'s.
const FORBIDDEN_CONSTRUCTS: [&str; 15] = [
    "fs::",
    "File",
    "OpenOptions",
    "Command",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "UnixStream",
    "socket",
    "env",
    "include_str",
    "Instant",
    "SystemTime",
    "now",
    "unsafe",
];

/// The six reasons a field may exist.
const REASONS: [&str; 6] = [
    "a caller-supplied identifier",
    "an identifier a refusal echoes",
    "a closed vocabulary value",
    "a value of a reviewed crate",
    "a value of this crate",
    "the user's own words",
];

/// The names whose call sites are counted, with the count and the one file.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 2] = [
    (
        "is_about",
        1,
        "crates/competency/src/sheet.rs",
        "the one place a record is joined to a criterion; a second caller would be a second join",
    ),
    (
        "ceiling",
        1,
        "crates/competency/src/evidence.rs",
        "the one place section 13.2 is asked whether a row licenses a promotion",
    ),
];

/// The tuple structs this crate declares, which have no named field for
/// [`type_fields`] to report, and what each holds.
const TUPLE_STRUCTS: [(&str, &str, &str); 4] = [
    (
        "CompetencyId",
        "pub struct CompetencyId(String);",
        "a caller-supplied identifier",
    ),
    (
        "CriterionId",
        "pub struct CriterionId(String);",
        "a caller-supplied identifier",
    ),
    (
        "RecordId",
        "pub struct RecordId(String);",
        "a caller-supplied identifier",
    ),
    (
        "Situation",
        "pub struct Situation(String);",
        "the user's own words",
    ),
];

/// The enumerations whose variants carry unnamed values, pinned whole.
const TUPLE_ENUMS: [(&str, &str); 3] = [
    ("src/identity.rs", "pub enum ConceptRef {"),
    ("src/evidence.rs", "pub enum EvidenceSource {"),
    ("src/sheet.rs", "pub enum CellState {"),
];

const FIELDS: [(&str, &str, &str, &str); 55] = [
    (
        "CriterionWire",
        "about",
        "Vec<ConceptRef>",
        "a value of this crate",
    ),
    (
        "CriterionWire",
        "id",
        "CriterionId",
        "a value of this crate",
    ),
    (
        "CriterionWire",
        "requirement",
        "String",
        "the user's own words",
    ),
    (
        "EnablingConcept",
        "concept",
        "ConceptRef",
        "a value of this crate",
    ),
    (
        "EnablingConcept",
        "importance",
        "ContributionImportance",
        "a closed vocabulary value",
    ),
    (
        "EnablingConcept",
        "necessity",
        "Necessity",
        "a closed vocabulary value",
    ),
    (
        "PerformanceCriterion",
        "about",
        "Vec<ConceptRef>",
        "a value of this crate",
    ),
    (
        "PerformanceCriterion",
        "id",
        "CriterionId",
        "a value of this crate",
    ),
    (
        "PerformanceCriterion",
        "requirement",
        "String",
        "the user's own words",
    ),
    (
        "EnablingEdge",
        "competency",
        "CompetencyId",
        "a value of this crate",
    ),
    (
        "EnablingEdge",
        "concept",
        "ConceptRef",
        "a value of this crate",
    ),
    (
        "EnablingEdge",
        "importance",
        "ContributionImportance",
        "a closed vocabulary value",
    ),
    (
        "EnablingEdge",
        "necessity",
        "Necessity",
        "a closed vocabulary value",
    ),
    (
        "EnablingGraph",
        "edges",
        "Vec<EnablingEdge>",
        "a value of this crate",
    ),
    (
        "PromotingEvidence",
        "inner",
        "EligibleEvidence",
        "a value of a reviewed crate",
    ),
    (
        "StageEvidence",
        "concept",
        "ConceptRef",
        "a value of this crate",
    ),
    ("StageEvidence", "id", "RecordId", "a value of this crate"),
    (
        "StageEvidence",
        "source",
        "EvidenceSource",
        "a value of this crate",
    ),
    (
        "StageEvidence",
        "stage",
        "EvidenceStage",
        "a closed vocabulary value",
    ),
    (
        "Competency",
        "context",
        "Situation",
        "a value of this crate",
    ),
    (
        "Competency",
        "criteria",
        "Vec<PerformanceCriterion>",
        "a value of this crate",
    ),
    (
        "Competency",
        "enabled_by",
        "Vec<EnablingConcept>",
        "a value of this crate",
    ),
    ("Competency", "id", "CompetencyId", "a value of this crate"),
    (
        "Competency",
        "rubric",
        "EvidenceRubric",
        "a value of this crate",
    ),
    (
        "CompetencyError::CriterionHasNoRubricRow",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::CriterionHasNoRubricRow",
        "criterion",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::CriterionNamesUnenablingConcept",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::CriterionNamesUnenablingConcept",
        "criterion",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::DuplicateCriterion",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::DuplicateCriterion",
        "criterion",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::RubricRowNamesUnknownCriterion",
        "competency",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyError::RubricRowNamesUnknownCriterion",
        "criterion",
        "String",
        "an identifier a refusal echoes",
    ),
    (
        "CompetencyStatement",
        "performances",
        "Vec<String>",
        "the user's own words",
    ),
    (
        "CompetencyStatement",
        "situation",
        "String",
        "the user's own words",
    ),
    (
        "CompetencyStatement",
        "witnesses",
        "Vec<String>",
        "the user's own words",
    ),
    (
        "CompetencyWire",
        "context",
        "Situation",
        "a value of this crate",
    ),
    (
        "CompetencyWire",
        "enabled_by_concepts",
        "Vec<EnablingConcept>",
        "a value of this crate",
    ),
    (
        "CompetencyWire",
        "evidence_rubric",
        "EvidenceRubric",
        "a value of this crate",
    ),
    (
        "CompetencyWire",
        "id",
        "CompetencyId",
        "a value of this crate",
    ),
    (
        "CompetencyWire",
        "performance_criteria",
        "Vec<PerformanceCriterion>",
        "a value of this crate",
    ),
    (
        "CompetencyWire",
        "statement",
        "String",
        "the user's own words",
    ),
    (
        "EvidenceRubric",
        "rows",
        "Vec<RubricRow>",
        "a value of this crate",
    ),
    ("RubricRow", "admits", "String", "the user's own words"),
    (
        "RubricRow",
        "criterion",
        "CriterionId",
        "a value of this crate",
    ),
    (
        "RubricRow",
        "stage",
        "EvidenceStage",
        "a closed vocabulary value",
    ),
    ("RubricRowWire", "admits", "String", "the user's own words"),
    (
        "RubricRowWire",
        "criterion",
        "CriterionId",
        "a value of this crate",
    ),
    (
        "RubricRowWire",
        "stage",
        "EvidenceStage",
        "a closed vocabulary value",
    ),
    (
        "RubricCell",
        "admits",
        "Option<String>",
        "the user's own words",
    ),
    (
        "RubricCell",
        "criterion",
        "CriterionId",
        "a value of this crate",
    ),
    (
        "RubricCell",
        "stage",
        "EvidenceStage",
        "a closed vocabulary value",
    ),
    ("RubricCell", "state", "CellState", "a value of this crate"),
    (
        "RubricSheet",
        "cells",
        "Vec<RubricCell>",
        "a value of this crate",
    ),
    (
        "RubricSheet",
        "competency",
        "CompetencyId",
        "a value of this crate",
    ),
    (
        "RubricSheet",
        "unmatched",
        "Vec<StageEvidence>",
        "a value of this crate",
    ),
];

// ---------------------------------------------------------------------------
// The scans.
// ---------------------------------------------------------------------------

/// The walk descends into this package rather than reading one flat directory.
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
        declared.len() >= 7,
        "this crate declares {} modules; the walk would be reading almost nothing",
        declared.len()
    );
    assert!(
        crate_product_sources()?.len() >= 8,
        "the product walk found too few files to be reading this package"
    );
    Ok(())
}

/// The three whole-set comparisons, and then the token pass.
#[test]
fn the_competency_crate_touches_no_file_and_no_socket() -> TestResult {
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
    let lib = "crates/competency/src/lib.rs";
    let expected: BTreeSet<(String, String)> = USE_ITEMS
        .iter()
        .chain(CRATE_IMPORTS.iter())
        .chain(SERDE_IMPORTS.iter())
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
        "this crate's `use` set changed; a filesystem or transport import is an extra key here"
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
            let permitted = (forbidden == "fs::" || forbidden == "env")
                && READERS
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
         holds, and a field holding a clock reading or a byte buffer does not get one"
    );

    let admitted: BTreeSet<&str> = REASONS.into_iter().collect();
    for (owner, field, declared, reason) in FIELDS {
        assert!(
            admitted.contains(reason),
            "{owner}.{field} is described as {reason:?}, which is not one of the six"
        );
        assert!(
            !declared.contains("u8"),
            "{owner}.{field} is declared {declared}, which is a byte buffer"
        );
        assert!(
            !declared.contains("u64") && !declared.contains("Instant"),
            "{owner}.{field} is declared {declared}, which could be a clock reading"
        );
    }

    // The user's own prose lives in exactly these places. A seventh is a new
    // place for text to arrive and is visible here rather than in a diff.
    let words: Vec<String> = FIELDS
        .into_iter()
        .filter(|(_, _, _, reason)| *reason == "the user's own words")
        .map(|(owner, field, _, _)| format!("{owner}.{field}"))
        .collect();
    assert_eq!(
        words,
        vec![
            "CriterionWire.requirement".to_owned(),
            "PerformanceCriterion.requirement".to_owned(),
            "CompetencyStatement.performances".to_owned(),
            "CompetencyStatement.situation".to_owned(),
            "CompetencyStatement.witnesses".to_owned(),
            "CompetencyWire.statement".to_owned(),
            "RubricRow.admits".to_owned(),
            "RubricRowWire.admits".to_owned(),
            "RubricCell.admits".to_owned(),
        ],
        "the places this crate holds the user's own prose have changed"
    );
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

    let identity = collapse(&strip_non_code(&source_of(
        &crate_root().join("src/identity.rs"),
    )?));
    let criterion = collapse(&strip_non_code(&source_of(
        &crate_root().join("src/criterion.rs"),
    )?));
    for (name, pin, reason) in TUPLE_STRUCTS {
        assert!(
            identity.contains(pin) || criterion.contains(pin),
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
        assert!(
            !block.contains("u8"),
            "{header} carries a byte buffer in an unnamed field"
        );
    }
    Ok(())
}

/// No public function edits a value in place.
#[test]
fn no_public_function_mutates_in_place() -> TestResult {
    let mut mutating: Vec<String> = Vec::new();
    for (file, code) in product_code()? {
        for (name, signature) in public_signatures(&code) {
            if signature.contains("&mut self") {
                mutating.push(format!("{file}::{name}"));
            }
        }
    }
    assert_eq!(
        mutating,
        Vec::<String>::new(),
        "`CONTRIBUTING.md` rule 2 is append-only and a correction is a new event; a public \
         function that takes `&mut self` is an edit in place"
    );
    Ok(())
}

/// Section 7.1's refusal of `knows X`, as the absence of a place to write one.
#[test]
fn the_statement_is_rendered_rather_than_stored() -> TestResult {
    let lib = strip_non_code(&source_of(&crate_root().join("src/lib.rs"))?);

    // `declare` takes five arguments and none of them is a sentence.
    let signature = public_signatures(&lib)
        .into_iter()
        .find(|(name, _)| name == "declare")
        .map(|(_, signature)| collapse(&signature))
        .ok_or("lib.rs declares no `declare`")?;
    assert_eq!(
        signature,
        "pub fn declare( id: CompetencyId, context: Situation, criteria: \
         Vec<PerformanceCriterion>, enabled_by: Vec<EnablingConcept>, rubric: EvidenceRubric, ) \
         -> Result<Competency, CompetencyError>",
        "`declare`'s argument list changed"
    );

    // And the statement is rendered from those parts, pinned whole.
    let rendered = whole_block(&lib, "pub fn statement(&self) -> CompetencyStatement {")?;
    assert_eq!(
        rendered,
        "pub fn statement(&self) -> CompetencyStatement { CompetencyStatement { situation: \
         self.context.as_str().to_owned(), performances: self .criteria .iter() .map(|criterion| \
         criterion.requirement().to_owned()) .collect(), witnesses: self .rubric .rows() .iter() \
         .map(|row| format!( , row.stage().as_str(), row.admits())) .collect(), } }",
        "`Competency::statement` no longer renders the sentence from the parts. The pin is \
         over stripped code, so the format string is not in it; that literal is pinned by \
         `competency_observability`, which compares the rendered sentence against section \
         24.1's own parts"
    );

    // The only field named `statement` in this crate is the wire type's, which
    // is written by `From<Competency>` and compared by `TryFrom` rather than
    // read as an input.
    let carriers: Vec<String> = FIELDS
        .into_iter()
        .filter(|(_, field, _, _)| *field == "statement")
        .map(|(owner, field, _, _)| format!("{owner}.{field}"))
        .collect();
    assert_eq!(carriers, vec!["CompetencyWire.statement".to_owned()]);
    assert!(
        lib.contains("if competency.statement().to_string() == wire.statement"),
        "the deserialized statement is no longer compared against the rendered one"
    );
    Ok(())
}

/// Section 24.3's first sentence, as a join with one key.
#[test]
fn the_join_has_no_second_key() -> TestResult {
    let sheet = strip_non_code(&source_of(&crate_root().join("src/sheet.rs"))?);
    let criterion = strip_non_code(&source_of(&crate_root().join("src/criterion.rs"))?);

    // The fill pass cannot reach the competency's enabling set at all, which is
    // the fallback section 24.3 refuses.
    assert_eq!(
        uses_of(&sheet, "enabled_by"),
        0,
        "the fill pass names the competency's enabling set; that is the weaker key"
    );
    assert_eq!(
        uses_of(&sheet, "about"),
        0,
        "the fill pass reads a criterion's concepts directly instead of asking it"
    );

    // The matching condition, pinned whole.
    assert!(
        sheet.contains("if record.stage() == stage && criterion.is_about(record.concept()) {"),
        "`fill`'s matching condition changed"
    );

    // And the one comparison it delegates to, pinned whole.
    let membership = whole_block(
        &criterion,
        "pub fn is_about(&self, concept: &ConceptRef) -> bool {",
    )?;
    assert_eq!(
        membership,
        "pub fn is_about(&self, concept: &ConceptRef) -> bool { self.about.contains(concept) }",
        "`PerformanceCriterion::is_about` no longer compares the whole pair"
    );

    // A criterion that names no concept has no representation, so there is no
    // case for a fallback to serve.
    assert!(
        criterion.contains("return Err(CompetencyError::CriterionNamesNoConcept("),
        "a criterion may now name no concept"
    );
    Ok(())
}

/// `P2-R5`'s two claims stay two, and only one of them reaches this crate.
#[test]
fn no_product_file_names_a_project_observation_claim() -> TestResult {
    for (file, code) in product_code()? {
        assert_eq!(
            uses_of(&code, "ProjectObservationClaim"),
            0,
            "{file} names `ProjectSnapshot OBSERVES Concept`; only the personal claim founds a cell"
        );
        assert_eq!(
            uses_of(&code, "ProjectProvenance"),
            0,
            "{file} names the project claim's provenance"
        );
    }
    let evidence = strip_non_code(&source_of(&crate_root().join("src/evidence.rs"))?);
    assert_eq!(
        uses_of(&evidence, "PersonalApplicationClaim"),
        2,
        "the personal claim is named by the import and by the one door that takes it"
    );
    Ok(())
}

/// The guarded names, each with its counted call sites in its one file.
#[test]
fn each_guarded_name_has_exactly_its_call_sites() -> TestResult {
    for (name, expected, file, reason) in CALL_SITE_COUNTS {
        let mut total = 0;
        for (path, code) in product_code()? {
            let calls = calls_of(&code, name);
            if calls > 0 {
                assert_eq!(path, file, "{name} is called from {path}: {reason}");
            }
            total += calls;
        }
        assert_eq!(total, expected, "{name} has {total} call sites: {reason}");
    }
    Ok(())
}

/// Both files of this package that read anything are on the inventory page.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for name in [
        "crates/competency/tests/competency_scans.rs",
        "crates/competency/tests/competency_model.rs",
    ] {
        assert!(
            page.contains(name),
            "{name} reads Rust source or the design document and is not on the inventory page"
        );
    }
    Ok(())
}

/// The helpers this file's claims rest on, exercised against known answers.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(collapse("  a   b \n c "), "a b c");
    assert_eq!(uses_of("a_fs_b fs fs::x", "fs"), 2);
    assert_eq!(uses_of("no match here", "fs"), 0);
    assert_eq!(
        absolute_paths("thiserror::Error and std::mem::swap"),
        ["std::mem".to_owned(), "thiserror::Error".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        macros_spelled("matches!(a, b) and include_str!(\"x\")"),
        ["include_str".to_owned(), "matches".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        type_fields("pub struct A { pub b: Vec<u8>, c: u64, }"),
        vec![
            ("A".to_owned(), "b".to_owned(), "Vec<u8>".to_owned()),
            ("A".to_owned(), "c".to_owned(), "u64".to_owned()),
        ]
    );
    assert_eq!(calls_of("x.judge(&a); judge(b); judged(c);", "judge"), 2);
    assert_eq!(declarations_of("fn judge(x: u32) {}", "judge"), 1);
    assert!(whole_block("impl A { fn b() {} }", "impl A {").is_ok());
    assert!(whole_block("impl A { fn b() {} }", "impl Z {").is_err());
    assert_eq!(
        public_signatures("pub fn a(x: u8) -> u8 {\n").len(),
        1,
        "the signature extractor finds a signature it is given"
    );

    // The two forbidden-token controls: the pass really does find a name when
    // one is there, so the zeros above are measurements.
    assert!(uses_of("let file = File::open(p);", "File") > 0);
    assert!(uses_of("let t = SystemTime::now();", "now") > 0);

    // And the walk really reads this package rather than an empty directory.
    assert!(crate_product_sources()?.len() >= 8);
    assert!(crate_all_sources()?.len() >= 10);
    assert!(!product_code()?.is_empty());
    Ok(())
}
